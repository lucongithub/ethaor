//! The searches' marks: where each search the page keeps found its word,
//! as the frame paints it — the letters themselves in the search's ink at
//! the reading level, a bar beside a chapter's number, a book's or a
//! portion's name above it.
//!
//! Which colour a search wears is the page's business: the module only says
//! which slots a run or a bar belongs to, as bits.

use super::*;
use crate::els;

/// Searches marked at once — every kind of search together. The page's
/// limit is this one.
pub(crate) const MARK_SLOTS: usize = 10;

/// One search's marks, placed once when it is set: nothing is searched or
/// walked again at drawing time.
#[derive(Clone, Default)]
pub(crate) struct Marks {
    /// Hidden, its letters are drawn as ours and its bars are gone; its
    /// places are kept for when it is shown again.
    pub(crate) shown: bool,
    /// `(verse, from, to)`: byte ranges of `drawn(verse)` holding a letter
    /// found — a consonant and its marks, contiguous ones merged — sorted.
    pub(crate) runs: Vec<(u32, u32, u32)>,
    /// First and last verse of each hit, in the order of the text: what its
    /// passages are made of, alone or with the other searches shown.
    pub(crate) hits: Vec<(u32, u32)>,
    /// Chapters, books and portions holding one of those letters.
    pub(crate) chapters: Vec<bool>,
    pub(crate) books: Vec<bool>,
    pub(crate) sections: [bool; 3],
}

impl App {
    /// Sets slot `slot` to the hits of an ELS search, `els::STRIDE` words
    /// each, as `els_search` leaves them — `span` letters per hit, its
    /// skip between them. Shown at once. Returns the number of runs placed;
    /// 0 for a slot out of range, or hits with nothing to place.
    ///
    /// A hit's first letter is named by its verse and its rank among the
    /// verse's consonants; the next ones are walked from it, verse after
    /// verse, so a word read across a verse break is marked on both sides.
    pub(crate) fn set_marks(&mut self, slot: usize, records: &[u32], span: usize) -> usize {
        if slot >= MARK_SLOTS {
            return 0;
        }
        let mut marks = Marks {
            shown: true,
            runs: placed(records, span),
            hits: hit_verses(records),
            chapters: vec![false; corpus::chapter_count()],
            books: vec![false; corpus::book_count()],
            sections: [false; 3],
        };
        let mut chapter = 0;
        for &(verse, _, _) in &marks.runs {
            while !corpus::verses_of(chapter).contains(&(verse as usize)) {
                chapter += 1;
            }
            marks.chapters[chapter] = true;
        }
        for book in 0..corpus::book_count() {
            marks.books[book] = corpus::chapters_of(book).any(|c| marks.chapters[c]);
        }
        for (section, &(_, first, last)) in treemap::SECTIONS.iter().enumerate() {
            marks.sections[section] = (first..last).any(|b| marks.books[b]);
        }

        let placed = marks.runs.len();
        self.marks[slot] = marks;
        placed
    }

    /// Shows or hides a slot's marks; its places are kept.
    pub(crate) fn show_marks(&mut self, slot: usize, on: bool) {
        if let Some(marks) = self.marks.get_mut(slot) {
            marks.shown = on;
        }
    }

    /// Forgets a slot's marks.
    pub(crate) fn clear_marks(&mut self, slot: usize) {
        if let Some(marks) = self.marks.get_mut(slot) {
            *marks = Marks::default();
        }
    }

    /// The slots shown that hold a letter of the chapter, as bits.
    pub(crate) fn chapter_marks(&self, chapter: usize) -> u32 {
        self.mask(|m| m.chapters.get(chapter).copied().unwrap_or(false))
    }

    pub(crate) fn book_marks(&self, book: usize) -> u32 {
        self.mask(|m| m.books.get(book).copied().unwrap_or(false))
    }

    pub(crate) fn section_marks(&self, section: usize) -> u32 {
        self.mask(|m| m.sections[section])
    }

    fn mask(&self, holds: impl Fn(&Marks) -> bool) -> u32 {
        let mut bits = 0;
        for (slot, marks) in self.marks.iter().enumerate() {
            if marks.shown && holds(marks) {
                bits |= 1 << slot;
            }
        }
        bits
    }

    /// The runs of a verse marked by the shown slots, each with the slots
    /// holding it as bits — `INK_MARK + bits`: pushed into `out` in the order
    /// of the text, never overlapping, neighbours of the same bits merged.
    ///
    /// A letter two searches found carries both: the page paints it in bands
    /// of both colours, so neither search loses a letter to the other.
    pub(crate) fn verse_marks(&self, verse: usize, out: &mut Vec<(usize, usize, f32)>) {
        self.verse_marks_with(verse, &[], out);
    }

