//! The map module's exports (`ethaor.wasm`, feature `map`): gestures, frames,
//! tiles, the landing. They only move values across the WebAssembly
//! boundary; the computation is `App`'s, in `crate::map`. No HTTP twin: a
//! frame is for a canvas, and only the page has one.

use crate::map::*;
use crate::corpus;

pub(crate) static mut APP: Option<App> = None;

#[allow(static_mut_refs)]
pub(crate) fn app() -> &'static mut App {
    unsafe { APP.as_mut().expect("init() was not called") }
}

// ----------------------------------------------------------------- interface

/// Prepares the state. Returns the number of books, which serves as a check.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn init(width: f32, height: f32) -> u32 {
    super::fault::guard();
    unsafe { APP = Some(App::new(width, height)) };
    corpus::book_count() as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn resize(width: f32, height: f32) {
    let app = app();
    let refit = app.at_opening();
    app.width = width.max(1.0);
    app.height = height.max(1.0);
    if refit {
        app.fit();
    }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn reset() {
    let app = app();
    app.level = 0;
    app.fit();
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn pan(dx: f32, dy: f32) {
    let app = app();
    app.camera.x += dx;
    app.camera.y += dy;
}

/// Factor of one wheel notch, computed for this window: the page raises it to
/// the number of notches of the gesture. See `App::zoom_step`.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn zoom_step() -> f32 {
    app().zoom_step()
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn zoom(x: f32, y: f32, factor: f32) {
    app().zoom_at(x, y, factor);
}

/// Chapter the cursor can click on, or -1. Only serves the pointer shape:
/// hovering changes nothing in the drawing.
///
/// **The pointer announces what a click will really do**, so it answers on the
/// very condition of a click: the same tolerance as `locate` — a book box,
/// never the air between two portions — and a chapter one can read. Above the
/// reading zoom nothing is clickable, and the cursor says so.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn hover(x: f32, y: f32) -> i32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    app.clickable_at(wx, wy).map_or(-1, |chapter| chapter as i32)
}

/// Fills the selection: book, chapter, the verse and its number — the last
/// two an (offset, length), all six slots readable via `selection_ptr`.
/// Returns 1 if something was hit.
///
/// **A click is only live on a passage one can read.** Above that zoom a
/// chapter is a number, and naming one of its verses would be a choice of the
/// geometry, not of the reader: the answer is then 0 and the selection is
/// cleared.
///
/// The export is only unmangled on wasm: on a host build — the one the test
/// suite uses — the symbol `select` collides with the one of the system socket
/// library, and the link fails.
#[cfg_attr(all(target_arch = "wasm32", feature = "map"), unsafe(no_mangle))]
pub extern "C" fn select(x: f32, y: f32) -> u32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    let picked = app.pick(wx, wy);
    keep(app, picked)
}

/// The book view's click, where the browser laid the verse out: the same
/// slots as `select`, for the verse of global index `verse`. Returns 0 for a
/// verse out of range or not landed.
///
/// No endpoint twin: like the map's own click it answers a pointer on a
/// page, and `GET /api/chapter` already names every verse.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn select_verse(verse: u32) -> u32 {
    let picked = App::pick_in_verse(verse as usize);
    keep(app(), picked)
}

fn keep(app: &mut App, picked: Option<[u32; 6]>) -> u32 {
    match picked {
        Some(selection) => {
            app.selection = selection;
            1
        }
        None => {
            app.selection = [0; 6];
            0
        }
    }
}

/// Fills `focus_ptr`: section, book, chapter and verse under the **centre of
/// the view**, each as an (offset, length) in the corpus. Zero length =
/// nothing at that level.
///
/// That is what feeds the breadcrumb when the pointer says nothing: it always
/// tells where one is, including when the title bands have left the screen.
/// Its depth is that of the zoom — the chapter arrives with the numbers, the
/// verse with the text.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn focus() {
    let app = app();
    if app.mouse_away {
        app.focus = [0; 8];
        return;
    }
    let (wx, wy) = app.camera.to_world(app.width / 2.0, app.height / 2.0);
    app.focus = app.path_at(wx, wy, false).unwrap_or([0; 8]);
}

