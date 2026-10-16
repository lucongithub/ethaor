//! Faults: each numbered once, its number the same in the answer and the
//! log, its code naming one place, its escalation logged whole only when
//! verbose — and the page's own faults landing in the same log.

use std::collections::HashSet;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get as route;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{api_only, get};
use crate::api::report;
use crate::fault::{self, CODES, Fault, Incident, Kind};
use crate::logs;

/// The log line written under an incident, once it is there.
async fn logged(incident: &str) -> Value {
    for _ in 0..100 {
        let lines = super::logged_lines();
        if let Some(line) = lines.iter().rev().find(|l| l.contains(incident) && l.contains("\"fault\"")) {
            let line: Value = serde_json::from_str(line).unwrap();
            let fault: Value = serde_json::from_str(line["fault"].as_str().unwrap()).unwrap();
            // What a log platform indexes, beside the fault, says the same.
            for key in ["incident", "code", "at"] {
                assert_eq!(line[key], fault[key], "{key}: the line and the fault disagree");
            }
            return fault;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("incident {incident} is not in the logs");
}

async fn post(uri: &str, body: String) -> (StatusCode, Vec<u8>) {
    let request = Request::post(uri).header("content-type", "application/json").body(Body::from(body)).unwrap();
    let response = api_only().oneshot(request).await.unwrap();
    let status = response.status();
    (status, response.into_body().collect().await.unwrap().to_bytes().to_vec())
}

#[test]
fn an_incident_number_reads_aloud_and_never_repeats() {
    let mut seen = HashSet::new();
    for _ in 0..20_000 {
        let incident = Incident::new();
        let s = incident.as_str();
        assert_eq!(s.len(), 12, "{s}");
        assert!(s.starts_with("ETH-"));
        assert!(s[4..].bytes().all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b)), "{s}");
        assert_eq!(Incident::parse(s), Some(incident.clone()));
        assert!(seen.insert(incident), "a number came back");
    }
    for bad in ["", "ETH-", "ETH-1234567", "ETH-123456789", "ETH-ILOU1234", "eth-12345678", "XYZ-12345678"] {
        assert_eq!(Incident::parse(bad), None, "{bad}");
    }
}

/// A refusal: its number the same in the answer and in the log, the log
/// saying the code, the place it was raised, and what the caller read.
#[tokio::test]
async fn an_answer_and_its_log_share_the_incident() {
    logs::init();
    let answer = get(api_only(), "/api/els?word=abc", &[]).await;
    let error = &answer.json()["error"];
    let incident = error["incident"].as_str().unwrap().to_string();
    assert!(Incident::parse(&incident).is_some(), "{incident}");
    let line = logged(&incident).await;
    assert_eq!(line["code"], "no_hebrew_letter");
    assert_eq!(line["user_message"], error["message"]);
    assert!(line["at"].as_str().unwrap().starts_with("server/src/api/els.rs:"), "{}", line["at"]);
    assert_eq!(line["status"], 400);
}

/// Every refusal under `/api` carries its number.
#[tokio::test]
async fn every_refusal_is_numbered() {
    for uri in ["/api/els", "/api/els?word=abc", "/api/els?word=%D7%90&book=Matthew", "/api/nothing", "/api/logs"] {
        let json = get(api_only(), uri, &[]).await.json();
        assert!(Incident::parse(json["error"]["incident"].as_str().unwrap_or("")).is_some(), "{uri}");
    }
}

/// Escalated, a fault keeps its number and gains a step per layer, in order,
/// each at its own line; verbose, the log has them all and the stack, else
/// only where it began.
#[test]
fn an_escalation_keeps_its_number_and_its_steps() {
    let first = line!() + 1;
    let fault = Fault::failure("task_lost", "the origin");
    let incident = fault.incident.clone();
    let second = line!() + 1;
    let fault = fault.escalate("a layer above");
    let fault = fault.escalate("the boundary");
    assert_eq!(fault.incident, incident);
    let steps: Vec<&str> = fault.chain.iter().map(|s| s.what.as_str()).collect();
    assert_eq!(steps, ["the origin", "a layer above", "the boundary"]);
    assert_eq!(fault.chain[0].at, format!("server/src/tests/faults.rs:{first}"));
    assert_eq!(fault.chain[1].at, format!("server/src/tests/faults.rs:{second}"));
    assert_eq!((fault.public_code(), fault.kind), ("internal", Kind::Failure));
    assert_eq!(fault.message, fault::FAILED, "the user message leaks the detail");

    let normal = fault.fields(false);
    assert_eq!(normal["at"], format!("server/src/tests/faults.rs:{first}"));
    assert_eq!(normal["code"], "task_lost");
    assert!(!normal.contains_key("chain") && !normal.contains_key("backtrace"));
    let verbose = fault.fields(true);
    assert_eq!(verbose["chain"].as_array().unwrap().len(), 3);
}

/// A handler that panics: the caller gets `internal`, a number and words it
/// can read; the log, under that number, the panic's own place.
#[tokio::test]
async fn a_panic_is_numbered_at_its_own_line() {
    logs::init();
    fault::install_panic_hook();
    async fn boom() -> &'static str {
        panic!("a handler gave up")
    }
    let line = line!() - 2;
    let app = crate::harden(Router::new().route("/boom", route(boom)));
    let answer = get(app, "/boom", &[]).await;
    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    let error = &answer.json()["error"];
    assert_eq!((error["code"].as_str(), error["message"].as_str()), (Some("internal"), Some(fault::FAILED)));
    let logged = logged(error["incident"].as_str().unwrap()).await;
    assert_eq!(logged["code"], "handler_panicked");
    assert_eq!(logged["at"], format!("server/src/tests/faults.rs:{line}"));
}

