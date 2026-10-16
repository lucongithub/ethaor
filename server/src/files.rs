//! The static build, served as a CDN would.
//!
//! - Range requests are honoured: tiles read `ethaor-text.plain.bin` by range.
//! - Nothing is compressed on the fly: the books are gzipped already, and a
//!   compressed range is no longer the range the page asked for.
//! - `_app/immutable/` carries content hashes in its names, so it is cached
//!   for a year — the rule `web/static/_headers` gives Cloudflare.
//! - A path naming no file gets `index.html`: the app routes itself — except
//!   under `_app/`, where a missing name is a 404: a stale page asking for an
//!   old hash must not get HTML for a script, and keep it for a year.

use std::path::Path;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use tower_http::services::{ServeDir, ServeFile};

pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub fn router(root: &Path) -> Router {
    let page = ServeDir::new(root).fallback(ServeFile::new(root.join("index.html")));
    Router::new()
        .nest_service("/_app", ServeDir::new(root.join("_app")))
        .fallback_service(page)
        .layer(middleware::from_fn(cache))
}

async fn cache(request: Request, next: Next) -> Response {
    let immutable = request.uri().path().starts_with("/_app/immutable/");
    let mut response = next.run(request).await;
    if immutable && response.status().is_success() {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
    }
    response
}
