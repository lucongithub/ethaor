//! ELS — equidistant letter sequences: a word read across the whole Tanakh
//! with the same number of letters skipped between each of its letters,
//! contiguous (skip 0) or not, over one skip or a whole range of them.
//!
//! The search does not read the text: it reads the letter stream it derives
//! from it at first use — every consonant of the Tanakh in reading order,
//! **one byte each**, final forms folded, with no space, mark or verse break. That is
//! what "without regard to passages" means: an occurrence may start in one
//! verse and end in the next, or in the next book.
//!
//! A letter's **global rank** in that stream is the one coordinate the search
//! speaks: a hit is the rank of its first letter and its skip. Everything else
//! — book, chapter, verse, rank inside the verse — derives from the rank
//! through the verse starts recorded with the stream.
//!
//! How hits are found is `method`'s business, over indices `index` derives
//! from the stream in memory; this module folds the query, bounds the output
//! and places each hit in the text.

mod index;
mod method;

pub use index::Index;
pub use method::{METHODS, Method};

use std::sync::OnceLock;

use crate::corpus;
use crate::letter::{self, ALPHABET};
use method::{Query, count_pairs, run};

/// Words of a hit record, a contract with the page: the rank of the first
/// letter in the stream, the skip, then book, chapter and verse of the first
/// letter (global indices), the rank of that letter inside its verse, and the
/// verse of the last letter — first and last verse bound the passage.
pub const STRIDE: usize = 7;

/// Records kept per query. Past it, the total is still exact, and the records
/// are the first ones in (skip, start) order: a pair of common letters over a
/// wide range has tens of millions of hits, which no page could show.
pub const CAP: usize = 100_000;

/// Bits of a key given to the start; the stream must stay below
/// `1 << START_BITS`.
const START_BITS: u32 = 21;

/// The stream and where each verse starts in it, with a closing sentinel.
struct Letters {
    stream: Vec<u8>,
    starts: Vec<u32>,
}

static LETTERS: OnceLock<Letters> = OnceLock::new();

/// Read off the corpus at first use rather than shipped: it is entirely
/// given by the text, and shipping it cost more than the text itself once
/// both were compressed.
fn letters() -> &'static Letters {
    LETTERS.get_or_init(|| {
        // Read before the text lands, the stream would be empty for good.
        assert!(corpus::has_text(), "lettres demandees avant le texte");
        let n = corpus::verse_count();
        let mut stream = Vec::with_capacity(corpus::text().len() / 3);
        let mut starts = Vec::with_capacity(n + 1);
        for v in 0..n {
            starts.push(stream.len() as u32);
            stream.extend(corpus::verse_text(v).chars().filter_map(letter::code));
        }
        starts.push(stream.len() as u32);
        Letters { stream, starts }
    })
}

/// Every consonant of the Tanakh, as its rank in the alphabet.
pub fn stream() -> &'static [u8] {
    &letters().stream
}

/// Rank of each verse's first letter in the stream, with a closing sentinel.
pub fn verse_starts() -> &'static [u32] {
    &letters().starts
}

/// Folds a query into the stream's code: consonants only, finals folded. A
/// pointed query and a bare one are the same search.
pub fn pattern(query: &str, out: &mut Vec<u8>) {
    out.clear();
    out.extend(query.chars().filter_map(letter::code));
}

