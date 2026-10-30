//! Test suite of the computation.
//!
//! Everything the page relies on is computed here, so everything is checked
//! here: the corpus and its index, the layout and its invariants, the camera
//! and its thresholds, the frame and what it is forbidden to contain.
//!
//! The tests are written as **invariants, not as snapshots**: a ratio, an
//! order, an equality between all the cells of the Tanakh. A figure only
//! appears when the prose of `CLAUDE.md` states it as measured — then the test
//! is what keeps that measurement true.

mod api;
mod breadcrumb;
mod camera;
mod click;
mod excerpt;
mod frame;
mod geometry;
mod handoff;
mod landing;
mod locate;
mod marks;
mod readings;
mod text;


use crate::*;
pub(crate) use crate::map::*;
pub(crate) use crate::treemap::{Rect, WORLD};
pub(crate) use crate::wasm::map::*;

/// Windows the suite sweeps: phone, tablet, laptop, desktop, and the two
/// extreme shapes — very flat and very narrow.
pub const WINDOWS: [(f32, f32); 9] = [
    (320.0, 568.0),
    (390.0, 844.0),
    (768.0, 1024.0),
    (1024.0, 620.0),
    (1280.0, 800.0),
    (1536.0, 730.0),
    (1920.0, 1080.0),
    (2560.0, 600.0),
    (3840.0, 2160.0),
];

/// The reference window of the measurements quoted in `CLAUDE.md`.
pub const REFERENCE: (f32, f32) = (1536.0, 730.0);

/// A fresh state, framed on the whole world.
pub fn app_at(width: f32, height: f32) -> App {
    App::new(width, height)
}

pub fn reference_app() -> App {
    app_at(REFERENCE.0, REFERENCE.1)
}

/// Brings the camera to the **framing**: the three portions and the
/// thirty-nine books.
///
/// The map opens one notch above that, on the portions alone, so every test
/// that speaks of the framing comes here first. Set rather than zoomed:
/// `zoom_at` pulls towards a cell, and the framing has to be exact.
pub fn at_framing(app: &mut App) {
    app.camera.z = app.fit_scale();
    app.camera.x = (app.width - WORLD.w * app.camera.z) / 2.0;
    app.camera.y = (app.height - WORLD.h * app.camera.z) / 2.0;
}

/// A measured state at the framing.
///
/// Measured first, then framed: the framing has a floor that depends on the
/// width of a real numeral, so a camera placed before the measurement would be
/// left a notch off once it arrives.
pub fn framed_app(width: f32, height: f32) -> App {
    let mut app = app_at(width, height);
    measure_all(&mut app);
    at_framing(&mut app);
    app.build_frame();
    app
}

pub fn reference_framed() -> App {
    framed_app(REFERENCE.0, REFERENCE.1)
}

/// Feeds the state the widths a real Hebrew serif gives, so that the tests run
/// on measured numbers and not on the estimate.
///
/// `0.58` per letter is what `CLAUDE.md` records for Noto Serif Hebrew; the
/// point is not the exact value but that the layout then runs on a measurement
/// contradicting `GUESS`, as it does in the browser.
pub const MEASURED: f32 = 0.58;

pub fn measure_all(app: &mut App) {
    // Two frames: the first one lists what it needs, the second one converges.
    for _ in 0..2 {
        app.build_frame();
        let pending: Vec<u32> = app.pending.clone();
        for pair in pending.chunks(2) {
            let (offset, len) = (pair[0], pair[1]);
            let text = &corpus::text()[offset as usize..(offset + len) as usize];
            let ratio = letter::letter_count(text).max(1) as f32 * MEASURED;
            app.ratios.insert((offset, len), ratio);
        }
        app.lines.clear();
        app.number = None;
    }
    app.build_frame();
}

/// One command of the frame, read back as the page reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Command {
    pub kind: f32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub alpha: f32,
    pub size: f32,
    pub offset: u32,
    pub len: u32,
    /// Last slot: the ink. `INK_OURS` everywhere but on the reading of the
    /// other edition.
    pub ink: f32,
}

impl Command {
    pub fn is_text(&self) -> bool {
        self.kind == KIND_TEXT
    }

    pub fn text(&self) -> &'static str {
        &corpus::text()[self.offset as usize..(self.offset + self.len) as usize]
    }
}

/// Reads the command list back into records.
pub fn commands(app: &App) -> Vec<Command> {
    assert_eq!(app.commands.len() % STRIDE, 0, "the list is not a multiple of STRIDE");
    app.commands
        .chunks(STRIDE)
        .map(|c| Command {
            kind: c[0],
            x: c[1],
            y: c[2],
            w: c[3],
            h: c[4],
            alpha: c[5],
            size: c[6],
            offset: c[7] as u32,
            len: c[8] as u32,
            ink: c[9],
        })
        .collect()
}

pub fn frame(app: &mut App) -> Vec<Command> {
    app.build_frame();
    commands(app)
}

pub fn texts(app: &mut App) -> Vec<Command> {
    frame(app).into_iter().filter(Command::is_text).collect()
}

pub fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

pub fn assert_close(a: f32, b: f32, tol: f32, what: &str) {
    assert!(close(a, b, tol), "{what}: {a} != {b} (tolerance {tol})");
}

pub fn book_of(chapter: usize) -> usize {
    (0..corpus::book_count())
        .find(|&b| corpus::chapters_of(b).contains(&chapter))
        .expect("chapter outside every book")
}
