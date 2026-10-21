//! What a click selects, and when it is live at all.
//!
//! Two rules. **A click only lives on a passage one can read**: above that
//! zoom a chapter is a number, and naming one of its verses would be a choice
//! of the geometry rather than of the reader. Its words are picked in the
//! passage the page opens; the word boxes of a line stay, for the readings'
//! index under the pointer.

use super::*;

/// A selection read back as the page reads it.
#[derive(Debug, PartialEq)]
pub struct Hit {
    pub book: usize,
    pub chapter: usize,
    pub verse: Option<&'static str>,
    pub number: Option<&'static str>,
}

fn read(slots: [u32; 6]) -> Hit {
    let level = |i: usize| {
        (slots[i + 1] > 0)
            .then(|| &corpus::text()[slots[i] as usize..(slots[i] + slots[i + 1]) as usize])
    };
    Hit {
        book: slots[0] as usize,
        chapter: slots[1] as usize,
        verse: level(2),
        number: level(4),
    }
}

fn click(app: &mut App, px: f32, py: f32) -> Option<Hit> {
    let (wx, wy) = app.camera.to_world(px, py);
    app.pick(wx, wy).map(read)
}

fn on_screen(app: &App, r: Rect) -> (f32, f32) {
    let p = app.camera.project(r);
    (p.x + p.w / 2.0, p.y + p.h / 2.0)
}

/// Zooms at the centre until the click goes live there, and gives the notch.
fn dive_to_reading(app: &mut App) -> usize {
    let step = app.zoom_step();
    let (cx, cy) = (app.width * 0.5, app.height * 0.5);
    for notch in 1..=20 {
        app.zoom_at(cx, cy, step);
        app.build_frame();
        let (wx, wy) = app.camera.to_world(cx, cy);
        if app.clickable_at(wx, wy).is_some() {
            return notch;
        }
    }
    panic!("the click never becomes live");
}

// --------------------------------------------------------- when it is live

/// **A click is live exactly where the text is drawn**, and nowhere else.
///
/// The condition is the one of the drawing — a line of at least
/// `READABLE_LINE` — so the two cannot part ways: every cell showing its text
/// is clickable, and every cell showing a number is not. On a wide screen a
/// chapter of two verses is read at the framing already, and clicking it is
/// right; on the reference window nothing is read before the tenth notch, and
/// clicking anything would be wrong.
#[test]
fn a_click_is_live_exactly_where_the_text_is_drawn() {
    // Which chapter a drawn fragment belongs to, by its offset in the corpus.
    let mut chapter_of: std::collections::HashMap<u32, usize> =
        std::collections::HashMap::new();
    for chapter in 0..corpus::chapter_count() {
        for verse in corpus::verses_of(chapter) {
            chapter_of.insert(key_of(corpus::verse_text(verse)).0, chapter);
        }
    }

    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let step = app.zoom_step();

        for notch in 0..12 {
            if notch > 0 {
                app.zoom_at(w * 0.5, h * 0.5, step);
            }
            let drawn: std::collections::HashSet<usize> = texts(&mut app)
                .iter()
                .filter_map(|c| chapter_of.get(&c.offset).copied())
                .collect();

            // Every third chapter: the sweep is over nine windows and twelve
            // notches, and a cell says nothing its neighbours do not.
            for chapter in (0..corpus::chapter_count()).step_by(3) {
                let cell = app.camera.project(app.layout.chapters[chapter]);
                // Only the cells wholly on screen: a cell cut by the edge may
                // have its lines outside, hence drawn nowhere.
                if cell.x < 0.0 || cell.y < 0.0 || cell.x + cell.w > w || cell.y + cell.h > h {
                    continue;
                }
                let (px, py) = (cell.x + cell.w / 2.0, cell.y + cell.h / 2.0);
                let live = click(&mut app, px, py).is_some();
                assert_eq!(
                    live,
                    drawn.contains(&chapter),
                    "{w}x{h} notch {notch}: click {live} on chapter {chapter}, text drawn {}",
                    drawn.contains(&chapter)
                );
            }
        }
    }
}

/// On the reference window, the measurement of `CLAUDE.md`: **nothing is
/// clickable at the framing, nor at the chapter level.** One reads titles and
/// numbers there, not passages.
#[test]
fn a_click_is_dead_at_the_framing_and_at_the_chapter_level() {
    let mut app = reference_app();
    measure_all(&mut app);

    for book in 0..corpus::book_count() {
        let (px, py) = on_screen(&app, app.layout.books[book]);
        assert!(
            click(&mut app, px, py).is_none(),
            "a live click on book {book} at the framing"
        );
    }

    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    app.build_frame();
    let (w, h) = (app.width, app.height);
    let mut cells = 0;
    for chapter in 0..corpus::chapter_count() {
        let (px, py) = on_screen(&app, app.layout.chapters[chapter]);
        if px < 0.0 || py < 0.0 || px > w || py > h {
            continue;
        }
        cells += 1;
        assert!(
            click(&mut app, px, py).is_none(),
            "a live click on cell {chapter}, whose text is not read"
        );
    }
    assert!(cells > 100, "only {cells} cells on screen");
}

