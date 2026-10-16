//! `GET /api`: the description an agent — or an MCP server — reads, held to
//! what the endpoints really accept.

use axum::http::StatusCode;
use ethaor::shared::els::{CAP, DEFAULT_LIMIT, MAX_LIMIT, Refusal};

use super::{api_only, get};
use crate::api::els::ERRORS;

#[tokio::test]
async fn the_description_lists_each_endpoint_as_a_tool() {
    let answer = get(api_only(), "/api", &[]).await;
    assert_eq!(answer.status, StatusCode::OK);
    let json = answer.json();
    let tools = json["endpoints"].as_array().unwrap();
    assert!(!tools.is_empty());
    for tool in tools {
        for field in ["name", "method", "path", "description"] {
            assert!(tool[field].as_str().is_some_and(|s| !s.is_empty()), "{field} missing");
        }
        let schema = &tool["input_schema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        for required in schema["required"].as_array().unwrap() {
            assert!(schema["properties"].get(required.as_str().unwrap()).is_some());
        }
    }
}

/// The schemas' numbers are the code's, every property they declare is
/// accepted — the scope's one way at a time — and one they do not is
/// refused; every refusal the code can give is declared.
#[tokio::test]
async fn the_els_schemas_are_what_the_endpoints_take() {
    let json = get(api_only(), "/api", &[]).await.json();
    let tools = json["endpoints"].as_array().unwrap();
    for (name, path, max, default) in [("els_search", "/api/els", MAX_LIMIT, DEFAULT_LIMIT), ("els_stream", "/api/els/stream", CAP, CAP)] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(tool["path"], path);
        let properties = tool["input_schema"]["properties"].as_object().unwrap();
        assert_eq!(properties["limit"]["maximum"], max, "{name}");
        assert_eq!(properties["limit"]["default"], default, "{name}");

        let scope = ["portion", "book", "chapter", "from", "to"];
        let base: Vec<(&str, &str)> = properties
            .keys()
            .filter(|k| !scope.contains(&k.as_str()))
            .map(|k| (k.as_str(), if k == "word" { "תורה" } else { "1" }))
            .collect();
        for way in [&[("portion", "torah")][..], &[("book", "Gen")], &[("chapter", "Gen.1")], &[("from", "Gen.1"), ("to", "Gen.2")]] {
            let mut pairs = base.clone();
            pairs.extend_from_slice(way);
            for (k, _) in way {
                assert!(properties.contains_key(*k), "{k} not declared");
            }
            let answer = get(api_only(), &format!("{path}?{}", super::query(&pairs)), &[]).await;
            assert_eq!(answer.status, StatusCode::OK, "{name} {way:?}");
        }
        let mut more = base.clone();
        more.push(("extra", "1"));
        let answer = get(api_only(), &format!("{path}?{}", super::query(&more)), &[]).await;
        assert_eq!(answer.json()["error"]["code"], "bad_param", "{name}");

        let declared: Vec<&str> = tool["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
        assert_eq!(declared, ERRORS);
    }
    let refusals = [
        Refusal::NoLetter,
        Refusal::SkipRange { min: 1, max: 0 },
        Refusal::Limit(0),
        Refusal::TextMissing,
        Refusal::UnknownReference { param: "book", value: String::new() },
        Refusal::ScopeConflict,
        Refusal::EmptyScope,
        Refusal::TooCostly { estimate_ms: 1.0, limit_ms: 0.0 },
    ];
    for refusal in refusals {
        assert!(ERRORS.contains(&refusal.code()), "{} not declared", refusal.code());
    }
}
