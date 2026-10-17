//! The interface the page really calls.
//!
//! These functions share one global state, so they are serialized here under a
//! single lock: a test holds the module the whole time it drives it.

use std::sync::{Mutex, MutexGuard};

use super::*;
use crate::wasm::els::*;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Takes the module, framed on a window, as the page does at startup.
fn hold(width: f32, height: f32) -> MutexGuard<'static, ()> {
    let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(init(width, height), corpus::book_count() as u32, "init does not return the number of books");
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
            set_ratio(pair[0], pair[1], letter::letter_count(text).max(1) as f32 * MEASURED);
        }
    }
    build_frame();
}

fn read_frame() -> Vec<Command> {
    let words = build_frame() as usize;
    assert_eq!(words % frame_stride() as usize, 0, "the list is not a multiple of the stride");
    let app = app();
    assert_eq!(app.commands.len(), words);
    commands(app)
}

#[test]
fn the_page_can_draw_a_frame_from_a_cold_start() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    assert_eq!(frame_stride(), STRIDE as u32);
    let commands = read_frame();
    assert!(!commands.is_empty(), "empty image at start");
    assert!(!frame_ptr().is_null());
    assert!(pending_len() > 0, "the first image must ask for measurements");
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
    assert_close(cam_z(), opening, opening * 1e-3, "the start is not on the portions");

    let step = zoom_step();
    assert!(step > 1.0 && step < 1.6, "notch of {step}");

    pan(60.0, -25.0);
    let (x, y) = (cam_x(), cam_y());
    zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
    assert!(cam_z() > fit, "the zoom did not grow the scale");
    assert!((cam_x(), cam_y()) != (x, y), "the zoom did not move the camera");

    reset();
    let opening = cam_fit() / zoom_step();
    assert_close(cam_z(), opening, opening * 1e-3, "Escape does not go back to the portions");
    assert_eq!(depth(), 0, "Escape must go back to the first world");
}

