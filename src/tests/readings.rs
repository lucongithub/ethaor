//! The other edition's reading, drawn inside our text.
//!
//! Two promises. **Nothing is painted over anything**: the other form takes its
//! place in the flow, because the verse is laid out from the display string
//! welded by the build. And **the ink is the only thing that tells them
//! apart**: a run marked `INK_OTHER` is theirs, everything else is ours, and
//! the page is told nothing more.

use super::*;

/// First verse carrying another reading, with its chapter.
fn sample() -> (usize, usize) {
    for chapter in 0..corpus::chapter_count() {
        for verse in corpus::verses_of(chapter) {
            if variant::display(verse).is_some() {
                return (chapter, verse);
            }
        }
    }
    panic!("aucun verset porteur");
}

/// Lines of a chapter, laid out on measured widths.
fn lines_of(app: &mut App, chapter: usize) -> Vec<Line> {
    measure_all(app);
    app.chapter_lines(chapter)
}

/// Draws one line alone, at a size that clears the legibility floor, and gives
/// back the commands.
fn draw_one(app: &mut App, line: Line, row: Rect) -> Vec<Command> {
    app.commands.clear();
    let scale = (MIN_TEXT * 2.0) / line.size;
    app.draw_line(line, row, scale, 1.0);
    commands(app)
}

#[test]
fn a_verse_with_another_reading_is_laid_out_from_the_display_string() {
    let (_, verse) = sample();
    assert_eq!(drawn(verse), variant::display(verse).unwrap(), "le verset porteur");
    // Every other verse is drawn from the corpus, untouched.
    let plain = (0..corpus::verse_count()).find(|&v| variant::display(v).is_none()).unwrap();
    assert_eq!(drawn(plain), corpus::verse_text(plain), "un verset ordinaire");
}

/// **The runs of a line follow each other from the right**, and together they
/// are the line. The text is set flush right, so the first run of the string is
/// the rightmost: each box ends where the next one begins.
#[test]
fn the_runs_of_a_line_tile_it_from_the_right() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let row = Rect { x: 100.0, y: 50.0, w: 900.0, h: 30.0 };

    let mut seen = 0usize;
    for line in lines.into_iter().filter(|l| l.verse == verse) {
        let out = draw_one(&mut app, line, row);
        assert!(!out.is_empty(), "une ligne sans commande");
        seen += 1;

        let joined: String = out.iter().map(|c| c.text()).collect();
        assert_eq!(joined, line.text, "les runs ne recomposent pas la ligne");

        // The first slot of a text record is the right edge the page lays the
        // fragment on, and the fourth is its width: a run begins where the
        // previous one ended.
        let mut right = row.x + row.w;
        for c in &out {
            assert_close(c.x, right, 1e-2, "un run ne touche pas le precedent");
            assert!(c.w > 0.0, "un run sans largeur");
            right = c.x - c.w;
        }
    }
    assert!(seen > 0, "le verset porteur n'a produit aucune ligne");
}

/// **The ink says theirs or ours, and nothing else does.** The two runs are
/// drawn at the same strength — the colour alone sets them apart, and which
/// colour is the page's business — so a run must never be found marked without
/// being the other reading, nor the other way round.
#[test]
fn only_the_other_reading_carries_the_other_ink() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let row = Rect { x: 0.0, y: 0.0, w: 900.0, h: 30.0 };
    let shown = variant::display(verse).unwrap();
    let base = corpus::offset_of(shown);

    let mut others = 0usize;
    for line in lines.into_iter().filter(|l| l.verse == verse) {
        for c in draw_one(&mut app, line, row) {
            assert!(c.ink == INK_OURS || c.ink == INK_OTHER, "encre inconnue : {}", c.ink);
            let at = c.offset as usize - base;
            let inside = variant::inserts_of(verse).any(|i| {
                let (from, len) = i.other_range();
                at >= from && at + c.len as usize <= from + len
            });
            if c.ink == INK_OTHER {
                others += 1;
                assert!(inside, "un run marque sans etre une autre lecture");
                assert_close(c.alpha, 1.0, 1e-3, "l'autre lecture a ete atenuee");
            } else {
                assert!(!inside, "une autre lecture laissee a l'encre du texte");
                assert_close(c.alpha, 1.0, 1e-3, "notre texte a ete atenue");
            }
        }
    }
    assert!(others > 0, "aucune autre lecture dessinee");
}

/// A verse without a variant is still one command: the cut costs nothing to
/// the twenty-three thousand others.
#[test]
fn a_verse_without_a_variant_stays_one_command() {
    let chapter = (0..corpus::chapter_count())
        .find(|&c| corpus::verses_of(c).all(|v| variant::display(v).is_none()))
        .expect("un chapitre sans variante");

    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let row = Rect { x: 0.0, y: 0.0, w: 900.0, h: 30.0 };
    for line in lines {
        let out = draw_one(&mut app, line, row);
        assert_eq!(out.len(), 1, "une ligne ordinaire tient en une commande");
        assert_eq!(out[0].ink, INK_OURS);
        assert_eq!(out[0].text(), line.text);
    }
}

/// The counter the page reads to show the index: zero when no other reading is
/// on screen, more as soon as one is drawn.
#[test]
fn the_counter_follows_what_is_drawn() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);

    app.variant_runs = 0;
    app.build_frame();
    assert_eq!(app.variant_runs, 0, "le cadrage ne montre aucune autre lecture");

    let row = Rect { x: 0.0, y: 0.0, w: 900.0, h: 30.0 };
    app.variant_runs = 0;
    for line in lines.into_iter().filter(|l| l.verse == verse) {
        let scale = (MIN_TEXT * 2.0) / line.size;
        app.draw_line(line, row, scale, 1.0);
    }
    assert!(app.variant_runs > 0, "le compteur ignore les autres lectures");
}

/// **The navbar keeps naming our text.** A word of ours is given back without
/// the mark that numbers it; a word of the other edition names nothing, since
/// the passage does not contain it.
#[test]
fn a_click_never_names_the_other_edition() {
    let (chapter, verse) = sample();
    let mut app = reference_app();
    let lines = lines_of(&mut app, chapter);
    let line = lines.into_iter().find(|l| l.verse == verse).expect("une ligne du verset");
    let mine = corpus::verse_text(verse);
    let (from, len) = key_of(mine);

    let words = app.word_cells(line.text, line.rect, line.size);
    assert!(!words.is_empty(), "aucun mot sur la ligne");

    let shown = variant::display(verse).unwrap();
    let base = corpus::offset_of(shown);
    for (_, word) in words {
        let (at, size) = key_of(word);
        match variant::corpus_span(verse, at as usize - base, size as usize) {
            // Ours: the span given back is inside the verse, and it is what the
            // corpus holds there.
            Some((start, kept)) => {
                assert!(start + kept <= mine.len(), "un mot deborde du verset");
                assert_eq!(
                    &mine[start..start + kept],
                    &corpus::text()[(from as usize + start)..(from as usize + start + kept)],
                    "le mot nomme n'est pas celui du corpus"
                );
                assert!(kept <= size as usize, "la marque n'a pas ete retiree");
            }
            // Theirs: nothing is named.
            None => {
                let at = at as usize - base;
                assert!(
                    variant::inserts_of(verse).any(|i| {
                        let (f, l) = i.range();
                        at >= f && at < f + l
                    }),
                    "un mot refuse sans etre une autre lecture"
                );
            }
        }
    }
    let _ = len;
}
