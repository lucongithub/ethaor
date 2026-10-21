//! The book view's exports (`ethaor.wasm`, feature `map`): a chapter as a
//! record of offsets into the corpus, and what the page needs to fetch one
//! before the whole text is in; and the Tanakh's contents, to go to one.
//!
//! The browser's side of the reading; `server/src/api/chapter.rs` and
//! `server/src/api/contents.rs` are the HTTP side of the same. Both run
//! `shared::passage` and `shared::contents`: this file only moves values
//! across the WebAssembly boundary.

use crate::corpus;
use crate::shared::contents;
use crate::shared::passage;
use crate::shared::reference::PORTIONS;

/// The last record written, read by the page at `chapter_record_ptr`.
static mut RECORD: Vec<u32> = Vec::new();

/// Words of a record's head, before its verses.
pub const HEAD: usize = 10;

/// Chapters of the Tanakh.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn chapter_count() -> u32 {
    corpus::chapter_count() as u32
}

/// Bytes a chapter's verses take, landed or not — what the page reserves
/// for it before they come. 0 out of range.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn chapter_size(chapter: u32) -> u32 {
    let chapter = chapter as usize;
    if chapter >= corpus::chapter_count() {
        return 0;
    }
    corpus::chapter_bytes(chapter) as u32
}

/// 1 when a chapter's verses are in, 0 when not — or out of range.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn chapter_landed(chapter: u32) -> u32 {
    let chapter = chapter as usize;
    u32::from(chapter < corpus::chapter_count() && corpus::chapter_ready(chapter))
}

/// Where the tile of chapters `first..end` starts in the opened text part,
/// and where it ends: the byte range a Range request asks for. 0, 0 for a
/// run out of range or empty.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn chapter_tile_from(first: u32, end: u32) -> u32 {
    tile(first, end).0
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn chapter_tile_to(first: u32, end: u32) -> u32 {
    tile(first, end).1
}

fn tile(first: u32, end: u32) -> (u32, u32) {
    let (first, end) = (first as usize, end as usize);
    if first >= end || end > corpus::chapter_count() {
        return (0, 0);
    }
    let (from, to) = corpus::tile_range(first..end);
    (from as u32, to as u32)
}

/// Writes a chapter's record at `chapter_record_ptr` and returns its length
/// in words; 0 when the chapter is out of range or its verses are not in.
///
/// Every string is an (offset, length) in the corpus text (`text_ptr`). The
/// head, `HEAD` words: book, section, the chapter's number, the book's name,
/// the section's name — each string as two words — whether it opens its book,
/// and the verse count. Then per verse: its global index, its number, the
/// verse as shown, the count of the other edition's runs, and each run's
/// offset and length.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn chapter_record(chapter: u32) -> u32 {
    let record = unsafe { &mut RECORD };
    record.clear();
    let Some(read) = passage::chapter(chapter as usize) else {
        return 0;
    };
    write(&read, record);
    record.len() as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn chapter_record_ptr() -> *const u32 {
    unsafe { RECORD.as_ptr() }
}

/// The contents record, written once: it never changes.
static mut CONTENTS: Vec<u32> = Vec::new();

/// Words of a contents record's head: portions, books, chapters.
pub const CONTENTS_HEAD: usize = 3;

/// Writes the Tanakh's contents at `contents_ptr` and returns its length in
/// words. Ready from the start: names and numbers are in the map part.
///
/// The head, `CONTENTS_HEAD` words: the counts of portions, books and
/// chapters. Then per portion its name; per book its portion, its name, its
/// first chapter and the one past its last; per chapter its number. Every
/// string is an (offset, length) in the corpus text (`text_ptr`).
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn contents_record() -> u32 {
    let record = unsafe { &mut CONTENTS };
    if record.is_empty() {
        write_contents(record);
    }
    record.len() as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn contents_ptr() -> *const u32 {
    unsafe { CONTENTS.as_ptr() }
}

/// The contents laid as words: the record the page reads.
pub fn write_contents(out: &mut Vec<u32>) {
    let books = contents::books();
    let portions = PORTIONS.len();
    out.extend_from_slice(&[portions as u32, books.len() as u32, corpus::chapter_count() as u32]);
    for section in 0..portions {
        out.extend_from_slice(&key(contents::portion_hebrew(section)));
    }
    for book in &books {
        out.push(book.section as u32);
        out.extend_from_slice(&key(book.hebrew));
        out.extend_from_slice(&[book.chapters.start as u32, book.chapters.end as u32]);
    }
    for chapter in 0..corpus::chapter_count() {
        out.extend_from_slice(&key(corpus::chapter_label(chapter)));
    }
}

fn key(text: &'static str) -> [u32; 2] {
    [corpus::offset_of(text) as u32, text.len() as u32]
}

/// A chapter laid as words: the record the page reads.
pub fn write(read: &passage::Chapter, out: &mut Vec<u32>) {
    out.extend_from_slice(&[read.book as u32, read.section as u32]);
    out.extend_from_slice(&key(corpus::chapter_label(read.chapter)));
    out.extend_from_slice(&key(corpus::book_name(read.book)));
    out.extend_from_slice(&key(corpus::section_name(read.section)));
    out.extend_from_slice(&[u32::from(read.opens), read.verses.len() as u32]);
    for verse in &read.verses {
        let shown = corpus::offset_of(verse.shown) as u32;
        out.push(verse.place.verse as u32);
        out.extend_from_slice(&key(corpus::verse_label(verse.place.verse)));
        out.extend_from_slice(&key(verse.shown));
        out.push(verse.others.len() as u32);
        for run in &verse.others {
            out.extend_from_slice(&[shown + run.start as u32, run.len() as u32]);
        }
    }
}