    /// `verse_marks`, with `own` — runs placed as `Marks::runs` are — under
    /// the bit `OWN` beside the shown slots': a search's own letters, shown
    /// on the map or not.
    fn verse_marks_with(&self, verse: usize, own: &[(u32, u32, u32)], out: &mut Vec<(usize, usize, f32)>) {
        // Each slot's runs first, its bit as the ink; then cut where any of
        // them starts or ends. A verse holds few runs: no table is built.
        let base = out.len();
        let of_verse = |runs: &[(u32, u32, u32)]| {
            let from = runs.partition_point(|r| (r.0 as usize) < verse);
            let to = from + runs[from..].partition_point(|r| r.0 as usize == verse);
            from..to
        };
        for (slot, marks) in self.marks.iter().enumerate() {
            if !marks.shown {
                continue;
            }
            for &(_, a, b) in &marks.runs[of_verse(&marks.runs)] {
                out.push((a as usize, b as usize, (1u32 << slot) as f32));
            }
        }
        for &(_, a, b) in &own[of_verse(own)] {
            out.push((a as usize, b as usize, OWN as f32));
        }
        let raw = out.len();
        let Some(mut cursor) = out[base..raw].iter().map(|r| r.0).min() else {
            return;
        };
        loop {
            let (mut next, mut bits) = (usize::MAX, 0u32);
            for &(a, b, bit) in &out[base..raw] {
                if a <= cursor && cursor < b {
                    bits |= bit as u32;
                    next = next.min(b);
                } else if a > cursor {
                    next = next.min(a);
                }
            }
            if next == usize::MAX {
                break;
            }
            if bits != 0 {
                let ink = INK_MARK + bits as f32;
                let cut = out.len() > raw;
                match out.last_mut() {
                    Some(last) if cut && last.1 == cursor && last.2 == ink => last.1 = next,
                    _ => out.push((cursor, next, ink)),
                }
            }
            cursor = next;
        }
        out.drain(base..raw);
    }
}

impl App {
    /// The letters the shown searches found in a verse, as the passage reads
    /// it — our text, the other edition's forms not in it: `(from, to,
    /// slots)` in bytes of `corpus::verse_text(verse)`, the slots holding the
    /// letters as bits, in the order of the text.
    pub(crate) fn passage_marks(&self, verse: usize, out: &mut Vec<u32>) {
        out.clear();
        self.passage_marks_with(verse, &[], out);
    }

    /// `passage_marks` with a search's own runs under `OWN`, appended to `out`.
    fn passage_marks_with(&self, verse: usize, own: &[(u32, u32, u32)], out: &mut Vec<u32>) {
        let mut runs = Vec::new();
        self.verse_marks_with(verse, own, &mut runs);
        // Our letters never lie inside an insertion: an offset of the drawn
        // string is ours once the insertions before it are taken out.
        let inserts: Vec<(usize, usize)> = variant::inserts_of(verse)
            .map(|insert| {
                let (at, len) = insert.range();
                (at + len, len)
            })
            .collect();
        let ours = |d: usize| d - inserts.iter().filter(|&&(end, _)| end <= d).map(|&(_, len)| len).sum::<usize>();
        for (from, to, ink) in runs {
            out.extend_from_slice(&[ours(from) as u32, ours(to) as u32, (ink - INK_MARK) as u32]);
        }
    }
}

impl App {
    /// The letters the shown searches found in a chapter, as the book view
    /// shows its verses — the strings the map draws, the other edition's
    /// forms in them: `(from, to, slots)` as offsets in the corpus text, the
    /// slots as bits, verse after verse. Nothing for a chapter not landed.
    pub(crate) fn book_marks_of(&self, chapter: usize, out: &mut Vec<u32>) {
        out.clear();
        if chapter >= corpus::chapter_count() || !corpus::chapter_ready(chapter) {
            return;
        }
        let mut runs = Vec::new();
        for verse in corpus::verses_of(chapter) {
            runs.clear();
            self.verse_marks(verse, &mut runs);
            let base = corpus::offset_of(drawn(verse));
            for &(from, to, ink) in &runs {
                out.extend_from_slice(&[(base + from) as u32, (base + to) as u32, (ink - INK_MARK) as u32]);
            }
        }
    }

