//! The other edition's reading, drawn inside our text.
//!
//! Two promises. **Nothing is painted over anything**: the other form takes its
//! place in the flow, because the verse is laid out from the display string
//! welded by the build. And **the ink is the only thing that tells them
//! apart**: a run marked `INK_OTHER` is theirs, everything else is ours, and
//! the page is told nothing more.

use super::*;

/// First verse carrying another reading, with its chapter.
fn sample() -> (usize, usize) {
    for chapter in 0..corpus::chapter_count() {
        for verse in corpus::verses_of(chapter) {
            if variant::display(verse).is_some() {
                return (chapter, verse);
            }
        }
    }
    panic!("no carrying verse");
}

/// Lines of a chapter, laid out on measured widths.
fn lines_of(app: &mut App, chapter: usize) -> Vec<Line> {
    measure_all(app);
    app.chapter_lines(chapter)
}

/// Draws one line alone, at a size that clears the legibility floor, and gives
/// back the commands.
fn draw_one(app: &mut App, line: Line, row: Rect) -> Vec<Command> {
    app.commands.clear();
    let scale = (MIN_TEXT * 2.0) / line.size;
    app.draw_line(line, row, scale, 1.0);
    commands(app)
}

#[test]
fn a_verse_with_another_reading_is_laid_out_from_the_display_string() {
    let (_, verse) = sample();
    assert_eq!(drawn(verse), variant::display(verse).unwrap(), "the carrying verse");
    // Every other verse is drawn from the corpus, untouched.
    let plain = (0..corpus::verse_count()).find(|&v| variant::display(v).is_none()).unwrap();
    assert_eq!(drawn(plain), corpus::verse_text(plain), "an ordinary verse");
}

/// **The runs of a line follow each other from the right**, and together they
/// are the line. The text is set flush right, so the first run of the string is
/// the rightmost: each box ends where the next one begins.
#[test]
fn the_runs_of_a_line_tile_it_from_the_right() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let row = Rect { x: 100.0, y: 50.0, w: 900.0, h: 30.0 };

    let mut seen = 0usize;
    for line in lines.into_iter().filter(|l| l.verse == verse) {
        let out = draw_one(&mut app, line, row);
        assert!(!out.is_empty(), "a line without a command");
        seen += 1;

        let joined: String = out.iter().map(|c| c.text()).collect();
        assert_eq!(joined, line.text, "the runs do not recompose the line");

        // The first slot of a text record is the right edge the page lays the
        // fragment on, and the fourth is its width: a run begins where the
        // previous one ended.
        let mut right = row.x + row.w;
        for c in &out {
            assert_close(c.x, right, 1e-2, "a run does not touch the previous one");
            assert!(c.w > 0.0, "a run without width");
            right = c.x - c.w;
        }
    }
    assert!(seen > 0, "the carrying verse produced no line");
}

/// **The ink says theirs or ours, and nothing else does.** The two runs are
/// drawn at the same strength — the colour alone sets them apart, and which
/// colour is the page's business — so a run must never be found marked without
/// being the other reading, nor the other way round.
#[test]
fn only_the_other_reading_carries_the_other_ink() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let row = Rect { x: 0.0, y: 0.0, w: 900.0, h: 30.0 };
    let shown = variant::display(verse).unwrap();
    let base = corpus::offset_of(shown);

    let mut others = 0usize;
    for line in lines.into_iter().filter(|l| l.verse == verse) {
        for c in draw_one(&mut app, line, row) {
            assert!(c.ink == INK_OURS || c.ink == INK_OTHER, "unknown ink: {}", c.ink);
            let at = c.offset as usize - base;
            let inside = variant::inserts_of(verse).any(|i| {
                let (from, len) = i.other_range();
                at >= from && at + c.len as usize <= from + len
            });
            if c.ink == INK_OTHER {
                others += 1;
                assert!(inside, "a run marked without being another reading");
                assert_close(c.alpha, 1.0, 1e-3, "the other reading was dimmed");
            } else {
                assert!(!inside, "another reading left in the text's ink");
                assert_close(c.alpha, 1.0, 1e-3, "our text was dimmed");
            }
        }
    }
    assert!(others > 0, "no other reading drawn");
}

