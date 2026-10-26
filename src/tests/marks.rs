//! A search's marks: its letters inked in the text at the reading level, a
//! dot beside the names of the chapters, books and portions holding them.
//!
//! Three promises. **The letters inked are the letters found**, read off the
//! search's own records, across verse breaks and the other edition's
//! insertions. **A dot says where, and only where**: the chapter, book or
//! portion holds a letter found. And **nothing covers anything**: a dot sits
//! beside its name, in the room its box leaves.

use super::*;
use crate::els::{self, Engine};

/// The hits of `word` at `skip`, as `els_search` leaves them for the page.
fn records(word: &str, skip: u32) -> Vec<u32> {
    let mut out = Vec::new();
    Engine::new().search(word, skip, skip, &mut out);
    out
}

/// Every letter a slot inks, as the consonants of our text, verse by verse.
fn inked(app: &App, slot: usize) -> Vec<(usize, String)> {
    app.marks[slot]
        .runs
        .iter()
        .map(|&(verse, from, to)| {
            let text = &drawn(verse as usize)[from as usize..to as usize];
            (verse as usize, text.chars().filter(|&c| letter::code(c).is_some()).collect())
        })
        .collect()
}

fn folded(word: &str) -> Vec<u8> {
    word.chars().filter_map(letter::code).collect()
}

/// A chapter holding the word's letters, and one holding none.
fn marked_and_bare(app: &App, slot: usize) -> (usize, usize) {
    let marked = app.marks[slot].chapters.iter().position(|&m| m).expect("no chapter marked");
    let bare = app.marks[slot].chapters.iter().position(|&m| !m).expect("every chapter marked");
    (marked, bare)
}

fn dots(commands: &[Command]) -> Vec<Command> {
    commands.iter().copied().filter(|c| c.kind == KIND_DOT).collect()
}

/// **Rank `k` among a verse's letters is the `k`-th consonant of our verse**,
/// in the string it is drawn from: every verse, the eighty-eight carrying
/// another reading included — their insertions are skipped whole. Each range
/// starts on a consonant and holds its marks.
#[test]
fn a_verse_s_letters_are_ours_in_order_with_their_marks() {
    for verse in 0..corpus::verse_count() {
        let ours: Vec<char> =
            corpus::verse_text(verse).chars().filter(|&c| letter::code(c).is_some()).collect();
        let shown = drawn(verse);
        let letters = letter_bytes(verse);
        assert_eq!(letters.len(), ours.len(), "verse {verse}: letter count");
        for (k, &(from, to)) in letters.iter().enumerate() {
            let piece = &shown[from as usize..to as usize];
            let mut chars = piece.chars();
            assert_eq!(chars.next(), Some(ours[k]), "verse {verse}: letter {k}");
            assert!(chars.all(|c| letter::code(c).is_none() && c != ' '), "verse {verse}: {piece:?}");
            let next = shown[to as usize..].chars().next();
            let mark = next.is_some_and(|c| ('\u{0591}'..='\u{05C7}').contains(&c));
            assert!(!mark, "verse {verse}: a mark left out of {piece:?}");
        }
    }
}

/// **Every hit is inked, letter for letter**: at skip 0 the runs placed spell
/// the word as many times as it was found — its occurrences never overlap —
/// and at a skip each letter stands alone.
#[test]
fn the_letters_inked_are_the_letters_found() {
    for (word, skip) in [("תורה", 0), ("אלהים", 0), ("תורה", 49)] {
        let mut app = reference_app();
        let found = records(word, skip);
        let hits = found.len() / els::STRIDE;
        assert!(hits > 0, "{word} at {skip}: nothing found");
        let span = folded(word).len();
        app.set_marks(0, &found, span);

        let letters: Vec<u8> = inked(&app, 0).iter().flat_map(|(_, l)| folded(l)).collect();
        assert_eq!(letters.len(), hits * span, "{word} at {skip}: letters inked");
        if skip == 0 {
            let spelled: Vec<u8> = folded(word).repeat(hits);
            assert_eq!(letters, spelled, "{word}: the runs do not spell the hits");
        }
    }
}

