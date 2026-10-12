//! What a click selects, and when it is live at all.
//!
//! Two rules. **A click only lives on a passage one can read**: above that
//! zoom a chapter is a number, and naming one of its verses would be a choice
//! of the geometry rather than of the reader. And **a double click goes one
//! step finer**, down to the word under the point.

use super::*;

/// A selection read back as the page reads it.
#[derive(Debug, PartialEq)]
pub struct Hit {
    pub book: usize,
    pub chapter: usize,
    pub verse: Option<&'static str>,
    pub word: Option<&'static str>,
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
        word: level(4),
    }
}

fn click(app: &mut App, px: f32, py: f32, word: bool) -> Option<Hit> {
    let (wx, wy) = app.camera.to_world(px, py);
    app.pick(wx, wy, word).map(read)
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
    panic!("le clic ne s'active jamais");
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
                let live = click(&mut app, px, py, false).is_some();
                assert_eq!(
                    live,
                    drawn.contains(&chapter),
                    "{w}x{h} cran {notch}: clic {live} sur le chapitre {chapter}, texte dessine {}",
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
            click(&mut app, px, py, false).is_none(),
            "un clic vivant sur le livre {book} au cadrage"
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
            click(&mut app, px, py, false).is_none(),
            "un clic vivant sur la case {chapter}, dont le texte n'est pas lu"
        );
    }
    assert!(cells > 100, "seulement {cells} cases a l'ecran");
}

/// And it goes live **exactly when the lines are read** — the same threshold
/// that puts the verse in the breadcrumb and the text in the frame.
#[test]
fn a_click_goes_live_with_the_text() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let notches = dive_to_reading(&mut app);
        assert!(notches <= 14, "{w}x{h}: {notches} crans avant un clic vivant");

        let (cx, cy) = (w * 0.5, h * 0.5);
        let (wx, wy) = app.camera.to_world(cx, cy);
        let chapter = app.clickable_at(wx, wy).expect("un chapitre cliquable");
        assert!(app.readable(chapter), "{w}x{h}: cliquable sans etre lisible");
        assert!(
            app.line_height(chapter, app.camera) >= READABLE_LINE,
            "{w}x{h}: le seuil du clic n'est pas celui du texte"
        );

        let hit = click(&mut app, cx, cy, false).expect("un clic");
        assert!(hit.verse.is_some(), "{w}x{h}: un clic vivant sans verset");
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
                let selected = click(&mut app, px, py, false).is_some();
                assert_eq!(
                    announced, selected,
                    "cran {notch}, point ({px}, {py}): le curseur ment sur le clic"
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
    assert!(click(&mut app, px, py, false).is_none(), "le blanc du bas est cliquable");

    let band = app.layout.section_bands[1];
    let (px, py) = on_screen(&app, Rect { h: band.h * 0.4, ..band });
    assert!(click(&mut app, px, py, false).is_none(), "l'air d'une portion est cliquable");
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
    let chapter = app.clickable_at(wx, wy).expect("un chapitre");
    let lines = app.chapter_lines(chapter);

    let mut seen = 0;
    for line in &lines {
        let (px, py) = on_screen(&app, line.rect);
        let Some(hit) = click(&mut app, px, py, false) else { continue };
        if hit.chapter != chapter {
            continue;
        }
        assert_eq!(hit.book, book_of(chapter));
        assert_eq!(
            hit.verse,
            Some(corpus::verse_text(line.verse)),
            "la ligne du verset {} rend un autre passage",
            line.verse
        );
        assert_eq!(hit.word, None, "un clic simple nomme un mot");
        seen += 1;
    }
    assert!(seen >= 2, "seulement {seen} lignes cliquees");
}

/// A double click names **the word under the point**, and the word is a word of
/// that very verse.
#[test]
fn a_double_click_names_the_word_under_the_point() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_reading(&mut app);

    let (wx, wy) = app.camera.to_world(app.width * 0.5, app.height * 0.5);
    let chapter = app.clickable_at(wx, wy).expect("un chapitre");
    let line = app.line_at(chapter, wy).expect("une ligne");
    let cells = app.word_cells(line.text, line.rect, line.size);
    assert!(cells.len() >= 2, "il faut deux mots pour comparer");

    let mut named: Vec<&'static str> = Vec::new();
    for (cell, word) in &cells {
        let (px, py) = on_screen(&app, *cell);
        let Some(hit) = click(&mut app, px, py, true) else { continue };
        if hit.chapter != chapter {
            continue;
        }
        let hit_word = hit.word.expect("un double clic vivant doit nommer un mot");
        assert_eq!(hit_word, *word, "le double clic nomme un autre mot");
        let verse = hit.verse.expect("un verset");
        assert!(verse.split(' ').any(|w| w == hit_word), "le mot n'est pas du verset");
        assert!(!hit_word.contains(' '), "le mot porte une espace");
        assert!(!hit_word.is_empty());
        named.push(hit_word);
    }
    assert!(named.len() >= 2, "seulement {} mots designes", named.len());
    assert!(named.windows(2).any(|p| p[0] != p[1]), "tous les points donnent le meme mot");
}

/// A double click above the reading zoom is as dead as a click: there is no
/// word to point at.
#[test]
fn a_double_click_is_dead_where_a_click_is() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    app.build_frame();

    let (px, py) = on_screen(&app, app.layout.chapters[0]);
    assert!(click(&mut app, px, py, true).is_none(), "un double clic vivant sans texte");
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
        assert_eq!(cells.len(), words.len(), "un mot a ete perdu");

        for (cell, word) in &cells {
            assert!(cell.w > 0.0, "le mot {word:?} n'a pas de largeur");
            assert_eq!(cell.y, line.rect.y, "un mot a quitte sa ligne");
            assert_eq!(cell.h, line.rect.h);
        }
        for (i, (cell, word)) in cells.iter().enumerate() {
            assert_eq!(*word, words[i], "l'ordre des mots a change");
            if i > 0 {
                let before = cells[i - 1].0;
                assert!(cell.x + cell.w <= before.x + 1e-3, "deux mots se chevauchent");
            }
        }

        // First word on the right edge, last one inside the line.
        let right = line.rect.x + line.rect.w;
        assert_close(cells[0].0.x + cells[0].0.w, right, 1e-3, "le premier mot n'est pas au bord");
        let last = cells.last().unwrap().0;
        assert!(
            last.x >= line.rect.x - line.rect.w * 0.01,
            "le dernier mot sort de sa ligne"
        );

        // The boxes span exactly the drawn width: the ink of the words plus the
        // spaces between them.
        let ink: f32 = cells.iter().map(|(c, _)| c.w).sum();
        let drawn = app.width_of(line.text, line.size);
        let span = (cells[0].0.x + cells[0].0.w) - last.x;
        assert_close(span, drawn, 1e-2, "les boites ne couvrent pas la largeur dessinee");
        assert!(ink <= drawn + 1e-3, "l'encre depasse la largeur dessinee");
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
    assert_close(cells[0].0.x + cells[0].0.w, row.x + row.w, 1e-3, "le mot n'est pas cale a droite");
    assert_close(cells[0].0.w, app.width_of(word, 2.0), 1e-3, "largeur du mot");
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
        assert!(cell.x >= left - 1e-2, "une lettre passe a gauche du dernier mot");
        assert!(cell.x + cell.w <= right + 1e-2, "une lettre passe a droite du premier mot");
    }
}
