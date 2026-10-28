//! `GET /api/els` and `GET /api/els/stream`: equidistant letter sequences,
//! the twins of `src/wasm/els.rs`. All run `ethaor::shared::els`; these
//! answer JSON — a page, or every listed hit as a stream of lines.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use axum::Extension;
use axum::body::{Body, Bytes};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State as Shared};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use ethaor::shared::els::{self, CAP, DEFAULT_LIMIT, MAX_LIMIT, Page, Refusal};
use ethaor::shared::reference;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::wrappers::ReceiverStream;

use super::guard::{Client, rate_limited};
use super::pool::{Failure, Pool};
use super::{PUBLIC, refuse};
use crate::config::Config;
use crate::fault::{Fault, Kind};

/// The engines, the answers already given, and the heaviest search allowed.
///
/// The corpus is frozen, so an answer is the same for everyone forever: a
/// query asked again — by anyone — is served from `answers` without an
/// engine, and the same query asked by many at once is searched once while
/// the others await it.
#[derive(Clone)]
pub struct State {
    pub pool: Arc<Pool>,
    answers: Cache<els::Query, Bytes>,
    max_cost_ms: f64,
}

impl State {
    pub fn new(config: &Config) -> State {
        State {
            pool: Arc::new(Pool::new(config.engines, config.wait)),
            answers: Cache::builder()
                .weigher(|_: &els::Query, v: &Bytes| v.len().min(u32::MAX as usize) as u32)
                .max_capacity(config.cache_bytes)
                .build(),
            max_cost_ms: config.max_cost_ms,
        }
    }
}

/// The query string. Unknown names are refused rather than ignored: a typo
/// would otherwise search something else without a word.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub word: String,
    /// Exact letters (final forms): `ם` and `מ` told apart.
    #[serde(default)]
    pub exact: bool,
    #[serde(default)]
    pub skip_min: u32,
    /// `skip_min` when absent: one skip.
    pub skip_max: Option<u32>,
    #[serde(default)]
    pub offset: usize,
    pub limit: Option<usize>,
    /// The scope, one way at most.
    pub portion: Option<String>,
    pub book: Option<String>,
    pub chapter: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Params {
    pub fn query(&self, default_limit: usize) -> Result<els::Query, Refusal> {
        let scope = els::scope(
            self.portion.as_deref(),
            self.book.as_deref(),
            self.chapter.as_deref(),
            self.from.as_deref(),
            self.to.as_deref(),
        )?;
        Ok(els::Query {
            word: self.word.clone(),
            exact: self.exact,
            skip_min: self.skip_min,
            skip_max: self.skip_max.unwrap_or(self.skip_min),
            offset: self.offset,
            limit: self.limit.unwrap_or(default_limit),
            scope,
        })
    }
}

/// Every code these endpoints may refuse with.
pub const ERRORS: [&str; 12] = [
    "bad_param",
    "no_hebrew_letter",
    "skip_range",
    "limit",
    "unknown_reference",
    "scope_conflict",
    "empty_scope",
    "too_costly",
    "rate_limited",
    "busy",
    "text_missing",
    "internal",
];

/// An OSIS place, as a JSON Schema pattern: one of the 39 codes, then its
/// chapter and verse, each optional, numbers from 1 with no leading zero.
pub fn place_pattern() -> String {
    osis_pattern(0)
}

/// An OSIS chapter, or a verse naming its chapter.
pub fn chapter_pattern() -> String {
    osis_pattern(1)
}

fn osis_pattern(least: usize) -> String {
    format!(r"^({})(\.[1-9][0-9]*){{{least},2}}$", osis_codes().join("|"))
}

/// The 39 books' OSIS codes: the one way a caller names a book.
pub fn osis_codes() -> Vec<&'static str> {
    reference::BOOKS.iter().map(|b| b.osis).collect()
}

