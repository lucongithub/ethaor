//! The search: the welded stream is the text's letters and nothing else, the
//! derived indices say what the stream says, every method finds exactly what
//! trying every start and every skip finds, and every record places its hit
//! where it really is.
//!
//! Most of the proof runs on **random streams over two to four letters**: hits
//! are dense there, so every edge — first letter, last letter, a step that
//! just fits, a range past the stream — is met thousands of times.

use std::sync::{Mutex, MutexGuard, OnceLock};

use super::method::window;
use super::*;
use crate::letter::ALPHABET;

// ----------------------------------------------------------------- helpers

/// Deterministic xorshift: the same streams on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A random stream of `n` letters drawn from `letters`.
fn random_text(rng: &mut Rng, n: usize, letters: &[u8]) -> Vec<u8> {
    (0..n).map(|_| letters[rng.below(letters.len())]).collect()
}

/// The reference every method is held to: every skip, every start, every
/// letter read. Sorted by skip then start, as `hits` promises.
fn naive(text: &[u8], pattern: &[u8], skip0: u32, skip1: u32) -> Vec<(u32, u32)> {
    let n = text.len();
    let m = pattern.len();
    if m == 0 || skip0 > skip1 {
        return Vec::new();
    }
    if m == 1 {
        return (0..n).filter(|&p| text[p] == pattern[0]).map(|p| (p as u32, skip0)).collect();
    }
    let mut out = Vec::new();
    let mut skip = skip0 as u64;
    while skip <= skip1 as u64 {
        let step = skip as usize + 1;
        let span = (m - 1) * step;
        if span >= n {
            break;
        }
        for start in 0..n - span {
            if pattern.iter().enumerate().all(|(j, &c)| text[start + j * step] == c) {
                out.push((start as u32, skip as u32));
            }
        }
        skip += 1;
    }
    out
}

/// What a method finds over a query, as (start, skip), sorted.
fn by_method(index: &mut Index, pattern: &[u8], skip0: u32, skip1: u32, method: Method) -> Vec<(u32, u32)> {
    let mut got = Vec::new();
    hits(index, pattern, skip0, skip1, usize::MAX, Some(method), &mut got);
    got
}

/// The corpus engine, built once for the whole suite.
fn engine() -> MutexGuard<'static, Engine> {
    static ONE: OnceLock<Mutex<Engine>> = OnceLock::new();
    ONE.get_or_init(|| Mutex::new(Engine::new())).lock().unwrap_or_else(|e| e.into_inner())
}

fn records(query: &str, skip0: u32, skip1: u32) -> (u64, Vec<[u32; STRIDE]>) {
    let mut out = Vec::new();
    let total = engine().search(query, skip0, skip1, &mut out);
    (total, out.chunks(STRIDE).map(|r| r.try_into().unwrap()).collect())
}

fn codes(query: &str) -> Vec<u8> {
    let mut out = Vec::new();
    pattern_of(query, false, &mut out);
    out
}

const ALEPH_BET: [char; ALPHABET] = [
    'א', 'ב', 'ג', 'ד', 'ה', 'ו', 'ז', 'ח', 'ט', 'י', 'כ', 'ל', 'מ', 'נ', 'ס', 'ע', 'פ', 'צ', 'ק', 'ר', 'ש', 'ת',
];

// ------------------------------------------------------------------ stream

/// The stream is the verses' consonants, folded, in order, and nothing else:
/// `build.rs` and `letter::code` agree on every verse.
#[test]
fn the_stream_is_every_verse_read_letter_by_letter() {
    let starts = verse_starts();
    assert_eq!(starts.len(), corpus::verse_count() + 1);
    assert_eq!(starts[0], 0);
    assert_eq!(*starts.last().unwrap() as usize, stream().len(), "the sentinel does not close the stream");
    for v in 0..corpus::verse_count() {
        let expected: Vec<u8> = corpus::verse_text(v).chars().filter_map(letter::code).collect();
        let welded = &stream()[starts[v] as usize..starts[v + 1] as usize];
        assert_eq!(welded, &expected[..], "verse {v}: the welded stream does not read the text back");
    }
    assert!(stream().len() < 1 << START_BITS, "the stream no longer fits in the key");
}

/// The letter count `build.rs` welds, which bounds a skip before the text
/// lands, is the stream's length once it has.
#[test]
fn the_letters_counted_at_build_are_the_stream() {
    assert_eq!(LETTER_COUNT as usize, stream().len(), "build.rs counts other letters than the stream holds");
}

