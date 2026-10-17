//! What lies under a point: the click (passage, word), the cursor, the
//! breadcrumb — each naming only what is drawn there.

use super::*;

impl App {
    pub(crate) fn chapter_at(&self, x: f32, y: f32) -> Option<usize> {
        self.layout.chapters.iter().position(|r| r.contains(x, y))
    }

    /// Chapter one can click on, under a point of the world.
    ///
    /// The same tolerance as `locate` — a book box, never the air between two
    /// portions — plus the reading condition.
    pub(crate) fn clickable_at(&mut self, x: f32, y: f32) -> Option<usize> {
        let (_, chapter, _, _) = self.locate(x, y)?;
        self.readable(chapter).then_some(chapter)
    }

    /// What a click selects: book, chapter, verse, and the word if one is
    /// asked for. Each level is an (offset, length) in the corpus.
    ///
    /// Returns nothing above the reading zoom: **a click is only live on a
    /// passage one can read.**
    pub(crate) fn pick(&mut self, x: f32, y: f32, word: bool) -> Option<[u32; 6]> {
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
    pub(crate) fn numeral_box(&mut self, chapter: usize) -> Rect {
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
    pub(crate) fn section_ink(&mut self, section: usize, camera: Camera) -> Option<Rect> {
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
    pub(crate) fn book_ink(&mut self, book: usize, camera: Camera) -> Option<Rect> {
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

    /// Whether the pointer is on a word the two editions read apart: our form,
    /// marked `¹`, or theirs, `[…]²`.
    ///
    /// **The words themselves, and nothing around them** — the owner's rule.
    /// The index explains two marks; lit for the whole chapter that holds
    /// them, it spoke while the reader was lines away. So the point must be
    /// on a drawn line, inside a word's box, and that word must touch an
    /// insertion. A number, the void of a cell, a book or a portion name no
    /// word, and say nothing.
    pub(crate) fn readings_at(&mut self, x: f32, y: f32) -> bool {
        let Some((_, chapter, _, _)) = self.locate(x, y) else {
            return false;
        };
        if !self.readable(chapter) {
            return false;
        }
        let Some(line) = self.line_at(chapter, y) else {
            return false;
        };
        if y < line.rect.y || y > line.rect.y + line.rect.h {
            return false;
        }
        let Some(shown) = variant::display(line.verse) else {
            return false;
        };
        let base = corpus::offset_of(shown);
        let inserts: Vec<(usize, usize)> = variant::inserts_of(line.verse).map(|i| i.range()).collect();
        self.word_cells(line.text, line.rect, line.size).into_iter().any(|(r, word)| {
            let from = corpus::offset_of(word) - base;
            let to = from + word.len();
            x >= r.x && x <= r.x + r.w && inserts.iter().any(|&(at, len)| from < at + len && at < to)
        })
    }

    /// Chapter nearest to a point, even if that point falls in a gap between two
    /// boxes.
    pub(crate) fn nearest_chapter(&self, x: f32, y: f32) -> Option<usize> {
        self.layout
            .chapters
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| gap_to(**a, x, y).total_cmp(&gap_to(**b, x, y)))
            .map(|(i, _)| i)
    }

    /// Book whose box contains a point.
    pub(crate) fn book_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..corpus::book_count()).find(|&b| self.layout.books[b].contains(x, y))
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
    pub(crate) fn path_at(&mut self, x: f32, y: f32, strict: bool) -> Option<[u32; 8]> {
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

    /// Where the view is, for the book view to open there: the chapter at the
    /// centre, and the verse when the text is read.
    ///
    /// **Never deeper than the image**, like the breadcrumb: at the opening —
    /// three words — the Tanakh's beginning; with the books alone, the first
    /// chapter of the book at the centre; with the numbers, that chapter; with
    /// the text, its verse too.
    pub(crate) fn here(&mut self) -> (usize, Option<usize>) {
        let camera = self.camera;
        if !self.books_drawn(camera) {
            return (0, None);
        }
        let (x, y) = camera.to_world(self.width / 2.0, self.height / 2.0);
        let Some(chapter) = self.chapter_at(x, y).or_else(|| self.nearest_chapter(x, y)) else {
            return (0, None);
        };
        if !self.chapters_ready(camera) {
            let book = (0..corpus::book_count()).find(|&b| corpus::chapters_of(b).contains(&chapter));
            return (book.map_or(0, |b| corpus::chapters_of(b).start), None);
        }
        if !self.readable(chapter) {
            return (chapter, None);
        }
        let verse = self.line_at(chapter, y).map(|line| line.verse);
        (chapter, verse)
    }

    /// Full path under a point of the world.
    ///
    /// If the point falls between two cells, we keep **the nearest one in the
    /// same book**. Cells are spaced out to take the full width of their box: in
    /// a book of three chapters, the gap exceeds a hundred world units, and a
    /// strict search returned nothing most of the time. Tolerance stops at the
    /// book — clicking in the air that separates two portions selects nothing.
    pub(crate) fn locate(&mut self, x: f32, y: f32) -> Option<(usize, usize, usize, Option<Rect>)> {
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

    pub(super) fn locate_in(
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
}

/// Whether a box holds a point.
fn holds(r: Rect, x: f32, y: f32) -> bool {
    x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
}

/// Distance from a point to a rectangle; zero if it is inside.
pub(crate) fn gap_to(r: Rect, x: f32, y: f32) -> f32 {
    let dx = horizontal_gap(r, x);
    let dy = vertical_gap(r, y);
    dx * dx + dy * dy
}

/// Vertical distance from a point to a rectangle; zero if it is inside.
pub(crate) fn vertical_gap(r: Rect, y: f32) -> f32 {
    (r.y - y).max(y - (r.y + r.h)).max(0.0)
}

/// Horizontal distance from a point to a rectangle; zero if it is inside.
pub(crate) fn horizontal_gap(r: Rect, x: f32) -> f32 {
    (r.x - x).max(x - (r.x + r.w)).max(0.0)
}
