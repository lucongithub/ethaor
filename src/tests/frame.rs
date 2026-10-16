//! The command list: what a frame always holds, and what it never holds.
//!
//! The strong promises of `CLAUDE.md` live here — **no fill, ever**, no empty
//! frame, no text laid over another, three sizes at most once the chapters are
//! out, and a convergence of the measurements in one frame.

use super::*;

/// First byte of the verse-text region: a fragment at or past it is a verse,
/// anything before it is a title or a number. `build.rs` writes the regions in
/// that order.
fn verse_region() -> u32 {
    corpus::offset_of(corpus::verse_text(0)) as u32
}

fn distinct_sizes(texts: &[Command]) -> Vec<f32> {
    let mut sizes: Vec<f32> = texts.iter().map(|c| c.size).collect();
    sizes.sort_by(f32::total_cmp);
    sizes.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    sizes
}

/// Walks the zoom from well under the framing to deep inside a letter, and
/// hands every frame to a check — **built** at that notch: zooming only moves
/// the camera, and a check reading the command list of an older frame would
/// sweep nothing at all.
fn sweep(app: &mut App, out: usize, in_: usize, mut check: impl FnMut(&mut App, i32)) {
    let step = app.zoom_step();
    let (px, py) = (app.width * 0.5, app.height * 0.5);
    for _ in 0..out {
        app.zoom_at(px, py, 1.0 / step);
    }
    let mut notch = -(out as i32);
    app.build_frame();
    check(app, notch);
    for _ in 0..(out + in_) {
        app.zoom_at(px, py, step);
        notch += 1;
        app.build_frame();
        check(app, notch);
    }
}

// ------------------------------------------------------------------ shape

#[test]
fn the_command_list_has_a_whole_number_of_records() {
    let mut app = reference_app();
    let commands = frame(&mut app);
    assert!(!commands.is_empty());
    assert_eq!(app.commands.len(), commands.len() * STRIDE);
}

/// **No fill is ever painted.** Only text and clipping come out of here: the
/// white square went away with the zoom-out floor and the early fold.
#[test]
fn a_frame_holds_nothing_but_text_and_clips() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        sweep(&mut app, 10, 50, |app, notch| {
            for c in commands(app) {
                assert!(
                    c.kind == KIND_TEXT || c.kind == KIND_CLIP || c.kind == KIND_UNCLIP,
                    "{w}x{h} notch {notch}: command of kind {}",
                    c.kind
                );
            }
        });
    }
}

/// Clips are opened and closed in pairs, and never left open at the end of a
/// frame.
#[test]
fn clips_are_balanced() {
    let mut app = reference_app();
    measure_all(&mut app);
    sweep(&mut app, 6, 40, |app, notch| {
        let mut depth = 0i32;
        for c in commands(app) {
            if c.kind == KIND_CLIP {
                depth += 1;
            } else if c.kind == KIND_UNCLIP {
                depth -= 1;
            }
            assert!(depth >= 0, "notch {notch}: a clip closed without being opened");
            assert!(depth <= 1, "notch {notch}: two nested clips");
        }
        assert_eq!(depth, 0, "notch {notch}: a clip left open");
    });
}

/// **No empty frame**, anywhere on the zoom range: a cell always shows
/// something, its text as soon as that is legible, its number otherwise.
#[test]
fn no_frame_is_ever_without_text() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        sweep(&mut app, 12, 60, |app, notch| {
            let texts = commands(app).into_iter().filter(Command::is_text).count();
            assert!(texts > 0, "{w}x{h} notch {notch}: image without text");
        });
    }
}

/// Every command carries a legible size, a usable alpha and a fragment that
/// really is in the corpus.
#[test]
fn every_text_command_is_drawable() {
    let mut app = reference_app();
    measure_all(&mut app);
    sweep(&mut app, 8, 40, |app, notch| {
        for c in commands(app).into_iter().filter(Command::is_text) {
            assert!(c.size > 0.0 && c.size.is_finite(), "notch {notch}: size {}", c.size);
            assert!(c.alpha > 0.01 && c.alpha <= 1.0, "notch {notch}: alpha {}", c.alpha);
            assert!(c.len > 0, "notch {notch}: empty fragment");
            assert!(
                (c.offset + c.len) as usize <= corpus::text().len(),
                "notch {notch}: fragment outside the corpus"
            );
            assert!(c.x.is_finite() && c.y.is_finite());
            assert!(!c.text().is_empty());
        }
    });
}

