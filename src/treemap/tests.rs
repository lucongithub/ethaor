//! The rules of the map, as `CLAUDE.md` states them.
//!
//! They are rules and not settings: **a single pitch for the 929 chapters**,
//! book boxes all identical, canonical order kept, the block set to the top
//! right, the air of a portion as the only adjustable value. Each one is an
//! assertion here, so that a change of taste cannot quietly break one.

use super::*;

const TOL: f32 = 0.01;

/// The estimated digit width the layout starts from, and the one a Hebrew
/// serif really gives — the two states the browser goes through.
const ESTIMATED: f32 = 3.0 * 0.42;
const MEASURED: f32 = 3.0 * 0.58;

fn wide() -> Layout {
    build(MEASURED, SECTION_LEAD_WIDE)
}

fn tight() -> Layout {
    build(MEASURED, SECTION_LEAD_TIGHT)
}

fn rows_of(layout: &Layout, book: usize) -> Vec<Vec<Rect>> {
    let mut rows: Vec<Vec<Rect>> = Vec::new();
    for chapter in corpus::chapters_of(book) {
        let cell = layout.chapters[chapter];
        match rows.last_mut() {
            Some(row) if (row[0].y - cell.y).abs() < TOL => row.push(cell),
            _ => rows.push(vec![cell]),
        }
    }
    rows
}

// --------------------------------------------------------------------- Rect

#[test]
fn contains_holds_the_right_edge_open() {
    let r = Rect { x: 10.0, y: 20.0, w: 4.0, h: 6.0 };
    assert!(r.contains(10.0, 20.0));
    assert!(r.contains(13.9, 25.9));
    assert!(!r.contains(14.0, 25.0), "the right edge must be excluded");
    assert!(!r.contains(13.0, 26.0), "the bottom edge must be excluded");
    assert!(!r.contains(9.9, 20.0));
    assert!(!Rect::ZERO.contains(0.0, 0.0));
}

#[test]
fn shrink_keeps_the_centre() {
    let r = Rect { x: 10.0, y: 20.0, w: 4.0, h: 8.0 };
    let s = r.shrink(0.5);
    assert_eq!((s.w, s.h), (2.0, 4.0));
    assert_eq!(r.x + r.w / 2.0, s.x + s.w / 2.0);
    assert_eq!(r.y + r.h / 2.0, s.y + s.h / 2.0);
    assert_eq!(r.shrink(1.0), r);
}

#[test]
fn inset_never_turns_a_box_inside_out() {
    let r = Rect { x: 0.0, y: 0.0, w: 10.0, h: 4.0 };
    let i = r.inset(1.0);
    assert_eq!(i, Rect { x: 1.0, y: 1.0, w: 8.0, h: 2.0 });
    // Beyond half the smallest side, the pad is clamped: the box shrinks to a
    // segment but keeps a positive size.
    let flat = r.inset(50.0);
    assert!(flat.w >= 0.0 && flat.h >= 0.0, "inset turned the box over");
    assert_eq!(r.inset(-5.0), r, "a negative pad must do nothing");
}

// ------------------------------------------------------------------ numbers

/// A number is bounded by the height of its cell **and** by its width: if the
/// digits are wider than the estimate, width is what decides.
#[test]
fn number_fill_is_bounded_by_height_then_by_width() {
    assert_eq!(number_fill(0.1), NUMBER_HEIGHT, "narrow digits are bound by the height");
    assert!(number_fill(MEASURED) < number_fill(ESTIMATED), "wider digits, a smaller body");
    assert!(number_fill(100.0) > 0.0, "the share must never vanish");
    assert!(number_fill(0.0).is_finite(), "a zero width must not diverge");
    for numeral in [0.5, 1.0, 1.26, 1.74, 3.0, 10.0] {
        let fill = number_fill(numeral);
        assert!(fill <= NUMBER_HEIGHT + f32::EPSILON);
        assert!(fill * numeral <= NUMBER_WIDTH + 1e-6, "the number would overflow in width");
    }
}

// ------------------------------------------------------------- band / pitch

