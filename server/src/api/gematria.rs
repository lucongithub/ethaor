//! `GET /api/gematria`: a Hebrew word or phrase read as a number by each
//! gematria method — the twin of `src/wasm/gematria.rs`. Both run
//! `ethaor::shared::gematria`; this one answers JSON, every method named
//! every way beside its value.
//!
//! The value itself takes no engine and no cache: the text is read once and
//! every method costs a few multiplications, far less than a cache's own
//! bookkeeping. The text is bounded (`MAX_LETTERS`), so the work is; each
//! request is charged its one unit by the guard, like every other.
//!
//! `GET /api/gematria/search` reads the whole Tanakh, and has no twin — the
//! page reads a selection, never searches by value: it runs as the other
//! searches do, admitted on its cost, charged to its client, on the pool,
//! its answers kept.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State as Shared};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use ethaor::shared::els::{DEFAULT_LIMIT, MAX_LIMIT, Refusal};
use ethaor::shared::gematria::{self, MAX_LETTERS, Method, Reading};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::els::{Scope, VerseRef, chapter_pattern, failure_while, osis_codes, place_pattern, status};
use super::guard::Client;
use super::pool::{Failure, Pool};
use super::{PUBLIC, refuse};
use crate::config::Config;

/// The query string; unknown names refused, never ignored.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub text: String,
    /// Method keys parted by commas; every method when absent.
    pub method: Option<String>,
}

/// Every code this endpoint may refuse with.
pub const ERRORS: [&str; 6] = ["bad_param", "no_hebrew_letter", "text_too_long", "unknown_method", "rate_limited", "internal"];

/// Every method's key, in the order answers list them.
pub fn keys() -> Vec<&'static str> {
    Method::ALL.map(|m| gematria::info(m).key).to_vec()
}

/// What `method` takes: one key or more, parted by commas.
pub fn method_pattern() -> String {
    let key = format!("({})", keys().join("|"));
    format!("^{key}(,{key})*$")
}

/// The tool as `GET /api` lists it.
pub fn tool() -> Value {
    let methods: Vec<Value> = Method::ALL
        .map(|m| {
            let info = gematria::info(m);
            json!({ "method": info.key, "name": info.name, "hebrew": info.hebrew })
        })
        .to_vec();
    json!({
        "name": "gematria",
        "method": "GET",
        "path": "/api/gematria",
        "description": format!(
            "Gematria: the numerical value of a Hebrew word or phrase, by each of {} methods — Mispar \
             haPanim (the standard value), Mispar Gadol, Mispar Katan, AtBash, AlBam and the others \
             TorahCalc documents. Only Hebrew consonants count (final forms as the method says); vowel \
             points, cantillation and anything else are ignored. Each method is answered with its key, its \
             English and Hebrew names, the names it also goes by, a description of how it counts, and its \
             value; a method that reads each letter as another (AtBash, AlBam, Ofanim…) also gives \
             `letters`, the consonants as it reads them. Values are exact integers.",
            Method::ALL.len()
        ),
        "input_schema": {
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": format!(
                        "The Hebrew word or phrase, pointed or not, {MAX_LETTERS} letters at most. Words are \
                         parted by spaces, a maqaf or punctuation: Mispar Kolel counts them."
                    ),
                    "examples": ["תורה", "בְּרֵאשִׁית בָּרָא"],
                },
                "method": {
                    "type": "string", "pattern": method_pattern(),
                    "description": "The methods answered: one key, or several parted by commas \
                        (mispar_hapanim,atbash). Every method when absent. Answered in the order of \
                        `x-methods`, each once.",
                    "x-methods": methods,
                },
            },
            "required": ["text"],
            "additionalProperties": false,
        },
        "errors": ERRORS,
    })
}

/// One method's value, named every way.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct MethodValue {
    pub method: &'static str,
    pub name: &'static str,
    pub hebrew: &'static str,
    pub also_known_as: &'static [&'static str],
    pub description: &'static str,
    pub value: u64,
    /// The consonants as the method reads them, for one that replaces
    /// letters (the ciphers, Ofanim); absent for the others.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letters: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Echo {
    pub text: String,
    /// The consonants read, in order.
    pub letters: String,
    pub letter_count: u32,
    pub words: u32,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Answer {
    pub query: Echo,
    pub methods: Vec<MethodValue>,
}