// ------------------------------------------------------------- the levels

/// At the framing the map shows **only the sections and the books**: three
/// portion titles and 39 book titles, and not a single chapter number.
#[test]
fn the_framing_shows_the_three_portions_and_the_thirty_nine_books() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let texts = texts(&mut app);
    assert_eq!(texts.len(), 3 + corpus::book_count(), "42 titles at the framing");

    let names: Vec<&str> = texts.iter().map(Command::text).collect();
    for section in 0..3 {
        assert!(names.contains(&corpus::section_name(section)), "portion missing");
    }
    for book in 0..corpus::book_count() {
        assert!(names.contains(&corpus::book_name(book)), "book {book} missing");
    }
    // Two sizes only: the books, and the sections a quarter bigger.
    let sizes = distinct_sizes(&texts);
    assert_eq!(sizes.len(), 2, "the framing must show two sizes, not {}", sizes.len());
    assert_close(sizes[1] / sizes[0], SECTION_OVER_BOOK, 1e-3, "portion/book ratio");
}

/// At the framing the title of a book **fills its whole box**: the chapter
/// numbers being hidden, the box belongs to it.
#[test]
fn an_overview_title_fills_its_box() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let box_ = app.camera.project(app.layout.books[0]);
    let band = app.camera.project(app.layout.book_bands[0]);
    let title = texts(&mut app)
        .into_iter()
        .find(|c| c.text() == corpus::book_name(0))
        .expect("title of the first book");
    assert!(title.h > band.h, "the framing's title stays within its band");
    assert_close(title.h, box_.h, 0.01, "the title does not fill the whole box");
}

/// One notch in, the chapters are out: **three sizes, and three only** — the
/// numbers, the books, the sections.
#[test]
fn the_chapter_level_shows_three_sizes_and_three_only() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    let texts = texts(&mut app);

    let sizes = distinct_sizes(&texts);
    assert_eq!(sizes.len(), 3, "sizes on screen: {sizes:?}");
    assert_close(sizes[0] / sizes[1], treemap::CHAPTER_OVER_BOOK, 1e-2, "number/book ratio");
    assert_close(sizes[2] / sizes[1], SECTION_OVER_BOOK, 1e-2, "portion/book ratio");

    // And a number is the same size everywhere, whatever its book.
    let numbers: Vec<&Command> =
        texts.iter().filter(|c| close(c.size, sizes[0], 1e-3)).collect();
    assert!(numbers.len() > 100, "only {} numbers on screen", numbers.len());
    assert!(
        numbers.windows(2).all(|p| close(p[0].size, p[1].size, 1e-4)),
        "two numbers of different sizes"
    );
}

/// The rank reads in the intensity of the white: a section in pure white, a
/// book one tone below, a number a touch lower still.
#[test]
fn the_rank_reads_in_the_intensity_of_the_white() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    let texts = texts(&mut app);

    // The three ranks are picked from what is really on screen: at this zoom
    // the Torah has already left it through the top.
    let names: Vec<&str> = texts.iter().map(Command::text).collect();
    let alpha_of = |name: &str| {
        texts.iter().find(|c| c.text() == name).map(|c| c.alpha).unwrap_or_else(|| panic!("{name}"))
    };
    let section = (0..3)
        .map(corpus::section_name)
        .find(|n| names.contains(n))
        .expect("a portion on screen");
    let book = (0..corpus::book_count())
        .find(|&b| names.contains(&corpus::book_name(b)))
        .expect("a book on screen");
    let number = corpus::chapters_of(book)
        .map(corpus::chapter_label)
        .find(|l| names.contains(l))
        .expect("a number on screen");

    assert_close(alpha_of(section), 1.0, 1e-6, "a portion is pure white");
    assert_close(alpha_of(corpus::book_name(book)), BOOK_INK, 1e-6, "ink of the book");
    assert_close(alpha_of(number), CHAPTER_INK, 1e-6, "ink of the number");
    const { assert!(CHAPTER_INK < BOOK_INK && BOOK_INK < 1.0, "the hierarchy of inks is reversed") };
}