/// Same, under a point of the screen: **what the pointer is on**.
///
/// Returns 1 when the pointer names something — a chapter of a book box, or a
/// portion by its title — and fills `focus_ptr` with it. Returns 0 when it
/// names nothing, the air between two portions for instance, and falls back on
/// the centre of the view, so that the breadcrumb never blinks out while the
/// pointer wanders.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn focus_at(x: f32, y: f32) -> u32 {
    let app = app();
    // A pointer on the canvas: whatever left before is back.
    app.mouse_away = false;
    let (wx, wy) = app.camera.to_world(x, y);
    match app.path_at(wx, wy, true) {
        Some(path) => {
            app.focus = path;
            1
        }
        None => {
            // **The pointer on nothing clears the breadcrumb.** Falling back
            // to the centre of the view kept the last position on screen,
            // which read as a place the pointer had already left. The price is
            // a breadcrumb that blinks out between two letters; naming
            // somewhere one is not is worse than naming nowhere.
            //
            // With no pointer at all — a touch screen — the page asks `focus`
            // instead, and the centre speaks: there is no hover to contradict.
            app.focus = [0; 8];
            0
        }
    }
}

/// The pointer has left the canvas; `mouse` is 1 for a mouse, 0 for a finger
/// or a pen lifted.
///
/// **A mouse gone names nothing.** It went somewhere — often onto the
/// breadcrumb row itself — and the centre of the view would then answer for a
/// place the reader is not looking at, which read as a selected portion. A
/// lifted finger is different: a touch screen has no hover, and the centre is
/// the only position there is. `focus_at` clears the state as soon as a
/// pointer is back on the canvas.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn leave(mouse: u32) {
    app().mouse_away = mouse != 0;
}

/// The chapter at the centre of the view, never deeper than the image: what
/// the book view opens on. See `App::here`.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn here_chapter() -> u32 {
    app().here().0 as u32
}

/// The verse at the centre of the view when its text is read, -1 otherwise.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn here_verse() -> i32 {
    app().here().1.map_or(-1, |v| v as i32)
}

/// How deep the image goes: 0 the portions alone, 1 the books, 2 the
/// chapters' numbers, 3 the text. See `App::depth`.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn here_depth() -> u32 {
    app().depth()
}

/// Back to the framing — the portions and their books — at the top world,
/// as `reset` goes back to the opening one notch above.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn frame_books() {
    let app = app();
    app.level = 0;
    app.frame_books();
}

/// Brings the book holding `chapter` to the chapters' level, its box filling
/// the view. Out of range, nothing. See `App::show_book`.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn show_book(chapter: u32) {
    app().show_book(chapter as usize);
}

/// The size, in pixels, the book view gives the three portions' names: the
/// map's opening draws them at it. Not finite or not positive, ignored.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn set_portion_size(px: f32) {
    if px.is_finite() && px > 0.0 {
        app().portion_size = px;
    }
}

/// Brings `chapter` to its reading level, `verse` at the centre (-1: its
/// top): where the book view hands the reader back. Out of range, nothing.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn show(chapter: u32, verse: i32) {
    app().show(chapter as usize, usize::try_from(verse).ok());
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn focus_ptr() -> *const u32 {
    app().focus.as_ptr()
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn selection_ptr() -> *const u32 {
    app().selection.as_ptr()
}

/// Builds the frame's command list. Returns its number of words.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn build_frame() -> u32 {
    let app = app();
    app.build_frame();
    app.commands.len() as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn frame_ptr() -> *const f32 {
    app().commands.as_ptr()
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn frame_stride() -> u32 {
    STRIDE as u32
}

/// Fragments whose width the page must measure: (offset, size) pairs.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn pending_ptr() -> *const u32 {
    app().pending.as_ptr()
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn pending_len() -> u32 {
    app().pending.len() as u32
}

/// Forgets every measured width.
///
/// To be called when the font changes — when the final font loads, for
/// instance: widths taken on a fallback font would be wrong for good.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn clear_ratios() {
    let app = app();
    app.ratios.clear();
    app.lines.clear();
    app.number = None;
}

/// Records a measured width, for a font of size 1.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn set_ratio(offset: u32, len: u32, ratio: f32) {
    let app = app();
    app.ratios.insert((offset, len), ratio);
    // The layout depends on the widths: it must be rebuilt.
    app.lines.clear();
    app.number = None;
}

