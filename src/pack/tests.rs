//! The packed corpus gives back the working form byte for byte, and the rules
//! it rests on — numerals, label exceptions, varints — hold on their own.

use super::*;

/// The working form `build.rs` writes beside the packed one: the reference.
fn reference() -> Blobs {
    let words = |bytes: &[u8]| -> Vec<u32> {
        bytes.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect()
    };
    Blobs {
        text: include_bytes!(concat!(env!("OUT_DIR"), "/text.bin")).to_vec(),
        index: words(include_bytes!(concat!(env!("OUT_DIR"), "/index.bin"))),
        variants: words(include_bytes!(concat!(env!("OUT_DIR"), "/variants.bin"))),
    }
}

const MAP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/map.pack"));
const PLAIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.plain"));

/// **The packed corpus unpacks to the working form, byte for byte**: text,
/// index and readings. Every other module reads the unpacked form, so this
/// one equality is what lets them all go on unchanged.
#[test]
fn the_packed_corpus_unpacks_byte_for_byte() {
    let want = reference();
    let got = unpack_text(unpack_map(MAP), PLAIN);
    assert_eq!(got.index, want.index, "the unpacked index differs");
    assert_eq!(got.variants, want.variants, "the unpacked readings table differs");
    assert!(got.text == want.text, "the unpacked text differs");
}

/// The map alone already holds what the overview draws — every name and
/// label in place, the index complete but for the verse offsets — and where
/// every chapter will land.
#[test]
fn the_map_alone_holds_names_labels_and_counts() {
    let want = reference();
    let map = unpack_map(MAP);
    let nv = want.index[2] as usize;
    let verse_text = want.index.len() - (nv + 1);
    assert_eq!(map.index[..verse_text], want.index[..verse_text], "the map's index differs");
    let names_end = want.index[verse_text] as usize;
    assert_eq!(map.text, want.text[..names_end], "the map's names and numbers differ");
    assert_eq!(map.total, want.text.len(), "wrong announced size");
    // Each chapter's place, as a tile lands it: its verses' bytes in the
    // working text, end to end from the map's strings.
    let offsets = &want.index[verse_text..];
    for (c, span) in map.chapters.iter().enumerate() {
        let verses = map.verses_of(c);
        let (from, to) = (offsets[verses.start] as usize, offsets[verses.end] as usize);
        assert_eq!(span.text, (from, to - from), "place of chapter {c}");
    }
    assert_eq!(map.verses_end(), offsets[nv] as usize);
}

/// What packing is for: the map is a sliver, and the text part holds about
/// one byte per character, where UTF-8 spends two on a Hebrew one.
#[test]
fn the_packed_corpus_is_a_fraction_of_the_working_form() {
    let want = reference();
    let working = want.text.len() + want.index.len() * 4 + want.variants.len() * 4;
    let chars = std::str::from_utf8(&want.text).unwrap().chars().count();
    assert!(MAP.len() * 100 < working, "the map weighs {} bytes", MAP.len());
    assert!(PLAIN.len() < chars, "the text weighs {} bytes for {chars} characters", PLAIN.len());
}

#[test]
fn hebrew_numerals_follow_the_tradition() {
    for (n, want) in [(1, "א"), (10, "י"), (11, "יא"), (15, "טו"), (16, "טז"), (99, "צט"), (100, "ק"), (115, "קטו"), (150, "קנ"), (400, "ת"), (500, "תק")] {
        assert_eq!(hebrew_numeral(n), want, "{n}");
    }
}

/// Labels that follow the rule cost nothing; one that breaks it is written
/// whole, with the count it resumes from — and laying them gives the labels
/// back.
#[test]
fn label_exceptions_carry_only_what_breaks_the_rule() {
    let plain = vec![vec!["א", "ב", "ג"], vec!["א", "ב"]];
    assert!(label_exceptions(&plain).is_empty());

    let joined = vec![vec!["א", "ב"], vec!["א-ב", "ג", "ד"], vec!["א", "?", "ג"]];
    let exceptions = label_exceptions(&joined);
    assert_eq!(exceptions, vec![(2, "א-ב".to_string(), 3), (6, "?".to_string(), 3)]);

    let mut text = Vec::new();
    let mut offsets = Vec::new();
    lay_labels(joined.iter().map(|g| g.len()), &exceptions, &mut text, &mut offsets);
    offsets.push(text.len() as u32);
    let laid: Vec<&str> = offsets.windows(2).map(|w| std::str::from_utf8(&text[w[0] as usize..w[1] as usize]).unwrap()).collect();
    assert_eq!(laid, joined.concat());
}

#[test]
fn varints_round_trip() {
    let values = [0u32, 1, 127, 128, 255, 16_383, 16_384, 1 << 21, u32::MAX];
    let mut out = Vec::new();
    for &v in &values {
        put(&mut out, v);
    }
    let mut r = Reader::new(&out);
    for &v in &values {
        assert_eq!(r.get(), v);
    }
    assert_eq!(r.at, out.len());
}

