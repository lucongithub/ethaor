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

/// **A click keeps naming our text.** On a verse carrying another reading,
/// the passage a click gives is ours — the other edition's forms and the
/// marks that number them are not in it — so every word the passage offers
/// to pick is ours.
#[test]
fn a_click_never_names_the_other_edition() {
    let (_, verse) = sample();
    let slots = App::pick_in_verse(verse).expect("the verse is selected");
    let given = &corpus::text()[slots[2] as usize..(slots[2] + slots[3]) as usize];
    assert_eq!(given, corpus::verse_text(verse), "the passage is not our verse");
    assert_ne!(Some(given), variant::display(verse), "the other edition's forms are given");
    assert!(!given.contains('¹') && !given.contains('²'), "a mark is given: {given:?}");
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
    app.readings_at(wx, wy).is_some()
}

/// Whether a stretch of verses holds another reading.
fn carries(verses: core::ops::Range<usize>) -> bool {
    verses.into_iter().any(|v| variant::display(v).is_some())
}

/// **The index answers on the two forms, and on nothing else** — the
/// owner's rule. In a chapter holding another reading, read at its level:
/// the pointer on our marked form or on any word of theirs lights it; on
/// every other word of the chapter, the verses that carry them included, it
/// stays shut. Which words are the forms is read off the marks the
/// reader sees — `¹` on ours, theirs from `[` to `]²` — not off the table.
#[test]
fn the_index_answers_on_the_two_forms_alone() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    app.show(chapter, Some(verse));
    measure_all(&mut app);
    app.show(chapter, Some(verse));
    app.build_frame();
    assert!(app.readable(chapter), "the chapter is not read");

    let (mut lit, mut shut) = (0, 0);
    let mut theirs = false;
    for line in app.chapter_lines(chapter) {
        let cells = app.word_cells(line.text, line.rect, line.size);
        for (r, word) in cells {
            if word.is_empty() {
                continue;
            }
            let form = theirs || word.contains('¹') || word.starts_with('[');
            if word.starts_with('[') {
                theirs = true;
            }
            if word.contains(']') {
                theirs = false;
            }
            let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            assert_eq!(app.readings_at(x, y).is_some(), form, "{word:?} in verse {}", line.verse);
            if form { lit += 1 } else { shut += 1 }
        }
    }
    assert!(lit >= 2, "our form and theirs did not both light the index");
    assert!(shut > lit, "the test met no word the index must ignore");
}

/// **The answer is the pointed pair's reading, theirs alone** — what the page
/// colours, and only while it is pointed at. On a word of theirs the range
/// holds that word; on our marked form it starts right after it, nothing but
/// the space between. Either way it reads `[…]²` and is exactly the stretch
/// the frame cuts out as `INK_OTHER`, so the page can tell that reading's runs
/// from every other on screen.
#[test]
fn the_answer_is_the_pointed_pair_s_reading() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    app.show(chapter, Some(verse));
    measure_all(&mut app);
    app.show(chapter, Some(verse));
    app.build_frame();

    let others: Vec<(usize, usize)> = commands(&app)
        .iter()
        .filter(|c| c.ink == INK_OTHER)
        .map(|c| (c.offset as usize, c.offset as usize + c.len as usize))
        .collect();
    let text = corpus::text();
    let (mut ours, mut theirs) = (0, 0);
    for line in app.chapter_lines(chapter) {
        for (r, word) in app.word_cells(line.text, line.rect, line.size) {
            let Some((from, to)) = app.readings_at(r.x + r.w / 2.0, r.y + r.h / 2.0) else {
                continue;
            };
            let reading = &text[from..to];
            assert!(reading.trim_start().starts_with('[') && reading.ends_with('²'), "{reading:?}");
            assert!(!reading.contains('¹'), "{reading:?} takes our form in");
            let at = corpus::offset_of(word);
            if word.contains('¹') {
                assert!(text[at + word.len()..from].trim().is_empty(), "{word:?} is not right before {reading:?}");
                ours += 1;
            } else {
                assert!(from <= at && at + word.len() <= to, "{word:?} lies outside {reading:?}");
                theirs += 1;
            }
            // Every run the frame paints apart inside the range, and none
            // straddling it: colouring by range colours this reading whole.
            for &(a, b) in &others {
                assert!(b <= from || a >= to || (a >= from && b <= to), "a run straddles {reading:?}");
            }
            assert!(others.iter().any(|&(a, b)| a >= from && b <= to), "no run of {reading:?} drawn");
        }
    }
    assert!(ours >= 1 && theirs >= 1, "the test met only one of the two forms");
}

/// Verses of a stretch of books, as one range.
fn verses_of_books(books: core::ops::Range<usize>) -> core::ops::Range<usize> {
    let chapters = corpus::chapters_of(books.start).start..corpus::chapters_of(books.end - 1).end;
    corpus::verses_of(chapters.start).start..corpus::verses_of(chapters.end - 1).end
}

/// **At the framing, nothing.** No word is drawn, so none can be pointed at
/// — and a book holds a reading almost always.
#[test]
fn at_the_framing_the_index_says_nothing() {
    let full = (0..corpus::book_count()).find(|&b| carries(verses_of_books(b..b + 1))).expect("a carrying book");

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
        assert!(carries(verses_of_books(first..last)), "portion {section} holds no reading, the test proves nothing");
        let band = app.layout.section_bands[section];
        let (cx, cy) = centre(&app, Rect { x: band.x, y: band.y, w: band.w, h: band.h * 0.5 });
        assert!(!asks(&mut app, cx, cy), "the band of portion {section} answers");
    }
}

/// **A chapter's number lights no index**, though its chapter holds a
/// reading: a number is not one of the two forms. It used to answer for the
/// whole chapter.
#[test]
fn a_chapter_number_lights_no_index() {
    let (chapter, _) = sample();
    let mut app = reference_app();
    measure_all(&mut app);
    let (cx, cy) = focus_chapter(&mut app, chapter);
    assert!(!app.readable(chapter), "the chapter is read already, the test proves nothing");
    assert!(!asks(&mut app, cx, cy), "the number of the carrying chapter lights the index");
}
