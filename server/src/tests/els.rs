//! `GET /api/els`: the answer's shape, its refusals, its pages.

use axum::http::StatusCode;
use ethaor::shared::els::{self as shared, Engine, MAX_LIMIT};
use serde_json::Value;

use super::{api_only, els, get};
use crate::api::els::{Answer, ERRORS};

/// `תורה` every fiftieth letter from the first tav of Genesis, named every
/// way an agent may need.
#[tokio::test]
async fn a_hit_names_its_place_every_way() {
    let answer = els(&[("word", "תּוֹרָה"), ("skip_min", "49")]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.headers["content-type"], "application/json");
    let json = answer.json();
    assert_eq!(json["query"]["letters"], "תורה");
    assert_eq!(json["query"]["skip_max"], 49, "skip_max does not default to skip_min");
    assert_eq!(json["returned"], json["hits"].as_array().unwrap().len());
    let first = &json["hits"][0];
    assert_eq!((first["skip"].as_u64(), first["step"].as_u64()), (Some(49), Some(50)));
    let from = &first["from"];
    assert_eq!(from["osis"], "Gen.1.1");
    assert_eq!(from["ref"], "Genesis 1:1");
    assert_eq!(from["hebrew"], "בראשית א,א");
    assert_eq!((from["book"].as_str(), from["book_osis"].as_str()), (Some("Genesis"), Some("Gen")));
    assert_eq!((from["chapter"].as_u64(), from["verse"].as_u64(), from["index"].as_u64()), (Some(1), Some(1), Some(0)));
    let letters = first["letters"].as_array().unwrap();
    let word: String = letters.iter().map(|l| l["char"].as_str().unwrap()).collect();
    assert_eq!(word, "תורה");
    assert_eq!(letters[0]["position"], 5);
    assert_eq!(letters[3]["osis"], first["to"]["osis"]);
}

/// The JSON is the shared layer's page, field for field: the endpoint adds
/// a transport, never a computation.
#[tokio::test]
async fn the_answer_is_the_shared_page() {
    let json = els(&[("word", "שלמה"), ("skip_min", "0"), ("skip_max", "4"), ("offset", "3"), ("limit", "20")]).await.json();
    let query = shared::Query { word: "שלמה".into(), exact: false, skip_min: 0, skip_max: 4, offset: 3, limit: 20, scope: None };
    let page = shared::search(&mut Engine::new(), &query).unwrap();
    assert_eq!(json, serde_json::to_value(Answer::of(&query, &page)).unwrap());
}

/// Following `next_offset` walks every listed hit once, and the last page
/// says null.
#[tokio::test]
async fn next_offset_walks_every_listed_hit() {
    let whole = els(&[("word", "שלמה"), ("limit", &MAX_LIMIT.to_string())]).await.json();
    let mut walked: Vec<Value> = Vec::new();
    let mut offset = Some(0);
    while let Some(at) = offset {
        let page = els(&[("word", "שלמה"), ("offset", &at.to_string()), ("limit", "64")]).await.json();
        walked.extend(page["hits"].as_array().unwrap().iter().cloned());
        offset = page["next_offset"].as_u64();
    }
    assert_eq!(&walked, whole["hits"].as_array().unwrap());
    assert_eq!(walked.len() as u64, whole["listed"].as_u64().unwrap());
    let counted = els(&[("word", "שלמה"), ("limit", "0")]).await.json();
    assert_eq!((counted["total"].clone(), counted["returned"].as_u64()), (whole["total"].clone(), Some(0)));
}

/// Each mistake gets its own code, among those the description announces,
/// with a 400 and a message.
#[tokio::test]
async fn every_refusal_says_why() {
    let cases: [(super::Pairs, &str); 7] = [
        (&[("word", "\u{05B8}\u{05B7}")], "no_hebrew_letter"),
        (&[], "bad_param"),
        (&[("word", "תורה"), ("skip", "3")], "bad_param"),
        (&[("word", "תורה"), ("limit", "-1")], "bad_param"),
        (&[("word", "torah")], "no_hebrew_letter"),
        (&[("word", "תורה"), ("skip_min", "5"), ("skip_max", "4")], "skip_range"),
        (&[("word", "תורה"), ("limit", &(MAX_LIMIT + 1).to_string())], "limit"),
    ];
    for (pairs, code) in cases {
        let answer = els(pairs).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{pairs:?}");
        let json = answer.json();
        assert_eq!(json["error"]["code"], code, "{pairs:?}");
        assert!(!json["error"]["message"].as_str().unwrap().is_empty());
        assert!(ERRORS.contains(&code));
    }
}

#[tokio::test]
async fn an_unknown_endpoint_answers_json() {
    let answer = get(api_only(), "/api/nothing", &[]).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(answer.json()["error"]["code"], "not_found");
}

/// A flood over the HTTP side: the exact total beyond the listed hits, and
/// the last listed page closing the walk.
#[tokio::test]
async fn a_flood_answers_its_total_and_its_last_page() {
    let first = els(&[("word", "את"), ("skip_max", "200"), ("limit", "1")]).await.json();
    let listed = first["listed"].as_u64().unwrap();
    assert_eq!(listed, ethaor::shared::els::CAP as u64);
    assert!(first["total"].as_u64().unwrap() > listed);
    let last = els(&[("word", "את"), ("skip_max", "200"), ("offset", &(listed - 1).to_string())]).await.json();
    assert_eq!((last["returned"].as_u64(), last["next_offset"].clone()), (Some(1), Value::Null));
}

