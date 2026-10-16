//! Under `/api`, every answer is JSON — a wrong path, a wrong method, a slash
//! too many — and many callers at once get what each would alone.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::{api_only, els, get};

#[tokio::test]
async fn a_trailing_slash_still_describes() {
    let (bare, slash) = (get(api_only(), "/api", &[]).await, get(api_only(), "/api/", &[]).await);
    assert_eq!(slash.status, StatusCode::OK);
    assert_eq!(slash.json(), bare.json(), "/api/ does not describe the API");
}

#[tokio::test]
async fn a_wrong_path_or_method_answers_json() {
    let missing = get(api_only(), "/api/els/more", &[]).await;
    assert_eq!((missing.status, missing.json()["error"]["code"].clone()), (StatusCode::NOT_FOUND, "not_found".into()));
    for (method, path) in [(Method::POST, "/api/els?word=%D7%90"), (Method::DELETE, "/api"), (Method::PUT, "/api/")] {
        let response = api_only().oneshot(Request::builder().method(method.clone()).uri(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{method} {path}");
        assert_eq!(response.headers()["allow"], "GET, HEAD");
        let body: Value = serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(body["error"]["code"], "method_not_allowed", "{method} {path}");
    }
}

/// HEAD says what GET would, without the body.
#[tokio::test]
async fn head_answers_without_a_body() {
    let request = Request::head("/api/els?word=%D7%90%D7%91").body(Body::empty()).unwrap();
    let response = api_only().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert!(response.into_body().collect().await.unwrap().to_bytes().is_empty());
}

/// Callers at once share one engine: each gets the answer it would alone,
/// whatever the order the lock serves them in.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_callers_at_once_get_their_own_answers() {
    let words = ["שלמה", "משה", "אדם", "תורה", "דוד", "ירושלים"];
    let mut alone = Vec::new();
    for w in words {
        alone.push(els(&[("word", w), ("skip_min", "0"), ("skip_max", "3")]).await.json());
    }
    let app = api_only();
    let mut tasks = Vec::new();
    for round in 0..4 {
        for (k, w) in words.iter().enumerate() {
            let app = app.clone();
            let uri = format!("/api/els?{}", super::query(&[("word", w), ("skip_min", "0"), ("skip_max", "3")]));
            tasks.push(tokio::spawn(async move { (k, round, get(app, &uri, &[]).await.json()) }));
        }
    }
    for task in tasks {
        let (k, round, json) = task.await.unwrap();
        assert_eq!(json, alone[k], "{} at round {round}", words[k]);
    }
}