impl Answer {
    pub fn of(text: &str, reading: &Reading) -> Answer {
        Answer {
            query: Echo {
                text: text.into(),
                letters: reading.letters.clone(),
                letter_count: reading.letter_count,
                words: reading.words,
            },
            methods: reading
                .values
                .iter()
                .zip(&reading.read_as)
                .map(|(&(m, value), letters)| {
                    let info = gematria::info(m);
                    MethodValue {
                        method: info.key,
                        name: info.name,
                        hebrew: info.hebrew,
                        also_known_as: info.also_known_as,
                        description: info.description,
                        value,
                        letters: letters.clone(),
                    }
                })
                .collect(),
        }
    }
}

pub async fn read(params: Result<Query<Params>, QueryRejection>) -> Response {
    let params = match params {
        Ok(Query(params)) => params,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text()),
    };
    match gematria::methods(params.method.as_deref()).and_then(|methods| gematria::read(&params.text, &methods)) {
        Ok(reading) => ([(header::CACHE_CONTROL, PUBLIC)], Json(Answer::of(&params.text, &reading))).into_response(),
        Err(r) => refuse(status(&r), r.code(), r.message()),
    }
}

// ---------------------------------------------------------------- the search

/// The search's engines — the ELS pool's, lent for its bounds and its
/// panics, though a gematria search reads no engine — the answers already
/// given, and the heaviest search allowed.
#[derive(Clone)]
pub struct State {
    pool: Arc<Pool>,
    answers: Cache<gematria::Query, Bytes>,
    max_cost_ms: f64,
}

impl State {
    pub fn new(config: &Config, pool: Arc<Pool>) -> State {
        State {
            pool,
            answers: Cache::builder()
                .weigher(|_: &gematria::Query, v: &Bytes| v.len().min(u32::MAX as usize) as u32)
                .max_capacity(config.cache_bytes)
                .build(),
            max_cost_ms: config.max_cost_ms,
        }
    }
}

/// The search's query string; unknown names refused, never ignored.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    pub text: String,
    /// One method's key; Mispar haPanim when absent.
    pub method: Option<String>,
    #[serde(default)]
    pub offset: usize,
    pub limit: Option<usize>,
    pub portion: Option<String>,
    pub book: Option<String>,
    pub chapter: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl SearchParams {
    pub fn query(&self) -> Result<gematria::Query, Refusal> {
        let scope = ethaor::shared::els::scope(
            self.portion.as_deref(),
            self.book.as_deref(),
            self.chapter.as_deref(),
            self.from.as_deref(),
            self.to.as_deref(),
        )?;
        let method = match self.method.as_deref() {
            None => Method::MisparHapanim,
            Some(key) => gematria::method(key).ok_or_else(|| Refusal::UnknownMethod(key.into()))?,
        };
        Ok(gematria::Query {
            text: self.text.clone(),
            method,
            offset: self.offset,
            limit: self.limit.unwrap_or(DEFAULT_LIMIT),
            scope,
        })
    }
}

/// Every code the search may refuse with.
pub const SEARCH_ERRORS: [&str; 13] = [
    "bad_param",
    "no_hebrew_letter",
    "text_too_long",
    "unknown_method",
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

/// The search's tool as `GET /api` lists it.
pub fn search_tool() -> Value {
    json!({
        "name": "gematria_search",
        "method": "GET",
        "path": "/api/gematria/search",
        "description": "Gematria search: finds every word of the Tanakh, and every run of consecutive words \
            of one verse, whose gematria value by one method is the value of the given text — a phrase never \
            crosses from one verse to the next. Mispar Katan Mispari, a digital root, matches single words \
            only. `value` says what the text is worth; each hit names its verse every way, the words found \
            (pointed) and the ranks of their letters. The search may be kept to a portion, a book, a chapter or \
            a stretch from one passage to another. Hits come in the order of the text; `total` is exact even \
            when only the first `listed` can be paged through.",
        "input_schema": {
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": format!(
                        "The Hebrew word or phrase whose value is searched, {MAX_LETTERS} letters at most; \
                         vowel points and anything not a Hebrew letter are ignored."
                    ),
                    "examples": ["תורה", "אהבה"],
                },
                "method": {
                    "type": "string", "enum": keys(), "default": "mispar_hapanim",
                    "description": "The gematria method, by its key (see the `gematria` tool's x-methods).",
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
            "required": ["text"],
            "additionalProperties": false,
        },
        "errors": SEARCH_ERRORS,
    })
}

