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
const TEXT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.pack"));
const PLAIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text.plain"));

/// **The packed corpus unpacks to the working form, byte for byte**: text,
/// index and readings. Every other module reads the unpacked form, so this
/// one equality is what lets them all go on unchanged.
#[test]
fn the_packed_corpus_unpacks_byte_for_byte() {
    let want = reference();
    let got = unpack_text(unpack_map(MAP), PLAIN);
    assert_eq!(got.index, want.index, "l'index deplie differe");
    assert_eq!(got.variants, want.variants, "la table des lectures depliee differe");
    assert!(got.text == want.text, "le texte deplie differe");
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
    assert_eq!(map.index[..verse_text], want.index[..verse_text], "l'index de la carte differe");
    let names_end = want.index[verse_text] as usize;
    assert_eq!(map.text, want.text[..names_end], "noms et numeros de la carte different");
    assert_eq!(map.total, want.text.len(), "taille annoncee fausse");
    // Each chapter's place, as a tile lands it: its verses' bytes in the
    // working text, end to end from the map's strings.
    let offsets = &want.index[verse_text..];
    for (c, span) in map.chapters.iter().enumerate() {
        let verses = map.verses_of(c);
        let (from, to) = (offsets[verses.start] as usize, offsets[verses.end] as usize);
        assert_eq!(span.text, (from, to - from), "place du chapitre {c}");
    }
    assert_eq!(map.verses_end(), offsets[nv] as usize);
}

/// **The text as it travels opens to the text the corpus lands**, byte for
/// byte: the codec's round trip on the real corpus, through the very
/// functions the page's worker and the build run.
#[test]
fn the_sealed_text_opens_to_the_plain_one() {
    assert!(open(TEXT) == PLAIN, "le texte scelle ne redonne pas le texte clair");
    assert!(seal(PLAIN) == TEXT, "sceller ne redonne pas le texte publie");
}

/// What packing is for: the map is a sliver; the plain text holds about one
/// byte per character, where UTF-8 spends two on a Hebrew one; and the codec
/// takes the text part to a small fraction of the working text.
#[test]
fn the_packed_corpus_is_a_fraction_of_the_working_form() {
    let want = reference();
    let working = want.text.len() + want.index.len() * 4 + want.variants.len() * 4;
    let chars = std::str::from_utf8(&want.text).unwrap().chars().count();
    assert!(MAP.len() * 100 < working, "la carte pese {} octets", MAP.len());
    assert!(PLAIN.len() < chars, "le texte clair pese {} octets pour {chars} caracteres", PLAIN.len());
    assert!(TEXT.len() * 8 < want.text.len(), "le texte scelle pese {} octets", TEXT.len());
}

// ------------------------------------------------------------------- codec

/// **The codec's efficiency, held**: the sealed text stays under its measured
/// size with a hair of room — a model change that loses compression turns
/// this red — and the codec keeps a fifth or more of the plain text's bytes
/// off the wire. Against gzip, `web/scripts/copy-wasm.mjs` checks it at
/// every build, where gzip is at hand.
#[test]
fn the_codec_keeps_its_measured_efficiency() {
    assert!(TEXT.len() <= 502_000, "le texte scelle pese {} octets : le codec a regresse", TEXT.len());
    assert!(TEXT.len() * 5 < PLAIN.len(), "{} octets scelles pour {} clairs", TEXT.len(), PLAIN.len());
}