    /// The shown slots holding a place, as bits: a portion (level 0), a book
    /// (1) or a chapter (2). 0 out of range.
    pub(crate) fn place_marks(&self, level: u32, index: usize) -> u32 {
        match level {
            0 if index < 3 => self.section_marks(index),
            1 => self.book_marks(index),
            2 => self.chapter_marks(index),
            _ => 0,
        }
    }
}

/// The bit a search's own letters carry in its list of passages, past every
/// slot's: they wear its colour whether it is shown on the map or not.
pub(crate) const OWN: u32 = 1 << MARK_SLOTS;

/// Words before a verse's runs in a page of passages (`results_page`): its
/// book, chapter and global index, its number and its text as (offset,
/// length) in the corpus, and how many runs follow.
pub(crate) const RESULT_VERSE_HEAD: usize = 8;

/// A list of passages, the reader's to scroll — one search's hits, or the
/// shown searches' together: set when the list opens, read a page at a
/// time, forgotten when it closes.
#[derive(Clone, Default)]
pub(crate) struct Results {
    /// First and last verse of each passage, in the order of the text; hits
    /// sharing a verse share a passage.
    pub(crate) passages: Vec<(u32, u32)>,
    /// The search's own letters, placed as `Marks::runs` are; none for the
    /// shown searches together, whose letters their slots carry.
    pub(crate) runs: Vec<(u32, u32, u32)>,
}

impl App {
    /// Sets the list of passages to the hits written as `set_marks` takes
    /// them, `span` letters each. A hit names its first and last verse; it
    /// is shown whole, every verse between them, and hits whose verses
    /// overlap or touch one another's share one passage. Returns the number
    /// of passages.
    pub(crate) fn set_results(&mut self, records: &[u32], span: usize) -> usize {
        let passages = merged(hit_verses(records).into_iter().map(|(a, b)| (a, b, 0)));
        let passages = passages.into_iter().map(|(a, b, _)| (a, b)).collect();
        self.results = Results { passages, runs: placed(records, span) };
        self.results.passages.len()
    }

    /// The passages of the searches shown, their hits taken together as one
    /// search's are: those holding a hit of any (`all` false), or of every
    /// one (`all` true). None while no search is shown.
    pub(crate) fn shown_passages(&self, all: bool) -> Vec<(u32, u32)> {
        let mask = self.mask(|_| true);
        if mask == 0 {
            return Vec::new();
        }
        let mut hits: Vec<(u32, u32, u32)> = Vec::new();
        for (slot, marks) in self.marks.iter().enumerate().filter(|(_, m)| m.shown) {
            hits.extend(marks.hits.iter().map(|&(a, b)| (a, b, 1 << slot)));
        }
        hits.sort_unstable();
        merged(hits).into_iter().filter(|p| !all || p.2 == mask).map(|(a, b, _)| (a, b)).collect()
    }

    /// Sets the list of passages to the shown searches' (`shown_passages`).
    /// Returns the number of passages.
    pub(crate) fn set_shown_results(&mut self, all: bool) -> usize {
        self.results = Results { passages: self.shown_passages(all), runs: Vec::new() };
        self.results.passages.len()
    }

    /// Forgets the list of passages.
    pub(crate) fn clear_results(&mut self) {
        self.results = Results::default();
    }

    /// Passages `from..from + count` of the list, into `out`: per passage its
    /// number of verses, then per verse `RESULT_VERSE_HEAD` words and its
    /// runs — `(from, to, bits)` in bytes of our verse, as `passage_marks`
    /// gives them, the search's own letters under `OWN` beside the shown
    /// slots'. Returns the passages written; none past the end.
    pub(crate) fn results_page(&self, from: usize, count: usize, out: &mut Vec<u32>) -> usize {
        out.clear();
        let page = self.results.passages.iter().skip(from).take(count);
        let mut written = 0;
        for &(first, last) in page {
            out.push(last - first + 1);
            for verse in first as usize..=last as usize {
                let chapter = chapter_of(verse);
                let book = (0..corpus::book_count()).find(|&b| corpus::chapters_of(b).contains(&chapter)).unwrap_or(0);
                let (label, text) = (corpus::verse_label(verse), corpus::verse_text(verse));
                let head = out.len();
                out.extend_from_slice(&[
                    book as u32,
                    chapter as u32,
                    verse as u32,
                    corpus::offset_of(label) as u32,
                    label.len() as u32,
                    if text.is_empty() { 0 } else { corpus::offset_of(text) as u32 },
                    text.len() as u32,
                    0,
                ]);
                self.passage_marks_with(verse, &self.results.runs, out);
                out[head + RESULT_VERSE_HEAD - 1] = ((out.len() - head - RESULT_VERSE_HEAD) / 3) as u32;
            }
            written += 1;
        }
        written
    }
}

