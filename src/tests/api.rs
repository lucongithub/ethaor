//! The interface the page really calls.
//!
//! These functions share one global state, so they are serialized here under a
//! single lock: a test holds the module the whole time it drives it.

use std::sync::{Mutex, MutexGuard};

use super::*;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Takes the module, framed on a window, as the page does at startup.
fn hold(width: f32, height: f32) -> MutexGuard<'static, ()> {
    let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(init(width, height), corpus::book_count() as u32, "init ne rend pas le nombre de livres");
    guard
}

/// Feeds the module every width it asks for, exactly as the page does: read
/// `pending`, measure, call `set_ratio`, ask for another frame.
fn converge() {
    for _ in 0..3 {
        build_frame();
        let app = app();
        let pending: Vec<u32> = app.pending.clone();
        if pending.is_empty() {
            return;
        }
        for pair in pending.chunks(2) {
            let text = &corpus::text()[pair[0] as usize..(pair[0] + pair[1]) as usize];
            set_ratio(pair[0], pair[1], hier::letter_count(text).max(1) as f32 * MEASURED);
        }
    }
    build_frame();
}

fn read_frame() -> Vec<Command> {
    let words = build_frame() as usize;
    assert_eq!(words % frame_stride() as usize, 0, "la liste n'est pas un multiple du pas");
    let app = app();
    assert_eq!(app.commands.len(), words);
    commands(app)
}

#[test]
fn the_page_can_draw_a_frame_from_a_cold_start() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    assert_eq!(frame_stride(), STRIDE as u32);
    let commands = read_frame();
    assert!(!commands.is_empty(), "image vide au demarrage");
    assert!(!frame_ptr().is_null());
    assert!(pending_len() > 0, "la premiere image doit reclamer des mesures");
    assert!(!pending_ptr().is_null());
    assert_eq!(text_len() as usize, corpus::text().len());
    assert!(!text_ptr().is_null());
    assert_eq!(depth(), 0);
}

/// The gestures the page sends: wheel, drag, Escape.
#[test]
fn the_gestures_move_the_camera_and_come_back() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();

    let fit = cam_fit();
    assert_close(cam_z(), fit, fit * 1e-3, "le demarrage n'est pas cadre");

    let step = zoom_step();
    assert!(step > 1.0 && step < 1.6, "cran de {step}");

    pan(60.0, -25.0);
    let (x, y) = (cam_x(), cam_y());
    zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
    assert!(cam_z() > fit, "le zoom n'a pas grandi l'echelle");
    assert!((cam_x(), cam_y()) != (x, y), "le zoom n'a pas bouge la camera");

    reset();
    assert_close(cam_z(), cam_fit(), cam_fit() * 1e-3, "Echap ne recadre pas");
    assert_eq!(depth(), 0, "Echap doit revenir au premier monde");
}

/// Resizing keeps the framing when it was framed, and leaves the view alone
/// otherwise.
#[test]
fn resizing_keeps_the_framing() {
    let _hold = hold(1280.0, 800.0);
    converge();
    resize(1920.0, 1080.0);
    assert_close(cam_z(), cam_fit(), cam_fit() * 1e-3, "le recadrage n'a pas suivi la fenetre");
    let frame = read_frame();
    assert!(frame.iter().any(Command::is_text), "image vide apres redimensionnement");

    // Zoomed in, a resize must not snap the view back to the framing.
    zoom(960.0, 540.0, zoom_step() * 2.0);
    let z = cam_z();
    resize(1024.0, 620.0);
    assert_eq!(cam_z(), z, "le redimensionnement a recadre une vue zoomee");
}

/// `select` fills the four slots — book, chapter, offset and length of the
/// verse — and answers 0 when nothing was hit.
#[test]
fn selecting_gives_the_full_path() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();

    let (book, chapter, px, py) = {
        let app = app();
        // The centre of a cell, brought back to the screen.
        let chapter = 40;
        let cell = app.layout.chapters[chapter];
        let r = app.camera.project(cell);
        (book_of(chapter), chapter, r.x + r.w / 2.0, r.y + r.h / 2.0)
    };

    assert_eq!(hover(px, py), chapter as i32, "le survol ne designe pas la case visee");
    assert_eq!(select(px, py), 1, "le clic n'a rien touche");

    let view = unsafe { std::slice::from_raw_parts(selection_ptr(), 4) };
    assert_eq!(view[0] as usize, book);
    assert_eq!(view[1] as usize, chapter);
    if view[3] > 0 {
        let text = &corpus::text()[view[2] as usize..(view[2] + view[3]) as usize];
        assert!(corpus::verses_of(chapter).any(|v| corpus::verse_text(v) == text));
    }

    // Far outside the map: nothing, and the selection is cleared.
    let outside = {
        let app = app();
        app.camera.project(WORLD)
    };
    let (ox, oy) = (outside.x - 500.0, outside.y - 500.0);
    assert_eq!(hover(ox, oy), -1, "le survol designe quelque chose dans le vide");
    assert_eq!(select(ox, oy), 0);
    let view = unsafe { std::slice::from_raw_parts(selection_ptr(), 4) };
    assert_eq!(view, [0, 0, 0, 0], "la selection n'a pas ete effacee");
}

