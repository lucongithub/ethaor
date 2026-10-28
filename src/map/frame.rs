//! The frame: the command list the page paints — titles, numbers, verse
//! lines, clips — and the sizes each level is drawn at.

use super::*;

/// One record of the command list, in 4-byte words.
pub(crate) const STRIDE: usize = 10;

/// There is **no fill left** in the rendering: only text and clipping commands
/// come out of here. The world reduced to a white square went away with the
/// zoom-out floor and the early fold.
pub(crate) const KIND_TEXT: f32 = 1.0;
/// Opens then closes a clip. A chapter's text must never bite into its
/// neighbours, even if a badly measured width makes it overflow.
pub(crate) const KIND_CLIP: f32 = 2.0;
pub(crate) const KIND_UNCLIP: f32 = 3.0;
/// A search's square beside a name: its box (left, top, side, side), its
/// slot's bit as the ink. The one mark that is not a letter, and it carries
/// information too: this chapter, book or portion holds what the search found.
pub(crate) const KIND_SQUARE: f32 = 4.0;

/// Ink of a text command, last slot of the record.
///
/// **The one colour of the map, and it carries information.** A run marked
/// `INK_OTHER` is the other publication's reading, inserted after ours; the
/// page paints it `yellow-200` and everything else a shade of white. The
/// intensity of the white was tried first and set aside: it read as a faded
/// text rather than as another voice. Which colour is the page's business —
/// the module only says which run is which.
pub(crate) const INK_OURS: f32 = 0.0;
pub(crate) const INK_OTHER: f32 = 1.0;
/// A search's letters, and its squares: `INK_MARK + bits`, one bit per slot
/// holding them — a square has one, a letter as many as searches found it. The
/// page gives each slot the colour its search wears, and paints a letter of
/// several in bands of theirs.
pub(crate) const INK_MARK: f32 = 2.0;

/// A square's side and the space to the next one, its gap from the name, in
/// shares of the name's size; and how far above the box's middle its centre
/// sits, at the height of a letter's body rather than of the line. The side
/// is the room a cell leaves under its number, below the digits.
pub(crate) const SQUARE: f32 = 0.26;
const SQUARE_SPACE: f32 = 0.1;
const SQUARE_GAP: f32 = 0.35;
const SQUARE_RISE: f32 = 0.06;
/// The widest a row of squares grows, in shares of the name's size: each
/// search shown adds its square, and past this width they all shrink to fit —
/// the row fits under every chapter number, every cell sharing one size.
pub(crate) const SQUARE_ROW: f32 = 1.4;
/// Under a chapter number: how far under its baseline the squares' tops
/// sit, in shares of its size.
const SQUARE_DROP: f32 = 0.17;
/// A row whose single square would be smaller than this, in pixels, is not
/// seen: not drawn.
pub(crate) const SQUARE_MIN: f32 = 1.0;

/// A square's side and the space after it, for the squares of `marks` by a
/// name of `size`: the nominal ones while the row is narrower than
/// `SQUARE_ROW`, both shrunk alike past it.
pub(crate) fn square_sides(marks: u32, size: f32) -> (f32, f32) {
    let n = marks.count_ones().max(1) as f32;
    let natural = n * SQUARE + (n - 1.0) * SQUARE_SPACE;
    let k = (SQUARE_ROW / natural).min(1.0);
    (size * SQUARE * k, size * SQUARE_SPACE * k)
}

/// The width a row of the squares of `marks` takes, end to end.
fn row_width(marks: u32, side: f32, space: f32) -> f32 {
    let n = marks.count_ones() as f32;
    n * side + (n - 1.0).max(0.0) * space
}

/// A title's text, with the searches holding its place, as bits: their
/// squares go by it.
#[derive(Clone, Copy)]
struct Named {
    text: &'static str,
    marks: u32,
}

/// Sizes of one level. Every drawn world has its own.
#[derive(Clone, Copy)]
pub(super) struct Scale {
    pub(super) show_chapters: bool,
    chapter: f32,
    pub(super) book: f32,
    pub(super) section: f32,
}