/// Finds `pattern` with any skip in `skip0..=skip1` and writes the hits as
/// (start, skip) into `out`, sorted by skip then start, at most `cap` of them.
/// Returns the exact number of hits. A single letter has no skip: each of its
/// occurrences counts once, under `skip0`.
///
/// `force` imposes a method; `None` lets the cost model choose.
pub fn hits(
    index: &mut Index,
    pattern: &[u8],
    skip0: u32,
    skip1: u32,
    cap: usize,
    force: Option<Method>,
    out: &mut Vec<(u32, u32)>,
) -> u64 {
    out.clear();
    let (n, m) = (index.text.len(), pattern.len());
    assert!(n < 1 << START_BITS, "le flux deborde la cle des occurrences");
    if m == 0 || n == 0 || skip0 > skip1 || pattern.iter().any(|&c| c as usize >= ALPHABET) {
        return 0;
    }
    if m == 1 {
        let positions = index.positions(pattern[0]);
        out.extend(positions.iter().take(cap).map(|&p| (p, skip0)));
        return positions.len() as u64;
    }
    // Steps in u64: `skip + 1` overflows a u32, and a usize on wasm32.
    let s0 = skip0 as u64 + 1;
    let s1 = (skip1 as u64 + 1).min(((n - 1) / (m - 1)) as u64);
    if s0 > s1 {
        return 0;
    }
    let (s0, s1) = (s0 as usize, s1 as usize);

    // Two letters can be counted without listing. When they are expected to
    // pass the cap, only the steps that hold the first `cap` hits are listed:
    // the narrowest range from `s0` that does, found by doubling — each count
    // is one pass over two position lists. Below the cap, listing is the
    // count, and the extra pass would be wasted.
    let mut s1 = s1;
    let mut counted = None;
    let f = features(index, pattern, skip0, skip1);
    if m == 2 && f.steps * f.second > cap as f64 / 2.0 {
        let total = count_pairs(index, &Query { pattern, s0, s1 });
        if total > cap as u64 {
            // A step holds about `anchors × second` hits: start there.
            let mut width = ((cap as f64 / (f.anchors * f.second).max(1.0)) as usize).max(1);
            while s0 + width - 1 < s1 && count_pairs(index, &Query { pattern, s0, s1: s0 + width - 1 }) < cap as u64 {
                width *= 2;
            }
            s1 = s1.min(s0 + width - 1);
        }
        counted = Some(total);
    }

    let method = force.unwrap_or_else(|| choose(&features(index, pattern, skip0, (s1 - 1) as u32)));
    let mut keep = Keep::new(cap, s0);
    let found = run(index, &Query { pattern, s0, s1 }, method, &mut keep);
    let total = counted.unwrap_or(found);
    radix_sort(&mut keep.keys);
    keep.keys.truncate(cap);
    let mask = (1u64 << START_BITS) - 1;
    out.extend(keep.keys.iter().map(|&k| ((k & mask) as u32, (s0 - 1) as u32 + (k >> START_BITS) as u32)));
    total
}

/// Sorts keys by their low bits first, eleven at a time, and only as many
/// passes as the largest key needs: two for a single skip, four at most. A
/// comparison sort of the hundred thousand keys a flood keeps cost more than
/// the search that found them.
pub fn radix_sort(keys: &mut Vec<u64>) {
    const DIGIT: u32 = 11;
    let bits = 64 - keys.iter().max().map_or(64, |k| k.leading_zeros());
    let mut other = vec![0u64; keys.len()];
    let mut shift = 0;
    while shift < bits {
        let mut at = vec![0usize; (1 << DIGIT) + 1];
        for &k in keys.iter() {
            at[((k >> shift) & ((1 << DIGIT) - 1)) as usize + 1] += 1;
        }
        for d in 0..1 << DIGIT {
            at[d + 1] += at[d];
        }
        for &k in keys.iter() {
            let d = ((k >> shift) & ((1 << DIGIT) - 1)) as usize;
            other[at[d]] = k;
            at[d] += 1;
        }
        core::mem::swap(keys, &mut other);
        shift += DIGIT;
    }
}

/// Keeps the `cap` first occurrences in (skip, start) order out of any number,
/// in one pass and bounded memory.
///
/// An occurrence is a key — step above, start below — so the order is the
/// keys'. Keys gather up to twice the cap; then the cap smallest are kept and
/// the largest of them becomes a threshold no later key may reach. The
/// threshold only goes down, and it tells the methods the step past which
/// they may merely count.
struct Keep {
    keys: Vec<u64>,
    cap: usize,
    s0: usize,
    threshold: u64,
}

impl Keep {
    fn new(cap: usize, s0: usize) -> Keep {
        Keep { keys: Vec::new(), cap, s0, threshold: if cap == 0 { 0 } else { u64::MAX } }
    }
}

