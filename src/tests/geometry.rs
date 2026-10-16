//! The small pure functions the rest leans on: distances, text column, pull,
//! keys, size of a number.

use super::*;

#[test]
fn a_point_inside_a_box_is_at_no_distance() {
    let r = Rect { x: 10.0, y: 20.0, w: 30.0, h: 40.0 };
    assert_eq!(gap_to(r, 20.0, 30.0), 0.0);
    assert_eq!(horizontal_gap(r, 25.0), 0.0);
    assert_eq!(vertical_gap(r, 25.0), 0.0);
    // Edges count as inside: the tolerance of `locate` must not drop a click
    // landing exactly on a border.
    assert_eq!(horizontal_gap(r, 10.0), 0.0);
    assert_eq!(horizontal_gap(r, 40.0), 0.0);
}

#[test]
fn distance_grows_with_the_offset() {
    let r = Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
    assert_eq!(horizontal_gap(r, -3.0), 3.0);
    assert_eq!(horizontal_gap(r, 14.0), 4.0);
    assert_eq!(vertical_gap(r, -2.0), 2.0);
    assert_eq!(vertical_gap(r, 15.0), 5.0);
    // `gap_to` is a square distance: it only serves to compare.
    assert_eq!(gap_to(r, -3.0, -4.0), 25.0);
    assert!(gap_to(r, 20.0, 0.0) > gap_to(r, 11.0, 0.0));
}

/// **The margin of a cell follows the cell** — 6 % of its side — because the
/// whole rendering is proportional. Fixed at 2 world units, it threw away 40 %
/// of the ink of a small cell.
#[test]
fn the_text_column_is_a_share_of_its_cell() {
    for side in [4.0_f32, 6.3, 8.8, 40.0] {
        let cell = Rect { x: 100.0, y: 200.0, w: side, h: side };
        let col = column(cell);
        let pad = (cell.w - col.w) / 2.0;
        assert_close(pad, side * CELL_PAD, 1e-4, "the margin is not proportional");
        assert_close(col.h, side * (1.0 - 2.0 * CELL_PAD), 1e-4, "the vertical margin differs");
        assert_close(
            cell.x + cell.w / 2.0,
            col.x + col.w / 2.0,
            1e-4,
            "the column is not centred",
        );
    }
}

/// **The pull must beat the drift of the zoom.** One notch multiplies the
/// distance from cursor to target by `factor`; a correction of `1 - 1/factor`
/// compensates it exactly, and anything below lets the target escape.
#[test]
fn the_pull_beats_the_drift_of_one_notch() {
    for factor in [1.0_f32, 1.1, 1.22, 1.4, 1.47, 2.0, 4.0] {
        let k = pull(factor);
        assert!(k >= 1.0 - 1.0 / factor, "the pull lets the target escape at {factor}");
        assert!(k <= 0.75, "the pull goes past its ceiling at {factor}");
        assert!(k > 0.0);
    }
    assert_close(pull(1.0), 0.12, 1e-6, "the convergence margin");
    // A zoom-out notch never pulls beyond the clamp, and never goes negative.
    assert!(pull(0.5) >= 0.0);
}

/// The key is all that crosses the boundary: an offset and a length into the
/// corpus.
#[test]
fn a_key_points_back_into_the_corpus() {
    let name = corpus::book_name(7);
    let (offset, len) = key_of(name);
    assert_eq!(len as usize, name.len());
    assert_eq!(&corpus::text()[offset as usize..(offset + len) as usize], name);
}

/// **A number is worth half a book title.** `chapters_fit` deduces the height
/// of the band from that ratio, so the ratio must hold in the finished layout —
/// under the estimate as under a measurement.
#[test]
fn a_number_is_exactly_half_a_title() {
    for numeral in [ESTIMATED_NUMERAL, 3.0 * MEASURED, 1.0, 2.0] {
        let (wide, tight) = levels(numeral);
        for layout in [&wide, &tight] {
            let number = number_in(layout, numeral);
            let band = layout.book_bands[0].h;
            assert_close(
                number / band,
                treemap::CHAPTER_OVER_BOOK,
                1e-3,
                "number/band title ratio",
            );
            // It reads in full or not at all: never wider than its cell, never
            // taller.
            let cell = layout.chapters[0];
            assert!(number * numeral <= cell.w * treemap::NUMBER_WIDTH + 1e-4);
            assert!(number <= cell.h * treemap::NUMBER_HEIGHT + 1e-4);
        }
    }
}

/// The two levels of detail are built on the same world, which is what keeps
/// the camera valid when they are swapped.
#[test]
fn the_two_levels_share_one_world() {
    let (wide, tight) = levels(ESTIMATED_NUMERAL);
    assert_eq!(wide.chapters.len(), tight.chapters.len());
    assert!(tight.chapters[0].w > wide.chapters[0].w);
    for layout in [&wide, &tight] {
        for r in &layout.books {
            assert!(r.x >= WORLD.x && r.x + r.w <= WORLD.x + WORLD.w + 0.01);
            assert!(r.y >= WORLD.y && r.y + r.h <= WORLD.y + WORLD.h + 0.01);
        }
    }
}

/// A number of chapters never reads wider than its cell, whatever the digits
/// the page comes back with.
#[test]
fn a_measured_numeral_never_overflows_its_cell() {
    let mut app = reference_app();
    measure_all(&mut app);
    let widest = app.widest_numeral();
    let size = number_in(&app.layout, widest);
    let cell = app.layout.chapters[0];
    assert!(widest * size <= cell.w, "the widest number overflows its cell");
}
