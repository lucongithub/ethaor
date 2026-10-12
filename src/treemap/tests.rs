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
    assert!(!r.contains(14.0, 25.0), "le bord droit doit etre exclu");
    assert!(!r.contains(13.0, 26.0), "le bord bas doit etre exclu");
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
    assert!(flat.w >= 0.0 && flat.h >= 0.0, "inset a retourne la boite");
    assert_eq!(r.inset(-5.0), r, "un pad negatif ne doit rien faire");
}

// ------------------------------------------------------------------ numbers

/// A number is bounded by the height of its cell **and** by its width: if the
/// digits are wider than the estimate, width is what decides.
#[test]
fn number_fill_is_bounded_by_height_then_by_width() {
    assert_eq!(number_fill(0.1), NUMBER_HEIGHT, "des chiffres etroits butent sur la hauteur");
    assert!(number_fill(MEASURED) < number_fill(ESTIMATED), "des chiffres plus larges, un corps plus petit");
    assert!(number_fill(100.0) > 0.0, "la part ne doit jamais s'annuler");
    assert!(number_fill(0.0).is_finite(), "une largeur nulle ne doit pas diverger");
    for numeral in [0.5, 1.0, 1.26, 1.74, 3.0, 10.0] {
        let fill = number_fill(numeral);
        assert!(fill <= NUMBER_HEIGHT + f32::EPSILON);
        assert!(fill * numeral <= NUMBER_WIDTH + 1e-6, "le numero deborderait en largeur");
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
        assert_eq!(columns, 150usize.div_ceil(CHAPTER_ROWS), "les colonnes viennent du livre le plus dense");
        // The body is what the band leaves, shared between the rows.
        assert!(
            band + pitch * CHAPTER_ROWS as f32 <= inner.h + TOL,
            "bande et rangees debordent la boite"
        );
        // The number — the cell times its fill — is half the title, which fills
        // the band.
        let number = (pitch - 2.0 * CHAPTER_GAP) * fill;
        assert!(
            (number / band - CHAPTER_OVER_BOOK).abs() < 1e-3,
            "rapport numero/titre {} pour un numeral de {numeral}",
            number / band
        );
    }
}

/// The band is **never** trimmed to a share of the box: a bound at `h / 2`
/// gave the Torah, alone with a single row, boxes far taller than the others.
#[test]
fn the_band_is_only_trimmed_by_a_box_too_low_to_hold_it() {
    assert_eq!(header(100.0, 9.0), 9.0, "une boite haute garde toute sa bande");
    assert!(header(10.0, 9.0) < 9.0, "une boite basse rogne sa bande");
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
    assert!(cells.iter().all(|c| c.y == 0.0), "trois elements tiennent sur une rangee");
}

/// Reading order: right to left on a row, then the next row **below** — never
/// above.
#[test]
fn a_grid_fills_right_to_left_then_downwards() {
    let area = Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };
    let cells = grid(7, area, Some(3), false);
    for pair in cells.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(b.y > a.y - TOL, "une cellule est remontee");
        if (a.y - b.y).abs() < TOL {
            assert!(b.x < a.x, "la rangee ne va pas de droite a gauche");
        }
    }
}

