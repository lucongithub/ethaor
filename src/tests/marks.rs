//! A search's marks: its letters inked in the text at the reading level, a
//! square beside the names of the chapters, books and portions holding them.
//!
//! Three promises. **The letters inked are the letters found**, read off the
//! search's own records, across verse breaks and the other edition's
//! insertions. **A square says where, and only where**: the chapter, book or
//! portion holds a letter found. And **nothing covers anything**: a square
//! sits beside its name, in the room its box leaves.

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

fn squares(commands: &[Command]) -> Vec<Command> {
    commands.iter().copied().filter(|c| c.kind == KIND_SQUARE).collect()
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

/// **A square names the places of the letters, and only them**: a chapter is
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

/// **At the reading level, the found letters wear their slot's ink** (its
/// bit), cut
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
        let inked: Vec<&Command> = out.iter().filter(|c| c.ink == INK_MARK + 8.0).collect();
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

/// **A letter several searches found carries every one of them**, as bits in
/// its ink: on every verse the first search marks, the runs come in the
/// order of the text, never overlap, neighbours of the same bits merged, and
/// each byte's bits are exactly the shown slots whose letters hold it — a
/// search identical to another, one finding part of its word (`שה` in
/// `משה`), one hidden. On the line, the runs recompose it once, never laid
/// over each other.
#[test]
fn a_letter_found_by_several_searches_carries_them_all() {
    let mut app = reference_app();
    measure_all(&mut app);
    let moshe = records("משה", 0);
    app.set_marks(0, &moshe, 3);
    app.set_marks(1, &records("שה", 0), 2);
    app.set_marks(3, &moshe, 3);
    app.set_marks(4, &moshe, 3);
    app.show_marks(4, false);

    let mut verses: Vec<usize> = app.marks[0].runs.iter().map(|r| r.0 as usize).collect();
    verses.dedup();
    let (mut all, mut mem) = (0, 0);
    for &verse in &verses {
        let mut runs = Vec::new();
        app.verse_marks(verse, &mut runs);
        for pair in runs.windows(2) {
            assert!(pair[0].1 <= pair[1].0, "verse {verse}: runs overlap or out of order");
            assert!(pair[0].1 < pair[1].0 || pair[0].2 != pair[1].2, "verse {verse}: same bits not merged");
        }
        let own: Vec<Vec<(usize, usize)>> = (0..MARK_SLOTS)
            .map(|s| {
                let runs = app.marks[s].runs.iter().filter(|r| r.0 as usize == verse);
                runs.map(|r| (r.1 as usize, r.2 as usize)).collect()
            })
            .collect();
        for byte in 0..drawn(verse).len() {
            let want = (0..MARK_SLOTS)
                .filter(|&s| app.marks[s].shown && own[s].iter().any(|&(a, b)| (a..b).contains(&byte)))
                .fold(0u32, |bits, s| bits | 1 << s);
            let got = runs.iter().find(|r| (r.0..r.1).contains(&byte)).map_or(0, |r| (r.2 - INK_MARK) as u32);
            assert_eq!(got, want, "verse {verse}, byte {byte}: the slots of the letter");
            all += usize::from(got == 0b01011);
            mem += usize::from(got == 0b01001);
        }
    }
    assert!(all > 0 && mem > 0, "no letter of the three searches ({all}), or of two ({mem})");

    let (chapter, _) = marked_and_bare(&app, 0);
    let row = Rect { x: 100.0, y: 50.0, w: 900.0, h: 30.0 };
    let mut shared = 0;
    for line in app.chapter_lines(chapter) {
        app.commands.clear();
        app.draw_line(line, row, MIN_TEXT * 2.0 / line.size, 1.0);
        let out = commands(&app);
        let joined: String = out.iter().map(|c| c.text()).collect();
        assert_eq!(joined, line.text, "a letter drawn twice, or lost");
        shared += out.iter().filter(|c| c.ink == INK_MARK + 11.0).count();
    }
    assert!(shared > 0, "chapter {chapter}: no run in the three searches' ink");
}

/// **Above the text, a square of each search by each name holding the word,
/// none by the others** — under a chapter's number, beside a book's name or
/// a portion's — each in its slot's ink, one bit, as wide as it is high;
/// none without a search, and none ever on a letter. Every slot is set: all
/// of them fit by every name.
#[test]
fn a_square_sits_beside_each_name_holding_the_word() {
    for (w, h) in WINDOWS {
        let mut bare = framed_app(w, h);
        assert!(squares(&frame(&mut bare)).is_empty(), "{w}x{h}: a square without a search");

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
            let found = squares(&all);
            for square in &found {
                let bits = (square.ink - INK_MARK) as u32;
                assert!(bits.is_power_of_two() && bits < 1 << MARK_SLOTS, "a square's ink is not one slot");
                let (sw, sh) = (square.w, square.h);
                assert!((sw - sh).abs() < 1e-3, "{w}x{h} at {level}: a mark {sw}x{sh} is not a square");
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
                // Its squares: on its line or just under it, by its ink, within a
                // few of its sizes — and in its box, a book's or a cell's.
                let near = found
                    .iter()
                    .filter(|b| {
                        let (x, y) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
                        let by = x < text.x + text.size
                            && x > left - 3.0 * text.size
                            && y > top
                            && y < text.y + text.size * 0.6;
                        let inside = room.is_none_or(|r| x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h);
                        by && inside
                    })
                    .count();
                // A name so small its squares may fall under the visible
                // floor has them all or none.
                if text.size * SQUARE < SQUARE_MIN * 4.0 {
                    assert!(near == 0 || near == MARK_SLOTS, "{w}x{h} at {level}: {name:?} has {near} squares");
                } else {
                    let want = if holds { MARK_SLOTS } else { 0 };
                    assert_eq!(near, want, "{w}x{h} at {level}: {name:?}, marked {holds}");
                    named += usize::from(holds);
                }

                for square in &found {
                    let apart = square.x + square.w <= left
                        || square.x >= text.x
                        || square.y + square.h <= top
                        || square.y >= text.y;
                    assert!(apart, "{w}x{h} at {level}: a square on {name:?}");
                }
            }
        }
        assert!(named > 0, "{w}x{h}: no name marked");
    }
}