/// A verse without a variant is still one command: the cut costs nothing to
/// the twenty-three thousand others.
#[test]
fn a_verse_without_a_variant_stays_one_command() {
    let chapter = (0..corpus::chapter_count())
        .find(|&c| corpus::verses_of(c).all(|v| variant::display(v).is_none()))
        .expect("a chapter without a variant");

    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let row = Rect { x: 0.0, y: 0.0, w: 900.0, h: 30.0 };
    for line in lines {
        let out = draw_one(&mut app, line, row);
        assert_eq!(out.len(), 1, "an ordinary line fits in one command");
        assert_eq!(out[0].ink, INK_OURS);
        assert_eq!(out[0].text(), line.text);
    }
}

/// The counter the page reads to show the index: zero when no other reading is
/// on screen, more as soon as one is drawn.
#[test]
fn the_counter_follows_what_is_drawn() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);

    app.variant_runs = 0;
    app.build_frame();
    assert_eq!(app.variant_runs, 0, "the framing shows no other reading");

    let row = Rect { x: 0.0, y: 0.0, w: 900.0, h: 30.0 };
    app.variant_runs = 0;
    for line in lines.into_iter().filter(|l| l.verse == verse) {
        let scale = (MIN_TEXT * 2.0) / line.size;
        app.draw_line(line, row, scale, 1.0);
    }
    assert!(app.variant_runs > 0, "the counter ignores the other readings");
}

/// **The navbar keeps naming our text.** A word of ours is given back without
/// the mark that numbers it; a word of the other edition names nothing, since
/// the passage does not contain it.
#[test]
fn a_click_never_names_the_other_edition() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let line = lines.into_iter().find(|l| l.verse == verse).expect("a line of the verse");
    let mine = corpus::verse_text(verse);
    let (from, len) = key_of(mine);

    let words = app.word_cells(line.text, line.rect, line.size);
    assert!(!words.is_empty(), "no word on the line");

    let shown = variant::display(verse).unwrap();
    let base = corpus::offset_of(shown);
    for (_, word) in words {
        let (at, size) = key_of(word);
        match variant::corpus_span(verse, at as usize - base, size as usize) {
            // Ours: the span given back is inside the verse, and it is what the
            // corpus holds there.
            Some((start, kept)) => {
                assert!(start + kept <= mine.len(), "a word runs past the verse");
                assert_eq!(
                    &mine[start..start + kept],
                    &corpus::text()[(from as usize + start)..(from as usize + start + kept)],
                    "the word named is not the corpus' one"
                );
                assert!(kept <= size as usize, "the mark was not removed");
            }
            // Theirs: nothing is named.
            None => {
                let at = at as usize - base;
                assert!(
                    variant::inserts_of(verse).any(|i| {
                        let (f, l) = i.range();
                        at >= f && at < f + l
                    }),
                    "a word refused without being another reading"
                );
            }
        }
    }
    let _ = len;
}

// ------------------------------------------------- the index under the canvas

/// Zooms on a chapter cell until the numbers are out, and gives its centre on
/// screen.
fn focus_chapter(app: &mut App, chapter: usize) -> (f32, f32) {
    let step = app.zoom_step();
    for _ in 0..30 {
        let cell = app.camera.project(app.layout.chapters[chapter]);
        let (cx, cy) = (cell.x + cell.w / 2.0, cell.y + cell.h / 2.0);
        if app.chapters_ready(app.camera) {
            return (cx, cy);
        }
        app.zoom_at(cx, cy, step);
        app.build_frame();
    }
    panic!("the numbers never show");
}

/// Centre of a box on screen.
fn centre(app: &App, r: Rect) -> (f32, f32) {
    let p = app.camera.project(r);
    (p.x + p.w / 2.0, p.y + p.h / 2.0)
}

fn asks(app: &mut App, px: f32, py: f32) -> bool {
    let (wx, wy) = app.camera.to_world(px, py);
    app.readings_at(wx, wy)
}

