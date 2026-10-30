//! A stretch selected on the map (`map/excerpt.rs`): from the letter a drag
//! began on to the one it reached, read back as our text and platted where
//! it is drawn.
//!
//! Three promises. **A drag over a verse reads that verse**, ours, the other
//! edition's forms out, whichever way it went. **A selection is only made on
//! a text one reads.** And **the plate says what is selected**: under the
//! selected lines and nowhere else, before their text, as wide as it.

use super::*;

/// A chapter at its reading level, measured, `verse` at the centre.
fn reading(chapter: usize, verse: usize) -> App {
    let mut app = reference_app();
    measure_all(&mut app);
    app.show(chapter, Some(verse));
    measure_all(&mut app);
    assert!(app.readable(chapter), "chapter {chapter} not read once shown");
    app
}

fn chapter_of(verse: usize) -> usize {
    (0..corpus::chapter_count()).find(|&c| corpus::verses_of(c).contains(&verse)).unwrap()
}

/// The first line of `first` and the last of `last`, in the world.
fn ends(app: &mut App, chapter: usize, first: usize, last: usize) -> (Line, Line) {
    let lines = app.chapter_lines(chapter);
    let a = *lines.iter().find(|l| l.verse == first).unwrap();
    let b = *lines.iter().rev().find(|l| l.verse == last).unwrap();
    (a, b)
}

/// A point on the right edge of a line's ink, where the text begins.
fn start_of(line: Line) -> (f32, f32) {
    (line.rect.x + line.rect.w - line.size * 0.05, line.rect.y + line.rect.h / 2.0)
}

/// A point left of a line's ink, past where it ends.
fn end_of(line: Line) -> (f32, f32) {
    (line.rect.x, line.rect.y + line.rect.h / 2.0)
}

fn drag(app: &mut App, from: (f32, f32), to: (f32, f32)) {
    assert!(app.excerpt_start(from.0, from.1), "no letter where the drag began");
    app.excerpt_to(to.0, to.1);
}

/// **A drag from a verse's first letter to its last reads the verse**, our
/// text byte for byte — on a verse carrying the other edition's reading too,
/// whose forms are drawn in it and taken out of what is read.
#[test]
fn a_drag_over_a_verse_reads_that_verse_ours() {
    let carrying = (0..corpus::verse_count()).find(|&v| variant::display(v).is_some()).unwrap();
    for verse in [0, carrying] {
        let chapter = chapter_of(verse);
        let mut app = reading(chapter, verse);
        let (a, b) = ends(&mut app, chapter, verse, verse);
        drag(&mut app, start_of(a), end_of(b));
        assert_eq!(app.excerpt_text(), corpus::verse_text(verse), "verse {verse}");
    }
}

/// **The way of the drag does not matter**: backwards, the same text.
#[test]
fn a_drag_backwards_reads_the_same() {
    let mut app = reading(0, 0);
    let (a, b) = ends(&mut app, 0, 0, 2);
    drag(&mut app, start_of(a), end_of(b));
    let forwards = app.excerpt_text();
    drag(&mut app, end_of(b), start_of(a));
    assert_eq!(app.excerpt_text(), forwards);
}

/// **A selection runs across verses**, each read whole on its own line.
#[test]
fn a_selection_across_verses_reads_each_on_its_line() {
    let mut app = reading(0, 1);
    let (a, b) = ends(&mut app, 0, 0, 2);
    drag(&mut app, start_of(a), end_of(b));
    let expected: Vec<&str> = (0..=2).map(corpus::verse_text).collect();
    assert_eq!(app.excerpt_text(), expected.join("\n"));
}

/// **A drag inside a line reads whole letters**, a stretch of the verse
/// whose every letter keeps its marks: the text read is a piece of ours.
#[test]
fn a_drag_inside_a_line_reads_whole_letters() {
    let mut app = reading(0, 0);
    let (line, _) = ends(&mut app, 0, 0, 0);
    let y = line.rect.y + line.rect.h / 2.0;
    let right = line.rect.x + line.rect.w;
    let ink = app.width_of(line.text, line.size);
    drag(&mut app, (right - ink * 0.2, y), (right - ink * 0.7, y));
    let text = app.excerpt_text();
    assert!(!text.is_empty(), "nothing read");
    assert!(corpus::verse_text(0).contains(&text), "{text:?} is not a stretch of the verse");
    let first = text.chars().next().unwrap();
    assert!(letter::code(first).is_some(), "the stretch begins on a mark: {text:?}");
}