impl method::Sink for Keep {
    #[inline]
    fn hit(&mut self, start: usize, step: usize) {
        let key = ((step - self.s0) as u64) << START_BITS | start as u64;
        if key < self.threshold {
            self.keys.push(key);
            if self.keys.len() >= self.cap.saturating_mul(2) {
                // The cap-th smallest lands at `cap - 1`, everything smaller
                // before it: it is the largest key kept.
                self.keys.select_nth_unstable(self.cap - 1);
                self.keys.truncate(self.cap);
                self.threshold = self.keys[self.cap - 1];
            }
        }
    }

    #[inline]
    fn limit(&self) -> usize {
        if self.threshold == u64::MAX {
            usize::MAX
        } else {
            self.s0 + (self.threshold >> START_BITS) as usize
        }
    }
}

/// What the cost model reads of a query.
pub struct Features {
    /// Occurrences of the rarest letter: the rows every method walks.
    pub anchors: f64,
    /// Steps tried over all anchors, once each is cut to the stream.
    pub steps: f64,
    /// Rarity of the second rarest letter, as a share of the stream.
    pub second: f64,
    /// Letters besides the anchor.
    pub lanes: f64,
}

/// Anchors whose reach is measured to estimate `steps`: the reach varies
/// smoothly along the stream, so a sample spread over it is enough.
const SAMPLE: usize = 64;

pub fn features(index: &Index, pattern: &[u8], skip0: u32, skip1: u32) -> Features {
    let n = index.text.len();
    let m = pattern.len();
    let mut freqs: Vec<u32> = pattern.iter().map(|&c| index.freq[c as usize]).collect();
    freqs.sort_unstable();
    let anchor = (0..m).min_by_key(|&k| index.freq[pattern[k] as usize]).unwrap_or(0);
    let s0 = skip0 as usize + 1;
    let s1 = (skip1 as usize).saturating_add(1).min((n.max(1) - 1) / (m.max(2) - 1));
    let positions = index.positions(pattern.get(anchor).copied().unwrap_or(0));
    let take = positions.len().min(SAMPLE);
    let mut reach = 0usize;
    for k in 0..take {
        let i = positions[k * positions.len() / take] as usize;
        let mut hi = s1;
        if let Some(before) = i.checked_div(anchor) {
            hi = hi.min(before);
        }
        if let Some(after) = (n - 1 - i).checked_div(m - 1 - anchor) {
            hi = hi.min(after);
        }
        reach += (hi + 1).saturating_sub(s0);
    }
    let anchors = positions.len() as f64;
    Features {
        anchors,
        steps: if take == 0 { 0.0 } else { reach as f64 / take as f64 * anchors },
        second: freqs.get(1).map_or(0.0, |&f| f as f64 / n.max(1) as f64),
        lanes: m.saturating_sub(1) as f64,
    }
}

/// The method the cost model expects to be fastest.
///
/// Each method's time is linear in what it walks: anchors for all three,
/// then steps for `Direct`, pairs of letters for `Pairs`, words of 64 steps
/// per plane — three at most — for `Planes`. The constants are nanoseconds,
/// measured in V8 on the grid of `bench_methods` (12 words, 17 ranges) and
/// tuned to minimise the time actually picked over it.
/// `the_cost_model_picks_the_winner` holds the cases that decide it.
pub fn choose(f: &Features) -> Method {
    let direct = 1.89 * f.anchors + 1.84 * f.steps;
    let pairs = 19.0 * f.anchors + 2.35 * f.steps * f.second;
    let planes = 9.9 * f.anchors + 1.97 * f.steps / 64.0 * f.lanes.min(3.0);
    if direct <= pairs && direct <= planes {
        Method::Direct
    } else if pairs <= planes {
        Method::Pairs
    } else {
        Method::Planes
    }
}