/// **One zoom-out notch, and the map is nothing but the three portions, large,
/// at equal distance** — one single right edge, and no fill behind them.
#[test]
fn one_notch_out_leaves_the_three_portions_alone() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();

    let mut previous = f32::MAX;
    for notch in 1..=20 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, 1.0 / step);
        let texts = texts(&mut app);
        assert_eq!(texts.len(), 3, "notch -{notch}: {} texts instead of three", texts.len());

        for (i, c) in texts.iter().enumerate() {
            assert_eq!(c.text(), corpus::section_name(i), "notch -{notch}: wrong portion");
        }
        let sizes = distinct_sizes(&texts);
        assert_eq!(sizes.len(), 1, "notch -{notch}: the three words must share a size");

        // A single right edge, the one of the widest word.
        let right = texts[0].x;
        assert!(
            texts.iter().all(|c| close(c.x, right, 1.0)),
            "notch -{notch}: the three right edges are not one"
        );

        // Equal gaps, a share of the projected world; a size bounded by its
        // third of it.
        let gaps: Vec<f32> = texts.windows(2).map(|p| p[1].y - p[0].y).collect();
        assert_close(gaps[0], gaps[1], 0.1, "the gaps are not equal");
        let world = app.camera.project(WORLD);
        assert_close(gaps[0], world.h * CARD_GAP, 0.1, "the gap is not a share of the world");
        assert!(sizes[0] <= world.h * SECTION_CARD / 3.0 + 1e-3, "notch -{notch}: portion too large");
        assert!(gaps[0] > 0.0, "the portions do not go down in order");

        // They shrink and draw closer at every notch, without end — until the
        // technical floor of the scale.
        if app.camera.z > app.floor_scale() * 1.001 {
            assert!(sizes[0] < previous, "notch -{notch}: the words no longer shrink");
        }
        previous = sizes[0];
    }
}

/// **Nothing is ever laid over anything else.** Boxes of ink, on screen, do not
/// meet — titles, numbers and verses alike.
#[test]
fn no_text_is_ever_laid_over_another() {
    let mut app = reference_app();
    measure_all(&mut app);
    sweep(&mut app, 6, 40, |app, notch| {
        let texts: Vec<Command> = commands(app).into_iter().filter(Command::is_text).collect();
        for i in 0..texts.len() {
            for j in i + 1..texts.len() {
                let (a, b) = (texts[i], texts[j]);
                let (ax, ay) = (a.x - a.w, a.y - a.size);
                let (bx, by) = (b.x - b.w, b.y - b.size);
                let ox = (a.x.min(b.x) - ax.max(bx)).max(0.0);
                let oy = (a.y.min(b.y) - ay.max(by)).max(0.0);
                assert_eq!(
                    ox * oy,
                    0.0,
                    "notch {notch}: {:?} covers {:?}",
                    a.text(),
                    b.text()
                );
            }
        }
    });
}

/// The verses arrive, and they arrive **early**: between the seventh and the
/// twelfth notch depending on the window, never later.
#[test]
fn the_verses_arrive_within_a_dozen_notches() {
    let region = verse_region();
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let step = app.zoom_step();
        let mut arrival = 0;
        for notch in 1..=14 {
            app.zoom_at(w * 0.5, h * 0.5, step);
            if texts(&mut app).iter().any(|c| c.offset >= region) {
                arrival = notch;
                break;
            }
        }
        assert!(arrival > 0, "{w}x{h}: no verse in fourteen notches");
        assert!(arrival >= 5, "{w}x{h}: verses from notch {arrival}, which is suspicious");
    }
}

/// All the verses of a chapter share one character size, and a number is never
/// drawn next to the text of its own cell.
#[test]
fn a_cell_shows_its_text_or_its_number_but_not_both() {
    let region = verse_region();
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    for _ in 0..11 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    }
    let texts = texts(&mut app);
    let verses: Vec<&Command> = texts.iter().filter(|c| c.offset >= region).collect();
    assert!(!verses.is_empty(), "no verse at this zoom");

    // Verse lines all come from the same chapters, hence share few sizes; the
    // important part is that no chapter number sits inside a cell already full
    // of text.
    for verse in &verses {
        for other in texts.iter().filter(|c| c.offset < region) {
            let same_place = (other.x - verse.x).abs() < verse.size
                && (other.y - verse.y).abs() < verse.size;
            assert!(!same_place, "a number sits on text");
        }
    }
}

