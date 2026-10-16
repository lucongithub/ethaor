//! ELS as both transports ask it: a query checked, the hits paged and each
//! of their letters placed in the text.
//!
//! The search itself is `els::Engine`'s; the browser reads its records
//! straight from memory, the HTTP API reads them through `search` below.
//! Paging and placing live here so the two never disagree on either.

pub use crate::els::{CAP, Engine, STRIDE};
use crate::letter::ALPHABET;
use crate::{corpus, els};

use super::reference;

/// Hits returned when the caller does not say.
pub const DEFAULT_LIMIT: usize = 100;

/// Hits one page may hold: each carries every letter of the word placed in
/// the text, so a page is kept to what a reader — or an agent's context —
/// can take in. The total is exact whatever the page.
pub const MAX_LIMIT: usize = 1000;

/// The consonants in the stream's code order, final forms folded.
const LETTERS: [char; ALPHABET] = [
    'א', 'ב', 'ג', 'ד', 'ה', 'ו', 'ז', 'ח', 'ט', 'י', 'כ', 'ל', 'מ', 'נ', 'ס', 'ע', 'פ', 'צ', 'ק', 'ר', 'ש', 'ת',
];

/// What a caller asks. Hashable: a server keys its answers by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    /// Hebrew; marks are ignored and final forms folded.
    pub word: String,
    /// Letters jumped between two letters of the word, from `skip_min` to
    /// `skip_max` inclusive; 0 reads the word as written.
    pub skip_min: u32,
    pub skip_max: u32,
    /// Rank of the first hit returned, in (skip, start) order.
    pub offset: usize,
    /// Hits returned at most; 0 only counts.
    pub limit: usize,
    /// The verses the search is kept to, as global indices `[first, end)`:
    /// an occurrence counts only if every letter lies in them. `None` is the
    /// whole Tanakh.
    pub scope: Option<(usize, usize)>,
}

/// Why a query is not searched. Each has a stable `code` a program can
/// branch on, and a `message` a person can read.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// No Hebrew consonant in the word.
    NoLetter,
    SkipRange { min: u32, max: u32 },
    Limit(usize),
    /// The text has not landed: only in the browser, before the last book.
    TextMissing,
    /// A reference names no book, chapter or verse: the parameter, then
    /// what it held.
    UnknownReference { param: &'static str, value: String },
    /// Two ways of scoping at once — a portion and a book, say.
    ScopeConflict,
    /// A scope that holds no verse: its end before its start.
    EmptyScope,
    /// Expected to cost more than the caller allows, in V8 milliseconds.
    TooCostly { estimate_ms: f64, limit_ms: f64 },
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::NoLetter => "no_hebrew_letter",
            Refusal::SkipRange { .. } => "skip_range",
            Refusal::Limit(_) => "limit",
            Refusal::TextMissing => "text_missing",
            Refusal::UnknownReference { .. } => "unknown_reference",
            Refusal::ScopeConflict => "scope_conflict",
            Refusal::EmptyScope => "empty_scope",
            Refusal::TooCostly { .. } => "too_costly",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Refusal::NoLetter => "word holds no Hebrew consonant (U+05D0..U+05EA)".into(),
            Refusal::SkipRange { min, max } => format!("skip_min ({min}) is above skip_max ({max})"),
            Refusal::Limit(limit) => format!("limit ({limit}) is above {MAX_LIMIT}"),
            Refusal::TextMissing => "the text has not landed yet".into(),
            Refusal::UnknownReference { param, value } => format!(
                "{param}={value:?} names no book, chapter or verse (use OSIS like Gen, Gen.1, Gen.1.1, \
                 or English like Genesis 1:1; portions: torah, neviim, ketuvim)"
            ),
            Refusal::ScopeConflict => "give one scope only: portion, book, chapter, or from/to".into(),
            Refusal::EmptyScope => "the scope holds no verse: `from` comes after `to`".into(),
            Refusal::TooCostly { estimate_ms, limit_ms } => format!(
                "this search is estimated at {estimate_ms:.0} ms, above the {limit_ms:.0} ms allowed: \
                 narrow skip_min..skip_max, scope it to a portion, book or chapter, or use a rarer word"
            ),
        }
    }
}

/// One letter of a hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Letter {
    /// Its rank among every consonant of the Tanakh, from 0.
    pub rank: u32,
    /// The consonant, final forms folded.
    pub char: char,
    /// The verse it lies in, global index.
    pub verse: usize,
    /// Its rank among that verse's consonants, from 0.
    pub position: u32,
}

/// One occurrence of the word.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub skip: u32,
    /// Each letter of the word, in reading order.
    pub letters: Vec<Letter>,
}

impl Hit {
    pub fn first_verse(&self) -> reference::Verse {
        reference::verse(self.letters[0].verse)
    }

    pub fn last_verse(&self) -> reference::Verse {
        reference::verse(self.letters[self.letters.len() - 1].verse)
    }
}

/// A page of hits and the exact count behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The word as searched: consonants only, finals folded.
    pub letters: String,
    /// Every hit, counted.
    pub total: u64,
    /// Hits that can be paged through: the first `CAP` in (skip, start)
    /// order. Past them the total is still exact.
    pub listed: usize,
    pub offset: usize,
    pub hits: Vec<Hit>,
}

impl Page {
    /// The offset of the next page, if one remains among the listed hits.
    pub fn next_offset(&self) -> Option<usize> {
        let next = self.offset + self.hits.len();
        (!self.hits.is_empty() && next < self.listed).then_some(next)
    }
}

/// Checks a query and folds its word, as the engine will read it.
pub fn check(query: &Query) -> Result<String, Refusal> {
    check_up_to(query, MAX_LIMIT)
}