/// Numbers past any sensible range are taken, not crashed on; past their
/// type, refused as a parameter.
#[tokio::test]
async fn extreme_numbers_answer() {
    let wide = els(&[("word", "את"), ("skip_max", &u32::MAX.to_string()), ("offset", &usize::MAX.to_string())]).await;
    assert_eq!(wide.status, StatusCode::OK);
    assert_eq!(wide.json()["returned"], 0);
    let past = els(&[("word", "את"), ("skip_max", "4294967296")]).await;
    assert_eq!(past.json()["error"]["code"], "bad_param");
}

/// The text not there is the service's state, not the caller's mistake.
#[test]
fn a_missing_text_is_unavailable_not_bad() {
    use crate::api::els::status;
    use ethaor::shared::els::Refusal;
    assert_eq!(status(&Refusal::TextMissing), StatusCode::SERVICE_UNAVAILABLE);
    for refusal in [Refusal::NoLetter, Refusal::SkipRange { min: 1, max: 0 }, Refusal::Limit(MAX_LIMIT + 1)] {
        assert_eq!(status(&refusal), StatusCode::BAD_REQUEST);
    }
}

/// Each way of scoping keeps every letter of every hit inside, and says the
/// verses it kept to.
#[tokio::test]
async fn a_scope_keeps_every_letter_inside() {
    let cases: [(super::Pairs, &str, &str); 4] = [
        (&[("portion", "torah")], "Gen.1.1", "Deut.34.12"),
        (&[("book", "Gen")], "Gen.1.1", "Gen.50.26"),
        (&[("chapter", "Ps.119")], "Ps.119.1", "Ps.119.176"),
        (&[("from", "Gen.12"), ("to", "Exod.3.6")], "Gen.12.1", "Exod.3.6"),
    ];
    let whole = els(&[("word", "אל"), ("skip_max", "5")]).await.json();
    for (scope, first, last) in cases {
        let mut pairs = vec![("word", "אל"), ("skip_max", "5"), ("limit", "1000")];
        pairs.extend_from_slice(scope);
        let json = els(&pairs).await.json();
        let echo = &json["query"]["scope"];
        assert_eq!((echo["from"]["osis"].as_str(), echo["to"]["osis"].as_str()), (Some(first), Some(last)), "{scope:?}");
        let (from, to) = (echo["from"]["index"].as_u64().unwrap(), echo["to"]["index"].as_u64().unwrap());
        assert!(json["total"].as_u64().unwrap() > 0 && json["total"].as_u64() < whole["total"].as_u64(), "{scope:?}");
        for hit in json["hits"].as_array().unwrap() {
            for end in ["from", "to"] {
                let v = hit[end]["index"].as_u64().unwrap();
                assert!(from <= v && v <= to, "{scope:?}: {} outside the passage", hit[end]["osis"]);
            }
        }
    }
    assert_eq!(whole["query"]["scope"], Value::Null);
}

/// Each scoping mistake gets its own code.
#[tokio::test]
async fn a_scope_mistake_says_which() {
    let cases: [(super::Pairs, &str); 5] = [
        (&[("book", "Matthew")], "unknown_reference"),
        (&[("chapter", "Gen")], "unknown_reference"),
        (&[("portion", "torah"), ("book", "Gen")], "scope_conflict"),
        (&[("from", "Exod"), ("to", "Gen")], "empty_scope"),
        (&[("from", "Gen.99")], "unknown_reference"),
    ];
    for (scope, code) in cases {
        let mut pairs = vec![("word", "אל")];
        pairs.extend_from_slice(scope);
        let answer = els(&pairs).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{scope:?}");
        assert_eq!(answer.json()["error"]["code"], code, "{scope:?}");
    }
    let named = els(&[("word", "אל"), ("book", "Matthew")]).await.json();
    assert!(named["error"]["message"].as_str().unwrap().contains("Matthew"));
}

/// **`exact=true` tells the final forms apart**: the answer says it searched
/// exactly, its word keeps its final, every letter is that form, and it
/// finds a part of what the folded search finds — each spelling its share.
#[tokio::test]
async fn exact_letters_tell_the_final_forms_apart() {
    let folded = els(&[("word", "אדם"), ("limit", "0")]).await.json();
    let mut totals = 0;
    for word in ["אדם", "אדמ"] {
        let json = els(&[("word", word), ("exact", "true"), ("limit", "20")]).await.json();
        assert_eq!(json["query"]["exact"], true);
        assert_eq!(json["query"]["letters"], word);
        for hit in json["hits"].as_array().unwrap() {
            let spelled: String = hit["letters"].as_array().unwrap().iter().map(|l| l["char"].as_str().unwrap()).collect();
            assert_eq!(spelled, word, "a letter placed as another form");
        }
        totals += json["total"].as_u64().unwrap();
    }
    assert_eq!(folded["query"]["exact"], false);
    assert_eq!(folded["query"]["letters"], "אדמ");
    assert_eq!(Some(totals), folded["total"].as_u64(), "the spellings do not add up to the folded search");
    let bad = els(&[("word", "אדם"), ("exact", "yes")]).await;
    assert_eq!(bad.json()["error"]["code"], "bad_param", "exact=yes accepted");
}
