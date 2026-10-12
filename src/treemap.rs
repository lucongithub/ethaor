//! Tanakh layout: three sections, then the books, then the chapters.
//!
//! **Canonical order is preserved everywhere.** A classic squarified treemap
//! sorts by size to get nice proportions; here that would be wrong — the order
//! of the books of the Tanakh is not to be shuffled. So we use a strip
//! treemap, which keeps the order and leaves only the choice of when to start
//! a new row.
//!
//! Reading order is that of a page: **right to left within a row, then the
//! next row below**. An element that comes after another is therefore either
//! to its left on the same line or lower down — never above. Rows are always
//! horizontal: a vertical column would break that continuity.
//!
//! The layout is computed once in a fixed world space; the camera then
//! projects it. Zooming never recomputes the geometry.

use crate::corpus;

/// World space. Proportions of an ordinary landscape window.
pub const WORLD: Rect = Rect { x: 0.0, y: 0.0, w: 1600.0, h: 1000.0 };

/// The three sections, in order: Torah, Nevi'im, Ketuvim.
/// The three sections and their bounds, in book indices: 5 for the Torah,
/// 21 for the Prophets, 13 for the Writings.
pub const SECTIONS: [(&str, usize, usize); 3] =
    [("תורה", 0, 5), ("נביאים", 5, 26), ("כתובים", 26, 39)];


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// Shrinks towards the centre, by a factor.
    pub fn shrink(&self, factor: f32) -> Rect {
        let (w, h) = (self.w * factor, self.h * factor);
        Rect { x: self.x + (self.w - w) / 2.0, y: self.y + (self.h - h) / 2.0, w, h }
    }

    pub fn inset(&self, pad: f32) -> Rect {
        let pad = pad.min(self.w / 2.0).min(self.h / 2.0).max(0.0);
        Rect { x: self.x + pad, y: self.y + pad, w: self.w - 2.0 * pad, h: self.h - 2.0 * pad }
    }
}

pub struct Layout {
    /// One strip per section, stacked top to bottom.
    pub sections: [Rect; 3],
    pub books: Vec<Rect>,
    pub chapters: Vec<Rect>,
    /// Title bands, in world coordinates. The title sits at the **bottom** of
    /// the band, just above the first chapter: the gap that separates them then
    /// stays the same at every zoom.
    pub section_bands: [Rect; 3],
    pub book_bands: Vec<Rect>,
}

/// Height of a section's title band, in world units.
///
/// It is taller than a book's, so that its name stays above the book names in
/// the hierarchy — colour alone would not be enough if a book title were
/// bigger.
///
/// A section's band reads in two parts: the air that precedes it — **that** is
/// what separates one portion from the previous one, the title sitting at the
/// bottom of its band — then the title line itself.
/// A portion's air has **two values**, one per level of detail.
///
/// In the overview, where only titles are read, it is what says the Tanakh
/// divides in three: it is worth thirty times the gutter between two rows of
/// books. As soon as the chapters appear, reading happens inside a book, the
/// division is already settled, and the same void only pushes the text away:
/// we halve it, which gives height back to the rows and grows the cells by a
/// third.
///
/// Measured: boxes of 56.7 and cells of 6.3 at wide air, 76.7 and 8.8 at tight
/// air.
pub const SECTION_LEAD_WIDE: f32 = 90.0;
pub const SECTION_LEAD_TIGHT: f32 = SECTION_LEAD_WIDE / 2.0;
const SECTION_TITLE: f32 = 28.0;

/// Wanted ratio between a chapter number and the title of the book holding it.
pub const CHAPTER_OVER_BOOK: f32 = 0.5;

/// Share of a cell's width that a number's ink may take, and share of its
/// height. Overflow is therefore impossible in both directions.
pub const NUMBER_WIDTH: f32 = 0.92;
pub const NUMBER_HEIGHT: f32 = 0.72;