/// **A word read across a verse break is inked on both sides**: its first
/// letters end one verse, its last ones open the next.
#[test]
fn a_hit_across_two_verses_is_inked_in_both() {
    let found = records("אב", 0);
    let across = found
        .chunks_exact(els::STRIDE)
        .find(|r| r[4] != r[6])
        .expect("no hit of אב across two verses");
    let mut app = reference_app();
    app.set_marks(0, across, 2);
    let verses: Vec<usize> = inked(&app, 0).iter().map(|(v, _)| *v).collect();
    assert_eq!(verses, vec![across[4] as usize, across[6] as usize]);
}

/// **A dot names the places of the letters, and only them**: a chapter is
/// marked if and only if a run lies in one of its verses, a book if one of its
/// chapters is, a portion if one of its books is.
#[test]
fn the_places_marked_are_the_places_of_the_letters() {
    let mut app = reference_app();
    app.set_marks(2, &records("משה", 0), 3);
    let verses: Vec<usize> = app.marks[2].runs.iter().map(|r| r.0 as usize).collect();
    for chapter in 0..corpus::chapter_count() {
        let holds = verses.iter().any(|v| corpus::verses_of(chapter).contains(v));
        assert_eq!(app.marks[2].chapters[chapter], holds, "chapter {chapter}");
        assert_eq!(app.chapter_marks(chapter) == 1 << 2, holds, "chapter {chapter}'s bits");
    }
    for book in 0..corpus::book_count() {
        let holds = corpus::chapters_of(book).any(|c| app.marks[2].chapters[c]);
        assert_eq!(app.book_marks(book) == 1 << 2, holds, "book {book}");
    }
    for (section, &(_, first, last)) in treemap::SECTIONS.iter().enumerate() {
        let holds = (first..last).any(|b| app.book_marks(b) != 0);
        assert_eq!(app.section_marks(section) == 1 << 2, holds, "section {section}");
    }
}

/// **Hidden, a search marks nothing and keeps its places**; shown again, they
/// come back; cleared, they are gone. A slot past the last is refused.
#[test]
fn a_search_hidden_shown_and_cleared() {
    let mut app = reference_app();
    let found = records("משה", 0);
    assert!(app.set_marks(1, &found, 3) > 0);
    let (marked, _) = marked_and_bare(&app, 1);
    assert_eq!(app.chapter_marks(marked), 1 << 1);

    app.show_marks(1, false);
    assert_eq!(app.chapter_marks(marked), 0, "hidden, still marked");
    let mut cuts = Vec::new();
    app.verse_marks(app.marks[1].runs[0].0 as usize, &mut cuts);
    assert!(cuts.is_empty(), "hidden, still inked");

    app.show_marks(1, true);
    assert_eq!(app.chapter_marks(marked), 1 << 1, "shown again, unmarked");

    app.clear_marks(1);
    assert!(app.marks[1].runs.is_empty() && app.chapter_marks(marked) == 0, "cleared, still marked");
    assert_eq!(app.set_marks(MARK_SLOTS, &found, 3), 0, "a slot past the last");
}

/// **At the reading level, the found letters wear their slot's ink**, cut
/// out as their own runs; the runs still tile the line from the right and
/// recompose it, and a line holding none stays one command.
#[test]
fn the_letters_found_are_their_own_runs_in_the_line() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_marks(3, &records("אלהים", 0), 5);
    let (chapter, _) = marked_and_bare(&app, 3);
    let lines = app.chapter_lines(chapter);
    let row = Rect { x: 100.0, y: 50.0, w: 900.0, h: 30.0 };
    let scale = MIN_TEXT * 2.0 / lines[0].size;

    let (mut marked, mut plain) = (0, 0);
    for line in lines {
        app.commands.clear();
        app.draw_line(line, row, scale, 1.0);
        let out = commands(&app);
        let joined: String = out.iter().map(|c| c.text()).collect();
        assert_eq!(joined, line.text, "the runs do not recompose the line");
        let mut right = row.x + row.w;
        for c in &out {
            assert_close(c.x, right, 1e-2, "a run does not touch the previous one");
            right = c.x - c.w;
        }
        let inked: Vec<&Command> = out.iter().filter(|c| c.ink == INK_MARK + 3.0).collect();
        for c in &inked {
            let letters = folded(c.text());
            assert!(!letters.is_empty(), "an inked run without a letter");
            assert!(folded("אלהים").windows(letters.len()).any(|w| w == letters), "{:?}", c.text());
        }
        if inked.is_empty() {
            if variant::display(line.verse).is_none() {
                assert_eq!(out.len(), 1, "a line without a mark is cut");
            }
            plain += 1;
        } else {
            marked += 1;
        }
    }
    assert!(marked > 0 && plain > 0, "chapter {chapter}: {marked} lines marked, {plain} not");
}