/// Band, pitch and columns are **deduced together**: the title fills its band,
/// the number fills its cell, and the number is worth half the title.
#[test]
fn chapters_fit_keeps_the_number_at_half_the_title() {
    let inner = Rect { x: 0.0, y: 0.0, w: 1580.0, h: 52.0 };
    for numeral in [ESTIMATED, MEASURED, 1.0, 2.5] {
        let fill = number_fill(numeral);
        let (pitch, band, columns) = chapters_fit(inner, 150, fill);
        assert!(pitch > 0.0 && band > 0.0);
        assert_eq!(columns, 150usize.div_ceil(CHAPTER_ROWS), "the columns come from the densest book");
        // The body is what the band leaves, shared between the rows.
        assert!(
            band + pitch * CHAPTER_ROWS as f32 <= inner.h + TOL,
            "band and rows overflow the box"
        );
        // The number — the cell times its fill — is half the title, which fills
        // the band.
        let number = (pitch - 2.0 * CHAPTER_GAP) * fill;
        assert!(
            (number / band - CHAPTER_OVER_BOOK).abs() < 1e-3,
            "number/title ratio {} for a numeral of {numeral}",
            number / band
        );
    }
}

/// The band is **never** trimmed to a share of the box: a bound at `h / 2`
/// gave the Torah, alone with a single row, boxes far taller than the others.
#[test]
fn the_band_is_only_trimmed_by_a_box_too_low_to_hold_it() {
    assert_eq!(header(100.0, 9.0), 9.0, "a tall box keeps its whole band");
    assert!(header(10.0, 9.0) < 9.0, "a low box trims its band");
    assert!((header(10.0, 9.0) - 3.3).abs() < 1e-5);
}

// ------------------------------------------------------------------- grid

#[test]
fn an_empty_grid_has_no_cell() {
    assert!(grid(0, WORLD, None, false).is_empty());
    assert_eq!(grid(3, Rect::ZERO, None, false), vec![Rect::ZERO; 3]);
}

/// An imposed column count does **not** fall back on the number of elements: a
/// row of five books in a grid of five keeps boxes the width of the others.
#[test]
fn an_imposed_grid_keeps_its_cells_identical() {
    let area = Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 };
    let cells = grid(3, area, Some(5), false);
    assert_eq!(cells.len(), 3);
    assert!(cells.iter().all(|c| (c.w - 20.0).abs() < TOL));
    // Right to left, from the right edge.
    assert_eq!(cells[0].x, 80.0);
    assert_eq!(cells[1].x, 60.0);
    assert_eq!(cells[2].x, 40.0);
    assert!(cells.iter().all(|c| c.y == 0.0), "three elements fit on one row");
}

/// Reading order: right to left on a row, then the next row **below** — never
/// above.
#[test]
fn a_grid_fills_right_to_left_then_downwards() {
    let area = Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };
    let cells = grid(7, area, Some(3), false);
    for pair in cells.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(b.y > a.y - TOL, "a cell went up");
        if (a.y - b.y).abs() < TOL {
            assert!(b.x < a.x, "the row does not go right to left");
        }
    }
}

#[test]
fn a_square_grid_never_overflows_its_area() {
    let area = Rect { x: 5.0, y: 7.0, w: 90.0, h: 30.0 };
    for count in 1..40 {
        for cell in grid(count, area, None, true) {
            assert!((cell.w - cell.h).abs() < TOL, "the cell is not square");
            assert!(cell.x >= area.x - TOL && cell.x + cell.w <= area.x + area.w + TOL);
            assert!(cell.y >= area.y - TOL && cell.y + cell.h <= area.y + area.h + TOL);
        }
    }
}

// ------------------------------------------------------------------- build

#[test]
fn the_layout_covers_the_whole_corpus() {
    let l = wide();
    assert_eq!(l.books.len(), corpus::book_count());
    assert_eq!(l.book_bands.len(), corpus::book_count());
    assert_eq!(l.chapters.len(), corpus::chapter_count());
    assert!(l.books.iter().all(|r| r.w > 0.0 && r.h > 0.0), "a book without area");
    assert!(l.chapters.iter().all(|r| r.w > 0.0 && r.h > 0.0), "a chapter without area");
}