/// How many passages a search's hits make, as `set_results` would list them.
pub(crate) fn passage_count(records: &[u32]) -> usize {
    merged(hit_verses(records).into_iter().map(|(a, b)| (a, b, 0))).len()
}

/// First and last verse of each hit of `records` (`els::STRIDE` words
/// each), in the order of the text; a hit past the text is left out.
fn hit_verses(records: &[u32]) -> Vec<(u32, u32)> {
    let verses = corpus::verse_count() as u32;
    let mut hits: Vec<(u32, u32)> = records
        .chunks_exact(els::STRIDE)
        .filter(|r| r[4] < verses)
        .map(|r| (r[4], r[6].clamp(r[4], verses - 1)))
        .collect();
    hits.sort_unstable();
    hits
}

/// Hits as passages: `(first, last, bits)` sorted, those whose verses
/// overlap or touch one another's made one, their bits joined.
fn merged(hits: impl IntoIterator<Item = (u32, u32, u32)>) -> Vec<(u32, u32, u32)> {
    let mut passages: Vec<(u32, u32, u32)> = Vec::new();
    for (first, last, bits) in hits {
        match passages.last_mut() {
            Some(open) if first <= open.1 => {
                open.1 = open.1.max(last);
                open.2 |= bits;
            }
            _ => passages.push((first, last, bits)),
        }
    }
    passages
}

/// The chapter holding a verse: its global index.
fn chapter_of(verse: usize) -> usize {
    let (mut lo, mut hi) = (0, corpus::chapter_count());
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if corpus::verses_of(mid).start <= verse { lo = mid } else { hi = mid }
    }
    lo
}

/// The letters of hits as the search module leaves them, `els::STRIDE`
/// words each, `span` letters per hit, its skip between them: `(verse,
/// from, to)` in bytes of `drawn(verse)`, sorted, letters next to each
/// other — a word's — merged into one run.
///
/// A hit's first letter is named by its verse and its rank among the
/// verse's consonants; the next ones are walked from it, verse after verse,
/// so a word read across a verse break is placed on both sides.
fn placed(records: &[u32], span: usize) -> Vec<(u32, u32, u32)> {
    let verses = corpus::verse_count();
    let mut tables: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
    let mut runs: Vec<(u32, u32, u32)> = Vec::new();

    for record in records.chunks_exact(els::STRIDE) {
        let (step, mut verse, mut rank) = (record[1] as usize + 1, record[4] as usize, record[5] as usize);
        'letters: for i in 0..span {
            if i > 0 {
                rank += step;
            }
            loop {
                if verse >= verses {
                    break 'letters;
                }
                let letters = tables.entry(verse).or_insert_with(|| letter_bytes(verse));
                if rank < letters.len() {
                    let (from, to) = letters[rank];
                    runs.push((verse as u32, from, to));
                    break;
                }
                rank -= letters.len();
                verse += 1;
            }
        }
    }

    runs.sort_unstable();
    runs.dedup();
    let mut merged: Vec<(u32, u32, u32)> = Vec::with_capacity(runs.len());
    for run in runs {
        match merged.last_mut() {
            Some(last) if last.0 == run.0 && run.1 <= last.2 => last.2 = last.2.max(run.2),
            _ => merged.push(run),
        }
    }
    merged
}

/// Our consonants in the string a verse is drawn from, in reading order: the
/// bytes of each, its marks with it. Rank `k` here is rank `k` among the
/// verse's consonants in the corpus — the rank an ELS hit names — since the
/// other edition's insertions are skipped whole.
pub(crate) fn letter_bytes(verse: usize) -> Vec<(u32, u32)> {
    let shown = drawn(verse);
    let inserts: Vec<(usize, usize)> = variant::inserts_of(verse)
        .map(|insert| {
            let (at, len) = insert.range();
            (at, at + len)
        })
        .collect();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for (i, c) in shown.char_indices() {
        if inserts.iter().any(|&(a, b)| (a..b).contains(&i)) {
            continue;
        }
        let end = (i + c.len_utf8()) as u32;
        if letter::code(c).is_some() {
            out.push((i as u32, end));
        } else if c != ' '
            && let Some(last) = out.last_mut()
            && last.1 == i as u32
        {
            // A mark rides on the consonant before it.
            last.1 = end;
        }
    }
    out
}