/// Share of the cell a number takes, **given the width of its digits**.
///
/// `numeral` is the width of the widest number of the Tanakh — three letters —
/// for a font of size 1. Until the page has measured, it is an estimate; as
/// soon as the measurement arrives, the layout is rebuilt with it.
///
/// That is what keeps the number/title ratio exact in the browser: if the
/// Hebrew digits are wider than the estimate, width is what bounds the number,
/// and the title band must tighten accordingly.
pub fn number_fill(numeral: f32) -> f32 {
    (NUMBER_WIDTH / numeral.max(0.1)).min(NUMBER_HEIGHT)
}

/// Title band, pitch and width of the chapter block, **all three deduced
/// together** from a book's box. Returns `(pitch, band, columns)`.
///
/// Rows are imposed: `CHAPTER_ROWS`. The rest is tied together — the title
/// fills its band, the number fills its cell, the number is worth half the
/// title, and what the band does not take is shared between the rows. Fixing
/// one fixes the others, so we solve them by fixed point.
///
/// Columns do **not** come from the width of the box: they come from the
/// densest book, which must fill its rows. That is what gives the 150 Psalms
/// seven rows of 22 rather than four of 38 — at the width of the box, they
/// would fit in four. The block then takes only half the box in width, and
/// that is intended: height is what is scarce.
///
/// Measured, box of 56.7: under the estimated digit width, a pitch of 6.8 and
/// a band of 7.3; under digits measured at 0.58 per letter — what a Hebrew
/// serif gives — a pitch of 6.6 and a band of 6.6. The number/title ratio
/// stays 0.500 in both cases.
fn chapters_fit(inner: Rect, densest: usize, fill: f32) -> (f32, f32, usize) {
    let rows = CHAPTER_ROWS;
    let columns = densest.div_ceil(rows).max(1);
    let (mut pitch, mut band) = (0.0, 0.0);

    // The pitch follows from what the band leaves; the band follows from the
    // pitch. Eight passes are more than enough to settle the pair.
    for _ in 0..8 {
        pitch = ((inner.h - band) / rows as f32).max(0.1).min(inner.w / columns as f32);
        band = ((fill / CHAPTER_OVER_BOOK) * (pitch - 2.0 * CHAPTER_GAP))
            .clamp(0.0, (inner.h * 0.5).max(0.0));
    }

    // Never negative: a box lower than its own gutters would otherwise give
    // cells turned inside out, which every geometry downstream relies on not
    // happening.
    (pitch.min((inner.h - band) / rows as f32).max(0.0), band, columns)
}

/// Number of book columns, for the three sections.
///
/// It is the width imposed by the five books of the Torah; the other sections
/// take it over, and all book boxes are then the same size from one end of the
/// Tanakh to the other. The 21 Prophets thus fit in five rows, the 13 Writings
/// in three.
///
/// Seven columns were tried — six rows of books, boxes of 221.7 x 87.0, cells a
/// fifth bigger — then dropped: the book grid is not a setting for cell size,
/// and the Torah lost its full row there.
const BOOK_COLUMNS: usize = 5;

/// Outer margin of the world. It only applies at the edge, never between two
/// sections.
const WORLD_MARGIN: f32 = 10.0;

/// The three sections are **flush**: what separates them is exactly what
/// separates two columns of books, i.e. `2 x BOOK_GAP`. The title band is
/// enough to tell them apart; a wider breathing space dug a 114 world-unit void
/// where books only get 6, and stole from the rows the height the titles need.
const BOOK_GAP: f32 = 2.0;
/// Air a book's box keeps beyond its chapter block, as a share of that block's
/// width.
///
/// **It is what sets the width of a book column**, not the other way round. The
/// box was 312 units for a block of 145: more than half was emptiness left of
/// the chapters, and a book title floated in the middle of nothing. At a tenth,
/// it is 159 and the block takes 91 % of it.
///
/// The column therefore follows the pitch: if the pitch changes, the box width
/// changes with it, and the ratio stays the same.
const BOOK_SLACK: f32 = 0.1;

/// Breathing space around a chapter cell. **Very small**: the pitch is what
/// keeps the rhythm, and the cell must take nearly all of it.
///
/// At 0.5, the gap ate 1 unit out of a pitch of 6.6, i.e. 15 % of the ink,
/// which shows as soon as rows are many and the pitch tight. At 0.15, the cell
/// goes from 5.7 to 6.3 without us losing the ability to tell two chapters
/// apart: the background being black, the gap needs not be wide, only non-zero.
const CHAPTER_GAP: f32 = 0.15;

