//! `GET /api/gematria`: every method named every way beside its value, its
//! refusals, its description. Its WebAssembly twin, the page's reading of a
//! selection, is held to it in `parity.rs`.

use axum::http::StatusCode;
use ethaor::shared::els::Refusal;
use ethaor::shared::gematria::{self as shared, MAX_LETTERS, Method};

use super::{api_only, get, query};
use crate::api::gematria::{Answer, ERRORS, SEARCH_ERRORS, SearchAnswer, keys, method_pattern};

async fn ask(pairs: &[(&str, &str)]) -> super::Answer {
    get(api_only(), &format!("/api/gematria?{}", query(pairs)), &[]).await
}

/// Without `method`, every method answers, in the catalogue's order, each
/// with its key, its English and Hebrew names, the names it also goes by,
/// its description and its value — and a method replacing letters, the
/// letters it reads; the text is echoed with the consonants read, and the
/// answer may be kept by anyone.
#[tokio::test]
async fn every_method_answers_named_every_way() {
    let answer = ask(&[("text", "תּוֹרָה")]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.headers["cache-control"].to_str().unwrap().starts_with("public"));
    let json = answer.json();
    assert_eq!(json["query"]["text"], "תּוֹרָה");
    assert_eq!(json["query"]["letters"], "תורה");
    assert_eq!((json["query"]["letter_count"].as_u64(), json["query"]["words"].as_u64()), (Some(4), Some(1)));
    let methods = json["methods"].as_array().unwrap();
    let answered: Vec<&str> = methods.iter().map(|m| m["method"].as_str().unwrap()).collect();
    assert_eq!(answered, keys(), "not every method, or out of order");
    for (m, method) in methods.iter().zip(Method::ALL) {
        let info = shared::info(method);
        assert_eq!((m["name"].as_str(), m["hebrew"].as_str()), (Some(info.name), Some(info.hebrew)));
        assert_eq!(m["description"], info.description);
        assert_eq!(m["also_known_as"].as_array().unwrap().len(), info.also_known_as.len());
    }
    assert_eq!(methods[0]["method"], "mispar_hapanim");
    assert_eq!(methods[0]["hebrew"], "מספר הפנים");
    assert_eq!(methods[0]["value"], 611);
    assert!(methods[0].get("letters").is_none(), "a method weighing the letters gives letters");
    let atbash = methods.iter().find(|m| m["method"] == "atbash").unwrap();
    assert_eq!(atbash["letters"], "אפגצ");
}

/// The JSON is the shared layer's reading, field for field.
#[tokio::test]
async fn the_gematria_answer_is_the_shared_reading() {
    let text = "בְּרֵאשִׁית בָּרָא אֱלֹהִים";
    let json = ask(&[("text", text)]).await.json();
    let reading = shared::read(text, &Method::ALL).unwrap();
    assert_eq!(json, serde_json::to_value(Answer::of(text, &reading)).unwrap());
}

/// `method` keeps to the keys asked, in the catalogue's order, each once.
#[tokio::test]
async fn method_keeps_to_the_keys_asked() {
    let json = ask(&[("text", "אחד"), ("method", "mispar_katan_mispari,mispar_boneeh,mispar_boneeh")]).await.json();
    let methods = json["methods"].as_array().unwrap();
    let answered: Vec<(&str, u64)> =
        methods.iter().map(|m| (m["method"].as_str().unwrap(), m["value"].as_u64().unwrap())).collect();
    assert_eq!(answered, [("mispar_boneeh", 23), ("mispar_katan_mispari", 4)]);
}

/// Each refusal its code, a 400 and nothing kept: no Hebrew letter, a text
/// past the bound, a method no key names, a parameter missing or unknown.
#[tokio::test]
async fn each_gematria_refusal_has_its_code() {
    let over: String = std::iter::repeat_n('א', MAX_LETTERS + 1).collect();
    let longest: String = std::iter::repeat_n('א', MAX_LETTERS).collect();
    assert_eq!(ask(&[("text", &longest)]).await.status, StatusCode::OK, "the bound itself is refused");
    for (pairs, code) in [
        (&[("text", "abc")][..], "no_hebrew_letter"),
        (&[("text", "")], "no_hebrew_letter"),
        (&[("text", over.as_str())], "text_too_long"),
        (&[("text", "א"), ("method", "gematria")], "unknown_method"),
        (&[("text", "א"), ("method", "")], "unknown_method"),
        (&[("method", "atbash")], "bad_param"),
        (&[("text", "א"), ("alef", "111")], "bad_param"),
    ] {
        let answer = ask(pairs).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{code}");
        assert_eq!(answer.json()["error"]["code"], code);
        assert!(ERRORS.contains(&code), "{code} not declared");
        assert!(answer.headers["cache-control"].to_str().unwrap().contains("no-store"), "a refusal may be kept");
    }
}