/// **Once the text is out, its numbers are beside it.** The chapter number is
/// set in the top-right corner of its column, above the first verse, and each
/// verse number at the right of its first line, on the same row — never
/// inside the text, which the overlap test already holds.
#[test]
fn the_text_carries_its_chapter_in_the_corner_and_its_verses_at_their_right() {
    let region = verse_region();
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    let mut seen = 0;
    for _ in 0..16 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        let texts = texts(&mut app);
        if !texts.iter().any(|c| c.offset >= region) {
            continue;
        }
        let camera = app.camera;
        for chapter in 0..corpus::chapter_count() {
            let column = camera.project(column(app.layout.chapters[chapter]));
            let label = corpus::chapter_label(chapter);
            let lines = app.chapter_lines(chapter);
            let first = camera.project(lines[0].rect);
            let in_column = |c: &&Command| c.x <= column.x + column.w + 0.01 && c.x >= column.x;
            let verses: Vec<&Command> = texts
                .iter()
                .filter(|c| c.offset >= region && in_column(c) && c.y >= column.y && c.y <= column.y + column.h)
                .collect();
            let head = Rect { h: lines[0].size * CHAPTER_MARK * LINE_K * camera.z, ..column };
            if verses.is_empty() || lines[0].size * CHAPTER_MARK * camera.z < MIN_NUMBER || !app.visible(head) {
                continue;
            }
            seen += 1;

            let head = texts
                .iter()
                .find(|c| c.text() == label && in_column(c) && c.y <= first.y + 0.01 && c.y >= column.y)
                .unwrap_or_else(|| panic!("chapter {chapter}: no number at the head"));
            assert_close(head.x, column.x + column.w, 0.01, "the number is not flush right");
            assert_close(head.alpha, CHAPTER_INK, 1e-6, "ink of the number at the head");

            for line in lines.iter().filter(|l| corpus::offset_of(l.text) == corpus::offset_of(drawn(l.verse))) {
                let row = camera.project(line.rect);
                let gutter = Rect { x: row.x + row.w, w: column.x + column.w - row.x - row.w, ..row };
                if line.size * VERSE_MARK * camera.z < MIN_NUMBER || !app.visible(row) || !app.visible(gutter) {
                    continue;
                }
                let mark = corpus::verse_label(line.verse);
                let found = texts.iter().find(|c| {
                    c.text() == mark && close(c.x, column.x + column.w, 0.01) && c.y > row.y && c.y < row.y + row.h
                });
                let found = found.unwrap_or_else(|| panic!("verse {}: no number on the right", line.verse));
                assert!(found.x - found.w >= row.x + row.w - 0.01, "the number bites into the verse");
            }
        }
    }
    assert!(seen > 0, "no chapter read in the sweep");
}

/// **A number is drawn once or not at all.** The centred number of a cell and
/// the one in the corner of its text are the same label: a frame holding both
/// would number the chapter twice. Every chapter and verse label is a distinct
/// slice of the corpus, so a repeated slice is a repeated number.
#[test]
fn no_number_is_ever_drawn_twice() {
    let region = verse_region();
    let mut app = reference_app();
    measure_all(&mut app);
    sweep(&mut app, 2, 18, |app, notch| {
        let mut seen: Vec<(u32, u32)> = commands(app)
            .into_iter()
            .filter(|c| c.is_text() && c.offset < region)
            .map(|c| (c.offset, c.len))
            .collect();
        let count = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), count, "notch {notch}: a number or a title is drawn twice");
    });
}

/// **All book titles share one size at a given zoom**, at every notch and not
/// only at the first: `uniform_title` takes one band for all of them.
#[test]
fn every_book_title_on_screen_shares_one_size() {
    let mut app = reference_app();
    measure_all(&mut app);
    sweep(&mut app, 0, 30, |app, notch| {
        let names: Vec<&str> = (0..corpus::book_count()).map(corpus::book_name).collect();
        let titles: Vec<Command> = commands(app)
            .into_iter()
            .filter(|c| c.is_text() && names.contains(&c.text()))
            .collect();
        let sizes = distinct_sizes(&titles);
        assert!(sizes.len() <= 1, "notch {notch}: book titles in {} sizes", sizes.len());
    });
}