#[test]
fn a_square_grid_never_overflows_its_area() {
    let area = Rect { x: 5.0, y: 7.0, w: 90.0, h: 30.0 };
    for count in 1..40 {
        for cell in grid(count, area, None, true) {
            assert!((cell.w - cell.h).abs() < TOL, "la case n'est pas carree");
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
    assert!(l.books.iter().all(|r| r.w > 0.0 && r.h > 0.0), "un livre sans surface");
    assert!(l.chapters.iter().all(|r| r.w > 0.0 && r.h > 0.0), "un chapitre sans surface");
}

/// The 39 books sit on a grid of five columns — the one the Torah imposes —
/// which gives nine rows: one, five, three.
#[test]
fn the_books_sit_on_a_grid_of_five_columns() {
    let l = wide();
    for section in 0..3 {
        let (_, first, last) = SECTIONS[section];
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
        assert!(per_row.iter().all(|&n| n <= 5), "plus de cinq livres sur une rangee");
        assert_eq!(per_row.len(), (last - first).div_ceil(5), "mauvais nombre de rangees");
    }
    assert_eq!(SECTIONS[0].2 - SECTIONS[0].1, 5, "la Torah doit remplir sa rangee");
}

/// **All book boxes are identical**, from one section to the next: that is what
/// the height of a section is computed for.
#[test]
fn every_book_box_is_the_same_size() {
    for layout in [wide(), tight()] {
        let first = layout.books[0];
        for (book, r) in layout.books.iter().enumerate() {
            assert!((r.w - first.w).abs() < TOL, "largeur du livre {book}");
            assert!((r.h - first.h).abs() < TOL, "hauteur du livre {book}");
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
            assert!((r.w - first.w).abs() < 1e-3, "largeur de la case {chapter}");
            assert!((r.h - first.h).abs() < 1e-3, "hauteur de la case {chapter}");
        }
        assert!((first.w - first.h).abs() < 1e-3, "une case doit etre carree");
    }
}

/// A chapter stays in its book, a book in its section, a section in the world.
#[test]
fn the_hierarchy_nests_geometrically() {
    let l = wide();
    let inside = |inner: Rect, outer: Rect, what: &str| {
        assert!(inner.x >= outer.x - TOL, "{what}: bord gauche");
        assert!(inner.y >= outer.y - TOL, "{what}: bord haut");
        assert!(inner.x + inner.w <= outer.x + outer.w + TOL, "{what}: bord droit");
        assert!(inner.y + inner.h <= outer.y + outer.h + TOL, "{what}: bord bas");
    };

    for section in 0..3 {
        inside(l.sections[section], WORLD, "une section sort du monde");
        inside(l.section_bands[section], l.sections[section], "une bande sort de sa section");
        let (_, first, last) = SECTIONS[section];
        for book in first..last {
            inside(l.books[book], l.sections[section], &format!("le livre {book}"));
            inside(l.book_bands[book], l.books[book], &format!("la bande du livre {book}"));
            for chapter in corpus::chapters_of(book) {
                inside(l.chapters[chapter], l.books[book], &format!("le chapitre {chapter}"));
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
                "le chapitre {chapter} monte dans la bande du livre {book}"
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
                assert!(apart, "les chapitres {i} et {j} du livre {book} se chevauchent");
            }
        }
        for pair in cells.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(b.y > a.y - TOL, "un chapitre du livre {book} est remonte");
            if (a.y - b.y).abs() < TOL {
                assert!(b.x < a.x, "la rangee du livre {book} ne va pas de droite a gauche");
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
        assert!((offset - head).abs() < TOL, "le bloc du livre {book} ne pend pas du haut");
    }
    // Right edge: the first cell of every row ends at the same distance from
    // the right edge of the box.
    for book in 0..corpus::book_count() {
        let box_ = l.books[book];
        for row in rows_of(&l, book) {
            let cell = row[0];
            assert!(
                (box_.x + box_.w - (cell.x + cell.w)) < box_.w * 0.1,
                "la rangee du livre {book} ne part pas du bord droit"
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
        assert!(rows.len() <= CHAPTER_ROWS, "le livre {book} depasse {CHAPTER_ROWS} rangees");
        assert!(!rows.is_empty());
        // Equal rows: the chapters are spread evenly, and the last row is
        // never a stump — a book of 50 chapters gave a full row of 45 and a
        // stump of 5 before.
        let widest = rows.iter().map(|r| r.len()).max().unwrap();
        let shortest = rows.iter().map(|r| r.len()).min().unwrap();
        assert_eq!(widest, count.div_ceil(rows.len()), "le livre {book} n'est pas reparti a parts egales");
        assert!(shortest >= 1, "le livre {book} a une rangee vide");
        assert!(
            2 * shortest >= widest,
            "le livre {book} finit sur un moignon : {shortest} contre {widest}"
        );
        assert_eq!(rows.iter().map(|r| r.len()).sum::<usize>(), count);
        if rows.len() > 1 {
            many += 1;
        } else {
            assert!(count <= 5, "le livre {book} garde une seule rangee avec {count} chapitres");
        }
    }
    assert_eq!(many, 29, "29 livres sur 39 passent a deux rangees ou plus");
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
    assert!(share > 0.85 && share < 1.0, "le bloc occupe {share} de sa boite");

    let left = l.books.iter().map(|r| r.x).fold(f32::MAX, f32::min);
    let right = l.books.iter().map(|r| r.x + r.w).fold(0.0, f32::max);
    assert!(
        ((left - WORLD.x) - (WORLD.x + WORLD.w - right)).abs() < 1.0,
        "l'aire des livres n'est pas centree dans le monde"
    );
    assert!(right - left < WORLD.w, "la carte doit laisser du noir sur les cotes");
}

/// Sections are flush and stacked in order; what separates them is the air of
/// their band.
#[test]
fn the_three_sections_are_stacked_and_flush() {
    let l = wide();
    for pair in l.sections.windows(2) {
        assert!((pair[0].y + pair[0].h - pair[1].y).abs() < TOL, "les sections ne sont pas jointives");
    }
    assert!(l.sections[0].y >= WORLD.y);
    for section in 0..3 {
        assert_eq!(l.section_bands[section].y, l.sections[section].y, "la bande n'est pas en haut");
        assert!(l.section_bands[section].h > SECTION_LEAD_TIGHT, "la bande a perdu son air");
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
    assert!((apart - expected).abs() < 1.0, "{apart} unites entre deux portions, attendu {expected}");
    assert!(apart > 30.0 * gutter, "une portion doit se distinguer d'une rangee");
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
            "{gap} unites sous le dernier livre pour une air de {lead}"
        );
    }
}

/// **The air of a portion is the only setting of the map, and it is paid for in
/// cells.** Tight air gives boxes a third taller and cells a third bigger — the
/// level of detail where the chapters are read.
#[test]
fn tight_air_gives_bigger_cells() {
    let (w, t) = (wide(), tight());
    assert!(t.books[0].h > w.books[0].h, "l'air resserre doit grandir les boites");
    assert!(t.chapters[0].w > w.chapters[0].w, "l'air resserre doit grandir les cases");
    let gain = t.chapters[0].w / w.chapters[0].w;
    assert!(gain > 1.25 && gain < 1.5, "gain de {gain} sur la case");
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
    assert!(measured.book_bands[0].h < estimated.book_bands[0].h, "la bande devait se resserrer");
    assert!(measured.chapters[0].h > estimated.chapters[0].h, "la case devait grandir");
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