/// Chapter rows of the densest book: **seven for the 150 Psalms**, i.e. seven
/// rows of 22.
///
/// That count sets everything else: columns are `ceil(150 / 7)`, the pitch is
/// the body height divided by seven, and the band follows. The other books
/// spread into the same grid.
///
/// The price is known: at constant box, every extra row takes from the pitch
/// what it takes in height. Measured, box of 56.7: five rows gave a cell of
/// 8.5, six one of 7.2, seven one of 6.3.
const CHAPTER_ROWS: usize = 7;

/// Height reserved for the title, at the top of a box.
///
/// **Constant**, not proportional to the box: a proportional band is
/// non-existent for a small book, and its title then bites into the chapters.
/// It is only trimmed if the box is too low to hold it.
///
/// The one-third bound now only serves boxes that are too low: the book band is
/// henceforth deduced from the title/number ratio by `book_band`, and is worth
/// 9.4 units for a box of 45.6, i.e. a fifth of its height.
fn header(h: f32, band: f32) -> f32 {
    band.min(h * 0.33)
}

pub fn build(numeral: f32, lead: f32) -> Layout {
    let band = lead + SECTION_TITLE;
    // --- the three strips
    //
    // Their height is not proportional to text volume but computed so that
    // **all book boxes are identical**, from one section to the next. At a
    // common row height `row`, a section carrying `n` rows measures
    // `band + n x row`.
    let rows: Vec<usize> =
        SECTIONS.iter().map(|&(_, first, last)| (last - first).div_ceil(BOOK_COLUMNS)).collect();
    let total_rows: usize = rows.iter().sum();

    // The bottom of the map takes back **the air that precedes a portion
    // title**, so that the last row of books breathes like the others. Without
    // it, the Writings stopped 21 units from the edge when a portion gets 94:
    // the bottom of the page looked cut off.
    //
    // The top needs no such treatment: the first section already carries its
    // own `SECTION_LEAD` above its title.
    let full = Rect {
        x: WORLD_MARGIN,
        y: WORLD_MARGIN,
        w: WORLD.w - 2.0 * WORLD_MARGIN,
        h: WORLD.h - WORLD_MARGIN - lead,
    };
    let fixed = 3.0 * band;
    let row = ((full.h - fixed) / total_rows as f32).max(1.0);

    // --- the width of a book column
    //
    // It is not given by the world but **by the chapter block**. The height of a
    // box is already fixed: it is `row` minus the two `BOOK_GAP` margins, and
    // that is where the pitch and the band come from. So we solve the vertical
    // first, then cut the column so that the widest block — Tehilim's seven rows
    // of 22 — fits in it with a tenth of air.
    //
    // The world keeps its proportions: the book area is simply centred inside
    // it, which leaves black on the left and right rather than emptiness inside
    // each box.
    let densest =
        (0..corpus::book_count()).map(|book| corpus::chapters_of(book).len()).max().unwrap_or(1);
    let inner = Rect { h: row - 4.0 * BOOK_GAP, ..full };
    let (pitch, book_band, columns) = chapters_fit(inner, densest, number_fill(numeral));

    let cell = columns as f32 * pitch * (1.0 + BOOK_SLACK) + 4.0 * BOOK_GAP;
    let wide = (cell * BOOK_COLUMNS as f32).min(full.w);
    let area = Rect { x: (WORLD.w - wide) / 2.0, w: wide, ..full };

    let mut sections = [Rect::ZERO; 3];
    let mut section_bands = [Rect::ZERO; 3];
    let mut y = area.y;
    for (i, &count) in rows.iter().enumerate() {
        let h = band + count as f32 * row;
        sections[i] = Rect { x: area.x, y, w: area.w, h };
        y += h;
    }

    // --- the books, in order, inside their strip
    let mut books = vec![Rect::ZERO; corpus::book_count()];
    for (i, &(_, start, end)) in SECTIONS.iter().enumerate() {
        let strip = sections[i];
        // No proportional share here: the section height is computed to hold
        // the **whole** band plus its rows, and trimming it would make book
        // boxes unequal from one section to the next — the Torah, which has
        // only one row, inherited 84 units when the others got 64.
        let head = band.min(strip.h - 1.0);
        section_bands[i] = Rect { h: head, ..strip };
        let body = Rect { y: strip.y + head, h: (strip.h - head).max(1.0), ..strip };

        for (book, rect) in (start..end).zip(grid(end - start, body, Some(BOOK_COLUMNS), false)) {
            books[book] = rect.inset(BOOK_GAP);
        }
    }

    // --- the chapters, in order, inside their book
    //
    // **A single pitch for the 929 chapters of the Tanakh.** A cell therefore
    // has the same size everywhere, and consequently so does a chapter number.
    // It is the densest book — the 150 Psalms, four rows of 38 — that sets it,
    // in width as in height.
    //
    // A book that does not fill its row starts at the right edge and stops
    // where it ends: the spacing stays that of all the others, instead of
    // stretching three chapters over 300 units.
    let mut chapters = vec![Rect::ZERO; corpus::chapter_count()];
    let mut book_bands = vec![Rect::ZERO; corpus::book_count()];

    let bodies: Vec<Rect> = (0..corpus::book_count())
        .map(|book| {
            let inner = books[book].inset(BOOK_GAP);
            let head = header(inner.h, book_band);
            book_bands[book] = Rect { h: head, ..inner };
            Rect { y: inner.y + head, h: (inner.h - head).max(1.0), ..inner }
        })
        .collect();


    for book in 0..corpus::book_count() {
        let range = corpus::chapters_of(book);
        let body = bodies[book];
        // Number of rows: the one that gives the block **the shape of its
        // body**.
        //
        // Taking the minimum — everything on one row as soon as the width
        // allows — put 34 books out of 39 in a long strip, and set two
        // unrelated layouts side by side: Shemot lined up its 40 chapters on
        // one row while Bereshit, right next to it, laid 50 on two, for the
        // sole reason that 50 exceeded the 45 available columns.
        //
        // The pitch being common to the whole Tanakh, the ink laid down is the
        // same whatever the split: only the shape of the block changes. So we
        // keep the one that most resembles the box — the most compact, the best
        // centred, and the one that leaves the least air between two books.
        //
        // At equal ratio, the most rows: two rows of ten beat one of twenty,
        // which stretches across the box.
        // The target shape is that of the densest block — `columns x pitch` —
        // and not that of the box: blocks are bounded in width by the columns,
        // so that is the window they must fit into.
        let shape = columns as f32 * pitch / body.h;
        let drift = |rows: usize| {
            let ratio = range.len().div_ceil(rows) as f32 / rows as f32;
            (ratio / shape).max(shape / ratio)
        };
        let rows = (1..=CHAPTER_ROWS.min(range.len()))
            .rev()
            .filter(|&rows| range.len().div_ceil(rows) <= columns)
            .min_by(|&a, &b| drift(a).total_cmp(&drift(b)))
            .unwrap_or_else(|| range.len().div_ceil(columns));

        // Equal rows: we spread evenly between them. Without this, a book of 50
        // chapters gave one full row of 45 and a stump of 5.
        let columns = range.len().div_ceil(rows);

        // **The block is set to the top right**, where reading starts, and not
        // centred in the body. Centred, a book with few chapters laid its row in
        // the middle of an empty box, far from its title: it read poorly which
        // book it belonged to. Set to the top, every row immediately follows the
        // book title, and the emptiness falls to the bottom of the box.
        let top = body.y;
        let right = body.x + body.w;

        for (i, chapter) in range.enumerate() {
            let slot = Rect {
                x: right - (i % columns + 1) as f32 * pitch,
                y: top + (i / columns) as f32 * pitch,
                w: pitch,
                h: pitch,
            };
            chapters[chapter] = slot.inset(CHAPTER_GAP);
        }
    }

    Layout { sections, books, chapters, section_bands, book_bands }
}

