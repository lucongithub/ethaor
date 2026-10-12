//! The breadcrumb: where one is, from the zoom and from the pointer.
//!
//! Two rules hold it. **Its depth is that of the zoom**, and of what the frame
//! really draws: the chapter appears with the numbers, the verse with the text.
//! **Its point is the pointer** when the pointer is on something, the centre of
//! the view otherwise.

use super::*;

/// A path read back as the page reads it: four levels, each one empty or not.
#[derive(Debug, Default, PartialEq)]
pub struct Path {
    pub section: Option<&'static str>,
    pub book: Option<&'static str>,
    pub chapter: Option<&'static str>,
    pub verse: Option<&'static str>,
}

fn read(slots: [u32; 8]) -> Path {
    let level = |i: usize| {
        (slots[i + 1] > 0)
            .then(|| &corpus::text()[slots[i] as usize..(slots[i] + slots[i + 1]) as usize])
    };
    Path { section: level(0), book: level(2), chapter: level(4), verse: level(6) }
}

fn centre(app: &mut App) -> Path {
    let (wx, wy) = app.camera.to_world(app.width / 2.0, app.height / 2.0);
    read(app.path_at(wx, wy, false).unwrap_or([0; 8]))
}

fn under(app: &mut App, px: f32, py: f32) -> Option<Path> {
    let (wx, wy) = app.camera.to_world(px, py);
    app.path_at(wx, wy, true).map(read)
}

/// Screen position of a world box.
fn on_screen(app: &App, r: Rect) -> (f32, f32) {
    let p = app.camera.project(r);
    (p.x + p.w / 2.0, p.y + p.h / 2.0)
}

/// Zooms at the centre until the chapter there is read as text, and gives the
/// number of notches it took.
fn dive_to_text(app: &mut App) -> usize {
    let step = app.zoom_step();
    for notch in 1..=20 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        app.build_frame();
        if centre(app).verse.is_some() {
            return notch;
        }
    }
    panic!("le verset n'arrive jamais sous le centre");
}

// ----------------------------------------------------------- depth by zoom

/// At the framing, the map shows the portions and the books: the breadcrumb
/// says exactly that, and not a chapter nobody can see.
#[test]
fn the_framing_names_a_portion_and_a_book() {
    for (w, h) in WINDOWS {
        let mut app = framed_app(w, h);
        measure_all(&mut app);
        let path = centre(&mut app);
        assert!(path.section.is_some(), "{w}x{h}: aucune portion au cadrage");
        assert!(path.book.is_some(), "{w}x{h}: aucun livre au cadrage");
        assert_eq!(path.chapter, None, "{w}x{h}: un chapitre annonce avant d'etre visible");
        assert_eq!(path.verse, None, "{w}x{h}: un verset annonce au cadrage");
    }
}

/// The chapter arrives **with its number**, at the first notch.
#[test]
fn the_chapter_arrives_with_the_numbers() {
    for (w, h) in WINDOWS {
        let mut app = framed_app(w, h);
        measure_all(&mut app);
        let step = app.zoom_step();
        app.zoom_at(w * 0.5, h * 0.5, step);
        app.build_frame();

        let camera = app.camera;
        assert!(app.chapters_ready(camera), "{w}x{h}: les numeros devaient etre la");
        let path = centre(&mut app);
        assert!(path.chapter.is_some(), "{w}x{h}: pas de chapitre au premier cran");
        assert_eq!(path.verse, None, "{w}x{h}: un verset des le premier cran");
    }
}

/// The verse arrives **with the text**, and it arrives: a dozen notches at
/// most, on every window.
#[test]
fn the_verse_arrives_with_the_text() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let notches = dive_to_text(&mut app);
        assert!(notches <= 14, "{w}x{h}: {notches} crans avant le verset");

        let path = centre(&mut app);
        assert!(path.section.is_some() && path.book.is_some() && path.chapter.is_some());
        let verse = path.verse.expect("un verset");
        assert!(!verse.is_empty());
    }
}

