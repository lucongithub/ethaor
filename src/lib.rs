//! ethaor — the Tanakh as a zoomable map, with no background.
//!
//! All the computation lives here, compiled to WebAssembly: corpus, layout,
//! camera, zoom folding, pruning, click detection. The page only paints the
//! command list produced for each frame. Nothing travels over the network
//! during interaction.
//!
//! One single thing stays with the browser: **measuring and drawing text**.
//! That is where the shaping of pointed Hebrew lives — bidi, combining marks —
//! and no in-house implementation would do it as well. Rust asks for the
//! widths it lacks, caches them, and never comes back to them.

mod corpus;
mod hier;
#[cfg(test)]
mod tests;
mod treemap;
mod variant;

use std::collections::HashMap;

use treemap::{Layout, Rect, WORLD};

// ---------------------------------------------------------------- settings

/// Share of a letter's cell taken by the world it contains.
///
/// **Large, and that is what removes the white square.** At 0.004, the next
/// Tanakh measured only six pixels at the moment of folding: nothing was
/// legible in it, and a white fill was needed so the screen would not be black
/// during the fifteen or so notches it took to grow.
///
/// At 0.9, it is born nine tenths of the screen wide, so its book titles pass
/// `MIN_TEXT` immediately: the rendering goes from one text to the next without
/// ever painting anything in between. Measured: 1 382 px on a 1 536 window,
/// 351 px on a 390 phone — and in both cases zero frame without text. At 0.7,
/// the phone had a black frame at the fold.
const NEST: f32 = 0.9;

/// Folding only happens once the letter is wider than the screen: one can
/// therefore zoom fully into a letter without seeing anything arrive. Nothing
/// of the world it contains is drawn before that moment, and by then it is
/// already a thousand pixels wide: we go from one text to the next with no
/// intermediate state.
const NEST_AFTER: f32 = 1.0;

/// Below this, text would be no more than a grey smear.
const MIN_TEXT: f32 = 4.0;

/// Share of its third of the world taken by a portion title, when the map is
/// nothing but three words, and the gap between two of those words as a share
/// of the world height.
///
/// Both are taken on the **projected** world: the three names are born large at
/// the first zoom-out notch, then shrink and draw closer at every following
/// notch, endlessly. Measured, 1 536 x 730 window: 61 px and a 130 px gap at
/// the first notch, then 43 and 93, 31 and 66, 22 and 48.
///
/// The gap was tried at a third of the world height — the three words then held
/// the corners of the screen and no longer read as a group — then at a sixth,
/// too tight. At a quarter, it is worth 2.1 times the size of the words.
const SECTION_CARD: f32 = 0.35;
const CARD_GAP: f32 = 1.0 / 4.0;

/// Floor specific to chapter numbers, lower than the one for running text: a
/// number has one to three letters, known in advance and read as a shape, where
/// a line of verse has to be deciphered.
///
/// That is what guarantees the promise "chapters from the first notch": at the
/// notch above the framing, a number measures 4.7 px on a window 730 px tall
/// and 4.0 px on one of 620. Since the portions' air tightens at chapter level,
/// the floor no longer decides anything: it is only there as a guard rail.
const MIN_NUMBER: f32 = 3.5;

/// Hierarchy of whites: a section is pure white, a book one tone below, a
/// chapter number a shade lower still. Rank therefore reads by intensity,
/// without any frame being needed.
const BOOK_INK: f32 = 0.78;
const CHAPTER_INK: f32 = 0.58;

/// Share of the title band taken by the text.
///
/// The title is sized by **the band**, which is in world units: it therefore
/// grows with the zoom, exactly like the box and the chapters, and the space
/// between the top of the frame and the first chapter keeps the same look. The
/// band being of constant height, every book carries the same title size at a
/// given zoom.
const TITLE_FILL: f32 = 1.0;

/// Ceiling, as a share of the window height. Past that point the band has left
/// the screen — we have entered the book — and the title has no need to grow
/// further: it is only there to recall where one is.
const TITLE_CEILING: f32 = 0.085;

/// Minimum line height for a chapter to show its verses.
///
/// Just above the text floor: at a leading of 1.30, a 5.5 px line carries
/// characters of 4.2 px, i.e. `MIN_TEXT`. Demanding more gained nothing in
/// legibility and pushed the verses two zoom notches further away.
const READABLE_LINE: f32 = 5.5;

/// Inner margin of a cell, as a **share of its side**.
///
/// Fixed at 2 world units, it left only 5.9 units of text in a cell of 9.9 —
/// 40 % of the ink thrown away, in width as in height, and verses that only
/// appeared six zoom notches later. A cell's margin must follow the cell, like
/// all the rest of the rendering.
const CELL_PAD: f32 = 0.06;

/// Nominal factor of one wheel notch, and length of the dive in notches: from
/// the framing to a full-screen chapter. `zoom_step` deduces the real factor
/// from it, so that this length is the same on every window.
const ZOOM_STEP: f32 = 1.4;
const DIVE_NOTCHES: f32 = 14.0;

/// Beyond this multiple of the initial framing, chapter numbers appear.
///
/// At launch the map therefore shows only the sections and the books, large;
/// the numbers arrive from the first zoom notch. The threshold is relative to
/// the framing and not to a cell size: it thus falls in the same place whatever
/// the window, and after a fold as before.
const CHAPTER_FROM: f32 = 1.1;

/// Below this multiple of the initial framing, the map is nothing but the three
/// portion names.
///
/// The three levels are therefore **one notch from each other**, on either side
/// of the framing: one zoom-out notch gives the three portions, the framing
/// gives the portions and the books, one zoom-in notch gives the chapters. Like
/// `CHAPTER_FROM`, the threshold is relative to the framing, not to a size: it
/// falls in the same place whatever the window.
///
/// It is just below 1 and not at 1: the framing itself must stay on the books'
/// side, rounding included.
const PORTIONS_UNDER: f32 = 0.95;

/// Line height and breathing space between verses, as multiples of the
/// character size of a chapter's text.
const LINE_K: f32 = 1.30;
const GAP_K: f32 = 0.55;

/// Marks of a chapter laid out as text, as multiples of its character size.
///
/// **The chapter number sits in the top-right corner of its column**, where
/// reading starts, on a row of its own above the first verse: once the text is
/// out, the centred number has gone, and the reader still has to know which
/// chapter he is reading. **A verse number sits at the right of its first
/// line**, in a gutter of its own: Hebrew reads from the right, so that is
/// where a verse begins. Both are reserved by the layout, never laid over the
/// text — the gutter and the header row are paid on the character size.
const CHAPTER_MARK: f32 = 2.0;
const VERSE_MARK: f32 = 0.8;

/// Section titles are two thirds bigger than book titles.
const SECTION_OVER_BOOK: f32 = 1.25;

/// Sizes of one level. Every drawn world has its own.
#[derive(Clone, Copy)]
struct Scale {
    show_chapters: bool,
    chapter: f32,
    book: f32,
    section: f32,
}

/// Estimated width of a letter, until the browser really measures.
const GUESS: f32 = 0.42;

/// Estimated width of the widest number — three letters — that the layout
/// starts from. `build_frame` rebuilds it as soon as the page has measured the
/// real one.
const ESTIMATED_NUMERAL: f32 = 3.0 * GUESS;

// ------------------------------------------------------------------ output

/// One record of the command list, in 4-byte words.
const STRIDE: usize = 10;

/// There is **no fill left** in the rendering: only text and clipping commands
/// come out of here. The world reduced to a white square went away with the
/// zoom-out floor and the early fold.
const KIND_TEXT: f32 = 1.0;
/// Opens then closes a clip. A chapter's text must never bite into its
/// neighbours, even if a badly measured width makes it overflow.
const KIND_CLIP: f32 = 2.0;
const KIND_UNCLIP: f32 = 3.0;

/// Ink of a text command, last slot of the record.
///
/// **The one colour of the map, and it carries information.** A run marked
/// `INK_OTHER` is the other publication's reading, inserted after ours; the
/// page paints it `yellow-200` and everything else a shade of white. The
/// intensity of the white was tried first and set aside: it read as a faded
/// text rather than as another voice. Which colour is the page's business —
/// the module only says which run is which.
const INK_OURS: f32 = 0.0;
const INK_OTHER: f32 = 1.0;

// ------------------------------------------------------------------ camera

#[derive(Clone, Copy)]
struct Camera {
    z: f32,
    x: f32,
    y: f32,
}

impl Camera {
    fn project(&self, r: Rect) -> Rect {
        Rect { x: r.x * self.z + self.x, y: r.y * self.z + self.y, w: r.w * self.z, h: r.h * self.z }
    }

    fn to_world(&self, px: f32, py: f32) -> (f32, f32) {
        ((px - self.x) / self.z, (py - self.y) / self.z)
    }

    /// Re-roots the camera in `cell`, which contains a copy of the world.
    /// The image does not move; only the scale comes back down, which keeps the
    /// floats from losing their precision.
    fn wrap_in(&mut self, cell: Rect) {
        let z = self.z;
        let z2 = cell.w * z / WORLD.w;
        self.x = cell.x * z + self.x;
        self.y = cell.y * z + self.y + (cell.h * z - WORLD.h * z2) / 2.0;
        self.z = z2;
    }
}

