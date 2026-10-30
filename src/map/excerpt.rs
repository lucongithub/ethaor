//! A stretch of the map selected: from what a drag began on to what it
//! reached, in reading order, at every level — a portion's name, a book's,
//! a chapter's number, the letters of a verse — across verses, chapters,
//! books and portions. The page's selection mode. What it holds is platted
//! where it is drawn, and its text is read back, ours alone, to be copied.

use super::*;

/// A piece of text the map draws, which a selection runs over: a portion's
/// name, a book's, a chapter's number, a verse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Unit {
    Section(usize),
    Book(usize),
    Chapter(usize),
    Verse(usize),
}

impl Unit {
    /// Where it lies in the reading: by the first verse it opens, a portion's
    /// name before its first book's, that before its first chapter's number,
    /// that before its first verse.
    fn order(self) -> (usize, u8) {
        let first_of = |book: usize| corpus::verses_of(corpus::chapters_of(book).start).start;
        match self {
            Unit::Section(s) => (first_of(treemap::SECTIONS[s].1), 0),
            Unit::Book(b) => (first_of(b), 1),
            Unit::Chapter(c) => (corpus::verses_of(c).start, 2),
            Unit::Verse(v) => (v, 3),
        }
    }

    /// Its level: a portion's name 0, a book's 1, a chapter's number 2, a
    /// verse 3.
    fn level(self) -> u8 {
        self.order().1
    }

    /// The string it is drawn from: a verse with the other edition's forms
    /// in it (`drawn`).
    pub(crate) fn text(self) -> &'static str {
        match self {
            Unit::Section(s) => corpus::section_name(s),
            Unit::Book(b) => corpus::book_name(b),
            Unit::Chapter(c) => corpus::chapter_label(c),
            Unit::Verse(v) => drawn(v),
        }
    }
}

/// A place in what the map draws: a unit, and a byte offset in its string —
/// between two letters of a verse; before or after a name or a number, which
/// are selected whole. Ordered as the text reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Spot {
    pub(crate) unit: Unit,
    pub(crate) at: usize,
}

impl Ord for Spot {
    fn cmp(&self, other: &Spot) -> std::cmp::Ordering {
        (self.unit.order(), self.at).cmp(&(other.unit.order(), other.at))
    }
}

impl PartialOrd for Spot {
    fn partial_cmp(&self, other: &Spot) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The plate under what is selected, a share of the white: it says which
/// letters are selected, as the browser's selection does in the book view.
pub(crate) const SELECT_PLATE: f32 = 0.25;

impl App {
    /// The place nearest a point of the world in what is drawn there, level
    /// by level as the pointer names it (`path_at`): a portion's name, a
    /// book's, a chapter's number — in its cell, or in its column's corner
    /// once the text is read — then a line of verse. Each gives the edge of
    /// the letter under the point nearer to it, as any text selects: **a
    /// name or a number is selected letter by letter too**, never whole by
    /// force. Nothing where nothing is drawn.
    pub(crate) fn spot_at(&mut self, x: f32, y: f32) -> Option<Spot> {
        let camera = self.camera;
        let (px, py) = (x * camera.z + camera.x, y * camera.z + camera.y);
        let whole = |app: &mut App, unit: Unit, ink: Rect, x: f32| {
            let text = unit.text();
            let size = app.fitted(text, ink);
            app.edge_at(text, ink, size, x).map(|at| Spot { unit, at })
        };

        for section in 0..3 {
            if let Some(ink) = self.section_ink(section, camera)
                && ink.contains(px, py)
            {
                return whole(self, Unit::Section(section), ink, px);
            }
        }
        for book in 0..corpus::book_count() {
            if let Some(ink) = self.book_ink(book, camera)
                && ink.contains(px, py)
            {
                return whole(self, Unit::Book(book), ink, px);
            }
        }

        let (_, chapter, _, _) = self.locate(x, y)?;
        if !self.readable(chapter) {
            let ink = self.numeral_box(chapter);
            let drawn = self.chapters_ready(camera) && ink.contains(x, y);
            return if drawn { whole(self, Unit::Chapter(chapter), ink, x) } else { None };
        }
        if let Some(head) = self.chapter_mark_box(chapter)
            && y <= head.y + head.h
        {
            return whole(self, Unit::Chapter(chapter), head, x);
        }

        let line = self.line_at(chapter, y)?;
        let start = corpus::offset_of(line.text) - corpus::offset_of(drawn(line.verse));
        let at = start + self.edge_at(line.text, line.rect, line.size, x)?;
        Some(Spot { unit: Unit::Verse(line.verse), at })
    }

