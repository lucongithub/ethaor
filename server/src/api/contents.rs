//! `GET /api/contents`: the Tanakh's table of contents — its portions, their
//! books, each book's chapters, all named every way. The twin of
//! `contents_record` in `src/wasm/chapter.rs`, which the book view reads to
//! go to a book. Both run `ethaor::shared::contents`; this one answers JSON.
//!
//! No parameter, no engine, no cache: the answer is a walk of the frozen
//! map, the same for everyone, and kept by anyone (`PUBLIC`).

use axum::extract::Query;
use axum::extract::rejection::QueryRejection;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use ethaor::shared::contents;
use ethaor::shared::reference::{BOOKS, PORTIONS};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{PUBLIC, refuse};

/// The query string: empty, and a name in it refused, never ignored.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Params {}

/// Every code this endpoint may refuse with.
pub const ERRORS: [&str; 3] = ["bad_param", "rate_limited", "internal"];

/// The tool as `GET /api` lists it.
pub fn tool() -> Value {
    json!({
        "name": "contents",
        "method": "GET",
        "path": "/api/contents",
        "description": "The Tanakh's table of contents (Mechon Mamre edition): its three portions — Torah, \
            Nevi'im, Ketuvim — in order, each with its books, each book with its chapters. Every portion, book \
            and chapter is named every way (OSIS, English, Hebrew, global index), so any chapter can then be \
            read with `chapter_read` by its OSIS.",
        "input_schema": {
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false,
        },
        "errors": ERRORS,
    })
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Chapter {
    pub osis: String,
    #[serde(rename = "ref")]
    pub human: String,
    pub hebrew: String,
    /// Global chapter index, 0-based.
    pub index: usize,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Book {
    pub osis: &'static str,
    pub name: &'static str,
    pub hebrew: &'static str,
    /// Global book index, 0-based.
    pub index: usize,
    pub chapters: Vec<Chapter>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Portion {
    /// The key a search's `portion` takes.
    pub key: &'static str,
    pub name: &'static str,
    pub hebrew: &'static str,
    pub books: Vec<Book>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Answer {
    pub portions: Vec<Portion>,
}

impl Answer {
    pub fn of(books: &[contents::Book]) -> Answer {
        let portions = PORTIONS
            .iter()
            .enumerate()
            .map(|(section, (key, name, _))| Portion {
                key,
                name,
                hebrew: contents::portion_hebrew(section),
                books: books.iter().filter(|b| b.section == section).map(Book::of).collect(),
            })
            .collect();
        Answer { portions }
    }
}

impl Book {
    fn of(book: &contents::Book) -> Book {
        let names = &BOOKS[book.book];
        let hebrew = book.hebrew;
        Book {
            osis: names.osis,
            name: names.english,
            hebrew,
            index: book.book,
            chapters: book
                .chapters
                .clone()
                .zip(book.labels())
                .enumerate()
                .map(|(k, (chapter, label))| Chapter {
                    osis: format!("{}.{}", names.osis, k + 1),
                    human: format!("{} {}", names.english, k + 1),
                    hebrew: format!("{hebrew} {label}"),
                    index: chapter,
                })
                .collect(),
        }
    }
}

pub async fn list(params: Result<Query<Params>, QueryRejection>) -> Response {
    if let Err(e) = params {
        return refuse(StatusCode::BAD_REQUEST, "bad_param", e.body_text());
    }
    ([(header::CACHE_CONTROL, PUBLIC)], Json(Answer::of(&contents::books()))).into_response()
}
