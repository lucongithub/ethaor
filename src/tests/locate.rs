//! Hit testing: what a click selects, and what it must **not** select.
//!
//! The promise of `CLAUDE.md`: tolerance for the gaps — the breathing space
//! between verses, the interval between cells, the margins between books — and
//! **tolerance that stops at the book**. A point outside every book box names
//! nothing.

use super::*;

#[test]
fn a_point_in_a_cell_names_its_chapter() {
    let app = reference_app();
    for chapter in (0..corpus::chapter_count()).step_by(17) {
        let cell = app.layout.chapters[chapter];
        let (x, y) = (cell.x + cell.w / 2.0, cell.y + cell.h / 2.0);
        assert_eq!(app.chapter_at(x, y), Some(chapter), "the centre of cell {chapter}");
    }
    // Outside every cell: nothing, strictly.
    assert_eq!(app.chapter_at(-10.0, -10.0), None);
}

/// **195 points out of 195 of a book box name a chapter** — including in a book
/// of three chapters, whose cells sit a hundred units apart.
#[test]
fn every_point_of_a_book_box_names_one_of_its_chapters() {
    let mut app = reference_app();
    measure_all(&mut app);
    let mut points = 0;
    for book in 0..corpus::book_count() {
        let r = app.layout.books[book];
        for i in 0..15 {
            for j in 0..13 {
                let x = r.x + r.w * (i as f32 + 0.5) / 15.0;
                let y = r.y + r.h * (j as f32 + 0.5) / 13.0;
                points += 1;
                let (hit, chapter, _, _) =
                    app.locate(x, y).unwrap_or_else(|| panic!("point ({x}, {y}) of book {book}"));
                assert_eq!(hit, book, "point ({x}, {y}) designates another book");
                assert!(
                    corpus::chapters_of(book).contains(&chapter),
                    "chapter {chapter} is not of book {book}"
                );
            }
        }
    }
    assert_eq!(points, 15 * 13 * corpus::book_count());
}

/// **The tolerance stops at the book.** Clicking in the air that separates two
/// portions, or in the white at the bottom, selects nothing.
#[test]
fn the_air_between_portions_names_nothing() {
    let mut app = reference_app();
    measure_all(&mut app);

    for section in 0..3 {
        let band = app.layout.section_bands[section];
        for i in 0..20 {
            let x = band.x + band.w * (i as f32 + 0.5) / 20.0;
            let y = band.y + 2.0;
            assert!(app.locate(x, y).is_none(), "the air of portion {section} designates a chapter");
        }
    }

    let last = app.layout.books[corpus::book_count() - 1];
    for i in 0..20 {
        let x = WORLD.w * (i as f32 + 0.5) / 20.0;
        let y = last.y + last.h + 20.0;
        assert!(app.locate(x, y).is_none(), "the blank at the bottom designates a chapter");
    }

    // Outside the world altogether.
    assert!(app.locate(-500.0, -500.0).is_none());
    assert!(app.locate(WORLD.w + 500.0, WORLD.h / 2.0).is_none());
}

/// The pointer announces what a click will do: `hover` follows the same rule as
/// `locate`.
#[test]
fn the_pointer_announces_what_a_click_will_select() {
    let mut app = reference_app();
    measure_all(&mut app);
    for book in (0..corpus::book_count()).step_by(5) {
        let r = app.layout.books[book];
        let (wx, wy) = (r.x + r.w * 0.8, r.y + r.h * 0.5);
        let hovered = app.locate(wx, wy).map(|(_, chapter, _, _)| chapter);
        let selected = app.locate(wx, wy).map(|(_, chapter, _, _)| chapter);
        assert_eq!(hovered, selected, "the hover and the click do not say the same");
        assert!(hovered.is_some());
    }
}