/// And it goes live **exactly when the lines are read** — the same threshold
/// that puts the verse in the breadcrumb and the text in the frame.
#[test]
fn a_click_goes_live_with_the_text() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let notches = dive_to_reading(&mut app);
        assert!(notches <= 14, "{w}x{h}: {notches} notches before a live click");

        let (cx, cy) = (w * 0.5, h * 0.5);
        let (wx, wy) = app.camera.to_world(cx, cy);
        let chapter = app.clickable_at(wx, wy).expect("a clickable chapter");
        assert!(app.readable(chapter), "{w}x{h}: clickable without being readable");
        assert!(
            app.line_height(chapter, app.camera) >= READABLE_LINE,
            "{w}x{h}: the click's threshold is not the text's"
        );

        let hit = click(&mut app, cx, cy).expect("a click");
        assert!(hit.verse.is_some(), "{w}x{h}: a live click without a verse");
    }
}

/// **The pointer announces what a click will do**: `hover` answers on the very
/// condition of a click, everywhere and at every zoom.
#[test]
fn the_cursor_announces_exactly_what_a_click_will_do() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    let (w, h) = (app.width, app.height);

    for notch in 0..14 {
        if notch > 0 {
            app.zoom_at(w * 0.5, h * 0.5, step);
        }
        app.build_frame();
        for i in 0..7 {
            for j in 0..5 {
                let (px, py) = (w * (i as f32 + 0.5) / 7.0, h * (j as f32 + 0.5) / 5.0);
                let (wx, wy) = app.camera.to_world(px, py);
                let announced = app.clickable_at(wx, wy).is_some();
                let selected = click(&mut app, px, py).is_some();
                assert_eq!(
                    announced, selected,
                    "notch {notch}, point ({px}, {py}): the cursor lies about the click"
                );
            }
        }
    }
}

/// In the air between two portions, or in the black that borders the map, a
/// click selects nothing — whatever the zoom.
#[test]
fn a_click_in_the_air_selects_nothing() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_reading(&mut app);

    let last = app.layout.books[corpus::book_count() - 1];
    let far = Rect { y: last.y + last.h + 40.0, h: 1.0, ..last };
    let (px, py) = on_screen(&app, far);
    assert!(click(&mut app, px, py).is_none(), "the blank at the bottom is clickable");

    let band = app.layout.section_bands[1];
    let (px, py) = on_screen(&app, Rect { h: band.h * 0.4, ..band });
    assert!(click(&mut app, px, py).is_none(), "the air of a portion is clickable");
}

// --------------------------------------------------------- what it selects

/// A click gives a coherent path, and the verse it gives is the one under the
/// point.
#[test]
fn a_click_gives_the_passage_under_the_point() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_reading(&mut app);

    let (wx, wy) = app.camera.to_world(app.width * 0.5, app.height * 0.5);
    let chapter = app.clickable_at(wx, wy).expect("a chapter");
    let lines = app.chapter_lines(chapter);

    let mut seen = 0;
    for line in &lines {
        let (px, py) = on_screen(&app, line.rect);
        let Some(hit) = click(&mut app, px, py) else { continue };
        if hit.chapter != chapter {
            continue;
        }
        assert_eq!(hit.book, book_of(chapter));
        assert_eq!(
            hit.verse,
            Some(corpus::verse_text(line.verse)),
            "the line of verse {} gives another passage",
            line.verse
        );
        seen += 1;
    }
    assert!(seen >= 2, "only {seen} lines clicked");
}

/// The passage comes with **its number**, the very label its verse is drawn
/// with — so the page can name the place
/// it shows without guessing it from the text.
#[test]
fn a_click_numbers_the_verse_it_gives() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_reading(&mut app);

    let (wx, wy) = app.camera.to_world(app.width * 0.5, app.height * 0.5);
    let chapter = app.clickable_at(wx, wy).expect("a chapter");
    let mut numbers = Vec::new();
    for line in app.chapter_lines(chapter) {
        let (px, py) = on_screen(&app, line.rect);
        let Some(hit) = click(&mut app, px, py) else { continue };
        if hit.chapter != chapter {
            continue;
        }
        assert_eq!(
            hit.number,
            Some(corpus::verse_label(line.verse)),
            "the line of verse {} is numbered as another",
            line.verse
        );
        numbers.push(line.verse);
    }
    numbers.dedup();
    assert!(numbers.len() >= 2, "only {} verses numbered", numbers.len());
}

// ------------------------------------------------------------- word cells

