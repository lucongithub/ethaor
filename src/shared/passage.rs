//! A chapter as a reader reads it: its verses in order, each with its
//! number, its text, and — where the other publication of the edition reads
//! otherwise — the verse as the map draws it, the other forms set apart.
//!
//! The page's book view reads it from the module's memory
//! (`src/wasm/chapter.rs`), the API as JSON (`server/src/api/chapter.rs`):
//! both from here, so a verse is never cut two ways.

use std::ops::Range;

use crate::{corpus, variant};

use super::els::Refusal;
use super::reference;

/// One verse of a chapter.
#[derive(Debug, Clone, PartialEq)]
pub struct Verse {
    pub place: reference::Verse,
    /// The verse as the corpus holds it: our edition.
    pub text: &'static str,
    /// The verse as it is shown: `text` itself, or, where the other edition
    /// reads otherwise, the display string with its form inserted after ours.
    pub shown: &'static str,
    /// Byte ranges of `shown` that are the other edition's — painted apart.
    /// Empty when `shown` is `text`.
    pub others: Vec<Range<usize>>,
}

/// A chapter, every verse in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    /// Global indices: book 0..39, chapter 0..929.
    pub book: usize,
    pub chapter: usize,
    /// The portion holding its book: 0 Torah, 1 Nevi'im, 2 Ketuvim.
    pub section: usize,
    /// Its number inside its book, from 1.
    pub number: u32,
    /// Whether it is its book's first chapter: where a book begins.
    pub opens: bool,
    pub verses: Vec<Verse>,
}

impl Chapter {
    /// `Gen.1`.
    pub fn osis(&self) -> String {
        format!("{}.{}", reference::BOOKS[self.book].osis, self.number)
    }

    /// `Genesis 1`.
    pub fn human(&self) -> String {
        format!("{} {}", reference::BOOKS[self.book].english, self.number)
    }

    /// `בראשית א`: the corpus' own labels.
    pub fn hebrew(&self) -> String {
        format!("{} {}", corpus::book_name(self.book), corpus::chapter_label(self.chapter))
    }

    /// The chapter before, and after, across books: `None` at either end of
    /// the Tanakh.
    pub fn previous(&self) -> Option<usize> {
        self.chapter.checked_sub(1)
    }

    pub fn next(&self) -> Option<usize> {
        (self.chapter + 1 < corpus::chapter_count()).then_some(self.chapter + 1)
    }
}

/// A chapter by its global index, once its verses have landed; `None` out of
/// range or before — a chapter not landed has no verse to give.
pub fn chapter(chapter: usize) -> Option<Chapter> {
    if chapter >= corpus::chapter_count() || !corpus::chapter_ready(chapter) {
        return None;
    }
    let verses = corpus::verses_of(chapter)
        .map(|v| {
            let text = corpus::verse_text(v);
            let (shown, others) = match variant::display(v) {
                Some(shown) => (
                    shown,
                    variant::inserts_of(v)
                        .map(|insert| {
                            let (at, len) = insert.other_range();
                            at..at + len
                        })
                        .collect(),
                ),
                None => (text, Vec::new()),
            };
            Verse { place: reference::verse(v), text, shown, others }
        })
        .collect::<Vec<_>>();
    let first = corpus::verses_of(chapter).start;
    let place = reference::verse(first);
    let section = reference::PORTIONS.iter().position(|(_, _, books)| books.contains(&place.book)).unwrap_or(0);
    let opens = corpus::chapters_of(place.book).start == chapter;
    Some(Chapter { book: place.book, chapter, section, number: place.chapter_number, opens, verses })
}

/// The chapter a reference names: a chapter (`Gen.1`, `Genesis 1`), or the
/// chapter holding a verse (`Gen.1.3`). A book alone names no chapter.
pub fn named(reference: &str) -> Result<usize, Refusal> {
    let unknown = || Refusal::UnknownReference { param: "ref", value: reference.into() };
    let verses = reference::verses(reference).ok_or_else(unknown)?;
    let chapter = reference::verse(verses.start).chapter;
    // A book's verses run past its first chapter: that is a book, not one.
    if verses.end > corpus::verses_of(chapter).end {
        return Err(unknown());
    }
    Ok(chapter)
}

/// A chapter as a caller names it: refused when the name is not one, or —
/// in the browser only — when its verses have not landed yet.
pub fn read(reference: &str) -> Result<Chapter, Refusal> {
    chapter(named(reference)?).ok_or(Refusal::TextMissing)
}