/// **Of two searches on the same letters, the first slot keeps them**: the
/// line is cut once, never into runs laid over each other.
#[test]
fn two_searches_on_one_letter_ink_it_once() {
    let mut app = reference_app();
    measure_all(&mut app);
    let found = records("משה", 0);
    app.set_marks(1, &found, 3);
    app.set_marks(4, &found, 3);
    let (chapter, _) = marked_and_bare(&app, 1);
    let row = Rect { x: 100.0, y: 50.0, w: 900.0, h: 30.0 };
    for line in app.chapter_lines(chapter) {
        app.commands.clear();
        app.draw_line(line, row, MIN_TEXT * 2.0 / line.size, 1.0);
        let out = commands(&app);
        let joined: String = out.iter().map(|c| c.text()).collect();
        assert_eq!(joined, line.text, "a letter drawn twice");
        assert!(out.iter().all(|c| c.ink != INK_MARK + 4.0), "the later slot took a letter");
    }
}

/// **Above the text, five dots by each name holding the word, none by the
/// others** — under a chapter's number, beside a book's name or a
/// portion's — each in its slot's ink; none without a search, and none ever
/// on a letter.
#[test]
fn a_dot_sits_beside_each_name_holding_the_word() {
    for (w, h) in WINDOWS {
        let mut bare = framed_app(w, h);
        assert!(dots(&frame(&mut bare)).is_empty(), "{w}x{h}: a dot without a search");

        let mut app = framed_app(w, h);
        for slot in 0..MARK_SLOTS {
            app.set_marks(slot, &records("משה", 0), 3);
        }
        let step = app.zoom_step();
        let levels = [
            ("the opening", app.fit_scale() / step),
            ("the framing", app.fit_scale()),
            ("the chapters", app.fit_scale() * step),
        ];
        let mut named = 0;
        for (level, z) in levels {
            let (cx, cy) = (app.width / 2.0, app.height / 2.0);
            let (wx, wy) = app.camera.to_world(cx, cy);
            app.camera = Camera { z, x: cx - wx * z, y: cy - wy * z };
            measure_all(&mut app);
            let all = frame(&mut app);
            let found = dots(&all);
            for dot in &found {
                assert!(dot.ink >= INK_MARK && dot.ink < INK_MARK + MARK_SLOTS as f32, "a dot's ink");
            }

            // Each name drawn, with whether its place holds the word.
            let cells: Vec<Rect> =
                (0..corpus::chapter_count()).map(|c| app.camera.project(app.layout.chapters[c])).collect();
            for text in all.iter().filter(|c| c.is_text()) {
                let name = text.text();
                let ink = app.width_of(name, text.size).min(text.w);
                let (left, top) = (text.x - ink, text.y - text.size);
                let chapter = (0..corpus::chapter_count()).find(|&c| {
                    let r = cells[c];
                    corpus::chapter_label(c) == name
                        && text.x > r.x
                        && text.x <= r.x + r.w + 0.01
                        && top >= r.y - 0.01
                        && text.y <= r.y + r.h + 0.01
                });
                let book = (0..corpus::book_count()).find(|&b| corpus::book_name(b) == name);
                let (holds, room) = if let Some(s) = (0..3).find(|&s| corpus::section_name(s) == name) {
                    (app.section_marks(s) != 0, None)
                } else if let Some(b) = book {
                    (app.book_marks(b) != 0, Some(app.camera.project(app.layout.books[b])))
                } else if let Some(c) = chapter {
                    (app.chapter_marks(c) != 0, Some(cells[c]))
                } else {
                    continue;
                };
                // Its dots: on its line or just under it, by its ink, within a
                // few of its sizes — and in its box, a book's or a cell's.
                let near = found
                    .iter()
                    .filter(|d| {
                        let by = d.x < text.x + text.size
                            && d.x > left - 3.0 * text.size
                            && d.y > top
                            && d.y < text.y + text.size * 0.6;
                        let inside = room.is_none_or(|r| d.x > r.x && d.x < r.x + r.w && d.y > r.y && d.y < r.y + r.h);
                        by && inside
                    })
                    .count();
                // A name so small its dots may fall under the visible floor
                // has them all or none.
                if text.size * DOT_RADIUS < DOT_MIN * 4.0 {
                    assert!(near == 0 || near == MARK_SLOTS, "{w}x{h} at {level}: {name:?} has {near} dots");
                } else {
                    let want = if holds { MARK_SLOTS } else { 0 };
                    assert_eq!(near, want, "{w}x{h} at {level}: {name:?}, marked {holds}");
                    named += usize::from(holds);
                }

                for dot in &found {
                    let apart = dot.x + dot.w <= left
                        || dot.x - dot.w >= text.x
                        || dot.y + dot.w <= top
                        || dot.y - dot.w >= text.y;
                    assert!(apart, "{w}x{h} at {level}: a dot on {name:?}");
                }
            }
        }
        assert!(named > 0, "{w}x{h}: no name dotted");
    }
}

