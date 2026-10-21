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
                self.emit_text(corpus::section_name(section), area, cards.size, alpha);
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
    pub(crate) fn draw_line(&mut self, line: Line, row: Rect, scale: f32, alpha: f32) {
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
