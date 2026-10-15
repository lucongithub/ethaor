//! The text lands after the map: until it does, the map is whole but the
//! reading level stays closed — nothing drawn, clicked or named needs a verse
//! that is not there — and it opens, at the same zoom, the moment it lands.

use super::*;

/// Offset in the corpus where the verses start: names and labels lie before,
/// verses and display strings from here on.
fn verses_start() -> u32 {
    corpus::offset_of(corpus::verse_text(0)) as u32
}

/// Zooms at the centre until a click goes live there, and gives the notch.
fn notches_to_reading(width: f32, height: f32) -> usize {
    let mut app = app_at(width, height);
    measure_all(&mut app);
    let step = app.zoom_step();
    let (cx, cy) = (width * 0.5, height * 0.5);
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

/// **Before the text, the dive goes on with the numbers**: well past the
/// reading level no frame is black, no verse is drawn, clicked or named —
/// and the moment the text lands the verses appear.
#[test]
fn the_map_stays_whole_until_the_text_lands() {
    for (w, h) in WINDOWS {
        let notches = notches_to_reading(w, h);

        let mut app = app_at(w, h);
        app.text = false;
        measure_all(&mut app);
        let step = app.zoom_step();
        let (cx, cy) = (w * 0.5, h * 0.5);
        for notch in 0..notches + 5 {
            app.zoom_at(cx, cy, step);
            app.build_frame();
            let drawn = commands(&app);
            assert!(drawn.iter().any(|c| c.is_text()), "{w}x{h}, cran {notch}: image vide sans le texte");
            assert!(
                drawn.iter().filter(|c| c.is_text()).all(|c| c.offset < verses_start()),
                "{w}x{h}, cran {notch}: un verset dessine avant l'arrivee du texte"
            );
            let (wx, wy) = app.camera.to_world(cx, cy);
            assert!(app.clickable_at(wx, wy).is_none(), "{w}x{h}: un clic vivant sans texte");
            assert!(app.path_at(wx, wy, false).is_none_or(|p| p[7] == 0), "{w}x{h}: un verset nomme sans texte");
        }

        // The text lands: the verses come.
        app.text = true;
        let mut opened = false;
        for _ in 0..notches + 2 {
            app.zoom_at(cx, cy, step);
            measure_all(&mut app);
            if commands(&app).iter().any(|c| c.is_text() && c.offset >= verses_start()) {
                opened = true;
                break;
            }
        }
        assert!(opened, "{w}x{h}: le texte arrive mais aucun verset n'apparait");
    }
}

/// The map alone is the overview: at the framing, nothing drawn without the
/// text differs from what is drawn with it.
#[test]
fn the_overview_needs_no_verse() {
    let with = reference_framed();
    let mut without = app_at(REFERENCE.0, REFERENCE.1);
    without.text = false;
    measure_all(&mut without);
    at_framing(&mut without);
    without.build_frame();
    assert_eq!(commands(&without), commands(&with), "la vue d'ensemble depend du texte");
}



/// **A chapter's tile is asked for before its lines are legible**: diving at
/// the centre with no verse in, the chapter under it is asked two notches or
/// more ahead of the reading level — so its verses travel while the zoom
/// closes in. Nothing is asked twice, only chapters on screen are, and each
/// run asks for its chapters' very bytes.
#[test]
fn tiles_are_asked_for_ahead_of_the_reading_level() {
    for (w, h) in WINDOWS {
        let reading = notches_to_reading(w, h);
        let mut app = app_at(w, h);
        app.text = false;
        measure_all(&mut app);
        let step = app.zoom_step();
        let (cx, cy) = (w * 0.5, h * 0.5);
        let mut asked = vec![0u32; corpus::chapter_count()];
        let mut centre_asked = None;
        for notch in 1..=reading {
            app.zoom_at(cx, cy, step);
            app.build_frame();
            let runs = app.want_tiles();
            for run in app.tiles.chunks(4).take(runs) {
                let (first, end) = (run[0] as usize, run[1] as usize);
                assert_eq!(corpus::tile_range(first..end), (run[2] as usize, run[3] as usize), "{w}x{h}: tranche fausse");
                for (c, count) in asked.iter_mut().enumerate().take(end).skip(first) {
                    *count += 1;
                    let cell = app.camera.project(app.layout.chapters[c]);
                    assert!(app.visible(cell), "{w}x{h}: chapitre {c} demande hors de l'ecran");
                }
            }
            let (wx, wy) = app.camera.to_world(cx, cy);
            if let Some((_, chapter, _, _)) = app.locate(wx, wy)
                && centre_asked.is_none()
                && asked[chapter] > 0
            {
                centre_asked = Some(notch);
            }
        }
        assert!(asked.iter().all(|&n| n <= 1), "{w}x{h}: un chapitre demande deux fois");
        let at = centre_asked.unwrap_or_else(|| panic!("{w}x{h}: le chapitre vise n'est jamais demande"));
        assert!(at + 2 <= reading, "{w}x{h}: demande au cran {at}, lisible au cran {reading}");
    }
}

/// **The text follows the visitor**: the next book to fetch is the one under
/// the centre of the view, then the nearest; every book is asked for exactly
/// once; and when the view moves, the next one is the nearest to where it
/// now is.
#[test]
fn books_are_fetched_nearest_to_the_view_first() {
    let mut app = reference_framed();
    let (cx, cy) = app.camera.to_world(app.width / 2.0, app.height / 2.0);
    let first = app.next_book().expect("un livre");
    assert!(
        (0..corpus::book_count()).all(|b| gap_to(app.layout.books[first], cx, cy) <= gap_to(app.layout.books[b], cx, cy)),
        "le premier livre n'est pas le plus proche du centre"
    );

    // The view moves to the last book: the next asked is it.
    let last = corpus::book_count() - 1;
    let target = app.layout.books[last];
    let (tx, ty) = (target.x + target.w / 2.0, target.y + target.h / 2.0);
    app.camera.x += app.width / 2.0 - (tx * app.camera.z + app.camera.x);
    app.camera.y += app.height / 2.0 - (ty * app.camera.z + app.camera.y);
    assert_eq!(app.next_book(), Some(last), "le texte ne suit pas la vue");

    let mut seen = vec![false; corpus::book_count()];
    seen[first] = true;
    seen[last] = true;
    while let Some(b) = app.next_book() {
        assert!(!seen[b], "le livre {b} demande deux fois");
        seen[b] = true;
    }
    assert!(seen.iter().all(|&s| s), "un livre jamais demande");
}
