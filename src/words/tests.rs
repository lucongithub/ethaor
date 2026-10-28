//! First and last letters: each stream is the picked letter of every word
//! long enough, where it really lies; both searches find exactly what trying
//! every window finds, on random streams dense in hits; the engine scopes,
//! places and remembers what it found.

use std::sync::{Mutex, MutexGuard, OnceLock};

use super::*;

/// Deterministic xorshift: the same streams on every run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// The reference both searches are held to: every window read, its letters
/// compared as written, or sorted.
fn naive(stream: &[u8], pattern: &[u8], any_order: bool) -> Vec<u32> {
    let m = pattern.len();
    if m == 0 || m > stream.len() {
        return Vec::new();
    }
    let sorted = |w: &[u8]| {
        let mut w = w.to_vec();
        w.sort_unstable();
        w
    };
    (0..=stream.len() - m)
        .filter(|&i| {
            let w = &stream[i..i + m];
            if any_order { sorted(w) == sorted(pattern) } else { w == pattern }
        })
        .map(|i| i as u32)
        .collect()
}

/// The corpus engine, built once for the whole suite.
fn engine() -> MutexGuard<'static, Engine> {
    static ONE: OnceLock<Mutex<Engine>> = OnceLock::new();
    ONE.get_or_init(|| Mutex::new(Engine::default())).lock().unwrap_or_else(|e| e.into_inner())
}

/// The consonants of a word of the text.
fn consonants(word: &str) -> Vec<char> {
    word.chars().filter(|&c| letter::exact_code(c).is_some()).collect()
}

const WHOLE: Range<usize> = 0..usize::MAX;

/// The longest word `build.rs` welds is the longest the text holds: a stream
/// past it is empty, the one at it is not.
#[test]
fn the_longest_word_welded_is_the_texts() {
    let longest = (0..corpus::verse_count())
        .flat_map(|v| corpus::verse_text(v).split_whitespace())
        .map(letter::letter_count)
        .max()
        .unwrap();
    assert_eq!(LONGEST_WORD as usize, longest, "build.rs counts another longest word");
    for from_end in [false, true] {
        let mut engine = engine();
        assert!(!engine.stream(Pick { nth: LONGEST_WORD, from_end }).is_empty());
        assert!(Stream::of(Pick { nth: LONGEST_WORD + 1, from_end }).is_empty());
        assert!(Stream::of(Pick { nth: 0, from_end }).is_empty());
    }
}

/// Every stream is, word after word in reading order, the picked letter of
/// each word long enough to hold it — from its start or its end — and one
/// `BREAK` for each run of words too short; each letter names its verse and
/// its rank there: the very consonant the ELS stream holds at that rank,
/// folded and exact.
#[test]
fn a_stream_is_the_picked_letter_of_every_word_long_enough() {
    let (folded, exact, starts) = (els::stream(), els::exact_stream(), els::verse_starts());
    for pick in [Pick::FIRST, Pick::LAST, Pick { nth: 2, from_end: false }, Pick { nth: 3, from_end: true }] {
        let mut expected: Vec<Option<char>> = Vec::new();
        for v in 0..corpus::verse_count() {
            for word in corpus::verse_text(v).split_whitespace() {
                let cs = consonants(word);
                let n = pick.nth as usize;
                if n <= cs.len() {
                    expected.push(Some(cs[if pick.from_end { cs.len() - n } else { n - 1 }]));
                } else if !cs.is_empty() && expected.last() != Some(&None) {
                    expected.push(None);
                }
            }
        }
        let mut engine = engine();
        let s = engine.stream(pick);
        assert_eq!(s.len(), expected.len(), "{pick:?}: one letter per word long enough, one break per run of others");
        for (i, &c) in expected.iter().enumerate() {
            let Some(c) = c else {
                assert_eq!((s.folded[i], s.exact[i]), (BREAK, BREAK), "{pick:?} break {i}");
                continue;
            };
            assert_eq!(s.exact[i], letter::exact_code(c).unwrap(), "{pick:?} letter {i}");
            assert_eq!(s.folded[i], letter::code(c).unwrap(), "{pick:?} letter {i}");
            let rank = starts[s.verse[i] as usize] as usize + s.position[i] as usize;
            assert!(rank < starts[s.verse[i] as usize + 1] as usize, "{pick:?} letter {i} past its verse");
            assert_eq!((folded[rank], exact[rank]), (s.folded[i], s.exact[i]), "{pick:?} letter {i} misplaced");
        }
        assert!(s.verse.windows(2).all(|w| w[0] <= w[1]), "{pick:?} out of reading order");
    }
}

