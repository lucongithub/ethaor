//! The static build, served as the page needs it: ranges for tiles, no
//! compression on top of gzip, a year of cache for hashed names, the app's
//! own routing.

use std::path::PathBuf;

use axum::http::StatusCode;

use super::get;
use crate::files::IMMUTABLE;

/// A build of a few files, in a directory of its own per test.
fn build(name: &str) -> (PathBuf, Vec<u8>) {
    let root = std::env::temp_dir().join(format!("ethaor-server-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("_app/immutable")).unwrap();
    std::fs::write(root.join("index.html"), "<!doctype html>ethaor").unwrap();
    std::fs::write(root.join("_app/immutable/app.1234.js"), "export {}").unwrap();
    std::fs::write(root.join("_app/immutable/ethaor.5678.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let plain: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    std::fs::write(root.join("_app/immutable/text.plain.bin"), &plain).unwrap();
    (root, plain)
}

#[tokio::test]
async fn the_page_and_its_routes_get_index_html() {
    let (root, _) = build("index");
    for path in ["/", "/somewhere/else"] {
        let answer = get(super::app_at(&root), path, &[]).await;
        assert_eq!(answer.status, StatusCode::OK, "{path}");
        assert_eq!(answer.body, b"<!doctype html>ethaor", "{path}");
        assert!(answer.headers.get("cache-control").is_none(), "{path} kept in cache");
    }
}

/// A page prerendered as a directory's index (`/logs/`, the log page) is
/// served there, never in the cache, and `/logs` is sent to it with its
/// query whole — the token it carries once must not be dropped on the way.
#[tokio::test]
async fn a_page_of_its_own_is_served_at_its_directory() {
    let (root, _) = build("logs-page");
    std::fs::create_dir_all(root.join("logs")).unwrap();
    std::fs::write(root.join("logs/index.html"), "<!doctype html>logs").unwrap();
    let page = get(super::app_at(&root), "/logs/", &[]).await;
    assert_eq!((page.status, page.body.as_slice()), (StatusCode::OK, &b"<!doctype html>logs"[..]));
    assert!(page.headers.get("cache-control").is_none(), "the log page kept in cache");
    let sent = get(super::app_at(&root), "/logs?token=abc", &[]).await;
    assert!(sent.status.is_redirection(), "{}", sent.status);
    assert_eq!(sent.headers["location"], "/logs/?token=abc");
}

#[tokio::test]
async fn hashed_names_are_kept_a_year_and_the_module_streams() {
    let (root, _) = build("cache");
    let script = get(super::app_at(&root), "/_app/immutable/app.1234.js", &[]).await;
    assert_eq!(script.headers["cache-control"], IMMUTABLE);
    let module = get(super::app_at(&root), "/_app/immutable/ethaor.5678.wasm", &[]).await;
    assert_eq!(module.headers["content-type"], "application/wasm");
    let missing = get(super::app_at(&root), "/_app/immutable/gone.js", &[]).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND, "a missing name gets the page");
    assert!(missing.headers.get("cache-control").is_none(), "a missing name gets a year of cache");
}

/// A tile is a Range request: the bytes asked, exactly, never re-encoded.
#[tokio::test]
async fn a_tile_gets_its_range_uncompressed() {
    let (root, plain) = build("range");
    let answer = get(
        super::app_at(&root),
        "/_app/immutable/text.plain.bin",
        &[("range", "bytes=1000-1999"), ("accept-encoding", "gzip, br")],
    )
    .await;
    assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(answer.body, &plain[1000..2000]);
    assert!(answer.headers.get("content-encoding").is_none());
}

/// `/api` is never shadowed by the build, whatever it holds.
#[tokio::test]
async fn the_api_wins_over_the_files() {
    let (root, _) = build("api");
    std::fs::create_dir_all(root.join("api")).unwrap();
    std::fs::write(root.join("api/index.html"), "shadow").unwrap();
    let answer = get(super::app_at(&root), "/api", &[]).await;
    assert_eq!(answer.json()["name"], "ethaor");
}

/// Nothing outside the build is ever served, however the path climbs.
#[tokio::test]
async fn no_path_climbs_out_of_the_build() {
    let (root, _) = build("climb");
    let secret = root.parent().unwrap().join(format!("ethaor-secret-{}.txt", std::process::id()));
    std::fs::write(&secret, "SECRET").unwrap();
    let name = secret.file_name().unwrap().to_str().unwrap();
    for path in [
        format!("/../{name}"),
        format!("/_app/../../{name}"),
        format!("/%2e%2e/{name}"),
        format!("/_app/%2e%2e/%2e%2e/{name}"),
        format!("/..%2f{name}"),
        format!("/_app/immutable/..%5c..%5c..%5c{name}"),
    ] {
        let answer = get(super::app_at(&root), &path, &[]).await;
        assert!(!String::from_utf8_lossy(&answer.body).contains("SECRET"), "{path} leaves the build");
    }
    let _ = std::fs::remove_file(secret);
}

/// A browser asking again with what it holds is told it holds it: no
/// bytes resent.
#[tokio::test]
async fn a_file_asked_again_is_not_resent() {
    let (root, _) = build("again");
    let first = get(super::app_at(&root), "/_app/immutable/app.1234.js", &[]).await;
    assert!(first.headers["content-type"].to_str().unwrap().contains("javascript"));
    let modified = first.headers["last-modified"].to_str().unwrap().to_string();
    let again = get(super::app_at(&root), "/_app/immutable/app.1234.js", &[("if-modified-since", &modified)]).await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    assert!(again.body.is_empty());
}
