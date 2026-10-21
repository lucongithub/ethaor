//! The Tanakh's table of contents: its three portions, their books, and
//! each book's chapters — what the book view offers to go to, and what
//! `GET /api/contents` lists.
//!
//! All of it lies in the map part of the corpus: nothing here waits for the
//! text to land.

use std::ops::Range;

use crate::corpus;

use super::reference::PORTIONS;

/// One book of the contents.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    /// Global index, 0..39.
    pub book: usize,
    /// The portion holding it: 0 Torah, 1 Nevi'im, 2 Ketuvim.
    pub section: usize,
    /// Its name, in Hebrew: the corpus' own.
    pub hebrew: &'static str,
    /// Its chapters, as global indices.
    pub chapters: Range<usize>,
}

impl Book {
    /// Its chapters' numbers, in Hebrew letters, in order.
    pub fn labels(&self) -> impl Iterator<Item = &'static str> {
        self.chapters.clone().map(corpus::chapter_label)
    }
}

/// A portion's name, in Hebrew: `תורה`, `נביאים`, `כתובים`.
pub fn portion_hebrew(section: usize) -> &'static str {
    corpus::section_name(section)
}

/// Every book in the Tanakh's order, each with its portion and chapters.
pub fn books() -> Vec<Book> {
    PORTIONS
        .iter()
        .enumerate()
        .flat_map(|(section, (_, _, books))| {
            books.clone().map(move |book| Book { book, section, hebrew: corpus::book_name(book), chapters: corpus::chapters_of(book) })
        })
        .collect()
}
