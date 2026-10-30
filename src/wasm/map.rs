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
    let place = app.pick_place(wx, wy);
    let picked = app.pick(wx, wy);
    keep(app, picked, place.map(|(_, verse)| verse))
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
    keep(app(), picked, Some(verse as usize))
}

/// The selection mode's drag begins at a point of the canvas: the stretch
/// starts at the letter there. 1 if a letter one reads is there; 0 — and no
/// selection — otherwise, and the page pans as usual.
///
/// No endpoint twin, as for every gesture: a stretch is selected on a
/// canvas, and `GET /api/chapter` already gives any verse's text.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn excerpt_start(x: f32, y: f32) -> u32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    app.excerpt_start(wx, wy) as u32
}

/// The drag reaches a point of the canvas: the stretch runs to the letter
/// there.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn excerpt_to(x: f32, y: f32) {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    app.excerpt_to(wx, wy);
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn excerpt_clear() {
    app().excerpt_clear();
}

/// The text selected, ours, each verse on its own line.
static mut EXCERPT: String = String::new();

/// Writes the text selected at `excerpt_ptr`; its length in bytes, 0 for
/// none.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn excerpt_text() -> u32 {
    let text = app().excerpt_text();
    unsafe {
        EXCERPT = text;
        EXCERPT.len() as u32
    }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn excerpt_ptr() -> *const u8 {
    unsafe { EXCERPT.as_ptr() }
}

/// The verses' letters selected: `(verse, rank)` pairs.
static mut EXCERPT_LETTERS: Vec<u32> = Vec::new();

/// Writes the verses' letters the selection holds at `excerpt_letters_ptr`,
/// as a letters record names them — each letter's verse and its rank among
/// the verse's consonants; their number.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn excerpt_letters() -> u32 {
    unsafe {
        app().excerpt_letters(&mut EXCERPT_LETTERS);
        (EXCERPT_LETTERS.len() / 2) as u32
    }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn excerpt_letters_ptr() -> *const u32 {
    unsafe { EXCERPT_LETTERS.as_ptr() }
}

/// The found letters of the verse selected: `(from, to, slots)` triples.
static mut SELECTION_MARKS: Vec<u32> = Vec::new();

#[allow(static_mut_refs)]
fn keep(app: &mut App, picked: Option<[u32; 6]>, verse: Option<usize>) -> u32 {
    let marks = unsafe { &mut SELECTION_MARKS };
    marks.clear();
    match (picked, verse) {
        (Some(selection), Some(verse)) => {
            app.selection = selection;
            app.passage_marks(verse, marks);
            1
        }
        _ => {
            app.selection = [0; 6];
            0
        }
    }
}

/// How many found letters the passage selected holds: `(from, to, slots)`
/// triples at `selection_marks_ptr`, in bytes of its verse — the selection's
/// own text — the shown searches holding them as bits, in text order.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn selection_marks_len() -> u32 {
    unsafe { (SELECTION_MARKS.len() / 3) as u32 }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn selection_marks_ptr() -> *const u32 {
    unsafe { SELECTION_MARKS.as_ptr() }
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

// --------------------------------------------------------------------- marks

/// A search's hits as the search module left them, until `marks_set` places
/// them.
static mut MARKS_IN: Vec<u32> = Vec::new();

/// The largest skip a word of `letters` letters can be read with in the
/// whole Tanakh (`els::max_skip`): what bounds the equidistant search's
/// fields, before the text lands. No HTTP twin: past it the engine finds
/// nothing, and the API answers that by itself.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn skip_max(letters: u32) -> u32 {
    crate::els::max_skip(letters as usize)
}

/// Letters of the Tanakh's longest word (`words::LONGEST_WORD`): how many
/// letters of each word "every nth letter" reads, known before the text
/// lands. No HTTP twin: `GET /api` declares the bound in its schema.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn longest_word() -> u32 {
    crate::words::LONGEST_WORD
}

/// Searches the map marks at once; the page keeps no more.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn marks_slots() -> u32 {
    MARK_SLOTS as u32
}

/// Room for `words` words of hit records: where the page copies a search's
/// `els_hits` before `marks_set`.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn marks_buffer(words: u32) -> *mut u32 {
    super::fault::guard();
    unsafe {
        MARKS_IN.clear();
        MARKS_IN.resize(words as usize, 0);
        MARKS_IN.as_mut_ptr()
    }
}

