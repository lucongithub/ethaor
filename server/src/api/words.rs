//! `GET /api/words`: first and last letters — a word read one letter from
//! each word of the Tanakh — the twin of `src/wasm/words.rs`. Both run
//! `ethaor::shared::words`; this one answers a page of JSON.

use std::sync::Arc;
use std::time::Instant;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State as Shared};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use ethaor::shared::els::{DEFAULT_LIMIT, Engine, MAX_LIMIT, Refusal};
use ethaor::shared::words::{self, LONGEST_WORD, Page, Pick};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::els::{Letter, Scope, VerseRef, chapter_pattern, failure_while, osis_codes, place_pattern, status};
use super::guard::Client;
use super::pool::{Failure, Pool};
use super::{PUBLIC, refuse};
use crate::config::Config;

/// The engines — the ELS search's own, which lend this one — the answers
/// already given, and the heaviest search allowed.
#[derive(Clone)]
pub struct State {
    pool: Arc<Pool>,
    answers: Cache<words::Query, Bytes>,
    max_cost_ms: f64,
}

impl State {
    pub fn new(config: &Config, pool: Arc<Pool>) -> State {
        State {
            pool,
            answers: Cache::builder()
                .weigher(|_: &words::Query, v: &Bytes| v.len().min(u32::MAX as usize) as u32)
                .max_capacity(config.cache_bytes)
                .build(),
            max_cost_ms: config.max_cost_ms,
        }
    }
}

/// The query string. Unknown names are refused rather than ignored.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub word: String,
    #[serde(default)]
    pub exact: bool,
    /// Which letter of each word: 1 the first. Defaults to 1.
    pub letter: Option<u32>,
    /// Counted from the word's end: 1 the last.
    #[serde(default)]
    pub from_end: bool,
    #[serde(default)]
    pub any_order: bool,
    #[serde(default)]
    pub offset: usize,
    pub limit: Option<usize>,
    pub portion: Option<String>,
    pub book: Option<String>,
    pub chapter: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Params {
    pub fn query(&self) -> Result<words::Query, Refusal> {
        let scope = ethaor::shared::els::scope(
            self.portion.as_deref(),
            self.book.as_deref(),
            self.chapter.as_deref(),
            self.from.as_deref(),
            self.to.as_deref(),
        )?;
        Ok(words::Query {
            word: self.word.clone(),
            exact: self.exact,
            pick: Pick { nth: self.letter.unwrap_or(1), from_end: self.from_end },
            any_order: self.any_order,
            offset: self.offset,
            limit: self.limit.unwrap_or(DEFAULT_LIMIT),
            scope,
        })
    }
}

/// Every code this endpoint may refuse with.
pub const ERRORS: [&str; 12] = [
    "bad_param",
    "no_hebrew_letter",
    "letter_range",
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

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "word": {
                "type": "string",
                "description": "Hebrew word; vowel points and non-Hebrew characters are ignored.",
            },
            "letter": {
                "type": "integer", "minimum": 1, "maximum": LONGEST_WORD, "default": 1,
                "description": "Which letter of each word is read: 1 the first, 2 the second… (with \
                    from_end, 1 the last). A hit's words are adjacent: a word too short to hold \
                    it breaks the run.",
            },
            "from_end": {
                "type": "boolean", "default": false,
                "description": "Count `letter` from the end of each word: last letters.",
            },
            "any_order": {
                "type": "boolean", "default": false,
                "description": "The word's letters in any order: consecutive words whose letters, \
                    taken together, are the word's — an anagram, the word as written included.",
            },
            "exact": {
                "type": "boolean", "default": false,
                "description": "Exact letters (final forms): a final form matches only itself — ם a final \
                    mem, מ a non-final one. Off, final forms are folded (ם = מ).",
            },
            "offset": {
                "type": "integer", "minimum": 0, "default": 0,
                "description": "Index of the first hit returned; pass the previous answer's next_offset.",
            },
            "limit": {
                "type": "integer", "minimum": 0, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT,
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
                "description": "Search from this book, chapter or verse on, in OSIS; with `to`, a stretch \
                    of passages, both ends included.",
            },
            "to": {
                "type": "string", "pattern": place_pattern(),
                "description": "Search up to this book, chapter or verse, included, in OSIS.",
            },
        },
        "required": ["word"],
        "additionalProperties": false,
    })
}