/// The 39 books sit on a grid of five columns — the one the Torah imposes —
/// which gives nine rows: one, five, three.
#[test]
fn the_books_sit_on_a_grid_of_five_columns() {
    let l = wide();
    for &(_, first, last) in &SECTIONS {
        let mut per_row: Vec<usize> = Vec::new();
        let mut y: Option<f32> = None;
        for book in first..last {
            let r = l.books[book];
            if y.is_none_or(|y| (r.y - y).abs() > TOL) {
                y = Some(r.y);
                per_row.push(0);
            }
            *per_row.last_mut().unwrap() += 1;
        }
        assert!(per_row.iter().all(|&n| n <= 5), "more than five books on a row");
        assert_eq!(per_row.len(), (last - first).div_ceil(5), "wrong number of rows");
    }
    assert_eq!(SECTIONS[0].2 - SECTIONS[0].1, 5, "the Torah must fill its row");
}

/// **All book boxes are identical**, from one section to the next: that is what
/// the height of a section is computed for.
#[test]
fn every_book_box_is_the_same_size() {
    for layout in [wide(), tight()] {
        let first = layout.books[0];
        for (book, r) in layout.books.iter().enumerate() {
            assert!((r.w - first.w).abs() < TOL, "width of book {book}");
            assert!((r.h - first.h).abs() < TOL, "height of book {book}");
        }
    }
}

/// **A single pitch for the 929 chapters.** A cell has exactly the same size
/// everywhere, from one end of the corpus to the other — and so, therefore,
/// does a chapter number. It is a rule, not a setting.
#[test]
fn every_chapter_cell_is_the_same_size() {
    for layout in [wide(), tight()] {
        let first = layout.chapters[0];
        for (chapter, r) in layout.chapters.iter().enumerate() {
            assert!((r.w - first.w).abs() < 1e-3, "width of cell {chapter}");
            assert!((r.h - first.h).abs() < 1e-3, "height of cell {chapter}");
        }
        assert!((first.w - first.h).abs() < 1e-3, "a cell must be square");
    }
}

/// A chapter stays in its book, a book in its section, a section in the world.
#[test]
fn the_hierarchy_nests_geometrically() {
    let l = wide();
    let inside = |inner: Rect, outer: Rect, what: &str| {
        assert!(inner.x >= outer.x - TOL, "{what}: left edge");
        assert!(inner.y >= outer.y - TOL, "{what}: top edge");
        assert!(inner.x + inner.w <= outer.x + outer.w + TOL, "{what}: right edge");
        assert!(inner.y + inner.h <= outer.y + outer.h + TOL, "{what}: bottom edge");
    };

    for (section, &(_, first, last)) in SECTIONS.iter().enumerate() {
        inside(l.sections[section], WORLD, "a section leaves the world");
        inside(l.section_bands[section], l.sections[section], "a band leaves its section");
        for book in first..last {
            inside(l.books[book], l.sections[section], &format!("book {book}"));
            inside(l.book_bands[book], l.books[book], &format!("the band of book {book}"));
            for chapter in corpus::chapters_of(book) {
                inside(l.chapters[chapter], l.books[book], &format!("chapter {chapter}"));
            }
        }
    }
}

/// A chapter never bites into the title band of its book: that is what the band
/// is reserved for.
#[test]
fn no_chapter_enters_its_book_band() {
    let l = wide();
    for book in 0..corpus::book_count() {
        let band = l.book_bands[book];
        for chapter in corpus::chapters_of(book) {
            let cell = l.chapters[chapter];
            assert!(
                cell.y >= band.y + band.h - TOL,
                "chapter {chapter} goes up into the band of book {book}"
            );
        }
    }
}

/// Cells never overlap, and they follow reading order: right to left, then the
/// row below.
#[test]
fn cells_do_not_overlap_and_keep_reading_order() {
    let l = wide();
    for book in 0..corpus::book_count() {
        let cells: Vec<Rect> = corpus::chapters_of(book).map(|c| l.chapters[c]).collect();
        for i in 0..cells.len() {
            for j in i + 1..cells.len() {
                let (a, b) = (cells[i], cells[j]);
                let apart = a.x + a.w <= b.x + TOL
                    || b.x + b.w <= a.x + TOL
                    || a.y + a.h <= b.y + TOL
                    || b.y + b.h <= a.y + TOL;
                assert!(apart, "chapters {i} and {j} of book {book} overlap");
            }
        }
        for pair in cells.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(b.y > a.y - TOL, "a chapter of book {book} went up");
            if (a.y - b.y).abs() < TOL {
                assert!(b.x < a.x, "the row of book {book} does not go right to left");
            }
        }
    }
}

