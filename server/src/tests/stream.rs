//! `GET /api/els/stream`: every listed hit as a line, the same hits the
//! pages give, and nothing left running when the caller goes.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::{api_only, els, get, query};

fn lines(body: &[u8]) -> Vec<Value> {
    body.split(|&b| b == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect()
}

/// A head line, one line per hit — the pages' hits, in their order — and a
/// closing line.
#[tokio::test]
async fn a_stream_is_the_pages_in_one_go() {
    let pairs = [("word", "שלמה"), ("skip_max", "6")];
    let answer = get(api_only(), &format!("/api/els/stream?{}", query(&pairs)), &[]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.headers["content-type"], "application/x-ndjson");
    let lines = lines(&answer.body);
    let (head, hits, last) = (&lines[0], &lines[1..lines.len() - 1], &lines[lines.len() - 1]);
    assert_eq!(last, &serde_json::json!({ "done": true }));
    assert_eq!(head["returned"].as_u64(), Some(hits.len() as u64));
    assert_eq!(head["listed"], head["returned"]);

    let mut paged = Vec::new();
    let mut offset = Some(0);
    while let Some(at) = offset {
        let page = els(&[pairs[0], pairs[1], ("offset", &at.to_string()), ("limit", "1000")]).await.json();
        paged.extend(page["hits"].as_array().unwrap().iter().cloned());
        offset = page["next_offset"].as_u64();
    }
    assert_eq!(hits, &paged[..]);
    assert_eq!(head["total"].as_u64(), Some(paged.len() as u64));
}

/// Past a page's bound: a stream lists thousands, up to the cap.
#[tokio::test]
async fn a_stream_goes_past_a_page() {
    let pairs = [("word", "את"), ("skip_max", "50"), ("limit", "5000")];
    let answer = get(api_only(), &format!("/api/els/stream?{}", query(&pairs)), &[]).await;
    let lines = lines(&answer.body);
    assert_eq!(lines.len(), 5000 + 2);
    assert!(lines[0]["total"].as_u64().unwrap() > 5000);
    let too_many = get(api_only(), &format!("/api/els/stream?{}", query(&[("word", "את"), ("limit", "100001")])), &[]).await;
    assert_eq!(too_many.json()["error"]["code"], "limit");
}

/// A refusal is a plain JSON refusal, not a stream.
#[tokio::test]
async fn a_refused_stream_answers_json() {
    let answer = get(api_only(), &format!("/api/els/stream?{}", query(&[("word", "abc")])), &[]).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.json()["error"]["code"], "no_hebrew_letter");
}

/// The caller gone after the first lines: the placing stops, and says so.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stream_left_stops() {
    crate::logs::init();
    let pairs = [("word", "את"), ("skip_max", "400"), ("limit", "100000")];
    let request = Request::get(format!("/api/els/stream?{}", query(&pairs))).body(Body::empty()).unwrap();
    let response = api_only().oneshot(request).await.unwrap();
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap();
    assert!(first.is_data());
    drop(body);
    let mut left = false;
    for _ in 0..200 {
        if super::logged_lines().iter().any(|l| l.contains("stream left by its caller")) {
            left = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(left, "the stream went on without a caller");
}