/// The word boxes of a line go **right to left**, hold their ink and nothing
/// else, and never overlap: the gaps between them are the spaces.
#[test]
fn the_words_of_a_line_fill_it_right_to_left() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);

    for chapter in [0usize, 100, 500, 928] {
        let line = app.chapter_lines(chapter)[0];
        let cells = app.word_cells(line.text, line.rect, line.size);
        let words: Vec<&str> = line.text.split(' ').collect();
        assert_eq!(cells.len(), words.len(), "a word was lost");

        for (cell, word) in &cells {
            assert!(cell.w > 0.0, "the word {word:?} has no width");
            assert_eq!(cell.y, line.rect.y, "a word left its line");
            assert_eq!(cell.h, line.rect.h);
        }
        for (i, (cell, word)) in cells.iter().enumerate() {
            assert_eq!(*word, words[i], "the order of the words changed");
            if i > 0 {
                let before = cells[i - 1].0;
                assert!(cell.x + cell.w <= before.x + 1e-3, "two words overlap");
            }
        }

        // First word on the right edge, last one inside the line.
        let right = line.rect.x + line.rect.w;
        assert_close(cells[0].0.x + cells[0].0.w, right, 1e-3, "the first word is not at the edge");
        let last = cells.last().unwrap().0;
        assert!(
            last.x >= line.rect.x - line.rect.w * 0.01,
            "the last word leaves its line"
        );

        // The boxes span exactly the drawn width: the ink of the words plus the
        // spaces between them.
        let ink: f32 = cells.iter().map(|(c, _)| c.w).sum();
        let drawn = app.width_of(line.text, line.size);
        let span = (cells[0].0.x + cells[0].0.w) - last.x;
        assert_close(span, drawn, 1e-2, "the boxes do not cover the drawn width");
        assert!(ink <= drawn + 1e-3, "the ink goes past the drawn width");
    }
}

/// A line of a single word gives one box, laid on the right edge.
#[test]
fn a_single_word_takes_the_whole_line() {
    let mut app = reference_app();
    measure_all(&mut app);
    let word = corpus::verse_text(0).split(' ').next().unwrap();
    let row = Rect { x: 10.0, y: 20.0, w: 30.0, h: 4.0 };
    let cells = app.word_cells(word, row, 2.0);
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].1, word);
    assert_close(cells[0].0.x + cells[0].0.w, row.x + row.w, 1e-3, "the word is not flush right");
    assert_close(cells[0].0.w, app.width_of(word, 2.0), 1e-3, "width of the word");
}

#[test]
fn a_line_without_size_has_no_word_cell() {
    let mut app = reference_app();
    let row = Rect { x: 0.0, y: 0.0, w: 10.0, h: 2.0 };
    assert!(app.word_cells(corpus::verse_text(0), row, 0.0).is_empty());
    let empty = &corpus::text()[0..0];
    assert!(app.word_cells(empty, row, 1.0).is_empty());
}

/// Words and letters agree: a letter box is inside the box of the word that
/// holds it — the two splits share one measured width.
#[test]
fn the_letters_of_a_word_stay_inside_it() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    let line = app.chapter_lines(0)[0];

    let words = app.word_cells(line.text, line.rect, line.size);
    let letters = app.letter_cells(line.text, line.rect, line.size);
    let left = words.last().unwrap().0.x;
    let right = words[0].0.x + words[0].0.w;
    for cell in &letters {
        assert!(cell.x >= left - 1e-2, "a letter goes left of the last word");
        assert!(cell.x + cell.w <= right + 1e-2, "a letter goes right of the first word");
    }
}

// ------------------------------------------------------- in the book view

/// The book's click, where the browser lays the verse out, **names what the
/// map's click names**: for every line of a chapter at reading level, the
/// map's pick at the line and the book's pick of its verse give the same
/// slots.
#[test]
fn a_click_in_the_book_names_what_the_map_names() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_reading(&mut app);

    let (wx, wy) = app.camera.to_world(app.width * 0.5, app.height * 0.5);
    let chapter = app.clickable_at(wx, wy).expect("a chapter");
    let mut compared = 0;
    for line in app.chapter_lines(chapter) {
        let (lx, ly) = on_screen(&app, line.rect);
        let (px, py) = app.camera.to_world(lx, ly);
        assert_eq!(
            App::pick_in_verse(line.verse),
            app.pick(px, py),
            "verse {}: the book's click names another passage",
            line.verse
        );
        compared += 1;
    }
    assert!(compared >= 2, "only {compared} lines compared");
}

/// Every landed verse is selected by the book with its own text and number;
/// a verse out of range names nothing.
#[test]
fn a_click_in_the_book_names_every_verse() {
    for verse in (0..corpus::verse_count()).step_by(97) {
        let hit = read(App::pick_in_verse(verse).expect("a landed verse is selected"));
        assert_eq!(hit.verse, Some(corpus::verse_text(verse)), "verse {verse}: another passage");
        assert_eq!(hit.number, Some(corpus::verse_label(verse)), "verse {verse}: another number");
        assert!(corpus::verses_of(hit.chapter).contains(&verse), "verse {verse}: another chapter");
        assert_eq!(hit.book, book_of(hit.chapter));
    }
    assert_eq!(App::pick_in_verse(corpus::verse_count()), None, "a verse out of range");
}