/// **The index answers for the chapter under the pointer**, once the numbers
/// are out: that is the ensemble the reader has under his cursor.
#[test]
fn the_index_answers_for_the_chapter_pointed_at() {
    let (chapter, _) = sample();
    let empty = (0..corpus::chapter_count())
        .find(|&c| !variant::in_verses(corpus::verses_of(c)))
        .expect("a chapter without another reading");

    let mut app = reference_app();
    measure_all(&mut app);
    let (cx, cy) = focus_chapter(&mut app, chapter);
    assert!(asks(&mut app, cx, cy), "the carrying chapter does not answer");

    let mut app = reference_app();
    measure_all(&mut app);
    let (cx, cy) = focus_chapter(&mut app, empty);
    assert!(!asks(&mut app, cx, cy), "a chapter without a difference answers");
}

/// Verses of a stretch of books, as one range. Only the tests need it: the
/// index itself never asks above the chapter.
fn verses_of_books(books: core::ops::Range<usize>) -> core::ops::Range<usize> {
    let chapters = corpus::chapters_of(books.start).start..corpus::chapters_of(books.end - 1).end;
    corpus::verses_of(chapters.start).start..corpus::verses_of(chapters.end - 1).end
}

/// **Above the chapter, nothing.** At the framing the numbers are not out, so
/// there is no chapter under the pointer — and a book holds a reading almost
/// always, so answering for it would light the index everywhere and say
/// nothing.
#[test]
fn at_the_framing_the_index_says_nothing() {
    let full = (0..corpus::book_count())
        .find(|&b| variant::in_verses(verses_of_books(b..b + 1)))
        .expect("a carrying book");

    let mut app = reference_app();
    measure_all(&mut app);
    assert!(!app.chapters_ready(app.camera), "the numbers are already out at the framing");

    let (cx, cy) = centre(&app, app.layout.books[full]);
    assert!(!asks(&mut app, cx, cy), "the framing answers for a book");
}

/// **Outside every box, nothing is promised.** The black around the map and
/// the air between portions name no passage, so they can name no reading —
/// the same rule the breadcrumb and the click follow.
#[test]
fn the_black_around_the_map_promises_nothing() {
    let mut app = reference_app();
    measure_all(&mut app);

    let far = app.camera.project(WORLD);
    assert!(!asks(&mut app, far.x - 200.0, far.y + far.h / 2.0), "the black on the left answers");
    assert!(!asks(&mut app, far.x + far.w + 200.0, far.y + far.h / 2.0), "the black on the right answers");
}

/// A portion band names its portion and stops there, so the index stays shut:
/// each of the three holds readings, and saying so would be an index lit from
/// the first frame to the last.
#[test]
fn a_portion_band_says_nothing() {
    let mut app = reference_app();
    measure_all(&mut app);

    for (section, &(_, first, last)) in treemap::SECTIONS.iter().enumerate() {
        assert!(
            variant::in_verses(verses_of_books(first..last)),
            "portion {section} holds no reading, the test proves nothing"
        );
        let band = app.layout.section_bands[section];
        let (cx, cy) = centre(&app, Rect { x: band.x, y: band.y, w: band.w, h: band.h * 0.5 });
        assert!(!asks(&mut app, cx, cy), "the band of portion {section} answers");
    }
}

/// The index obeys the breadcrumb's rule: **the void of a cell names nothing**,
/// so it promises nothing either. Pointing a few letters away from the number
/// used to light the row.
#[test]
fn the_void_of_a_cell_lights_no_index() {
    let (chapter, _) = sample();
    let mut app = reference_app();
    measure_all(&mut app);
    let (cx, cy) = focus_chapter(&mut app, chapter);
    assert!(asks(&mut app, cx, cy), "the number of the carrying chapter does not answer");

    // In the world, where the geometry does not depend on the zoom: a cell is
    // far wider than its number, the stride being the densest book's.
    let cell = app.layout.chapters[chapter];
    let ink = app.numeral_box(chapter);
    assert!(ink.x - cell.x > cell.w * 0.1, "the cell has no void left of its number");

    let (vx, vy) = ((cell.x + ink.x) / 2.0, cell.y + cell.h / 2.0);
    assert!(!app.readings_at(vx, vy), "the cell's void lights the index");
    let _ = cy;
}