/// The parameters both endpoints take, as a JSON Schema.
fn schema(max_limit: usize, default_limit: usize) -> Value {
    json!({
        "type": "object",
        "properties": {
            "word": {
                "type": "string",
                "description": "Hebrew word; vowel points and non-Hebrew characters are ignored.",
            },
            "exact": {
                "type": "boolean", "default": false,
                "description": "Exact letters (final forms): a final form matches only itself — ם a final \
                    mem, מ a non-final one. Off, final forms are folded (ם = מ).",
            },
            "skip_min": {
                "type": "integer", "minimum": 0, "default": 0,
                "description": "Smallest number of letters skipped between two letters of the word.",
            },
            "skip_max": {
                "type": "integer", "minimum": 0,
                "description": "Largest skip, inclusive; defaults to skip_min (a single skip).",
            },
            "offset": {
                "type": "integer", "minimum": 0, "default": 0,
                "description": "Index of the first hit returned; pass the previous answer's next_offset.",
            },
            "limit": {
                "type": "integer", "minimum": 0, "maximum": max_limit, "default": default_limit,
                "description": "Hits returned at most; 0 only counts.",
            },
            "portion": {
                "type": "string", "enum": ["torah", "neviim", "ketuvim"],
                "description": "Search only one portion of the Tanakh.",
            },
            "book": {
                "type": "string", "enum": osis_codes(),
                "description": "Search only one book, by its OSIS code.",
            },
            "chapter": {
                "type": "string", "pattern": chapter_pattern(),
                "description": "Search only one chapter, in OSIS: Gen.1, Ps.119.",
            },
            "from": {
                "type": "string", "pattern": place_pattern(),
                "description": "Search from this book, chapter or verse on, in OSIS (Gen.12, Gen.12.1, \
                    Exod); with `to`, a stretch of passages, both ends included.",
            },
            "to": {
                "type": "string", "pattern": place_pattern(),
                "description": "Search up to this book, chapter or verse, included, in OSIS (Deut, \
                    Deut.34.12).",
            },
        },
        "required": ["word"],
        "additionalProperties": false,
    })
}

const SEARCH: &str = "Equidistant letter sequence search: finds a Hebrew word spelled across the Tanakh by \
    taking one letter, skipping `skip` letters, taking the next, and so on. Skip 0 is the word as written. \
    Only consonants count (no spaces, vowel points or verse breaks); final forms are folded (ם = מ) unless \
    `exact` tells them apart. The \
    search may be kept to a portion, a book, a chapter or a stretch from one passage to another: an \
    occurrence then counts only if all its letters lie there. Hits come in (skip, start) order; `total` is \
    exact even when only the first `listed` can be paged through. A search estimated too heavy is refused \
    (`too_costly`): narrow the skips or the scope.";

/// The tools as `GET /api` lists them: what an MCP server would declare.
pub fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "els_search",
            "method": "GET",
            "path": "/api/els",
            "description": format!("{SEARCH} Answers one page of hits."),
            "input_schema": schema(MAX_LIMIT, DEFAULT_LIMIT),
            "errors": ERRORS,
        }),
        json!({
            "name": "els_stream",
            "method": "GET",
            "path": "/api/els/stream",
            "description": format!(
                "{SEARCH} Streams every listed hit, up to {CAP}, as newline-delimited JSON \
                 (application/x-ndjson): a first line with the query, total and listed counts, one line per \
                 hit, and a last line {{\"done\": true}}."
            ),
            "input_schema": schema(CAP, CAP),
            "errors": ERRORS,
        }),
    ]
}

/// A verse, named every way.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct VerseRef {
    /// `Gen.1.1` (`Gen.1.1-Gen.1.2` for a joined verse).
    pub osis: String,
    /// `Genesis 1:1`.
    #[serde(rename = "ref")]
    pub human: String,
    /// `בראשית א,א`.
    pub hebrew: String,
    pub book: &'static str,
    pub book_osis: &'static str,
    pub book_hebrew: &'static str,
    pub chapter: u32,
    pub verse: u32,
    pub verse_last: u32,
    /// Global verse index, 0-based.
    pub index: usize,
}