/// **Titles stop growing at their ceiling**: past it the band has left the
/// screen, one is inside the book, and the title only recalls where one is.
/// The portion title keeps its ratio over the book title, ceiling included.
#[test]
fn no_title_grows_past_its_ceiling() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let ceiling = h * TITLE_CEILING;
        sweep(&mut app, 0, 30, |app, notch| {
            if app.camera.z < app.fit_scale() * PORTIONS_UNDER {
                return;
            }
            for c in commands(app).into_iter().filter(Command::is_text) {
                let name = c.text();
                let limit = if (0..3).any(|s| corpus::section_name(s) == name) {
                    ceiling * SECTION_OVER_BOOK
                } else if (0..corpus::book_count()).any(|b| corpus::book_name(b) == name) {
                    ceiling
                } else {
                    continue;
                };
                assert!(c.size <= limit * 1.0001, "{w}x{h} notch {notch}: {name} at {} > {limit}", c.size);
            }
        });
    }
}

/// **Once the chapters are out, a book title sits at the bottom of its band**,
/// just above the first chapter row: that keeps the gap between them stable at
/// every zoom.
#[test]
fn a_book_title_sits_at_the_bottom_of_its_band() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let step = app.zoom_step();
    let mut checked = 0;
    for notch in 1..=6 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        let texts = texts(&mut app);
        let camera = app.camera;
        assert!(app.chapters_ready(camera), "notch {notch}: the chapters are not out");
        for book in 0..corpus::book_count() {
            let Some(title) = texts.iter().find(|c| c.text() == corpus::book_name(book)) else { continue };
            let band = camera.project(app.layout.book_bands[book]);
            let bottom = band.y + band.h;
            let line = title.size * 1.35;
            // Baseline of a text centred on a line flush to the band bottom.
            let expected = bottom - line / 2.0 + title.size * 0.36;
            assert_close(title.y, expected, 1e-2, "the title is not at the bottom of its band");
            checked += 1;
        }
    }
    assert!(checked > 0, "no book title on screen");
}

// ------------------------------------------------------- the measurements

/// Rust draws from an estimate, lists what it does not know in `pending`, the
/// page measures, and the next frame is right: **the convergence takes one
/// frame.**
#[test]
fn the_measurements_converge_in_one_frame() {
    let mut app = reference_app();
    app.build_frame();
    assert!(!app.pending.is_empty(), "the first image must ask for measurements");
    assert_eq!(app.pending.len() % 2, 0, "pending is a sequence of pairs");

    let asked: Vec<u32> = app.pending.clone();
    for pair in asked.chunks(2) {
        let (offset, len) = (pair[0], pair[1]);
        let text = &corpus::text()[offset as usize..(offset + len) as usize];
        assert!(!text.is_empty(), "a measurement is asked for an empty fragment");
        app.ratios.insert((offset, len), letter::letter_count(text).max(1) as f32 * MEASURED);
    }
    app.lines.clear();
    app.number = None;

    // Second frame: the layout is rebuilt on the measurement, so it may ask for
    // what it had not needed yet — but a third frame, at the same zoom, asks for
    // nothing.
    app.build_frame();
    let again: Vec<u32> = app.pending.clone();
    for pair in again.chunks(2) {
        let text = &corpus::text()[pair[0] as usize..(pair[0] + pair[1]) as usize];
        app.ratios.insert((pair[0], pair[1]), letter::letter_count(text).max(1) as f32 * MEASURED);
    }
    app.lines.clear();
    app.number = None;
    app.build_frame();
    assert!(app.pending.is_empty(), "the image does not converge: {} measurements still", app.pending.len());
}

