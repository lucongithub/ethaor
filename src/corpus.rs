//! Corpus access, with no copying and no allocation once landed.
//!
//! The corpus travels **packed** (`pack`): only what cannot be recomputed. It
//! lands in pieces. The **map** — names, labels, counts, a sliver — is
//! embedded in the module and unpacked at first access: the overview needs
//! nothing else. The **verses** arrive later, chapter by chapter — a tile
//! fetched for the chapters the view is about to read, books in the
//! background, nearest to the view first (`land_chapters`) — each straight
//! into its final place in a buffer reserved at the corpus' full size.
//! Nothing moves, so every slice handed out stays valid, and the order of
//! arrival does not matter.
//!
//! Until a chapter lands its verses are the empty string, and until the whole
//! text has landed no verse carries another reading: never a panic, only
//! blank. Every function below is an `O(1)` lookup.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::pack;

static MAP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/map.pack"));

/// Away from the browser — the test suite, a native build — the text part
/// is embedded too and lands at first use: everything behaves as once the
/// page has fed it.
#[cfg(not(target_arch = "wasm32"))]
static TEXT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.plain"));

/// The corpus as it lands: the map unpacked, and the text buffer filling in.
///
/// A value of its own rather than bare statics, so the tests can land a
/// corpus piece by piece without touching the module's.
pub struct Landing {
    map: pack::Map,
    /// The working text: the map's strings, then zeros where the verses and
    /// readings will land — zeros are valid UTF-8, so the whole buffer is a
    /// `str` at any time. Never reallocated.
    buf: *mut u8,
    len: usize,
    /// Per verse, its start and end in the buffer, packed in one word; zero
    /// until its chapter lands.
    bounds: Box<[AtomicU64]>,
    /// Per chapter, whether its verses are in.
    ready: Box<[AtomicBool]>,
    /// The readings' table, once the whole text is in.
    readings: OnceLock<Box<[u32]>>,
}

// SAFETY: the buffer is written only where nothing has been handed out yet —
// a chapter not landed, the readings' stretch before they are laid — and
// each place is written once, guarded by `ready` and `readings`; the module
// runs on a single thread, and the tests land a corpus of their own.
unsafe impl Sync for Landing {}
unsafe impl Send for Landing {}

impl Landing {
    pub fn new(map: &[u8]) -> Landing {
        let map = pack::unpack_map(map);
        let mut text = map.text.clone();
        text.resize(map.total, 0);
        let mut text = std::mem::ManuallyDrop::new(text.into_boxed_slice());
        let nv = map.index[2] as usize;
        let nc = map.index[1] as usize;
        Landing {
            buf: text.as_mut_ptr(),
            len: text.len(),
            bounds: (0..nv).map(|_| AtomicU64::new(0)).collect(),
            ready: (0..nc).map(|_| AtomicBool::new(false)).collect(),
            readings: OnceLock::new(),
            map,
        }
    }

