//! `POST /api/report`: what goes wrong in the page reaches the service's
//! logs, under the incident number the page already showed its user.
//!
//! The page raises its faults as the service does (`web/src/lib/fault.ts`):
//! a number, a code, where, the escalation, the message the user read. It
//! sends them here; one log line each, in the same shape as the service's
//! own — so an incident a user quotes is found whichever side raised it.
//!
//! Bounded like everything else: the body (`BODY`), the steps (`STEPS`),
//! each text (`TEXT`); every report is charged to its client by the guard.
//! A malformed report is refused, never a reason to fail.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::refuse;
use crate::fault::{self, Incident};

/// Bytes a report may hold.
pub const BODY: usize = 16 * 1024;
/// Steps of an escalation kept.
pub const STEPS: usize = 32;
/// Characters of any one text kept.
pub const TEXT: usize = 2000;

#[derive(Deserialize, Debug)]
pub struct StepIn {
    #[serde(default)]
    pub at: String,
    #[serde(default)]
    pub what: String,
}

#[derive(Deserialize, Debug)]
pub struct Report {
    pub incident: String,
    pub code: String,
    /// What the user read.
    #[serde(default)]
    pub message: String,
    /// `page`, `worker` or `wasm`.
    #[serde(default)]
    pub source: String,
    /// `error` (default) or `warn` — a degradation the page lived through.
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub chain: Vec<StepIn>,
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub url: String,
}

pub fn endpoint() -> Value {
    json!({
        "name": "page_report",
        "method": "POST",
        "path": "/api/report",
        "description": "Where the page sends its faults (web/src/lib/fault.ts), to be logged under the \
            incident number its user saw. Not for agents.",
        "errors": ["bad_report", "rate_limited"],
    })
}

fn cut(text: &str) -> String {
    text.chars().take(TEXT).collect()
}

/// A code as the page names its faults: short, lower case, no spaces.
fn code_of(text: &str) -> String {
    let ok = !text.is_empty()
        && text.len() <= 64
        && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-.".contains(&b));
    if ok { text.into() } else { "unnamed".into() }
}

/// The log line's fields, as the service's own faults write them: always
/// the incident, code, where and what the user read; verbose, the steps
/// and the stack.
pub fn fields(report: &Report, verbose: bool) -> (Incident, Map<String, Value>) {
    let given = Incident::parse(report.incident.trim());
    let incident = given.clone().unwrap_or_default();
    let chain: Vec<Value> = report
        .chain
        .iter()
        .take(STEPS)
        .map(|s| json!({ "at": cut(&s.at), "what": cut(&s.what) }))
        .collect();
    let mut f = Map::new();
    f.insert("incident".into(), incident.as_str().into());
    if given.is_none() {
        f.insert("incident_given".into(), cut(&report.incident).into());
    }
    f.insert("code".into(), code_of(&report.code).into());
    f.insert("at".into(), chain.first().and_then(|s| s["at"].as_str()).unwrap_or("unknown").into());
    f.insert("source".into(), (if report.source.is_empty() { "page".into() } else { cut(&report.source) }).into());
    f.insert("user_message".into(), cut(&report.message).into());
    if !report.url.is_empty() {
        f.insert("url".into(), cut(&report.url).into());
    }
    if verbose {
        f.insert("chain".into(), chain.into());
        if !report.stack.is_empty() {
            f.insert("stack".into(), cut(&report.stack).into());
        }
    }
    (incident, f)
}

pub async fn receive(body: Result<Json<Report>, JsonRejection>) -> Response {
    let report = match body {
        Ok(Json(report)) => report,
        Err(e) => {
            let status = if e.status() == StatusCode::PAYLOAD_TOO_LARGE { e.status() } else { StatusCode::BAD_REQUEST };
            return refuse(status, "bad_report", format!("a report is the JSON fault.ts sends: {}", e.body_text()));
        }
    };
    let (incident, fields) = fields(&report, fault::verbose());
    let text = |key: &str| fields[key].as_str().unwrap_or_default().to_string();
    let (incident, code, at) = (incident.to_string(), text("code"), text("at"));
    let fields = Value::Object(fields).to_string();
    if report.level == "warn" {
        tracing::warn!(incident, code, at, fault = %fields, "page degraded");
    } else {
        tracing::error!(incident, code, at, fault = %fields, "page failed");
    }
    StatusCode::NO_CONTENT.into_response()
}