/// The tool as `GET /api` lists it.
pub fn tool() -> Value {
    json!({
        "name": "words_search",
        "method": "GET",
        "path": "/api/words",
        "description": "First and last letters search: finds a Hebrew word spelled by consecutive words \
            of the Tanakh, one letter from each — the first letter of every word by default, or its \
            second, third… (`letter`), or counted from its end (`from_end`: last letters). The words are \
            adjacent in the text: a word too short to hold that letter breaks the run. With `any_order`, the letters may come in any order. Only \
            consonants count; final forms are folded (ם = מ) unless `exact` tells them apart. The search \
            may be kept to a portion, a book, a chapter or a stretch from one passage to another: a hit \
            then counts only if all its letters lie there. Hits come in the order of the text; `total` is \
            exact even when only the first `listed` can be paged through. Answers one page of hits.",
        "input_schema": schema(),
        "errors": ERRORS,
    })
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Hit {
    /// Verse of the first letter, and of the last.
    pub from: VerseRef,
    pub to: VerseRef,
    pub letters: Vec<Letter>,
}

impl Hit {
    pub fn of(hit: &words::Hit) -> Hit {
        Hit {
            from: VerseRef::of(&hit.first_verse()),
            to: VerseRef::of(&hit.last_verse()),
            letters: hit.letters.iter().map(Letter::of).collect(),
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Echo {
    pub word: String,
    /// The word as searched: consonants only, finals folded unless exact.
    pub letters: String,
    pub exact: bool,
    pub letter: u32,
    pub from_end: bool,
    pub any_order: bool,
    pub offset: usize,
    pub limit: usize,
    /// Null for the whole Tanakh.
    pub scope: Option<Scope>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Answer {
    pub query: Echo,
    pub total: u64,
    pub listed: usize,
    pub returned: usize,
    pub next_offset: Option<usize>,
    pub hits: Vec<Hit>,
}

impl Answer {
    pub fn of(query: &words::Query, page: &Page) -> Answer {
        let hits = page.hits.iter().map(Hit::of).collect::<Vec<_>>();
        Answer {
            query: Echo {
                word: query.word.clone(),
                letters: page.letters.clone(),
                exact: query.exact,
                letter: query.pick.nth,
                from_end: query.pick.from_end,
                any_order: query.any_order,
                offset: query.offset,
                limit: query.limit,
                scope: Scope::of(query.scope),
            },
            total: page.total,
            listed: page.listed,
            returned: hits.len(),
            next_offset: page.next_offset(),
            hits,
        }
    }
}

/// Admits a search on its estimate and charges it to its client.
fn admit(engine: &mut Engine, query: &words::Query, max_cost_ms: f64, client: Option<&Client>) -> Result<f64, Failure> {
    let estimate = words::admit(engine, query, max_cost_ms).map_err(Failure::Refused)?;
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
    let params = match params {
        Ok(params) => params.0,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text()),
    };
    // A bad query costs neither an engine nor a cache slot.
    let query = match params.query().and_then(|q| words::check(&q).map(|_| q)) {
        Ok(query) => query,
        Err(r) => return refuse(status(&r), r.code(), r.message()),
    };
    let (pool, max_cost_ms) = (state.pool.clone(), state.max_cost_ms);
    let asked = query.clone();
    let client = client.map(|c| c.0);
    let answer = state
        .answers
        .try_get_with(query, async move {
            let json = pool
                .run(move |engine| {
                    let started = Instant::now();
                    let estimate_ms = admit(engine, &asked, max_cost_ms, client.as_ref())?;
                    let page = words::search(engine, &asked).map_err(Failure::Refused)?;
                    tracing::info!(
                        letters = %page.letters,
                        total = page.total,
                        estimate_ms,
                        search_ms = started.elapsed().as_secs_f64() * 1e3,
                        "searched words"
                    );
                    serde_json::to_vec(&Answer::of(&asked, &page))
                        .map_err(|e| Failure::ours("words_answer_unwritten", e.to_string()))
                })
                .await??;
            Ok::<_, Failure>(Bytes::from(json))
        })
        .await;
    match answer {
        Ok(json) => ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, PUBLIC)], json).into_response(),
        Err(f) => failure_while(&f, "answering a words search"),
    }
}
