//! Logs: one JSON line per event, each request's lines carrying its id,
//! written to files that roll within their budget, and followed live from
//! them by the operator alone.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::get;
use crate::api::Api;
use crate::config::Config;
use crate::logs;

const TOKEN: &str = "a-token-long-enough-1234";

fn app() -> axum::Router {
    crate::logs::init();
    let config = Config { logs_token: Some(TOKEN.into()), ..super::open() };
    crate::app_with(Path::new("no-such-build"), Api::new(&config))
}

/// Reads server-sent events until `found` holds for one, or gives up.
async fn follow_until(app: axum::Router, uri: &str, found: impl Fn(&Value) -> bool) -> Option<Value> {
    let request = Request::get(uri).header("authorization", format!("Bearer {TOKEN}")).body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body();
    let mut text = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while let Ok(Some(Ok(frame))) = tokio::time::timeout_at(deadline, body.frame()).await {
        if let Some(data) = frame.data_ref() {
            text.push_str(&String::from_utf8_lossy(data));
            for line in text.lines().filter_map(|l| l.strip_prefix("data: ")) {
                if let Ok(json) = serde_json::from_str::<Value>(line)
                    && found(&json)
                {
                    return Some(json);
                }
            }
        }
    }
    None
}

#[tokio::test]
async fn the_logs_are_off_without_a_token_and_closed_without_it() {
    let off = get(super::api_only(), "/api/logs", &[]).await;
    assert_eq!((off.status, off.json()["error"]["code"].clone()), (StatusCode::FORBIDDEN, "logs_disabled".into()));
    for header in ["Bearer wrong-token-of-some-length", "a-token-long-enough-1234", ""] {
        // Bounded: let in, the stream never ends, and the test must fail, not hang.
        let refused = tokio::time::timeout(Duration::from_secs(5), get(app(), "/api/logs", &[("authorization", header)]))
            .await
            .unwrap_or_else(|_| panic!("{header:?} opened the logs"));
        assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{header:?}");
        assert_eq!(refused.json()["error"]["code"], "unauthorized");
    }
    let bad = get(app(), "/api/logs?level=loud", &[("authorization", &format!("Bearer {TOKEN}"))]).await;
    assert_eq!(bad.json()["error"]["code"], "bad_param");
}

/// A request's lines carry the id it was answered with, and the operator
/// finds them in what the endpoint replays.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_request_is_found_in_the_logs_by_its_id() {
    let app = app();
    let answered = get(app.clone(), "/api/els?word=%D7%9E%D7%A9%D7%94&skip_min=7", &[]).await;
    let id = answered.headers["x-request-id"].to_str().unwrap().to_string();
    let line = follow_until(app.clone(), "/api/logs?history=2000", |l| l["request_id"] == id.as_str() && l["message"] == "answered").await;
    let line = line.expect("the request is not in the logs");
    assert_eq!(line["status"], 200);
    assert_eq!(line["path"], "/api/els");
    assert!(line["ms"].as_f64().is_some() && line["ts_ms"].as_u64().is_some());
    let searched = follow_until(app, "/api/logs?history=2000", |l| l["request_id"] == id.as_str() && l["message"] == "searched").await;
    assert!(searched.is_some_and(|l| l["total"].as_u64().is_some()), "the search was not logged under its id");
}

/// Lines written after the follower arrived reach it live; `level` keeps
/// only what is at least that grave.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_logs_follow_live_and_filter_by_level() {
    let app = app();
    let marker = format!("live-{}", uuid::Uuid::new_v4());
    let writer = {
        let marker = marker.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            tracing::warn!(marker = %marker, "a warning");
            tracing::info!(marker = %marker, "an info");
        })
    };
    let warned = follow_until(app.clone(), "/api/logs?history=0&level=warn", |l| l["marker"] == marker.as_str()).await;
    writer.await.unwrap();
    assert_eq!(warned.unwrap()["level"], "WARN");
    let found_info = follow_until(app, "/api/logs?history=2000&level=warn", |l| {
        l["marker"] == marker.as_str() && l["level"] == "INFO"
    });
    let found_info = tokio::time::timeout(Duration::from_secs(1), found_info).await.ok().flatten();
    assert!(found_info.is_none(), "level=warn let an info through");
}

