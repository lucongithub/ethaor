//! Camera, framing, notch and thresholds.
//!
//! The promise checked here is the one of the levels: **the chapter numbers at
//! the first notch, on every window**, the three portions one notch below the
//! framing, and a dive of `DIVE_NOTCHES` notches down to a full-screen chapter.

use super::*;

#[test]
fn projecting_and_coming_back_is_the_identity() {
    let cam = Camera { z: 2.5, x: -40.0, y: 17.0 };
    for (x, y) in [(0.0, 0.0), (100.0, 200.0), (-30.0, 900.0)] {
        let r = cam.project(Rect { x, y, w: 10.0, h: 20.0 });
        let (bx, by) = cam.to_world(r.x, r.y);
        assert_close(bx, x, 1e-3, "retour en x");
        assert_close(by, y, 1e-3, "retour en y");
        assert_close(r.w, 25.0, 1e-3, "largeur projetee");
    }
}

/// `wrap_in` re-roots the camera in a cell holding a copy of the world: **the
/// image does not move**, only the scale comes back down — which is what keeps
/// the floats exact.
#[test]
fn re_rooting_does_not_move_the_image() {
    let mut cam = Camera { z: 3.0, x: 17.0, y: -9.0 };
    let cell = Rect { x: 100.0, y: 200.0, w: 50.0, h: 50.0 };
    let before = cam.project(cell);
    cam.wrap_in(cell);
    let after = cam.project(WORLD);

    assert_close(after.x, before.x, 1e-3, "le monde glisse en x");
    assert_close(after.w, before.w, 1e-3, "le monde change de largeur");
    // The world keeps its proportions, so it is centred vertically in the cell.
    assert_close(
        after.y + after.h / 2.0,
        before.y + before.h / 2.0,
        1e-3,
        "le monde n'est pas centre dans la lettre",
    );
    assert!(after.h <= before.h + 1e-3, "le monde depasse sa lettre");
    assert_close(cam.z, cell.w * 3.0 / WORLD.w, 1e-6, "l'echelle n'a pas ete rabaissee");
}

/// Re-rooting is **a matter of precision**: after a fold the scale must come
/// back near 1, whatever the depth reached.
#[test]
fn a_fold_brings_the_scale_back_down() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    let (px, py) = (app.width * 0.5, app.height * 0.5);

    let mut folds = 0;
    for _ in 0..120 {
        let before = app.level;
        app.zoom_at(px, py, step);
        if app.level > before {
            folds += 1;
            assert!(
                app.camera.z < 10.0,
                "apres repli l'echelle vaut {} : les f32 vont perdre leur precision",
                app.camera.z
            );
        }
        assert!(app.camera.z < 1e7, "l'echelle a derive jusqu'a {}", app.camera.z);
    }
    assert!(folds >= 2, "seulement {folds} replis sur 120 crans");
    assert_eq!(app.level, folds, "la profondeur ne suit pas les replis");
}

/// The descent is one-way: zooming back out never brings the previous Tanakh
/// back.
#[test]
fn the_descent_is_one_way() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    let (px, py) = (app.width * 0.5, app.height * 0.5);
    while app.level == 0 {
        app.zoom_at(px, py, step);
    }
    let reached = app.level;
    for _ in 0..40 {
        app.zoom_at(px, py, 1.0 / step);
    }
    assert_eq!(app.level, reached, "le de-zoom est revenu au monde precedent");
}

/// The framing is the whole world in the window — **unless** that made the
/// numbers illegible at the first notch. Below that floor we crop the map
/// rather than push the levels back.
#[test]
fn the_framing_has_a_floor() {
    for (w, h) in WINDOWS {
        let app = app_at(w, h);
        let contain = (w / WORLD.w).min(h / WORLD.h);
        assert!(app.fit_scale() >= app.base - 1e-6, "le cadrage est passe sous son plancher");
        assert!(app.fit_scale() >= contain - 1e-6, "le cadrage est plus petit que le monde contenu");
        assert_close(app.fit_scale(), contain.max(app.base), 1e-6, "cadrage");
    }
}

