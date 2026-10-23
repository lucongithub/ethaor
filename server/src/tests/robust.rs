//! The service never goes down for a request: a panic is answered and the
//! next request served, an engine that panicked is replaced, a bad setting
//! falls back to its default.

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get as route;

use super::get;
use crate::api::pool::{Failure, Pool};
use crate::config::Config;

/// A handler that panics gets a JSON 500, logged — and the same service
/// answers the next request.
#[tokio::test]
async fn a_panic_is_answered_and_the_service_goes_on() {
    async fn boom() -> &'static str {
        panic!("boom")
    }
    let app = crate::harden(Router::new().route("/boom", route(boom)).route("/fine", route(|| async { "fine" })));
    for _ in 0..3 {
        let answer = get(app.clone(), "/boom", &[]).await;
        assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(answer.json()["error"]["code"], "internal");
        assert_eq!(get(app.clone(), "/fine", &[]).await.body, b"fine");
    }
}

/// A search that panics: its caller gets a numbered fault whose origin is
/// the panic's own line and message, the pool's step after it; its engine
/// is dropped rather than lent again half-written, and the pool goes on.
#[tokio::test]
async fn an_engine_that_panicked_is_replaced() {
    crate::fault::install_panic_hook();
    let pool = std::sync::Arc::new(Pool::new(1, Duration::from_secs(5)));
    assert!(matches!(pool.run(|e| e.runs).await, Ok(0)));
    assert_eq!(pool.built(), 1);
    let line = line!() + 1;
    let failed = pool.run(|_| -> u64 { panic!("inside a search") }).await;
    let Err(Failure::Fault(fault)) = failed else { panic!("not a fault: {failed:?}") };
    assert_eq!(fault.code, "search_panicked");
    assert_eq!(fault.chain[0].what, "inside a search");
    assert_eq!(fault.chain[0].at, format!("server/src/tests/robust.rs:{line}"));
    assert!(fault.chain[1].at.starts_with("server/src/api/pool.rs:"), "{:?}", fault.chain);
    assert!(fault.backtrace.as_deref().is_some_and(|b| !b.is_empty()), "the panic's stack is lost");
    assert_eq!((pool.built(), pool.idle()), (0, 0), "the faulty engine was kept");
    assert!(matches!(pool.run(|e| e.runs).await, Ok(0)), "the rebuilt engine is not new");
    assert_eq!((pool.built(), pool.idle()), (1, 1));
}

/// A setting that is not a positive number is ignored for its default; a
/// burst below the heaviest search is raised to it; a short token is no
/// token.
#[test]
fn a_bad_setting_falls_back() {
    let d = Config::default();
    let config = Config::from_vars(|name| {
        Some(
            match name {
                "ETHAOR_ENGINES" => "many",
                "ETHAOR_WAIT_MS" => "-5",
                "ETHAOR_CACHE_MB" => "inf",
                "ETHAOR_MAX_COST_MS" => "800",
                "ETHAOR_IP_BURST_MS" => "100",
                "ETHAOR_IP_RATE_MS" => "NaN",
                "ETHAOR_TRUST_PROXY" => "maybe",
                "ETHAOR_LOGS_TOKEN" => "short",
                "ETHAOR_OPEN" => "maybe",
                _ => return None,
            }
            .to_string(),
        )
    });
    assert_eq!((config.engines, config.wait, config.cache_bytes), (d.engines, d.wait, d.cache_bytes));
    assert_eq!(config.max_cost_ms, 800.0);
    assert!(config.ip_burst_ms > config.max_cost_ms, "the budget does not cover the admitted search");
    assert_eq!(config.ip_rate_ms, d.ip_rate_ms);
    assert!(!config.trust_proxy);
    assert_eq!(config.logs_token, None);
    assert!(!config.open);
    assert_eq!(Config::from_vars(|_| None), d, "nothing set does not give the defaults");
}

/// The browser opens only when asked: off by default, on with `1`, `true`
/// or `yes`, as every on/off setting.
#[test]
fn the_browser_opens_only_when_asked() {
    assert!(!Config::default().open);
    for on in ["1", "true", " yes "] {
        let config = Config::from_vars(|name| (name == "ETHAOR_OPEN").then(|| on.to_string()));
        assert!(config.open, "`{on}` does not open the browser");
    }
}