/// The tool declares what the endpoint takes — `text` and `method`, and
/// nothing else — `method`'s pattern takes every key alone and listed and
/// nothing else, `x-methods` names the catalogue, and every refusal the
/// code can give is declared.
#[tokio::test]
async fn the_gematria_schema_is_what_the_endpoint_takes() {
    let json = get(api_only(), "/api", &[]).await.json();
    let tools = json["endpoints"].as_array().unwrap();
    let tool = tools.iter().find(|t| t["name"] == "gematria").expect("gematria not listed").clone();
    assert_eq!(tool["path"], "/api/gematria");
    let properties = tool["input_schema"]["properties"].as_object().unwrap();
    assert_eq!(properties.keys().collect::<Vec<_>>(), ["method", "text"]);
    assert_eq!(tool["input_schema"]["required"], serde_json::json!(["text"]));
    let listed: Vec<&str> =
        properties["method"]["x-methods"].as_array().unwrap().iter().map(|m| m["method"].as_str().unwrap()).collect();
    assert_eq!(listed, keys());
    assert_eq!(properties["method"]["pattern"], method_pattern());
    let pattern = keys_pattern(&method_pattern());
    for key in keys() {
        assert!(pattern(key), "{key} does not match");
        assert!(pattern(&format!("{key},atbash")), "{key},atbash does not match");
        assert_eq!(ask(&[("text", "א"), ("method", key)]).await.status, StatusCode::OK, "{key}");
    }
    for bad in ["", "atbash,", "AtBash", "nope", "atbash, albam"] {
        assert!(!pattern(bad), "{bad:?} matches");
    }
    let declared: Vec<&str> = tool["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(declared, ERRORS);
    for refusal in [Refusal::NoLetter, Refusal::TooManyLetters, Refusal::UnknownMethod(String::new())] {
        assert!(ERRORS.contains(&refusal.code()), "{} not declared", refusal.code());
    }
}

/// The declared pattern, read the way an agent's validator reads it: keys
/// from the alternation, parted by commas.
fn keys_pattern(pattern: &str) -> impl Fn(&str) -> bool + use<> {
    let inner = pattern.strip_prefix("^(").and_then(|p| p.split_once(')')).map(|(keys, _)| keys.to_string()).unwrap();
    assert_eq!(pattern, format!("^({inner})(,({inner}))*$"), "the pattern is not keys parted by commas");
    let keys: Vec<String> = inner.split('|').map(String::from).collect();
    move |s: &str| s.split(',').all(|k| keys.iter().any(|key| key == k))
}

// ---------------------------------------------------------------- the search

async fn search(pairs: &[(&str, &str)]) -> super::Answer {
    get(api_only(), &format!("/api/gematria/search?{}", query(pairs)), &[]).await
}

/// A search's answer names the method and the text's value, each hit its
/// verse every way and the words found, pointed, worth that value; Mispar
/// haPanim when no method is named; the answer may be kept by anyone.
#[tokio::test]
async fn a_gematria_hit_names_its_place_and_its_words() {
    let answer = search(&[("text", "בראשית ברא")]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.headers["cache-control"].to_str().unwrap().starts_with("public"));
    let json = answer.json();
    assert_eq!(json["query"]["method"], "mispar_hapanim");
    assert_eq!(json["query"]["hebrew"], "מספר הפנים");
    assert_eq!(json["value"], 1116);
    let first = &json["hits"][0];
    assert_eq!(first["osis"], "Gen.1.1");
    assert_eq!(first["hebrew"], "בראשית א,א");
    assert_eq!((first["from_letter"].as_u64(), first["to_letter"].as_u64()), (Some(0), Some(9)));
    for hit in json["hits"].as_array().unwrap() {
        let words = hit["words"].as_str().unwrap();
        assert_eq!(shared::read(words, &[Method::MisparHapanim]).unwrap().values[0].1, 1116, "{words}");
    }
    let gadol = search(&[("text", "אחד"), ("method", "mispar_gadol"), ("limit", "0")]).await.json();
    assert_eq!((gadol["query"]["method"].as_str(), gadol["returned"].as_u64()), (Some("mispar_gadol"), Some(0)));
}

/// The JSON is the shared layer's page, field for field, and `next_offset`
/// walks every listed hit.
#[tokio::test]
async fn the_gematria_search_answer_is_the_shared_page() {
    let pairs = [("text", "אהבה"), ("method", "atbash"), ("limit", "3")];
    let json = search(&pairs).await.json();
    let q = shared::Query { text: "אהבה".into(), method: Method::Atbash, offset: 0, limit: 3, scope: None };
    let page = shared::search(&q).unwrap();
    assert_eq!(json, serde_json::to_value(SearchAnswer::of(&q, &page)).unwrap());
    let (mut offset, mut walked) = (Some(0u64), 0);
    while let Some(at) = offset {
        let page = search(&[("text", "אהבה"), ("method", "atbash"), ("limit", "1000"), ("offset", &at.to_string())]).await.json();
        walked += page["returned"].as_u64().unwrap();
        offset = page["next_offset"].as_u64();
    }
    assert_eq!(walked, json["listed"].as_u64().unwrap());
}

/// Each refusal of the search its code and status, nothing kept; a scope
/// keeps every hit inside it.
#[tokio::test]
async fn each_gematria_search_refusal_has_its_code() {
    for (pairs, code) in [
        (&[("text", "abc")][..], "no_hebrew_letter"),
        (&[("text", "א"), ("method", "nope")], "unknown_method"),
        (&[("text", "א"), ("limit", "100000")], "limit"),
        (&[("text", "א"), ("book", "Matthew")], "unknown_reference"),
        (&[("text", "א"), ("book", "Gen"), ("portion", "torah")], "scope_conflict"),
        (&[("text", "א"), ("from", "Exod"), ("to", "Gen")], "empty_scope"),
        (&[("text", "א"), ("extra", "1")], "bad_param"),
    ] {
        let answer = search(pairs).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{code}");
        assert_eq!(answer.json()["error"]["code"], code);
        assert!(SEARCH_ERRORS.contains(&code), "{code} not declared");
        assert!(answer.headers["cache-control"].to_str().unwrap().contains("no-store"));
    }
    let kept = search(&[("text", "אהבה"), ("book", "Song"), ("limit", "1000")]).await.json();
    assert!(kept["total"].as_u64().unwrap() > 0);
    for hit in kept["hits"].as_array().unwrap() {
        assert_eq!(hit["book_osis"], "Song", "a hit outside the book");
    }
}

/// A search too heavy for the ceiling is refused before it runs, with its
/// estimate; the ceiling of the tests' service admits it.
#[tokio::test]
async fn a_gematria_search_too_heavy_is_refused() {
    let config = crate::config::Config { max_cost_ms: 0.001, ..super::open() };
    let app = crate::app(std::path::Path::new("no-such-build"), &config);
    let answer = get(app, "/api/gematria/search?text=%D7%90", &[]).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.json()["error"]["code"], "too_costly");
    assert_eq!(search(&[("text", "א")]).await.status, StatusCode::OK);
}