/// `focus` feeds the breadcrumb: section, book and chapter under the centre of
/// the view, each as an (offset, length) into the corpus.
#[test]
fn the_breadcrumb_is_filled_from_the_centre_of_the_view() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    let step = zoom_step();
    for _ in 0..6 {
        zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
    }
    focus();
    let view = unsafe { std::slice::from_raw_parts(focus_ptr(), 6) };
    let read = |slot: &[u32]| &corpus::text()[slot[0] as usize..(slot[0] + slot[1]) as usize];

    assert!(view[1] > 0 && view[3] > 0 && view[5] > 0, "le fil d'Ariane est vide");
    let section = read(&view[0..2]);
    let book = read(&view[2..4]);
    let chapter = read(&view[4..6]);
    assert!((0..3).any(|s| corpus::section_name(s) == section), "portion inconnue : {section}");
    let book = (0..corpus::book_count())
        .find(|&b| corpus::book_name(b) == book)
        .unwrap_or_else(|| panic!("livre inconnu : {book}"));
    assert!(
        corpus::chapters_of(book).any(|c| corpus::chapter_label(c) == chapter),
        "le chapitre {chapter} n'est pas du livre"
    );
}

/// A font change forgets every measured width: a width taken on a fallback font
/// would be wrong for good.
#[test]
fn a_font_change_forgets_every_width() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    assert_eq!(pending_len(), 0, "les mesures n'ont pas converge");

    clear_ratios();
    assert!(app().ratios.is_empty(), "les mesures n'ont pas ete oubliees");
    let frame = read_frame();
    assert!(frame.iter().any(Command::is_text), "image vide apres oubli des mesures");
    assert!(pending_len() > 0, "les mesures ne sont pas redemandees");
}

/// The probes the sondes read: they must answer on every state, and never lie
/// about the depth.
#[test]
fn the_probes_answer_on_every_state() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    let (cx, cy) = (REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);

    assert_eq!(probe_nest(-1000.0, -1000.0), -1.0, "un nid dans le vide");
    assert_eq!(probe_letter(-1000.0, -1000.0), -1.0, "une lettre dans le vide");
    assert!(probe_book(0, 2) > 0.0 && probe_book(0, 3) > 0.0);
    assert!(probe_chapter(0, 2) > 0.0);
    assert_eq!(probe_book(0, 0), app().layout.books[0].x);
    assert_eq!(probe_chapter(5, 1), app().layout.chapters[5].y);

    let step = zoom_step();
    let mut folded = false;
    for _ in 0..120 {
        zoom(cx, cy, step);
        assert!(probe_fade() == 0.0 || probe_fade() == 1.0);
        assert!(probe_points() <= 1);
        if depth() > 0 {
            folded = true;
        }
    }
    assert!(folded, "aucun repli en 120 crans");
    assert!(probe_nest(cx, cy) >= -1.0);
    assert!(probe_letter(cx, cy) >= -1.0);
}

/// Everything the page needs to paint is reachable from the module's memory: a
/// frame is a flat list of words, and a fragment is an offset into the corpus.
#[test]
fn a_frame_can_be_painted_from_the_module_memory_alone() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    let words = build_frame() as usize;
    let list = unsafe { std::slice::from_raw_parts(frame_ptr(), words) };
    let text = unsafe { std::slice::from_raw_parts(text_ptr(), text_len() as usize) };

    let mut drawn = 0;
    for record in list.chunks(frame_stride() as usize) {
        if record[0] != KIND_TEXT {
            continue;
        }
        let (offset, len) = (record[7] as usize, record[8] as usize);
        let fragment = std::str::from_utf8(&text[offset..offset + len])
            .expect("un fragment doit etre de l'UTF-8 valide");
        assert!(!fragment.is_empty());
        drawn += 1;
    }
    assert!(drawn > 0, "rien a peindre");
}