/// **A selection is only made on a text one reads**: at the framing a drag
/// begins on no letter and selects nothing; a press without a drag selects
/// nothing either.
#[test]
fn nothing_is_selected_where_no_text_is_read() {
    let mut app = reference_framed();
    let (x, y) = (app.width / 2.0, app.height / 2.0);
    let (wx, wy) = app.camera.to_world(x, y);
    assert!(!app.excerpt_start(wx, wy));
    assert_eq!(app.excerpt_text(), "");

    let mut app = reading(0, 0);
    let (line, _) = ends(&mut app, 0, 0, 0);
    let at = start_of(line);
    assert!(app.excerpt_start(at.0, at.1));
    assert!(app.excerpt_range().is_none(), "a press alone selected something");
    assert_eq!(app.excerpt_text(), "");
}

/// **A drag leaving the text keeps the last letter it reached**: a point on
/// no letter read moves nothing.
#[test]
fn a_drag_off_the_text_keeps_its_last_letter() {
    let mut app = reading(0, 0);
    let (a, b) = ends(&mut app, 0, 0, 1);
    drag(&mut app, start_of(a), end_of(b));
    let before = app.excerpt;
    let outside = app.camera.to_world(-1000.0, -1000.0);
    app.excerpt_to(outside.0, outside.1);
    assert_eq!(app.excerpt, before);
    app.excerpt_clear();
    assert!(app.excerpt.is_none());
}

/// **The plate says what is selected**: one under each line the selection
/// holds and none elsewhere, drawn before that line's text, in its row — as
/// wide as the line where the line is selected whole. No selection, no
/// plate: the frame is the one it was.
#[test]
fn the_plate_lies_under_the_selected_lines_alone() {
    let mut app = reading(0, 1);
    let bare = frame(&mut app);
    assert!(bare.iter().all(|c| c.kind != KIND_SELECT), "a plate with nothing selected");

    let (a, b) = ends(&mut app, 0, 1, 1);
    drag(&mut app, start_of(a), end_of(b));
    measure_all(&mut app);
    let all = frame(&mut app);
    let plates: Vec<(usize, Command)> =
        all.iter().copied().enumerate().filter(|(_, c)| c.kind == KIND_SELECT).collect();
    let selected = app.chapter_lines(0).into_iter().filter(|l| l.verse == 1).count();
    assert_eq!(plates.len(), selected, "one plate per line of the verse");

    for (at, plate) in plates {
        assert!(plate.alpha > 0.0 && plate.alpha < 1.0, "the plate is a share of the white");
        let text = all[at + 1..].iter().find(|c| c.is_text()).expect("a plate with no text after it");
        let verse = corpus::verse_text(1);
        let shown = drawn(1);
        assert!(
            shown.contains(text.text()) || verse.contains(text.text()),
            "the plate is not under the verse's own line"
        );
        assert_close(plate.y, text.y - text.h / 2.0 - text.size * 0.36, 0.5, "plate row");
        assert_close(plate.x + plate.w, text.x, 0.5, "plate's right edge");
        let ink = app.width_of(text.text(), text.size);
        assert_close(plate.w, ink, 0.5, "plate's width");
    }
    // The rest of the frame is the bare one: the plate adds, it never moves.
    let without: Vec<Command> = all.into_iter().filter(|c| c.kind != KIND_SELECT).collect();
    assert_eq!(without, bare);
}

/// **A plate stands under its own letters, not at the line's start**: a
/// stretch cut inside a line begins left of the line's right edge, past what
/// precedes it, and ends inside the line's ink.
#[test]
fn a_plate_inside_a_line_stands_under_its_letters() {
    let mut app = reading(0, 0);
    let (line, _) = ends(&mut app, 0, 0, 0);
    let y = line.rect.y + line.rect.h / 2.0;
    let right = line.rect.x + line.rect.w;
    let ink = app.width_of(line.text, line.size);
    drag(&mut app, (right - ink * 0.3, y), (right - ink * 0.6, y));
    measure_all(&mut app);
    let all = frame(&mut app);
    let at = all.iter().position(|c| c.kind == KIND_SELECT).expect("no plate");
    let plate = all[at];
    let text = all[at + 1..].iter().find(|c| c.is_text()).unwrap();
    let ink = app.width_of(text.text(), text.size);
    let edge = plate.x + plate.w;
    assert!(edge < text.x - ink * 0.2 && edge > text.x - ink * 0.4, "the plate does not start under the stretch");
    assert!(plate.x > text.x - ink * 0.7 && plate.x < text.x - ink * 0.5, "the plate does not end under the stretch");
}

