//! Gematria search: every run of consecutive words of one verse whose value,
//! by a method, is the value of the text asked for.
//!
//! **One pass per verse, two pointers.** Every word has at least one letter
//! and every letter a value above zero, so a run's value grows when a word is
//! added at its end and shrinks when one is taken from its start — for every
//! method but Mispar Katan Mispari, whose digital root does neither. For each
//! first word there is therefore at most one last word that hits the target,
//! and the last word only ever moves forward: the verse's words are walked
//! twice at most, whatever the target. Bone'eh and Ha'achor weigh a letter by
//! its place in the run, not in the verse; prefix sums over the verse give
//! any run's value in constant time all the same.
//!
//! - **A hit stays in its verse** — the owner's rule: a phrase read as one.
//! - **Mispar Katan Mispari matches single words** — the owner's rule: a
//!   digital root (1 to 9) would make about one run in nine a hit.
//! - **HaMerubah HaKlali is the standard value squared**: its runs are those
//!   of Mispar haPanim at the target's square root, none when it has none.
//!
//! The Tanakh is read once into `Stream` — each letter's form, where each
//! word and each verse starts — at the first search, and kept.

use std::ops::Range;
use std::sync::OnceLock;

use super::{Class, FORMS, Method, SHEMI, STANDARD, class};
use crate::corpus;

/// Words of a hit record: its verse, then the rank of its first letter
/// among the verse's consonants, and the rank past its last — a contract
/// with the page, which marks every letter between.
pub const STRIDE: usize = 3;

/// What a search costs, in V8 nanoseconds per letter of the stretch read.
/// `bench_search` times a few natively; V8 is given twice that.
const NS_PER_LETTER: f64 = 8.0;

/// What a search over `letters` letters is expected to cost, in V8
/// nanoseconds — read before searching, so a server can refuse one too
/// heavy and charge it to its caller.
pub fn cost(letters: usize) -> f64 {
    letters as f64 * NS_PER_LETTER
}

/// The Tanakh as gematria reads it.
pub struct Stream {
    /// Every letter's form, in reading order.
    forms: Vec<u8>,
    /// The first letter of each word, then the number of letters.
    words: Vec<u32>,
    /// The first word of each verse, then the number of words.
    verse_words: Vec<u32>,
    /// The first letter of each verse, then the number of letters.
    verse_letters: Vec<u32>,
}

static STREAM: OnceLock<Stream> = OnceLock::new();

/// The stream, read at the first call — `None` while the text has not
/// landed, so a half-landed text is never kept.
pub fn stream() -> Option<&'static Stream> {
    if let Some(stream) = STREAM.get() {
        return Some(stream);
    }
    corpus::has_text().then(|| STREAM.get_or_init(Stream::read))
}

impl Stream {
    fn read() -> Stream {
        let verses = corpus::verse_count();
        let mut stream = Stream {
            forms: Vec::new(),
            words: Vec::new(),
            verse_words: Vec::with_capacity(verses + 1),
            verse_letters: Vec::with_capacity(verses + 1),
        };
        for verse in 0..verses {
            stream.verse_words.push(stream.words.len() as u32);
            stream.verse_letters.push(stream.forms.len() as u32);
            let mut in_word = false;
            for c in corpus::verse_text(verse).chars() {
                match class(c) {
                    Class::Letter(form) => {
                        if !in_word {
                            stream.words.push(stream.forms.len() as u32);
                            in_word = true;
                        }
                        stream.forms.push(form);
                    }
                    Class::Mark => {}
                    Class::Gap => in_word = false,
                }
            }
        }
        stream.verse_words.push(stream.words.len() as u32);
        stream.verse_letters.push(stream.forms.len() as u32);
        stream.words.push(stream.forms.len() as u32);
        stream
    }

    /// Letters of the verses `verses`: what a search over them reads.
    pub fn letters_in(&self, verses: Range<usize>) -> usize {
        let end = verses.end.min(self.verse_letters.len() - 1);
        let first = verses.start.min(end);
        (self.verse_letters[end] - self.verse_letters[first]) as usize
    }

    /// Words of a verse, by their global indices.
    fn words_of(&self, verse: usize) -> Range<usize> {
        self.verse_words[verse] as usize..self.verse_words[verse + 1] as usize
    }

