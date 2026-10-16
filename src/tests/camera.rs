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
        assert_close(r.w, 25.0, 1e-3, "projected width");
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

    assert_close(after.x, before.x, 1e-3, "the world slides in x");
    assert_close(after.w, before.w, 1e-3, "the world changes width");
    // The world keeps its proportions, so it is centred vertically in the cell.
    assert_close(
        after.y + after.h / 2.0,
        before.y + before.h / 2.0,
        1e-3,
        "the world is not centred in the letter",
    );
    assert!(after.h <= before.h + 1e-3, "the world goes past its letter");
    assert_close(cam.z, cell.w * 3.0 / WORLD.w, 1e-6, "the scale was not brought down");
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
                "after a fold the scale is {}: the f32s will lose their precision",
                app.camera.z
            );
        }
        assert!(app.camera.z < 1e7, "the scale drifted up to {}", app.camera.z);
    }
    assert!(folds >= 2, "only {folds} folds over 120 notches");
    assert_eq!(app.level, folds, "the depth does not follow the folds");
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
    assert_eq!(app.level, reached, "the zoom-out went back to the previous world");
}

/// The framing is the whole world in the window — **unless** that made the
/// numbers illegible at the first notch. Below that floor we crop the map
/// rather than push the levels back.
#[test]
fn the_framing_has_a_floor() {
    for (w, h) in WINDOWS {
        let app = app_at(w, h);
        let contain = (w / WORLD.w).min(h / WORLD.h);
        assert!(app.fit_scale() >= app.base - 1e-6, "the framing went under its floor");
        assert!(app.fit_scale() >= contain - 1e-6, "the framing is smaller than the world it holds");
        assert_close(app.fit_scale(), contain.max(app.base), 1e-6, "cadrage");
    }
}

/// At launch the whole map is framed and centred.
#[test]
fn the_framing_centres_the_world() {
    for (w, h) in WINDOWS {
        let app = app_at(w, h);
        let world = app.camera.project(WORLD);
        assert_close(world.x + world.w / 2.0, w / 2.0, 0.01, "horizontal centre");
        assert_close(world.y + world.h / 2.0, h / 2.0, 0.01, "vertical centre");
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
        assert!(step > 1.0, "a notch must zoom");
        assert!(step >= ZOOM_STEP - 0.01, "notch {step} is under the nominal notch");
        assert!(step < 1.6, "notch {step} is too abrupt");

        // The target is a cell of the **chapter** layout, the one in place once
        // the numbers are out — the same one `zoom_step` reckons on.
        let detail = if app.detail { &app.layout } else { &app.spare };
        let cell = detail.chapters[0].h.max(0.01);
        let reached = app.fit_scale() * step.powf(DIVE_NOTCHES);
        assert_close(cell * reached / h, 1.0, 0.02, "the dive does not reach a full chapter");
    }
}

/// On a landscape window the framing is already proportional to the height, so
/// the computation gives exactly the nominal notch.
#[test]
fn a_landscape_window_keeps_the_nominal_notch() {
    let mut app = reference_app();
    measure_all(&mut app);
    assert_close(app.zoom_step(), ZOOM_STEP, 0.01, "the notch of the reference window");
}

/// **The three levels are one notch apart, and the map opens on the first.**
///
/// The portions alone, then the thirty-nine books, then the chapters: the
/// Tanakh is read in three parts before being read in books, and the opening
/// image says so instead of showing everything at once. The framing has a
/// floor (`base_scale`) precisely so that the ladder falls on the same notches
/// whatever the window — a phone and a wide desktop climb it alike.
#[test]
fn the_three_levels_are_one_notch_apart_on_every_window() {
    // What `draw_world` asks to show the portions alone.
    let portions = |app: &App| app.camera.z < app.fit_scale() * PORTIONS_UNDER;

    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        measure_all(&mut app);

        // The opening: the three portions, and nothing else.
        assert!(portions(&app), "{w}x{h}: the opening does not show the portions alone");
        let camera = app.camera;
        assert!(!app.chapters_ready(camera), "{w}x{h}: numbers at the opening");

        // One notch: the books.
        let step = app.zoom_step();
        app.zoom_at(w / 2.0, h / 2.0, step);
        app.build_frame();
        assert!(!portions(&app), "{w}x{h}: still the portions at the first notch");
        let camera = app.camera;
        assert!(!app.chapters_ready(camera), "{w}x{h}: numbers from the first notch");

        // Two notches: the chapters — and legible, the floor deciding nothing.
        let step = app.zoom_step();
        app.zoom_at(w / 2.0, h / 2.0, step);
        app.build_frame();
        let camera = app.camera;
        assert!(app.chapters_ready(camera), "{w}x{h}: no numbers at the second notch");
        let widest = app.widest_numeral();
        let detail = if app.detail { &app.layout } else { &app.spare };
        let size = number_in(detail, widest) * camera.z;
        assert!(size >= MIN_NUMBER, "{w}x{h}: number of {size} px");
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
        assert!(first_on.is_some(), "{w}x{h}: the chapter level never comes");
        assert!(
            seen[first_on.unwrap()..].iter().all(|&d| d),
            "{w}x{h}: the detail level oscillates: {seen:?}"
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
    assert_close(app.layout.chapters[0].w, detail, 1e-6, "the layout was not swapped");
    assert!(app.lines.is_empty(), "the verse layout should have been dropped");
    assert!(app.number.is_none(), "the number size should have been dropped");

    // Idempotent: asking for the level already in place changes nothing.
    app.lines.insert(0, Vec::new());
    app.set_detail(true);
    assert_eq!(app.lines.len(), 1, "a needless swap dropped the cache");

    app.set_detail(false);
    assert_close(app.layout.chapters[0].w, overview, 1e-6, "back to the overview");
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
        assert!(fit >= fit * PORTIONS_UNDER, "the framing must stay on the books' side");

        let step = app.zoom_step();
        app.zoom_at(w / 2.0, h / 2.0, 1.0 / step);
        assert!(
            app.camera.z < fit * PORTIONS_UNDER,
            "{w}x{h}: one zoom-out notch is not enough to reach the portions"
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
        assert!(app.camera.z >= app.floor_scale(), "the scale went under the floor");
        let world = app.camera.project(WORLD);
        assert_close(world.x + world.w / 2.0, app.width / 2.0, 0.05, "the map is no longer centred");
        assert_close(world.y + world.h / 2.0, app.height / 2.0, 0.05, "the map is no longer centred");
    }
    assert_close(app.camera.z, app.floor_scale(), 1e-9, "the floor is not reached");
}

/// Panning moves the world and nothing else; `fit` brings everything back.
#[test]
fn panning_then_framing_comes_back_to_the_start() {
    let mut app = reference_app();
    let before = app.camera;
    app.camera.x += 150.0;
    app.camera.y -= 70.0;
    assert_eq!(app.camera.z, before.z, "a move does not change the scale");
    app.fit();
    assert_close(app.camera.x, before.x, 1e-3, "framing back in x");
    assert_close(app.camera.y, before.y, 1e-3, "framing back in y");
    assert_close(app.camera.z, before.z, 1e-6, "framing back in z");
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
    assert_close(after2.0, before2.0, 0.5, "the cursor slid in x");
    assert_close(after2.1, before2.1, 0.5, "the cursor slid in y");
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