impl App {
    pub(crate) fn visible(&self, r: Rect) -> bool {
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
    fn title(&mut self, name: Named, box_: Rect, band: Rect, size: f32, full: bool, alpha: f32) {
        let Named { text: content, marks } = name;
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
        if marks != 0 {
            // A title filling its box has no room beside it: its squares go
            // under it, in the room the box leaves below the letters.
            let w = self.width_of(content, size).min(area.w);
            let right = area.x + area.w;
            if full {
                self.squares_under(marks, right - w / 2.0, area.y + area.h / 2.0 + size * 0.36, size);
            } else {
                self.squares(marks, right - w, box_.x, area, size);
            }
        }
        self.unclip();
    }

    /// The squares of the slots in `marks`, laid leftwards from `left` — the
    /// left end of a name, where Hebrew ends — on the name's box, at its size,
    /// and never past `bound`: where the room is short, they shrink into it.
    fn squares(&mut self, marks: u32, left: f32, bound: f32, area: Rect, size: f32) {
        let (side, space) = square_sides(marks, size);
        let wide = row_width(marks, side, space);
        let k = ((left - bound) / (size * SQUARE_GAP + wide)).min(1.0);
        let middle = area.y + area.h / 2.0 + size * SQUARE_RISE;
        let at = (left - size * SQUARE_GAP * k, middle - side * k / 2.0);
        self.square_row(marks, at, side * k, space * k, size * SQUARE * k);
    }

    /// The squares of a name, in a row under its baseline, centred on
    /// `middle`. A chapter's number has them there — a cell is square, and
    /// beside its number there is no room — and the row fits in the cell's
    /// width: every cell and every number share one size, so it fits
    /// everywhere.
    fn squares_under(&mut self, marks: u32, middle: f32, baseline: f32, size: f32) {
        let (side, space) = square_sides(marks, size);
        let wide = row_width(marks, side, space);
        self.square_row(marks, (middle + wide / 2.0, baseline + size * SQUARE_DROP), side, space, size * SQUARE);
    }

    /// A row of squares of `side`, the first one's top right corner `at`,
    /// lowest slot first. Seen or not is `alone`'s to say, the side one
    /// square would have by itself: a full row's squares go under a pixel
    /// on a small number, and still read as a row of dots.
    fn square_row(&mut self, marks: u32, at: (f32, f32), side: f32, space: f32, alone: f32) {
        if alone < SQUARE_MIN {
            return;
        }
        let (right, top) = at;
        let mut edge = right;
        for slot in 0..MARK_SLOTS {
            if marks & (1 << slot) == 0 {
                continue;
            }
            let x = edge - side;
            self.commands
                .extend_from_slice(&[KIND_SQUARE, x, top, side, side, 1.0, 0.0, 0.0, 0.0, INK_MARK + (1u32 << slot) as f32]);
            edge = x - space;
        }
    }

    /// Lays a fragment, set on the right edge of its box, at an imposed size.
    /// That is what lets all the verses of a chapter share the same size
    /// whatever their length.
    /// The legibility floor is the caller's choice: a title and a line of verse
    /// stop at `MIN_TEXT`, a chapter number at `MIN_NUMBER`.
    /// Where the three portions' names lie when the map is nothing else.
    ///
    /// Everything is taken on the **projected** world: the three words are
    /// born large at the first zoom-out notch, then shrink and draw closer
    /// with it, endlessly. Freezing them at a screen size was tried: they read
    /// well, but zooming out no longer did anything at all.
    ///
    /// **A single right edge for all three**, the one given by the widest
    /// word: reading demands it, and three words each centred for themselves
    /// went off in three directions. The block thus formed is itself centred
    /// on the world. The drawing and the pointer both read it here.
    ///
    /// Once the page gives the book view's size (`portion_size`), the opening
    /// draws the names at that very size, spaced as the book view spaces
    /// them: switching views shows the same three words.
    pub(crate) fn cards(&mut self, camera: Camera) -> Cards {
        let widest = (0..3).map(|section| self.ratio(corpus::section_name(section))).fold(0.3_f32, f32::max);
        let world = camera.project(WORLD);
        let fits = world.w * 0.8 / widest;
        let (mut size, mut gap) = if self.portion_size > 0.0 {
            // The book view's size at the opening, and in proportion to the
            // zoom on either side of it: they still shrink as the map recedes.
            let size = (self.portion_size * camera.z / self.overview_scale()).min(fits);
            (size, size * CARD_LEAD)
        } else {
            ((world.h * SECTION_CARD / 3.0).min(fits), world.h * CARD_GAP)
        };
        let tall = 2.0 * gap + size * 1.35;
        if tall > self.height * CARD_ROOM {
            let k = self.height * CARD_ROOM / tall;
            (size, gap) = (size * k, gap * k);
        }
        Cards {
            size,
            gap,
            middle: world.y + world.h / 2.0,
            left: world.x,
            span: (world.w + widest * size) / 2.0,
        }
    }

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

    pub(crate) fn build_frame(&mut self) {
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
    pub(crate) fn number_size(&mut self) -> f32 {
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
    pub(crate) fn chapters_ready(&mut self, camera: Camera) -> bool {
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
    pub(super) fn scale_of(&mut self, camera: Camera) -> Scale {
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
            let cards = self.cards(camera);
            for section in 0..3 {
                let area = cards.area(section);
                let name = corpus::section_name(section);
                self.emit_text(name, area, cards.size, alpha);
                let marks = self.section_marks(section);
                if marks != 0 {
                    let w = self.width_of(name, cards.size);
                    self.squares(marks, area.x + area.w - w, area.x, area, cards.size);
                }
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
            let name = Named { text: corpus::section_name(section), marks: self.section_marks(section) };
            self.title(name, pinned, strip, scale.section, false, alpha);

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
                    if corpus::offset_of(line.text) == corpus::offset_of(super::text::drawn(line.verse)) {
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
                let marks = self.chapter_marks(chapter);
                if marks != 0 {
                    let baseline = area.y + area.h / 2.0 + scale.chapter * 0.36;
                    self.squares_under(marks, cell.x + cell.w / 2.0, baseline, scale.chapter);
                }
            }
        }

        let strip = camera.project(self.layout.book_bands[book]);
        let full = !scale.show_chapters;
        let name = Named { text: corpus::book_name(book), marks: self.book_marks(book) };
        self.title(name, rect, strip, scale.book, full, alpha * BOOK_INK);
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
    ///
    /// The line is cut into runs where its ink changes: the other edition's
    /// reading, and the letters a shown search found. A verse carrying a
    /// reading is drawn from its display string, where the other form already
    /// takes its place in the flow — so nothing is ever painted over anything.
    /// A line with neither stays one single command.
    pub(crate) fn draw_line(&mut self, line: Line, row: Rect, scale: f32, alpha: f32) {
        let size = line.size * scale;
        if size < MIN_TEXT {
            return;
        }

        let shown = super::text::drawn(line.verse);
        let start = corpus::offset_of(line.text) - corpus::offset_of(shown);
        let end = start + line.text.len();

        let mut cuts = std::mem::take(&mut self.cuts);
        cuts.clear();
        for insert in variant::inserts_of(line.verse) {
            let (at, len) = insert.other_range();
            cuts.push((at, at + len, INK_OTHER));
        }
        self.verse_marks(line.verse, &mut cuts);
        // Neither kind overlaps itself nor the other: our letters never lie
        // in an insertion.
        cuts.sort_by_key(|cut| cut.0);

        let mut right = row.x + row.w;
        let mut cursor = start;
        for &(at, until, ink) in &cuts {
            let (from, to) = (at.max(cursor), until.min(end));
            if to <= from {
                continue;
            }
            if from > cursor {
                right = self.emit_run(&shown[cursor..from], row, right, size, alpha, INK_OURS);
            }
            right = self.emit_run(&shown[from..to], row, right, size, alpha, ink);
            cursor = to;
        }
        if cursor == start {
            self.emit_text(line.text, row, size, alpha);
        } else if cursor < end {
            self.emit_run(&shown[cursor..end], row, right, size, alpha, INK_OURS);
        }
        self.cuts = cuts;
    }
}

/// The three portions' names alone, on screen: one size, a gap between
/// their centres, the middle one's centre, and the box they are laid flush
/// right in — from `left`, `span` wide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cards {
    pub size: f32,
    pub gap: f32,
    pub middle: f32,
    pub left: f32,
    pub span: f32,
}

impl Cards {
    /// The line a portion's name is laid in, flush right on the shared edge.
    pub(crate) fn area(&self, section: usize) -> Rect {
        let line = self.size * 1.35;
        let y = self.middle + (section as f32 - 1.0) * self.gap;
        Rect { x: self.left, y: y - line / 2.0, w: self.span, h: line }
    }
}