// ------------------------------------------------------------------- state

/// One laid-out line: a slice of verse, its box and its character size. The
/// unit of rendering is not the verse but the line — a long verse takes
/// several.
#[derive(Clone, Copy)]
struct Line {
    rect: Rect,
    text: &'static str,
    verse: usize,
    size: f32,
}

struct App {
    layout: Layout,
    camera: Camera,
    width: f32,
    height: f32,
    /// Number of Tanakhs crossed. The descent is one-way: once the new world
    /// has appeared, the old one no longer exists.
    level: u32,
    /// Width of a fragment for a font of size 1, measured by the page.
    ratios: HashMap<(u32, u32), f32>,
    /// Fragments whose width the page has yet to give.
    pending: Vec<u32>,
    commands: Vec<f32>,
    /// Diagnostic: nests kept at the last frame.
    last_nests: u32,
    /// Runs of the other edition's reading drawn at the last frame. The page
    /// shows the index at the bottom only when there is one on screen.
    variant_runs: u32,
    /// One space from the corpus, to measure its width once and for all.
    space: Option<&'static str>,
    /// Layout of the chapters already visited. Invalidated as soon as a
    /// measurement arrives from the page.
    lines: HashMap<usize, Vec<Line>>,
    /// Size of chapter numbers, in world units. Computed once, thrown away as
    /// soon as a measurement arrives from the page.
    number: Option<f32>,
    /// Width of the widest number, as the current layout assumed it. A
    /// measurement that contradicts it rebuilds the layouts.
    numeral: f32,
    /// The other level of detail, ready to take the current one's place.
    spare: Layout,
    /// True when the current layout is the one for chapters.
    detail: bool,
    /// Minimum framing, cached: see `base_scale`.
    base: f32,
    /// Selection: book, chapter, verse, word — the last two as (offset, length).
    selection: [u32; 6],
    /// Breadcrumb: section, book, chapter and verse, each an (offset, length).
    focus: [u32; 8],
    /// A mouse has left the canvas. The centre of the view then names nothing
    /// until the pointer comes back — see `leave`.
    mouse_away: bool,
}

static mut APP: Option<App> = None;

#[allow(static_mut_refs)]
fn app() -> &'static mut App {
    unsafe { APP.as_mut().expect("init() n'a pas ete appele") }
}