/// One run found, named every way.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Hit {
    #[serde(flatten)]
    pub place: VerseRef,
    /// The words found, pointed, as the verse writes them.
    pub words: &'static str,
    /// Ranks of the first letter and past the last among the verse's
    /// consonants, from 0.
    pub from_letter: u32,
    pub to_letter: u32,
}

impl Hit {
    pub fn of(hit: &gematria::Hit) -> Hit {
        Hit { place: VerseRef::of(&hit.place()), words: hit.words(), from_letter: hit.from, to_letter: hit.to }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SearchEcho {
    pub text: String,
    /// The consonants read.
    pub letters: String,
    pub method: &'static str,
    pub name: &'static str,
    pub hebrew: &'static str,
    pub offset: usize,
    pub limit: usize,
    /// Null for the whole Tanakh.
    pub scope: Option<Scope>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SearchAnswer {
    pub query: SearchEcho,
    /// What the text is worth by the method: what every hit is worth.
    pub value: u64,
    pub total: u64,
    pub listed: usize,
    pub returned: usize,
    pub next_offset: Option<usize>,
    pub hits: Vec<Hit>,
}

impl SearchAnswer {
    pub fn of(query: &gematria::Query, page: &gematria::Page) -> SearchAnswer {
        let info = gematria::info(query.method);
        let hits: Vec<Hit> = page.hits.iter().map(Hit::of).collect();
        SearchAnswer {
            query: SearchEcho {
                text: query.text.clone(),
                letters: page.letters.clone(),
                method: info.key,
                name: info.name,
                hebrew: info.hebrew,
                offset: query.offset,
                limit: query.limit,
                scope: Scope::of(query.scope),
            },
            value: page.value,
            total: page.total,
            listed: page.listed,
            returned: hits.len(),
            next_offset: page.next_offset(),
            hits,
        }
    }
}

/// Admits a search on its estimate and charges it to its client.
fn admit(query: &gematria::Query, max_cost_ms: f64, client: Option<&Client>) -> Result<f64, Failure> {
    let estimate = gematria::admit(query, max_cost_ms).map_err(Failure::Refused)?;
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
    params: Result<Query<SearchParams>, QueryRejection>,
) -> Response {
    let params = match params {
        Ok(params) => params.0,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text()),
    };
    // A bad query costs neither an engine nor a cache slot.
    let query = match params.query().and_then(|q| gematria::check_search(&q).map(|_| q)) {
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
                .run(move |_| {
                    let started = Instant::now();
                    let estimate_ms = admit(&asked, max_cost_ms, client.as_ref())?;
                    let page = gematria::search(&asked).map_err(Failure::Refused)?;
                    tracing::info!(
                        letters = %page.letters,
                        method = gematria::info(asked.method).key,
                        total = page.total,
                        estimate_ms,
                        search_ms = started.elapsed().as_secs_f64() * 1e3,
                        "searched gematria"
                    );
                    serde_json::to_vec(&SearchAnswer::of(&asked, &page))
                        .map_err(|e| Failure::ours("gematria_answer_unwritten", e.to_string()))
                })
                .await??;
            Ok::<_, Failure>(Bytes::from(json))
        })
        .await;
    match answer {
        Ok(json) => ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, PUBLIC)], json).into_response(),
        Err(f) => failure_while(&f, "answering a gematria search"),
    }
}