    /// The edge between two letters of `text`, laid flush right in `row` at
    /// `size`, nearest to `x`: of the letter under it — or the nearest one —
    /// its start on its right half, its end on its left, the text reading
    /// from the right. A byte offset in `text`.
    fn edge_at(&mut self, text: &'static str, row: Rect, size: f32, x: f32) -> Option<usize> {
        let cells = self.letter_cells(text, row, size);
        let letters = text.split(' ').flat_map(letter::letters);
        let (cell, part) = cells
            .into_iter()
            .zip(letters)
            .min_by(|a, b| horizontal_gap(a.0, x).total_cmp(&horizontal_gap(b.0, x)))?;
        let from = corpus::offset_of(part) - corpus::offset_of(text);
        Some(if x >= cell.x + cell.w / 2.0 { from } else { from + part.len() })
    }

    /// The size a name or a number is laid at in its ink: the one its width
    /// fills the ink at, the ink being as wide as its text.
    fn fitted(&mut self, text: &'static str, ink: Rect) -> f32 {
        let unit = self.width_of(text, 1.0);
        if unit > 0.0 { ink.w / unit } else { 0.0 }
    }

    /// Where a chapter's number stands once its text is read, in the world:
    /// the top right corner of its column, on the header row the layout
    /// leaves it — the geometry `draw_book` lays it in.
    pub(crate) fn chapter_mark_box(&mut self, chapter: usize) -> Option<Rect> {
        let first = *self.chapter_lines(chapter).first()?;
        let size = first.size * CHAPTER_MARK;
        let col = column(self.layout.chapters[chapter]);
        let w = self.width_of(corpus::chapter_label(chapter), size);
        Some(Rect { x: col.x + col.w - w, y: col.y, w, h: size * LINE_K })
    }

    /// A drag begins at a point: the selection starts there, empty. False,
    /// and no selection, where nothing is drawn.
    pub(crate) fn excerpt_start(&mut self, x: f32, y: f32) -> bool {
        self.excerpt = self.spot_at(x, y).map(|spot| (spot, spot));
        self.excerpt.is_some()
    }

    /// The drag reaches a point: the selection runs from where it began to
    /// what is drawn there. A point on nothing keeps the last place.
    pub(crate) fn excerpt_to(&mut self, x: f32, y: f32) {
        let Some((anchor, _)) = self.excerpt else {
            return;
        };
        if let Some(head) = self.spot_at(x, y) {
            self.excerpt = Some((anchor, head));
        }
    }

    pub(crate) fn excerpt_clear(&mut self) {
        self.excerpt = None;
    }

    /// The selection in reading order, its first spot first; nothing when
    /// it holds nothing.
    pub(crate) fn excerpt_range(&self) -> Option<(Spot, Spot)> {
        let (a, b) = self.excerpt?;
        (a != b).then(|| (a.min(b), a.max(b)))
    }

    /// Whether the selection holds any of a unit.
    ///
    /// **What it runs over is what its ends were drawn on**: a unit counts
    /// only down to the deepest level of the two ends. Two books' names select
    /// the names between, never the thousands of verses the reading puts
    /// there; two chapters' numbers, the numbers and the names; a verse, the
    /// text too.
    pub(crate) fn excerpt_holds(&self, unit: Unit) -> bool {
        self.excerpt_range().is_some_and(|(a, b)| {
            unit.level() <= a.unit.level().max(b.unit.level())
                && a < Spot { unit, at: unit.text().len() }
                && Spot { unit, at: 0 } < b
        })
    }

    /// The bytes of a unit's string the selection holds, if any.
    fn excerpt_span(&self, unit: Unit) -> Option<(usize, usize)> {
        if !self.excerpt_holds(unit) {
            return None;
        }
        let (a, b) = self.excerpt_range()?;
        let (from, to) = (Spot { unit, at: 0 }.max(a), Spot { unit, at: unit.text().len() }.min(b));
        Some((if from.unit == unit { from.at } else { 0 }, to.at))
    }

    /// The text selected, as it is copied: each name, number and verse it
    /// holds (`excerpt_holds`) on its own line, in reading order — **our
    /// text**, the other edition's forms taken out. Empty for no selection.
    pub(crate) fn excerpt_text(&self) -> String {
        let Some((a, b)) = self.excerpt_range() else {
            return String::new();
        };
        let mut out = Vec::new();
        let mut take = |unit: Unit| {
            let Some((from, to)) = self.excerpt_span(unit) else {
                return;
            };
            let piece = match unit {
                Unit::Verse(verse) => ours(verse, from, to),
                _ => unit.text()[from..to].to_string(),
            };
            let piece = piece.split_whitespace().collect::<Vec<_>>().join(" ");
            if !piece.is_empty() {
                out.push(piece);
            }
        };
        let (first, last) = (a.unit.order().0, b.unit.order().0);
        for section in 0..3 {
            let (_, books_from, books_to) = treemap::SECTIONS[section];
            take(Unit::Section(section));
            for book in books_from..books_to {
                take(Unit::Book(book));
                for chapter in corpus::chapters_of(book) {
                    let verses = corpus::verses_of(chapter);
                    if verses.end <= first || verses.start > last {
                        continue;
                    }
                    take(Unit::Chapter(chapter));
                    for verse in verses {
                        take(Unit::Verse(verse));
                    }
                }
            }
        }
        out.join("\n")
    }