/// Each fault code is raised at one place, so it names that place; each
/// raised is in the catalogue.
#[test]
fn every_code_is_raised_at_one_place() {
    let mut sources = String::new();
    for entry in walk("src") {
        let path = entry.to_string_lossy().replace('\\', "/");
        if !path.contains("/tests") && !path.ends_with("fault.rs") {
            sources.push_str(&std::fs::read_to_string(&entry).unwrap());
        }
    }
    for (code, _) in CODES {
        let raised = sources.matches(&format!("\"{code}\"")).count();
        assert_eq!(raised, 1, "{code} is raised {raised} times");
    }
    let known: HashSet<&str> = CODES.iter().map(|(c, _)| *c).collect();
    for prefix in ["Fault::failure(\"", "Failure::ours(\"", "from_panic(\""] {
        for raised in sources.split(prefix).skip(1) {
            let code = &raised[..raised.find('"').unwrap()];
            assert!(known.contains(code), "{code} is not in the catalogue");
        }
    }
}

fn walk(dir: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(path.to_str().unwrap()));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// A page's fault lands in the log under the number its user saw, with its
/// code, place and the message the user read.
#[tokio::test]
async fn a_page_report_is_logged_under_its_number() {
    logs::init();
    let incident = Incident::new().to_string();
    let body = json!({
        "incident": incident,
        "code": "search-failed",
        "message": "The search failed.",
        "source": "worker",
        "chain": [{ "at": "src/els.rs:412:9", "what": "index out of bounds" }, { "at": "search-worker.ts", "what": "a search" }],
        "stack": "RuntimeError: unreachable",
    });
    let (status, _) = post("/api/report", body.to_string()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let line = logged(&incident).await;
    assert_eq!(line["code"], "search-failed");
    assert_eq!(line["at"], "src/els.rs:412:9");
    assert_eq!(line["source"], "worker");
    assert_eq!(line["user_message"], "The search failed.");
}

/// A report is bounded and checked: steps and texts cut, a number not of
/// ours replaced (the one given kept beside it), the code tamed.
#[test]
fn a_report_is_bounded() {
    let steps: Vec<Value> = (0..100).map(|k| json!({ "at": "x".repeat(5000), "what": format!("{k}") })).collect();
    let report: report::Report = serde_json::from_value(json!({
        "incident": "not-a-number",
        "code": "Has Spaces And CAPS",
        "message": "m".repeat(9000),
        "chain": steps,
        "stack": "s".repeat(9000),
    }))
    .unwrap();
    let (incident, fields) = report::fields(&report, true);
    assert!(Incident::parse(incident.as_str()).is_some());
    assert_eq!(fields["incident_given"], "not-a-number");
    assert_eq!(fields["code"], "unnamed");
    assert_eq!(fields["chain"].as_array().unwrap().len(), report::STEPS);
    assert_eq!(fields["at"].as_str().unwrap().chars().count(), report::TEXT);
    assert_eq!(fields["user_message"].as_str().unwrap().chars().count(), report::TEXT);
    assert_eq!(fields["stack"].as_str().unwrap().chars().count(), report::TEXT);
    let (_, normal) = report::fields(&report, false);
    assert!(!normal.contains_key("chain") && !normal.contains_key("stack"));
}

/// A malformed or oversized report is refused, numbered; a report is a POST.
#[tokio::test]
async fn a_bad_report_is_refused() {
    let (status, body) = post("/api/report", "{not json".into()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "bad_report");
    assert!(Incident::parse(json["error"]["incident"].as_str().unwrap()).is_some());
    let huge = json!({ "incident": "x", "code": "x", "stack": "s".repeat(report::BODY) }).to_string();
    assert_eq!(post("/api/report", huge).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    let read = get(api_only(), "/api/report", &[]).await;
    assert_eq!(read.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(read.headers["allow"], "POST");
}

/// One log line per fault, whatever layers it crossed: of all the lines a
/// failing request wrote, besides the one saying it was answered, there is
/// one — the fault's, under its number.
#[tokio::test]
async fn a_fault_is_logged_once() {
    logs::init();
    let config = crate::config::Config { ip_burst_ms: 2.0, ip_rate_ms: 1e-6, ..super::open() };
    let app = crate::app_with(std::path::Path::new("no-such-build"), crate::api::Api::new(&config));
    let me = [("x-forwarded-for", "203.0.113.77")];
    let refused = get(app.clone(), "/api/els?word=abc", &me).await;
    get(app.clone(), "/api", &me).await;
    let limited = get(app, "/api", &me).await;
    assert_eq!(limited.json()["error"]["code"], "rate_limited");
    for answer in [refused, limited] {
        let incident = answer.json()["error"]["incident"].as_str().unwrap().to_string();
        let request = answer.headers["x-request-id"].to_str().unwrap().to_string();
        logged(&incident).await;
        let lines = super::logged_lines();
        let written: Vec<&str> = lines
            .iter()
            .map(|l| l.as_str())
            .filter(|l| l.contains(&request) && !l.contains("\"message\":\"answered\""))
            .collect();
        assert_eq!(written.len(), 1, "{incident} : {written:#?}");
        assert!(written[0].contains(&incident));
    }
}