/// **The breadcrumb never says more than the map shows.** On the whole zoom
/// range: a chapter only once the numbers are out, a verse only once the lines
/// are read.
#[test]
fn the_depth_never_exceeds_what_is_drawn() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    for _ in 0..8 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, 1.0 / step);
    }
    for notch in 0..40 {
        app.zoom_at(app.width * 0.5, app.height * 0.5, step);
        app.build_frame();

        let camera = app.camera;
        let ready = app.chapters_ready(camera);
        let path = centre(&mut app);

        if path.chapter.is_some() {
            assert!(ready, "cran {notch}: un chapitre annonce sans numero a l'ecran");
        }
        if path.verse.is_some() {
            // A chapter number is only unique inside its book — א names 39
            // chapters — so the book comes first.
            let book = (0..corpus::book_count())
                .find(|&b| Some(corpus::book_name(b)) == path.book)
                .expect("le livre du verset");
            let chapter = corpus::chapters_of(book)
                .find(|&c| Some(corpus::chapter_label(c)) == path.chapter)
                .expect("le chapitre du verset");
            assert!(
                app.line_height(chapter, camera) >= READABLE_LINE,
                "cran {notch}: un verset annonce sans texte lisible"
            );
        }
        // A level is never filled without the one above it.
        if path.book.is_some() {
            assert!(path.section.is_some(), "cran {notch}: un livre sans portion");
        }
        if path.chapter.is_some() {
            assert!(path.book.is_some(), "cran {notch}: un chapitre sans livre");
        }
        if path.verse.is_some() {
            assert!(path.chapter.is_some(), "cran {notch}: un verset sans chapitre");
        }
    }
}

/// Whatever the depth, the path holds together: the verse is of the chapter,
/// the chapter of the book, the book of the portion.
#[test]
fn the_path_holds_together() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_text(&mut app);

    let path = centre(&mut app);
    let book = (0..corpus::book_count())
        .find(|&b| Some(corpus::book_name(b)) == path.book)
        .expect("un livre nomme");
    let section = treemap::SECTIONS
        .iter()
        .position(|&(_, first, last)| book >= first && book < last)
        .expect("une portion pour ce livre");
    assert_eq!(path.section, Some(corpus::section_name(section)));

    let chapter = corpus::chapters_of(book)
        .find(|&c| Some(corpus::chapter_label(c)) == path.chapter)
        .expect("le chapitre doit etre du livre");
    assert!(
        corpus::verses_of(chapter).any(|v| Some(corpus::verse_label(v)) == path.verse),
        "le verset doit etre du chapitre"
    );
}

/// The verse level carries **a number, not the text of the verse**: it is a
/// position, and it has a navbar to fit in.
#[test]
fn the_verse_level_is_a_number() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_text(&mut app);
    let verse = centre(&mut app).verse.expect("un verset");
    assert!(verse.chars().count() <= 3, "le niveau du verset porte {verse:?}");
    assert!(!verse.contains(' '), "le niveau du verset porte du texte");
}

// -------------------------------------------------------- point by pointer

/// **The pointer names what it is on.** Pointing at a book box gives that book,
/// whatever the centre of the view says.
#[test]
fn the_pointer_names_the_book_it_is_on() {
    let mut app = reference_framed();
    let camera = app.camera;

    let mut named = 0;
    for book in 0..corpus::book_count() {
        let ink = app.book_ink(book, camera).expect("le titre du livre est dessine");
        let (px, py) = (ink.x + ink.w / 2.0, ink.y + ink.h / 2.0);
        if px < 0.0 || py < 0.0 || px > app.width || py > app.height {
            continue;
        }
        let path = under(&mut app, px, py).expect("un titre de livre nomme son livre");
        assert_eq!(path.book, Some(corpus::book_name(book)), "le pointeur nomme un autre livre");
        named += 1;
    }
    assert_eq!(named, corpus::book_count(), "tous les livres sont a l'ecran au cadrage");
}

/// **A portion is named by its own letters, and by nothing else.**
///
/// Its name belongs to no book, so it names the portion alone. And the band
/// around the letters is not the letters: pointing in the air beside them used
/// to name the portion from half a screen away. Checked in the two states the
/// name is drawn in — the cards of the opening, and the band above the books.
#[test]
fn the_pointer_on_a_portion_title_names_the_portion_alone() {
    for framed in [false, true] {
        let mut app = if framed { reference_framed() } else { reference_app() };
        measure_all(&mut app);
        if framed {
            at_framing(&mut app);
            app.build_frame();
        }
        let camera = app.camera;

        for section in 0..3 {
            let ink = app.section_ink(section, camera).expect("le nom de la portion est dessine");
            let (px, py) = (ink.x + ink.w / 2.0, ink.y + ink.h / 2.0);
            let path = under(&mut app, px, py).expect("les lettres nomment leur portion");
            assert_eq!(path.section, Some(corpus::section_name(section)), "portion {section}");
            assert_eq!(path.book, None, "un nom de portion nomme un livre");
            assert_eq!(path.chapter, None, "un nom de portion nomme un chapitre");

            // The air at the left of the same letters: nothing.
            let beside = ink.x - ink.w;
            if beside > 0.0 {
                assert!(
                    under(&mut app, beside, py).is_none(),
                    "l'air a cote du nom de la portion {section} nomme quelque chose"
                );
            }
        }
    }
}