/// Drags from the start of one ink to the end of another, on the screen —
/// as the page drags — and gives the text read.
fn drag_screen(app: &mut App, from: Rect, to: Rect) -> String {
    let (ax, ay) = app.camera.to_world(from.x + from.w - 0.5, from.y + from.h / 2.0);
    let (bx, by) = app.camera.to_world(to.x + 0.5, to.y + to.h / 2.0);
    drag(app, (ax, ay), (bx, by));
    app.excerpt_text()
}

/// **A portion's names are selected at the opening**, each whole, each on
/// its line: the three, from the first to the last.
#[test]
fn the_portions_names_are_selected_at_the_opening() {
    let mut app = reference_app();
    measure_all(&mut app);
    let camera = app.camera;
    let (first, last) = (app.section_ink(0, camera).unwrap(), app.section_ink(2, camera).unwrap());
    let text = drag_screen(&mut app, first, last);
    let names: Vec<&str> = (0..3).map(corpus::section_name).collect();
    assert_eq!(text, names.join("\n"));
}

/// **Books' names are selected at the framing, and nothing under them**: a
/// drag from the first book's name to the third's reads the three names,
/// never the chapters and verses the reading puts between them. Each lies
/// on a plate, before its title.
#[test]
fn books_names_are_selected_without_what_lies_under_them() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let camera = app.camera;
    let (first, last) = (app.book_ink(0, camera).unwrap(), app.book_ink(2, camera).unwrap());
    let text = drag_screen(&mut app, first, last);
    let names: Vec<&str> = (0..3).map(corpus::book_name).collect();
    assert_eq!(text, names.join("\n"));

    let all = frame(&mut app);
    let plates: Vec<usize> = (0..all.len()).filter(|&i| all[i].kind == KIND_SELECT).collect();
    assert_eq!(plates.len(), 3, "one plate per name selected");
    for at in plates {
        let next = all[at + 1..].iter().find(|c| c.is_text()).unwrap();
        assert!(names.contains(&next.text()), "a plate before {:?}, not a name selected", next.text());
    }
}

/// **Chapters' numbers are selected once they show**, and only them: a drag
/// from the first chapter's number to the third's reads the three numbers.
#[test]
fn chapters_numbers_are_selected_without_their_verses() {
    let mut app = reference_framed();
    let step = app.zoom_step();
    let centre = app.camera.project(app.layout.chapters[1]);
    app.zoom_at(centre.x + centre.w / 2.0, centre.y + centre.h / 2.0, step);
    measure_all(&mut app);
    assert!(app.chapters_ready(app.camera) && !app.readable(0), "not at the numbers' level");
    let ink = |app: &mut App, c: usize| {
        let world = app.numeral_box(c);
        app.camera.project(world)
    };
    let (first, last) = (ink(&mut app, 0), ink(&mut app, 2));
    let text = drag_screen(&mut app, first, last);
    let numbers: Vec<&str> = (0..3).map(corpus::chapter_label).collect();
    assert_eq!(text, numbers.join("\n"));
    let plates = frame(&mut app).iter().filter(|c| c.kind == KIND_SELECT).count();
    assert_eq!(plates, 3, "one plate per number selected");
}

/// **Once the text is read, its chapter's number is selected with it**: from
/// the number in the column's corner to the end of the first verse, the
/// number then the verse.
#[test]
fn a_chapter_s_number_is_selected_with_its_text() {
    let mut app = reading(0, 0);
    let head = app.chapter_mark_box(0).unwrap();
    let (_, line) = ends(&mut app, 0, 0, 0);
    let from = (head.x + head.w - head.w * 0.1, head.y + head.h / 2.0);
    drag(&mut app, from, end_of(line));
    assert_eq!(app.excerpt_text(), format!("{}\n{}", corpus::chapter_label(0), corpus::verse_text(0)));
    let plates = frame(&mut app).iter().filter(|c| c.kind == KIND_SELECT).count();
    let lines = app.chapter_lines(0).iter().filter(|l| l.verse == 0).count();
    assert_eq!(plates, 1 + lines, "the number's plate and one per line");
}

