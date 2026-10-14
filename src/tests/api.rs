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

    // The map opens on the three portions alone, one notch below the framing.
    let fit = cam_fit();
    let opening = fit / zoom_step();
    assert_close(cam_z(), opening, opening * 1e-3, "le demarrage n'est pas sur les portions");

    let step = zoom_step();
    assert!(step > 1.0 && step < 1.6, "cran de {step}");

    pan(60.0, -25.0);
    let (x, y) = (cam_x(), cam_y());
    zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
    assert!(cam_z() > fit, "le zoom n'a pas grandi l'echelle");
    assert!((cam_x(), cam_y()) != (x, y), "le zoom n'a pas bouge la camera");

    reset();
    let opening = cam_fit() / zoom_step();
    assert_close(cam_z(), opening, opening * 1e-3, "Echap ne revient pas sur les portions");
    assert_eq!(depth(), 0, "Echap doit revenir au premier monde");
}

/// Resizing keeps the framing when it was framed, and leaves the view alone
/// otherwise.
#[test]
fn resizing_keeps_the_framing() {
    let _hold = hold(1280.0, 800.0);
    converge();
    resize(1920.0, 1080.0);
    let opening = cam_fit() / zoom_step();
    assert_close(cam_z(), opening, opening * 1e-3, "le recadrage n'a pas suivi la fenetre");
    let frame = read_frame();
    assert!(frame.iter().any(Command::is_text), "image vide apres redimensionnement");

    // Zoomed in, a resize must not snap the view back to the framing.
    zoom(960.0, 540.0, zoom_step() * 2.0);
    let z = cam_z();
    resize(1024.0, 620.0);
    assert_eq!(cam_z(), z, "le redimensionnement a recadre une vue zoomee");
}

/// **A click is only live on a passage one can read.** At the framing, where a
/// chapter is a title in a box, nothing is clickable and the cursor says so.
#[test]
fn a_click_is_dead_above_the_reading_zoom() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();

    let (px, py) = {
        let app = app();
        let r = app.camera.project(app.layout.chapters[40]);
        (r.x + r.w / 2.0, r.y + r.h / 2.0)
    };
    assert_eq!(hover(px, py), -1, "le curseur annonce un clic au cadrage");
    assert_eq!(select(px, py), 0, "le clic a touche un verset invisible");
    assert_eq!(select_word(px, py), 0, "le double clic a touche un mot invisible");
    let view = unsafe { std::slice::from_raw_parts(selection_ptr(), 6) };
    assert_eq!(view, [0; 6], "la selection n'a pas ete effacee");
}

/// Once the lines are read, a click gives the passage and a double click the
/// word inside it — the six slots of `selection_ptr`.
#[test]
fn clicking_gives_the_passage_then_the_word() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();

    // Down to the reading zoom, aiming at the centre of the view.
    let (cx, cy) = (REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);
    let step = zoom_step();
    let mut live = false;
    for _ in 1..=20 {
        zoom(cx, cy, step);
        build_frame();
        if hover(cx, cy) >= 0 {
            live = true;
            break;
        }
    }
    assert!(live, "le clic ne s'active jamais en vingt crans");

    let read = |slot: usize| {
        let view = unsafe { std::slice::from_raw_parts(selection_ptr(), 6) };
        (view[slot + 1] > 0)
            .then(|| &corpus::text()[view[slot] as usize..(view[slot] + view[slot + 1]) as usize])
    };
    let indices = || {
        let view = unsafe { std::slice::from_raw_parts(selection_ptr(), 6) };
        (view[0] as usize, view[1] as usize)
    };

    // One click: the passage, and no word.
    assert_eq!(select(cx, cy), 1, "le clic n'a rien touche au zoom de lecture");
    let (book, chapter) = indices();
    let verse = read(2).expect("un verset");
    assert_eq!(book, book_of(chapter));
    assert!(
        corpus::verses_of(chapter).any(|v| corpus::verse_text(v) == verse),
        "le verset doit etre du chapitre"
    );
    assert_eq!(read(4), None, "un clic simple ne doit pas nommer de mot");

    // Double click: the same passage, plus the word under the point.
    assert_eq!(select_word(cx, cy), 1, "le double clic n'a rien touche");
    assert_eq!(indices(), (book, chapter), "le double clic a change de chapitre");
    assert_eq!(read(2), Some(verse), "le double clic a change de verset");
    let word = read(4).expect("un mot");
    assert!(!word.contains(' '), "le mot porte une espace : {word:?}");
    assert!(verse.split(' ').any(|w| w == word), "le mot {word:?} n'est pas du verset");
}

