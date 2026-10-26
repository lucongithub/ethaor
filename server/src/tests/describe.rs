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
            .map(|k| {
                let value = match (k.as_str(), properties[k]["type"].as_str()) {
                    ("word", _) => "תורה",
                    (_, Some("boolean")) => "true",
                    _ => "1",
                };
                (k.as_str(), value)
            })
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

/// The chapter's tool declares what the endpoint takes — `ref`, and nothing
/// else — and every code it can refuse with.
#[tokio::test]
async fn the_chapter_schema_is_what_the_endpoint_takes() {
    use crate::api::chapter;
    let json = get(api_only(), "/api", &[]).await.json();
    let tools = json["endpoints"].as_array().unwrap();
    let tool = tools.iter().find(|t| t["name"] == "chapter_read").expect("chapter_read not listed");
    assert_eq!(tool["path"], "/api/chapter");
    let properties = tool["input_schema"]["properties"].as_object().unwrap();
    assert_eq!(properties.keys().collect::<Vec<_>>(), ["ref"]);
    assert_eq!(get(api_only(), "/api/chapter?ref=Ps.119", &[]).await.status, StatusCode::OK);
    // The pattern it declares is what it reads: every chapter's OSIS matches
    // it and is read, a name in another notation matches it not.
    let pattern = regex_lite(tool["input_schema"]["properties"]["ref"]["pattern"].as_str().unwrap());
    for c in 0..ethaor::shared::reference::counts()[1] {
        let osis = ethaor::shared::passage::chapter(c).unwrap().osis();
        assert!(pattern(&osis), "{osis} does not match the declared pattern");
    }
    for other in ["Genesis.1", "Gen 1", "gen.1", "Gen.01", "Gen"] {
        assert!(!pattern(other), "{other} matches the declared pattern");
    }
    let declared: Vec<&str> = tool["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(declared, chapter::ERRORS);
    for refusal in [Refusal::UnknownReference { param: "ref", value: String::new() }, Refusal::TextMissing] {
        assert!(chapter::ERRORS.contains(&refusal.code()), "{} not declared", refusal.code());
    }
}

/// The contents' tool declares no parameter and refuses one, and its
/// errors are the endpoint's.
#[tokio::test]
async fn the_contents_schema_is_what_the_endpoint_takes() {
    use crate::api::contents;
    let json = get(api_only(), "/api", &[]).await.json();
    let tools = json["endpoints"].as_array().unwrap();
    let tool = tools.iter().find(|t| t["name"] == "contents").expect("contents not listed");
    assert_eq!(tool["path"], "/api/contents");
    assert!(tool["input_schema"]["properties"].as_object().unwrap().is_empty());
    assert_eq!(get(api_only(), "/api/contents", &[]).await.status, StatusCode::OK);
    assert_eq!(get(api_only(), "/api/contents?x=1", &[]).await.status, StatusCode::BAD_REQUEST);
    let declared: Vec<&str> = tool["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(declared, contents::ERRORS);
}

/// The declared OSIS pattern, read by hand: `^(codes)(\.[1-9][0-9]*){1,2}$`
/// — its alternation must be the 39 codes, checked here without a regex
/// crate.
fn regex_lite(pattern: &str) -> impl Fn(&str) -> bool {
    assert_eq!(pattern, crate::api::els::chapter_pattern(), "the tool declares another pattern");
    let codes: Vec<String> = pattern["^(".len()..pattern.find(')').unwrap()].split('|').map(String::from).collect();
    assert_eq!(codes, crate::api::els::osis_codes(), "the pattern's books are not the 39 codes");
    move |s: &str| {
        let mut parts = s.split('.');
        let book = parts.next().unwrap_or("");
        let numbers: Vec<&str> = parts.collect();
        codes.iter().any(|c| c == book)
            && (1..=2).contains(&numbers.len())
            && numbers.iter().all(|n| n.starts_with(|c: char| ('1'..='9').contains(&c)) && n.bytes().all(|b| b.is_ascii_digit()))
    }
}