/// The search's tool (`search_tool`) declares what the endpoint takes —
/// every property accepted, the scope one way at a time — its methods are
/// the catalogue's keys, and every refusal it gives is declared.
#[tokio::test]
async fn the_gematria_search_schema_is_what_the_endpoint_takes() {
    let json = get(api_only(), "/api", &[]).await.json();
    let tool = json["endpoints"].as_array().unwrap().iter().find(|t| t["name"] == "gematria_search").expect("not listed").clone();
    assert_eq!(tool["path"], "/api/gematria/search");
    let properties = tool["input_schema"]["properties"].as_object().unwrap();
    let methods: Vec<&str> = properties["method"]["enum"].as_array().unwrap().iter().map(|m| m.as_str().unwrap()).collect();
    assert_eq!(methods, keys());
    assert_eq!(properties["limit"]["maximum"], ethaor::shared::els::MAX_LIMIT);
    let scope = ["portion", "book", "chapter", "from", "to"];
    let base: Vec<(&str, &str)> = properties
        .keys()
        .filter(|k| !scope.contains(&k.as_str()))
        .map(|k| match k.as_str() {
            "text" => (k.as_str(), "אהבה"),
            "method" => (k.as_str(), "atbash"),
            _ => (k.as_str(), "1"),
        })
        .collect();
    for way in [&[("portion", "torah")][..], &[("book", "Gen")], &[("chapter", "Gen.1")], &[("from", "Gen.1"), ("to", "Gen.2")]] {
        let mut pairs = base.clone();
        pairs.extend_from_slice(way);
        assert_eq!(search(&pairs).await.status, StatusCode::OK, "{way:?}");
    }
    let declared: Vec<&str> = tool["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(declared, SEARCH_ERRORS);
    for refusal in [
        Refusal::NoLetter,
        Refusal::TooManyLetters,
        Refusal::UnknownMethod(String::new()),
        Refusal::Limit(0),
        Refusal::TextMissing,
        Refusal::UnknownReference { param: "book", value: String::new() },
        Refusal::ScopeConflict,
        Refusal::EmptyScope,
        Refusal::TooCostly { estimate_ms: 0.0, limit_ms: 0.0 },
    ] {
        assert!(SEARCH_ERRORS.contains(&refusal.code()), "{} not declared", refusal.code());
    }
}