    /// Every run of whole words of one verse among `verses` worth `target`
    /// by `method`, in the order of the text, written into `out` as
    /// `STRIDE` words each — the first `cap`. Returns the exact number.
    pub fn search(&self, method: Method, target: u64, verses: Range<usize>, cap: usize, out: &mut Vec<u32>) -> u64 {
        out.clear();
        let verses = verses.start.min(self.verse_count())..verses.end.min(self.verse_count());
        let (rule, target) = match Rule::of(method, target) {
            Some(found) => found,
            None => return 0,
        };
        let (mut sums, mut weighted) = (Vec::new(), Vec::new());
        let mut total = 0;
        for verse in verses {
            let first = self.verse_letters[verse] as usize;
            let letters = &self.forms[first..self.verse_letters[verse + 1] as usize];
            rule.prefix(letters, &mut sums, &mut weighted);
            let words = self.words_of(verse);
            // A word's first letter, as a rank in the verse; the end of the
            // last word is the verse's end.
            let at = |w: usize| self.words[w] as usize - first;
            let mut hit = |a: usize, b: usize| {
                if total < cap as u64 {
                    out.extend_from_slice(&[verse as u32, a as u32, b as u32]);
                }
                total += 1;
            };
            if let Rule::Root = rule {
                for w in words {
                    let (a, b) = (at(w), at(w + 1));
                    if root(sums[b] - sums[a]) == target {
                        hit(a, b);
                    }
                }
                continue;
            }
            let value = |start: usize, end: usize| rule.value(&sums, &weighted, at(start), at(end), end - start);
            let mut end = words.start;
            for start in words.clone() {
                end = end.max(start + 1);
                while end < words.end && value(start, end) < target {
                    end += 1;
                }
                if value(start, end) == target {
                    hit(at(start), at(end));
                }
            }
        }
        total
    }

    fn verse_count(&self) -> usize {
        self.verse_words.len() - 1
    }
}

/// How a method values a run of letters `a..b` of a verse. One is built per
/// search and read in place: its table's size costs nothing.
#[allow(clippy::large_enum_variant)]
enum Rule {
    /// A sum over its letters, each worth its table's value, plus `per_word`
    /// for each word — Mispar Kolel's one.
    Sum { table: [u64; FORMS], per_word: bool },
    /// Each letter's standard value times its place in the run, from 1.
    Haachor,
    /// Each letter's standard value times the letters from it to the run's
    /// end: the running totals summed.
    Boneeh,
    /// The digital root of one word's standard value.
    Root,
}

impl Rule {
    /// The rule for a method, and the target it reads — `None` when no run
    /// can reach it (a square's target that is no square).
    fn of(method: Method, target: u64) -> Option<(Rule, u64)> {
        let standard = STANDARD.map(u64::from);
        let sum = |table: [u64; FORMS]| Rule::Sum { table, per_word: false };
        Some(match method {
            Method::MisparMusafi => (sum(standard.map(|v| v + 1)), target),
            Method::MisparNeelam => (sum(std::array::from_fn(|f| u64::from(SHEMI[f] - STANDARD[f]))), target),
            Method::MisparKolel => (Rule::Sum { table: standard, per_word: true }, target),
            Method::MisparHamerubahHaklali => {
                let side = target.isqrt();
                if side * side != target {
                    return None;
                }
                (sum(standard), side)
            }
            Method::MisparHaachor => (Rule::Haachor, target),
            Method::MisparBoneeh => (Rule::Boneeh, target),
            Method::MisparKatanMispari => (Rule::Root, target),
            m => (sum(m.table().map_or(standard, |t| t.map(u64::from))), target),
        })
    }

    /// The verse's prefix sums: `sums[k]` the values of its first `k`
    /// letters; `weighted[k]` each of those times its rank.
    fn prefix(&self, letters: &[u8], sums: &mut Vec<u64>, weighted: &mut Vec<u64>) {
        let table = match self {
            Rule::Sum { table, .. } => table,
            _ => &STANDARD.map(u64::from),
        };
        sums.clear();
        weighted.clear();
        let (mut s, mut w) = (0, 0);
        sums.push(0);
        weighted.push(0);
        for (k, &form) in letters.iter().enumerate() {
            let v = table[form as usize];
            s += v;
            w += v * k as u64;
            sums.push(s);
            weighted.push(w);
        }
    }

    /// The value of letters `a..b`, `words` whole words.
    fn value(&self, sums: &[u64], weighted: &[u64], a: usize, b: usize, words: usize) -> u64 {
        let (s, w) = (sums[b] - sums[a], weighted[b] - weighted[a]);
        match self {
            Rule::Sum { per_word: false, .. } => s,
            Rule::Sum { per_word: true, .. } => s + words as u64,
            // Σ v (k − a + 1) = Σ v·k − a·Σ v + Σ v; every k ≥ a.
            Rule::Haachor => w - a as u64 * s + s,
            // Σ v (b − k); every k < b.
            Rule::Boneeh => b as u64 * s - w,
            Rule::Root => root(s),
        }
    }
}

/// The digits of a value summed until one is left; 0 for none.
fn root(v: u64) -> u64 {
    if v == 0 { 0 } else { 1 + (v - 1) % 9 }
}

