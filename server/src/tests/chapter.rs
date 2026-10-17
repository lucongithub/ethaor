//! `GET /api/chapter`: a chapter read as an agent reads it — every verse
//! named every way, the other edition's forms set apart, the neighbours to
//! read on, and each refusal its code.

use axum::http::StatusCode;
use ethaor::shared::passage;

use super::{Answer, api_only, get, query};

async fn chapter(pairs: &[(&str, &str)]) -> Answer {
    get(api_only(), &format!("/api/chapter?{}", query(pairs)), &[]).await
}

/// A chapter answers its verses in order, each our text and named every
/// way — OSIS, English, Hebrew — the chapter too, its neighbours across
/// books, and may be kept by anyone.
#[tokio::test]
async fn a_chapter_is_read_verse_by_verse_and_named_every_way() {
    let answer = chapter(&[("ref", "Gen.50")]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.headers["cache-control"].to_str().unwrap().starts_with("public"));
    let json = answer.json();
    assert_eq!(json["chapter"]["osis"], "Gen.50");
    assert_eq!(json["chapter"]["ref"], "Genesis 50");
    assert_eq!(json["chapter"]["hebrew"], "בראשית נ");
    assert_eq!(json["book_osis"], "Gen");
    assert_eq!(json["previous"]["osis"], "Gen.49");
    assert_eq!(json["next"]["osis"], "Exod.1", "the next chapter does not cross into Exodus");
    let read = passage::chapter(49).unwrap();
    let verses = json["verses"].as_array().unwrap();
    assert_eq!(verses.len(), read.verses.len());
    for (k, (got, want)) in verses.iter().zip(&read.verses).enumerate() {
        assert_eq!(got["text"], want.text, "verse {k}");
        assert_eq!(got["index"], want.place.verse, "verse {k}");
        assert_eq!(got["osis"], want.place.osis(), "verse {k}");
        assert!(got["other_reading"].is_null(), "Genesis 50 holds no other reading");
    }
    let first = chapter(&[("ref", "Gen.1")]).await.json();
    assert!(first["previous"].is_null(), "Genesis 1 has a chapter before it");
    let verse = chapter(&[("ref", "Gen.1.3")]).await.json();
    assert_eq!(verse["chapter"]["osis"], "Gen.1", "a verse does not name its chapter");
}

/// Where the other edition reads otherwise, the verse carries it: the verse
/// as shown, and the other forms alone, each found in it.
#[tokio::test]
async fn a_verse_carries_the_other_reading_where_there_is_one() {
    let json = chapter(&[("ref", "Gen.14")]).await.json();
    let verses = json["verses"].as_array().unwrap();
    let carrying: Vec<_> = verses.iter().filter(|v| !v["other_reading"].is_null()).collect();
    assert!(!carrying.is_empty(), "Genesis 14 carries no other reading");
    for v in carrying {
        let shown = v["other_reading"]["shown"].as_str().unwrap();
        let forms = v["other_reading"]["forms"].as_array().unwrap();
        assert!(!forms.is_empty());
        for form in forms {
            let form = form.as_str().unwrap();
            assert!(shown.contains(form) && form.contains('²'), "{form:?} is not theirs in {shown:?}");
        }
        assert_ne!(shown, v["text"], "a verse with another reading shown as ours alone");
    }
}

/// Each mistake its code and a 400, numbered, never kept.
#[tokio::test]
async fn a_chapter_not_named_is_refused_with_its_code() {
    for (uri, code) in [
        ("/api/chapter", "bad_param"),
        ("/api/chapter?ref=Gen.1&extra=1", "bad_param"),
        ("/api/chapter?ref=Gen", "unknown_reference"),
        ("/api/chapter?ref=Gen.51", "unknown_reference"),
        ("/api/chapter?ref=Matthew.1", "unknown_reference"),
        ("/api/chapter?ref=Genesis.1", "unknown_reference"),
        ("/api/chapter?ref=Bereshit.1", "unknown_reference"),
        ("/api/chapter?ref=Gen%201", "unknown_reference"),
    ] {
        let answer = get(api_only(), uri, &[]).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(answer.headers["cache-control"], "no-store", "{uri}");
        let json = answer.json();
        assert_eq!(json["error"]["code"], code, "{uri}");
        assert!(json["error"]["incident"].as_str().is_some_and(|i| i.starts_with("ETH-")), "{uri}");
    }
}
