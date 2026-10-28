//! `GET /api/words`: the answer's shape, its refusals, its pages, its
//! description — and the page's export finding the same hits.

use std::path::Path;

use axum::http::StatusCode;
use ethaor::shared::els::{Engine, MAX_LIMIT, Refusal};
use ethaor::shared::words::{self as shared, LONGEST_WORD, Pick};
use ethaor::wasm::els::els_query_ptr;
use ethaor::wasm::words::{words_hits_len, words_hits_ptr, words_search, words_stride};
use serde_json::Value;

use super::{api_only, get, query, words};
use crate::api::words::{Answer, ERRORS};

/// Genesis opens on בראשית ברא אלהים: its first letters spell בבא from the
/// first verse to the same — `first_verse`, `last_verse` — named every way,
/// each letter the first of its word.
#[tokio::test]
async fn a_words_hit_names_its_place_every_way() {
    let answer = words(&[("word", "בבא")]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.headers["content-type"], "application/json");
    assert!(answer.headers["cache-control"].to_str().unwrap().starts_with("public"));
    let json = answer.json();
    assert_eq!((json["query"]["letter"].as_u64(), json["query"]["from_end"].as_bool()), (Some(1), Some(false)));
    let first = &json["hits"][0];
    assert_eq!(first["from"]["osis"], "Gen.1.1");
    assert_eq!(first["from"]["hebrew"], "בראשית א,א");
    assert_eq!(first["to"]["osis"], "Gen.1.1", "the last letter's verse");
    let positions: Vec<u64> =
        first["letters"].as_array().unwrap().iter().map(|l| l["position"].as_u64().unwrap()).collect();
    assert_eq!(positions, [0, 6, 9], "בראשית ברא אלהים: each word's first letter");
    let last = words(&[("word", "תאם"), ("from_end", "true"), ("exact", "true")]).await.json();
    assert_eq!(last["hits"][0]["from"]["osis"], "Gen.1.1");
}

/// The JSON is the shared layer's page, field for field.
#[tokio::test]
async fn the_words_answer_is_the_shared_page() {
    let json =
        words(&[("word", "שמה"), ("letter", "2"), ("any_order", "true"), ("offset", "3"), ("limit", "20")]).await.json();
    let query = shared::Query {
        word: "שמה".into(),
        exact: false,
        pick: Pick { nth: 2, from_end: false },
        any_order: true,
        offset: 3,
        limit: 20,
        scope: None,
    };
    let page = shared::search(&mut Engine::new(), &query).unwrap();
    assert_eq!(json, serde_json::to_value(Answer::of(&query, &page)).unwrap());
}

/// Following `next_offset` walks every listed hit once.
#[tokio::test]
async fn next_offset_walks_every_listed_words_hit() {
    let whole = words(&[("word", "אמר"), ("limit", &MAX_LIMIT.to_string())]).await.json();
    let (mut walked, mut offset): (Vec<Value>, Option<u64>) = (Vec::new(), Some(0));
    while let Some(at) = offset {
        let page = words(&[("word", "אמר"), ("offset", &at.to_string()), ("limit", "64")]).await.json();
        walked.extend(page["hits"].as_array().unwrap().iter().cloned());
        offset = page["next_offset"].as_u64();
    }
    assert_eq!(&walked, whole["hits"].as_array().unwrap());
    assert_eq!(walked.len() as u64, whole["listed"].as_u64().unwrap());
}

/// Each refusal its code and a 400, never kept: no letter, a letter no
/// word holds, a page too big, a scope unknown, doubled or empty, a name
/// unknown.
#[tokio::test]
async fn each_words_refusal_has_its_code() {
    let past = (LONGEST_WORD + 1).to_string();
    for (pairs, code) in [
        (vec![("word", "abc")], "no_hebrew_letter"),
        (vec![("word", "אב"), ("letter", "0")], "letter_range"),
        (vec![("word", "אב"), ("letter", past.as_str())], "letter_range"),
        (vec![("word", "אב"), ("limit", "1001")], "limit"),
        (vec![("word", "אב"), ("book", "Genesis")], "unknown_reference"),
        (vec![("word", "אב"), ("book", "Gen"), ("portion", "torah")], "scope_conflict"),
        (vec![("word", "אב"), ("from", "Exod"), ("to", "Gen")], "empty_scope"),
        (vec![("word", "אב"), ("skip_min", "1")], "bad_param"),
        (vec![("word", "אב"), ("letter", "-1")], "bad_param"),
    ] {
        let answer = words(&pairs).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{pairs:?}");
        assert_eq!(answer.json()["error"]["code"], code, "{pairs:?}");
        assert_eq!(answer.headers["cache-control"], "no-store", "{pairs:?}");
    }
}

/// A scoped search keeps every letter inside its verses.
#[tokio::test]
async fn a_scoped_words_search_stays_in_its_verses() {
    let json = words(&[("word", "אל"), ("book", "Exod"), ("limit", "1000")]).await.json();
    assert!(json["total"].as_u64().unwrap() > 0);
    for hit in json["hits"].as_array().unwrap() {
        for l in hit["letters"].as_array().unwrap() {
            assert!(l["osis"].as_str().unwrap().starts_with("Exod."), "{l}");
        }
    }
    assert_eq!(json["query"]["scope"]["from"]["osis"], "Exod.1.1");
}

