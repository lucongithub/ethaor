//! Corpus access, with no copying and no allocation.
//!
//! Text and index are produced by `build.rs` and welded into the binary. They
//! live in the read-only segment of the executable, which the system maps
//! straight from the file: nothing is read, parsed or allocated at startup.
//! Every function below is an `O(1)` lookup.

/// Wrapper guaranteeing 4-byte alignment.
///
/// `include_bytes!` promises no alignment; without this, reinterpreting the
/// bytes as `u32` would be undefined behaviour.
#[repr(C)]
struct Align4<T: ?Sized> {
    _align: [u32; 0],
    bytes: T,
}

static TEXT: &str = include_str!(concat!(env!("OUT_DIR"), "/text.bin"));

static INDEX: &Align4<[u8]> = &Align4 {
    _align: [],
    bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/index.bin")),
};

#[cfg(target_endian = "big")]
compile_error!("index.bin est ecrit en little-endian");

fn index() -> &'static [u32] {
    // SAFETY: `Align4` forces 4-byte alignment; `index.bin` is a multiple of
    // 4 bytes written little-endian by build.rs, and we checked above that the
    // target is little-endian.
    unsafe {
        core::slice::from_raw_parts(INDEX.bytes.as_ptr().cast::<u32>(), INDEX.bytes.len() / 4)
    }
}

/// Split of the index into six series of offsets. See `write_output` in
/// `build.rs` for the layout.
#[allow(dead_code)]
struct Layout {
    section_name: &'static [u32],
    book_name: &'static [u32],
    book_chap: &'static [u32],
    chap_label: &'static [u32],
    chap_verse: &'static [u32],
    verse_label: &'static [u32],
    verse_text: &'static [u32],
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
    let (verse_label, rest) = rest.split_at(nv + 1);
    let (verse_text, _) = rest.split_at(nv + 1);

    Layout { section_name, book_name, book_chap, chap_label, chap_verse, verse_label, verse_text }
}

#[inline]
fn slice(offsets: &[u32], i: usize) -> &'static str {
    &TEXT[offsets[i] as usize..offsets[i + 1] as usize]
}

/// The whole corpus, in UTF-8. The page reads the fragments to draw straight
/// from it, with no copying: only an offset and a length cross the boundary.
pub fn text() -> &'static str {
    TEXT
}

/// Offset of a fragment inside `text()`. Every rendered fragment is a slice of
/// that string, so the pointer difference is enough.
pub fn offset_of(fragment: &str) -> usize {
    fragment.as_ptr() as usize - TEXT.as_ptr() as usize
}

/// Number of books (35 for the Mechon-Mamre Tanakh, Ezra and Nehemiah apart).
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

#[allow(dead_code)]
pub fn verse_label(verse: usize) -> &'static str {
    slice(layout().verse_label, verse)
}

pub fn verse_text(verse: usize) -> &'static str {
    slice(layout().verse_text, verse)
}