/// Times the codec's decode of the whole text, at once and as it would
/// stream, in release: `cargo test --release bench_codec -- --ignored
/// --nocapture`. The budget is loose — what it catches is a decode that
/// would no longer hide behind a slow download.
#[test]
#[ignore]
fn bench_codec() {
    let mut runs = Vec::new();
    for _ in 0..5 {
        let start = std::time::Instant::now();
        assert!(open(TEXT) == PLAIN);
        runs.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let per_kb = runs[2] / (TEXT.len() as f64 / 1024.0);
    eprintln!("decodage : {:.0} ms pour {} Ko scelles ({per_kb:.3} ms/Ko)", runs[2], TEXT.len() / 1024);
    assert!(runs[2] < 1000.0, "le decodage prend {:.0} ms", runs[2]);
}

/// Deterministic pseudo-Hebrew: consonants each followed by 0..3 marks, words
/// of 2..8 letters from a small vocabulary — so the match and word models
/// see repeats — verses of 5..20 words.
fn synthetic(n_verses: usize) -> Vec<u8> {
    let mut s: u32 = 12345;
    let mut rnd = |m: u32| {
        s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (s >> 16) % m
    };
    let mut vocab: Vec<Vec<u8>> = Vec::new();
    for _ in 0..300 {
        let mut w = Vec::new();
        for _ in 0..2 + rnd(7) {
            w.push(16 + rnd(27) as u8);
            for _ in 0..rnd(4) {
                w.push(1 + rnd(15) as u8);
            }
        }
        vocab.push(w);
    }
    let mut out = Vec::new();
    for _ in 0..n_verses {
        let words = 5 + rnd(16);
        for i in 0..words {
            if i > 0 {
                out.push(codec::SPACE);
            }
            out.extend_from_slice(&vocab[rnd(300) as usize]);
        }
        out.push(codec::SEP);
    }
    out
}

fn round_trip(data: &[u8]) -> usize {
    let c = codec::encode(data);
    assert_eq!(codec::decoded_len(&c), data.len());
    assert_eq!(codec::decode(&c), data, "le codec ne redonne pas la donnee");
    c.len()
}

/// Empty, a lone separator, one symbol repeated, every symbol, cycles: the
/// codec gives each back.
#[test]
fn the_codec_round_trips_its_edge_cases() {
    round_trip(&[]);
    round_trip(&[codec::SEP]);
    round_trip(&[16; 1000]);
    let all: Vec<u8> = (0..codec::NSYM as u8).collect();
    round_trip(&all);
    round_trip(&all.repeat(50));
    let rev: Vec<u8> = (0..codec::NSYM as u8).rev().cycle().take(5000).collect();
    round_trip(&rev);
}

#[test]
fn the_codec_compresses_what_repeats() {
    let data = synthetic(3000);
    let size = round_trip(&data);
    assert!(size * 3 < data.len(), "{} -> {}", data.len(), size);
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

/// Feeds bytes in chunks of the given sizes, cycling, and runs as it goes.
fn stream_in_chunks(packed: &[u8], sizes: &[usize]) -> (Vec<u8>, usize) {
    let mut stream = codec::Stream::default();
    let (mut at, mut k, mut before_last) = (0, 0, 0);
    while at < packed.len() {
        let end = (at + sizes[k % sizes.len()].max(1)).min(packed.len());
        if end == packed.len() {
            before_last = stream.decoded();
        }
        stream.push(&packed[at..end]);
        stream.run(false);
        at = end;
        k += 1;
    }
    (stream.finish(), before_last)
}

/// **Decoding as the bytes arrive gives what decoding at once gives**, for
/// any cut — a byte at a time, odd sizes, the header split anywhere — and it
/// really decodes on the way: before the last chunk, nearly everything is
/// done. That overlap is what hides the decode behind a slow download.
#[test]
fn the_codec_decodes_as_the_bytes_arrive() {
    let data = synthetic(400);
    let packed = codec::encode(&data);
    for sizes in [&[1usize][..], &[7], &[64], &[1000], &[3, 500, 1, 9000, 70], &[packed.len()]] {
        let (got, before_last) = stream_in_chunks(&packed, sizes);
        assert_eq!(got, data, "decoupage {sizes:?}");
        if sizes.len() == 1 && sizes[0] < 100 {
            assert!(before_last * 10 > data.len() * 9, "decoupage {sizes:?} : {before_last} symboles avant le dernier morceau");
        }
    }
}

/// The text part opens as it arrives too — its head, the stream, the
/// readings after it, each cut anywhere — to the very plain text.
#[test]
fn the_text_part_opens_as_it_arrives() {
    let mut opening = Opening::default();
    let mut at = 0;
    let mut size = 1;
    while at < TEXT.len() {
        let end = (at + size).min(TEXT.len());
        opening.push(&TEXT[at..end]);
        at = end;
        size = size * 3 % 65_537 + 1;
    }
    assert!(opening.decoded() > 0, "rien de decode en chemin");
    assert!(opening.finish() == PLAIN, "le texte ouvert en chemin differe");
}