/// The largest skip is the last at which a word's first and last letters
/// both lie in the stream: one more and they cannot, and the engine finds
/// nothing past it. A single letter has none.
#[test]
fn the_largest_skip_is_the_last_a_word_fits_in_the_tanakh() {
    let n = stream().len() as u64;
    assert_eq!(max_skip(0), 0);
    assert_eq!(max_skip(1), 0);
    for m in 2..=12u64 {
        let s = max_skip(m as usize) as u64;
        assert!((m - 1) * (s + 1) < n, "{m} letters at skip {s} overflow the stream");
        assert!((m - 1) * (s + 2) >= n, "{m} letters still fit at skip {}", s + 1);
    }
    // Two letters: any skip but the stream's end; three: half of that.
    assert_eq!(max_skip(2) as u64, n - 2);
    assert_eq!(max_skip(3) as u64, (n - 1) / 2 - 1);
    for word in ["את", "יהוה", "תורה"] {
        let s = max_skip(letter::letter_count(word));
        assert_eq!(records(word, s + 1, s + 1000).0, 0, "{word} found past its largest skip");
    }
}

#[test]
fn final_forms_fold_onto_their_letter() {
    for (final_form, plain) in [('ך', 'כ'), ('ם', 'מ'), ('ן', 'נ'), ('ף', 'פ'), ('ץ', 'צ')] {
        assert_eq!(letter::code(final_form), letter::code(plain));
    }
    assert_eq!(letter::code('א'), Some(0));
    assert_eq!(letter::code('ת'), Some(ALPHABET as u8 - 1));
    assert_eq!(letter::code('\u{05B8}'), None, "a vowel is not a letter");
    assert_eq!(letter::code(' '), None);
}

// ----------------------------------------------------------------- indices

/// Postings list every position of every letter, ascending, and the counts
/// are theirs.
#[test]
fn postings_list_every_position_of_every_letter() {
    let mut rng = Rng(7);
    let text = random_text(&mut rng, 5000, &[0, 3, 9, 21]);
    for t in [&text[..], stream()] {
        let index = Index::new(t);
        let mut total = 0;
        for c in 0..ALPHABET as u8 {
            let want: Vec<u32> = (0..t.len()).filter(|&p| t[p] == c).map(|p| p as u32).collect();
            assert_eq!(index.positions(c), &want[..], "positions of code {c}");
            assert_eq!(index.freq[c as usize] as usize, want.len());
            total += want.len();
        }
        assert_eq!(total, t.len());
    }
}

/// A residue list holds the positions of its letter with its residue, and
/// only those, ascending.
#[test]
fn residue_lists_split_a_letter_by_its_gap() {
    let mut rng = Rng(11);
    let text = random_text(&mut rng, 3001, &[2, 5]);
    let mut index = Index::new(&text);
    for gap in 2..9 {
        for c in [2u8, 5] {
            let k = index.residues(c, gap);
            for r in 0..gap {
                let want: Vec<u32> =
                    (0..text.len()).filter(|&p| text[p] == c && p % gap == r).map(|p| p as u32).collect();
                assert_eq!(index.residue_list(k, r), &want[..], "code {c}, ecart {gap}, residu {r}");
            }
        }
    }
}

/// A plane's bit for an anchor and a step is the letter at that distance:
/// forward for a gap after the anchor, backward for one before. And a window
/// read anywhere gives the 64 bits that follow, wherever the word boundary.
#[test]
fn a_plane_bit_is_the_letter_at_that_step() {
    let mut rng = Rng(13);
    let text = random_text(&mut rng, 1500, &[1, 4, 6]);
    let n = text.len();
    let mut index = Index::new(&text);
    for gap in 1..6 {
        for reverse in [false, true] {
            let k = index.plane(4, gap, reverse);
            let plane = index.plane_at(k);
            for i in 0..n {
                let o = plane.origin(i, n);
                let mut s = 0;
                loop {
                    let p = if reverse { i.checked_sub(gap * s) } else { Some(i + gap * s).filter(|&p| p < n) };
                    let Some(p) = p else { break };
                    let bit = (plane.bits[(o + s) >> 6] >> ((o + s) & 63)) & 1;
                    assert_eq!(bit == 1, text[p] == 4, "gap {gap}, reverse {reverse}, anchor {i}, step {s}");
                    s += 1;
                }
            }
            for at in 0..n {
                // SAFETY: `at < n`, and a plane holds `n / 64 + 3` words.
                let w = unsafe { window(&plane.bits, at) };
                for b in 0..64 {
                    let bit = at + b;
                    let want = (plane.bits[bit >> 6] >> (bit & 63)) & 1;
                    assert_eq!((w >> b) & 1, want, "window at {at}, bit {b}");
                }
            }
        }
    }
}

