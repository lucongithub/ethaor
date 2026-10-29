//! The API: JSON in the shape an agent reads without a guide.
//!
//! - `GET /api` describes every endpoint as a tool: name, description and a
//!   JSON Schema of its parameters — what an MCP server lists, as is.
//! - Every answer is an object; every refusal is
//!   `{"error": {"code", "message"}}` with a 4xx or 5xx status, the code
//!   stable enough to branch on.
//! - Places are named every way at once: OSIS (`Gen.1.1`), English
//!   (`Genesis 1:1`), Hebrew (`בראשית א,א`) and the corpus' global index.

pub mod chapter;
pub mod contents;
pub mod els;
pub mod gematria;
pub mod guard;
pub mod logs;
pub mod pool;
pub mod report;
pub mod words;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Extension, Json};
use serde_json::{Value, json};

use crate::config::Config;
use crate::fault::{Fault, Kind};

/// What the API holds: the search's engines and answers, the clients'
/// budgets, the operator's token.
#[derive(Clone)]
pub struct Api {
    pub els: els::State,
    pub words: words::State,
    pub gematria: gematria::State,
    pub guard: guard::Guard,
    pub logs_token: Option<String>,
}

impl Api {
    pub fn new(config: &Config) -> Api {
        let els = els::State::new(config);
        let words = words::State::new(config, els.pool.clone());
        let gematria = gematria::State::new(config, els.pool.clone());
        Api { els, words, gematria, guard: guard::Guard::new(config), logs_token: config.logs_token.clone() }
    }
}

/// Every path under `/api` answers JSON — a wrong path or method too, never
/// the page: an agent reading HTML for an error would have nothing to parse.
/// Every request is charged to its client first (`guard`).
pub fn router(api: Api) -> Router {
    Router::new()
        .route("/api", get(describe).fallback(not_allowed))
        .route("/api/", get(describe).fallback(not_allowed))
        .route("/api/chapter", get(chapter::read).fallback(not_allowed))
        .route("/api/contents", get(contents::list).fallback(not_allowed))
        .route("/api/els", get(els::search).with_state(api.els.clone()).fallback(not_allowed))
        .route("/api/els/stream", get(els::stream).with_state(api.els.clone()).fallback(not_allowed))
        .route("/api/words", get(words::search).with_state(api.words.clone()).fallback(not_allowed))
        .route("/api/gematria", get(gematria::read).fallback(not_allowed))
        .route("/api/gematria/search", get(gematria::search).with_state(api.gematria.clone()).fallback(not_allowed))
        .route("/api/logs", get(logs::follow).fallback(not_allowed))
        .route(
            "/api/report",
            post(report::receive).layer(DefaultBodyLimit::max(report::BODY)).fallback(not_allowed_post),
        )
        .route("/api/{*rest}", any(not_found))
        .layer(Extension(logs::Source { token: api.logs_token.clone(), dir: crate::logs::dir().map(Into::into) }))
        .layer(middleware::from_fn_with_state(api.guard.clone(), guard::limit))
}

/// A successful answer may be kept by anyone: the corpus is frozen, so it
/// is the same for every caller. An hour in a browser, a day in a shared
/// cache — a CDN in front then answers repeated queries without reaching
/// the service at all. A refusal is never kept.
pub const PUBLIC: &str = "public, max-age=3600, s-maxage=86400";

/// What `GET /api` answers: every public endpoint, as a tool, and the
/// operator's apart — an agent has no use for them.
pub fn description() -> Value {
    let [books, chapters, verses] = ethaor::shared::reference::counts();
    json!({
        "name": "Ethaor",
        "description": format!(
            "The Hebrew Bible (Tanakh, Mechon Mamre edition: {books} books, {chapters} chapters, \
             {verses} verses), searched and read the way the Ethaor map does it in the browser."
        ),
        "endpoints": els::tools().into_iter().chain([words::tool(), gematria::tool(), gematria::search_tool(), chapter::tool(), contents::tool()]).collect::<Vec<_>>(),
        "operator": [logs::endpoint(), report::endpoint()],
    })
}

async fn describe() -> Response {
    ([(header::CACHE_CONTROL, PUBLIC)], Json(description())).into_response()
}

async fn not_allowed() -> Response {
    let mut response = refuse(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", "this endpoint only answers GET".into());
    response.headers_mut().insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
    response
}

async fn not_allowed_post() -> Response {
    let mut response = refuse(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", "this endpoint only takes POST".into());
    response.headers_mut().insert(header::ALLOW, HeaderValue::from_static("POST"));
    response
}

async fn not_found() -> Response {
    refuse(StatusCode::NOT_FOUND, "not_found", "no such endpoint — GET /api lists them".into())
}

/// A refusal, raised here: numbered, logged once, answered. The caller's
/// place is its `at`. 429 and 503 are the service's limits, the rest the
/// caller's request.
#[track_caller]
pub fn refuse(status: StatusCode, code: &'static str, message: String) -> Response {
    let kind = match status {
        StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE => Kind::Unavailable,
        _ => Kind::Refused,
    };
    Fault::new(kind, code, status, message).respond()
}
