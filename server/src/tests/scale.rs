//! Many callers at once: engines lent one per search, answers shared by
//! everyone, a full queue turned away — and nothing lost when a caller goes.

use std::path::Path;
use std::time::Duration;

use axum::http::StatusCode;

use super::{get, query};
use crate::api::PUBLIC;
use crate::api::Api;
use crate::config::Config;

fn state(engines: usize, wait_ms: u64) -> Api {
    let config = Config { engines, wait: Duration::from_millis(wait_ms), cache_bytes: 64 << 20, ..super::open() };
    Api::new(&config)
}

fn app(api: &Api) -> axum::Router {
    crate::app_with(Path::new("no-such-build"), api.clone())
}

fn uri(word: &str, skip_min: u32, skip_max: u32) -> String {
    let pairs = [("word", word), ("skip_min", &skip_min.to_string()), ("skip_max", &skip_max.to_string())];
    format!("/api/els?{}", query(&pairs))
}

/// Searches run side by side, one engine each, never more engines than
/// the pool allows — and every engine is back once they are done.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn searches_run_side_by_side_on_engines_of_their_own() {
    let s = state(4, 60_000);
    let mut tasks = Vec::new();
    for k in 0..8 {
        let (app, uri) = (app(&s), uri("ירושלים", k, 3000 + k));
        tasks.push(tokio::spawn(async move { get(app, &uri, &[]).await.status }));
    }
    for t in tasks {
        assert_eq!(t.await.unwrap(), StatusCode::OK);
    }
    assert!(s.els.pool.built() >= 2, "the searches waited for a single engine");
    assert!(s.els.pool.built() <= s.els.pool.size(), "more engines than places");
    assert_eq!(s.els.pool.idle(), s.els.pool.built(), "an engine did not come back");
    assert_eq!(s.els.pool.runs(), 8);
}

/// A query asked again — by anyone — is answered without an engine, and the
/// same query asked by many at once is searched once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_answer_is_searched_once_for_everyone() {
    let s = state(4, 60_000);
    let first = get(app(&s), &uri("שלמה", 0, 40), &[]).await;
    let again = get(app(&s), &uri("שלמה", 0, 40), &[]).await;
    assert_eq!(first.body, again.body);
    assert_eq!(s.els.pool.runs(), 1, "the answer asked again was searched again");

    let mut tasks = Vec::new();
    for _ in 0..16 {
        let (app, uri) = (app(&s), uri("ירושלים", 7, 2500));
        tasks.push(tokio::spawn(async move { get(app, &uri, &[]).await.body }));
    }
    let mut bodies = Vec::new();
    for t in tasks {
        bodies.push(t.await.unwrap());
    }
    assert!(bodies.windows(2).all(|w| w[0] == w[1]));
    assert_eq!(s.els.pool.runs(), 2, "the same simultaneous query was searched several times");
}

/// A bad query takes no engine and leaves nothing in the cache.
#[tokio::test]
async fn a_refusal_costs_no_engine() {
    let s = state(2, 60_000);
    for _ in 0..3 {
        let answer = get(app(&s), &uri("torah", 0, 0), &[]).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST);
        assert_eq!(answer.headers["cache-control"], "no-store");
    }
    assert_eq!((s.els.pool.built(), s.els.pool.runs()), (0, 0));
}

/// Every engine taken past `wait`: the caller is turned away at once with a
/// 503 it may retry, never kept; once an engine frees, the same query
/// answers.
#[tokio::test]
async fn a_full_queue_turns_callers_away() {
    let s = state(2, 30);
    let held = s.els.pool.occupy().await;
    let answer = get(app(&s), &uri("משה", 0, 9), &[]).await;
    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(answer.json()["error"]["code"], "busy");
    assert_eq!(answer.headers["retry-after"], "1");
    assert_eq!(answer.headers["cache-control"], "no-store");
    drop(held);
    assert_eq!(get(app(&s), &uri("משה", 0, 9), &[]).await.status, StatusCode::OK, "the refusal was kept");
}

/// A caller gone mid-search does not take its engine with it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_caller_gone_gives_its_engine_back() {
    let s = state(1, 60_000);
    let gone = tokio::time::timeout(Duration::from_millis(1), get(app(&s), &uri("ירושלים", 0, 20_000), &[])).await;
    assert!(gone.is_err(), "the search finished before the caller left");
    for _ in 0..600 {
        if s.els.pool.built() == 1 && s.els.pool.idle() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!((s.els.pool.built(), s.els.pool.idle()), (1, 1), "the engine left with the caller");
    assert_eq!(get(app(&s), &uri("משה", 0, 9), &[]).await.status, StatusCode::OK);
}

/// What anyone may keep says so, a CDN included; what is refused does not.
#[tokio::test]
async fn answers_say_they_may_be_kept() {
    let s = state(1, 60_000);
    assert_eq!(get(app(&s), &uri("משה", 0, 9), &[]).await.headers["cache-control"], PUBLIC);
    assert_eq!(get(app(&s), "/api", &[]).await.headers["cache-control"], PUBLIC);
    assert_eq!(get(app(&s), "/api/nothing", &[]).await.headers["cache-control"], "no-store");
}

/// Unset, the endpoint takes a core per engine, a short queue and a cache.
#[test]
fn the_defaults_scale_with_the_machine() {
    let config = Config::from_env();
    if std::env::var_os("ETHAOR_ENGINES").is_none() {
        assert_eq!(config.engines, std::thread::available_parallelism().unwrap().get());
    }
    assert!(config.engines >= 1 && !config.wait.is_zero() && config.cache_bytes > 0);
}