    /// The whole working text: names, then verses as they land, zeros where
    /// they have not.
    pub fn text(&self) -> &str {
        // SAFETY: `len` bytes from `buf` are initialised — the map's strings
        // and zeros, then the verses written into the zeros — and UTF-8 at all
        // times: zeros are, and every write lays whole characters.
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.buf, self.len)) }
    }

    /// Writable view of `range` of the buffer.
    ///
    /// # Safety
    /// Nothing handed out overlaps `range`, and nothing else writes it.
    #[allow(clippy::mut_from_ref)]
    unsafe fn place(&self, range: core::ops::Range<usize>) -> &mut [u8] {
        assert!(range.end <= self.len);
        unsafe { std::slice::from_raw_parts_mut(self.buf.add(range.start), range.len()) }
    }

    pub fn chapter_ready(&self, chapter: usize) -> bool {
        self.ready[chapter].load(Ordering::Acquire)
    }

    /// Where a run of chapters lies in the opened text part, in bytes: what a
    /// tile request asks for.
    pub fn tile_range(&self, chapters: core::ops::Range<usize>) -> (usize, usize) {
        let first = self.map.chapters[chapters.start];
        let last = self.map.chapters[chapters.end - 1];
        let base = self.map.stream_base;
        (base + first.stream.0, base + last.stream.0 + last.stream.1)
    }

    /// Lands a run of chapters from their codes — their stretch of the verse
    /// stream, a tile. Chapters already in are left as they are.
    pub fn land_chapters(&self, chapters: core::ops::Range<usize>, codes: &[u8]) {
        let origin = self.map.chapters[chapters.start].stream.0;
        for c in chapters {
            let span = self.map.chapters[c];
            let from = span.stream.0 - origin;
            self.land(c, &codes[from..from + span.stream.1]);
        }
    }

    fn land(&self, chapter: usize, codes: &[u8]) {
        if self.chapter_ready(chapter) {
            return;
        }
        let (at, len) = self.map.chapters[chapter].text;
        let verses = self.map.verses_of(chapter);
        let mut bounds = vec![(0u32, 0u32); verses.len()];
        // SAFETY: the chapter is not landed, so no slice of its place has
        // been handed out, and it is landed once.
        let out = unsafe { self.place(at..at + len) };
        pack::lay_chapter(&self.map, chapter, codes, out, &mut bounds);
        for (v, (start, end)) in verses.zip(bounds) {
            self.bounds[v].store((start as u64) << 32 | end as u64, Ordering::Release);
        }
        self.ready[chapter].store(true, Ordering::Release);
    }

    /// Lands the whole text part: every chapter not in yet, then the
    /// readings.
    pub fn land_text(&self, plain: &[u8]) {
        // Called at every access away from the browser: once in, at once out.
        if self.readings.get().is_some() {
            return;
        }
        let (stream, rest) = pack::split_text(&self.map, plain);
        let n = self.map.chapters.len();
        self.land_chapters(0..n, stream);
        self.land_readings(rest);
    }

    /// Lands the other edition's readings — what follows the verse stream in
    /// the text part — once every chapter is in; before, nothing. Returns
    /// whether the whole text is now in.
    pub fn land_readings(&self, rest: &[u8]) -> bool {
        if self.readings.get().is_some() {
            return true;
        }
        if !(0..self.ready.len()).all(|c| self.chapter_ready(c)) {
            return false;
        }
        let bounds: Vec<(u32, u32)> = (0..self.bounds.len()).map(|v| self.bound(v)).collect();
        let end = self.map.verses_end();
        // SAFETY: the verses are read and the readings' stretch, past them,
        // is written — disjoint; nothing has been handed out of the latter.
        let verses = unsafe { std::slice::from_raw_parts(self.buf, end) };
        let out = unsafe { self.place(end..self.len) };
        let table = pack::lay_readings(&self.map, rest, verses, out, &bounds);
        let _ = self.readings.set(table.into_boxed_slice());
        true
    }

    fn bound(&self, verse: usize) -> (u32, u32) {
        let word = self.bounds[verse].load(Ordering::Acquire);
        ((word >> 32) as u32, word as u32)
    }

    pub fn verse_text(&self, verse: usize) -> &str {
        let (start, end) = self.bound(verse);
        &self.text()[start as usize..end as usize]
    }

    pub fn has_text(&self) -> bool {
        self.readings.get().is_some()
    }
}

static LANDING: OnceLock<Landing> = OnceLock::new();

fn landing() -> &'static Landing {
    let landing = LANDING.get_or_init(|| Landing::new(MAP));
    #[cfg(not(target_arch = "wasm32"))]
    landing.land_text(TEXT);
    landing
}

/// Lands a run of chapters from their tile — their stretch of the opened
/// text part's verse stream.
pub fn land_chapters(chapters: core::ops::Range<usize>, codes: &[u8]) {
    landing().land_chapters(chapters, codes);
}

/// Lands the other edition's readings once every chapter is in. Returns
/// whether the whole text is now in.
pub fn land_readings(rest: &[u8]) -> bool {
    landing().land_readings(rest)
}

/// Where a run of chapters lies in the opened text part, in bytes.
pub fn tile_range(chapters: core::ops::Range<usize>) -> (usize, usize) {
    landing().tile_range(chapters)
}