/// Every answer carries its request id — a refusal too.
#[tokio::test]
async fn every_answer_has_an_id() {
    for path in ["/api", "/api/nothing", "/api/els?word=abc", "/"] {
        let answer = get(super::api_only(), path, &[]).await;
        let id = answer.headers["x-request-id"].to_str().unwrap();
        assert_eq!(uuid::Uuid::parse_str(id).map(|u| u.get_version_num()), Ok(4), "{path}");
    }
}

// --------------------------------------------------------------- the files

/// A directory of the test's own, empty.
fn fresh(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ethaor-logs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A log line of about `len` bytes, newline included, numbered `n`.
fn line(n: usize, level: &str, len: usize) -> String {
    let head = format!("{{\"level\":\"{level}\",\"n\":{n},\"pad\":\"");
    let pad = len.saturating_sub(head.len() + 3);
    format!("{head}{}\"}}\n", "x".repeat(pad))
}

fn number(line: &str) -> usize {
    serde_json::from_str::<Value>(line).unwrap()["n"].as_u64().unwrap() as usize
}

/// The lines of every file left, oldest first.
fn on_disk(dir: &Path) -> Vec<String> {
    logs::files(dir)
        .iter()
        .flat_map(|(_, p)| std::fs::read_to_string(p).unwrap().lines().map(String::from).collect::<Vec<_>>())
        .collect()
}

fn settings(pairs: &[(&str, &str)]) -> (logs::Settings, Option<String>) {
    logs::Settings::from_vars(|name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string()))
}

/// Unset, the logs go to `logs` with the default budget; `off` writes no
/// file; a budget that is not a positive number falls back and says so.
#[test]
fn the_log_settings_are_read_and_fall_back() {
    let mb = |n: f64| (n * (1u64 << 20) as f64) as u64;
    assert_eq!(settings(&[]), (logs::Settings { dir: Some("logs".into()), bytes: mb(logs::Settings::MB) }, None));
    let set = settings(&[("ETHAOR_LOG_DIR", "/var/log/ethaor"), ("ETHAOR_LOG_MB", "64")]).0;
    assert_eq!(set, logs::Settings { dir: Some("/var/log/ethaor".into()), bytes: mb(64.0) });
    for off in ["off", "OFF", " ", ""] {
        assert_eq!(settings(&[("ETHAOR_LOG_DIR", off)]).0.dir, None, "{off:?} still writes a file");
    }
    for bad in ["0", "-5", "inf", "NaN", "many"] {
        let (set, note) = settings(&[("ETHAOR_LOG_MB", bad)]);
        assert_eq!(set.bytes, mb(logs::Settings::MB), "{bad}");
        assert!(note.is_some_and(|n| n.contains("ETHAOR_LOG_MB")), "{bad} fell back without saying so");
    }
}

/// Written past its share, a file rolls to the next; past `FILES` the
/// oldest go. What is left is the newest lines, in order, none cut, and
/// the disk never holds more than its budget.
#[test]
fn the_files_roll_and_stay_within_their_budget() {
    let dir = fresh("roll");
    let budget = 8 * 1000;
    let mut rolling = logs::Rolling::open(&dir, budget).unwrap();
    for n in 0..400 {
        put(&mut rolling, line(n, "INFO", 100).as_bytes());
    }
    let files = logs::files(&dir);
    assert_eq!(files.len(), logs::FILES, "{files:?}");
    let bytes: u64 = files.iter().map(|(_, p)| std::fs::metadata(p).unwrap().len()).sum();
    assert!(bytes <= budget, "{bytes} bytes on disk, over a budget of {budget}");
    for (_, p) in &files {
        assert!(std::fs::metadata(p).unwrap().len() <= budget / logs::FILES as u64, "{} went past its share", p.display());
    }
    let left: Vec<usize> = on_disk(&dir).iter().map(|l| number(l)).collect();
    assert_eq!(left.last(), Some(&399), "the newest line is not on disk");
    assert_eq!(left, (left[0]..400).collect::<Vec<_>>(), "lines lost or out of order between the files");
    let _ = std::fs::remove_dir_all(dir);
}