/// The search behind the page: the corpus' index, built at the first query,
/// and where each hit falls.
pub struct Engine {
    pub index: Index<'static>,
    /// Verse holding the first letter of each block of `1 << BLOCK` letters:
    /// a letter's verse is a step or two from there.
    verse_at_block: Vec<u16>,
    chapter_of_verse: Vec<u16>,
    book_of_chapter: Vec<u8>,
    pub force: Option<Method>,
    codes: Vec<u8>,
    /// The last answers, most recent first: a query asked again — a redraw,
    /// a step back — is not searched again.
    memo: Vec<Answer>,
    /// Searches actually run, memo aside.
    pub runs: u64,
}

/// A query and what it found.
struct Answer {
    codes: Vec<u8>,
    skips: (u32, u32),
    force: Option<Method>,
    total: u64,
    hits: Vec<(u32, u32)>,
}

/// Answers kept in memory: each holds at most `CAP` hits, eight bytes each.
const MEMO: usize = 8;

/// Letters per block of `verse_at_block`, as a power of two. Verses hold
/// about fifty letters, so a block starts at most a few verses before its
/// letters'.
const BLOCK: u32 = 6;

impl Engine {
    pub fn new() -> Engine {
        let starts = verse_starts();
        let n = stream().len();
        let mut verse_at_block = Vec::with_capacity((n >> BLOCK) + 1);
        let mut v = 0usize;
        for b in 0..=(n >> BLOCK) {
            while starts[v + 1] as usize <= b << BLOCK && v + 2 < starts.len() {
                v += 1;
            }
            verse_at_block.push(v as u16);
        }
        let mut chapter_of_verse = vec![0u16; corpus::verse_count()];
        for c in 0..corpus::chapter_count() {
            for v in corpus::verses_of(c) {
                chapter_of_verse[v] = c as u16;
            }
        }
        let mut book_of_chapter = vec![0u8; corpus::chapter_count()];
        for b in 0..corpus::book_count() {
            for c in corpus::chapters_of(b) {
                book_of_chapter[c] = b as u8;
            }
        }
        Engine {
            index: Index::new(stream()),
            verse_at_block,
            chapter_of_verse,
            book_of_chapter,
            force: None,
            codes: Vec::new(),
            memo: Vec::new(),
            runs: 0,
        }
    }

    /// Forgets the remembered answers, so the next query is searched.
    pub fn forget_answers(&mut self) {
        self.memo.clear();
    }

    /// The verse holding a letter of the stream.
    #[inline]
    fn verse_of(&self, letter: usize) -> usize {
        let starts = verse_starts();
        let mut v = self.verse_at_block[letter >> BLOCK] as usize;
        while starts[v + 1] as usize <= letter {
            v += 1;
        }
        v
    }

    /// Runs a search and writes one `STRIDE` record per kept hit into `out`.
    /// Returns the exact number of hits, which may exceed the records.
    pub fn search(&mut self, query: &str, skip0: u32, skip1: u32, out: &mut Vec<u32>) -> u64 {
        pattern(query, &mut self.codes);
        let asked = |a: &Answer| a.codes == self.codes && a.skips == (skip0, skip1) && a.force == self.force;
        let answer = match self.memo.iter().position(asked) {
            Some(k) => self.memo.remove(k),
            None => {
                self.runs += 1;
                let mut found = Vec::new();
                let total = hits(&mut self.index, &self.codes, skip0, skip1, CAP, self.force, &mut found);
                Answer { codes: self.codes.clone(), skips: (skip0, skip1), force: self.force, total, hits: found }
            }
        };
        self.memo.insert(0, answer);
        self.memo.truncate(MEMO);
        let answer = &self.memo[0];

        let span = self.codes.len().saturating_sub(1);
        let starts = verse_starts();
        out.clear();
        out.reserve(answer.hits.len() * STRIDE);
        for &(start, skip) in &answer.hits {
            let start = start as usize;
            let verse = self.verse_of(start);
            let chapter = self.chapter_of_verse[verse] as usize;
            out.extend_from_slice(&[
                start as u32,
                skip,
                self.book_of_chapter[chapter] as u32,
                chapter as u32,
                verse as u32,
                (start - starts[verse] as usize) as u32,
                self.verse_of(start + span * (skip as usize + 1)) as u32,
            ]);
        }
        answer.total
    }
}

#[cfg(test)]
mod tests;