/// Pointing at the air between two portions names nothing — the same rule as a
/// click — and the caller falls back on the centre of the view.
#[test]
fn the_pointer_in_the_air_names_nothing() {
    let mut app = reference_app();
    measure_all(&mut app);

    let last = app.layout.books[corpus::book_count() - 1];
    let (px, py) = on_screen(&app, Rect { y: last.y + last.h + 20.0, h: 1.0, ..last });
    assert!(under(&mut app, px, py).is_none(), "le blanc du bas nomme quelque chose");

    // Left of the map, outside the centred book area.
    let (px, py) = (1.0, app.height * 0.5);
    assert!(under(&mut app, px, py).is_none(), "le noir du bord nomme quelque chose");

    // And the centre, asked for itself, still answers: that is what the page
    // falls back on when there is no pointer at all.
    assert!(centre(&mut app).section.is_some());
}

/// Once the chapters are out, the pointer names the cell it is on — and the
/// cell next to it names the next chapter.
#[test]
fn the_pointer_names_the_cell_it_is_on() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    app.build_frame();

    let mut seen: Vec<&'static str> = Vec::new();
    for chapter in 0..corpus::chapter_count() {
        let (px, py) = on_screen(&app, app.layout.chapters[chapter]);
        if px < 0.0 || py < 0.0 || px > app.width || py > app.height {
            continue;
        }
        let path = under(&mut app, px, py).expect("une case nomme son chapitre");
        assert_eq!(
            path.chapter,
            Some(corpus::chapter_label(chapter)),
            "la case du chapitre {chapter} en nomme un autre"
        );
        assert_eq!(path.book, Some(corpus::book_name(book_of(chapter))));
        seen.push(corpus::chapter_label(chapter));
    }
    assert!(seen.len() > 100, "seulement {} cases a l'ecran", seen.len());
}

/// Deep in a chapter, the pointer follows the **lines**: two lines of two
/// different verses give two different breadcrumbs.
#[test]
fn the_pointer_follows_the_verses_of_a_chapter() {
    let mut app = reference_app();
    measure_all(&mut app);
    dive_to_text(&mut app);

    let centre_path = centre(&mut app);
    let book = (0..corpus::book_count())
        .find(|&b| Some(corpus::book_name(b)) == centre_path.book)
        .expect("un livre sous le centre");
    let chapter = corpus::chapters_of(book)
        .find(|&c| Some(corpus::chapter_label(c)) == centre_path.chapter)
        .expect("un chapitre sous le centre");

    let lines = app.chapter_lines(chapter);
    assert!(lines.len() >= 2, "il faut deux lignes pour comparer");

    // The cells of a book are a few units apart, so a point near the edge of a
    // line can fall nearer to the neighbouring chapter: we only keep the
    // readings that landed on the chapter aimed at.
    let wanted = corpus::chapter_label(chapter);
    let mut verses: Vec<&'static str> = Vec::new();
    for line in &lines {
        let (px, py) = on_screen(&app, line.rect);
        let Some(path) = under(&mut app, px, py) else { continue };
        if path.chapter != Some(wanted) || path.book != centre_path.book {
            continue;
        }
        let verse = path.verse.expect("une ligne du chapitre vise doit nommer son verset");
        if verses.last() != Some(&verse) {
            verses.push(verse);
        }
    }

    assert!(verses.len() >= 2, "le pointeur ne distingue pas deux versets : {verses:?}");
    // In reading order, and each one really of that chapter.
    let order: Vec<&'static str> = corpus::verses_of(chapter).map(corpus::verse_label).collect();
    let ranks: Vec<usize> =
        verses.iter().map(|v| order.iter().position(|o| o == v).expect("verset du chapitre")).collect();
    assert!(ranks.windows(2).all(|p| p[0] < p[1]), "les versets ne descendent pas dans l'ordre");
}

