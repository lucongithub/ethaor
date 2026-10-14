//! Verses: measuring, wrapping, laying out, placing the letters.
//!
//! Two promises hold everything together: a line is always **a slice of the
//! corpus** — only an offset and a length cross the boundary with the page —
//! and **all the verses of a chapter share one character size**, a long verse
//! taking several lines rather than smaller letters.

use super::*;

/// A sample that covers the shapes of the corpus: a short chapter, a long one,
/// the densest, the first and the last.
const SAMPLE: [usize; 8] = [0, 1, 100, 500, 928, 460, 700, 850];

#[test]
fn the_width_of_a_fragment_is_deduced_from_its_words() {
    let mut app = reference_app();
    measure_all(&mut app);
    let space = app.space_ratio();
    let verse = corpus::verse_text(0);
    let words: Vec<&'static str> = verse.split(' ').collect();
    let sum: f32 = words.iter().map(|w| app.ratio(w)).sum();
    let expected = (sum + (words.len() - 1) as f32 * space) * 3.0;
    assert_close(app.width_of(verse, 3.0), expected, 1e-3, "largeur deduite des mots");
    // Linear in the size: that is what lets `lay_out` try a size and scale.
    let one = app.width_of(verse, 1.0);
    assert_close(app.width_of(verse, 7.0), one * 7.0, 1e-2, "la largeur n'est pas lineaire");
    assert_eq!(app.width_of(verse, 0.0), 0.0);
}

#[test]
fn a_space_is_measured_on_a_real_space_of_the_corpus() {
    let mut app = reference_app();
    let ratio = app.space_ratio();
    assert!(ratio > 0.0, "une espace sans largeur");
    let space = app.space.expect("l'espace retenue");
    assert_eq!(space, " ");
    assert_eq!(app.space_ratio(), ratio, "l'espace est remesuree a chaque appel");
}

/// An unmeasured fragment is answered with an estimate **and** recorded: the
/// next frame will be right.
#[test]
fn an_unknown_width_is_estimated_then_asked_for() {
    let mut app = reference_app();
    let word = corpus::verse_text(0).split(' ').next().unwrap();
    let estimated = app.ratio(word);
    assert_close(
        estimated,
        letter::letter_count(word).max(1) as f32 * GUESS,
        1e-6,
        "l'estimation n'est pas celle de GUESS",
    );
    let (offset, len) = key_of(word);
    assert!(
        app.pending.chunks(2).any(|p| p == [offset, len]),
        "le fragment n'a pas ete inscrit dans pending"
    );

    app.ratios.insert((offset, len), 1.5);
    let before = app.pending.len();
    assert_close(app.ratio(word), 1.5, 1e-6, "la mesure n'est pas reprise");
    assert_eq!(app.pending.len(), before, "un fragment mesure est redemande");
}

/// **Wrapping keeps slices of the corpus**: the lines of a verse, joined back
/// with a space, give the verse again, to the byte.
#[test]
fn wrapping_keeps_contiguous_slices_of_the_corpus() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    for chapter in SAMPLE {
        let cell = column(app.layout.chapters[chapter]);
        let size = app.chapter_lines(chapter)[0].size;
        for verse in corpus::verses_of(chapter) {
            let text = corpus::verse_text(verse);
            let parts = app.wrap(text, cell.w, size);
            assert!(!parts.is_empty(), "verset {verse} sans ligne");
            assert!(parts.iter().all(|p| !p.is_empty()), "verset {verse} a une ligne vide");
            assert_eq!(parts.join(" "), text, "verset {verse} ne se recompose pas");
            // Every slice really is a slice of the corpus, findable by its key.
            for part in &parts {
                let (offset, len) = key_of(part);
                assert_eq!(&corpus::text()[offset as usize..(offset + len) as usize], *part);
            }
        }
    }
}

