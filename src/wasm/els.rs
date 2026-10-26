//! The search module's ELS exports (`ethaor-search.wasm`, feature `search`).
//!
//! The browser's side of the search; `server/src/api/els.rs` is the HTTP
//! side of the same one. Both run `shared::els`: this file only moves bytes
//! across the WebAssembly boundary — a query in, records out.

use crate::shared::els::{Engine, letters_of};
use crate::{corpus, els};

/// Room for a query, in UTF-8 bytes: a pointed letter takes up to a dozen, so
/// this holds a few hundred letters.
const ELS_QUERY_CAP: usize = 4096;

/// The ELS search lives apart from `App`: it needs no window, so the page can
/// search before `init` and a search never touches the map's state.
static mut ELS_QUERY: [u8; ELS_QUERY_CAP] = [0; ELS_QUERY_CAP];
static mut ELS_HITS: Vec<u32> = Vec::new();
static mut ELS_ENGINE: Option<Engine> = None;

#[allow(static_mut_refs)]
pub(crate) fn els_engine() -> &'static mut Engine {
    unsafe { ELS_ENGINE.get_or_insert_with(Engine::new) }
}

/// Builds the search index now rather than at the first query: the page may
/// call it once idle, so the first search answers at full speed. Nothing
/// before the text has landed — the index is read off the verses.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn els_prepare() {
    if corpus::has_text() {
        els_engine();
    }
}

/// Where the page writes its query, as UTF-8, before calling `els_search`.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn els_query_ptr() -> *mut u8 {
    unsafe { ELS_QUERY.as_mut_ptr() }
}

#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn els_query_cap() -> u32 {
    ELS_QUERY_CAP as u32
}

/// Searches the `len` bytes written at `els_query_ptr` across the whole Tanakh,
/// with any skip from `skip0` to `skip1` letters jumped between two letters
/// of the query (0 = contiguous). Marks are ignored; final forms are folded,
/// or with `exact` (1) told apart — the API's `exact`.
///
/// Returns the exact number of hits — an f64, since a wide range of common
/// letters exceeds a u32 — or -1 while the text has not landed. The records, `els_stride` words each and at most
/// `els_cap` of them, sit at `els_hits_ptr`; `els_hits_len` says how many.
///
#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn els_search(len: u32, skip0: u32, skip1: u32, exact: u32) -> f64 {
    els_search_in(len, skip0, skip1, 0, corpus::verse_count() as u32, exact)
}

/// `els_search`, kept to the verses `first..end` (global indices): only the
/// occurrences whose every letter lies in them count. The twin of the API's
/// `portion`, `book`, `chapter` and `from`/`to`, which name such a range.
/// An empty or out-of-bounds range finds nothing.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn els_search_in(len: u32, skip0: u32, skip1: u32, first: u32, end: u32, exact: u32) -> f64 {
    super::fault::guard();
    if !corpus::has_text() {
        return -1.0;
    }
    let bytes = unsafe { &ELS_QUERY[..(len as usize).min(ELS_QUERY_CAP)] };
    let hits = unsafe { &mut ELS_HITS };
    let (first, end) = (first as usize, end as usize);
    let scope = (first < end && end <= corpus::verse_count()).then_some((first, end));
    match (core::str::from_utf8(bytes), scope) {
        (Ok(query), Some(scope)) => {
            let letters = letters_of(Some(scope));
            let engine = els_engine();
            engine.exact = exact != 0;
            engine.search_in(query, skip0, skip1, letters, hits) as f64
        }
        _ => {
            hits.clear();
            0.0
        }
    }
}

#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn els_hits_ptr() -> *const u32 {
    unsafe { ELS_HITS.as_ptr() }
}

#[cfg_attr(feature = "search", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn els_hits_len() -> u32 {
    unsafe { (ELS_HITS.len() / els::STRIDE) as u32 }
}

#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn els_stride() -> u32 {
    els::STRIDE as u32
}

#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn els_cap() -> u32 {
    els::CAP as u32
}

/// Probe: imposes a search method — 1, 2, 3 for `els::METHODS` — or gives
/// the choice back to the cost model with 0. Only the benchmark calls it.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn probe_els_method(method: u32) {
    els_engine().force = els::METHODS.get((method as usize).wrapping_sub(1)).copied();
}

/// Probe: forgets the remembered answers — and with `cold`, the planes and
/// residue lists too — so the benchmark times a search, not a recall.
#[cfg_attr(feature = "search", unsafe(no_mangle))]
pub extern "C" fn probe_els_forget(cold: u32) {
    let engine = els_engine();
    engine.forget_answers();
    if cold != 0 {
        engine.index.forget();
    }
}