/// Regular grid: **every cell has exactly the same width and the same
/// height.**
///
/// A strip split does give each chapter the same area, but not the same shape:
/// the last row, less filled, stretches its cells over the whole width. Here
/// the number of columns is fixed in advance, and the last row simply leaves
/// emptiness to its left.
///
/// We keep the number of columns that makes the cells **as square as
/// possible**. A chapter has no reading direction of its own: the square is the
/// shape that compares best from one cell to the next, and that makes the most
/// of the space. Book boxes, for their part, always aim for the golden
/// rectangle.
///
/// Cells fill right to left, then downwards — reading order.
fn grid(count: usize, area: Rect, fixed: Option<usize>, square: bool) -> Vec<Rect> {
    let mut out = vec![Rect::ZERO; count];
    if count == 0 || area.w <= 0.0 || area.h <= 0.0 {
        return out;
    }

    // Number of cells per row.
    //
    // We do not force a square grid — 25 chapters in 5 x 5 would fit in a
    // 26-unit square in the middle of a box 300 wide, with cells of 3 units. Nor
    // do we take the biggest possible cell, which lines everything up on a
    // single row as soon as the book has fewer than 23 chapters.
    //
    // We keep **the most balanced arrangement among those that cost almost
    // nothing in cell size**: at most a fifth of the best side. A row of 21
    // cells of 14.3 units thus gives way to two rows of 13.2, and a book of
    // three chapters keeps its single row, whose cells are four times bigger
    // than if it were balanced.
    const KEEP: f32 = 0.80;

    let side_of = |columns: usize| {
        let rows = count.div_ceil(columns);
        (area.w / columns as f32).min(area.h / rows as f32)
    };

    let columns = match fixed {
        // The imposed count does **not** fall back on the number of elements: a
        // row of five books in a grid of seven keeps boxes as wide as the
        // others, and leaves two slots empty. Without this the Torah inherited
        // boxes a third wider than those of the other sections, and the chapter
        // cell was no longer unique.
        Some(n) => n.max(1),
        None => {
            let best = (1..=count).map(side_of).fold(0.0_f32, f32::max);
            // Most balanced first, biggest cell to break the tie.
            (1..=count)
                .filter(|&c| side_of(c) >= best * KEEP)
                .min_by(|&a, &b| {
                    let balance = |c: usize| {
                        let r = count.div_ceil(c) as f32;
                        let c = c as f32;
                        (c / r).max(r / c)
                    };
                    balance(a)
                        .total_cmp(&balance(b))
                        .then(side_of(b).total_cmp(&side_of(a)))
                })
                .unwrap_or(1)
        }
    };
    let rows = count.div_ceil(columns);

    let mut w = area.w / columns as f32;
    let mut h = area.h / rows as f32;

    // **Square** cells, even if that leaves emptiness. Stretching a cell to
    // fill the box would make it flat as soon as a book has few chapters — a
    // one-chapter book would take the whole width over a derisory height. The
    // square always holds: its side is the smaller of the two, so the grid never
    // overflows.
    //
    // Books, for their part, fill their strip: it is the width of the first five
    // that sets the column.
    //
    // **The remaining emptiness goes between the cells**, and not around the
    // block: the grid then spreads over all the available space, up to the
    // gutter of the neighbouring book. A single column, or a single row, has no
    // gap to spread the emptiness into: we centre it.
    let (mut dx, mut dy) = (0.0, 0.0);
    let (mut step_x, mut step_y) = (w, h);
    if square {
        let side = w.min(h);
        step_x = if columns > 1 { (area.w - side) / (columns - 1) as f32 } else { 0.0 };
        step_y = if rows > 1 { (area.h - side) / (rows - 1) as f32 } else { 0.0 };
        if columns == 1 {
            dx = (area.w - side) / 2.0;
        }
        if rows == 1 {
            dy = (area.h - side) / 2.0;
        }
        w = side;
        h = side;
    }

    for (i, slot) in out.iter_mut().enumerate() {
        let column = i % columns;
        let row = i / columns;
        *slot = Rect {
            // Right to left, starting from the right edge.
            x: area.x + area.w - dx - w - column as f32 * step_x,
            y: area.y + dy + row as f32 * step_y,
            w,
            h,
        };
    }

    out
}




#[cfg(test)]
mod tests;