/// At launch the whole map is framed and centred.
#[test]
fn the_framing_centres_the_world() {
    for (w, h) in WINDOWS {
        let app = app_at(w, h);
        let world = app.camera.project(WORLD);
        assert_close(world.x + world.w / 2.0, w / 2.0, 0.01, "centre horizontal");
        assert_close(world.y + world.h / 2.0, h / 2.0, 0.01, "centre vertical");
    }
}

/// **The notch is computed, not fixed**: the dive from the framing down to a
/// full-screen chapter always takes `DIVE_NOTCHES` notches, whatever the
/// window.
#[test]
fn the_dive_always_takes_the_same_number_of_notches() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let step = app.zoom_step();
        assert!(step > 1.0, "un cran doit zoomer");
        assert!(step >= ZOOM_STEP - 0.01, "le cran {step} est sous le cran nominal");
        assert!(step < 1.6, "le cran {step} est trop brutal");

        // The target is a cell of the **chapter** layout, the one in place once
        // the numbers are out — the same one `zoom_step` reckons on.
        let detail = if app.detail { &app.layout } else { &app.spare };
        let cell = detail.chapters[0].h.max(0.01);
        let reached = app.fit_scale() * step.powf(DIVE_NOTCHES);
        assert_close(cell * reached / h, 1.0, 0.02, "la plongee n'arrive pas a un chapitre plein");
    }
}

/// On a landscape window the framing is already proportional to the height, so
/// the computation gives exactly the nominal notch.
#[test]
fn a_landscape_window_keeps_the_nominal_notch() {
    let mut app = reference_app();
    measure_all(&mut app);
    assert_close(app.zoom_step(), ZOOM_STEP, 0.01, "le cran de la fenetre de reference");
}

/// **The chapter numbers appear from the first notch**, on all nine windows —
/// and not at the framing, where the map shows only the sections and the
/// books.
#[test]
fn the_numbers_appear_at_the_first_notch_on_every_window() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let camera = app.camera;
        assert!(!app.chapters_ready(camera), "{w}x{h}: les numeros paraissent au cadrage");

        let step = app.zoom_step();
        app.zoom_at(w / 2.0, h / 2.0, step);
        let camera = app.camera;
        assert!(app.chapters_ready(camera), "{w}x{h}: pas de numeros au premier cran");

        // And they are legible: the floor no longer decides anything.
        let widest = app.widest_numeral();
        let detail = if app.detail { &app.layout } else { &app.spare };
        let size = number_in(detail, widest) * camera.z;
        assert!(size >= MIN_NUMBER, "{w}x{h}: numero de {size} px");
    }
}

/// The decision is always taken on the **chapter** layout, the one that will be
/// in place after the swap: judging it on the current one made the level
/// oscillate from one frame to the next.
#[test]
fn the_level_of_detail_does_not_oscillate() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let step = app.zoom_step();
        let mut seen = Vec::new();
        for _ in 0..12 {
            app.zoom_at(w / 2.0, h / 2.0, step);
            app.build_frame();
            seen.push(app.detail);
        }
        // Once on, the chapter layout never goes back off while zooming in.
        let first_on = seen.iter().position(|&d| d);
        assert!(first_on.is_some(), "{w}x{h}: le niveau des chapitres n'arrive jamais");
        assert!(
            seen[first_on.unwrap()..].iter().all(|&d| d),
            "{w}x{h}: le niveau de detail oscille : {seen:?}"
        );
    }
}

/// Swapping the level throws away what depends on the cells, and nothing else:
/// the layouts are exchanged, never rebuilt.
#[test]
fn swapping_the_level_exchanges_the_two_layouts() {
    let mut app = reference_app();
    measure_all(&mut app);
    let overview = app.layout.chapters[0].w;
    let detail = app.spare.chapters[0].w;
    assert!(detail > overview);

    app.set_detail(true);
    assert!(app.detail);
    assert_close(app.layout.chapters[0].w, detail, 1e-6, "la disposition n'a pas ete echangee");
    assert!(app.lines.is_empty(), "la mise en page des versets devait etre jetee");
    assert!(app.number.is_none(), "la taille des numeros devait etre jetee");

    // Idempotent: asking for the level already in place changes nothing.
    app.lines.insert(0, Vec::new());
    app.set_detail(true);
    assert_eq!(app.lines.len(), 1, "un echange inutile a jete le cache");

    app.set_detail(false);
    assert_close(app.layout.chapters[0].w, overview, 1e-6, "retour a la vue d'ensemble");
}

