//! What the unpacked corpus promises: counts, a well-formed index, a hierarchy
//! that partitions, and clean text.
//!
//! Those figures are **not** re-estimated here: `CLAUDE.md` records them as
//! verified, and a build that changes them has changed the corpus.

use super::*;

const BOOKS: usize = 39;
const CHAPTERS: usize = 929;
const VERSES: usize = 23_200;

#[test]
fn counts_are_the_measured_ones() {
    assert_eq!(book_count(), BOOKS);
    assert_eq!(chapter_count(), CHAPTERS);
    assert_eq!(verse_count(), VERSES);
}

#[test]
fn index_follows_its_layout() {
    let expected = 3 + 4 + 2 * (BOOKS + 1) + 2 * (CHAPTERS + 1) + 2 * (VERSES + 1);
    assert_eq!(index().len(), expected, "la taille de l'index ne suit pas sa disposition");
}

/// Every series of offsets is monotonic and stays inside the text: that is
/// what makes `slice` safe without a single check at run time.
#[test]
fn every_series_of_offsets_is_monotonic_and_bounded() {
    let l = layout();
    for (name, series) in [
        ("section_name", l.section_name),
        ("book_name", l.book_name),
        ("chap_label", l.chap_label),
        ("verse_label", l.verse_label),
    ] {
        assert!(series.windows(2).all(|w| w[0] <= w[1]), "{name} n'est pas croissant");
        assert!(*series.last().unwrap() as usize <= text().len(), "{name} depasse le texte");
    }
    for (name, series) in [("book_chap", l.book_chap), ("chap_verse", l.chap_verse)] {
        assert!(series.windows(2).all(|w| w[0] <= w[1]), "{name} n'est pas croissant");
    }
    assert_eq!(l.book_chap[0], 0);
    assert_eq!(*l.book_chap.last().unwrap() as usize, CHAPTERS);
    assert_eq!(l.chap_verse[0], 0);
    assert_eq!(*l.chap_verse.last().unwrap() as usize, VERSES);
    // The verses, landed, lie end to end from where the map's strings stop.
    let mut at = verses_offset();
    for v in 0..VERSES {
        let verse = verse_text(v);
        assert_eq!(offset_of(verse), at, "le verset {v} ne suit pas le precedent");
        at += verse.len();
    }
}

/// Books partition the chapters, chapters partition the verses: no hole, no
/// overlap, canonical order kept.
#[test]
fn the_hierarchy_partitions_the_corpus() {
    let mut next = 0;
    for book in 0..book_count() {
        let range = chapters_of(book);
        assert_eq!(range.start, next, "livre {book} ne suit pas le precedent");
        assert!(!range.is_empty(), "livre {book} sans chapitre");
        next = range.end;
    }
    assert_eq!(next, CHAPTERS);

    let mut next = 0;
    for chapter in 0..chapter_count() {
        let range = verses_of(chapter);
        assert_eq!(range.start, next, "chapitre {chapter} ne suit pas le precedent");
        assert!(!range.is_empty(), "chapitre {chapter} sans verset");
        next = range.end;
    }
    assert_eq!(next, VERSES);
}

#[test]
fn the_canonical_order_is_the_one_displayed() {
    assert_eq!(book_name(0), "בראשית");
    assert_eq!(book_name(4), "דברים");
    assert_eq!(book_name(5), "יהושוע");
    assert_eq!(book_name(26), "תהילים");
    assert_eq!(book_name(38), "דברי הימים ב");

    let names: Vec<&str> = (0..book_count()).map(book_name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "deux livres portent le meme nom");
    assert!(names.iter().all(|n| !n.is_empty()));
}

/// The four books the source gives in one piece, cut at the traditional rank.
/// The totals must match exactly.
#[test]
fn the_split_books_are_cut_at_the_traditional_rank() {
    let count = |name: &str| {
        let book = (0..book_count()).find(|&b| book_name(b) == name).expect(name);
        chapters_of(book).len()
    };
    assert_eq!(count("שמואל א"), 31);
    assert_eq!(count("שמואל ב"), 24);
    assert_eq!(count("מלכים א"), 22);
    assert_eq!(count("מלכים ב"), 25);
    assert_eq!(count("דברי הימים א"), 29);
    assert_eq!(count("דברי הימים ב"), 36);
    assert_eq!(count("עזרא"), 10);
    assert_eq!(count("נחמיה"), 13);
}