/// Under a ceiling below its estimate, a search is refused before it runs.
#[tokio::test]
async fn a_words_search_too_heavy_is_refused() {
    let config = crate::config::Config { max_cost_ms: 1e-6, ..super::open() };
    let app = crate::app(Path::new("no-such-build"), &config);
    let answer = get(app, &format!("/api/words?{}", query(&[("word", "א")])), &[]).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.json()["error"]["code"], "too_costly");
}

/// `GET /api` declares the tool as it is: every property accepted — the
/// scope one way at a time — an unknown one refused, its bounds the code's,
/// every refusal declared.
#[tokio::test]
async fn the_words_schema_is_what_the_endpoint_takes() {
    let json = get(api_only(), "/api", &[]).await.json();
    let tool = json["endpoints"].as_array().unwrap().iter().find(|t| t["name"] == "words_search").unwrap().clone();
    assert_eq!(tool["path"], "/api/words");
    let properties = tool["input_schema"]["properties"].as_object().unwrap();
    assert_eq!(properties["letter"]["maximum"], LONGEST_WORD);
    assert_eq!(properties["limit"]["maximum"], MAX_LIMIT);
    let scope = ["portion", "book", "chapter", "from", "to"];
    let base: Vec<(&str, &str)> = properties
        .keys()
        .filter(|k| !scope.contains(&k.as_str()))
        .map(|k| match (k.as_str(), properties[k]["type"].as_str()) {
            ("word", _) => (k.as_str(), "אב"),
            (_, Some("boolean")) => (k.as_str(), "true"),
            _ => (k.as_str(), "1"),
        })
        .collect();
    for way in [&[("portion", "torah")][..], &[("book", "Gen")], &[("chapter", "Gen.1")], &[("from", "Gen.1"), ("to", "Gen.2")]] {
        let mut pairs = base.clone();
        pairs.extend_from_slice(way);
        let answer = get(api_only(), &format!("/api/words?{}", query(&pairs)), &[]).await;
        assert_eq!(answer.status, StatusCode::OK, "{way:?}");
    }
    let mut more = base.clone();
    more.push(("extra", "1"));
    assert_eq!(words(&more).await.json()["error"]["code"], "bad_param");
    let declared: Vec<&str> = tool["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert_eq!(declared, ERRORS);
    for refusal in [
        Refusal::NoLetter,
        Refusal::LetterRange(0),
        Refusal::Limit(0),
        Refusal::TextMissing,
        Refusal::UnknownReference { param: "book", value: String::new() },
        Refusal::ScopeConflict,
        Refusal::EmptyScope,
        Refusal::TooCostly { estimate_ms: 0.0, limit_ms: 0.0 },
    ] {
        assert!(ERRORS.contains(&refusal.code()), "{} not declared", refusal.code());
    }
}

/// The search exactly as `search-core.ts` drives it: the total, the records,
/// the words of one record.
fn as_the_page(word: &str, nth: u32, from_end: bool, any_order: bool, exact: bool) -> (f64, Vec<u32>, usize) {
    // The query buffer is the ELS search's too: one lock for both.
    let _hold = super::parity::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { core::ptr::copy_nonoverlapping(word.as_ptr(), els_query_ptr(), word.len()) };
    let total = words_search(word.len() as u32, nth, from_end as u32, any_order as u32, exact as u32);
    let read = unsafe { core::slice::from_raw_parts(words_hits_ptr(), words_hits_len() as usize) }.to_vec();
    (total, read, words_stride() as usize)
}

/// **The page's export and the API find the same hits**, their letters in
/// the same verses at the same ranks: first and last letters, a later
/// letter, any order, exact letters, a flood of one letter.
#[tokio::test]
async fn the_page_and_the_api_find_the_same_words_hits() {
    for (word, nth, from_end, any_order, exact) in [
        ("משה", 1, false, false, false),
        ("משה", 1, false, true, false),
        ("ים", 1, true, false, true),
        ("אל", 3, false, true, false),
        ("רא", 2, true, false, false),
        ("א", 1, false, false, false),
    ] {
        let (total, records, stride) = as_the_page(word, nth, from_end, any_order, exact);
        let flag = |on: bool| if on { "true" } else { "false" };
        let json = words(&[
            ("word", word),
            ("letter", &nth.to_string()),
            ("from_end", flag(from_end)),
            ("any_order", flag(any_order)),
            ("exact", flag(exact)),
            ("limit", &MAX_LIMIT.to_string()),
        ])
        .await
        .json();
        assert_eq!(json["total"].as_f64(), Some(total), "{word}");
        assert_eq!(json["listed"].as_u64(), Some((records.len() / stride) as u64), "{word}");
        let hits = json["hits"].as_array().unwrap();
        assert_eq!(hits.len(), (records.len() / stride).min(MAX_LIMIT), "{word}");
        for (hit, r) in hits.iter().zip(records.chunks(stride)) {
            for (l, pair) in hit["letters"].as_array().unwrap().iter().zip(r.chunks(2)) {
                let verse = ethaor::shared::reference::verse(pair[0] as usize).osis();
                let got = (l["osis"].as_str(), l["position"].as_u64());
                assert_eq!(got, (Some(verse.as_str()), Some(pair[1] as u64)), "{word}");
            }
        }
    }
}