/// The block is **set to the top right**, where reading starts: every book puts
/// its first row at the same distance from the top of its box, and the
/// emptiness falls to the bottom.
#[test]
fn every_block_hangs_from_the_top_right_of_its_box() {
    let l = wide();
    let offsets: Vec<f32> = (0..corpus::book_count())
        .map(|book| {
            let first = corpus::chapters_of(book).start;
            l.chapters[first].y - l.books[book].y
        })
        .collect();
    let head = offsets[0];
    for (book, &offset) in offsets.iter().enumerate() {
        assert!((offset - head).abs() < TOL, "the block of book {book} does not hang from the top");
    }
    // Right edge: the first cell of every row ends at the same distance from
    // the right edge of the box.
    for book in 0..corpus::book_count() {
        let box_ = l.books[book];
        for row in rows_of(&l, book) {
            let cell = row[0];
            assert!(
                (box_.x + box_.w - (cell.x + cell.w)) < box_.w * 0.1,
                "the row of book {book} does not start at the right edge"
            );
        }
    }
}

/// The number of rows is the one that gives the block **the shape of its
/// body**, never the minimum — and never more than `CHAPTER_ROWS`.
#[test]
fn the_rows_follow_the_shape_of_the_box() {
    let l = wide();
    let mut many = 0;
    for book in 0..corpus::book_count() {
        let rows = rows_of(&l, book);
        let count = corpus::chapters_of(book).len();
        assert!(rows.len() <= CHAPTER_ROWS, "book {book} goes past {CHAPTER_ROWS} rows");
        assert!(!rows.is_empty());
        // Equal rows: the chapters are spread evenly, and the last row is
        // never a stump — a book of 50 chapters gave a full row of 45 and a
        // stump of 5 before.
        let widest = rows.iter().map(|r| r.len()).max().unwrap();
        let shortest = rows.iter().map(|r| r.len()).min().unwrap();
        assert_eq!(widest, count.div_ceil(rows.len()), "book {book} is not split into equal parts");
        assert!(shortest >= 1, "book {book} has an empty row");
        assert!(
            2 * shortest >= widest,
            "book {book} ends on a stub: {shortest} against {widest}"
        );
        assert_eq!(rows.iter().map(|r| r.len()).sum::<usize>(), count);
        if rows.len() > 1 {
            many += 1;
        } else {
            assert!(count <= 5, "book {book} keeps a single row with {count} chapters");
        }
    }
    assert_eq!(many, 29, "29 books of 39 take two rows or more");
}

/// The densest book — the 150 Psalms — is what sets the grid: **seven rows of
/// 22**.
#[test]
fn the_psalms_hold_seven_rows_of_twenty_two() {
    let l = wide();
    let psalms = (0..corpus::book_count()).find(|&b| corpus::book_name(b) == "תהילים").unwrap();
    let rows = rows_of(&l, psalms);
    assert_eq!(rows.len(), 7);
    assert_eq!(rows[0].len(), 22);
    assert_eq!(rows.last().unwrap().len(), 150 - 6 * 22);
}

/// The width of a column comes from the chapter block, not from the world: the
/// block takes some nine tenths of its box, and the map is centred in the
/// world.
#[test]
fn a_book_column_is_cut_on_its_chapter_block() {
    let l = wide();
    let box_ = l.books[0];
    // The block is counted at its pitch — cell plus its breathing space —
    // since that is what the column was cut on.
    let pitch = l.chapters[0].w + 2.0 * CHAPTER_GAP;
    let share = 22.0 * pitch / box_.w;
    assert!(share > 0.85 && share < 1.0, "the block takes {share} of its box");

    let left = l.books.iter().map(|r| r.x).fold(f32::MAX, f32::min);
    let right = l.books.iter().map(|r| r.x + r.w).fold(0.0, f32::max);
    assert!(
        ((left - WORLD.x) - (WORLD.x + WORLD.w - right)).abs() < 1.0,
        "the books' area is not centred in the world"
    );
    assert!(right - left < WORLD.w, "the map must leave black on the sides");
}

