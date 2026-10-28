//! First and last letters: a word read across the Tanakh's words, one letter
//! taken from each — the first letter of every word, or its second, its
//! third…, or counted from its end: its last, the one before… — the letters
//! side by side as written, or in any order.
//!
//! Each choice of letter (`Pick`) is a stream of its own: one letter per word
//! long enough to hold it, in reading order. **A hit's words are adjacent in
//! the text**: a word too short to hold the letter breaks the run — the
//! stream holds a `BREAK` there, which no letter matches — so `אדם` in the
//! third letters is three words in a row, the third letter of each. It is
//! read off the text at its first search and kept, with where each letter
//! lies: its verse and its rank among the verse's consonants, the rank an
//! ELS hit names.
//!
//! A hit is the stream index of its first letter. In order, the word is
//! matched as written; in any order, a window of as many letters holding
//! the same letters as many times — an anagram, the word as written among
//! them. Both are one pass over the stream, with no index: a stream holds
//! one letter per word, a few hundred thousand, and one pass costs less than
//! building any index over it would.

use std::ops::Range;

use crate::corpus;
use crate::els::{self, CAP};
use crate::letter::{self, FORMS};

pub use crate::els::LONGEST_WORD;

/// Which letter of each word a search reads: the `nth`, from 1, counted from
/// the word's start — or from its end, `from_end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pick {
    pub nth: u32,
    pub from_end: bool,
}

impl Pick {
    pub const FIRST: Pick = Pick { nth: 1, from_end: false };
    pub const LAST: Pick = Pick { nth: 1, from_end: true };

    /// Whether a word holds that letter: `nth` from 1 to the longest word.
    pub fn valid(self) -> bool {
        (1..=LONGEST_WORD).contains(&self.nth)
    }
}

/// What a stream holds where words too short for the letter break the run:
/// one for each run of them, past every letter's code, so no pattern holds
/// it and no window across it matches.
pub const BREAK: u8 = FORMS as u8;

/// One stream: per word holding the picked letter, the letter's code — folded
/// and exact — and where it lies; `BREAK` for a run of words too short,
/// where its first lies.
pub struct Stream {
    pub folded: Vec<u8>,
    pub exact: Vec<u8>,
    /// The verse holding it, global index.
    pub verse: Vec<u16>,
    /// Its rank among that verse's consonants, from 0.
    pub position: Vec<u16>,
}

impl Stream {
    /// Reads the picked letter of every word off the text.
    pub fn of(pick: Pick) -> Stream {
        assert!(corpus::has_text(), "words asked before the text");
        let mut s = Stream { folded: Vec::new(), exact: Vec::new(), verse: Vec::new(), position: Vec::new() };
        if !pick.valid() {
            return s;
        }
        let nth = pick.nth as usize;
        for v in 0..corpus::verse_count() {
            let mut before = 0usize;
            for word in corpus::verse_text(v).split_whitespace() {
                let len = letter::letter_count(word);
                if len > 0 && len < nth && s.folded.last() != Some(&BREAK) {
                    s.folded.push(BREAK);
                    s.exact.push(BREAK);
                    s.verse.push(v as u16);
                    s.position.push(before as u16);
                }
                if nth <= len {
                    let k = if pick.from_end { len - nth } else { nth - 1 };
                    // The k-th consonant; marks between them are not letters.
                    if let Some(c) = word.chars().filter(|&c| letter::exact_code(c).is_some()).nth(k) {
                        s.folded.extend(letter::code(c));
                        s.exact.extend(letter::exact_code(c));
                        s.verse.push(v as u16);
                        s.position.push((before + k) as u16);
                    }
                }
                before += len;
            }
        }
        s
    }

    pub fn len(&self) -> usize {
        self.folded.len()
    }

    pub fn is_empty(&self) -> bool {
        self.folded.is_empty()
    }

    /// The codes a search reads: folded, or exact.
    pub fn codes(&self, exact: bool) -> &[u8] {
        if exact { &self.exact } else { &self.folded }
    }

    /// The stretch of the stream lying in the verses `first..end`.
    pub fn stretch(&self, verses: Range<usize>) -> Range<usize> {
        let at = |v: usize| self.verse.partition_point(|&x| (x as usize) < v);
        let from = at(verses.start);
        from..at(verses.end).max(from)
    }
}

/// Finds `pattern` in `stream`, its letters side by side — in their order,
/// or with `any_order` in any — and writes the index of each hit's first
/// letter into `out`, in the order of the stream, at most `cap` of them.
/// Returns the exact number of hits; overlapping ones each count.
pub fn hits(stream: &[u8], pattern: &[u8], any_order: bool, cap: usize, out: &mut Vec<u32>) -> u64 {
    out.clear();
    let (n, m) = (stream.len(), pattern.len());
    if m == 0 || m > n || pattern.iter().any(|&c| c as usize >= FORMS) {
        return 0;
    }
    let mut total = 0u64;
    let mut found = |i: usize| {
        total += 1;
        if out.len() < cap {
            out.push(i as u32);
        }
    };
    if !any_order || m == 1 {
        let (first, rest) = (pattern[0], &pattern[1..]);
        for i in 0..=n - m {
            if stream[i] == first && &stream[i + 1..i + m] == rest {
                found(i);
            }
        }
        return total;
    }
    // A window of `m` letters slides along the stream; `off` counts the
    // letters it holds a different number of times than the pattern does.
    // At zero the window is the pattern's letters, in some order.
    let mut diff = [0i32; FORMS + 1];
    for &c in pattern {
        diff[c as usize] -= 1;
    }
    let mut off = diff.iter().filter(|&&d| d != 0).count();
    let mut step = |c: u8, by: i32, off: &mut usize| {
        let d = &mut diff[c as usize];
        let was = *d;
        *d += by;
        if was == 0 {
            *off += 1;
        } else if *d == 0 {
            *off -= 1;
        }
    };
    for &c in &stream[..m] {
        step(c, 1, &mut off);
    }
    if off == 0 {
        found(0);
    }
    for i in m..n {
        step(stream[i], 1, &mut off);
        step(stream[i - m], -1, &mut off);
        if off == 0 {
            found(i + 1 - m);
        }
    }
    total
}

