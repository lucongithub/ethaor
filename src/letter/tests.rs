//! Splitting into letters: a consonant carries the marks that follow it, and
//! nothing is ever lost on the way.

use super::*;
use crate::corpus;

#[test]
fn an_empty_word_has_no_letter() {
    assert_eq!(letters("").count(), 0);
    assert_eq!(letter_count(""), 0);
}

/// A letter = one consonant and its marks. Every chunk must therefore start on
/// a consonant, and hold a single one.
#[test]
fn a_letter_is_a_consonant_and_its_marks() {
    let word = "בְּרֵאשִׁית";
    let parts: Vec<&str> = letters(word).collect();
    assert_eq!(parts.len(), letter_count(word));
    assert_eq!(parts.len(), 6);
    for part in &parts {
        assert!(is_base(part.chars().next().unwrap()), "{part:?} does not start with a consonant");
        assert_eq!(part.chars().filter(|&c| is_base(c)).count(), 1, "{part:?} has two consonants");
    }
    assert_eq!(parts.concat(), word, "the split loses marks");
}

/// Marks must stay stuck to their consonant: detaching them would break the
/// rendering.
#[test]
fn the_marks_stay_with_their_consonant() {
    let parts: Vec<&str> = letters("אָ֣דָם").collect();
    assert_eq!(parts.len(), 3);
    assert!(parts[0].chars().count() > 1, "the qamats was detached");
    assert!(parts.iter().all(|p| !p.is_empty()));
}

/// A string with no consonant at all — marks alone — is one single chunk: the
/// iterator never loops and never returns an empty slice.
#[test]
fn a_string_without_consonant_is_one_chunk() {
    let parts: Vec<&str> = letters("ְָ").collect();
    assert_eq!(parts, vec!["ְָ"]);
    assert_eq!(letter_count("ְָ"), 0);
}

#[test]
fn final_forms_count_as_consonants() {
    for c in ['ך', 'ם', 'ן', 'ף', 'ץ'] {
        assert!(is_base(c), "{c} should be a consonant");
    }
    for c in ['ְ', 'ָ', '֣', ' ', 'a'] {
        assert!(!is_base(c), "{c:?} is not a consonant");
    }
}

/// On the real corpus: the split is lossless and the count matches, word by
/// word. That is the invariant `letter_cells` leans on to place a letter.
#[test]
fn the_split_is_lossless_on_the_corpus() {
    let mut words = 0usize;
    for verse in (0..corpus::verse_count()).step_by(503) {
        for word in corpus::verse_text(verse).split(' ') {
            let parts: Vec<&str> = letters(word).collect();
            assert_eq!(parts.concat(), word, "word {word:?} lost in the split");
            assert_eq!(parts.len(), letter_count(word).max(1), "word {word:?} miscounted");
            assert!(parts.iter().all(|p| !p.is_empty()));
            words += 1;
        }
    }
    assert!(words > 100, "the sample is too thin: {words} words");
}

/// `Clone` is part of the contract: `letter_cells` walks the letters twice,
/// once for the sum of the widths and once to place the cells.
#[test]
fn the_iterator_can_be_walked_twice() {
    let word = "שָׁלוֹם";
    let first = letters(word);
    let second = first.clone();
    assert_eq!(first.count(), second.count());
}
