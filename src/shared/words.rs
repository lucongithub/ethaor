//! First and last letters as both transports ask them: a query checked, the
//! hits paged and each of their letters placed in the text.
//!
//! The search itself is `words::Engine`'s, lent inside the ELS engine; the
//! browser reads its records straight from memory, the HTTP API reads them
//! through `search` below.

pub use crate::words::{LONGEST_WORD, Pick};
use crate::{corpus, els};

use super::els::{Engine, Letter, MAX_LIMIT, Refusal, char_of};
use super::reference;

/// What a caller asks. Hashable: a server keys its answers by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    /// Hebrew; marks are ignored and final forms folded — unless `exact`.
    pub word: String,
    pub exact: bool,
    /// Which letter of each word is read: the first, the second… or, from
    /// the end, the last, the one before…
    pub pick: Pick,
    /// The word's letters in any order: an anagram of it, side by side.
    pub any_order: bool,
    /// Rank of the first hit returned, in the order of the text.
    pub offset: usize,
    /// Hits returned at most; 0 only counts.
    pub limit: usize,
    /// The verses the search is kept to, `[first, end)`: a hit counts only
    /// if every letter lies in them. `None` is the whole Tanakh.
    pub scope: Option<(usize, usize)>,
}

/// One occurrence of the word: each letter in the order of the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
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
    /// The word as searched: consonants only, finals folded unless exact.
    pub letters: String,
    pub total: u64,
    /// Hits that can be paged through: the first `CAP` in the order of the
    /// text. Past them the total is still exact.
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
    let mut codes = Vec::new();
    els::pattern_of(&query.word, query.exact, &mut codes);
    if codes.is_empty() {
        return Err(Refusal::NoLetter);
    }
    if !query.pick.valid() {
        return Err(Refusal::LetterRange(query.pick.nth));
    }
    if query.limit > MAX_LIMIT {
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
    Ok(codes.iter().map(|&c| char_of(c, query.exact)).collect())
}

/// The verses a scope covers: the whole Tanakh for none.
fn verses_of(scope: Option<(usize, usize)>) -> std::ops::Range<usize> {
    scope.map_or(0..corpus::verse_count(), |(first, end)| first..end)
}

/// What a query is expected to cost, in V8 milliseconds, without running
/// it. Refuses it past `limit_ms`.
pub fn admit(engine: &mut Engine, query: &Query, limit_ms: f64) -> Result<f64, Refusal> {
    let estimate_ms = engine.words.cost(query.pick, verses_of(query.scope)) / 1e6;
    if estimate_ms > limit_ms {
        return Err(Refusal::TooCostly { estimate_ms, limit_ms });
    }
    Ok(estimate_ms)
}

/// Searches, then keeps `limit` hits from `offset` and places each of their
/// letters — those of the page only.
pub fn search(engine: &mut Engine, query: &Query) -> Result<Page, Refusal> {
    let letters = check(query)?;
    let span = letters.chars().count();
    let words = &mut engine.words;
    let (total, found) =
        words.hits_in(&query.word, query.pick, query.any_order, query.exact, verses_of(query.scope));
    let (total, listed) = (total, found.len());
    let from = query.offset.min(listed);
    let to = query.offset.saturating_add(query.limit).min(listed);
    let starts = found[from..to].to_vec();
    let stream = words.stream(query.pick);
    let (codes, verse_starts) = (stream.codes(query.exact), els::verse_starts());
    let hits = starts
        .iter()
        .map(|&start| Hit {
            letters: (start as usize..start as usize + span)
                .map(|i| {
                    let (verse, position) = (stream.verse[i] as usize, stream.position[i] as u32);
                    Letter {
                        rank: verse_starts[verse] + position,
                        char: char_of(codes[i], query.exact),
                        verse,
                        position,
                    }
                })
                .collect(),
        })
        .collect();
    Ok(Page { letters, total, listed, offset: query.offset, hits })
}