/// A line never overflows its column, except when a single word is wider than
/// the column — there is nowhere left to cut.
#[test]
fn a_line_fits_in_its_column() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    for chapter in SAMPLE {
        let cell = column(app.layout.chapters[chapter]);
        let size = app.chapter_lines(chapter)[0].size;
        for verse in corpus::verses_of(chapter).take(12) {
            for part in app.wrap(corpus::verse_text(verse), cell.w, size) {
                let width = app.width_of(part, size);
                let single = !part.contains(' ');
                assert!(
                    width <= cell.w * 1.001 || single,
                    "la ligne {part:?} deborde sa colonne de {}",
                    width - cell.w
                );
            }
        }
    }
}

/// Degenerate cases: a zero width or size gives the verse back whole rather
/// than looping.
#[test]
fn wrapping_survives_a_column_without_width() {
    let mut app = reference_app();
    let verse = corpus::verse_text(0);
    assert_eq!(app.wrap(verse, 0.0, 1.0), vec![verse]);
    assert_eq!(app.wrap(verse, 10.0, 0.0), vec![verse]);
    assert_eq!(app.wrap(verse, f32::MAX, 1.0), vec![verse]);
}

/// **All the verses of a chapter share one size**, and the laid-out column fits
/// in its cell.
#[test]
fn a_chapter_lays_out_at_one_single_size() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    for chapter in SAMPLE {
        let cell = column(app.layout.chapters[chapter]);
        let lines = app.chapter_lines(chapter);
        assert!(!lines.is_empty(), "chapitre {chapter} sans ligne");

        let size = lines[0].size;
        assert!(size > 0.0);
        let width = cell.w - app.gutter(chapter, size);
        for line in &lines {
            assert_close(line.size, size, 1e-6, "deux tailles dans le chapitre");
            assert_close(line.rect.w, width, 1e-3, "une ligne n'a pas la largeur de sa colonne");
            assert_close(line.rect.x, cell.x, 1e-3, "une ligne n'est pas calee sur sa colonne");
        }

        // At least one line per verse, in order, stacked downwards.
        assert!(lines.len() >= corpus::verses_of(chapter).len(), "des versets ont disparu");
        for pair in lines.windows(2) {
            assert!(pair[1].rect.y >= pair[0].rect.y - 1e-4, "une ligne est remontee");
            assert!(pair[1].verse >= pair[0].verse, "les versets ne sont pas dans l'ordre");
        }
        assert_eq!(lines[0].verse, corpus::verses_of(chapter).start);
        assert_eq!(lines.last().unwrap().verse, corpus::verses_of(chapter).end - 1);

        // The column stays inside its cell.
        let bottom = lines.last().unwrap().rect.y + lines.last().unwrap().rect.h;
        assert!(bottom <= cell.y + cell.h + 0.01, "le chapitre {chapter} deborde sa case");
        assert!(lines[0].rect.y >= cell.y - 1e-3, "le chapitre {chapter} commence trop haut");

        // The marks are paid on the layout: a header row for the chapter
        // number above the first line, a gutter for the verse numbers at the
        // right of every line, wide enough for the widest of them.
        assert_close(lines[0].rect.y - cell.y, CHAPTER_MARK * LINE_K * size, 1e-3, "pas de rangee pour le numero");
        let widest = corpus::verses_of(chapter)
            .map(|v| app.width_of(corpus::verse_label(v), size * VERSE_MARK))
            .fold(0.0_f32, f32::max);
        assert!(cell.w - width > widest, "chapitre {chapter}: la gouttiere ne tient pas ses numeros");
    }
}

/// A breathing space separates the verses, so one sees where one ends and the
/// next begins.
#[test]
fn a_breathing_space_separates_two_verses() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    // Chapter 1 of Bereshit holds verses long enough to take two lines, so the
    // two kinds of gap are both there to compare.
    let lines = app.chapter_lines(1);
    let mut inside: Vec<f32> = Vec::new();
    let mut between: Vec<f32> = Vec::new();
    for pair in lines.windows(2) {
        let gap = pair[1].rect.y - (pair[0].rect.y + pair[0].rect.h);
        if pair[0].verse == pair[1].verse {
            inside.push(gap);
        } else {
            between.push(gap);
        }
    }
    assert!(!inside.is_empty() && !between.is_empty(), "l'echantillon manque un des deux ecarts");
    let widest_inside = inside.iter().copied().fold(f32::MIN, f32::max);
    let tightest_between = between.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        tightest_between > widest_inside,
        "les versets ne respirent pas plus que les lignes d'un meme verset"
    );
}