// ----------------------------------------------------------------- methods

/// **Every method finds exactly what the naive search finds**, on random
/// streams of every size from empty to thousands — codes of the 27 forms
/// included — over patterns of two to six letters, contiguous, one skip, narrow and wide ranges, and ranges past the
/// end of the stream.
#[test]
fn every_method_finds_exactly_what_the_naive_search_finds() {
    let mut rng = Rng(2024);
    let mut checked = 0;
    for &n in &[0usize, 1, 2, 3, 5, 8, 63, 64, 65, 129, 400, 2000] {
        // The last alphabet holds codes only the exact stream has: finals.
        for alphabet in [&[3u8, 8][..], &[0, 1, 2][..], &[5, 6, 7, 9][..], &[11, 22, 26][..]] {
            let text = random_text(&mut rng, n, alphabet);
            let mut index = Index::new(&text);
            for _ in 0..6 {
                let m = 2 + rng.below(5);
                let pattern: Vec<u8> = (0..m).map(|_| alphabet[rng.below(alphabet.len())]).collect();
                let ranges = [
                    (0, 0),
                    (1, 1),
                    (0, 3),
                    (0, 70),
                    (rng.below(50) as u32, 50 + rng.below(200) as u32),
                    (0, u32::MAX),
                    (u32::MAX - 1, u32::MAX),
                ];
                for (skip0, skip1) in ranges {
                    let want = naive(&text, &pattern, skip0, skip1);
                    for method in METHODS {
                        let got = by_method(&mut index, &pattern, skip0, skip1, method);
                        assert_eq!(got, want, "{method:?}, n {n}, pattern {pattern:?}, skips {skip0}..={skip1}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 1000);
}

/// The methods agree on the corpus itself: all three over a narrow range,
/// `Pairs` and `Planes` over ranges no naive search — nor `Direct` in a test
/// build — could cover. Each is held to the naive search on random streams;
/// here the real letter frequencies and the stream's ends are met.
#[test]
fn the_methods_agree_on_the_corpus() {
    let mut engine = engine();
    for (query, skip0, skip1) in [("אדם", 0, 300), ("תורה", 40, 200), ("טז", 0, 500)] {
        let pattern = codes(query);
        let reference = by_method(&mut engine.index, &pattern, skip0, skip1, Method::Direct);
        assert!(!reference.is_empty(), "{query}: no occurrence");
        for method in [Method::Pairs, Method::Planes] {
            assert_eq!(by_method(&mut engine.index, &pattern, skip0, skip1, method), reference, "{query}, {method:?}");
        }
    }
    for (query, skip0, skip1) in [("אדם", 0, 2000), ("משיח", 10, 20_000), ("צדק", 0, 50_000)] {
        let pattern = codes(query);
        let reference = by_method(&mut engine.index, &pattern, skip0, skip1, Method::Planes);
        assert!(!reference.is_empty(), "{query}: no occurrence");
        assert_eq!(by_method(&mut engine.index, &pattern, skip0, skip1, Method::Pairs), reference, "{query}");
    }
    let pattern = codes("ישראל");
    let want = naive(stream(), &pattern, 0, 4);
    for method in METHODS {
        assert_eq!(by_method(&mut engine.index, &pattern, 0, 4, method), want, "{method:?}");
    }
}

/// Past the cap, the total stays exact and the records are the first `cap`
/// hits in (skip, start) order — for every cap, including one that falls in
/// the middle of a skip.
#[test]
fn the_cap_keeps_the_first_hits_and_the_exact_total() {
    let mut rng = Rng(99);
    let text = random_text(&mut rng, 3000, &[1, 2]);
    let mut index = Index::new(&text);
    for (pattern, skip0, skip1) in [(&[1u8, 2, 1][..], 0, 400), (&[2, 2][..], 3, 900), (&[1][..], 5, 9)] {
        let want = naive(&text, pattern, skip0, skip1);
        assert!(want.len() > 1000);
        for cap in [0, 1, 7, 500, want.len() - 1, want.len(), want.len() + 1] {
            for method in METHODS {
                let mut got = Vec::new();
                let total = hits(&mut index, pattern, skip0, skip1, cap, Some(method), &mut got);
                assert_eq!(total, want.len() as u64, "wrong total, cap {cap}, {method:?}");
                assert_eq!(got, want[..cap.min(want.len())], "plafond {cap}, {method:?}");
            }
        }
    }
}

#[test]
fn an_empty_query_finds_nothing() {
    assert_eq!(records("", 0, 10).0, 0);
    assert_eq!(records("abc 123", 0, 3).0, 0, "a query with no Hebrew letter finds something");
    assert_eq!(records("אדם", 5, 4).0, 0, "a reversed range finds something");
    let mut out = Vec::new();
    let mut index = Index::new(&[]);
    assert_eq!(hits(&mut index, &[0, 1], 0, 9, CAP, None, &mut out), 0);
}

/// A single letter has no skip: every occurrence once, under the first skip.
#[test]
fn a_single_letter_ignores_the_skip() {
    let (total, recs) = records("ט", 3, 9000);
    assert_eq!(total, engine().index.freq[8] as u64);
    assert!(recs.iter().all(|r| r[1] == 3));
}

// ---------------------------------------------------------------- examples

/// Marks are ignored and final forms folded: pointed, bare and unfolded
/// spellings are one search.
#[test]
fn pointing_and_final_forms_do_not_change_the_search() {
    let bare = records("אדם", 0, 0);
    assert_eq!(records("אָדָם", 0, 0), bare);
    assert_eq!(records("אדמ", 0, 0), bare);
    // Genesis 1:26, "נעשה אדם": global verse 25.
    assert!(bare.1.iter().any(|r| r[4] == 25), "Gen 1:26 missing from the occurrences of אדם");
}

/// The classic skip: "תורה" spelled from the ת of בראשית, every fiftieth
/// letter — 49 letters jumped between two. A range that holds 49 finds it too.
#[test]
fn torah_is_read_every_fiftieth_letter_from_the_first_tav() {
    let first = records("תורה", 49, 49).1[0];
    assert_eq!(first, [5, 49, 0, 0, 0, 5, first[6]]);
    assert!(first[6] > 0, "the occurrence should run past the first verse");
    assert!(records("תורה", 0, 100).1.contains(&first));
}

/// An occurrence ignores verse boundaries: the last letter of a verse and the
/// first two of the next are one hit, whose passage spans both.
#[test]
fn an_occurrence_may_span_two_verses() {
    let at = verse_starts()[1] as usize - 1;
    let query: String = stream()[at..at + 3].iter().map(|&c| ALEPH_BET[c as usize]).collect();
    let (_, recs) = records(&query, 0, 0);
    let hit = recs.iter().find(|r| r[0] as usize == at).expect("the occurrence across verses is missing");
    assert_eq!((hit[4], hit[6]), (0, 1), "the passage must cover verses 0 and 1");
}

// ----------------------------------------------------------------- records

/// Every record places its hit where it is: the letter is the verse's start
/// plus its rank, the verse is in the chapter, the chapter in the book, the
/// last letter in the last verse — and the letters read at its skip spell the
/// pattern. Records come sorted by skip, then start.
#[test]
fn every_record_places_its_hit_where_it_is() {
    let starts = verse_starts();
    let text = stream();
    for (query, skip0, skip1) in [("ה", 0u32, 0u32), ("יהוה", 0, 0), ("אדם", 0, 200), ("משה", 100, 1000), ("יה", 0, 400)] {
        let pattern = codes(query);
        let (total, recs) = records(query, skip0, skip1);
        assert!(!recs.is_empty(), "{query}: no occurrence");
        assert!(recs.len() as u64 <= total && recs.len() <= CAP);
        assert!(recs.windows(2).all(|w| (w[0][1], w[0][0]) < (w[1][1], w[1][0])), "{query}: wrong order");
        for r in &recs {
            let [letter, skip, book, chapter, verse, rank, last] = r.map(|w| w as usize);
            assert!((skip0 as usize..=skip1 as usize).contains(&skip));
            let step = skip + 1;
            let span = (pattern.len() - 1) * step;
            let verse_of = |l: usize| starts.partition_point(|&x| x as usize <= l) - 1;
            assert_eq!(verse, verse_of(letter), "{query}: wrong first verse");
            assert_eq!(last, verse_of(letter + span), "{query}: wrong last verse");
            assert_eq!(letter, starts[verse] as usize + rank);
            assert!(corpus::verses_of(chapter).contains(&verse));
            assert!(corpus::chapters_of(book).contains(&chapter));
            for (j, &c) in pattern.iter().enumerate() {
                assert_eq!(text[letter + j * step], c, "{query}: letter {j} does not match");
            }
        }
    }
}

/// A pair of common letters over a wide range overflows the cap: the total is
/// still exact — every method counts the same — and the page gets `CAP`
/// records.
#[test]
fn a_flood_of_hits_is_capped_and_still_counted() {
    let (total, recs) = records("יה", 0, 1000);
    assert!(total > CAP as u64, "the case does not overflow: {total}");
    assert_eq!(recs.len(), CAP);
    let mut engine = engine();
    let pattern = codes("יה");
    for method in METHODS {
        let mut out = Vec::new();
        assert_eq!(hits(&mut engine.index, &pattern, 0, 1000, 0, Some(method), &mut out), total, "{method:?}");
    }
}

/// Ten thousand skips of a common word, in an unoptimised test build. The
/// bound is loose — release answers in a few tens of milliseconds — but one
/// scan per skip, the approach the range search replaced, takes tens of
/// seconds here: that is the day it catches.
#[test]
fn a_range_search_runs_in_a_blink() {
    let mut out = Vec::new();
    engine().search("אדם", 0, 0, &mut out);
    let start = std::time::Instant::now();
    engine().search("משיח", 0, 10_000, &mut out);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    assert!(ms < 1500.0, "10,001 skips take {ms:.1} ms");
}

/// The cost model picks, on the corpus, the method the benchmark measured
/// fastest by a wide margin — in V8 and natively alike. Close calls are left
/// out: there the choice hardly matters, and the measure is noise.
#[test]
fn the_cost_model_picks_the_winner() {
    let engine = engine();
    let cases = [
        // One skip: trying it beats any index walk.
        ("אדם", 0, 0, Method::Direct),
        ("יהוה", 1000, 1000, Method::Direct),
        ("בראשית", 0, 0, Method::Direct),
        // A very rare second letter over a wide range: list its pairs.
        ("טז", 0, 999, Method::Pairs),
        // Common letters over tens of steps and more: planes.
        ("תורה", 0, 63, Method::Planes),
        ("אדם", 0, 9_999, Method::Planes),
        ("יהוה", 0, 999, Method::Planes),
        ("משה", 0, u32::MAX, Method::Planes),
        ("ישראל", 0, 99_999, Method::Planes),
    ];
    for (word, skip0, skip1, want) in cases {
        let got = choose(&features(&engine.index, &codes(word), skip0, skip1));
        assert_eq!(got, want, "{word} {skip0}..={skip1}");
    }
}

// --------------------------------------------------------------- benchmark

/// Words and ranges the methods are timed on: rare and common letters, two
/// to six of them, one skip to the whole stream.
const BENCH_WORDS: [&str; 12] =
    ["טז", "יה", "אדם", "צדק", "משה", "דוד", "תורה", "יהוה", "משיח", "שלום", "ישראל", "בראשית"];
const BENCH_RANGES: [(u32, u32); 17] = [
    (0, 0),
    (1000, 1000),
    (0, 1),
    (0, 3),
    (0, 7),
    (0, 15),
    (0, 31),
    (0, 63),
    (0, 127),
    (0, 255),
    (0, 999),
    (0, 4_999),
    (0, 9_999),
    (0, 49_999),
    (0, 99_999),
    (0, 499_999),
    (0, u32::MAX),
];

/// Times every method on every word and range, and prints what the cost
/// model reads — anchors, steps tried, the second letter's rarity — beside
/// each time. `COLD=1` empties the caches before each run, as a first query
/// pays them. Not a test: run it in release with
/// `cargo test --release bench_methods -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench_methods() {
    let cold = std::env::var("COLD").is_ok();
    let mut engine = engine();
    let mut out = Vec::new();
    eprintln!("word;skip0;skip1;anchors;steps;f2;lanes;total;direct;pairs;planes");
    for word in BENCH_WORDS {
        let pattern = codes(word);
        for (skip0, skip1) in BENCH_RANGES {
            let f = features(&engine.index, &pattern, skip0, skip1);
            let mut line = format!("{word};{skip0};{skip1};{};{};{:.5};{}", f.anchors, f.steps, f.second, f.lanes);
            let mut times = Vec::new();
            let mut total = 0;
            for method in METHODS {
                // Hopeless runs are skipped: tens of billions of reads.
                let work = match method {
                    Method::Direct => f.steps,
                    Method::Pairs => f.steps * f.second,
                    Method::Planes => f.steps / 64.0,
                };
                if work > 2e9 {
                    times.push(String::from("-"));
                    continue;
                }
                let mut runs = Vec::new();
                let budget = std::time::Instant::now();
                while runs.len() < 3 || (budget.elapsed().as_millis() < 200 && runs.len() < 40) {
                    if cold {
                        engine.index.forget();
                    }
                    let t = std::time::Instant::now();
                    total = hits(&mut engine.index, &pattern, skip0, skip1, CAP, Some(method), &mut out);
                    runs.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                times.push(format!("{:.4}", runs[runs.len() / 2]));
            }
            line += &format!(";{total};{}", times.join(";"));
            eprintln!("{line}");
        }
    }
}

/// The radix sort sorts: random keys of every width, duplicates, empty.
#[test]
fn the_radix_sort_sorts() {
    let mut rng = Rng(5);
    for (len, bits) in [(0usize, 8u32), (1, 8), (1000, 3), (5000, 21), (5000, 40), (3000, 64)] {
        let mut keys: Vec<u64> =
            (0..len).map(|_| if bits == 64 { rng.next() } else { rng.next() & ((1 << bits) - 1) }).collect();
        let mut want = keys.clone();
        want.sort_unstable();
        radix_sort(&mut keys);
        assert_eq!(keys, want, "{len} keys of {bits} bits");
    }
}

/// The placement tables give each letter its verse, each verse its chapter,
/// each chapter its book — the very answers of the corpus index.
#[test]
fn the_placement_tables_agree_with_the_corpus() {
    let engine = engine();
    let starts = verse_starts();
    for letter in (0..stream().len()).step_by(7).chain([stream().len() - 1]) {
        assert_eq!(engine.verse_of(letter), starts.partition_point(|&x| x as usize <= letter) - 1, "letter {letter}");
    }
    for c in 0..corpus::chapter_count() {
        for v in corpus::verses_of(c) {
            assert_eq!(engine.chapter_of_verse[v] as usize, c);
        }
    }
    for b in 0..corpus::book_count() {
        for c in corpus::chapters_of(b) {
            assert_eq!(engine.book_of_chapter[c] as usize, b);
        }
    }
}

/// A query asked again is answered from memory, identically; the memory
/// holds a few answers and forgets the oldest.
#[test]
fn a_repeated_query_is_answered_from_memory() {
    let mut engine = Engine::new();
    let mut first = Vec::new();
    let total = engine.search("אדם", 0, 50, &mut first);
    let mut again = Vec::new();
    assert_eq!(engine.search("אָדָם", 0, 50, &mut again), total, "the total changes");
    assert_eq!(again, first, "the answer changes");
    assert_eq!(engine.runs, 1, "the repeated query was searched again");
    for k in 0..MEMO as u32 {
        engine.search("אדם", 1, 1 + k, &mut again);
    }
    engine.search("אדם", 0, 50, &mut again);
    assert_eq!(again, first);
    assert_eq!(engine.runs, 2 + MEMO as u64, "the oldest answer was not forgotten");
}

/// Two letters are counted without listing, and the count is the naive one —
/// forward and backward gaps, every range, up to past the stream.
#[test]
fn two_letters_are_counted_exactly_without_listing() {
    let mut rng = Rng(31);
    for &n in &[2usize, 3, 64, 500, 3000] {
        let text = random_text(&mut rng, n, &[0, 1, 2]);
        let mut index = Index::new(&text);
        for pattern in [[0u8, 1], [1, 0], [2, 2], [0, 2]] {
            for (skip0, skip1) in [(0u32, 0u32), (0, 5), (3, 90), (0, u32::MAX), (7, 7)] {
                let s0 = skip0 as usize + 1;
                let s1 = (skip1 as usize + 1).min(n - 1);
                if s0 > s1 {
                    continue;
                }
                let got = method::count_pairs(&mut index, &method::Query { pattern: &pattern, s0, s1 });
                assert_eq!(got as usize, naive(&text, &pattern, skip0, skip1).len(), "n {n}, {pattern:?}, {skip0}..={skip1}");
            }
        }
    }
}


/// `hits` is what `search` places: the same (start, skip) pairs, in the same
/// order, with the same total — and one memory serves both, so a page asked
/// through one is not searched again through the other.
#[test]
fn hits_are_the_records_unplaced() {
    let mut engine = Engine::new();
    let mut records = Vec::new();
    let total = engine.search("משה", 10, 30, &mut records);
    let (again, found) = engine.hits("משה", 10, 30);
    assert_eq!(again, total);
    let pairs: Vec<(u32, u32)> = records.chunks(STRIDE).map(|r| (r[0], r[1])).collect();
    assert_eq!(found, &pairs[..]);
    assert_eq!(engine.runs, 1, "hits searched what search had already found");
}

/// Every occurrence of `pattern` with a skip in `skip0..=skip1` whose every
/// letter lies in `range`, found by trying them all.
fn naive_in(text: &[u8], pattern: &[u8], skip0: u32, skip1: u32, range: std::ops::Range<usize>) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for skip in skip0..=skip1 {
        let step = skip as usize + 1;
        for start in range.clone() {
            let end = start + (pattern.len() - 1) * step;
            if end < range.end && (0..pattern.len()).all(|k| text[start + k * step] == pattern[k]) {
                out.push((start as u32, skip));
            }
        }
    }
    out
}

/// A search kept to a stretch finds exactly the occurrences lying wholly in
/// it — tried one by one — given back as ranks in the whole stream.
#[test]
fn a_scoped_search_finds_what_lies_wholly_in_its_stretch() {
    let mut engine = Engine::new();
    let starts = verse_starts();
    let text = stream();
    let mut codes = Vec::new();
    for (word, skip0, skip1, first, end) in
        [("אל", 0, 30, 0, 31), ("יהוה", 0, 12, 50, 80), ("משה", 3, 40, 1533, 1600), ("ה", 2, 9, 7, 9)]
    {
        let range = starts[first] as usize..starts[end] as usize;
        pattern_of(word, false, &mut codes);
        let (total, found) = engine.hits_in(word, skip0, skip1, range.clone());
        let expected = naive_in(text, &codes, skip0, if codes.len() == 1 { skip0 } else { skip1 }, range);
        assert_eq!(total as usize, expected.len(), "{word}");
        assert_eq!(found, &expected[..], "{word}");
    }
}

/// The whole stream as a stretch is the plain search, and a stretch asked
/// again is remembered like any query.
#[test]
fn the_whole_stretch_is_the_plain_search() {
    let mut engine = Engine::new();
    let (total, found) = engine.hits("שלום", 0, 20);
    let found = found.to_vec();
    let (again, scoped) = engine.hits_in("שלום", 0, 20, 0..stream().len());
    assert_eq!((again, scoped), (total, &found[..]));
    assert_eq!(engine.runs, 1);
    engine.hits_in("שלום", 0, 20, 100..90_000);
    engine.hits_in("שלום", 0, 20, 100..90_000);
    assert_eq!(engine.runs, 2, "a passage asked again was searched again");
}

/// The estimate is read without searching, grows with the range of skips,
/// and tells a light query from a flood by orders of magnitude.
#[test]
fn the_cost_is_estimated_without_searching() {
    let mut engine = Engine::new();
    let n = stream().len();
    let light = engine.cost("משה", 49, 49, 0..n);
    let wider = engine.cost("משה", 0, 500, 0..n);
    let flood = engine.cost("את", 0, u32::MAX, 0..n);
    let scoped = engine.cost("את", 0, u32::MAX, 0..n / 20);
    assert_eq!(engine.runs, 0, "the estimate ran a search");
    assert!(light < wider && wider < flood, "{light} {wider} {flood}");
    assert!(flood > 1000.0 * light);
    assert!(scoped < flood, "a passage costs as much as the whole text");
}

/// An engine searching exact letters, built once for the tests that need
/// it: the shared one stays folded, whatever runs beside.
fn exact_records(query: &str, skip0: u32, skip1: u32) -> (u64, Vec<(u32, u32)>) {
    static ONE: OnceLock<Mutex<Engine>> = OnceLock::new();
    let mut engine = ONE
        .get_or_init(|| {
            let mut engine = Engine::new();
            engine.exact = true;
            Mutex::new(engine)
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (total, hits) = engine.hits(query, skip0, skip1);
    (total, hits.to_vec())
}

/// **The exact stream is the folded one with its finals told apart**: one
/// byte for one byte, each folding onto the folded stream's — so a rank,
/// and every verse start, name the same letter in both — and finals stand
/// only where the text writes them.
#[test]
fn the_exact_stream_tells_the_finals_apart() {
    let (folded, exact) = (stream(), exact_stream());
    assert_eq!(exact.len(), folded.len());
    for (k, (&e, &f)) in exact.iter().zip(folded).enumerate() {
        let c = char::from_u32(0x05D0 + e as u32).unwrap();
        assert_eq!(letter::code(c), Some(f), "letter {k}");
    }
    let mut seen = [false; letter::FORMS];
    for v in (0..corpus::verse_count()).step_by(97) {
        let written: Vec<u8> = corpus::verse_text(v).chars().filter_map(letter::exact_code).collect();
        let starts = verse_starts();
        assert_eq!(&exact[starts[v] as usize..starts[v + 1] as usize], &written[..], "verse {v}");
        for &e in &written {
            seen[e as usize] = true;
        }
    }
    assert!(seen.iter().all(|&s| s), "a form never written");
}

/// **Folded is the union of the exact spellings**: every word, over a skip
/// and a range, finds folded exactly what its spellings with each foldable
/// letter final or not find exactly — counted and listed — and no hit twice.
#[test]
fn a_folded_search_is_the_union_of_its_exact_spellings() {
    // Letters with a final form, and the form.
    let finals = [('כ', 'ך'), ('מ', 'ם'), ('נ', 'ן'), ('פ', 'ף'), ('צ', 'ץ')];
    for (word, skip0, skip1) in [("אדם", 0, 0), ("מלך", 0, 0), ("נפש", 0, 3), ("צדק", 5, 5)] {
        let chars: Vec<char> = word.chars().collect();
        let open: Vec<usize> = (0..chars.len()).filter(|&k| finals.iter().any(|f| f.0 == chars[k] || f.1 == chars[k])).collect();
        let mut total = 0;
        let mut union = Vec::new();
        for bits in 0..1u32 << open.len() {
            let mut spelled = chars.clone();
            for (j, &k) in open.iter().enumerate() {
                let f = finals.iter().find(|f| f.0 == chars[k] || f.1 == chars[k]).unwrap();
                spelled[k] = if bits & 1 << j != 0 { f.1 } else { f.0 };
            }
            let spelled: String = spelled.into_iter().collect();
            let (t, hits) = exact_records(&spelled, skip0, skip1);
            total += t;
            union.extend(hits);
        }
        let (folded, mut hits) = {
            let mut engine = engine();
            let (t, h) = engine.hits(word, skip0, skip1);
            (t, h.to_vec())
        };
        assert_eq!(total, folded, "{word}: the spellings do not add up");
        union.sort_by_key(|&(start, skip)| (skip, start));
        hits.sort_by_key(|&(start, skip)| (skip, start));
        assert_eq!(union, hits, "{word}: the spellings do not list the same hits");
    }
}

/// **An exact hit is spelled as asked**: every letter of every hit of `אדמ`
/// is in the exact stream the form the word gives — a mem never final — and
/// `אדם` exactly finds its final mem only.
#[test]
fn an_exact_hit_is_spelled_as_asked() {
    for word in ["אדמ", "אדם"] {
        let want: Vec<u8> = word.chars().filter_map(letter::exact_code).collect();
        let (total, hits) = exact_records(word, 0, 0);
        assert!(total > 0, "{word}: nothing found");
        for (start, skip) in hits {
            let got: Vec<u8> =
                (0..want.len()).map(|i| exact_stream()[start as usize + i * (skip as usize + 1)]).collect();
            assert_eq!(got, want, "{word} at {start}");
        }
    }
    let (folded, _) = engine().hits("אדם", 0, 0);
    let (a, _) = exact_records("אדמ", 0, 0);
    let (b, _) = exact_records("אדם", 0, 0);
    assert!(a > 0 && b > 0 && a + b == folded, "{a} + {b} != {folded}");
}

/// **A query asked folded then exact is two answers**: the memory keys the
/// mode — codes alone collide, folded `כ` and exact `ך` both being 10 — so a
/// recall never hands one for the other.
#[test]
fn the_memory_keeps_folded_and_exact_apart() {
    let mut exact_codes = Vec::new();
    pattern_of("אך", true, &mut exact_codes);
    assert_eq!((codes("אכ"), exact_codes), (vec![0, 10], vec![0, 10]), "the codes no longer collide");
    // One engine, both modes, one after the other: what the memory holds.
    let mut engine = Engine::new();
    let (folded, _) = engine.hits("אכ", 0, 0);
    engine.exact = true;
    let (exact, _) = engine.hits("אך", 0, 0);
    engine.exact = false;
    let (again, _) = engine.hits("אכ", 0, 0);
    assert!(exact < folded, "exact אך found as many as folded אכ: {exact} {folded}");
    assert_eq!(again, folded, "the folded answer came back exact");
}