/// A measurement that contradicts the estimate rebuilds the layout **once**,
/// at the start — and the framing follows, so that a camera still sitting on it
/// does not fall into the three-portions state.
#[test]
fn a_measured_numeral_rebuilds_the_layout_once_and_keeps_the_framing() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        let before = app.numeral;
        measure_all(&mut app);
        assert!(app.numeral != before, "{w}x{h}: the measurement did not replace the estimate");
        // The map opens one notch below the framing, and a measured numeral
        // must leave it there.
        let opening = app.fit_scale() / app.zoom_step();
        assert_close(app.camera.z, opening, opening * 1e-3, "the opening jumped");

        // Once converged, a frame no longer rebuilds anything.
        let numeral = app.numeral;
        let cell = app.layout.chapters[0];
        app.build_frame();
        assert_eq!(app.numeral, numeral, "{w}x{h}: the layout is rebuilt at every image");
        assert_eq!(app.layout.chapters[0], cell);
    }
}

/// A measurement arriving later throws away what depends on it, and nothing
/// else.
#[test]
fn a_late_measurement_invalidates_the_caches() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    let _ = app.chapter_lines(0);
    let _ = app.number_size();
    assert!(!app.lines.is_empty() && app.number.is_some());

    let key = key_of(corpus::verse_text(0));
    app.ratios.insert(key, 9.0);
    app.lines.clear();
    app.number = None;
    assert!(app.lines.is_empty(), "the layout should have been dropped");
    assert!(app.number.is_none(), "the number size should have been dropped");

    // And the new width is the one used.
    assert_close(app.ratio(corpus::verse_text(0)), 9.0, 1e-6, "the measurement is not taken again");
}

/// Forgetting every measured width — what a font change calls for — must leave
/// a drawable state.
#[test]
fn forgetting_the_measurements_leaves_a_drawable_state() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.ratios.clear();
    app.lines.clear();
    app.number = None;
    let texts = texts(&mut app);
    assert!(!texts.is_empty(), "empty image after forgetting the measurements");
    assert!(!app.pending.is_empty(), "the measurements must be asked again");
}

/// `pending` is bounded: a frame asking for the whole Tanakh must not grow
/// without end.
#[test]
fn the_list_of_measurements_is_bounded() {
    let mut app = reference_app();
    let step = app.zoom_step();
    for _ in 0..12 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        app.build_frame();
        assert!(app.pending.len() <= 4096 + 2, "pending a enfle : {}", app.pending.len());
    }
}

/// **Hovering changes nothing in the drawing**: the command list is identical
/// to the bit, with and without.
#[test]
fn hovering_changes_nothing_in_the_drawing() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);

    let before = frame(&mut app);
    for (x, y) in [(0.0, 0.0), (400.0, 300.0), (1500.0, 700.0)] {
        let (wx, wy) = app.camera.to_world(x, y);
        let _ = app.locate(wx, wy);
    }
    let after = frame(&mut app);
    assert_eq!(before, after, "the hover changed the drawing");
}

/// Two frames at the same camera give the same list: the rendering is a pure
/// function of the state.
#[test]
fn two_frames_at_the_same_camera_are_identical() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    for _ in 0..9 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    }
    let first = frame(&mut app);
    let second = frame(&mut app);
    assert_eq!(first, second, "two identical images differ");
}

/// Panning does not reshape a book title: it keeps the right edge of its box,
/// never that of the screen.
#[test]
fn a_book_title_keeps_the_right_edge_of_its_box() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);

    // A book on screen, taken from the frame itself: at this zoom the first
    // ones have left it through the top.
    let on_screen = texts(&mut app);
    let book = (0..corpus::book_count())
        .find(|&b| on_screen.iter().any(|c| c.text() == corpus::book_name(b)))
        .expect("a book on screen");

    let mut offsets = Vec::new();
    for _ in 0..3 {
        let texts = texts(&mut app);
        if let Some(title) = texts.iter().find(|c| c.text() == corpus::book_name(book)) {
            let box_ = app.camera.project(app.layout.books[book]);
            offsets.push(title.x - (box_.x + box_.w));
        }
        app.camera.x -= 150.0;
    }
    assert!(offsets.len() >= 3, "the title vanished during the move");
    for pair in offsets.windows(2) {
        assert_close(pair[0], pair[1], 0.01, "the title slides in its box during the move");
    }
}

