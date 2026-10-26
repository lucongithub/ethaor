//! The searches' marks: where each search the page keeps found its word,
//! as the frame paints it — the letters themselves in the search's ink at
//! the reading level, a dot beside a chapter's number, a book's or a
//! portion's name above it.
//!
//! Which colour a search wears is the page's business: the module only says
//! which slot a run or a dot belongs to.

use super::*;
use crate::els;

/// Searches marked at once — every kind of search together. The page's
/// limit is this one.
pub(crate) const MARK_SLOTS: usize = 5;

/// One search's marks, placed once when it is set: nothing is searched or
/// walked again at drawing time.
#[derive(Clone, Default)]
pub(crate) struct Marks {
    /// Hidden, its letters are drawn as ours and its dots are gone; its
    /// places are kept for when it is shown again.
    pub(crate) shown: bool,
    /// `(verse, from, to)`: byte ranges of `drawn(verse)` holding a letter
    /// found — a consonant and its marks, contiguous ones merged — sorted.
    pub(crate) runs: Vec<(u32, u32, u32)>,
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
        // Letters next to each other — a word's — make one run.
        let mut merged: Vec<(u32, u32, u32)> = Vec::with_capacity(runs.len());
        for run in runs {
            match merged.last_mut() {
                Some(last) if last.0 == run.0 && run.1 <= last.2 => last.2 = last.2.max(run.2),
                _ => merged.push(run),
            }
        }

        let mut marks = Marks {
            shown: true,
            runs: merged,
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
    pub(crate) fn chapter_marks(&self, chapter: usize) -> u8 {
        self.mask(|m| m.chapters.get(chapter).copied().unwrap_or(false))
    }

    pub(crate) fn book_marks(&self, book: usize) -> u8 {
        self.mask(|m| m.books.get(book).copied().unwrap_or(false))
    }

    pub(crate) fn section_marks(&self, section: usize) -> u8 {
        self.mask(|m| m.sections[section])
    }

    fn mask(&self, holds: impl Fn(&Marks) -> bool) -> u8 {
        let mut bits = 0;
        for (slot, marks) in self.marks.iter().enumerate() {
            if marks.shown && holds(marks) {
                bits |= 1 << slot;
            }
        }
        bits
    }

    /// The runs of a verse marked by a shown slot, with that slot: pushed
    /// into `out`, lowest slot first, each slot's in the order of the text.
    pub(crate) fn verse_marks(&self, verse: usize, out: &mut Vec<(usize, usize, f32)>) {
        for (slot, marks) in self.marks.iter().enumerate() {
            if !marks.shown {
                continue;
            }
            let from = marks.runs.partition_point(|r| (r.0 as usize) < verse);
            for &(_, a, b) in marks.runs[from..].iter().take_while(|r| r.0 as usize == verse) {
                out.push((a as usize, b as usize, INK_MARK + slot as f32));
            }
        }
    }
}

impl App {
    /// The letters the shown searches found in a verse, as the passage reads
    /// it — our text, the other edition's forms not in it: `(from, to,
    /// slot)` in bytes of `corpus::verse_text(verse)`, lowest slot first,
    /// each slot's in the order of the text.
    pub(crate) fn passage_marks(&self, verse: usize, out: &mut Vec<u32>) {
        out.clear();
        let mut runs = Vec::new();
        self.verse_marks(verse, &mut runs);
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
    /// forms in them: `(from, to, slot)` as offsets in the corpus text, verse
    /// after verse. Nothing for a chapter not landed.
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
    pub(crate) fn place_marks(&self, level: u32, index: usize) -> u8 {
        match level {
            0 if index < 3 => self.section_marks(index),
            1 => self.book_marks(index),
            2 => self.chapter_marks(index),
            _ => 0,
        }
    }
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