#[test]
fn the_densest_book_is_the_hundred_and_fifty_psalms() {
    let densest = (0..book_count()).map(|b| chapters_of(b).len()).max().unwrap();
    assert_eq!(densest, 150);
    assert_eq!(chapters_of(26).len(), 150);
    assert_eq!(chapters_of(0).len(), 50);
    let obadiah = (0..book_count()).find(|&b| book_name(b) == "עובדיה").unwrap();
    assert_eq!(chapters_of(obadiah).len(), 1);
}

/// Every book restarts its chapters at `א`, and `15` / `16` never form a
/// divine name.
#[test]
fn chapter_labels_restart_at_one_and_avoid_the_divine_name() {
    for book in 0..book_count() {
        let first = chapters_of(book).start;
        assert_eq!(chapter_label(first), "א", "livre {book} ne commence pas a א");
    }
    for chapter in 0..chapter_count() {
        let label = chapter_label(chapter);
        assert!(!label.is_empty(), "chapitre {chapter} sans numero");
        assert!(label.chars().count() <= 3, "numero de plus de trois lettres : {label}");
        assert_ne!(label, "יה");
        assert_ne!(label, "יו");
    }
    let psalms = chapters_of(26);
    assert_eq!(chapter_label(psalms.start + 14), "טו");
    assert_eq!(chapter_label(psalms.start + 15), "טז");
    assert_eq!(chapter_label(psalms.start + 149), "קנ");

    // The second half of a split book is renumbered by `hebrew_numeral`, so it
    // is there that `15` and `16` are really at stake — the source never writes
    // them, having never restarted at א.
    for name in ["שמואל ב", "מלכים ב", "דברי הימים ב"] {
        let book = (0..book_count()).find(|&b| book_name(b) == name).expect(name);
        let first = chapters_of(book).start;
        assert_eq!(chapter_label(first + 14), "טו", "{name} chapitre 15");
        assert_eq!(chapter_label(first + 15), "טז", "{name} chapitre 16");
    }

    // And no two chapters of one book share a number.
    for book in 0..book_count() {
        let mut labels: Vec<&str> = chapters_of(book).map(chapter_label).collect();
        let count = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), count, "deux chapitres du livre {book} portent le meme numero");
    }
}

/// Cleaning, as `build.rs` promises it: no marker, no punctuation, words
/// separated by a single space, and the maqaf turned into a space rather than
/// dropped.
#[test]
fn verse_texts_are_clean() {
    for verse in 0..verse_count() {
        let text = verse_text(verse);
        assert!(!text.is_empty(), "verset {verse} vide");
        assert!(!text.starts_with(' ') && !text.ends_with(' '), "verset {verse} borde d'espace");
        assert!(!text.contains("  "), "verset {verse} a deux espaces de suite");
        for c in text.chars() {
            assert!(
                c == ' ' || ('\u{0590}'..='\u{05F4}').contains(&c),
                "verset {verse} garde {c:?}"
            );
            assert_ne!(c, '\u{05BE}', "le maqaf devait devenir une espace");
            assert_ne!(c, '{', "marqueur massoretique restant");
            assert_ne!(c, '}', "marqueur massoretique restant");
        }
    }
}

/// The 0xCA fix of `decode_cp1255`: without it, 414 signs are lost.
#[test]
fn the_holam_haser_of_mechon_mamre_survived() {
    // Counted over the verses, not over the whole block: the display strings
    // of `variant` repeat a few words at the end of it.
    let count = (0..verse_count())
        .map(verse_text)
        .flat_map(str::chars)
        .filter(|&c| c == '\u{05BA}')
        .count();
    assert_eq!(count, 414, "le correctif de l'octet 0xCA a bouge");
}

/// `offset_of` is the only thing that crosses the boundary with the page: a
/// fragment must be found back from its offset and its length alone.
#[test]
fn a_fragment_is_found_back_from_its_offset() {
    for verse in (0..verse_count()).step_by(997) {
        let fragment = verse_text(verse);
        let at = offset_of(fragment);
        assert_eq!(&text()[at..at + fragment.len()], fragment);
    }
    for book in 0..book_count() {
        let name = book_name(book);
        let at = offset_of(name);
        assert_eq!(&text()[at..at + name.len()], name);
    }
    for chapter in (0..chapter_count()).step_by(31) {
        let label = chapter_label(chapter);
        let at = offset_of(label);
        assert_eq!(&text()[at..at + label.len()], label);
    }
    for section in 0..3 {
        let name = section_name(section);
        let at = offset_of(name);
        assert_eq!(&text()[at..at + name.len()], name);
    }
}

/// `App::space_ratio` measures the width of a space on a real space taken from
/// the corpus: there must be one.
#[test]
fn the_corpus_holds_a_space() {
    assert!(text().contains(' '));
}