/// Resizing keeps the framing when it was framed, and leaves the view alone
/// otherwise.
#[test]
fn resizing_keeps_the_framing() {
    let _hold = hold(1280.0, 800.0);
    converge();
    resize(1920.0, 1080.0);
    let opening = cam_fit() / zoom_step();
    assert_close(cam_z(), opening, opening * 1e-3, "the framing did not follow the window");
    let frame = read_frame();
    assert!(frame.iter().any(Command::is_text), "empty image after resizing");

    // Zoomed in, a resize must not snap the view back to the framing.
    zoom(960.0, 540.0, zoom_step() * 2.0);
    let z = cam_z();
    resize(1024.0, 620.0);
    assert_eq!(cam_z(), z, "resizing reframed a zoomed view");
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
    assert_eq!(hover(px, py), -1, "the cursor announces a click at the framing");
    assert_eq!(select(px, py), 0, "the click hit an invisible verse");
    assert_eq!(select_word(px, py), 0, "the double click hit an invisible word");
    let view = unsafe { std::slice::from_raw_parts(selection_ptr(), 6) };
    assert_eq!(view, [0; 6], "the selection was not cleared");
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
    assert!(live, "the click never becomes live in twenty notches");

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
    assert_eq!(select(cx, cy), 1, "the click hit nothing at reading zoom");
    let (book, chapter) = indices();
    let verse = read(2).expect("a verse");
    assert_eq!(book, book_of(chapter));
    assert!(
        corpus::verses_of(chapter).any(|v| corpus::verse_text(v) == verse),
        "the verse must be of the chapter"
    );
    assert_eq!(read(4), None, "a single click must not name a word");

    // Double click: the same passage, plus the word under the point.
    assert_eq!(select_word(cx, cy), 1, "the double click hit nothing");
    assert_eq!(indices(), (book, chapter), "the double click changed chapter");
    assert_eq!(read(2), Some(verse), "the double click changed verse");
    let word = read(4).expect("a word");
    assert!(!word.contains(' '), "the word holds a space: {word:?}");
    assert!(verse.split(' ').any(|w| w == word), "the word {word:?} is not of the verse");
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
    assert!(read(0).is_some(), "no portion at the opening");
    assert_eq!(read(2), None, "a book announced at the opening");

    // One notch in, at the framing: a portion and a book, nothing below.
    let step = zoom_step();
    zoom(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0, step);
    build_frame();
    focus();
    assert!(read(0).is_some() && read(2).is_some(), "the breadcrumb is empty at the framing");
    assert_eq!(read(4), None, "a chapter announced at the framing");
    assert_eq!(read(6), None, "a verse announced at the framing");

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
    assert!(dived > 0, "no verse in twenty notches");

    let section = read(0).expect("a portion");
    let name = read(2).expect("a book");
    let label = read(4).expect("a chapter");
    let verse = read(6).expect("a verse");
    assert!((0..3).any(|s| corpus::section_name(s) == section), "unknown portion: {section}");
    let book = (0..corpus::book_count())
        .find(|&b| corpus::book_name(b) == name)
        .unwrap_or_else(|| panic!("unknown book: {name}"));
    let chapter = corpus::chapters_of(book)
        .find(|&c| corpus::chapter_label(c) == label)
        .expect("the chapter must be of the book");
    assert!(
        corpus::verses_of(chapter).any(|v| corpus::verse_label(v) == verse),
        "the verse must be of the chapter"
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
            app.book_ink(book, camera).expect("the book title is drawn")
        };
        let (px, py) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(focus_at(px, py), 1, "book {book} names nothing");
        assert_eq!(read(2), Some(corpus::book_name(book)), "the pointer names another book");
        named.push(read(0).expect("a portion"));
    }
    assert!(named.contains(&corpus::section_name(0)) && named.contains(&corpus::section_name(2)));

    // The black at the left edge names nothing, and the breadcrumb empties:
    // keeping the last position would name a place the pointer has left.
    assert_eq!(focus_at(1.0, REFERENCE.1 / 2.0), 0, "the edge of the screen names a book");
    assert_eq!(read(0), None, "the breadcrumb kept the last position");
    assert_eq!(read(2), None, "a book survives the pointer in the void");

    // Asked for itself, the centre answers again: that is the fallback when
    // there is no pointer on the canvas at all.
    focus();
    assert!(read(0).is_some(), "the centre no longer answers");
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
    assert!(section() > 0, "the centre names nothing at the opening");

    leave(1);
    focus();
    assert_eq!(section(), 0, "a mouse gone leaves a name in the breadcrumb");

    // Back on the canvas: the pointer speaks again, and once gone again
    // without a mouse, so does the centre.
    focus_at(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);
    leave(0);
    focus();
    assert!(section() > 0, "a lifted finger must hand over to the centre");

    // Leaving with a mouse, then coming back, is a pointer like any other.
    leave(1);
    focus_at(REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);
    focus();
    assert!(section() > 0, "the pointer's return does not lift the absence");
}

/// A font change forgets every measured width: a width taken on a fallback font
/// would be wrong for good.
#[test]
fn a_font_change_forgets_every_width() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    assert_eq!(pending_len(), 0, "the measurements did not converge");

    clear_ratios();
    assert!(app().ratios.is_empty(), "the measurements were not forgotten");
    let frame = read_frame();
    assert!(frame.iter().any(Command::is_text), "empty image after forgetting the measurements");
    assert!(pending_len() > 0, "the measurements are not asked again");
}

/// The probes the sondes read: they must answer on every state, and never lie
/// about the depth.
#[test]
fn the_probes_answer_on_every_state() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    let (cx, cy) = (REFERENCE.0 / 2.0, REFERENCE.1 / 2.0);

    assert_eq!(probe_nest(-1000.0, -1000.0), -1.0, "a nest in the void");
    assert_eq!(probe_letter(-1000.0, -1000.0), -1.0, "a letter in the void");
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
    assert!(folded, "no fold in 120 notches");
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
            .expect("a fragment must be valid UTF-8");
        assert!(!fragment.is_empty());
        drawn += 1;
    }
    assert!(drawn > 0, "rien a peindre");
}

