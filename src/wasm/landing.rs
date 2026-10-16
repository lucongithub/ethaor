//! The text landing, exported by **both** modules: the map's corpus and the
//! search's fill from the same bytes, a tile or a book at a time.

use crate::corpus;
use crate::wasm::map::APP;


/// A tile as fetched, until `tile_load` lands it.
pub(crate) static mut TILE_IN: Vec<u8> = Vec::new();

/// Room for a tile of `len` bytes: where the page writes it.
#[unsafe(no_mangle)]
#[allow(static_mut_refs)]
pub extern "C" fn tile_buffer(len: u32) -> *mut u8 {
    super::fault::guard();
    unsafe {
        TILE_IN.clear();
        TILE_IN.resize(len as usize, 0);
        TILE_IN.as_mut_ptr()
    }
}

/// Lands the tile written at `tile_buffer`, chapters `first..end` — a run of
/// chapters or a whole book; the page then asks for a frame. Both modules
/// take them: the search module's corpus fills from the same bytes.
#[unsafe(no_mangle)]
#[allow(static_mut_refs)]
pub extern "C" fn tile_load(first: u32, end: u32) -> u32 {
    unsafe { corpus::land_chapters(first as usize..end as usize, &TILE_IN) };
    1
}

/// Lands the other edition's readings, written at `tile_buffer`, once every
/// chapter is in: the whole text is then there — the searches can run, and
/// the chapters laid without their readings are laid again. Returns 1 then,
/// 0 if chapters are still missing.
#[unsafe(no_mangle)]
#[allow(static_mut_refs)]
pub extern "C" fn readings_load() -> u32 {
    if !unsafe { corpus::land_readings(&TILE_IN) } {
        return 0;
    }
    if let Some(app) = unsafe { APP.as_mut() } {
        app.lines.clear();
    }
    1
}