/// Hovering still changes nothing in the drawing: the breadcrumb is read
/// beside the frame, never inside it.
#[test]
fn reading_the_breadcrumb_does_not_touch_the_drawing() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);

    let before = frame(&mut app);
    let (width, middle) = (app.width, app.height * 0.5);
    for i in 0..20 {
        let _ = under(&mut app, width * (i as f32) / 20.0, middle);
    }
    let after = frame(&mut app);
    assert_eq!(before, after, "lire le fil d'Ariane a change le dessin");
}

/// **The pointer names what it is on, not what it is near.**
///
/// A cell is far wider than its number — the stride is the densest book's — so
/// most of a cell is void. Aiming in that void used to name the chapter all
/// the same, several letters away from the pointer: the breadcrumb announced a
/// position the reader had no way to check.
#[test]
fn the_void_of_a_cell_names_no_chapter() {
    let mut app = reference_framed();
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    app.build_frame();

    let camera = app.camera;
    assert!(app.chapters_ready(camera), "les numeros ne sont pas sortis");

    let mut tested = 0usize;
    for chapter in 0..corpus::chapter_count() {
        assert!(!app.readable(chapter), "le chapitre {chapter} est deja lisible");
        let cell = app.layout.chapters[chapter];
        let ink = app.numeral_box(chapter);

        // The number itself, which must still be named.
        let (px, py) = on_screen(&app, ink);
        if px < 0.0 || py < 0.0 || px > app.width || py > app.height {
            continue;
        }
        let on = under(&mut app, px, py).expect("le numero nomme son chapitre");
        assert_eq!(
            on.chapter,
            Some(corpus::chapter_label(chapter)),
            "le numero du chapitre {chapter} en nomme un autre"
        );

        // The void at the left of the same cell, where no ink is: the book,
        // and no further.
        let void = app.camera.project(cell);
        let (vx, vy) = (void.x + void.w * 0.02, void.y + void.h / 2.0);
        if vx < 0.0 || vy < 0.0 || vx > app.width || vy > app.height {
            continue;
        }
        if vx >= on_screen(&app, ink).0 - app.camera.project(ink).w {
            continue; // a narrow cell: the void and the ink overlap
        }
        // Nothing is drawn there: neither the number, nor the title of the
        // book, so nothing is named and the centre of the view takes over.
        assert!(
            under(&mut app, vx, vy).is_none(),
            "le vide de la case {chapter} nomme quelque chose"
        );
        tested += 1;
    }
    assert!(tested > 50, "seulement {tested} cases testees");
}

/// **The void of a book box names no book.** Once the numbers are out, the
/// title falls back into its band and the block takes the top right: most of
/// the box carries no ink, and pointing there used to name a book whose letters
/// were an inch away. On nothing, the pointer names nothing, and the centre of
/// the view takes over — the rule the air between portions already followed.
#[test]
fn the_void_of_a_book_box_names_no_book() {
    let mut app = reference_framed();
    let step = app.zoom_step();
    app.zoom_at(app.width * 0.5, app.height * 0.5, step);
    app.build_frame();

    let camera = app.camera;
    assert!(app.chapters_ready(camera), "les numeros ne sont pas sortis");

    let on_screen_point = |app: &App, x: f32, y: f32| x >= 0.0 && y >= 0.0 && x <= app.width && y <= app.height;
    let mut named = 0usize;
    let mut voids = 0usize;

    for book in 0..corpus::book_count() {
        let Some(ink) = app.book_ink(book, camera) else { continue };
        let rect = camera.project(app.layout.books[book]);

        // The name itself: it names its book, and no chapter — a title belongs
        // to no cell.
        let (tx, ty) = (ink.x + ink.w / 2.0, ink.y + ink.h / 2.0);
        if on_screen_point(&app, tx, ty) {
            let path = under(&mut app, tx, ty).expect("le titre nomme son livre");
            assert_eq!(path.book, Some(corpus::book_name(book)), "le titre du livre {book}");
            assert_eq!(path.chapter, None, "le titre du livre {book} nomme un chapitre");
            named += 1;
        }

        // The void: at the left of the box, below the band where the title
        // sits. No ink of any level is there.
        let (vx, vy) = (rect.x + rect.w * 0.03, ink.y + ink.h * 1.5);
        if !on_screen_point(&app, vx, vy) || vy > rect.y + rect.h {
            continue;
        }
        assert!(
            under(&mut app, vx, vy).is_none(),
            "le vide du livre {book} nomme quelque chose"
        );
        voids += 1;
    }
    assert!(named > 5, "seulement {named} titres testes");
    assert!(voids > 5, "seulement {voids} vides testes");
}