/// What a search costs, in V8 nanoseconds per letter of the stream read:
/// one pass, in order or in any alike. `bench_words` times two at most
/// natively; V8 is given twice that.
const NS_PER_LETTER: f64 = 4.0;

/// What a search over `letters` letters of a stream is expected to cost, in
/// V8 nanoseconds — read before searching, so a server can refuse one too
/// heavy and charge it to its caller.
pub fn cost(letters: usize) -> f64 {
    letters as f64 * NS_PER_LETTER
}

/// Words of a hit record per letter of the word: its verse, then its rank
/// among the verse's consonants — a contract with the page, which marks
/// each letter where it lies.
pub const LETTER_WORDS: usize = 2;

/// A query and what it found.
struct Answer {
    codes: Vec<u8>,
    pick: Pick,
    any_order: bool,
    exact: bool,
    verses: (usize, usize),
    total: u64,
    hits: Vec<u32>,
}

/// Answers kept in memory: each holds at most `CAP` hits, four bytes each.
const MEMO: usize = 8;

/// The searches behind the page and the API: the streams read so far, kept,
/// and the last answers.
#[derive(Default)]
pub struct Engine {
    streams: Vec<(Pick, Stream)>,
    codes: Vec<u8>,
    memo: Vec<Answer>,
    /// Searches actually run, memo aside.
    pub runs: u64,
}

impl Engine {
    /// The stream of a pick: read at its first search, then kept.
    pub fn stream(&mut self, pick: Pick) -> &Stream {
        let k = match self.streams.iter().position(|(p, _)| *p == pick) {
            Some(k) => k,
            None => {
                self.streams.push((pick, Stream::of(pick)));
                self.streams.len() - 1
            }
        };
        &self.streams[k].1
    }

    /// The hits of a query kept to the verses `verses` — only those whose
    /// every letter lies there — as stream indices, the first `CAP` in the
    /// order of the stream, and the exact total: searched, or remembered.
    pub fn hits_in(&mut self, query: &str, pick: Pick, any_order: bool, exact: bool, verses: Range<usize>) -> (u64, &[u32]) {
        els::pattern_of(query, exact, &mut self.codes);
        let key = (verses.start, verses.end);
        let asked = |a: &Answer| {
            a.codes == self.codes && a.pick == pick && a.any_order == any_order && a.exact == exact && a.verses == key
        };
        let answer = match self.memo.iter().position(asked) {
            Some(k) => self.memo.remove(k),
            None => {
                self.runs += 1;
                let codes = std::mem::take(&mut self.codes);
                let stream = self.stream(pick);
                let stretch = stream.stretch(verses);
                let mut found = Vec::new();
                let total = hits(&stream.codes(exact)[stretch.clone()], &codes, any_order, CAP, &mut found);
                for hit in &mut found {
                    *hit += stretch.start as u32;
                }
                self.codes = codes;
                Answer { codes: self.codes.clone(), pick, any_order, exact, verses: key, total, hits: found }
            }
        };
        self.memo.insert(0, answer);
        self.memo.truncate(MEMO);
        (self.memo[0].total, &self.memo[0].hits)
    }

    /// What a query is expected to cost, in V8 nanoseconds, without running
    /// it: the letters of its stretch, read once.
    pub fn cost(&mut self, pick: Pick, verses: Range<usize>) -> f64 {
        let stream = self.stream(pick);
        cost(stream.stretch(verses).len())
    }

    /// Runs a search and writes each kept hit into `out` as `LETTER_WORDS`
    /// words per letter: its verse and its rank there. Returns the exact
    /// number of hits, which may exceed the records.
    pub fn search_in(
        &mut self,
        query: &str,
        pick: Pick,
        any_order: bool,
        exact: bool,
        verses: Range<usize>,
        out: &mut Vec<u32>,
    ) -> u64 {
        self.hits_in(query, pick, any_order, exact, verses);
        let span = self.codes.len();
        let answer = &self.memo[0];
        let stream = &self.streams.iter().find(|(p, _)| *p == pick).expect("the stream searched").1;
        out.clear();
        out.reserve(answer.hits.len() * span * LETTER_WORDS);
        for &start in &answer.hits {
            for i in start as usize..start as usize + span {
                out.extend_from_slice(&[stream.verse[i] as u32, stream.position[i] as u32]);
            }
        }
        answer.total
    }
}

#[cfg(test)]
mod tests;