/// Every word holds a first letter, and a one-letter word's first is its
/// last: the first and last streams are the words, with no break.
#[test]
fn the_first_and_last_letters_break_nowhere() {
    let words = (0..corpus::verse_count())
        .flat_map(|v| corpus::verse_text(v).split_whitespace())
        .filter(|w| letter::letter_count(w) > 0)
        .count();
    let mut engine = engine();
    for pick in [Pick::FIRST, Pick::LAST] {
        let s = engine.stream(pick);
        assert_eq!(s.len(), words, "{pick:?}");
        assert!(!s.folded.contains(&BREAK), "{pick:?} broken");
    }
}

/// **A hit's words follow each other in the text** — the owner's rule: אדם
/// in the third letters is three adjacent words, the third letter of the
/// first an alef, of the next a dalet, of the next a mem. A word too short
/// between them breaks the run. The search finds exactly the runs of
/// adjacent words read that way, in order or in any, from the start or the
/// end of the words.
#[test]
fn a_hits_words_are_adjacent_in_the_text() {
    // Every word of the text, with its consonants, in reading order.
    let words: Vec<Vec<u8>> = (0..corpus::verse_count())
        .flat_map(|v| corpus::verse_text(v).split_whitespace())
        .map(|w| w.chars().filter_map(letter::code).collect::<Vec<u8>>())
        .filter(|w| !w.is_empty())
        .collect();
    let mut engine = engine();
    for (word, pick, any_order) in [
        ("אדם", Pick { nth: 3, from_end: false }, false),
        ("אדם", Pick { nth: 3, from_end: false }, true),
        ("יהו", Pick { nth: 2, from_end: false }, false),
        ("ומה", Pick { nth: 2, from_end: true }, true),
    ] {
        let pattern: Vec<u8> = word.chars().filter_map(letter::code).collect();
        let n = pick.nth as usize;
        let letter_of = |w: &Vec<u8>| (n <= w.len()).then(|| w[if pick.from_end { w.len() - n } else { n - 1 }]);
        let sorted = |mut v: Vec<u8>| {
            v.sort_unstable();
            v
        };
        let want = words
            .windows(pattern.len())
            .filter(|run| {
                let Some(got) = run.iter().map(letter_of).collect::<Option<Vec<u8>>>() else {
                    return false;
                };
                if any_order { sorted(got) == sorted(pattern.clone()) } else { got == pattern }
            })
            .count() as u64;
        let (total, _) = engine.hits_in(word, pick, any_order, false, WHOLE);
        assert!(want > 0, "{word} {pick:?}: the case holds nothing");
        assert_eq!(total, want, "{word} {pick:?} any_order={any_order}: not the runs of adjacent words");
    }
}

/// Both searches find exactly the windows the naive reading finds, on
/// random streams of two to four letters — where hits are dense and every
/// edge, first window and last, is met — for words of one to six letters.
#[test]
fn every_search_equals_reading_every_window() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for round in 0..400 {
        let alphabet = 2 + round % 3;
        let n = rng.below(60);
        // Breaks among the letters, as short words leave them.
        let stream: Vec<u8> =
            (0..n).map(|_| if rng.below(8) == 0 { BREAK } else { rng.below(alphabet) as u8 }).collect();
        let m = 1 + rng.below(6);
        let pattern: Vec<u8> = (0..m).map(|_| rng.below(alphabet) as u8).collect();
        for any_order in [false, true] {
            let mut got = Vec::new();
            let total = hits(&stream, &pattern, any_order, usize::MAX, &mut got);
            let want = naive(&stream, &pattern, any_order);
            assert_eq!(got, want, "{stream:?} {pattern:?} any_order={any_order}");
            assert_eq!(total, want.len() as u64);
        }
    }
}

