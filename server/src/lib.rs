//! ethaor's HTTP service: the static build of `web/`, and an API answering
//! with the code the WebAssembly modules run.
//!
//! Each file of `api/` has its twin in the crate's `src/wasm/`: same name,
//! same computation through `ethaor::shared`, JSON instead of records. The
//! browser keeps searching on its own once the text is in — no round trip —
//! and the API serves whoever has no browser: scripts, other apps, agents.
//!
//! **It never goes down for a request.** A panic anywhere is caught and
//! answered 500; a search is admitted on its estimated cost and charged to
//! its client; a full queue answers 503; caches, budgets and logs are all
//! bounded. Under too much, it serves less — never nothing.

pub mod api;
pub mod config;
pub mod fault;
pub mod files;
pub mod logs;

use std::any::Any;
use std::path::Path;
use std::time::Instant;

use axum::Router;
use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::{self, Next};
use axum::response::Response;
use tower_http::catch_panic::CatchPanicLayer;
use tracing::Instrument;

/// The address a browser opens for a server listening on `addr`: an
/// unspecified one (`0.0.0.0`, `[::]`, every interface) is reached here as
/// `127.0.0.1` — a browser cannot open it as such.
pub fn browse_url(addr: std::net::SocketAddr) -> String {
    let host = if addr.ip().is_unspecified() { std::net::Ipv4Addr::LOCALHOST.into() } else { addr.ip() };
    format!("http://{}/", std::net::SocketAddr::new(host, addr.port()))
}

/// The whole service, tuned by `config`: `/api/…`, then the files of `root`
/// for every other path.
pub fn app(root: &Path, config: &config::Config) -> Router {
    app_with(root, api::Api::new(config))
}

/// The same, around an API already built — the tests keep a hand on it.
pub fn app_with(root: &Path, api: api::Api) -> Router {
    harden(api::router(api).merge(files::router(root)))
}

/// What every route stands behind: a panic caught and answered, an id and a
/// log line per request.
pub fn harden(router: Router) -> Router {
    router.layer(CatchPanicLayer::custom(panicked)).layer(middleware::from_fn(trace))
}

/// A panic in a handler: logged, answered as any failure, and the service
/// goes on.
fn panicked(panic: Box<dyn Any + Send + 'static>) -> Response {
    fault::from_panic("handler_panicked", &*panic, "a request handler").respond()
}

/// Every request gets an id — given back as `x-request-id`, carried by its
/// every log line — and one line once answered: method, path, status, time.
async fn trace(request: Request, next: Next) -> Response {
    let id = uuid::Uuid::new_v4().to_string();
    let (method, path) = (request.method().clone(), request.uri().path().to_string());
    let span = tracing::info_span!("request", request_id = %id);
    let started = Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;
    let ms = started.elapsed().as_secs_f64() * 1e3;
    let status = response.status().as_u16();
    span.in_scope(|| match status {
        429 | 503 => tracing::warn!(%method, path, status, ms, "answered"),
        500.. => tracing::error!(%method, path, status, ms, "answered"),
        _ => tracing::info!(%method, path, status, ms, "answered"),
    });
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

#[cfg(test)]
mod tests;
