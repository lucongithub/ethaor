//! `GET /api/logs`: the service's logs, live, for its operator.
//!
//! Server-sent events (`text/event-stream`), one event per log line, its
//! data the line's JSON: first the last `history` lines of the log files,
//! then each new one as it is written there. `level` keeps that level and
//! above. The files are only read: the service never waits for a follower.
//! One so slow that a file it had not read was deleted is told how many it
//! missed (`event: lagged`) and goes on from the oldest left.
//!
//! **Off unless `ETHAOR_LOGS_TOKEN` is set**, and then only with
//! `Authorization: Bearer <token>`: logs name clients by address. With no
//! log file (`ETHAOR_LOG_DIR=off`, or a directory that cannot be written),
//! a 503 `logs_unavailable`.

use std::convert::Infallible;
use std::path::PathBuf;
use std::time::Duration;

use axum::Extension;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::Level;

use super::refuse;
use crate::logs::{self, Followed, HISTORY};

/// What the router hands the endpoint: the operator's token, and the
/// directory the log files are in, if there is one.
#[derive(Clone)]
pub struct Source {
    pub token: Option<String>,
    pub dir: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub level: Option<String>,
    pub history: Option<usize>,
}

pub fn endpoint() -> Value {
    json!({
        "name": "logs_follow",
        "method": "GET",
        "path": "/api/logs",
        "description": "The service's logs, live, as server-sent events: one JSON log line per event, \
            the last `history` first, read from the log files. Needs `Authorization: Bearer <ETHAOR_LOGS_TOKEN>`; \
            off when unset.",
        "input_schema": {
            "type": "object",
            "properties": {
                "level": { "type": "string", "enum": ["error", "warn", "info", "debug", "trace"], "default": "info" },
                "history": { "type": "integer", "minimum": 0, "maximum": HISTORY, "default": 100 },
            },
            "additionalProperties": false,
        },
        "errors": ["logs_disabled", "unauthorized", "bad_param", "logs_unavailable"],
    })
}

/// Compares in time independent of where they differ: the token must not
/// leak a byte at a time.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub async fn follow(
    Extension(Source { token, dir }): Extension<Source>,
    headers: HeaderMap,
    params: Result<Query<Params>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Some(token) = token else {
        return refuse(StatusCode::FORBIDDEN, "logs_disabled", "set ETHAOR_LOGS_TOKEN to follow the logs".into());
    };
    let given = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
    if !given.is_some_and(|g| same(g.trim().as_bytes(), token.as_bytes())) {
        return refuse(StatusCode::UNAUTHORIZED, "unauthorized", "Authorization: Bearer <token> expected".into());
    }
    let params = match params {
        Ok(Query(p)) => p,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text()),
    };
    let level = match params.level.as_deref().map(logs::level) {
        None => Level::INFO,
        Some(Some(level)) => level,
        Some(None) => return refuse(StatusCode::BAD_REQUEST, "bad_param", "level: error, warn, info, debug or trace".into()),
    };
    let history = params.history.unwrap_or(100).min(HISTORY);
    let Some(dir) = dir else {
        return refuse(StatusCode::SERVICE_UNAVAILABLE, "logs_unavailable", "no log file is written: see ETHAOR_LOG_DIR".into());
    };
    // Where the last whole line ends: the history stops there, the live
    // lines start there — none lost in between, none twice.
    let (from, past) = {
        let dir = dir.clone();
        tokio::task::spawn_blocking(move || {
            let from = logs::end(&dir);
            (from, logs::recent(&dir, from, history, level))
        })
        .await
        .unwrap_or_else(|_| (logs::Position { seq: 0, at: 0 }, Vec::new()))
    };
    Sse::new(events(past, logs::follow(dir, from, level)))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

/// The lines read back, then the live ones.
fn events(past: Vec<String>, live: impl Stream<Item = Followed>) -> impl Stream<Item = Result<Event, Infallible>> {
    let past = stream::iter(past.into_iter().map(|line| Ok(Event::default().data(line))));
    let live = live.map(|followed| {
        Ok(match followed {
            Followed::Line(line) => Event::default().data(line),
            Followed::Lagged(files) => Event::default().event("lagged").data(files.to_string()),
        })
    });
    past.chain(live)
}
