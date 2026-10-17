//! `GET /api/chapter`: a chapter's text, verse by verse — the twin of
//! `src/wasm/chapter.rs`, which the page's book view reads. Both run
//! `ethaor::shared::passage`; this one answers JSON.
//!
//! No engine and no cache: a chapter is a few lookups in the frozen corpus,
//! cheaper than a cache's own bookkeeping. Each request is charged its one
//! unit by the guard, like every other.

use axum::extract::Query;
use axum::extract::rejection::QueryRejection;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use ethaor::shared::passage::{self, Chapter};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::els::{VerseRef, chapter_pattern, osis_codes, status};
use super::{PUBLIC, refuse};

/// The query string; unknown names refused, never ignored.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Params {
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Every code this endpoint may refuse with.
pub const ERRORS: [&str; 5] = ["bad_param", "unknown_reference", "rate_limited", "text_missing", "internal"];

/// The tool as `GET /api` lists it.
pub fn tool() -> Value {
    json!({
        "name": "chapter_read",
        "method": "GET",
        "path": "/api/chapter",
        "description": "Reads one chapter of the Tanakh (Mechon Mamre edition, fully pointed Hebrew), verse \
            by verse, each named every way (OSIS, English, Hebrew, global index). Where another publication of \
            the edition (Miqra according to the Masorah) reads otherwise, the verse carries `other_reading`: \
            the verse with that edition's form inserted after ours, and the inserted forms alone. `previous` \
            and `next` name the neighbouring chapters, across books, to read on.",
        "input_schema": {
            "type": "object",
            "properties": {
                "ref": {
                    "type": "string", "pattern": chapter_pattern(),
                    "description": "The chapter, in OSIS: a book's code and the chapter's number \
                        (Gen.1, 1Sam.3, Ps.119); a verse (Gen.1.3) names the chapter holding it.",
                    "examples": ["Gen.1", "1Sam.3", "Ps.119"],
                    "x-book-codes": osis_codes(),
                },
            },
            "required": ["ref"],
            "additionalProperties": false,
        },
        "errors": ERRORS,
    })
}

/// Where the other edition reads otherwise.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct OtherReading {
    /// The verse with the other form inserted after ours, both marked.
    pub shown: &'static str,
    /// The other edition's forms, in the order of the verse.
    pub forms: Vec<&'static str>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Verse {
    #[serde(flatten)]
    pub place: VerseRef,
    pub text: &'static str,
    pub other_reading: Option<OtherReading>,
}

/// A chapter named every way.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ChapterRef {
    pub osis: String,
    #[serde(rename = "ref")]
    pub human: String,
    pub hebrew: String,
    /// Global chapter index, 0-based.
    pub index: usize,
}

impl ChapterRef {
    fn of(chapter: usize) -> Option<ChapterRef> {
        passage::chapter(chapter).map(|c| ChapterRef { osis: c.osis(), human: c.human(), hebrew: c.hebrew(), index: chapter })
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Answer {
    pub chapter: ChapterRef,
    pub book: &'static str,
    pub book_osis: &'static str,
    pub book_hebrew: &'static str,
    pub verses: Vec<Verse>,
    pub previous: Option<ChapterRef>,
    pub next: Option<ChapterRef>,
}

impl Answer {
    pub fn of(read: &Chapter) -> Answer {
        let book = &ethaor::shared::reference::BOOKS[read.book];
        Answer {
            chapter: ChapterRef { osis: read.osis(), human: read.human(), hebrew: read.hebrew(), index: read.chapter },
            book: book.english,
            book_osis: book.osis,
            book_hebrew: read.verses.first().map_or("", |v| v.place.book_hebrew()),
            verses: read
                .verses
                .iter()
                .map(|v| Verse {
                    place: VerseRef::of(&v.place),
                    text: v.text,
                    other_reading: (!v.others.is_empty()).then(|| OtherReading {
                        shown: v.shown,
                        forms: v.others.iter().map(|r| &v.shown[r.clone()]).collect(),
                    }),
                })
                .collect(),
            previous: read.previous().and_then(ChapterRef::of),
            next: read.next().and_then(ChapterRef::of),
        }
    }
}

pub async fn read(params: Result<Query<Params>, QueryRejection>) -> Response {
    let params = match params {
        Ok(Query(params)) => params,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text()),
    };
    match passage::read(&params.reference) {
        Ok(read) => ([(header::CACHE_CONTROL, PUBLIC)], Json(Answer::of(&read))).into_response(),
        Err(r) => refuse(status(&r), r.code(), r.message()),
    }
}
