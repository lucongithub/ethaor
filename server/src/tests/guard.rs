//! What a client may spend: a search too heavy is refused before it runs, a
//! client past its budget waits, and another client is not held by it.

use std::path::Path;

use axum::http::{HeaderMap, StatusCode};
use ethaor::shared::els::{self as shared, Engine};

use super::{get, query};
use crate::api::Api;
use crate::api::guard::Guard;
use crate::config::Config;

fn app(config: Config) -> axum::Router {
    crate::app_with(Path::new("no-such-build"), Api::new(&config))
}

fn uri(word: &str, skip_min: u32, skip_max: u32) -> String {
    let pairs = [("word", word), ("skip_min", &skip_min.to_string()), ("skip_max", &skip_max.to_string())];
    format!("/api/els?{}", query(&pairs))
}

fn estimate(word: &str, skip_min: u32, skip_max: u32) -> f64 {
    let q = shared::Query { word: word.into(), exact: false, skip_min, skip_max, offset: 0, limit: 1, scope: None };
    shared::admit(&mut Engine::new(), &q, f64::MAX).unwrap()
}

/// Past the allowed cost a search is refused before it runs, saying its
/// estimate and how to narrow it; under it, it runs.
#[tokio::test]
async fn a_search_too_heavy_is_refused_before_it_runs() {
    let config = Config { max_cost_ms: 50.0, ..super::open() };
    let api = Api::new(&config);
    let app = crate::app_with(Path::new("no-such-build"), api.clone());
    let heavy = get(app.clone(), &uri("את", 0, 100_000), &[]).await;
    assert_eq!(heavy.status, StatusCode::BAD_REQUEST);
    let json = heavy.json();
    assert_eq!(json["error"]["code"], "too_costly");
    assert!(json["error"]["message"].as_str().unwrap().contains("50 ms"));
    assert_eq!(heavy.headers["cache-control"], "no-store");
    assert_eq!(get(app.clone(), &uri("משה", 49, 49), &[]).await.status, StatusCode::OK);
    // Scoped, the same search is light enough.
    let scoped = format!("{}&chapter=Gen.1", uri("את", 0, 100_000));
    assert_eq!(get(app, &scoped, &[]).await.status, StatusCode::OK);
}

/// Each request costs one: past the burst a client waits, told how long,
/// while another client is served.
#[tokio::test]
async fn a_client_past_its_budget_waits_alone() {
    let config = Config { ip_burst_ms: 5.0, ip_rate_ms: 0.001, max_cost_ms: 1.0, ..super::open() };
    let app = app(config);
    let me = [("x-forwarded-for", "203.0.113.7")];
    for k in 0..5 {
        assert_eq!(get(app.clone(), "/api", &me).await.status, StatusCode::OK, "request {k}");
    }
    let refused = get(app.clone(), "/api", &me).await;
    assert_eq!(refused.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(refused.json()["error"]["code"], "rate_limited");
    assert!(refused.headers["retry-after"].to_str().unwrap().parse::<u64>().unwrap() >= 1);
    assert_eq!(refused.headers["cache-control"], "no-store");
    let other = get(app, "/api", &[("x-forwarded-for", "198.51.100.9")]).await;
    assert_eq!(other.status, StatusCode::OK, "another client pays for the first");
}

/// A new search costs its estimate; the same search from the cache costs
/// one — the budget is spent on engines, not on answers already given.
#[tokio::test]
async fn a_search_costs_its_estimate_and_a_cached_one_almost_nothing() {
    let (first, second) = (uri("ירושלים", 0, 600), uri("ירושלים", 601, 1200));
    let (e1, e2) = (estimate("ירושלים", 0, 600), estimate("ירושלים", 601, 1200));
    // Enough for the first search (one and its estimate) and five cached
    // answers (one each), then half of what the second search costs.
    let burst = (1.0 + e1) + 5.0 + (1.0 + e2) / 2.0;
    let config = Config { max_cost_ms: e1.max(e2) * 2.0, ip_burst_ms: burst, ip_rate_ms: 1e-6, ..super::open() };
    let app = app(config);
    let me = [("x-forwarded-for", "203.0.113.8")];
    assert_eq!(get(app.clone(), &first, &me).await.status, StatusCode::OK);
    for _ in 0..5 {
        assert_eq!(get(app.clone(), &first, &me).await.status, StatusCode::OK, "the cached answer cost something");
    }
    let refused = get(app, &second, &me).await;
    assert_eq!(refused.json()["error"]["code"], "rate_limited", "a new search cost nothing");
}

/// Who the client is: its socket, unless a trusted proxy names it — and
/// only then, or anyone could pick a fresh budget at every request.
#[test]
fn a_client_is_named_by_its_proxy_only_when_trusted() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "203.0.113.1, 10.0.0.1".parse().unwrap());
    let socket = Some("192.0.2.5:443".parse().unwrap());
    let trusted = Guard::new(&Config { trust_proxy: true, ..Config::default() });
    let untrusted = Guard::new(&Config::default());
    assert_eq!(trusted.client(&headers, socket).to_string(), "203.0.113.1");
    assert_eq!(untrusted.client(&headers, socket).to_string(), "192.0.2.5");
    headers.insert("cf-connecting-ip", "2001:db8::1".parse().unwrap());
    assert_eq!(trusted.client(&headers, socket).to_string(), "2001:db8::1");
    headers.insert("cf-connecting-ip", "not an address".parse().unwrap());
    headers.remove("x-forwarded-for");
    assert_eq!(trusted.client(&headers, socket).to_string(), "192.0.2.5");
}
