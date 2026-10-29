//! The service, called as a client calls it: a request in, status, headers
//! and body out — no socket, the router answers in memory.

mod chapter;
mod contents;
mod describe;
mod els;
mod faults;
mod files;
mod gematria;
mod guard;
mod load;
mod logs;
mod parity;
mod robust;
mod routes;
mod scale;
mod settings;
mod stream;
mod words;

use std::path::Path;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// Query pairs, as a test lists them.
pub type Pairs<'a> = &'a [(&'a str, &'a str)];

pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("the answer is not JSON")
    }
}

/// The service with no bound on cost or budget: what every test but the
/// guard's runs on, so a heavy case is searched rather than refused.
/// Clients are named by `x-forwarded-for`, so a test may play several.
pub fn open() -> crate::config::Config {
    crate::config::Config {
        max_cost_ms: f64::MAX,
        ip_burst_ms: f64::MAX,
        ip_rate_ms: 1.0,
        trust_proxy: true,
        ..Default::default()
    }
}

/// The service over a build directory, unbounded.
pub fn app_at(root: &Path) -> axum::Router {
    crate::app(root, &open())
}

/// Every line this run has logged so far, oldest first, read from its files.
pub fn logged_lines() -> Vec<String> {
    crate::logs::init();
    let dir = crate::logs::dir().expect("the test run writes no log file");
    crate::logs::recent(dir, crate::logs::end(dir), usize::MAX, tracing::Level::TRACE)
}

/// A root with no static build: the API alone.
pub fn api_only() -> axum::Router {
    app_at(Path::new("no-such-build"))
}

pub async fn get(app: axum::Router, uri: &str, headers: &[(&str, &str)]) -> Answer {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app.oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
    let (parts, body) = response.into_parts();
    Answer { status: parts.status, headers: parts.headers, body: body.collect().await.unwrap().to_bytes().to_vec() }
}

/// A query string, Hebrew percent-encoded as a browser sends it.
pub fn query(pairs: &[(&str, &str)]) -> String {
    let encode = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    pairs.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect::<Vec<_>>().join("&")
}

pub async fn els(pairs: &[(&str, &str)]) -> Answer {
    get(api_only(), &format!("/api/els?{}", query(pairs)), &[]).await
}

pub async fn words(pairs: &[(&str, &str)]) -> Answer {
    get(api_only(), &format!("/api/words?{}", query(pairs)), &[]).await
}