/// **One zoom-out notch, and the map is nothing but the three portions.** The
/// threshold is just under the framing, so the framing itself stays on the
/// books' side.
#[test]
fn the_three_portions_are_one_notch_under_the_framing() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);
        let fit = app.fit_scale();
        assert!(fit >= fit * PORTIONS_UNDER, "le cadrage doit rester du cote des livres");

        let step = app.zoom_step();
        app.zoom_at(w / 2.0, h / 2.0, 1.0 / step);
        assert!(
            app.camera.z < fit * PORTIONS_UNDER,
            "{w}x{h}: un cran de de-zoom ne suffit pas a passer aux portions"
        );
    }
}

/// Zooming out keeps the map centred and never lets the scale fall to zero —
/// the only bound left, and a technical one.
#[test]
fn zooming_out_centres_the_map_and_stops_at_the_technical_floor() {
    let mut app = reference_app();
    measure_all(&mut app);
    let step = app.zoom_step();
    for _ in 0..40 {
        app.zoom_at(app.width * 0.3, app.height * 0.8, 1.0 / step);
        assert!(app.camera.z >= app.floor_scale(), "l'echelle est passee sous le plancher");
        let world = app.camera.project(WORLD);
        assert_close(world.x + world.w / 2.0, app.width / 2.0, 0.05, "la carte n'est plus centree");
        assert_close(world.y + world.h / 2.0, app.height / 2.0, 0.05, "la carte n'est plus centree");
    }
    assert_close(app.camera.z, app.floor_scale(), 1e-9, "le plancher n'est pas atteint");
}

/// Panning moves the world and nothing else; `fit` brings everything back.
#[test]
fn panning_then_framing_comes_back_to_the_start() {
    let mut app = reference_app();
    let before = app.camera;
    app.camera.x += 150.0;
    app.camera.y -= 70.0;
    assert_eq!(app.camera.z, before.z, "un deplacement ne change pas l'echelle");
    app.fit();
    assert_close(app.camera.x, before.x, 1e-3, "retour du cadrage en x");
    assert_close(app.camera.y, before.y, 1e-3, "retour du cadrage en y");
    assert_close(app.camera.z, before.z, 1e-6, "retour du cadrage en z");
}

/// A zoom gesture keeps the point under the cursor still — as long as no pull
/// applies, i.e. when zooming out.
#[test]
fn zooming_out_keeps_the_cursor_still() {
    let mut app = reference_app();
    measure_all(&mut app);
    let (px, py) = (app.width * 0.42, app.height * 0.61);
    let before = app.camera.to_world(px, py);
    app.zoom_at(px, py, 0.9);
    let after = app.camera.to_world(px, py);
    // Below the framing the view re-centres, so we only check the gesture
    // itself, from a zoom well above the framing.
    let mut app = reference_app();
    measure_all(&mut app);
    app.camera.z = app.fit_scale() * 4.0;
    let before2 = app.camera.to_world(px, py);
    app.zoom_at(px, py, 0.9);
    let after2 = app.camera.to_world(px, py);
    assert_close(after2.0, before2.0, 0.5, "le curseur a glisse en x");
    assert_close(after2.1, before2.1, 0.5, "le curseur a glisse en y");
    let _ = (before, after);
}

/// Resizing keeps the state consistent: a window of one pixel, or of none,
/// must not break anything.
#[test]
fn absurd_windows_do_not_break_the_camera() {
    for (w, h) in [(0.0, 0.0), (1.0, 1.0), (1.0, 4000.0), (4000.0, 1.0)] {
        let mut app = app_at(w, h);
        assert!(app.fit_scale().is_finite() && app.fit_scale() > 0.0);
        assert!(app.zoom_step().is_finite() && app.zoom_step() > 1.0);
        app.build_frame();
        assert_eq!(app.commands.len() % STRIDE, 0);
    }
}