/// Past the cap the first hits are kept in the order of the stream, and the
/// total is still exact.
#[test]
fn the_cap_keeps_the_first_hits_with_the_exact_total() {
    let stream = vec![3u8; 50];
    for any_order in [false, true] {
        let mut got = Vec::new();
        assert_eq!(hits(&stream, &[3, 3], any_order, 5, &mut got), 49);
        assert_eq!(got, [0, 1, 2, 3, 4]);
    }
}

/// No letter, a word longer than the stream, or a code out of the alphabet
/// finds nothing.
#[test]
fn an_empty_or_impossible_query_finds_nothing() {
    let mut got = vec![7];
    for any_order in [false, true] {
        assert_eq!(hits(&[1, 2, 3], &[], any_order, 10, &mut got), 0);
        assert_eq!(hits(&[1, 2], &[1, 2, 3], any_order, 10, &mut got), 0);
        assert_eq!(hits(&[1, 2], &[FORMS as u8], any_order, 10, &mut got), 0);
        assert!(got.is_empty());
    }
}

/// Genesis opens on בראשית ברא אלהים: its first letters spell בבא from the
/// stream's first index, its last letters תאם, and in any order אבב finds
/// it too.
#[test]
fn the_first_words_of_genesis_spell_their_first_and_last_letters() {
    let mut engine = engine();
    let (_, hits) = engine.hits_in("בבא", Pick::FIRST, false, false, WHOLE);
    assert_eq!(hits.first(), Some(&0));
    let (_, hits) = engine.hits_in("תאם", Pick::LAST, false, true, WHOLE);
    assert_eq!(hits.first(), Some(&0));
    let (_, hits) = engine.hits_in("אבב", Pick::FIRST, true, false, WHOLE);
    assert_eq!(hits.first(), Some(&0));
    let (_, hits) = engine.hits_in("אבב", Pick::FIRST, false, false, WHOLE);
    assert_ne!(hits.first(), Some(&0), "in order, אבב is not בבא");
}

/// Any order finds the word as written and more: its hits hold every hit in
/// order, over each stream.
#[test]
fn any_order_holds_the_word_as_written() {
    let mut engine = engine();
    for pick in [Pick::FIRST, Pick::LAST, Pick { nth: 2, from_end: false }] {
        for word in ["אמר", "ויה", "בית"] {
            let ordered = engine.hits_in(word, pick, false, false, WHOLE).1.to_vec();
            let (total, any) = engine.hits_in(word, pick, true, false, WHOLE);
            assert!(total as usize >= ordered.len());
            assert!(ordered.iter().all(|h| any.binary_search(h).is_ok()), "{word} {pick:?}");
        }
    }
}

/// Exact letters tell finals apart: a final mem ends words, so the last
/// letters find ם where the first letters never do, and folded both are מ.
#[test]
fn exact_letters_tell_the_finals_apart() {
    let mut engine = engine();
    assert_eq!(engine.hits_in("ם", Pick::FIRST, false, true, WHOLE).0, 0);
    let last = engine.hits_in("ם", Pick::LAST, false, true, WHOLE).0;
    assert!(last > 0);
    let folded = engine.hits_in("ם", Pick::LAST, false, false, WHOLE).0;
    let open = engine.hits_in("מ", Pick::LAST, false, true, WHOLE).0;
    assert_eq!(folded, last + open, "folded is the union of the exact spellings");
}

/// A search kept to verses finds exactly the hits of the whole whose every
/// letter lies there, and the whole stretch is the plain search.
#[test]
fn a_scoped_search_keeps_the_hits_lying_wholly_in_its_verses() {
    let mut engine = engine();
    for any_order in [false, true] {
        let pick = Pick { nth: 2, from_end: false };
        let all = engine.hits_in("אל", pick, any_order, false, WHOLE).1.to_vec();
        assert!(all.len() < CAP, "the reference is the whole list");
        let s = engine.stream(pick);
        let (verse, span) = (s.verse.clone(), 2);
        for verses in [0..50, 1000..1533, 20_000..corpus::verse_count()] {
            let want: Vec<u32> = all
                .iter()
                .copied()
                .filter(|&h| (h as usize..h as usize + span).all(|i| verses.contains(&(verse[i] as usize))))
                .collect();
            let (total, got) = engine.hits_in("אל", pick, any_order, false, verses.clone());
            assert_eq!(got, &want[..], "{verses:?} any_order={any_order}");
            assert_eq!(total, want.len() as u64);
        }
        let whole = engine.hits_in("אל", pick, any_order, false, 0..corpus::verse_count()).1.to_vec();
        assert_eq!(whole, all);
    }
}