/// **A chapter's dots stay in its cell**, all five of them: in a row under
/// its number, never across into a neighbour's.
#[test]
fn a_chapter_s_dots_stay_in_its_cell() {
    for (w, h) in WINDOWS {
        let mut app = framed_app(w, h);
        for slot in 0..MARK_SLOTS {
            app.set_marks(slot, &records("משה", 0), 3);
        }
        // The first notches in, the centre held: a laptop shows its numbers
        // at the first, a phone a few later.
        let (step, framing) = (app.zoom_step(), app.fit_scale());
        let mut seen = 0;
        for notch in 1..=4 {
            let (cx, cy) = (app.width / 2.0, app.height / 2.0);
            let (wx, wy) = app.camera.to_world(cx, cy);
            let z = framing * step.powi(notch);
            app.camera = Camera { z, x: cx - wx * z, y: cy - wy * z };
            measure_all(&mut app);
            let all = frame(&mut app);
            let cells: Vec<Rect> = (0..corpus::chapter_count())
                .map(|c| app.camera.project(app.layout.chapters[c]))
                .collect();
            // Every dot lies wholly inside a cell or wholly outside each: none
            // straddles a cell's edge into its neighbour's room.
            for dot in dots(&all) {
                let (l, r, t, b) = (dot.x - dot.w, dot.x + dot.w, dot.y - dot.w, dot.y + dot.w);
                for cell in &cells {
                    let e = 1e-3;
                    let inside =
                        l >= cell.x - e && r <= cell.x + cell.w + e && t >= cell.y - e && b <= cell.y + cell.h + e;
                    let outside =
                        r <= cell.x + e || l >= cell.x + cell.w - e || b <= cell.y + e || t >= cell.y + cell.h - e;
                    assert!(inside || outside, "{w}x{h} notch {notch}: a dot across a cell's edge");
                    seen += usize::from(inside);
                }
            }
        }
        assert!(seen > 0, "{w}x{h}: no chapter dot");
    }
}

/// **Once the text is read, its letters speak**: a cell drawing its verses
/// carries no dot, its found letters inked instead.
#[test]
fn a_cell_reading_its_text_has_no_dot_but_its_letters() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_marks(0, &records("משה", 0), 3);
    let verse = app.marks[0].runs[0].0 as usize;
    let chapter = (0..corpus::chapter_count()).find(|&c| corpus::verses_of(c).contains(&verse)).unwrap();
    app.show(chapter, Some(verse));
    measure_all(&mut app);
    let all = frame(&mut app);
    let cell = app.camera.project(app.layout.chapters[chapter]);
    assert!(
        all.iter().any(|c| c.is_text() && c.ink == INK_MARK),
        "chapter {chapter} read without its letters inked"
    );
    for dot in dots(&all) {
        let inside = dot.x >= cell.x && dot.x <= cell.x + cell.w && dot.y >= cell.y && dot.y <= cell.y + cell.h;
        assert!(!inside, "a dot in a cell drawing its text");
    }
}