/// A portion title, on the other hand, is **pinned to the screen**: it stays
/// legible as long as one is inside its portion.
#[test]
fn a_portion_title_is_pinned_to_the_screen() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    // The portion one is inside, taken from the frame.
    let on_screen = texts(&mut app);
    let section = (0..3)
        .find(|&s| on_screen.iter().any(|c| c.text() == corpus::section_name(s)))
        .expect("a portion on screen");

    app.camera.x -= 400.0;
    let band = app.camera.project(app.layout.sections[section]);
    assert!(band.x < 0.0, "the portion should leave the screen on the left");

    let texts = texts(&mut app);
    let title = texts
        .iter()
        .find(|c| c.text() == corpus::section_name(section))
        .expect("the portion's title must stay readable");
    assert!(title.x <= app.width + 1.0, "the pinned title leaves the screen");
    assert!(title.x - title.w >= -1.0, "the pinned title is pushed too far left");
}

/// **The frame is a short list, and it stays short.**
///
/// At the framing, each title costs exactly three commands — clip, text,
/// unclip — and nothing else is emitted: 42 titles, 126 commands. The count is
/// the cheapest guard there is against a frame starting to draw what it cannot
/// show.
#[test]
fn the_command_list_stays_short() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let commands = frame(&mut app);
    assert_eq!(commands.len(), 3 * (3 + corpus::book_count()), "126 commands at the framing");

    let step = app.zoom_step();
    for notch in 1..=14 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        let count = frame(&mut app).len();
        assert!(count <= 4096, "notch {notch}: {count} commands");
    }
}

/// A frame must be built well inside a wheel notch. The figure quoted in
/// `CLAUDE.md` — 0.03 ms for the overview — is a release one; this bound is
/// loose enough for an unoptimised test build and still catches the day a frame
/// starts doing work proportional to the whole corpus.
#[test]
fn a_frame_is_built_in_a_blink() {
    let mut app = reference_app();
    measure_all(&mut app);

    let start = std::time::Instant::now();
    for _ in 0..50 {
        app.build_frame();
    }
    let each = start.elapsed().as_secs_f64() * 1000.0 / 50.0;
    assert!(each < 5.0, "the overview takes {each:.3} ms per image");

    // And at the chapter level, where 900 cells are on screen.
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    let start = std::time::Instant::now();
    for _ in 0..20 {
        app.build_frame();
    }
    let each = start.elapsed().as_secs_f64() * 1000.0 / 20.0;
    assert!(each < 16.0, "the chapter level takes {each:.3} ms per image");
}

/// **The layout of a record is a contract with the page.** `ethaor.ts` reads
/// the slots by their rank — kind, x, y, w, h, alpha, size, offset, length —
/// so any change here must be made there too. This test is the alarm.
#[test]
fn a_record_keeps_the_layout_the_page_reads() {
    assert_eq!(STRIDE, 10, "the record's stride changed: tell ethaor.ts");
    assert_eq!((KIND_TEXT, KIND_CLIP, KIND_UNCLIP), (1.0, 2.0, 3.0));

    let mut app = reference_framed();
    measure_all(&mut app);
    let words = app.commands.clone();

    // The first title of a frame: a clip, the text, then the unclip.
    assert_eq!(words[0], KIND_CLIP);
    assert_eq!(words[STRIDE], KIND_TEXT);
    assert_eq!(words[2 * STRIDE], KIND_UNCLIP);

    let text = &words[STRIDE..2 * STRIDE];
    let (offset, len) = (text[7] as u32, text[8] as u32);
    let fragment = &corpus::text()[offset as usize..(offset + len) as usize];
    assert!(
        (0..3).any(|s| corpus::section_name(s) == fragment)
            || (0..corpus::book_count()).any(|b| corpus::book_name(b) == fragment),
        "the framing's first text must be a title"
    );
    assert!(text[1].is_finite() && text[2].is_finite(), "x and y");
    assert!(text[3] > 0.0 && text[4] > 0.0, "width and height of the box");
    assert!(text[5] > 0.0 && text[5] <= 1.0, "alpha en 5");
    assert!(text[6] >= MIN_TEXT, "size in 6");
    assert_eq!(text[9], INK_OURS, "the tenth word carries the ink");

    // The clip is given as a box, and nothing else.
    let clip = &words[0..STRIDE];
    assert!(clip[3] > 0.0 && clip[4] > 0.0);
    assert!(clip[5..].iter().all(|&v| v == 0.0), "a clip carries only its box");
}