/// `focus` feeds the breadcrumb from the centre of the view: section, book,
/// chapter and verse, each as an (offset, length) into the corpus.
#[test]
fn the_breadcrumb_is_filled_from_the_centre_of_the_view() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();

    let read = |slot: usize| {
        let view = unsafe { std::slice::from_raw_parts(focus_ptr(), 8) };
        (view[slot + 1] > 0)
            .then(|| &corpus::text()[view[slot] as usize..(view[slot] + view[slot + 1]) as usize])
    };

    // At the opening the map is three portions: the breadcrumb says a portion
    // and stops there, since no book is drawn to be named.
    focus();
    assert!(read(0).is_some(), "aucune portion a l'ouverture");
    assert_eq!(read(2), None, "un livre annonce a l'ouverture");

    // One notch in, at the framing: a portion and a book, nothing below.
    let step = zoom_step();
    zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
    build_frame();
    focus();
    assert!(read(0).is_some() && read(2).is_some(), "le fil d'Ariane est vide au cadrage");
    assert_eq!(read(4), None, "un chapitre annonce au cadrage");
    assert_eq!(read(6), None, "un verset annonce au cadrage");

    // Deep in a chapter: the four levels, and they hold together.
    let mut dived = 0;
    for notch in 1..=20 {
        zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
        build_frame();
        focus();
        if read(6).is_some() {
            dived = notch;
            break;
        }
    }
    assert!(dived > 0, "pas de verset en vingt crans");

    let section = read(0).expect("une portion");
    let name = read(2).expect("un livre");
    let label = read(4).expect("un chapitre");
    let verse = read(6).expect("un verset");
    assert!((0..3).any(|s| corpus::section_name(s) == section), "portion inconnue : {section}");
    let book = (0..corpus::book_count())
        .find(|&b| corpus::book_name(b) == name)
        .unwrap_or_else(|| panic!("livre inconnu : {name}"));
    let chapter = corpus::chapters_of(book)
        .find(|&c| corpus::chapter_label(c) == label)
        .expect("le chapitre doit etre du livre");
    assert!(
        corpus::verses_of(chapter).any(|v| corpus::verse_label(v) == verse),
        "le verset doit etre du chapitre"
    );
}

/// `focus_at` is the hover: it answers 1 when the pointer is on something, and
/// 0 when it is on nothing — falling back on the centre of the view, so that
/// the navbar never blinks out.
#[test]
fn the_breadcrumb_follows_the_pointer() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();

    let read = |slot: usize| {
        let view = unsafe { std::slice::from_raw_parts(focus_ptr(), 8) };
        (view[slot + 1] > 0)
            .then(|| &corpus::text()[view[slot] as usize..(view[slot] + view[slot + 1]) as usize])
    };

    // One notch in: the books are drawn, hence nameable.
    zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, zoom_step());
    build_frame();

    // Two different titles under the pointer, two different breadcrumbs. On
    // the letters, which alone name their book.
    let mut named: Vec<&str> = Vec::new();
    for book in [0usize, 7, 26, 38] {
        let r = {
            let app = app();
            let camera = app.camera;
            app.book_ink(book, camera).expect("le titre du livre est dessine")
        };
        let (px, py) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(focus_at(px, py), 1, "le livre {book} ne nomme rien");
        assert_eq!(read(2), Some(corpus::book_name(book)), "le pointeur nomme un autre livre");
        named.push(read(0).expect("une portion"));
    }
    assert!(named.contains(&corpus::section_name(0)) && named.contains(&corpus::section_name(2)));

    // The black at the left edge names nothing, and the breadcrumb empties:
    // keeping the last position would name a place the pointer has left.
    assert_eq!(focus_at(1.0, REFERENCE.1 / 2.0), 0, "le bord de l'ecran nomme un livre");
    assert_eq!(read(0), None, "le fil d'Ariane a garde la derniere position");
    assert_eq!(read(2), None, "un livre survit au pointeur dans le vide");

    // Asked for itself, the centre answers again: that is the fallback when
    // there is no pointer on the canvas at all.
    focus();
    assert!(read(0).is_some(), "le centre ne repond plus");
}

/// **A mouse gone from the canvas empties the breadcrumb**; a lifted finger
/// hands it to the centre of the view. The mouse often leaves onto the
/// breadcrumb row itself, and the centre then named a portion under the
/// cursor, which read as a selection.
#[test]
fn a_mouse_gone_names_nothing_and_a_finger_lifted_hands_over_to_the_centre() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    let section = || unsafe { std::slice::from_raw_parts(focus_ptr(), 8) }[1];

    // At the opening, the centre of the view names a portion.
    focus();
    assert!(section() > 0, "le centre ne nomme rien a l'ouverture");

    leave(1);
    focus();
    assert_eq!(section(), 0, "une souris partie laisse un nom dans le fil d'Ariane");

    // Back on the canvas: the pointer speaks again, and once gone again
    // without a mouse, so does the centre.
    focus_at(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);
    leave(0);
    focus();
    assert!(section() > 0, "un doigt leve doit rendre la parole au centre");

    // Leaving with a mouse, then coming back, is a pointer like any other.
    leave(1);
    focus_at(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);
    focus();
    assert!(section() > 0, "le retour du pointeur ne leve pas l'absence");
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