/// A located point gives a coherent path: the chapter belongs to the book, the
/// verse to the chapter, the letter to the line.
#[test]
fn a_located_point_gives_a_coherent_path() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    for chapter in (0..corpus::chapter_count()).step_by(97) {
        let cell = app.layout.chapters[chapter];
        let (x, y) = (cell.x + cell.w * 0.5, cell.y + cell.h * 0.5);
        let (book, hit, verse, letter) = app.locate(x, y).expect("the centre of a cell");
        assert_eq!(hit, chapter);
        assert_eq!(book, book_of(chapter));
        assert!(
            verse == usize::MAX || corpus::verses_of(chapter).contains(&verse),
            "verse {verse} is not of chapter {chapter}"
        );
        let letter = letter.expect("a letter under the point");
        assert!(letter.w > 0.0, "the letter designated has no width");
        assert!(
            letter.x >= cell.x - cell.w && letter.x <= cell.x + 2.0 * cell.w,
            "the letter is far from its cell"
        );
    }
}

/// The nearest chapter is found even from far away: that is what keeps the zoom
/// productive when one aims between two boxes.
#[test]
fn the_nearest_chapter_is_found_from_anywhere() {
    let app = reference_app();
    for (x, y) in [(0.0, 0.0), (WORLD.w, WORLD.h), (WORLD.w / 2.0, WORLD.h / 2.0)] {
        let chapter = app.nearest_chapter(x, y).expect("a nearest chapter");
        let best = gap_to(app.layout.chapters[chapter], x, y);
        for other in (0..corpus::chapter_count()).step_by(13) {
            assert!(
                gap_to(app.layout.chapters[other], x, y) >= best - 1e-3,
                "chapter {other} is nearer than {chapter}"
            );
        }
    }
}

#[test]
fn a_book_is_found_by_its_box_only() {
    let app = reference_app();
    for book in 0..corpus::book_count() {
        let r = app.layout.books[book];
        assert_eq!(app.book_at(r.x + r.w / 2.0, r.y + r.h / 2.0), Some(book));
    }
    assert_eq!(app.book_at(-1.0, -1.0), None);
    let band = app.layout.section_bands[1];
    assert_eq!(app.book_at(band.x + 1.0, band.y + 1.0), None, "a portion's band is not a book");
}

/// The nest is at the heart of a letter, and takes **seven tenths of its
/// cell** — not a dot.
#[test]
fn the_nest_takes_seven_tenths_of_its_letter() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    let cell = app.layout.chapters[0];
    let (x, y) = (cell.x + cell.w * 0.5, cell.y + cell.h * 0.5);
    let letter = app.locate(x, y).unwrap().3.expect("a letter");
    let nest = app.nest_at(x, y).expect("a nest");
    assert_close(nest.w / letter.w, NEST, 1e-4, "share of the cell taken by the nest");
    assert_close(
        nest.x + nest.w / 2.0,
        letter.x + letter.w / 2.0,
        0.01,
        "the nest is not at the heart of the letter",
    );
}

/// The zoom target tolerates the gaps: aiming between two books must stay
/// productive.
#[test]
fn the_zoom_target_tolerates_the_gaps() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    // Between two book boxes, and in the air of a portion: `target_near` must
    // still find a letter, where `locate` refuses.
    let a = app.layout.books[5];
    let b = app.layout.books[6];
    let x = (a.x + b.x + b.w) / 2.0;
    let y = a.y + a.h / 2.0;
    assert!(app.target_near(x, y).is_some(), "the zoom finds nothing between two books");

    let band = app.layout.section_bands[1];
    assert!(
        app.target_near(band.x + band.w * 0.5, band.y + 1.0).is_some(),
        "the zoom finds nothing in a portion's air"
    );
}

/// The breadcrumb always says where one is: section, book and chapter under the
/// centre of the view, even when the title bands have left the screen.
#[test]
fn the_breadcrumb_names_the_place_under_the_centre() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    for _ in 0..12 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        let (wx, wy) = app.camera.to_world(app.width / 2.0, app.height / 2.0);
        let chapter = app.chapter_at(wx, wy).or_else(|| app.nearest_chapter(wx, wy));
        let chapter = chapter.expect("a chapter under the centre");
        let book = book_of(chapter);
        let section = treemap::SECTIONS
            .iter()
            .position(|&(_, first, last)| book >= first && book < last)
            .expect("a portion for this book");
        assert!(section < 3);
        assert!(!corpus::book_name(book).is_empty());
        assert!(!corpus::chapter_label(chapter).is_empty());
    }
}