/// A process starts a file of its own after the last one: what an earlier
/// run wrote is never written over.
#[test]
fn each_process_starts_a_new_file() {
    let dir = fresh("restart");
    let mut first = logs::Rolling::open(&dir, 1 << 20).unwrap();
    put(&mut first, line(1, "INFO", 50).as_bytes());
    drop(first);
    let mut second = logs::Rolling::open(&dir, 1 << 20).unwrap();
    put(&mut second, line(2, "INFO", 50).as_bytes());
    let files = logs::files(&dir);
    assert_eq!(files.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(on_disk(&dir).iter().map(|l| number(l)).collect::<Vec<_>>(), [1, 2]);
    let _ = std::fs::remove_dir_all(dir);
}

/// A directory that cannot be made is an error at start — the service
/// then logs to standard output alone — never a panic.
#[test]
fn a_directory_that_cannot_be_written_is_refused_at_start() {
    let dir = fresh("blocked");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a-file");
    std::fs::write(&file, "not a directory").unwrap();
    assert!(logs::Rolling::open(&file.join("logs"), 1 << 20).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

/// The history is the last lines at the level asked, oldest first, read
/// back across the files.
#[test]
fn the_history_is_read_back_across_files() {
    let dir = fresh("history");
    let mut rolling = logs::Rolling::open(&dir, 8 * 3000).unwrap();
    for n in 0..120 {
        let level = if n % 3 == 0 { "WARN" } else { "INFO" };
        put(&mut rolling, line(n, level, 200).as_bytes());
    }
    assert!(logs::files(&dir).len() > 1, "the test did not cross a file");
    let end = logs::end(&dir);
    let kept: Vec<usize> = logs::recent(&dir, end, usize::MAX, tracing::Level::INFO).iter().map(|l| number(l)).collect();
    assert_eq!(kept, (kept[0]..120).collect::<Vec<_>>(), "the history skipped or reordered lines");
    let last: Vec<usize> = logs::recent(&dir, end, 30, tracing::Level::INFO).iter().map(|l| number(l)).collect();
    assert_eq!(last, (90..120).collect::<Vec<_>>());
    let warned: Vec<usize> = logs::recent(&dir, end, 5, tracing::Level::WARN).iter().map(|l| number(l)).collect();
    assert_eq!(warned, [105, 108, 111, 114, 117]);
    let _ = std::fs::remove_dir_all(dir);
}

async fn next(live: &mut (impl futures_util::Stream<Item = logs::Followed> + Unpin)) -> logs::Followed {
    tokio::time::timeout(Duration::from_secs(5), live.next()).await.expect("the follower stopped").expect("the stream ended")
}

fn number_of(followed: logs::Followed) -> usize {
    match followed {
        logs::Followed::Line(l) => number(&l),
        other => panic!("{other:?} where a line was due"),
    }
}

/// A line half written is neither in the history nor sent live twice: the
/// end stops before it, and the follower sends it once whole.
#[tokio::test]
async fn a_half_written_line_is_sent_once_whole() {
    let dir = fresh("half");
    let mut rolling = logs::Rolling::open(&dir, 1 << 20).unwrap();
    put(&mut rolling, line(1, "INFO", 80).as_bytes());
    let second = line(2, "INFO", 80);
    let (head, tail) = second.split_at(30);
    put(&mut rolling, head.as_bytes());
    let end = logs::end(&dir);
    let past = logs::recent(&dir, end, usize::MAX, tracing::Level::INFO);
    assert_eq!(past.iter().map(|l| number(l)).collect::<Vec<_>>(), [1], "a half line entered the history");
    let mut live = Box::pin(logs::follow(dir.clone(), end, tracing::Level::INFO));
    put(&mut rolling, tail.as_bytes());
    assert_eq!(number_of(next(&mut live).await), 2);
    let _ = std::fs::remove_dir_all(dir);
}

/// A follower reads on as the files roll, every line once and in order;
/// one so slow that files it had not read were deleted is told how many,
/// then goes on from the oldest left.
#[tokio::test]
async fn a_follower_reads_across_files_and_is_told_what_it_missed() {
    let dir = fresh("follow");
    let mut rolling = logs::Rolling::open(&dir, 8 * 1000).unwrap();
    let mut live = Box::pin(logs::follow(dir.clone(), logs::end(&dir), tracing::Level::INFO));
    let mut seen = Vec::new();
    for n in 0..120 {
        put(&mut rolling, line(n, "INFO", 100).as_bytes());
        // Read as it goes: never more than a few files behind.
        if n % 7 == 6 {
            while seen.len() <= n {
                seen.push(number_of(next(&mut live).await));
            }
        }
    }
    while seen.len() < 120 {
        seen.push(number_of(next(&mut live).await));
    }
    assert_eq!(seen, (0..120).collect::<Vec<_>>(), "lines lost, doubled or reordered across files");
    assert!(logs::files(&dir)[0].0 > 1, "the test did not roll out a file");

    // Now far behind: everything it had not read rolled out.
    let from = logs::end(&dir);
    let mut slow = Box::pin(logs::follow(dir.clone(), from, tracing::Level::INFO));
    for n in 120..500 {
        put(&mut rolling, line(n, "INFO", 100).as_bytes());
    }
    let logs::Followed::Lagged(missed) = next(&mut slow).await else { panic!("not told it lagged") };
    let oldest = logs::files(&dir)[0].0;
    assert_eq!(missed, oldest - from.seq, "the files missed are miscounted");
    assert_eq!(number_of(next(&mut slow).await), number(&on_disk(&dir)[0]), "it did not go on from the oldest line left");
    let _ = std::fs::remove_dir_all(dir);
}

/// With no log file, the endpoint says so — a 503, not an empty stream.
#[tokio::test]
async fn no_log_file_is_a_503() {
    let source = crate::api::logs::Source { token: Some(TOKEN.into()), dir: None };
    let app = axum::Router::new().route("/api/logs", axum::routing::get(crate::api::logs::follow)).layer(axum::Extension(source));
    let answer = get(app, "/api/logs", &[("authorization", &format!("Bearer {TOKEN}"))]).await;
    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(answer.json()["error"]["code"], "logs_unavailable");
}

/// A line written then flushed, as the writing thread does once its queue
/// is empty.
fn put(rolling: &mut logs::Rolling, line: impl AsRef<[u8]>) {
    rolling.write_all(line.as_ref()).unwrap();
    rolling.flush().unwrap();
}

/// Lines wait in the buffer until the flush — no system call each — and
/// the flush wakes a waiting follower at once, well before it would have
/// looked on its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_flush_writes_the_lines_and_wakes_a_follower_at_once() {
    let dir = fresh("wake");
    let mut rolling = logs::Rolling::open(&dir, 1 << 20).unwrap();
    let mut live = Box::pin(logs::follow(dir.clone(), logs::end(&dir), tracing::Level::INFO));
    rolling.write_all(line(1, "INFO", 80).as_bytes()).unwrap();
    assert!(on_disk(&dir).is_empty(), "a line reached the disk before the flush");
    // Let the follower look, find nothing, and wait.
    let early = tokio::time::timeout(Duration::from_millis(150), live.next()).await;
    assert!(early.is_err(), "a line came before the flush: {early:?}");
    let flushed = std::time::Instant::now();
    rolling.flush().unwrap();
    assert_eq!(number_of(next(&mut live).await), 1);
    let waited = flushed.elapsed();
    assert!(waited < Duration::from_millis(400), "the follower was not woken: {waited:?}");
    let _ = std::fs::remove_dir_all(dir);
}
