//! The map: everything the page draws with. `App` holds one window's state;
//! each file below takes one subject of it, and the WebAssembly exports that
//! drive it live in `wasm/map.rs`.
//!
//! The computation holds no global state: a test drives a window of its own.

mod camera;
mod frame;
mod hit;
mod landing;
mod measure;
mod settings;
mod text;

use std::collections::HashMap;

use crate::treemap::{self, Layout, Rect, WORLD};
use crate::{corpus, letter, variant};

pub(crate) use camera::*;
pub(crate) use frame::*;
pub(crate) use hit::*;
pub(crate) use measure::*;
pub(crate) use settings::*;
pub(crate) use text::*;

/// One laid-out line: a slice of verse, its box and its character size. The
/// unit of rendering is not the verse but the line — a long verse takes
/// several.
#[derive(Clone, Copy)]
pub(crate) struct Line {
    pub(crate) rect: Rect,
    pub(crate) text: &'static str,
    pub(crate) verse: usize,
    pub(crate) size: f32,
}

pub(crate) struct App {
    pub(crate) layout: Layout,
    pub(crate) camera: Camera,
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// Number of Tanakhs crossed. The descent is one-way: once the new world
    /// has appeared, the old one no longer exists.
    pub(crate) level: u32,
    /// Width of a fragment for a font of size 1, measured by the page.
    pub(crate) ratios: HashMap<(u32, u32), f32>,
    /// Fragments whose width the page has yet to give.
    pub(crate) pending: Vec<u32>,
    pub(crate) commands: Vec<f32>,
    /// Diagnostic: nests kept at the last frame.
    pub(crate) last_nests: u32,
    /// Runs of the other edition's reading drawn at the last frame. The page
    /// shows the index at the bottom only when there is one on screen.
    pub(crate) variant_runs: u32,
    /// Corpus range of the other reading last found under the pointer
    /// (`readings_at`), (0, 0) for none: read back by the page with its end.
    pub(crate) reading_lit: (u32, u32),
    /// One space from the corpus, to measure its width once and for all.
    pub(crate) space: Option<&'static str>,
    /// Layout of the chapters already visited. Invalidated as soon as a
    /// measurement arrives from the page.
    pub(crate) lines: HashMap<usize, Vec<Line>>,
    /// Size of chapter numbers, in world units. Computed once, thrown away as
    /// soon as a measurement arrives from the page.
    pub(crate) number: Option<f32>,
    /// Width of the widest number, as the current layout assumed it. A
    /// measurement that contradicts it rebuilds the layouts.
    pub(crate) numeral: f32,
    /// The other level of detail, ready to take the current one's place.
    pub(crate) spare: Layout,
    /// True when the current layout is the one for chapters.
    pub(crate) detail: bool,
    /// Minimum framing, cached: see `base_scale`.
    pub(crate) base: f32,
    /// Selection: book, chapter, verse, verse number — the last two as
    /// (offset, length).
    pub(crate) selection: [u32; 6],
    /// Breadcrumb: section, book, chapter and verse, each an (offset, length).
    pub(crate) focus: [u32; 8],
    /// A mouse has left the canvas. The centre of the view then names nothing
    /// until the pointer comes back — see `leave`.
    pub(crate) mouse_away: bool,
    /// Whether verses may be read at all. A chapter opens to the reading
    /// level once its own verses have landed (`corpus::chapter_ready`);
    /// until then it draws its number, and nothing that needs a verse —
    /// drawing it, clicking it, naming it — is asked. Always on in the
    /// module; the tests turn it off to stand for a corpus with no verse in.
    pub(crate) text: bool,
    /// Size the three portions' names take at the opening, in pixels: the
    /// book view's own (`set_portion_size`), so the two views show them
    /// alike. 0 until the page gives it: the projected world's share then.
    pub(crate) portion_size: f32,
    /// Chapters whose tile has been asked for already.
    asked: Vec<bool>,
    /// Books whose text has been asked for, in the background.
    books_asked: Vec<bool>,
    /// Tiles to fetch, as `tiles_wanted` leaves them for the page: per run of
    /// chapters, first, end, and its byte range in the opened text part.
    pub(crate) tiles: Vec<u32>,
}

impl App {
    /// Fresh state for a window, framed on the whole world.
    ///
    /// `init` only wraps it in the global slot: the computation itself holds no
    /// global state, which is what lets a test drive a window of its own.
    pub(crate) fn new(width: f32, height: f32) -> App {
        let (wide, tight) = levels(ESTIMATED_NUMERAL);
        let mut app = App {
            layout: wide,
            spare: tight,
            detail: false,
            base: 0.0,
            camera: Camera { z: 1.0, x: 0.0, y: 0.0 },
            width: width.max(1.0),
            height: height.max(1.0),
            level: 0,
            ratios: HashMap::new(),
            pending: Vec::new(),
            commands: Vec::new(),
            last_nests: 0,
            variant_runs: 0,
            reading_lit: (0, 0),
            space: None,
            lines: HashMap::new(),
            number: None,
            numeral: ESTIMATED_NUMERAL,
            selection: [0; 6],
            focus: [0; 8],
            mouse_away: false,
            text: true,
            portion_size: 0.0,
            asked: vec![false; corpus::chapter_count()],
            books_asked: vec![false; corpus::book_count()],
            tiles: Vec::new(),
        };
        app.base = app.base_scale();
        app.fit();
        app
    }
}

/// The two layouts: wide air for the overview, half as much as soon as the
/// chapters appear.
pub(crate) fn levels(numeral: f32) -> (Layout, Layout) {
    (
        treemap::build(numeral, treemap::SECTION_LEAD_WIDE),
        treemap::build(numeral, treemap::SECTION_LEAD_TIGHT),
    )
}

/// Size of a chapter number in a given layout, in world units.
///
/// **Half a book title.** The title fills its band, whose height `chapters_fit`
/// deduced from that ratio and from the width of the digits: the cell is
/// therefore cut to hold that body. The two bounds that follow only keep the
/// invariant — a number reads in full or not at all.
pub(crate) fn number_in(layout: &Layout, widest: f32) -> f32 {
    let cell = layout.chapters[0];
    (layout.book_bands[0].h * treemap::CHAPTER_OVER_BOOK)
        .min(cell.h * treemap::NUMBER_HEIGHT)
        .min(cell.w * treemap::NUMBER_WIDTH / widest)
}