/// Marks in slot `slot` the hits written at `marks_buffer` — `span` letters
/// per hit, in `form`: 0 `els_stride` words each, 1 `words_stride` — shown
/// at once; the page then asks for a frame. Returns the runs placed, 0 for a
/// slot out of range.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn marks_set(slot: u32, span: u32, form: u32) -> u32 {
    let records = unsafe { core::mem::take(&mut MARKS_IN) };
    app().set_marks_of(slot as usize, &records, span as usize, Form::of(form)) as u32
}

/// Shows (1) or hides (0) a slot's marks, keeping them.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn marks_show(slot: u32, on: u32) {
    app().show_marks(slot as usize, on != 0);
}

/// Forgets a slot's marks.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn marks_clear(slot: u32) {
    app().clear_marks(slot as usize);
}

/// Joins the searches shown by any (`all` 0), each drawn whole, or by all
/// (1), each drawn only in the passages holding a hit of every one.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn marks_join(all: u32) {
    app().join_marks(all != 0);
}

/// The found letters of the chapter the book view last read.
static mut CHAPTER_MARKS: Vec<u32> = Vec::new();

/// The letters the shown searches found in a chapter, as the book view
/// shows it: `(from, to, slots)` triples at `chapter_marks_ptr`, slots as bits, offsets in
/// the corpus text of its verses as shown — the strings of its record. The
/// count of triples; 0 for none, or a chapter not landed.
///
/// No endpoint twin: a search's marks live in the page, kept by its reader.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn chapter_marks(chapter: u32) -> u32 {
    let out = unsafe { &mut CHAPTER_MARKS };
    app().book_marks_of(chapter as usize, out);
    (out.len() / 3) as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn chapter_marks_ptr() -> *const u32 {
    unsafe { CHAPTER_MARKS.as_ptr() }
}

/// Sets the list of passages to the hits written at `marks_buffer` — `span`
/// letters per hit, in `form` as `marks_set` takes them — for the page to
/// list them. Returns the number of passages.
///
/// No endpoint twin: `GET /api/els` already names every hit's verses; this
/// list carries the map's colours, which only a page wears.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn results_set(span: u32, form: u32) -> u32 {
    let records = unsafe { core::mem::take(&mut MARKS_IN) };
    app().set_results_of(&records, span as usize, Form::of(form)) as u32
}

/// Sets the list of passages to the searches shown together: those holding
/// a hit of any (`all` 0) or where two at least cross (`all` 1), grouped by
/// crossing (`results_groups`). Returns how many.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn results_shown(all: u32) -> u32 {
    app().set_shown_results(all != 0) as u32
}

/// The crossings of the list last set by `results_shown`.
static mut RESULTS_GROUPS: Vec<u32> = Vec::new();

/// Writes the list's crossings at `results_groups_ptr` — `(slots,
/// passages)` pairs in the order of the list, slots as bits — and returns
/// how many; none for one search's list.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn results_groups() -> u32 {
    let out = unsafe { &mut RESULTS_GROUPS };
    out.clear();
    for &(bits, count) in &app().results.groups {
        out.extend_from_slice(&[bits, count]);
    }
    (out.len() / 2) as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn results_groups_ptr() -> *const u32 {
    unsafe { RESULTS_GROUPS.as_ptr() }
}

/// How many passages the searches shown make together, of any (`all` 0)
/// or where two at least cross (`all` 1), the list left as it is.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn passages_shown(all: u32) -> u32 {
    app().shown_passages(all != 0).len() as u32
}

/// How many passages the hits written at `marks_buffer` make, `span`
/// letters each in `form`, as `results_set` would list them; the buffer is
/// taken.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn passages_count(span: u32, form: u32) -> u32 {
    let records = unsafe { core::mem::take(&mut MARKS_IN) };
    passage_count_of(&records, span as usize, Form::of(form)) as u32
}

/// Forgets the list of passages.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn results_clear() {
    app().clear_results();
    unsafe {
        RESULTS_OUT = Vec::new();
    }
}

/// The last page of passages asked for.
static mut RESULTS_OUT: Vec<u32> = Vec::new();

/// Writes passages `from..from + count` at `results_ptr` (`App::results_page`
/// says how) and returns its length in words; 0 past the end.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn results_page(from: u32, count: u32) -> u32 {
    let out = unsafe { &mut RESULTS_OUT };
    app().results_page(from as usize, count as usize, out);
    out.len() as u32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn results_ptr() -> *const u32 {
    unsafe { RESULTS_OUT.as_ptr() }
}

/// The shown searches holding a place, as bits of their slots: a portion
/// (`level` 0), a book (1) or a chapter (2) — the book view's squares.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn marks_at(level: u32, index: u32) -> u32 {
    app().place_marks(level, index as usize)
}