/// Each record names every letter of its hit — verse, rank there — and the
/// consonant there is the word's letter, in order, or one of its letters
/// as many times in any order.
#[test]
fn every_record_places_each_letter_where_it_lies() {
    let starts = els::verse_starts();
    let stream = els::exact_stream();
    let mut engine = engine();
    for (word, pick, any_order) in
        [("משה", Pick::FIRST, false), ("שמה", Pick::FIRST, true), ("ים", Pick::LAST, false), ("רא", Pick { nth: 3, from_end: true }, true)]
    {
        let mut out = Vec::new();
        let total = engine.search_in(word, pick, any_order, true, WHOLE, &mut out);
        let span = letter::letter_count(word);
        let want: Vec<u8> = word.chars().filter_map(letter::exact_code).collect();
        assert!(total > 0, "{word}");
        assert_eq!(out.len(), (total as usize).min(CAP) * span * LETTER_WORDS);
        for record in out.chunks_exact(span * LETTER_WORDS) {
            let mut got: Vec<u8> = record
                .chunks_exact(LETTER_WORDS)
                .map(|l| stream[starts[l[0] as usize] as usize + l[1] as usize])
                .collect();
            if any_order {
                got.sort_unstable();
                let mut w = want.clone();
                w.sort_unstable();
                assert_eq!(got, w, "{word}");
            } else {
                assert_eq!(got, want, "{word}");
            }
        }
    }
}

/// A query asked again is remembered, not searched; another pick, order,
/// mode or stretch is a new search.
#[test]
fn a_query_asked_again_is_remembered() {
    let mut engine = engine();
    engine.hits_in("שלם", Pick::FIRST, false, false, WHOLE);
    let runs = engine.runs;
    engine.hits_in("שלם", Pick::FIRST, false, false, WHOLE);
    assert_eq!(engine.runs, runs);
    engine.hits_in("שלם", Pick::LAST, false, false, WHOLE);
    engine.hits_in("שלם", Pick::FIRST, true, false, WHOLE);
    engine.hits_in("שלם", Pick::FIRST, false, true, WHOLE);
    engine.hits_in("שלם", Pick::FIRST, false, false, 0..100);
    assert_eq!(engine.runs, runs + 4);
}

/// The cost is the letters of the stretch read, known without searching:
/// a few verses cost a fraction of the whole, a later letter less than the
/// first, which every word holds.
#[test]
fn the_cost_is_the_stretch_read() {
    let mut engine = engine();
    let runs = engine.runs;
    let whole = engine.cost(Pick::FIRST, WHOLE);
    assert_eq!(whole, cost(engine.stream(Pick::FIRST).len()));
    assert!(engine.cost(Pick::FIRST, 0..100) < whole / 100.0);
    assert!(engine.cost(Pick { nth: 5, from_end: false }, WHOLE) < whole);
    assert_eq!(engine.runs, runs, "a cost searched");
}

/// Times each search over the first letters, natively — what the cost
/// constants were read from, scaled to V8.
/// `cargo test --release bench_words -- --ignored --nocapture`
#[test]
#[ignore]
fn bench_words() {
    let mut engine = engine();
    let n = engine.stream(Pick::FIRST).len();
    for any_order in [false, true] {
        for word in ["א", "אל", "משה", "יהוה", "בראשית"] {
            let codes: Vec<u8> = word.chars().filter_map(letter::code).collect();
            let stream = engine.stream(Pick::FIRST).folded.clone();
            let started = std::time::Instant::now();
            let mut out = Vec::new();
            let mut total = 0;
            for _ in 0..20 {
                total = hits(&stream, &codes, any_order, CAP, &mut out);
            }
            let ns = started.elapsed().as_nanos() as f64 / 20.0 / n as f64;
            eprintln!("{word}\tany_order={any_order}\t{total} hits\t{ns:.2} ns a letter");
        }
    }
}