/// **Each search shown adds its square, and past a width they shrink to
/// fit**: the row grows square by square at the nominal side, never wider
/// than `SQUARE_ROW` sizes of its name, its squares only ever smaller as it
/// fills — so many fit where one does.
#[test]
fn a_row_of_squares_grows_with_each_search_then_shrinks_to_fit() {
    let size = 40.0;
    let (one, _) = square_sides(1, size);
    let (mut last_side, mut last_row) = (f32::INFINITY, 0.0);
    // Past the slots too: the rule is the row's, not the slot count's.
    for n in 1..32 {
        let marks = (1u32 << n) - 1;
        let (side, space) = square_sides(marks, size);
        let row = n as f32 * side + (n as f32 - 1.0) * space;
        assert!(row <= SQUARE_ROW * size + 1e-3, "{n} squares wider than the row allows");
        assert!(row >= last_row - 1e-3, "{n} squares narrower than {}", n - 1);
        assert!(side <= last_side + 1e-6, "a square grew as the row filled");
        assert!(side > 0.0 && space >= 0.0, "{n} squares of no side");
        (last_side, last_row) = (side, row);
    }
    assert_eq!(square_sides(0b11, size).0, one, "two squares already shrunk");
    assert!(last_side < one, "a full row kept the nominal side");
}

/// **A chapter's squares stay in its cell**, every slot's: in a row
/// under its number, never across into a neighbour's.
#[test]
fn a_chapter_s_squares_stay_in_its_cell() {
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
            // Every square lies wholly inside a cell or wholly outside each:
            // none straddles a cell's edge into its neighbour's room.
            for square in squares(&all) {
                let (l, r, t, b) = (square.x, square.x + square.w, square.y, square.y + square.h);
                for cell in &cells {
                    let e = 1e-3;
                    let inside =
                        l >= cell.x - e && r <= cell.x + cell.w + e && t >= cell.y - e && b <= cell.y + cell.h + e;
                    let outside =
                        r <= cell.x + e || l >= cell.x + cell.w - e || b <= cell.y + e || t >= cell.y + cell.h - e;
                    assert!(inside || outside, "{w}x{h} notch {notch}: a square across a cell's edge");
                    seen += usize::from(inside);
                }
            }
        }
        assert!(seen > 0, "{w}x{h}: no chapter square");
    }
}