    /// The verses' letters the selection holds, as a letters record names
    /// them (`words::LETTER_WORDS`): each letter's verse and its rank among
    /// our verse's consonants, in reading order, pushed into `out`. A letter
    /// counts when its consonant is selected and its verse is held
    /// (`excerpt_holds`); names and numbers have none.
    pub(crate) fn excerpt_letters(&self, out: &mut Vec<u32>) {
        out.clear();
        let Some((a, b)) = self.excerpt_range() else {
            return;
        };
        let first = match a.unit {
            Unit::Verse(v) => v,
            unit => unit.order().0,
        };
        let last = match b.unit {
            Unit::Verse(v) => v,
            unit => unit.order().0.saturating_sub(1),
        };
        for verse in first..=last.min(corpus::verse_count().saturating_sub(1)) {
            let Some((from, to)) = self.excerpt_span(Unit::Verse(verse)) else {
                continue;
            };
            for (rank, &(start, _)) in letter_bytes(verse).iter().enumerate() {
                if (from..to).contains(&(start as usize)) {
                    out.extend_from_slice(&[verse as u32, rank as u32]);
                }
            }
        }
    }

    /// The plate under the selected stretch of a line drawn in `row`, if it
    /// holds any: from the right edge, past what precedes the stretch, as wide
    /// as the stretch — measured as the line itself is, flush right.
    pub(crate) fn excerpt_plate(&mut self, line: Line, row: Rect, size: f32) -> Option<Rect> {
        let (a, b) = self.excerpt_range()?;
        let unit = Unit::Verse(line.verse);
        if !self.excerpt_holds(unit) {
            return None;
        }
        let shown = drawn(line.verse);
        let start = corpus::offset_of(line.text) - corpus::offset_of(shown);
        let end = start + line.text.len();
        let (from, to) = (Spot { unit, at: start }.max(a), Spot { unit, at: end }.min(b));
        if to <= from {
            return None;
        }
        let from = if from.unit == unit { from.at } else { start };
        let to = if to.unit == unit { to.at } else { end };
        let right = row.x + row.w - self.stretch_width(&shown[start..from], size);
        let w = self.stretch_width(&shown[from..to], size);
        Some(Rect { x: right - w, w, ..row })
    }

    /// Lays the plate under the letters the selection holds of a name or a
    /// number, whose ink lies `on` the screen: before its text, so under it,
    /// and only under them — measured as a line is, flush right.
    pub(crate) fn plate_under(&mut self, unit: Unit, on: Option<Rect>, alpha: f32) {
        let (Some(ink), Some((from, to))) = (on, self.excerpt_span(unit)) else {
            return;
        };
        let text = unit.text();
        let size = self.fitted(text, ink);
        let right = ink.x + ink.w - self.stretch_width(&text[..from], size);
        let w = self.stretch_width(&text[from..to], size);
        self.plate(Rect { x: right - w, w, ..ink }, alpha);
    }

    pub(crate) fn plate(&mut self, r: Rect, alpha: f32) {
        self.commands
            .extend_from_slice(&[KIND_SELECT, r.x, r.y, r.w, r.h, alpha * SELECT_PLATE, 0.0, 0.0, 0.0, 0.0]);
    }

    /// Width of a stretch cut anywhere in a line: its words as `width_of`
    /// measures them, and the spaces it begins or ends with — nothing for
    /// an empty one, which `width_of` would guess a letter wide.
    fn stretch_width(&mut self, stretch: &'static str, size: f32) -> f32 {
        let core = stretch.trim_matches(' ');
        let spaces = (stretch.len() - core.len()) as f32;
        let ink = if core.is_empty() { 0.0 } else { self.width_of(core, size) };
        ink + spaces * self.space_ratio() * size
    }
}

/// A verse's bytes `from..to` of the string it is drawn from, the other
/// edition's insertions taken out: our text.
fn ours(verse: usize, from: usize, to: usize) -> String {
    let shown = drawn(verse);
    let inserts: Vec<(usize, usize)> = variant::inserts_of(verse)
        .map(|insert| {
            let (at, len) = insert.range();
            (at, at + len)
        })
        .collect();
    shown[from..to]
        .char_indices()
        .filter(|&(i, _)| !inserts.iter().any(|&(s, e)| (s..e).contains(&(from + i))))
        .map(|(_, c)| c)
        .collect()
}