impl VerseRef {
    pub fn of(v: &reference::Verse) -> VerseRef {
        VerseRef {
            osis: v.osis(),
            human: v.human(),
            hebrew: v.hebrew(),
            book: v.book_english(),
            book_osis: v.book_osis(),
            book_hebrew: v.book_hebrew(),
            chapter: v.chapter_number,
            verse: v.verse_number,
            verse_last: v.verse_last,
            index: v.verse,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Letter {
    pub char: char,
    /// The verse holding it, OSIS.
    pub osis: String,
    /// Its rank among that verse's consonants, 0-based.
    pub position: u32,
    /// Its rank among every consonant of the Tanakh, 0-based.
    pub rank: u32,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Hit {
    pub skip: u32,
    /// Distance between two letters: skip + 1.
    pub step: u64,
    /// Verse of the first letter, and of the last.
    pub from: VerseRef,
    pub to: VerseRef,
    pub letters: Vec<Letter>,
}

impl Letter {
    pub fn of(l: &els::Letter) -> Letter {
        Letter { char: l.char, osis: reference::verse(l.verse).osis(), position: l.position, rank: l.rank }
    }
}

impl Hit {
    pub fn of(hit: &els::Hit) -> Hit {
        Hit {
            skip: hit.skip,
            step: hit.skip as u64 + 1,
            from: VerseRef::of(&hit.first_verse()),
            to: VerseRef::of(&hit.last_verse()),
            letters: hit.letters.iter().map(Letter::of).collect(),
        }
    }
}

/// The verses a search was kept to.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Scope {
    /// First and last verse.
    pub from: VerseRef,
    pub to: VerseRef,
    pub verses: usize,
    /// Consonants in the scope: the stretch the search read.
    pub letters: usize,
}

impl Scope {
    pub fn of(scope: Option<(usize, usize)>) -> Option<Scope> {
        let (first, end) = scope?;
        Some(Scope {
            from: VerseRef::of(&reference::verse(first)),
            to: VerseRef::of(&reference::verse(end - 1)),
            verses: end - first,
            letters: els::letters_of(scope).len(),
        })
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Echo {
    pub word: String,
    /// The word as searched: consonants only, finals folded unless exact.
    pub letters: String,
    pub exact: bool,
    pub skip_min: u32,
    pub skip_max: u32,
    pub offset: usize,
    pub limit: usize,
    /// Null for the whole Tanakh.
    pub scope: Option<Scope>,
}

impl Echo {
    fn of(query: &els::Query, letters: &str) -> Echo {
        Echo {
            word: query.word.clone(),
            letters: letters.into(),
            exact: query.exact,
            skip_min: query.skip_min,
            skip_max: query.skip_max,
            offset: query.offset,
            limit: query.limit,
            scope: Scope::of(query.scope),
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Answer {
    pub query: Echo,
    /// Every hit, counted.
    pub total: u64,
    /// Hits that can be paged through; `total` may exceed it.
    pub listed: usize,
    pub returned: usize,
    /// Offset of the next page, or null when none remains.
    pub next_offset: Option<usize>,
    pub hits: Vec<Hit>,
}

impl Answer {
    pub fn of(query: &els::Query, page: &Page) -> Answer {
        let hits = page.hits.iter().map(Hit::of).collect::<Vec<_>>();
        Answer {
            query: Echo::of(query, &page.letters),
            total: page.total,
            listed: page.listed,
            returned: hits.len(),
            next_offset: page.next_offset(),
            hits,
        }
    }
}

/// The HTTP status a refusal answers with: the caller's mistake, or the
/// text not there yet.
pub fn status(refusal: &Refusal) -> StatusCode {
    match refusal {
        Refusal::TextMissing => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_REQUEST,
    }
}

/// Every failure as the caller reads it — never a bare status.
pub fn failure(failure: &Failure) -> Response {
    failure_while(failure, "answering an ELS search")
}

/// `failure`, a fault of ours escalated with what was being answered.
pub fn failure_while(failure: &Failure, what: &str) -> Response {
    match failure {
        Failure::Refused(refusal) => refuse(status(refusal), refusal.code(), refusal.message()),
        Failure::RateLimited(wait, client) => rate_limited(*wait, *client),
        Failure::Busy => {
            let message = "every engine is searching; retry shortly";
            Fault::new(Kind::Unavailable, "busy", StatusCode::SERVICE_UNAVAILABLE, message)
                .retry_after(std::time::Duration::from_secs(1))
                .respond()
        }
        Failure::Fault(fault) => (**fault).clone().escalate(what).respond(),
    }
}

/// Reads and checks a request, up to its search: the query, or the answer
/// that refuses it — a bad query costs neither an engine nor a cache slot.
fn read(
    params: Result<Query<Params>, QueryRejection>,
    max_limit: usize,
    default_limit: usize,
) -> Result<els::Query, Box<Response>> {
    let params = params.map_err(|e| Box::new(refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text())))?.0;
    let refused = |r: Refusal| Box::new(refuse(status(&r), r.code(), r.message()));
    let query = params.query(default_limit).map_err(refused)?;
    els::check_up_to(&query, max_limit).map_err(refused)?;
    Ok(query)
}

/// Admits a search on its estimate and charges it to its client: run on the
/// engine, before the search itself.
fn admit(engine: &mut els::Engine, query: &els::Query, max_cost_ms: f64, client: Option<&Client>) -> Result<f64, Failure> {
    let estimate = els::admit(engine, query, max_cost_ms).map_err(Failure::Refused)?;
    if let Some(client) = client
        && let Err(wait) = client.guard.charge(client.ip, estimate)
    {
        return Err(Failure::RateLimited(wait, client.ip));
    }
    Ok(estimate)
}

pub async fn search(
    Shared(state): Shared<State>,
    client: Option<Extension<Client>>,
    params: Result<Query<Params>, QueryRejection>,
) -> Response {
    let query = match read(params, MAX_LIMIT, DEFAULT_LIMIT) {
        Ok(query) => query,
        Err(refused) => return *refused,
    };
    let (pool, max_cost_ms) = (state.pool.clone(), state.max_cost_ms);
    let asked = query.clone();
    let client = client.map(|c| c.0);
    let answer = state
        .answers
        .try_get_with(query, async move {
            let page = pool
                .run(move |engine| {
                    let started = Instant::now();
                    let estimate_ms = admit(engine, &asked, max_cost_ms, client.as_ref())?;
                    let page = els::search(engine, &asked).map_err(Failure::Refused)?;
                    tracing::info!(
                        letters = %page.letters,
                        total = page.total,
                        estimate_ms,
                        search_ms = started.elapsed().as_secs_f64() * 1e3,
                        "searched"
                    );
                    serde_json::to_vec(&Answer::of(&asked, &page)).map_err(|e| Failure::ours("answer_unwritten", e.to_string()))
                })
                .await??;
            Ok::<_, Failure>(Bytes::from(page))
        })
        .await;
    match answer {
        Ok(json) => ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, PUBLIC)], json).into_response(),
        Err(f) => failure(&f),
    }
}

/// Hits placed per chunk of the stream: enough to amortise a send, few
/// enough that the first lines leave at once.
const CHUNK: usize = 256;

/// Every listed hit, one JSON line each, as they are placed. The engine is
/// free once the pairs are copied out; placing and sending run apart, and
/// stop as soon as the caller goes.
pub async fn stream(
    Shared(state): Shared<State>,
    client: Option<Extension<Client>>,
    params: Result<Query<Params>, QueryRejection>,
) -> Response {
    let query = match read(params, CAP, CAP) {
        Ok(query) => query,
        Err(refused) => return *refused,
    };
    let (max_cost_ms, asked) = (state.max_cost_ms, query.clone());
    let client = client.map(|c| c.0);
    let found = state
        .pool
        .run(move |engine| {
            admit(engine, &asked, max_cost_ms, client.as_ref())?;
            els::find(engine, &asked, CAP).map_err(Failure::Refused)
        })
        .await;
    let found = match found {
        Ok(Ok(found)) => found,
        Ok(Err(f)) | Err(f) => return failure(&f),
    };
    let head = json!({
        "query": Echo::of(&query, &found.letters),
        "total": found.total,
        "listed": found.listed,
        "returned": found.hits.len(),
    });
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(4);
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _span = span.enter();
        let span_letters = found.letters.chars().count();
        let line = |value: &Value| {
            let mut bytes = value.to_string().into_bytes();
            bytes.push(b'\n');
            bytes
        };
        if tx.blocking_send(Ok(Bytes::from(line(&head)))).is_err() {
            return;
        }
        for chunk in found.hits.chunks(CHUNK) {
            let mut bytes = Vec::with_capacity(chunk.len() * 600);
            for &(start, skip) in chunk {
                let hit = Hit::of(&els::place(start, skip, span_letters, query.exact));
                if let Ok(json) = serde_json::to_vec(&hit) {
                    bytes.extend_from_slice(&json);
                    bytes.push(b'\n');
                }
            }
            if tx.blocking_send(Ok(Bytes::from(bytes))).is_err() {
                tracing::info!("stream left by its caller");
                return;
            }
        }
        let _ = tx.blocking_send(Ok(Bytes::from(line(&json!({ "done": true })))));
    });
    (
        [(header::CONTENT_TYPE, "application/x-ndjson"), (header::CACHE_CONTROL, PUBLIC)],
        Body::from_stream(ReceiverStream::new(rx)),
    )
        .into_response()
}