/// **A selection of numbers plates no verse**, even once the text under it
/// is read: from the first chapter's number to the second's, zoomed into the
/// first, only numbers lie on plates and the text reads numbers alone.
#[test]
fn a_selection_of_numbers_plates_no_verse() {
    let mut app = reading(0, 0);
    let (one, two) = (Unit::Chapter(0), Unit::Chapter(1));
    app.excerpt = Some((Spot { unit: one, at: 0 }, Spot { unit: two, at: two.text().len() }));
    assert_eq!(app.excerpt_text(), format!("{}\n{}", corpus::chapter_label(0), corpus::chapter_label(1)));
    let all = frame(&mut app);
    for (at, _) in all.iter().enumerate().filter(|(_, c)| c.kind == KIND_SELECT) {
        let next = all[at + 1..].iter().find(|c| c.is_text()).unwrap();
        assert!(
            (0..2).any(|c| corpus::chapter_label(c) == next.text()),
            "a plate under {:?}, not a number",
            next.text()
        );
    }
}

/// **A name is selected letter by letter, as any text**: a drag inside a
/// book's name reads a stretch of it, not the whole, and its plate lies
/// under those letters alone, inside the name's ink.
#[test]
fn a_name_is_selected_letter_by_letter() {
    let mut app = reference_framed();
    measure_all(&mut app);
    let book = (0..corpus::book_count()).max_by_key(|&b| letter::letter_count(corpus::book_name(b))).unwrap();
    let name = corpus::book_name(book);
    let ink = app.book_ink(book, app.camera).unwrap();
    let y = ink.y + ink.h / 2.0;
    let a = app.camera.to_world(ink.x + ink.w * 0.8, y);
    let b = app.camera.to_world(ink.x + ink.w * 0.3, y);
    drag(&mut app, a, b);
    let text = app.excerpt_text();
    assert!(!text.is_empty() && text != name, "{text:?} is not a part of {name:?}");
    assert!(name.contains(&text), "{text:?} is not a stretch of {name:?}");

    let plates: Vec<Command> = frame(&mut app).into_iter().filter(|c| c.kind == KIND_SELECT).collect();
    assert_eq!(plates.len(), 1, "one plate, under the name");
    let plate = plates[0];
    assert!(plate.x > ink.x + 0.5 && plate.x + plate.w < ink.x + ink.w - 0.5, "the plate is not inside the name");
}

/// **The letters a selection holds are named as a letters record names
/// them**, verse and rank, in reading order: a drag over three verses gives
/// every consonant of each — the very letters the map's marks would ink —
/// and a selection of names alone gives none.
#[test]
fn a_selection_s_letters_are_named_by_verse_and_rank() {
    let mut app = reading(0, 1);
    let (a, b) = ends(&mut app, 0, 0, 2);
    drag(&mut app, start_of(a), end_of(b));
    let mut out = Vec::new();
    app.excerpt_letters(&mut out);
    let expected: Vec<u32> = (0..=2u32)
        .flat_map(|v| (0..letter_bytes(v as usize).len() as u32).flat_map(move |r| [v, r]))
        .collect();
    assert_eq!(out, expected);

    // A stretch inside a line: its letters are the ones read.
    let (line, _) = ends(&mut app, 0, 0, 0);
    let y = line.rect.y + line.rect.h / 2.0;
    let right = line.rect.x + line.rect.w;
    let ink = app.width_of(line.text, line.size);
    drag(&mut app, (right - ink * 0.2, y), (right - ink * 0.6, y));
    app.excerpt_letters(&mut out);
    let read: usize = app.excerpt_text().chars().filter(|&c| letter::code(c).is_some()).count();
    assert_eq!(out.len() / 2, read, "as many letters as the text read holds");
    assert!(out.chunks(2).all(|p| p[0] == 0), "a letter outside the verse");
    assert!(out.chunks(2).zip(out.chunks(2).skip(1)).all(|(p, q)| q[1] == p[1] + 1), "ranks not in a row");

    let mut app = reference_framed();
    measure_all(&mut app);
    let camera = app.camera;
    let (first, last) = (app.book_ink(0, camera).unwrap(), app.book_ink(2, camera).unwrap());
    drag_screen(&mut app, first, last);
    app.excerpt_letters(&mut out);
    assert!(out.is_empty(), "names alone hold no verse's letter");
}