/// Sections are flush and stacked in order; what separates them is the air of
/// their band.
#[test]
fn the_three_sections_are_stacked_and_flush() {
    let l = wide();
    for pair in l.sections.windows(2) {
        assert!((pair[0].y + pair[0].h - pair[1].y).abs() < TOL, "the sections do not touch");
    }
    assert!(l.sections[0].y >= WORLD.y);
    for section in 0..3 {
        assert_eq!(l.section_bands[section].y, l.sections[section].y, "the band is not at the top");
        assert!(l.section_bands[section].h > SECTION_LEAD_TIGHT, "the band lost its air");
    }
}

/// Measured: 4.0 world units between two rows of a same block, 122.0 between
/// two sections — 4 of gutter, 90 of air, 28 for the title line. Thirty times
/// the gutter: a portion is confused with nothing.
#[test]
fn a_portion_is_separated_by_thirty_times_the_gutter() {
    let l = wide();
    // Two rows of a same section: the Prophets, whose 21 books take five.
    let gutter = {
        let first = l.books[SECTIONS[1].1];
        let below = l.books[SECTIONS[1].1 + 5];
        below.y - (first.y + first.h)
    };
    assert!((gutter - 2.0 * BOOK_GAP).abs() < TOL, "gouttiere de {gutter}");

    let torah_last = l.books[4];
    let next_section_first = l.books[SECTIONS[1].1];
    let apart = next_section_first.y - (torah_last.y + torah_last.h);
    let expected = 2.0 * BOOK_GAP + SECTION_LEAD_WIDE + SECTION_TITLE;
    assert!((apart - expected).abs() < 1.0, "{apart} units between two portions, expected {expected}");
    assert!(apart > 30.0 * gutter, "a portion must stand apart from a row");
}

/// The bottom of the map takes back the air that precedes a portion title, so
/// the last row of books breathes like the others. Measured: 92 world units,
/// exactly `SECTION_LEAD + WORLD_MARGIN`.
#[test]
fn the_bottom_of_the_map_keeps_the_air_of_a_portion() {
    for (lead, layout) in [(SECTION_LEAD_WIDE, wide()), (SECTION_LEAD_TIGHT, tight())] {
        let last = layout.books[corpus::book_count() - 1];
        let gap = WORLD.h - (last.y + last.h);
        assert!(
            (gap - (lead + BOOK_GAP)).abs() < 1.0,
            "{gap} units under the last book for an air of {lead}"
        );
    }
}

/// **The air of a portion is the only setting of the map, and it is paid for in
/// cells.** Tight air gives boxes a third taller and cells a third bigger — the
/// level of detail where the chapters are read.
#[test]
fn tight_air_gives_bigger_cells() {
    let (w, t) = (wide(), tight());
    assert!(t.books[0].h > w.books[0].h, "the tightened air must grow the boxes");
    assert!(t.chapters[0].w > w.chapters[0].w, "the tightened air must grow the cells");
    let gain = t.chapters[0].w / w.chapters[0].w;
    assert!(gain > 1.25 && gain < 1.5, "gain of {gain} on the cell");
    // The world keeps its proportions, so the camera stays valid across the
    // swap: the two layouts live in the same world.
    assert_eq!(w.books.len(), t.books.len());
    assert_eq!(w.chapters.len(), t.chapters.len());
}

/// Wider digits tighten the band and grow the cell: the two settings are the
/// same one.
#[test]
fn measured_digits_tighten_the_band_and_grow_the_cell() {
    let estimated = build(ESTIMATED, SECTION_LEAD_WIDE);
    let measured = build(MEASURED, SECTION_LEAD_WIDE);
    assert!(measured.book_bands[0].h < estimated.book_bands[0].h, "the band should have tightened");
    assert!(measured.chapters[0].h > estimated.chapters[0].h, "the cell should have grown");
}

/// A degenerate call must not panic nor produce anything negative: the page
/// could ask for a frame before the window has a size.
#[test]
fn the_layout_survives_absurd_inputs() {
    for numeral in [0.0, 0.001, 1e6] {
        for lead in [0.0, SECTION_LEAD_WIDE, 1e4] {
            let l = build(numeral, lead);
            assert_eq!(l.chapters.len(), corpus::chapter_count());
            assert!(l.chapters.iter().all(|r| r.w >= 0.0 && r.h >= 0.0 && r.w.is_finite()));
            assert!(l.books.iter().all(|r| r.w >= 0.0 && r.h >= 0.0 && r.w.is_finite()));
        }
    }
}