/// The search as the page calls it: write the query at `els_query_ptr`, call
/// `els_search`, read `els_stride` words per hit at `els_hits_ptr`. It needs no
/// window, and a query that is not UTF-8 finds nothing.
#[test]
fn the_page_searches_through_the_query_buffer() {
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let query = "אָדָם";
    assert!(query.len() <= els_query_cap() as usize);
    unsafe { core::ptr::copy_nonoverlapping(query.as_ptr(), els_query_ptr(), query.len()) };
    let n = els_search(query.len() as u32, 0, 0) as usize;
    assert_eq!(els_hits_len() as usize, n);

    let mut expected = Vec::new();
    assert_eq!(els_engine().search("אדם", 0, 0, &mut expected) as usize, n);
    assert!(n > 0);
    let stride = els_stride() as usize;
    let read = unsafe { core::slice::from_raw_parts(els_hits_ptr(), n * stride) };
    assert_eq!(read, &expected[..], "the page does not read the records written");

    unsafe { *els_query_ptr() = 0xFF };
    assert_eq!(els_search(1, 0, 0), 0.0, "an invalid query finds something");
}

/// The landing exports, moved to `wasm/landing.rs`, as both modules call
/// them: room for a tile, a tile landed, the readings landed once the whole
/// text is in. Natively the text is in already: landing again changes
/// nothing, and says the text is whole.
#[test]
fn the_landing_exports_take_a_tile_as_the_page_writes_it() {
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let book = 0;
    let (first, end) = (book_first_chapter(book), book_end_chapter(book));
    let (from, to) = corpus::tile_range(first as usize..end as usize);
    let before = corpus::verse_text(0).to_string();
    let plain = include_bytes!(concat!(env!("OUT_DIR"), "/text.plain"));
    let room = crate::wasm::landing::tile_buffer((to - from) as u32);
    unsafe { core::ptr::copy_nonoverlapping(plain[from..to].as_ptr(), room, to - from) };
    assert_eq!(crate::wasm::landing::tile_load(first, end), 1);
    assert_eq!(corpus::verse_text(0), before, "a chapter already in was rewritten");
    assert_eq!(crate::wasm::landing::readings_load(), 1, "the whole text is not said complete");
}

/// The scoped search as the page would call it: the records of `search_in`
/// over the verses' letters, and nothing for an empty or outlying range.
#[test]
fn the_page_searches_a_stretch_of_verses() {
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let query = "משה";
    unsafe { core::ptr::copy_nonoverlapping(query.as_ptr(), els_query_ptr(), query.len()) };
    let (first, end) = (1533u32, 4000u32);
    let n = els_search_in(query.len() as u32, 0, 30, first, end) as usize;
    let mut expected = Vec::new();
    let letters = crate::shared::els::letters_of(Some((first as usize, end as usize)));
    assert_eq!(els_engine().search_in(query, 0, 30, letters, &mut expected) as usize, n);
    let stride = els_stride() as usize;
    let read = unsafe { core::slice::from_raw_parts(els_hits_ptr(), els_hits_len() as usize * stride) };
    assert_eq!(read, &expected[..]);
    assert!(read.chunks(stride).all(|r| (first..end).contains(&r[4]) && (first..end).contains(&r[6])));
    for (a, b) in [(9, 9), (9, 3), (0, u32::MAX)] {
        assert_eq!(els_search_in(query.len() as u32, 0, 30, a, b), 0.0, "{a}..{b}");
    }
}

/// What a panic leaves for the page: its place, a new line, its message —
/// read back through the two exports the page reads after a trap.
#[test]
fn a_panic_leaves_its_place_and_message_for_the_page() {
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    crate::wasm::fault::remember("src/els.rs:412:9", "index out of bounds");
    let (at, len) = (crate::wasm::fault::fault_ptr(), crate::wasm::fault::fault_len() as usize);
    let text = std::str::from_utf8(unsafe { core::slice::from_raw_parts(at, len) }).unwrap();
    assert_eq!(text, "src/els.rs:412:9\nindex out of bounds");
}

