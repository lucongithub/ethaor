//! What the welded table must hold for the drawing to be honest: the display
//! string is our verse plus insertions, nothing else; every byte of it is
//! either ours or named as the other edition's.

use super::*;

/// Verses carrying another reading, and differences in all. Measured against
/// Miqra according to the Masorah, book by book: the numbering plays no part,
/// so these two figures move only if the table or the source does.
#[test]
fn the_table_holds_what_was_measured() {
    assert_eq!(verse_count(), 88, "versets porteurs");
    assert_eq!(count(), 94, "ecarts");
    assert!(count() >= verse_count(), "un verset porte au moins un ecart");
}

/// Both series are read by dichotomy, which only answers on a sorted table.
#[test]
fn the_series_are_sorted_by_verse() {
    let d = displays();
    for i in 1..verse_count() {
        assert!(d[i * 3] > d[(i - 1) * 3], "versets non croissants en {i}");
    }

    let v = inserts();
    for i in 1..count() {
        let (before, now) = ((v[(i - 1) * 4], v[(i - 1) * 4 + 1]), (v[i * 4], v[i * 4 + 1]));
        assert!(now >= before, "insertions non ordonnees en {i}");
    }
}

/// Every verse of the table is found again, and nothing else is.
#[test]
fn only_the_verses_of_the_table_are_drawn_otherwise() {
    let d = displays();
    let mut seen = 0usize;
    for verse in 0..corpus::verse_count() {
        match display(verse) {
            Some(_) => seen += 1,
            None => continue,
        }
    }
    assert_eq!(seen, verse_count(), "versets retrouves");
    for i in 0..verse_count() {
        assert!(display(d[i * 3] as usize).is_some(), "verset {} introuvable", d[i * 3]);
    }
}

/// **The display string is our verse, with insertions.** Taking the insertions
/// out gives the verse back byte for byte — that is what guarantees the map
/// shows our text and adds to it, rather than replacing it.
#[test]
fn removing_the_insertions_gives_the_verse_back() {
    for i in 0..verse_count() {
        let verse = displays()[i * 3] as usize;
        let shown = display(verse).unwrap();
        let mine = corpus::verse_text(verse);

        let mut rebuilt = String::with_capacity(mine.len());
        let mut cursor = 0usize;
        for insert in inserts_of(verse) {
            let (at, len) = insert.range();
            rebuilt.push_str(&shown[cursor..at]);
            cursor = at + len;
        }
        rebuilt.push_str(&shown[cursor..]);

        assert_eq!(rebuilt, mine, "verset {verse} reconstruit");
        assert!(shown.len() > mine.len(), "verset {verse} : rien n'a ete ajoute");
    }
}

/// Each range stays inside the string and on character boundaries: the page
/// slices the bytes, so a range cutting a letter in half would draw a hole.
#[test]
fn every_range_is_a_slice_of_the_display() {
    for i in 0..verse_count() {
        let verse = displays()[i * 3] as usize;
        let shown = display(verse).unwrap();

        for insert in inserts_of(verse) {
            let (at, len) = insert.range();
            assert!(at + len <= shown.len(), "verset {verse} : insertion hors du texte");
            assert!(shown.is_char_boundary(at), "verset {verse} : debut d'insertion sur une lettre");
            assert!(shown.is_char_boundary(at + len), "verset {verse} : fin d'insertion sur une lettre");

            let (from, other) = insert.other_range();
            assert!(other > 0, "verset {verse} : lecture autre vide");
            assert!(shown.is_char_boundary(from), "verset {verse} : autre lecture sur une lettre");
            assert!(insert.mark > 0, "verset {verse} : marque vide");
            // The other reading opens on the space that sets it apart from
            // ours, stands between brackets as both editions print an
            // alternative reading, and the mark closes it.
            let run = &shown[from..from + other];
            assert!(run.starts_with(" ["), "verset {verse} : l'autre lecture n'est pas ouverte");
            assert!(run.ends_with("]\u{00B2}"), "verset {verse} : l'autre lecture n'est pas fermee");
            assert!(run.len() > 5, "verset {verse} : l'autre lecture est vide");
        }
    }
}

/// The insertions of one verse do not overlap and come in reading order.
#[test]
fn the_insertions_of_a_verse_follow_each_other() {
    for i in 0..verse_count() {
        let verse = displays()[i * 3] as usize;
        let mut end = 0usize;
        for insert in inserts_of(verse) {
            let (at, len) = insert.range();
            assert!(at >= end, "verset {verse} : insertions qui se chevauchent");
            end = at + len;
        }
    }
}

/// **A byte of the display string is ours or theirs, never both.** `to_corpus`
/// answers for ours and refuses for theirs, which is what keeps a click from
/// naming a word the passage does not contain.
#[test]
fn an_offset_is_mapped_back_or_refused() {
    for i in 0..verse_count() {
        let verse = displays()[i * 3] as usize;
        let shown = display(verse).unwrap();
        let mine = corpus::verse_text(verse);

        let mut inside: Vec<(usize, usize)> = inserts_of(verse).map(|v| v.range()).collect();
        inside.sort();

        let mut expected = 0usize;
        for at in 0..shown.len() {
            let in_insert = inside.iter().any(|&(from, len)| at >= from && at < from + len);
            match to_corpus(verse, at) {
                None => assert!(in_insert, "verset {verse} : offset {at} refuse a tort"),
                Some(mapped) => {
                    assert!(!in_insert, "verset {verse} : offset {at} accepte a tort");
                    assert_eq!(mapped, expected, "verset {verse} : offset {at} mal replace");
                    assert!(mapped < mine.len(), "verset {verse} : offset hors du verset");
                    assert_eq!(
                        shown.as_bytes()[at],
                        mine.as_bytes()[mapped],
                        "verset {verse} : octet {at} different"
                    );
                    expected += 1;
                }
            }
        }
        assert_eq!(expected, mine.len(), "verset {verse} : octets replaces");
    }
}