/// **Once the text is read, its letters speak**: a cell drawing its verses
/// carries no square, its found letters inked instead.
#[test]
fn a_cell_reading_its_text_has_no_square_but_its_letters() {
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
        all.iter().any(|c| c.is_text() && c.ink == INK_MARK + 1.0),
        "chapter {chapter} read without its letters inked"
    );
    for square in squares(&all) {
        let inside =
            square.x >= cell.x && square.x <= cell.x + cell.w && square.y >= cell.y && square.y <= cell.y + cell.h;
        assert!(!inside, "a square in a cell drawing its text");
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
            assert_eq!(piece[2] as f32 + INK_MARK, ink, "verse {verse}: the slots");
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
/// squares the map draws by its name.
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
        assert!(out.chunks(3).all(|t| t[2] == 1 << 2), "chapter {chapter}: the slot's bit");
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

/// One verse of a page of passages, read back as the page reads it.
struct Row {
    book: usize,
    chapter: usize,
    verse: usize,
    label: &'static str,
    text: &'static str,
    runs: Vec<[u32; 3]>,
}

/// A page of passages, read back: each passage its verses.
fn passages(app: &App, from: usize, count: usize) -> Vec<Vec<Row>> {
    let mut out = Vec::new();
    let written = app.results_page(from, count, &mut out);
    let text = |at: u32, len: u32| &corpus::text()[at as usize..(at + len) as usize];
    let (mut at, mut page) = (0, Vec::new());
    while at < out.len() {
        let verses = out[at] as usize;
        at += 1;
        let mut rows = Vec::new();
        for _ in 0..verses {
            let head = &out[at..at + RESULT_VERSE_HEAD];
            at += RESULT_VERSE_HEAD;
            let n = head[7] as usize;
            let runs = out[at..at + 3 * n].chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
            at += 3 * n;
            rows.push(Row {
                book: head[0] as usize,
                chapter: head[1] as usize,
                verse: head[2] as usize,
                label: text(head[3], head[4]),
                text: text(head[5], head[6]),
                runs,
            });
        }
        page.push(rows);
    }
    assert_eq!(page.len(), written, "the passages written and those read back");
    page
}

/// **Every hit lies whole in exactly one passage, and passages never touch**:
/// in the order of the text, each from its first hit's first verse to the
/// last verse any of its hits reaches, the next starting past it — so hits
/// sharing a verse are one passage, and every verse of a hit is listed.
#[test]
fn every_hit_lies_whole_in_one_passage() {
    for (word, skip) in [("משה", 0), ("אב", 0), ("תורה", 49)] {
        let mut app = reference_app();
        let found = records(word, skip);
        let count = app.set_results(&found, folded(word).len());
        let list = app.results.passages.clone();
        assert_eq!(count, list.len());
        assert!(list.iter().all(|&(a, b)| a <= b), "{word}: a passage ends before it starts");
        assert!(list.windows(2).all(|w| w[1].0 > w[0].1), "{word}: two passages overlap or touch");
        for hit in found.chunks_exact(els::STRIDE) {
            let holding = list.iter().filter(|&&(a, b)| a <= hit[4] && hit[6] <= b).count();
            assert_eq!(holding, 1, "{word}: hit at {} in {holding} passages", hit[0]);
        }
        for &(a, b) in &list {
            let starts = found.chunks_exact(els::STRIDE).any(|h| h[4] == a);
            let reaches = found.chunks_exact(els::STRIDE).filter(|h| a <= h[4] && h[4] <= b).map(|h| h[6]).max();
            assert!(starts && reaches == Some(b), "{word}: passage {a}..={b} is not its hits' span");
        }
    }
}

/// **A hit across two verses is listed with both**: its passage reads the
/// verse where it starts and the one where it ends, and the search's own
/// letters are marked on each side.
#[test]
fn a_hit_across_two_verses_lists_both() {
    let found = records("אב", 0);
    let across = found.chunks_exact(els::STRIDE).find(|r| r[4] != r[6]).expect("no hit of אב across two verses");
    let mut app = reference_app();
    assert_eq!(app.set_results(across, 2), 1);
    let page = passages(&app, 0, 10);
    let verses: Vec<usize> = page[0].iter().map(|r| r.verse).collect();
    assert_eq!(verses, (across[4] as usize..=across[6] as usize).collect::<Vec<_>>());
    assert!(page[0].iter().all(|r| r.runs.iter().any(|t| t[2] & OWN != 0)), "a side without its letters");
}

/// **Each verse is named and read as the passage reads it**: its book holds
/// its chapter, its chapter holds it, its number is its label and its text
/// is ours — the other edition's forms not in it — and every run is a slice
/// of that text on a letter boundary.
#[test]
fn each_verse_of_a_passage_is_named_and_ours() {
    let mut app = reference_app();
    app.set_results(&records("ישראל", 0), 5);
    let page = passages(&app, 0, 400);
    assert_eq!(page.len(), 400);
    for row in page.iter().flatten() {
        assert!(corpus::chapters_of(row.book).contains(&row.chapter), "verse {}: its book", row.verse);
        assert!(corpus::verses_of(row.chapter).contains(&row.verse), "verse {}: its chapter", row.verse);
        assert_eq!(row.label, corpus::verse_label(row.verse));
        assert_eq!(row.text, corpus::verse_text(row.verse));
        for t in &row.runs {
            assert!(t[0] < t[1] && row.text.is_char_boundary(t[0] as usize) && row.text.is_char_boundary(t[1] as usize));
        }
    }
}

/// **The search's own letters always wear its colour**, shown on the map
/// or not: with no slot set, the runs under `OWN` spell the word once per
/// hit, across the whole list; another search shown adds its slot's bit
/// to the letters it found, and only there.
#[test]
fn a_search_s_own_letters_are_marked_shown_or_not() {
    let mut app = reference_app();
    let found = records("משה", 0);
    let hits = found.len() / els::STRIDE;
    let count = app.set_results(&found, 3);
    let spelled = |app: &App, bit: u32| -> Vec<u8> {
        passages(app, 0, count)
            .iter()
            .flatten()
            .flat_map(|r| {
                r.runs.iter().filter(|t| t[2] & bit != 0).flat_map(|t| folded(&r.text[t[0] as usize..t[1] as usize]))
            })
            .collect()
    };
    assert_eq!(spelled(&app, OWN), folded("משה").repeat(hits), "its own letters, hidden");

    app.set_marks(4, &records("ה", 0), 1);
    let page = passages(&app, 0, count);
    for row in page.iter().flatten() {
        let mut want = Vec::new();
        app.passage_marks(row.verse, &mut want);
        let theirs: Vec<[u32; 3]> = row.runs.iter().filter(|t| t[2] & (1 << 4) != 0).copied().collect();
        let letters = |runs: &[[u32; 3]]| -> Vec<u8> {
            runs.iter().flat_map(|t| folded(&row.text[t[0] as usize..t[1] as usize])).collect()
        };
        let shown: Vec<[u32; 3]> = want.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
        assert_eq!(letters(&theirs), letters(&shown), "verse {}: the shown search's letters", row.verse);
    }
    assert_eq!(spelled(&app, OWN), folded("משה").repeat(hits), "its own letters, beside another");
}

/// **Pages tile the list**: read a few at a time they give back the whole
/// list in order, past the end nothing; cleared, the list is gone.
#[test]
fn pages_of_passages_tile_the_list() {
    let mut app = reference_app();
    let count = app.set_results(&records("אהרן", 0), 4);
    assert!(count > 100);
    let whole: Vec<usize> = passages(&app, 0, count).iter().map(|p| p[0].verse).collect();
    let mut walked = Vec::new();
    for from in (0..count).step_by(37) {
        walked.extend(passages(&app, from, 37).iter().map(|p| p[0].verse));
    }
    assert_eq!(walked, whole);
    assert!(passages(&app, count, 10).is_empty(), "a page past the end");
    app.clear_results();
    assert!(app.results.passages.is_empty() && passages(&app, 0, 10).is_empty());
}

/// **A search's passages are counted as they are listed**: the count the
/// panel shows is the length of the list its count opens.
#[test]
fn a_search_s_passages_are_counted_as_listed() {
    for (word, skip) in [("משה", 0), ("אב", 0), ("תורה", 49)] {
        let mut app = reference_app();
        let found = records(word, skip);
        assert_eq!(passage_count(&found), app.set_results(&found, folded(word).len()), "{word} at {skip}");
    }
    assert_eq!(passage_count(&[]), 0);
}

/// **The searches shown make passages together**: those of any are the
/// passages of their hits taken as one search's, a hidden one left out;
/// those of every one are exactly the ones among them holding a hit of
/// each shown search. None while nothing is shown, and none of all once a
/// shown search is found nowhere.
#[test]
fn the_searches_shown_make_passages_together() {
    let mut app = reference_app();
    assert!(app.shown_passages(false).is_empty() && app.shown_passages(true).is_empty(), "nothing shown");
    let (moshe, aaron, hidden) = (records("משה", 0), records("אהרן", 0), records("ישראל", 0));
    app.set_marks(0, &moshe, 3);
    app.set_marks(3, &aaron, 4);
    app.set_marks(5, &hidden, 5);
    app.show_marks(5, false);

    let both: Vec<u32> = moshe.iter().chain(&aaron).copied().collect();
    let mut alone = reference_app();
    alone.set_results(&both, 3);
    let any = app.shown_passages(false);
    assert_eq!(any, alone.results.passages, "any: the hits of the shown searches as one");

    let holds = |hits: &[u32], (a, b): (u32, u32)| hits.chunks_exact(els::STRIDE).any(|h| a <= h[4] && h[6] <= b);
    let all = app.shown_passages(true);
    let want: Vec<(u32, u32)> = any.iter().copied().filter(|&p| holds(&moshe, p) && holds(&aaron, p)).collect();
    assert!(!want.is_empty(), "no passage holds both");
    assert_eq!(all, want, "all: the passages holding each shown search");
    assert!(all.len() < any.len());

    assert_eq!(app.set_shown_results(true), all.len());
    assert_eq!(app.results.passages, all);
    assert!(app.results.runs.is_empty(), "the shown searches' letters are their slots'");

    app.set_marks(7, &[], 3);
    assert!(app.shown_passages(true).is_empty(), "a search found nowhere, yet passages of all");
    assert_eq!(app.shown_passages(false), any, "a search found nowhere changed the passages of any");
}

/// **Joined by all, the map draws a search only where every one is**: each
/// shown search's letters stay in the passages of all and nowhere else, and
/// a chapter, book or portion holds its square exactly when one of those
/// letters lies there. Joined by any, each is drawn whole again, as it was;
/// a search alone joined by all is drawn whole, its passages being its own.
#[test]
fn joined_by_all_the_map_draws_only_where_every_search_is() {
    let mut app = reference_app();
    app.set_marks(0, &records("משה", 0), 3);
    app.set_marks(3, &records("אהרן", 0), 4);
    let any: Vec<Vec<(u32, u32, u32)>> = app.marks.iter().map(|m| m.runs.clone()).collect();
    let drawn = |app: &App, slot: usize| -> Vec<u32> {
        (0..corpus::verse_count())
            .filter(|&v| {
                let mut runs = Vec::new();
                app.verse_marks(v, &mut runs);
                runs.iter().any(|r| (r.2 - INK_MARK) as u32 & (1 << slot) != 0)
            })
            .map(|v| v as u32)
            .collect()
    };
    let verses = |runs: &[(u32, u32, u32)]| {
        let mut v: Vec<u32> = runs.iter().map(|r| r.0).collect();
        v.dedup();
        v
    };
    let whole: Vec<Vec<u32>> = [0, 3].iter().map(|&s| drawn(&app, s)).collect();
    let squares: Vec<u32> = (0..corpus::chapter_count()).map(|c| app.chapter_marks(c)).collect();

    app.join_marks(true);
    let passages = app.shown_passages(true);
    let inside = |v: u32| passages.iter().any(|&(a, b)| a <= v && v <= b);
    for slot in [0, 3] {
        let want: Vec<u32> = verses(&any[slot]).into_iter().filter(|&v| inside(v)).collect();
        assert!(!want.is_empty(), "slot {slot}: nothing where both are");
        assert_eq!(drawn(&app, slot), want, "slot {slot}: drawn outside the passages of all, or missing in them");
        assert!(want.len() < verses(&any[slot]).len(), "slot {slot}: nothing left out");
        for chapter in 0..corpus::chapter_count() {
            let holds = want.iter().any(|&v| corpus::verses_of(chapter).contains(&(v as usize)));
            assert_eq!(app.chapter_marks(chapter) & (1 << slot) != 0, holds, "slot {slot}, chapter {chapter}");
        }
        for book in 0..corpus::book_count() {
            let holds = corpus::chapters_of(book).any(|c| app.chapter_marks(c) & (1 << slot) != 0);
            assert_eq!(app.book_marks(book) & (1 << slot) != 0, holds, "slot {slot}, book {book}");
        }
        for (section, &(_, first, last)) in treemap::SECTIONS.iter().enumerate() {
            let holds = (first..last).any(|b| app.book_marks(b) & (1 << slot) != 0);
            assert_eq!(app.section_marks(section) & (1 << slot) != 0, holds, "slot {slot}, portion {section}");
        }
    }

    // One search left shown: its passages of all are its own.
    app.show_marks(3, false);
    assert_eq!(drawn(&app, 0), whole[0], "a search alone, joined by all, not drawn whole");
    app.show_marks(3, true);
    assert!(drawn(&app, 0).len() < whole[0].len(), "shown again, the join not kept");

    app.join_marks(false);
    assert_eq!([drawn(&app, 0), drawn(&app, 3)], [whole[0].clone(), whole[1].clone()], "joined by any, not whole");
    let back: Vec<u32> = (0..corpus::chapter_count()).map(|c| app.chapter_marks(c)).collect();
    assert_eq!(back, squares, "joined by any, the squares not as they were");
}
