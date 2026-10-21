//! `GET /api/contents`: the table of contents as an agent reads it — every
//! portion, book and chapter named every way, and a parameter refused.

use axum::http::StatusCode;

use super::{api_only, get};

/// Three portions in order, the 39 books under them, the 929 chapters
/// under their books, each named every way; a chapter's OSIS is one
/// `/api/chapter` reads, and the answer may be kept by anyone.
#[tokio::test]
async fn the_contents_name_every_portion_book_and_chapter() {
    let answer = get(api_only(), "/api/contents", &[]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.headers["cache-control"].to_str().unwrap().starts_with("public"));
    let json = answer.json();
    let portions = json["portions"].as_array().unwrap();
    let keys: Vec<_> = portions.iter().map(|p| p["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["torah", "neviim", "ketuvim"]);
    assert_eq!(portions[0]["hebrew"], "תורה");
    let (mut book, mut chapter) = (0, 0);
    for portion in portions {
        for b in portion["books"].as_array().unwrap() {
            assert_eq!(b["index"], book, "a book out of order");
            book += 1;
            for (k, c) in b["chapters"].as_array().unwrap().iter().enumerate() {
                assert_eq!(c["index"], chapter, "a chapter out of order");
                assert_eq!(c["osis"], format!("{}.{}", b["osis"].as_str().unwrap(), k + 1));
                assert_eq!(c["ref"], format!("{} {}", b["name"].as_str().unwrap(), k + 1));
                assert!(c["hebrew"].as_str().unwrap().starts_with(b["hebrew"].as_str().unwrap()));
                chapter += 1;
            }
        }
    }
    assert_eq!((book, chapter), (39, 929));
    let psalms = &portions[2]["books"][0];
    assert_eq!((psalms["osis"].as_str(), psalms["hebrew"].as_str()), (Some("Ps"), Some("תהילים")));
    assert_eq!(psalms["chapters"][149]["hebrew"], "תהילים קנ");
    let osis = psalms["chapters"][22]["osis"].as_str().unwrap();
    let read = get(api_only(), &format!("/api/chapter?ref={osis}"), &[]).await.json();
    assert_eq!(read["chapter"]["index"], psalms["chapters"][22]["index"], "a listed chapter read as another");
}

/// The contents take no parameter: one given is refused with its code and
/// a 400, numbered, never kept.
#[tokio::test]
async fn a_parameter_to_the_contents_is_refused() {
    let answer = get(api_only(), "/api/contents?book=Gen", &[]).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.headers["cache-control"], "no-store");
    let json = answer.json();
    assert_eq!(json["error"]["code"], "bad_param");
    assert!(json["error"]["incident"].as_str().is_some_and(|i| i.starts_with("ETH-")));
}
