//! The search module's first- and last-letters exports
//! (`ethaor-search.wasm`, feature `search`).
//!
//! The browser's side of the search; `server/src/api/words.rs` is the HTTP
//! side of the same one. Both run `words::Engine` through the ELS engine
//! that lends it: this file only moves bytes across the WebAssembly
//! boundary — a query in, records out. The query is written where the ELS
//! one is (`els_query_ptr`).

use super::els::{els_engine, query_bytes};
use crate::corpus;
use crate::words::{LETTER_WORDS, Pick};

static mut WORDS_HITS: Vec<u32> = Vec::new();
/// Letters per hit of the last search: a record's length is twice that.
static mut WORDS_SPAN: u32 = 0;

/// Searches the `len` bytes written at `els_query_ptr` across the whole
/// Tanakh, one letter from each word: the `nth` (from 1), counted from the
/// word's end with `from_end` (1); its letters side by side, or in any order
/// with `any_order` (1); final forms folded, or told apart with `exact` (1).
///
/// Returns the exact number of hits — an f64, like the ELS — or -1 while the
/// text has not landed. Each kept record, `words_stride` words, names each
/// letter by its verse and its rank there; `words_hits_len` says how many.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn words_search(len: u32, nth: u32, from_end: u32, any_order: u32, exact: u32) -> f64 {
    words_search_in(len, nth, from_end, any_order, 0, corpus::verse_count() as u32, exact)
}

/// `words_search`, kept to the verses `first..end` (global indices): only the
/// hits whose every letter lies in them count. The twin of the API's
/// `portion`, `book`, `chapter` and `from`/`to`. An empty or out-of-bounds
/// range, or a letter no word holds, finds nothing.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn words_search_in(
    len: u32,
    nth: u32,
    from_end: u32,
    any_order: u32,
    first: u32,
    end: u32,
    exact: u32,
) -> f64 {
    super::fault::guard();
    if !corpus::has_text() {
        return -1.0;
    }
    let hits = unsafe { &mut WORDS_HITS };
    let pick = Pick { nth, from_end: from_end != 0 };
    let (first, end) = (first as usize, end as usize);
    let scope = first < end && end <= corpus::verse_count() && pick.valid();
    match core::str::from_utf8(query_bytes(len)) {
        Ok(query) if scope => {
            let total = els_engine().words.search_in(query, pick, any_order != 0, exact != 0, first..end, hits);
            unsafe { WORDS_SPAN = crate::letter::letter_count(query) as u32 };
            total as f64
        }
        _ => {
            hits.clear();
            0.0
        }
    }
}

#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn words_hits_ptr() -> *const u32 {
    unsafe { WORDS_HITS.as_ptr() }
}

/// Words of the records at `words_hits_ptr`, all hits together.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn words_hits_len() -> u32 {
    unsafe { WORDS_HITS.len() as u32 }
}

/// Words of one record of the last search: `LETTER_WORDS` per letter.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn words_stride() -> u32 {
    unsafe { WORDS_SPAN * LETTER_WORDS as u32 }
}