/// A chapter's record, read word by word as `book-view.ts` reads it: the
/// strings it points at are the chapter's number, its book's and portion's
/// names, and per verse its number and its text as shown — the other
/// edition's runs inside it. Every chapter; and nothing for one out of
/// range.
#[test]
fn the_book_view_reads_every_chapter_from_its_record() {
    use crate::shared::passage;
    use crate::wasm::chapter::*;
    let _hold = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let text = corpus::text();
    let at = |r: &[u32], i: usize| &text[r[i] as usize..(r[i] + r[i + 1]) as usize];
    assert_eq!(chapter_count() as usize, corpus::chapter_count());
    for c in 0..corpus::chapter_count() {
        let read = passage::chapter(c).unwrap();
        let words = chapter_record(c as u32) as usize;
        let r = unsafe { core::slice::from_raw_parts(chapter_record_ptr(), words) };
        assert_eq!((r[0] as usize, r[1] as usize), (read.book, read.section), "chapter {c}");
        assert_eq!(at(r, 2), corpus::chapter_label(c));
        assert_eq!(at(r, 4), corpus::book_name(read.book));
        assert_eq!(at(r, 6), corpus::section_name(read.section));
        assert_eq!(r[8] == 1, read.opens, "chapter {c}");
        assert_eq!(r[9] as usize, read.verses.len());
        let mut i = HEAD;
        for verse in &read.verses {
            assert_eq!(r[i] as usize, verse.place.verse, "chapter {c}");
            assert_eq!(at(r, i + 1), corpus::verse_label(verse.place.verse));
            assert_eq!(at(r, i + 3), verse.shown, "chapter {c}: a verse shown otherwise");
            let runs = r[i + 5] as usize;
            assert_eq!(runs, verse.others.len());
            for k in 0..runs {
                let (from, len) = (r[i + 6 + 2 * k] as usize, r[i + 7 + 2 * k] as usize);
                let base = corpus::offset_of(verse.shown);
                assert_eq!(from - base..from - base + len, verse.others[k], "chapter {c}: a run moved");
            }
            i += 6 + 2 * runs;
        }
        assert_eq!(i, words, "chapter {c}: the record runs past its verses");
        assert_eq!(chapter_landed(c as u32), 1);
        let size = chapter_size(c as u32) as usize;
        assert_eq!(size, corpus::chapter_bytes(c), "chapter {c}: the size is not the corpus' own");
        assert!(read.verses.iter().map(|v| v.text.len()).sum::<usize>() <= size, "chapter {c}: size");
        let (from, to) = corpus::tile_range(c..c + 1);
        assert_eq!((chapter_tile_from(c as u32, c as u32 + 1), chapter_tile_to(c as u32, c as u32 + 1)), (from as u32, to as u32));
    }
    let n = corpus::chapter_count() as u32;
    assert_eq!(chapter_record(n), 0);
    assert_eq!(chapter_record(u32::MAX), 0);
    assert_eq!((chapter_size(n), chapter_landed(n)), (0, 0));
    for (first, end) in [(5, 5), (6, 5), (0, n + 1), (u32::MAX, u32::MAX)] {
        assert_eq!((chapter_tile_from(first, end), chapter_tile_to(first, end)), (0, 0), "{first}..{end}");
    }
}

/// The hand-off as the page drives it: the map names where its centre is,
/// and `show` takes it back to a chapter and verse — a verse of -1 is the
/// chapter's top, a chapter out of range moves nothing.
#[test]
fn the_page_hands_the_place_back_and_forth() {
    let _hold = hold(REFERENCE.0, REFERENCE.1);
    converge();
    assert_eq!((here_chapter(), here_verse()), (0, -1), "the opening names a place");
    let psalm = corpus::chapters_of(26).start as u32 + 22;
    let verse = corpus::verses_of(psalm as usize).start as i32 + 3;
    show(psalm, verse);
    converge();
    show(psalm, verse);
    assert_eq!((here_chapter(), here_verse()), (psalm, verse));
    show(psalm, -1);
    assert_eq!(here_chapter(), psalm);
    let before = (cam_x(), cam_y(), cam_z());
    show(u32::MAX, 0);
    assert_eq!((cam_x(), cam_y(), cam_z()), before, "a chapter out of range moved the view");
}