impl App {
    /// Fresh state for a window, framed on the whole world.
    ///
    /// `init` only wraps it in the global slot: the computation itself holds no
    /// global state, which is what lets a test drive a window of its own.
    fn new(width: f32, height: f32) -> App {
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
            space: None,
            lines: HashMap::new(),
            number: None,
            numeral: ESTIMATED_NUMERAL,
            selection: [0; 6],
            focus: [0; 8],
            mouse_away: false,
        };
        app.base = app.base_scale();
        app.fit();
        app
    }

    /// Scale of the initial framing.
    ///
    /// It is the whole world in the window — unless that made the chapters
    /// illegible at the first notch. Below `base`, we prefer to **crop the map**
    /// rather than push the levels back: on a phone, insisting on showing all
    /// 1 600 units of width at once gave a framing three times smaller than on a
    /// laptop, hence numbers at the fourth notch instead of the first, and book
    /// titles of 5 px.
    ///
    /// The price is known: on a narrow window, the starting view no longer shows
    /// the full width of the map. One gets it back by zooming out one notch —
    /// the three portions — and panning.
    fn fit_scale(&self) -> f32 {
        let contain = (self.width / WORLD.w).min(self.height / WORLD.h);
        contain.max(self.base)
    }

    /// Minimum framing: the one that puts a chapter number just above its floor
    /// at the first zoom notch.
    ///
    /// Cached, because `fit_scale` is called everywhere and cannot borrow `self`
    /// mutably; recomputed when the layout changes.
    fn base_scale(&mut self) -> f32 {
        let widest = self.widest_numeral();
        let detail = if self.detail { &self.layout } else { &self.spare };
        // One per cent of margin: on a very flat window, the computed step drops
        // below the nominal step, and the number missed its floor by three
        // hundredths of a pixel at the first notch.
        MIN_NUMBER * 1.05 / (number_in(detail, widest) * ZOOM_STEP)
    }

    /// Factor of one wheel notch.
    ///
    /// **Computed, and not fixed at 1.4**: it is worth whatever it takes for the
    /// dive from the framing down to a full-screen chapter to always take
    /// `DIVE_NOTCHES` notches, whatever the window. On a landscape screen the
    /// framing is already proportional to the height, so the computation yields
    /// exactly 1.40; it is on narrow windows, where the framing has been
    /// floored, that it departs a little.
    fn zoom_step(&mut self) -> f32 {
        let cell = {
            let detail = if self.detail { &self.layout } else { &self.spare };
            detail.chapters[0].h.max(0.01)
        };
        let fit = self.fit_scale();
        let target = (self.height / cell).max(fit * 1.01);
        (target / fit).powf(1.0 / DIVE_NOTCHES)
    }

    /// Minimum zoom, **technical and not visible**.
    ///
    /// Zooming out no longer has a legibility bound: past the three-portions
    /// state, their size no longer depends on the zoom but on the screen, so
    /// carrying on zooming out does not shrink them. All that remains is to keep
    /// the scale from falling to zero, where world coordinates would lose all
    /// meaning.
    ///
    /// A legibility bound was tried — stopping the zoom-out as soon as the three
    /// titles reached `MIN_TEXT`: it froze the view on three words of 4 px,
    /// illegible, instead of letting them grow.
    fn floor_scale(&self) -> f32 {
        1.0 / WORLD.w
    }

    /// Current level of detail: are the chapters on screen?
    ///
    /// The two levels do not have the same layout — the portions' air is halved
    /// there — hence neither the same boxes nor the same pitch. We keep both side
    /// by side and swap them when the threshold is crossed; nothing is
    /// recomputed, and the verse layout, which depends on the cells, is thrown
    /// away.
    fn set_detail(&mut self, detail: bool) {
        if detail == self.detail {
            return;
        }
        std::mem::swap(&mut self.layout, &mut self.spare);
        self.detail = detail;
        self.lines.clear();
        self.number = None;
    }

    fn sync_detail(&mut self) {
        let camera = self.camera;
        let detail = self.chapters_ready(camera);
        self.set_detail(detail);
    }

    /// Scale the map opens on: **one notch below the framing**.
    ///
    /// The three levels are one notch apart, and the map starts at the top of
    /// them: the three portions alone, then the books, then the chapters. One
    /// reads the Tanakh in three parts before reading it in thirty-nine books,
    /// and the opening image says so rather than showing everything at once.
    /// Price: the dive to the text takes one notch more than `DIVE_NOTCHES`.
    /// Accepted — it buys the first thing the map has to say.
    ///
    /// It is the framing divided by the notch, never a scale of its own: the
    /// thresholds of the three levels all hang off `fit_scale`, and a second
    /// reference would let them drift apart.
    fn overview_scale(&mut self) -> f32 {
        self.fit_scale() / self.zoom_step()
    }

    /// Whether the camera still sits on the scale the map opens on.
    ///
    /// Two places ask it — a resize, and a numeral that rebuilds the layout —
    /// and they must ask it the same way: a view believed to be at the opening
    /// in one and not in the other would be refitted by one and left behind by
    /// the other.
    fn at_opening(&mut self) -> bool {
        let opening = self.overview_scale();
        (self.camera.z - opening).abs() < opening * 1e-3
    }

    fn fit(&mut self) {
        self.camera.z = self.overview_scale();
        self.camera.x = (self.width - WORLD.w * self.camera.z) / 2.0;
        self.camera.y = (self.height - WORLD.h * self.camera.z) / 2.0;
    }

    /// Width of the fragment for a font of size 1.
    ///
    /// As long as the page has not measured it, we answer an estimate and record
    /// the fragment in `pending`: the next frame will be right.
    fn ratio(&mut self, fragment: &'static str) -> f32 {
        let key = key_of(fragment);
        if let Some(&ratio) = self.ratios.get(&key) {
            return ratio;
        }
        if self.pending.len() < 4096 {
            self.pending.push(key.0);
            self.pending.push(key.1);
        }
        hier::letter_count(fragment).max(1) as f32 * GUESS
    }

    /// Width of a space, measured once on a real space from the corpus.
    fn space_ratio(&mut self) -> f32 {
        let space = *self.space.get_or_insert_with(|| {
            let text = corpus::text();
            let at = text.find(' ').unwrap_or(0);
            &text[at..at + 1]
        });
        self.ratio(space)
    }

    /// Width of a fragment, **deduced from its words** and not measured as
    /// such.
    ///
    /// That is what makes the layout stable: measuring the whole line would make
    /// the fragments to measure depend on the wrapping, which depends on the
    /// measurements. Every new layout would call for new measurements, which
    /// would invalidate the layout — endlessly. Words, for their part, never
    /// change.
    fn width_of(&mut self, text: &'static str, size: f32) -> f32 {
        let space = self.space_ratio();
        let mut total = 0.0;
        let mut count = 0usize;

        for word in text.split(' ') {
            total += self.ratio(word);
            count += 1;
        }

        (total + count.saturating_sub(1) as f32 * space) * size
    }

    fn chapter_at(&self, x: f32, y: f32) -> Option<usize> {
        self.layout.chapters.iter().position(|r| r.contains(x, y))
    }

    /// Layout of a chapter, in world coordinates.
    ///
    /// **All verses have the same character size.** A verse that is too long
    /// goes over several lines instead of being compressed: only the number of
    /// lines varies, never the size. A breathing space separates the verses, so
    /// that one sees where one ends and the next begins.
    ///
    /// Computed once per chapter, then cached: the layout depends only on the
    /// world, not on the zoom.
    fn chapter_lines(&mut self, chapter: usize) -> Vec<Line> {
        if let Some(lines) = self.lines.get(&chapter) {
            return lines.clone();
        }
        let laid = self.lay_out(chapter);
        self.lines.insert(chapter, laid.clone());
        laid
    }

    fn lay_out(&mut self, chapter: usize) -> Vec<Line> {
        let cell = column(self.layout.chapters[chapter]);
        let verses = corpus::verses_of(chapter);
        let count = verses.len().max(1);

        // We start from one line per verse, then tighten as long as the column
        // overflows. The number of lines can only grow as the size grows, so a
        // few passes are enough. The header row and the verse-number gutter
        // scale with the size, so they enter every pass.
        let mut size = cell.h / (count as f32 * LINE_K + (count - 1) as f32 * GAP_K + CHAPTER_MARK * LINE_K);
        for _ in 0..5 {
            let width = cell.w - self.gutter(chapter, size);
            let total = CHAPTER_MARK * LINE_K * size + self.column_height(chapter, width, size);
            if total <= cell.h {
                break;
            }
            size *= cell.h / total;
        }

        let gutter = self.gutter(chapter, size);
        let width = cell.w - gutter;
        let mut out = Vec::new();
        let mut y = cell.y + CHAPTER_MARK * LINE_K * size;

        for verse in verses {
            let text = drawn(verse);
            for slice in self.wrap(text, width, size) {
                out.push(Line {
                    rect: Rect { x: cell.x, y, w: width, h: size * LINE_K },
                    text: slice,
                    verse,
                    size,
                });
                y += size * LINE_K;
            }
            y += size * GAP_K;
        }

        out
    }

    /// Width of the verse-number gutter of a chapter, at a character size: its
    /// widest number, plus one space between it and the text.
    fn gutter(&mut self, chapter: usize, size: f32) -> f32 {
        let widest = corpus::verses_of(chapter)
            .map(|v| self.ratio(corpus::verse_label(v)))
            .fold(0.0_f32, f32::max);
        (widest * VERSE_MARK + self.space_ratio()) * size
    }

    /// Height the chapter's verses would take at this character size.
    fn column_height(&mut self, chapter: usize, width: f32, size: f32) -> f32 {
        let verses = corpus::verses_of(chapter);
        let count = verses.len().max(1);
        let mut lines = 0usize;

        for verse in verses {
            let drawn = self.width_of(drawn(verse), size);
            let needed = if width > 0.0 { (drawn / width).ceil() as usize } else { 1 };
            lines += needed.max(1);
        }

        lines as f32 * size * LINE_K + (count - 1) as f32 * size * GAP_K
    }

    /// Splits a verse into lines that fit in `width`, at word boundaries.
    ///
    /// Every line stays a slice of the corpus: only an offset and a length cross
    /// the boundary with the page.
    fn wrap(&mut self, verse: &'static str, width: f32, size: f32) -> Vec<&'static str> {
        if width <= 0.0 || size <= 0.0 || self.width_of(verse, size) <= width {
            return vec![verse];
        }

        let words: Vec<&'static str> = verse.split(' ').collect();
        let space = self.space_ratio();

        let mut out = Vec::new();
        let mut start = 0usize; // start of the current line, in bytes
        let mut at = 0usize; // start of the current word
        let mut run = 0.0f32;

        for word in &words {
            let w = self.ratio(word) * size;
            let first = at == start;
            let advance = if first { w } else { w + space * size };

            if !first && run + advance > width {
                out.push(&verse[start..at - 1]); // without the trailing space
                start = at;
                run = w;
            } else {
                run += advance;
            }
            at += word.len() + 1;
        }

        out.push(&verse[start..]);
        out
    }

    /// Cells of a line's letters, right to left.
    ///
    /// The text is drawn in one piece; this split only serves to locate a letter
    /// — for the nest it contains, and for clicks.
    fn letter_cells(&mut self, text: &'static str, row: Rect, size: f32) -> Vec<Rect> {
        let drawn = self.width_of(text, size);
        if drawn <= 0.0 {
            return Vec::new();
        }

        let parts: Vec<&'static str> = text.split(' ').flat_map(hier::letters).collect();
        let total: f32 = parts.iter().map(|p| self.ratio(p)).sum();
        if total <= 0.0 {
            return Vec::new();
        }

        let mut right = row.x + row.w;
        parts
            .iter()
            .map(|part| {
                let w = drawn * self.ratio(part) / total;
                right -= w;
                Rect { x: right, w, ..row }
            })
            .collect()
    }

    /// Cells of a line's words, right to left.
    ///
    /// Same split as `letter_cells`, one step coarser: it is what a double
    /// click reads. The spaces are counted — they are what separates two words
    /// on screen — so a word's box holds its ink and nothing else, and the gaps
    /// between boxes are the spaces.
    fn word_cells(&mut self, text: &'static str, row: Rect, size: f32) -> Vec<(Rect, &'static str)> {
        let drawn = self.width_of(text, size);
        if text.is_empty() || drawn <= 0.0 {
            return Vec::new();
        }

        let words: Vec<&'static str> = text.split(' ').collect();
        let space = self.space_ratio();
        let ink: f32 = words.iter().map(|w| self.ratio(w)).sum();
        let total = ink + words.len().saturating_sub(1) as f32 * space;
        if total <= 0.0 {
            return Vec::new();
        }

        // The drawn width is the measured one; the ratios only share it out.
        let scale = drawn / total;
        let mut right = row.x + row.w;
        let mut out = Vec::with_capacity(words.len());
        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                right -= space * scale;
            }
            let w = self.ratio(word) * scale;
            right -= w;
            out.push((Rect { x: right, w, ..row }, *word));
        }
        out
    }

    /// Line of a chapter nearest to a point, vertically.
    ///
    /// Nearest and not containing: the breathing spaces between verses leave
    /// gaps where the cursor often falls, and a strict search would find
    /// nothing there.
    fn line_at(&mut self, chapter: usize, y: f32) -> Option<Line> {
        self.chapter_lines(chapter)
            .into_iter()
            .min_by(|a, b| vertical_gap(a.rect, y).total_cmp(&vertical_gap(b.rect, y)))
    }

    /// Is a chapter **read** at this zoom, or merely numbered?
    ///
    /// That is the whole condition for a click: one only selects a passage one
    /// can see. Above that zoom a click would name a verse chosen by the
    /// geometry alone, which the reader has no way of checking.
    ///
    /// It is the condition of the **drawing**, taken in the same order: the
    /// cheap estimate first — a line of at least `READABLE_LINE`, which costs
    /// no layout — then the laid-out size, which is what `draw_line` really
    /// compares to `MIN_TEXT`. The estimate alone was not enough: a chapter of
    /// long verses wraps, hence lays out smaller than the estimate, and its
    /// cell fell back on its number while a click still believed the text was
    /// there.
    fn readable(&mut self, chapter: usize) -> bool {
        if self.line_height(chapter, self.camera) < READABLE_LINE {
            return false;
        }
        let z = self.camera.z;
        // All the lines of a chapter share one size.
        self.chapter_lines(chapter).first().is_some_and(|line| line.size * z >= MIN_TEXT)
    }

    /// Chapter one can click on, under a point of the world.
    ///
    /// The same tolerance as `locate` — a book box, never the air between two
    /// portions — plus the reading condition.
    fn clickable_at(&mut self, x: f32, y: f32) -> Option<usize> {
        let (_, chapter, _, _) = self.locate(x, y)?;
        self.readable(chapter).then_some(chapter)
    }

    /// What a click selects: book, chapter, verse, and the word if one is
    /// asked for. Each level is an (offset, length) in the corpus.
    ///
    /// Returns nothing above the reading zoom: **a click is only live on a
    /// passage one can read.**
    fn pick(&mut self, x: f32, y: f32, word: bool) -> Option<[u32; 6]> {
        let chapter = self.clickable_at(x, y)?;
        let book = (0..corpus::book_count()).find(|&b| corpus::chapters_of(b).contains(&chapter))?;
        let line = self.line_at(chapter, y)?;

        let mut out = [book as u32, chapter as u32, 0, 0, 0, 0];
        let (offset, len) = key_of(corpus::verse_text(line.verse));
        out[2] = offset;
        out[3] = len;

        if word {
            // Same tolerance horizontally as everywhere else: the text is set
            // to the right, so the left of the strip is often empty.
            let hit = self
                .word_cells(line.text, line.rect, line.size)
                .into_iter()
                .min_by(|(a, _), (b, _)| horizontal_gap(*a, x).total_cmp(&horizontal_gap(*b, x)));
            if let Some((_, word)) = hit {
                let (offset, len) = key_of(word);
                // The navbar names the passage, so it names our text. In a
                // verse carrying another reading, a word of ours is given back
                // without the mark that numbers it, and a word of the other
                // edition names nothing: it is not in the passage.
                match variant::display(line.verse) {
                    None => {
                        out[4] = offset;
                        out[5] = len;
                    }
                    Some(shown) => {
                        let rel = offset as usize - corpus::offset_of(shown);
                        if let Some((at, kept)) = variant::corpus_span(line.verse, rel, len as usize) {
                            let verse = corpus::offset_of(corpus::verse_text(line.verse));
                            out[4] = (verse + at) as u32;
                            out[5] = kept as u32;
                        }
                    }
                }
            }
        }

        Some(out)
    }

    /// Box the number of a chapter really occupies, in the world.
    ///
    /// The same geometry `draw_book` uses: centred in its cell, as wide as its
    /// digits and as tall as its body. A cell is far wider than its number —
    /// the stride is the one of the densest book — so the rest of the box is
    /// void.
    fn numeral_box(&mut self, chapter: usize) -> Rect {
        let cell = self.layout.chapters[chapter];
        let size = {
            let widest = self.widest_numeral();
            let detail = if self.detail { &self.layout } else { &self.spare };
            number_in(detail, widest)
        };
        let ink = self.ratio(corpus::chapter_label(chapter)) * size;
        Rect {
            x: cell.x + (cell.w - ink) / 2.0,
            y: cell.y + (cell.h - size) / 2.0,
            w: ink,
            h: size,
        }
    }

    /// Whether books are drawn at all at this zoom.
    ///
    /// Below the framing the map is the three portions and nothing else, so no
    /// book can be named there — not even by the view centre, which would
    /// otherwise announce a box that is not on screen.
    fn books_drawn(&mut self, camera: Camera) -> bool {
        camera.z >= self.fit_scale() * PORTIONS_UNDER
    }

    /// Ink of a fragment laid by `emit_text` in `area` at `size`.
    ///
    /// Flush right, as everything is here, and no taller than its line: a box
    /// taller than the glyphs would be named where nothing is drawn.
    fn ink_of(&mut self, content: &'static str, area: Rect, size: f32) -> Rect {
        let w = self.width_of(content, size);
        let line = (size * 1.35).min(area.h);
        Rect { x: area.x + area.w - w, y: area.y + area.h / 2.0 - line / 2.0, w, h: line }
    }

    /// Where the name of a portion really sits on screen, or `None` when it is
    /// not drawn.
    ///
    /// Two geometries, the drawing's own: below the framing the three names are
    /// cards spread over the height of the projected world, sharing one right
    /// edge; above it, each sits at the bottom of its band, clamped to the
    /// window since a portion is several screens wide.
    fn section_ink(&mut self, section: usize, camera: Camera) -> Option<Rect> {
        let name = corpus::section_name(section);

        if !self.books_drawn(camera) {
            let widest = (0..3)
                .map(|s| self.ratio(corpus::section_name(s)))
                .fold(0.3_f32, f32::max);
            let world = camera.project(WORLD);
            let size = (world.h * SECTION_CARD / 3.0).min(world.w * 0.8 / widest);
            if size < MIN_TEXT {
                return None;
            }
            let line = size * 1.35;
            let gap = world.h * CARD_GAP;
            let middle = world.y + world.h / 2.0;
            let right = (world.w + widest * size) / 2.0;
            let y = middle + (section as f32 - 1.0) * gap;
            let area = Rect { x: world.x, y: y - line / 2.0, w: right, h: line };
            return Some(self.ink_of(name, area, size));
        }

        let band = camera.project(self.layout.sections[section]);
        if !self.visible(band) {
            return None;
        }
        let size = self.scale_of(camera).section;
        if size < MIN_TEXT {
            return None;
        }
        let strip = camera.project(self.layout.section_bands[section]);
        let pinned = Rect {
            x: band.x.max(0.0),
            w: (band.x + band.w).min(self.width) - band.x.max(0.0),
            ..band
        };
        let line = size * 1.35;
        let area = Rect { x: pinned.x, y: strip.y + strip.h - line, w: pinned.w, h: line };
        Some(self.ink_of(name, area, size))
    }

    /// Where the name of a book really sits on screen, or `None` when it is not
    /// drawn.
    ///
    /// The drawing's geometry again: it fills its box while the numbers are
    /// hidden, and falls back to the bottom of its band once they are out.
    fn book_ink(&mut self, book: usize, camera: Camera) -> Option<Rect> {
        if !self.books_drawn(camera) {
            return None;
        }
        let rect = camera.project(self.layout.books[book]);
        if !self.visible(rect) || rect.w < 0.7 {
            return None;
        }
        let scale = self.scale_of(camera);
        if scale.book < MIN_TEXT {
            return None;
        }
        let area = if scale.show_chapters {
            let strip = camera.project(self.layout.book_bands[book]);
            let line = scale.book * 1.35;
            Rect { x: rect.x, y: strip.y + strip.h - line, w: rect.w, h: line }
        } else {
            rect
        };
        Some(self.ink_of(corpus::book_name(book), area, scale.book))
    }

    /// Whether the pointer is really **on** a chapter, and not merely in its
    /// box.
    ///
    /// **A chapter showing a number is named only when the pointer is on that
    /// number.** The number takes a fraction of its cell and the rest is void:
    /// answering for the whole box made the breadcrumb and the index name a
    /// letter the pointer was nowhere near, several cells away.
    ///
    /// Where the text is drawn the rule stays the click's, which the cursor
    /// must not contradict — and there the lines fill the column, so being in
    /// the box is being on the text.
    fn names_chapter(&mut self, chapter: usize, x: f32, y: f32) -> bool {
        if self.readable(chapter) {
            return true;
        }
        let ink = self.numeral_box(chapter);
        x >= ink.x && x <= ink.x + ink.w && y >= ink.y && y <= ink.y + ink.h
    }

    /// Whether the chapter under the pointer holds another reading.
    ///
    /// **The chapter, and nothing above it.** A portion or a book would answer
    /// yes nearly everywhere — a reading sits in almost every book — and an
    /// index that is always lit says nothing. At the chapter, the answer tells
    /// the reader something he can act on: there is a difference *here*. Above
    /// that level, and outside every box, it says nothing, like the breadcrumb
    /// and the click.
    fn readings_at(&mut self, x: f32, y: f32) -> bool {
        let camera = self.camera;
        if !self.chapters_ready(camera) {
            return false;
        }
        match self.locate(x, y) {
            Some((_, chapter, _, _)) => {
                self.names_chapter(chapter, x, y) && variant::in_verses(corpus::verses_of(chapter))
            }
            None => false,
        }
    }

    /// Chapter nearest to a point, even if that point falls in a gap between two
    /// boxes.
    fn nearest_chapter(&self, x: f32, y: f32) -> Option<usize> {
        self.layout
            .chapters
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| gap_to(**a, x, y).total_cmp(&gap_to(**b, x, y)))
            .map(|(i, _)| i)
    }

    /// Book whose box contains a point.
    fn book_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..corpus::book_count()).find(|&b| self.layout.books[b].contains(x, y))
    }


    /// Height of a line of this chapter, on screen, without laying the chapter
    /// out: 929 chapters per frame would be as many vectors thrown away.
    ///
    /// It is the one measure that says whether a chapter is **read** or merely
    /// numbered, so the drawing and the breadcrumb both take it from here.
    fn line_height(&self, chapter: usize, camera: Camera) -> f32 {
        let count = corpus::verses_of(chapter).len().max(1);
        column(self.layout.chapters[chapter]).h / count as f32 * camera.z
    }

    /// Path to show in the breadcrumb, for a point of the world: section, book,
    /// chapter, verse, each as an (offset, length) in the corpus.
    ///
    /// **Depth follows the zoom**, and follows it on what the frame really
    /// draws: the chapter appears with the numbers (`chapters_ready`), the
    /// verse with the text (`READABLE_LINE`). The breadcrumb says where one is,
    /// never more than what the map shows.
    ///
    /// `strict` is what tells a pointer from the centre of the view. Under the
    /// pointer, only a box names something — a book box, or the title band of a
    /// portion; pointing at the air between two portions names nothing, exactly
    /// as a click there selects nothing. At the centre of the view, the nearest
    /// chapter is taken, so that the breadcrumb never goes blank while panning.
    fn path_at(&mut self, x: f32, y: f32, strict: bool) -> Option<[u32; 8]> {
        let camera = self.camera;
        let mut out = [0u32; 8];
        let put = |slot: &mut [u32], text: &'static str| {
            let (offset, len) = key_of(text);
            slot[0] = offset;
            slot[1] = len;
        };
        let section_of = |book: usize| {
            treemap::SECTIONS
                .iter()
                .position(|&(_, first, last)| book >= first && book < last)
                .unwrap_or(0)
        };
        let book_of = |chapter: usize| {
            (0..corpus::book_count()).find(|&b| corpus::chapters_of(b).contains(&chapter))
        };

        // **The pointer names what it is on, level by level, and only what is
        // drawn.** Deepest first: a number or a line of text, then a book's
        // name, then a portion's. Two names never overlap — a band is reserved
        // for its title — so the order settles it. On nothing, nothing: the
        // caller then falls back to the centre of the view.
        if strict {
            if let Some((_, chapter, _, _)) = self.locate(x, y)
                && self.chapters_ready(camera)
                && self.names_chapter(chapter, x, y)
                && let Some(book) = book_of(chapter)
            {
                put(&mut out[0..2], corpus::section_name(section_of(book)));
                put(&mut out[2..4], corpus::book_name(book));
                put(&mut out[4..6], corpus::chapter_label(chapter));
                if self.readable(chapter)
                    && let Some((_, _, verse, _)) = self.locate_in(chapter, x, y)
                    && verse != usize::MAX
                {
                    put(&mut out[6..8], corpus::verse_label(verse));
                }
                return Some(out);
            }

            let (px, py) = (x * camera.z + camera.x, y * camera.z + camera.y);

            if let Some(book) = (0..corpus::book_count())
                .find(|&b| self.book_ink(b, camera).is_some_and(|ink| holds(ink, px, py)))
            {
                put(&mut out[0..2], corpus::section_name(section_of(book)));
                put(&mut out[2..4], corpus::book_name(book));
                return Some(out);
            }

            // A portion's name names its portion, and stops there: it belongs
            // to no book.
            let section = (0..3)
                .find(|&s| self.section_ink(s, camera).is_some_and(|ink| holds(ink, px, py)))?;
            put(&mut out[0..2], corpus::section_name(section));
            return Some(out);
        }

        // The centre of the view is not a pointer: it keeps the tolerance of
        // `locate` and answers wherever it falls. But it never goes deeper than
        // what the map draws, or it would name a book at a zoom where the map
        // is three words.
        let chapter = self.chapter_at(x, y).or_else(|| self.nearest_chapter(x, y))?;
        let book = book_of(chapter)?;
        put(&mut out[0..2], corpus::section_name(section_of(book)));

        if !self.books_drawn(camera) {
            return Some(out);
        }
        put(&mut out[2..4], corpus::book_name(book));

        if !self.chapters_ready(camera) {
            return Some(out);
        }
        put(&mut out[4..6], corpus::chapter_label(chapter));

        // Below this, the chapter is a number and not a text: there is no verse
        // to be on. It is the very condition of a click — the breadcrumb and
        // the cursor must agree on what is readable, or one would promise a
        // verse the other refuses.
        if !self.readable(chapter) {
            return Some(out);
        }
        if let Some((_, _, verse, _)) = self.locate_in(chapter, x, y)
            && verse != usize::MAX
        {
            put(&mut out[6..8], corpus::verse_label(verse));
        }
        Some(out)
    }

    /// Full path under a point of the world.
    ///
    /// If the point falls between two cells, we keep **the nearest one in the
    /// same book**. Cells are spaced out to take the full width of their box: in
    /// a book of three chapters, the gap exceeds a hundred world units, and a
    /// strict search returned nothing most of the time. Tolerance stops at the
    /// book — clicking in the air that separates two portions selects nothing.
    fn locate(&mut self, x: f32, y: f32) -> Option<(usize, usize, usize, Option<Rect>)> {
        let chapter = match self.chapter_at(x, y) {
            Some(chapter) => chapter,
            None => {
                let book = self.book_at(x, y)?;
                corpus::chapters_of(book)
                    .min_by(|&a, &b| {
                        gap_to(self.layout.chapters[a], x, y)
                            .total_cmp(&gap_to(self.layout.chapters[b], x, y))
                    })?
            }
        };
        self.locate_in(chapter, x, y)
    }

    fn locate_in(
        &mut self,
        chapter: usize,
        x: f32,
        y: f32,
    ) -> Option<(usize, usize, usize, Option<Rect>)> {
        let book = (0..corpus::book_count()).find(|&b| {
            let r = corpus::chapters_of(b);
            chapter >= r.start && chapter < r.end
        })?;

        let Some(line) = self.line_at(chapter, y) else {
            return Some((book, chapter, usize::MAX, None));
        };

        // Same tolerance horizontally: the text is set to the right, so the left
        // of the strip is often empty.
        let cell = self
            .letter_cells(line.text, line.rect, line.size)
            .into_iter()
            .min_by(|a, b| horizontal_gap(*a, x).total_cmp(&horizontal_gap(*b, x)));

        Some((book, chapter, line.verse, cell))
    }

    fn nest_at(&mut self, x: f32, y: f32) -> Option<Rect> {
        self.locate(x, y)?.3.map(|cell| cell.shrink(NEST))
    }

    /// Point aimed at by the zoom, with the letter containing it. Tolerates the
    /// gaps between boxes: aiming between two books must stay productive.
    fn target_near(&mut self, x: f32, y: f32) -> Option<(Rect, Rect)> {
        let chapter = self.chapter_at(x, y).or_else(|| self.nearest_chapter(x, y))?;
        let cell = self.locate_in(chapter, x, y)?.3?;
        Some((cell, cell.shrink(NEST)))
    }

    fn zoom_at(&mut self, px: f32, py: f32, factor: f32) {
        let (bx, by) = self.camera.to_world(px, py);
        self.camera.z *= factor;
        let (ax, ay) = self.camera.to_world(px, py);
        self.camera.x += (ax - bx) * self.camera.z;
        self.camera.y += (ay - by) * self.camera.z;

        // Descent. The nest sits at the centre of the letter, while the zoom
        // keeps the cursor still: without help, the nest drifts off screen and
        // becomes impossible to reach. So we gently pull the view towards it as
        // soon as the letter has been passed. The gesture stays free — the
        // correction is weak and only applies on approach.
        if factor > 1.0 {
            let (wx, wy) = self.camera.to_world(px, py);

            // As long as a chapter does not fill the screen, it is **it** that
            // the view follows. The backgrounds being black, the void between
            // cells is in no way distinguishable from the rest: without this
            // pull, zooming off-target led to a black screen with no way out.
            let aimed = self
                .chapter_at(wx, wy)
                .or_else(|| self.nearest_chapter(wx, wy))
                .map(|c| self.camera.project(self.layout.chapters[c]));

            if let Some(cell) = aimed
                && cell.w < self.width
            {
                let k = pull(factor);
                self.camera.x += (px - (cell.x + cell.w / 2.0)) * k;
                self.camera.y += (py - (cell.y + cell.h / 2.0)) * k;
            }

            let (wx, wy) = self.camera.to_world(px, py);
            if let Some((letter, nest)) = self.target_near(wx, wy) {
                let cell = self.camera.project(letter);
                let screen = self.camera.project(nest);

                if cell.w > self.width * NEST_AFTER {
                    // The letter has been passed: the new Tanakh appears, and
                    // the old one disappears at the same instant.
                    //
                    // We re-root **here**, and not when the nest covers the
                    // screen. Waiting cost some thirty notches spent handling
                    // coordinates of the order of 1e8 in f32: the nest, a few
                    // pixels wide, then drifted further and further from the
                    // centre (up to 800 px measured). Once the camera is
                    // returned to the nest's world, every number becomes small
                    // again and the framing exact.
                    self.camera.wrap_in(nest);
                    self.level += 1;
                } else if screen.w > 2.0 {
                    // Before that, a gentle pull towards the cursor: the nest is
                    // at the centre of its letter and would otherwise drift off
                    // screen.
                    let closeness = (cell.w / self.width).clamp(0.0, 1.0);
                    let k = (0.14 + 0.62 * closeness).max(pull(factor));
                    self.camera.x += (px - (screen.x + screen.w / 2.0)) * k;
                    self.camera.y += (py - (screen.y + screen.h / 2.0)) * k;
                }
            }
        }

        // Floor: we stop at the last zoom where the three portion titles are
        // still legible.
        self.camera.z = self.camera.z.max(self.floor_scale());

        // Below the framing, we re-centre: the view converges towards a single
        // point instead of drifting out of frame.
        if self.camera.z < self.fit_scale() {
            self.camera.x = (self.width - WORLD.w * self.camera.z) / 2.0;
            self.camera.y = (self.height - WORLD.h * self.camera.z) / 2.0;
        }
    }

    // --------------------------------------------------------------- drawing

    fn visible(&self, r: Rect) -> bool {
        r.x < self.width && r.y < self.height && r.x + r.w > 0.0 && r.y + r.h > 0.0
    }

    /// Restricts drawing to a rectangle, until `unclip`.
    fn clip(&mut self, r: Rect) {
        self.commands
            .extend_from_slice(&[KIND_CLIP, r.x, r.y, r.w, r.h, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    fn unclip(&mut self) {
        self.commands
            .extend_from_slice(&[KIND_UNCLIP, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    /// Title of a book or a section, laid **inside its band**.
    ///
    /// It is never pinned over the content: the band is reserved for it, so it
    /// covers nothing. When the band leaves the screen, the breadcrumb takes
    /// over — the information is never lost, and never superimposed.
    ///
    /// It stays clipped to its box: too long for a small book, it is trimmed
    /// rather than biting into its neighbour.
    fn title(
        &mut self,
        content: &'static str,
        box_: Rect,
        band: Rect,
        size: f32,
        full: bool,
        alpha: f32,
    ) {
        if alpha <= 0.01 || size < MIN_TEXT || !self.visible(band) {
            return;
        }

        // **The right edge is the box's, not the screen's.** A book title reads
        // just after the last cell of its chapter row: that is its place, and it
        // must not depend on the framing. Clamping it to the screen made the
        // title slide inside its box during a pan — one dragged a book to the
        // right and its name went off to the left.
        //
        // Portion titles, for their part, are pinned to the screen by their
        // caller: a portion measures several screens in width, and its name must
        // stay legible as long as one is inside it.
        let line = size * 1.35;
        let left = box_.x;
        let right = box_.x + box_.w;
        if right - left < 2.0 || right < 0.0 || left > self.width {
            return;
        }

        // Set at the bottom of the band, hence just above the first chapter: the
        // space between them keeps the same look at every zoom.
        //
        // Except when the box is free — chapter numbers still hidden: the title
        // then centres in it, instead of overflowing its band upwards and biting
        // into the section title.
        let area = if full {
            Rect { x: left, y: box_.y, w: right - left, h: box_.h }
        } else {
            Rect { x: left, y: band.y + band.h - line, w: right - left, h: line }
        };

        // Clipped **in width only**. A small book's band is lower than the text
        // line; clipping in height as well cut the tops off the letters and gave
        // the illusion of different sizes, when all titles share the same one.
        // The overflow goes upwards, into the gutter between books, never onto
        // the chapters: the title is anchored at the bottom of its band.
        self.clip(Rect { x: box_.x, y: 0.0, w: box_.w, h: self.height });
        self.emit_text(content, area, size, alpha);
        self.unclip();
    }

    /// Lays a fragment, set on the right edge of its box, at an imposed size.
    /// That is what lets all the verses of a chapter share the same size
    /// whatever their length.
    /// The legibility floor is the caller's choice: a title and a line of verse
    /// stop at `MIN_TEXT`, a chapter number at `MIN_NUMBER`.
    fn emit_text(&mut self, content: &'static str, area: Rect, size: f32, alpha: f32) {
        self.emit_ink(content, area, size, alpha, INK_OURS);
    }

    /// Same, naming the ink. Only the other edition's reading uses anything but
    /// `INK_OURS`.
    fn emit_ink(&mut self, content: &'static str, area: Rect, size: f32, alpha: f32, ink: f32) {
        if alpha <= 0.01 {
            return;
        }
        if ink == INK_OTHER {
            self.variant_runs += 1;
        }
        let (offset, len) = key_of(content);
        self.commands.extend_from_slice(&[
            KIND_TEXT,
            area.x + area.w, // right edge: the text is laid on it
            area.y + area.h / 2.0 + size * 0.36,
            area.w,
            area.h,
            alpha,
            size,
            offset as f32,
            len as f32,
            ink,
        ]);
    }

    /// Lays one run of a line and gives back the right edge for the next one.
    ///
    /// A line is set flush right, so the runs are laid from the right: the
    /// first of the string is the rightmost on screen. Alignment stays here,
    /// never delegated — the engine is only asked for a width.
    fn emit_run(
        &mut self,
        content: &'static str,
        row: Rect,
        right: f32,
        size: f32,
        alpha: f32,
        ink: f32,
    ) -> f32 {
        let w = self.width_of(content, size);
        self.emit_ink(content, Rect { x: right - w, y: row.y, w, h: row.h }, size, alpha, ink);
        right - w
    }

    /// Width of the widest number of the Tanakh, for a font of size 1.
    fn widest_numeral(&mut self) -> f32 {
        (0..corpus::chapter_count())
            .map(|c| self.ratio(corpus::chapter_label(c)))
            .fold(0.3_f32, f32::max)
    }

    fn build_frame(&mut self) {
        self.commands.clear();
        self.pending.clear();

        // The layout assumes a digit width; if the page has measured another
        // one, we rebuild it with the real one. Once only, at the start:
        // measurements do not change afterwards. That is the price of the
        // number/title ratio being exactly a half in the browser, and not only
        // under the estimate.
        let numeral = self.widest_numeral();
        if (numeral - self.numeral).abs() > self.numeral * 0.005 {
            // The opening scale depends on the layout: if it moves, a camera
            // still sitting on it must follow. Without this, on a narrow
            // window, the first measured frame left the map a notch off the
            // level it opens on.
            let framed = self.at_opening();

            self.numeral = numeral;
            let (wide, tight) = levels(numeral);
            (self.layout, self.spare) =
                if self.detail { (tight, wide) } else { (wide, tight) };
            self.lines.clear();
            self.number = None;
            self.base = self.base_scale();

            if framed {
                self.fit();
            }
        }

        // The level of detail follows the camera: the portions' air tightens by
        // half as soon as the chapters appear.
        self.sync_detail();

        let camera = self.camera;

        // A single world is drawn, always. The handover to the next one is no
        // longer played out here: `zoom_at` re-roots the camera as soon as the
        // letter passes the screen, so that the current world *is* already the
        // new one. And there is no fallback nest any more: the fold happens on a
        // world already a thousand pixels wide, and the zoom-out stops while its
        // titles are legible. A white fill never said anything.
        self.variant_runs = 0;
        self.draw_world(camera, 1.0);
        self.last_nests = u32::from(self.commands.is_empty());
    }

    fn book_title_size(&mut self, camera: Camera, show_chapters: bool) -> f32 {
        if show_chapters {
            return self.uniform_title(camera);
        }
        (self.overview_title() * camera.z).min(self.height * TITLE_CEILING)
    }

    /// Size of a book title in the overview, in world units: the whole box
    /// belongs to it as long as the chapters are hidden.
    fn overview_title(&mut self) -> f32 {
        let widest = (0..corpus::book_count())
            .map(|book| self.ratio(corpus::book_name(book)))
            .fold(0.3_f32, f32::max);

        let box_ = self.layout.books[0];
        (box_.h * 0.6).min(box_.w * 0.88 / widest)
    }

    fn uniform_title(&mut self, camera: Camera) -> f32 {
        let mut bands: Vec<f32> = Vec::new();

        for book in 0..corpus::book_count() {
            let rect = camera.project(self.layout.books[book]);
            if self.visible(rect) && rect.w >= 2.0 {
                bands.push(camera.project(self.layout.book_bands[book]).h);
            }
        }

        if bands.is_empty() {
            return 0.0;
        }

        bands.sort_by(f32::total_cmp);
        let median = bands[bands.len() / 2];
        (median * TITLE_FILL).min(self.height * TITLE_CEILING)
    }

    /// Size of chapter numbers, in world units.
    ///
    /// **The same everywhere.** Every cell of the Tanakh shares a single pitch,
    /// hence a single size: a number does not change body depending on the book
    /// one is looking at. A size per book had been tried, to show the numbers one
    /// notch earlier; it gave ten different sizes on screen at the same time.
    ///
    /// **A number reads in full or not at all**: we take the widest of the
    /// Tanakh — three letters — and the largest size that makes it fit in a cell,
    /// width included. Overflow is therefore impossible by construction.
    ///
    /// Cached, thrown away as soon as a measurement arrives from the page.
    fn number_size(&mut self) -> f32 {
        if let Some(size) = self.number {
            return size;
        }
        let widest = self.widest_numeral();
        let size = number_in(&self.layout, widest);
        self.number = Some(size);
        size
    }

    /// Are the chapters **showable** at this zoom?
    ///
    /// Two conditions, and the second was learnt on a small screen: past the
    /// framing threshold, the numbers still have to be legible. On a phone, the
    /// first notch crossed the threshold, a book title fitted into its band —
    /// three pixels — and the numbers were a pixel and a half: the frame was
    /// **entirely black**. So we wait for the number to pass its floor, which
    /// happens at the first notch on a laptop and the fourth on a phone.
    ///
    /// The decision is always taken on the **chapter** layout, the one that will
    /// be in place if we switch: judging it on the current layout would make the
    /// level oscillate from one frame to the next.
    fn chapters_ready(&mut self, camera: Camera) -> bool {
        if camera.z <= self.fit_scale() * CHAPTER_FROM {
            return false;
        }
        let widest = self.widest_numeral();
        let detail = if self.detail { &self.layout } else { &self.spare };
        number_in(detail, widest) * camera.z >= MIN_NUMBER
    }

    /// Sizes of one level, deduced from **its** camera.
    ///
    /// They cannot be computed once for the frame: a nested world is drawn at a
    /// completely different scale, and taking over the sizes of the level above
    /// gave absurd titles as soon as the next Tanakh appeared, then worse and
    /// worse as one kept zooming.
    fn scale_of(&mut self, camera: Camera) -> Scale {
        let show_chapters = self.chapters_ready(camera);
        let book = self.book_title_size(camera, show_chapters);

        let number = self.number_size();

        Scale {
            show_chapters,
            chapter: number * camera.z,
            book,
            // No ceiling of its own: the book titles' ceiling already bounds
            // both, so the ratio stays exact at every zoom.
            section: book * SECTION_OVER_BOOK,
        }
    }

    fn draw_world(&mut self, camera: Camera, alpha: f32) {
        if alpha <= 0.004 {
            return;
        }

        let scale = self.scale_of(camera);

        // **The three portions alone, large, at equal distance.**
        //
        // **A single zoom-out notch is enough** (`PORTIONS_UNDER`). The three
        // levels are one notch from each other: three portions, then portions
        // and books at the framing, then the chapters. Waiting for the book
        // titles to drop below the text floor took five notches, during which
        // the map was nothing but a grid of ever paler words: nothing was
        // legible in it and yet one had to carry on.
        //
        // Two things change rule at this level.
        //
        // Their place, first: in the world, it comes from the volume of the
        // sections — one row of books for the Torah, five for the Prophets —
        // which puts them at very unequal distances. Once the books are gone,
        // that inequality says nothing any more; we spread them out in equal
        // parts.
        //
        // Their size, next: **it leaves the world for the screen.** Following
        // the zoom made them tiny at the very moment they became the only
        // content of the frame — three words of 4 px on an empty window. So they
        // take back the size their third of the screen allows, and keep it: one
        // can carry on zooming out without them shrinking, and without any fill
        // having to take their place.
        //
        // The level is recognised by the zoom alone, and on the right side of
        // the framing: deep inside a book, the title bands have also left the
        // screen, and judging on them made the three portions get painted giant
        // over the verses.
        if camera.z < self.fit_scale() * PORTIONS_UNDER {
            let widest = (0..3)
                .map(|section| self.ratio(corpus::section_name(section)))
                .fold(0.3_f32, f32::max);

            // Everything is taken on the **projected** world: the three words
            // are born large at the first zoom-out notch, then shrink and draw
            // closer with it, endlessly. Freezing them at a screen size was
            // tried: they read well, but zooming out no longer did anything at
            // all.
            let world = camera.project(WORLD);
            let size = (world.h * SECTION_CARD / 3.0).min(world.w * 0.8 / widest);
            let line = size * 1.35;
            let gap = world.h * CARD_GAP;
            let middle = world.y + world.h / 2.0;

            // **A single right edge for all three**, the one given by the widest
            // word: reading demands it, and three words each centred for
            // themselves went off in three directions. The block thus formed is
            // itself centred on the world.
            let right = (world.w + widest * size) / 2.0;

            for section in 0..3 {
                let y = middle + (section as f32 - 1.0) * gap;
                let area = Rect { x: world.x, y: y - line / 2.0, w: right, h: line };
                self.emit_text(corpus::section_name(section), area, size, alpha);
            }
            return;
        }

        for section in 0..3 {
            let band = camera.project(self.layout.sections[section]);
            if !self.visible(band) {
                continue;
            }

            // The section title first: it sets the floor below which book titles
            // will come and pin themselves, so that they never cover each
            // other.
            // **A portion's name is pinned to the screen**, and it is the only
            // one: a portion measures several screens in width once the chapters
            // are visible, and its name must stay legible as long as one is
            // inside it. So we clamp its box to the window before laying the
            // title, where a book title keeps the right edge of its box.
            let strip = camera.project(self.layout.section_bands[section]);
            let pinned = Rect {
                x: band.x.max(0.0),
                w: (band.x + band.w).min(self.width) - band.x.max(0.0),
                ..band
            };
            self.title(corpus::section_name(section), pinned, strip, scale.section, false, alpha);

            let (first, last) = (treemap::SECTIONS[section].1, treemap::SECTIONS[section].2);
            for book in first..last {
                self.draw_book(book, camera, scale, alpha);
            }
        }
    }

    fn draw_book(&mut self, book: usize, camera: Camera, scale: Scale, alpha: f32) {
        let rect = camera.project(self.layout.books[book]);
        if !self.visible(rect) || rect.w < 0.7 {
            return;
        }
        for chapter in corpus::chapters_of(book) {
            let cell = camera.project(self.layout.chapters[chapter]);
            if !self.visible(cell) || cell.w < 0.7 || cell.h < 0.7 {
                continue;
            }

            // Line height, estimated without materialising the layout: 929
            // chapters per frame would be as many vectors thrown away.
            let line = self.line_height(chapter, camera);

            // A cell **always** shows something: its text as soon as that is
            // legible, its number otherwise. Without this guarantee, a cell too
            // small for its text and too small for the label threshold stayed
            // empty, and one saw numbers appear here and not there.
            let mut drawn = false;

            if line >= READABLE_LINE {
                self.clip(cell);
                let before = self.commands.len();
                let lines = self.chapter_lines(chapter);
                let column = camera.project(column(self.layout.chapters[chapter]));
                for line in &lines {
                    let row = camera.project(line.rect);
                    if !self.visible(row) {
                        continue;
                    }
                    self.draw_line(*line, row, camera.z, alpha);
                    // The verse number, on its first line only, in the gutter
                    // the layout left at its right.
                    if corpus::offset_of(line.text) == corpus::offset_of(crate::drawn(line.verse)) {
                        let gutter = Rect { x: row.x + row.w, w: column.x + column.w - row.x - row.w, ..row };
                        self.mark(corpus::verse_label(line.verse), gutter, line.size * VERSE_MARK * camera.z, alpha);
                    }
                }

                // Text really has been laid: only then does the chapter number
                // take its corner — it belongs to the text, not to the cell.
                drawn = self.commands.len() > before;
                if drawn && let Some(first) = lines.first() {
                    let size = first.size * CHAPTER_MARK * camera.z;
                    let head = Rect { h: size * LINE_K, ..column };
                    self.mark(corpus::chapter_label(chapter), head, size, alpha);
                }
                self.unclip();
            }

            if !drawn && scale.show_chapters && scale.chapter >= MIN_NUMBER {
                // Centred in its cell: `emit_text` sets on the right edge, so we
                // give it a box that stops in the middle of the number.
                let label = corpus::chapter_label(chapter);
                let ink = self.ratio(label) * scale.chapter;
                let area = Rect { w: (cell.w + ink) / 2.0, ..cell };
                self.emit_text(label, area, scale.chapter, alpha * CHAPTER_INK);
            }
        }

        let strip = camera.project(self.layout.book_bands[book]);
        let full = !scale.show_chapters;
        self.title(corpus::book_name(book), rect, strip, scale.book, full, alpha * BOOK_INK);
    }

    /// A chapter or verse number beside the text, flush right in its area and
    /// as wide as its ink. A number is read as a shape, so its floor is
    /// `MIN_NUMBER`, and it wears the ink of chapter numbers.
    fn mark(&mut self, label: &'static str, area: Rect, size: f32, alpha: f32) {
        if size < MIN_NUMBER || !self.visible(area) {
            return;
        }
        let w = self.width_of(label, size);
        let area = Rect { x: area.x + area.w - w, w, ..area };
        self.emit_text(label, area, size, alpha * CHAPTER_INK);
    }

    /// One line of text, set on the right edge of its box.
    ///
    /// The size comes from the layout, not from the box: all the verses of a
    /// chapter share the same one, a long verse taking several lines rather than
    /// smaller characters.
    /// Draws a line, cut at the readings of the other edition.
    ///
    /// A verse carrying one is drawn from its display string, where the other
    /// form already takes its place in the flow — so nothing is ever painted
    /// over anything. The line is then cut into runs, ours and theirs, each
    /// with its own ink; a line without a variant stays one single command, as
    /// before.
    fn draw_line(&mut self, line: Line, row: Rect, scale: f32, alpha: f32) {
        let size = line.size * scale;
        if size < MIN_TEXT {
            return;
        }

        let Some(shown) = variant::display(line.verse) else {
            self.emit_text(line.text, row, size, alpha);
            return;
        };

        let start = corpus::offset_of(line.text) - corpus::offset_of(shown);
        let end = start + line.text.len();
        let mut right = row.x + row.w;
        let mut cursor = start;

        for insert in variant::inserts_of(line.verse) {
            let (at, len) = insert.other_range();
            let (from, to) = (at.max(start), (at + len).min(end));
            if to <= from {
                continue;
            }
            if from > cursor {
                right = self.emit_run(&shown[cursor..from], row, right, size, alpha, INK_OURS);
            }
            right = self.emit_run(&shown[from..to], row, right, size, alpha, INK_OTHER);
            cursor = to;
        }
        if cursor < end {
            self.emit_run(&shown[cursor..end], row, right, size, alpha, INK_OURS);
        }
    }

}

/// The two layouts: wide air for the overview, half as much as soon as the
/// chapters appear.
fn levels(numeral: f32) -> (Layout, Layout) {
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
fn number_in(layout: &Layout, widest: f32) -> f32 {
    let cell = layout.chapters[0];
    (layout.book_bands[0].h * treemap::CHAPTER_OVER_BOOK)
        .min(cell.h * treemap::NUMBER_HEIGHT)
        .min(cell.w * treemap::NUMBER_WIDTH / widest)
}

/// Strength of the pull towards the target, for a given zoom notch.
///
/// **It must beat the drift of the zoom.** One notch multiplies the distance
/// between cursor and target by `factor`: a correction of `1 - 1/factor`
/// compensates it exactly, anything below lets the target escape. At a fixed
/// 18 % and notches of 1.4, the aimed cell left the screen and the frame went
/// black: measured, four empty frames over fifty notches once the chapter block
/// was set at the top of its box, hence further from the centre of the view. We
/// add a margin to converge instead of merely following.
fn pull(factor: f32) -> f32 {
    ((1.0 - 1.0 / factor.max(1.0)) + 0.12).clamp(0.0, 0.75)
}

/// The text of a verse **as it is drawn**.
///
/// For the eighty-eight verses where another publication of the edition reads
/// differently, that is the welded display string: our verse with the other
/// form inserted right after ours. Everything downstream works on this string
/// and nothing else — wrapping, measuring, word boxes — so the other reading
/// takes room in the flow instead of being painted over the text. The corpus
/// keeps the verse untouched, which is what the breadcrumb and the click name.
fn drawn(verse: usize) -> &'static str {
    variant::display(verse).unwrap_or_else(|| corpus::verse_text(verse))
}

/// Whether a box holds a point.
fn holds(r: Rect, x: f32, y: f32) -> bool {
    x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
}

/// Text column of a cell: the cell minus its margin.
fn column(cell: Rect) -> Rect {
    cell.inset(cell.w * CELL_PAD)
}

/// Distance from a point to a rectangle; zero if it is inside.
fn gap_to(r: Rect, x: f32, y: f32) -> f32 {
    let dx = horizontal_gap(r, x);
    let dy = vertical_gap(r, y);
    dx * dx + dy * dy
}

/// Vertical distance from a point to a rectangle; zero if it is inside.
fn vertical_gap(r: Rect, y: f32) -> f32 {
    (r.y - y).max(y - (r.y + r.h)).max(0.0)
}

/// Horizontal distance from a point to a rectangle; zero if it is inside.
fn horizontal_gap(r: Rect, x: f32) -> f32 {
    (r.x - x).max(x - (r.x + r.w)).max(0.0)
}

fn key_of(fragment: &'static str) -> (u32, u32) {
    (corpus::offset_of(fragment) as u32, fragment.len() as u32)
}

// ----------------------------------------------------------------- interface

/// Prepares the state. Returns the number of books, which serves as a check.
#[unsafe(no_mangle)]
pub extern "C" fn init(width: f32, height: f32) -> u32 {
    unsafe { APP = Some(App::new(width, height)) };
    corpus::book_count() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn resize(width: f32, height: f32) {
    let app = app();
    let refit = app.at_opening();
    app.width = width.max(1.0);
    app.height = height.max(1.0);
    if refit {
        app.fit();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset() {
    let app = app();
    app.level = 0;
    app.fit();
}

#[unsafe(no_mangle)]
pub extern "C" fn pan(dx: f32, dy: f32) {
    let app = app();
    app.camera.x += dx;
    app.camera.y += dy;
}

/// Factor of one wheel notch, computed for this window: the page raises it to
/// the number of notches of the gesture. See `App::zoom_step`.
#[unsafe(no_mangle)]
pub extern "C" fn zoom_step() -> f32 {
    app().zoom_step()
}

#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub extern "C" fn hover(x: f32, y: f32) -> i32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    app.clickable_at(wx, wy).map_or(-1, |chapter| chapter as i32)
}

/// Fills the selection: book, chapter, verse, and the word when one is asked
/// for — each level an (offset, length) readable via `selection_ptr`. Returns 1
/// if something was hit.
///
/// **A click is only live on a passage one can read.** Above that zoom a
/// chapter is a number, and naming one of its verses would be a choice of the
/// geometry, not of the reader: the answer is then 0 and the selection is
/// cleared.
///
/// The export is only unmangled on wasm: on a host build — the one the test
/// suite uses — the symbol `select` collides with the one of the system socket
/// library, and the link fails.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn select(x: f32, y: f32) -> u32 {
    pick_into(x, y, false)
}

/// Same, one step finer: **the word under the point**, which a double click
/// asks for. The verse is filled too, so the page can show the word inside it.
#[unsafe(no_mangle)]
pub extern "C" fn select_word(x: f32, y: f32) -> u32 {
    pick_into(x, y, true)
}

fn pick_into(x: f32, y: f32, word: bool) -> u32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.pick(wx, wy, word) {
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub extern "C" fn leave(mouse: u32) {
    app().mouse_away = mouse != 0;
}

#[unsafe(no_mangle)]
pub extern "C" fn focus_ptr() -> *const u32 {
    app().focus.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn selection_ptr() -> *const u32 {
    app().selection.as_ptr()
}

/// Builds the frame's command list. Returns its number of words.
#[unsafe(no_mangle)]
pub extern "C" fn build_frame() -> u32 {
    let app = app();
    app.build_frame();
    app.commands.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn frame_ptr() -> *const f32 {
    app().commands.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn frame_stride() -> u32 {
    STRIDE as u32
}

/// Fragments whose width the page must measure: (offset, size) pairs.
#[unsafe(no_mangle)]
pub extern "C" fn pending_ptr() -> *const u32 {
    app().pending.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn pending_len() -> u32 {
    app().pending.len() as u32
}

/// Forgets every measured width.
///
/// To be called when the font changes — when the final font loads, for
/// instance: widths taken on a fallback font would be wrong for good.
#[unsafe(no_mangle)]
pub extern "C" fn clear_ratios() {
    let app = app();
    app.ratios.clear();
    app.lines.clear();
    app.number = None;
}

/// Records a measured width, for a font of size 1.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub extern "C" fn cam_x() -> f32 {
    app().camera.x
}

#[unsafe(no_mangle)]
pub extern "C" fn cam_y() -> f32 {
    app().camera.y
}

#[unsafe(no_mangle)]
pub extern "C" fn cam_z() -> f32 {
    app().camera.z
}

#[unsafe(no_mangle)]
pub extern "C" fn cam_fit() -> f32 {
    app().fit_scale()
}

#[unsafe(no_mangle)]
pub extern "C" fn depth() -> u32 {
    app().level
}

/// Diagnostic: on-screen width of the aimed nest, or -1 if there is none.
#[unsafe(no_mangle)]
pub extern "C" fn probe_nest(x: f32, y: f32) -> f32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.nest_at(wx, wy) {
        Some(nest) => app.camera.project(nest).w,
        None => -1.0,
    }
}

/// Diagnostic: on-screen width of the aimed letter cell.
#[unsafe(no_mangle)]
pub extern "C" fn probe_letter(x: f32, y: f32) -> f32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    match app.locate(wx, wy) {
        Some((_, _, _, Some(cell))) => app.camera.project(cell).w,
        _ => -1.0,
    }
}

/// Diagnostic: 1 if the old world has given way to the next.
#[unsafe(no_mangle)]
pub extern "C" fn probe_fade() -> f32 {
    let app = app();
    let camera = app.camera;
    let _ = camera;
    if app.last_nests == 0 { 0.0 } else { 1.0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn probe_points() -> u32 {
    app().last_nests
}

/// Diagnostic: readings of the other edition drawn at the last frame.
///
/// The index under the canvas does not follow this count but the pointer —
/// see `readings_at`. A reading can be on screen without the reader having
/// anything under the cursor, and explaining a mark he is not looking at is
/// noise.
#[unsafe(no_mangle)]
pub extern "C" fn variants_drawn() -> u32 {
    app().variant_runs
}

/// 1 when the chapter under the pointer holds another reading, 0 otherwise.
///
/// The page shows the index under the canvas on that answer alone. Only at the
/// chapter: higher up the answer would be yes almost everywhere, and an index
/// always lit explains nothing.
#[unsafe(no_mangle)]
pub extern "C" fn readings_at(x: f32, y: f32) -> u32 {
    let app = app();
    let (wx, wy) = app.camera.to_world(x, y);
    u32::from(app.readings_at(wx, wy))
}

/// Diagnostic: geometry of a book in the world (0=x 1=y 2=w 3=h).
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub extern "C" fn probe_chapter(chapter: u32, field: u32) -> f32 {
    let r = app().layout.chapters[chapter as usize];
    match field {
        0 => r.x,
        1 => r.y,
        2 => r.w,
        _ => r.h,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn text_ptr() -> *const u8 {
    corpus::text().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn text_len() -> u32 {
    corpus::text().len() as u32
}