/// **The passage carries the letters the map inks**, cut in our verse — the
/// one it shows — not in the string drawn with the other edition's forms:
/// on every verse a search marks, and on a verse holding another reading,
/// marked after its insertion so every offset has moved.
#[test]
fn the_passage_carries_the_letters_the_map_inks() {
    let mut app = reference_app();
    app.set_marks(1, &records("משה", 0), 3);
    // A carrying verse, its last two letters marked: past its insertions.
    let carrying = (0..corpus::verse_count()).find(|&v| variant::display(v).is_some()).unwrap();
    let count = letter_bytes(carrying).len() as u32;
    app.set_marks(3, &[0, 0, 0, 0, carrying as u32, count - 2, carrying as u32], 2);

    let mut verses: Vec<usize> = app.marks[1].runs.iter().map(|r| r.0 as usize).collect();
    verses.dedup();
    verses.push(carrying);
    let mut out = Vec::new();
    for verse in verses {
        let mut drawn_runs = Vec::new();
        app.verse_marks(verse, &mut drawn_runs);
        app.passage_marks(verse, &mut out);
        assert_eq!(out.len() / 3, drawn_runs.len(), "verse {verse}: a run lost");
        let ours = corpus::verse_text(verse);
        for (piece, &(from, to, ink)) in out.chunks(3).zip(&drawn_runs) {
            let cut = &ours[piece[0] as usize..piece[1] as usize];
            let shown = &drawn(verse)[from..to];
            assert_eq!(cut, shown, "verse {verse}: the passage cuts other letters");
            assert_eq!(piece[2] as f32 + INK_MARK, ink, "verse {verse}: the slot");
        }
    }

    app.show_marks(1, false);
    app.passage_marks(app.marks[1].runs[0].0 as usize, &mut out);
    assert!(out.is_empty(), "a hidden search marks the passage");
}

/// **The book view carries the letters the map inks**, as offsets in the
/// corpus of the very strings its record shows — so on every chapter a
/// search marks, the stretches it is handed are the runs the map draws;
/// nothing for a chapter not marked or hidden; and a place's bits are the
/// dots the map draws by its name.
#[test]
fn the_book_view_carries_the_letters_the_map_inks() {
    let mut app = reference_app();
    app.set_marks(2, &records("משה", 0), 3);
    let (marked, bare) = marked_and_bare(&app, 2);
    let mut out = Vec::new();
    for chapter in [marked, marked + 1, bare] {
        app.book_marks_of(chapter, &mut out);
        let mut want = Vec::new();
        for verse in corpus::verses_of(chapter) {
            let mut runs = Vec::new();
            app.verse_marks(verse, &mut runs);
            let base = corpus::offset_of(drawn(verse));
            want.extend(runs.iter().map(|&(a, b, _)| &corpus::text()[base + a..base + b]));
        }
        let got: Vec<&str> = out.chunks(3).map(|t| &corpus::text()[t[0] as usize..t[1] as usize]).collect();
        assert_eq!(got, want, "chapter {chapter}");
        assert!(out.chunks(3).all(|t| t[2] == 2), "chapter {chapter}: the slot");
        assert_eq!(got.is_empty(), !app.marks[2].chapters[chapter], "chapter {chapter}");
    }
    assert_eq!(app.place_marks(2, marked), 1 << 2);
    assert_eq!(app.place_marks(2, bare), 0);
    let book = book_of(marked);
    assert_eq!(app.place_marks(1, book), app.book_marks(book));
    assert_eq!(app.place_marks(0, 0), app.section_marks(0));
    assert_eq!(app.place_marks(0, 3), 0, "a portion out of range");
    assert_eq!(app.place_marks(7, 0), 0, "a level out of range");

    app.show_marks(2, false);
    app.book_marks_of(marked, &mut out);
    assert!(out.is_empty() && app.place_marks(2, marked) == 0, "a hidden search marks the book");
}