/// Whether a chapter's verses are there to read.
pub fn chapter_ready(chapter: usize) -> bool {
    landing().chapter_ready(chapter)
}

/// Whether the whole text — every verse, and the readings — has landed.
pub fn has_text() -> bool {
    landing().has_text()
}

/// Offset in `text` where the verses start, landed or not: the map's
/// strings lie before.
pub fn verses_offset() -> usize {
    landing().map.text.len()
}

fn index() -> &'static [u32] {
    &landing().map.index
}

/// The other edition's readings, as `variant` reads them: empty until the
/// whole text lands.
pub fn variant_table() -> &'static [u32] {
    landing().readings.get().map_or(&[0, 0], |t| t)
}

/// Split of the index into its series of offsets. See `write_output` in
/// `build.rs` for the layout; the verses' own offsets live in `Landing`.
#[allow(dead_code)]
struct Layout {
    section_name: &'static [u32],
    book_name: &'static [u32],
    book_chap: &'static [u32],
    chap_label: &'static [u32],
    chap_verse: &'static [u32],
    verse_label: &'static [u32],
}

fn layout() -> Layout {
    let idx = index();
    let (nb, nc, nv) = (idx[0] as usize, idx[1] as usize, idx[2] as usize);
    let rest = &idx[3..];

    let (section_name, rest) = rest.split_at(4);
    let (book_name, rest) = rest.split_at(nb + 1);
    let (book_chap, rest) = rest.split_at(nb + 1);
    let (chap_label, rest) = rest.split_at(nc + 1);
    let (chap_verse, rest) = rest.split_at(nc + 1);
    let (verse_label, _) = rest.split_at(nv + 1);

    Layout { section_name, book_name, book_chap, chap_label, chap_verse, verse_label }
}

#[inline]
fn slice(offsets: &[u32], i: usize) -> &'static str {
    &text()[offsets[i] as usize..offsets[i + 1] as usize]
}

/// The whole corpus, in UTF-8. The page reads the fragments to draw straight
/// from it, with no copying: only an offset and a length cross the boundary.
pub fn text() -> &'static str {
    landing().text()
}

/// Offset of a fragment inside `text()`. Every rendered fragment is a slice of
/// that string, so the pointer difference is enough.
pub fn offset_of(fragment: &str) -> usize {
    fragment.as_ptr() as usize - text().as_ptr() as usize
}

/// Number of books: 39 — the 35 titles of the Mechon-Mamre source, four of
/// which `build.rs` splits in two.
pub fn book_count() -> usize {
    index()[0] as usize
}

/// Total number of chapters, across all books.
pub fn chapter_count() -> usize {
    index()[1] as usize
}

/// Total number of verses.
#[allow(dead_code)]
pub fn verse_count() -> usize {
    index()[2] as usize
}

pub fn section_name(section: usize) -> &'static str {
    slice(layout().section_name, section)
}

pub fn book_name(book: usize) -> &'static str {
    slice(layout().book_name, book)
}

/// Global indices of a book's chapters.
pub fn chapters_of(book: usize) -> core::ops::Range<usize> {
    let l = layout();
    l.book_chap[book] as usize..l.book_chap[book + 1] as usize
}

pub fn chapter_label(chapter: usize) -> &'static str {
    slice(layout().chap_label, chapter)
}

/// Global indices of a chapter's verses.
pub fn verses_of(chapter: usize) -> core::ops::Range<usize> {
    let l = layout();
    l.chap_verse[chapter] as usize..l.chap_verse[chapter + 1] as usize
}

/// Bytes a chapter's verses take in the working text, landed or not: what a
/// page reserves for a chapter before its verses come.
pub fn chapter_bytes(chapter: usize) -> usize {
    landing().map.chapters[chapter].text.1
}

#[allow(dead_code)]
pub fn verse_label(verse: usize) -> &'static str {
    slice(layout().verse_label, verse)
}

/// A verse's text: empty until its chapter lands.
pub fn verse_text(verse: usize) -> &'static str {
    landing().verse_text(verse)
}

#[cfg(test)]
mod tests;