/// The corpus, in UTF-8, straight from the module's memory.
/// Diagnostic: camera state and depth of descent.
/// Camera position, in pixels. Serves the probes: without it, a reading has to
/// guess the framing and gets it wrong as soon as one has zoomed.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn cam_x() -> f32 {
    app().camera.x
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn cam_y() -> f32 {
    app().camera.y
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn cam_z() -> f32 {
    app().camera.z
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn cam_fit() -> f32 {
    app().fit_scale()
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn depth() -> u32 {
    app().level
}

/// Diagnostic: on-screen width of the aimed nest, or -1 if there is none.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_nest(x: f32, y: f32) -> f32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.nest_at(wx, wy) {
        Some(nest) => app.camera.project(nest).w,
        None => -1.0,
    }
}

/// Diagnostic: on-screen width of the aimed letter cell.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_letter(x: f32, y: f32) -> f32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.locate(wx, wy) {
        Some((_, _, _, Some(cell))) => app.camera.project(cell).w,
        _ => -1.0,
    }
}

/// Diagnostic: 1 if the old world has given way to the next.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_fade() -> f32 {
    let app = app();
    let camera = app.camera;
    let _ = camera;
    if app.last_nests == 0 { 0.0 } else { 1.0 }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_points() -> u32 {
    app().last_nests
}

/// Diagnostic: readings of the other edition drawn at the last frame.
///
/// The index under the canvas does not follow this count but the pointer —
/// see `readings_at`. A reading can be on screen without the reader having
/// anything under the cursor, and explaining a mark he is not looking at is
/// noise.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn variants_drawn() -> u32 {
    app().variant_runs
}

/// Probe: 1 when the last frame draws at least one line of a verse — the
/// reading level reached and its text there. What the load benchmark times:
/// the first verse a visitor can read.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_verse_drawn() -> u32 {
    app().draws_a_verse() as u32
}

/// The other reading under the pointer: one past the corpus offset where
/// theirs starts, 0 when the pointer is on neither form. Its end is then at
/// `readings_end`.
///
/// The page opens the index under the canvas on that answer, and colours that
/// reading — and no other — in the text.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn readings_at(x: f32, y: f32) -> u32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    let lit = app.readings_at(wx, wy);
    app.reading_lit = lit.map_or((0, 0), |(from, to)| (from as u32, to as u32));
    lit.map_or(0, |(from, _)| from as u32 + 1)
}

/// End (excluded) of the reading `readings_at` last answered.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn readings_end() -> u32 {
    app().reading_lit.1
}

/// Diagnostic: geometry of a book in the world (0=x 1=y 2=w 3=h).
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_book(book: u32, field: u32) -> f32 {
    let r = app().layout.books[book as usize];
    match field {
        0 => r.x,
        1 => r.y,
        2 => r.w,
        _ => r.h,
    }
}

/// Diagnostic: geometry of a chapter in the world (0=x 1=y 2=w 3=h).
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn probe_chapter(chapter: u32, field: u32) -> f32 {
    let r = app().layout.chapters[chapter as usize];
    match field {
        0 => r.x,
        1 => r.y,
        2 => r.w,
        _ => r.h,
    }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn text_ptr() -> *const u8 {
    corpus::text().as_ptr()
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn text_len() -> u32 {
    corpus::text().len() as u32
}

// ---------------------------------------------------------------------- text

/// Asks the module which tiles the view needs now: chapters about to be read
/// whose verses are not in. Returns the number of runs, four words each at
/// `tiles_ptr`: first chapter, end chapter, and the byte range to request
/// from the opened text part (`ethaor-text.plain.bin`), end excluded.
///
/// The map of Google Maps' kind: while the whole text travels in the
/// background, the chapters the visitor dives into come first, a few
/// kilobytes each.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn tiles_wanted() -> u32 {
    app().want_tiles() as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn tiles_ptr() -> *const u32 {
    app().tiles.as_ptr()
}

/// The book whose text to fetch next, in the background: of those not asked
/// yet, the nearest to the centre of the view — so the text arrives where the
/// visitor is first, and follows them if they move. -1 once every book has
/// been asked for.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn next_book() -> i32 {
    app().next_book().map_or(-1, |b| b as i32)
}

/// First chapter of a book, and the chapter after its last.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn book_first_chapter(book: u32) -> u32 {
    corpus::chapters_of(book as usize).start as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn book_end_chapter(book: u32) -> u32 {
    corpus::chapters_of(book as usize).end as u32
}