/// The layout of a chapter is computed **once**, then cached: it depends on the
/// world, never on the zoom.
#[test]
fn a_chapter_is_laid_out_once() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    let first = app.chapter_lines(3);
    assert!(app.lines.contains_key(&3), "la mise en page n'est pas mise en cache");
    let again = app.chapter_lines(3);
    assert_eq!(first.len(), again.len());
    for (a, b) in first.iter().zip(&again) {
        assert_eq!(a.rect, b.rect);
        assert_eq!(a.text, b.text);
        assert_eq!(a.size, b.size);
    }
}

/// The cells of the letters of a line, **right to left**: their widths add up
/// to the width of the line, and none leaves the line.
#[test]
fn the_letters_of_a_line_fill_it_right_to_left() {
    let mut app = reference_app();
    measure_all(&mut app);
    app.set_detail(true);
    for chapter in [0usize, 500] {
        let line = app.chapter_lines(chapter)[0];
        let cells = app.letter_cells(line.text, line.rect, line.size);
        let expected: usize =
            line.text.split(' ').map(|w| letter::letter_count(w).max(1)).sum();
        assert_eq!(cells.len(), expected, "une lettre a ete perdue");

        let drawn = app.width_of(line.text, line.size);
        let total: f32 = cells.iter().map(|c| c.w).sum();
        assert_close(total, drawn, 1e-2, "les cellules ne couvrent pas la ligne");

        for pair in cells.windows(2) {
            assert!(pair[1].x < pair[0].x, "les lettres ne vont pas de droite a gauche");
        }
        let right = line.rect.x + line.rect.w;
        assert_close(cells[0].x + cells[0].w, right, 1e-3, "la premiere lettre n'est pas au bord droit");
        for cell in &cells {
            assert_eq!(cell.y, line.rect.y, "une lettre a quitte sa ligne");
            assert_eq!(cell.h, line.rect.h);
            assert!(cell.w > 0.0, "une lettre sans largeur");
        }
    }
}

#[test]
fn an_empty_line_has_no_letter_cell() {
    let mut app = reference_app();
    let row = Rect { x: 0.0, y: 0.0, w: 10.0, h: 2.0 };
    // An empty slice **of the corpus**: every fragment that crosses the
    // boundary is one, since only an offset and a length travel.
    let empty = &corpus::text()[0..0];
    assert!(app.letter_cells(empty, row, 1.0).is_empty());
    assert!(app.letter_cells(corpus::verse_text(0), row, 0.0).is_empty());
}

/// The height a chapter would take at a given size grows with the size: that is
/// what lets `lay_out` converge in a few passes.
#[test]
fn the_column_height_grows_with_the_size() {
    let mut app = reference_app();
    measure_all(&mut app);
    let width = column(app.layout.chapters[0]).w;
    let mut previous = 0.0;
    for size in [0.05_f32, 0.1, 0.2, 0.4, 0.8] {
        let height = app.column_height(0, width, size);
        assert!(height > previous, "la hauteur n'a pas cru de {previous} a {height}");
        previous = height;
    }
}

/// The layout follows the cell: at the chapter level, where cells are a third
/// bigger, the verses read at a bigger size.
#[test]
fn bigger_cells_give_bigger_verses() {
    let mut app = reference_app();
    measure_all(&mut app);
    let overview = app.chapter_lines(0)[0].size;
    app.set_detail(true);
    let detail = app.chapter_lines(0)[0].size;
    assert!(detail > overview, "des cases plus grandes doivent grossir le texte");
}