/// `check`, with another bound on `limit` — a stream lists up to `CAP`.
pub fn check_up_to(query: &Query, max_limit: usize) -> Result<String, Refusal> {
    let mut codes = Vec::new();
    els::pattern(&query.word, &mut codes);
    if codes.is_empty() {
        return Err(Refusal::NoLetter);
    }
    if query.skip_min > query.skip_max {
        return Err(Refusal::SkipRange { min: query.skip_min, max: query.skip_max });
    }
    if query.limit > max_limit {
        return Err(Refusal::Limit(query.limit));
    }
    if let Some((first, end)) = query.scope
        && (first >= end || end > corpus::verse_count())
    {
        return Err(Refusal::EmptyScope);
    }
    if !corpus::has_text() {
        return Err(Refusal::TextMissing);
    }
    Ok(codes.iter().map(|&c| LETTERS[c as usize]).collect())
}

/// The scope a caller names, at most one way: a portion (`torah`), a book
/// (`Gen`), a chapter (`Gen.1`), or a stretch `from` one reference `to`
/// another, both included — either may be left open. `None` is the whole
/// Tanakh.
pub fn scope(
    portion: Option<&str>,
    book: Option<&str>,
    chapter: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Option<(usize, usize)>, Refusal> {
    let unknown = |param: &'static str, value: &str| Refusal::UnknownReference { param, value: value.into() };
    let ways = [portion.is_some(), book.is_some(), chapter.is_some(), from.is_some() || to.is_some()];
    if ways.iter().filter(|&&w| w).count() > 1 {
        return Err(Refusal::ScopeConflict);
    }
    let verses = if let Some(p) = portion {
        let books = reference::portion(p).ok_or_else(|| unknown("portion", p))?;
        reference::book_verses(books.start).start..reference::book_verses(books.end - 1).end
    } else if let Some(b) = book {
        reference::book_verses(reference::book(b).ok_or_else(|| unknown("book", b))?)
    } else if let Some(c) = chapter {
        let verses = reference::verses(c).ok_or_else(|| unknown("chapter", c))?;
        // A chapter is a chapter: a whole book or a single verse is refused.
        if corpus::verses_of(reference::verse(verses.start).chapter) != verses {
            return Err(unknown("chapter", c));
        }
        verses
    } else if from.is_some() || to.is_some() {
        let first = match from {
            Some(f) => reference::verses(f).ok_or_else(|| unknown("from", f))?.start,
            None => 0,
        };
        let end = match to {
            Some(t) => reference::verses(t).ok_or_else(|| unknown("to", t))?.end,
            None => corpus::verse_count(),
        };
        if first >= end {
            return Err(Refusal::EmptyScope);
        }
        first..end
    } else {
        return Ok(None);
    };
    Ok(Some((verses.start, verses.end)))
}

/// The stretch of the letter stream a scope covers.
pub fn letters_of(scope: Option<(usize, usize)>) -> std::ops::Range<usize> {
    let starts = els::verse_starts();
    match scope {
        Some((first, end)) => starts[first] as usize..starts[end] as usize,
        None => 0..els::stream().len(),
    }
}

/// What a query is expected to cost, in V8 milliseconds, without running
/// it. Refuses it past `limit_ms`.
pub fn admit(engine: &mut Engine, query: &Query, limit_ms: f64) -> Result<f64, Refusal> {
    let estimate_ms = engine.cost(&query.word, query.skip_min, query.skip_max, letters_of(query.scope)) / 1e6;
    if estimate_ms > limit_ms {
        return Err(Refusal::TooCostly { estimate_ms, limit_ms });
    }
    Ok(estimate_ms)
}

/// One hit, placed: each letter's rank, consonant, verse and rank there.
pub fn place(start: u32, skip: u32, span: usize) -> Hit {
    let stream = els::stream();
    let letters = (0..span)
        .map(|i| {
            let rank = start as usize + i * (skip as usize + 1);
            let (verse, position) = reference::letter(rank);
            Letter { rank: rank as u32, char: LETTERS[stream[rank] as usize], verse, position: position as u32 }
        })
        .collect();
    Hit { skip, letters }
}

/// What a search found before anything is placed: the word as searched, the
/// exact total, the listed count, and the (start, skip) pairs of the asked
/// page — copied out, so the engine is free while they are placed.
pub struct Found {
    pub letters: String,
    pub total: u64,
    pub listed: usize,
    pub hits: Vec<(u32, u32)>,
}

/// Searches, and keeps the pairs of `limit` hits from `offset`.
pub fn find(engine: &mut Engine, query: &Query, max_limit: usize) -> Result<Found, Refusal> {
    let letters = check_up_to(query, max_limit)?;
    let range = letters_of(query.scope);
    let (total, found) = engine.hits_in(&query.word, query.skip_min, query.skip_max, range);
    let listed = found.len();
    let from = query.offset.min(listed);
    let to = query.offset.saturating_add(query.limit).min(listed);
    Ok(Found { letters, total, listed, hits: found[from..to].to_vec() })
}

/// Searches, then keeps `limit` hits from `offset` and places each of their
/// letters — those of the page only: placing the `CAP` hits a flood keeps,
/// to hand back a few, cost a repeated query more than recalling it.
pub fn search(engine: &mut Engine, query: &Query) -> Result<Page, Refusal> {
    let found = find(engine, query, MAX_LIMIT)?;
    let span = found.letters.chars().count();
    let hits = found.hits.iter().map(|&(start, skip)| place(start, skip, span)).collect();
    Ok(Page { letters: found.letters, total: found.total, listed: found.listed, offset: query.offset, hits })
}