#[test]
fn section_names_are_the_three_portions() {
    assert_eq!(section_name(0), "תורה");
    assert_eq!(section_name(1), "נביאים");
    assert_eq!(section_name(2), "כתובים");
}

#[test]
fn verse_labels_number_from_one() {
    for chapter in (0..chapter_count()).step_by(37) {
        let first = verses_of(chapter).start;
        assert_eq!(verse_label(first), "א", "chapitre {chapter} ne commence pas au verset א");
    }
}

/// The text region order `build.rs` writes — section names, book names,
/// chapter labels, verse labels, verse texts — is what lets the page tell a
/// verse from a title by its offset alone.
#[test]
fn the_text_regions_follow_the_documented_order() {
    assert!(offset_of(section_name(0)) < offset_of(book_name(0)));
    assert!(offset_of(book_name(book_count() - 1)) < offset_of(chapter_label(0)));
    assert!(offset_of(chapter_label(chapter_count() - 1)) < offset_of(verse_label(0)));
    assert!(offset_of(verse_label(verse_count() - 1)) < offset_of(verse_text(0)));

    // Region 5 closes the block: the display strings of the verses carrying
    // another reading, after the verses themselves.
    let last = verse_text(verse_count() - 1);
    let shown: Vec<&str> = (0..verse_count()).filter_map(crate::variant::display).collect();
    assert!(!shown.is_empty(), "aucune chaine d'affichage");
    assert!(offset_of(last) + last.len() <= offset_of(shown[0]));
    let tail = shown[shown.len() - 1];
    assert_eq!(offset_of(tail) + tail.len(), text().len());
}

// ------------------------------------------------------------------ landing

/// A deterministic shuffle: the same order on every run.
fn shuffled(n: usize, mut seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        order.swap(i, (seed % (i as u64 + 1)) as usize);
    }
    order
}

/// **Chapters land in any order, each straight into its place**: a landed
/// chapter reads exactly as the whole corpus has it, one not landed reads as
/// empty, the text stays valid UTF-8 throughout — and the whole text landing
/// last, over chapters already in, gives the corpus byte for byte, readings
/// included.
#[test]
fn chapters_land_in_any_order_then_the_whole_text() {
    const MAP_PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/map.pack"));
    const PLAIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.plain"));
    let landing = Landing::new(MAP_PACK);
    let order = shuffled(CHAPTERS, 2024);
    let (stream, _) = crate::pack::split_text(&landing.map, PLAIN);
    let base = landing.map.stream_base;

    // Runs of one to five chapters, as tiles come, in a shuffled order.
    let mut landed = vec![false; CHAPTERS];
    for (k, &first) in order.iter().take(300).enumerate() {
        let end = (first + 1 + k % 5).min(CHAPTERS);
        let (from, to) = landing.tile_range(first..end);
        landing.land_chapters(first..end, &stream[from - base..to - base]);
        landed[first..end].fill(true);
    }
    assert!(std::str::from_utf8(landing.text().as_bytes()).is_ok(), "le texte n'est plus de l'UTF-8");
    for (c, &in_) in landed.iter().enumerate() {
        assert_eq!(landing.chapter_ready(c), in_, "chapitre {c}");
        for v in verses_of(c) {
            let want = if in_ { verse_text(v) } else { "" };
            assert_eq!(landing.verse_text(v), want, "verset {v} du chapitre {c}");
        }
    }
    assert!(!landing.has_text());

    landing.land_text(PLAIN);
    assert!(landing.has_text());
    assert!(landing.text() == text(), "le texte entier differe apres les tuiles");
    assert_eq!(landing.readings.get().unwrap()[..], variant_table()[..], "les lectures different");
    for v in (0..VERSES).step_by(97) {
        assert_eq!(landing.verse_text(v), verse_text(v));
    }
}

/// A tile asks for exactly its chapters' stretch of the opened text part:
/// consecutive runs are contiguous, and the whole run covers the stream.
#[test]
fn a_tile_range_is_its_chapters_stretch() {
    let landing = landing();
    let (start, _) = landing.tile_range(0..1);
    assert_eq!(start, landing.map.stream_base);
    for c in 0..CHAPTERS - 1 {
        assert_eq!(landing.tile_range(c..c + 1).1, landing.tile_range(c + 1..c + 2).0, "chapitre {c}");
    }
    let (from, to) = landing.tile_range(0..CHAPTERS);
    let total: usize = landing.map.chapters.iter().map(|s| s.stream.1).sum();
    assert_eq!(to - from, total);
}
