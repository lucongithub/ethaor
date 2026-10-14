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
    pattern(query, &mut out);
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
    assert_eq!(*starts.last().unwrap() as usize, stream().len(), "la sentinelle ne ferme pas le flux");
    for v in 0..corpus::verse_count() {
        let expected: Vec<u8> = corpus::verse_text(v).chars().filter_map(letter::code).collect();
        let welded = &stream()[starts[v] as usize..starts[v + 1] as usize];
        assert_eq!(welded, &expected[..], "verset {v} : le flux soude ne relit pas le texte");
    }
    assert!(stream().len() < 1 << START_BITS, "le flux ne tient plus dans la cle");
}

#[test]
fn final_forms_fold_onto_their_letter() {
    for (final_form, plain) in [('ך', 'כ'), ('ם', 'מ'), ('ן', 'נ'), ('ף', 'פ'), ('ץ', 'צ')] {
        assert_eq!(letter::code(final_form), letter::code(plain));
    }
    assert_eq!(letter::code('א'), Some(0));
    assert_eq!(letter::code('ת'), Some(ALPHABET as u8 - 1));
    assert_eq!(letter::code('\u{05B8}'), None, "une voyelle n'est pas une lettre");
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
            assert_eq!(index.positions(c), &want[..], "positions du code {c}");
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
                    assert_eq!(bit == 1, text[p] == 4, "ecart {gap}, inverse {reverse}, ancre {i}, pas {s}");
                    s += 1;
                }
            }
            for at in 0..n {
                // SAFETY: `at < n`, and a plane holds `n / 64 + 3` words.
                let w = unsafe { window(&plane.bits, at) };
                for b in 0..64 {
                    let bit = at + b;
                    let want = (plane.bits[bit >> 6] >> (bit & 63)) & 1;
                    assert_eq!((w >> b) & 1, want, "fenetre a {at}, bit {b}");
                }
            }
        }
    }
}

// ----------------------------------------------------------------- methods

/// **Every method finds exactly what the naive search finds**, on random
/// streams of every size from empty to thousands, over patterns of two to six
/// letters, contiguous, one skip, narrow and wide ranges, and ranges past the
/// end of the stream.
#[test]
fn every_method_finds_exactly_what_the_naive_search_finds() {
    let mut rng = Rng(2024);
    let mut checked = 0;
    for &n in &[0usize, 1, 2, 3, 5, 8, 63, 64, 65, 129, 400, 2000] {
        for alphabet in [&[3u8, 8][..], &[0, 1, 2][..], &[5, 6, 7, 9][..]] {
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
                        assert_eq!(got, want, "{method:?}, n {n}, motif {pattern:?}, sauts {skip0}..={skip1}");
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
        assert!(!reference.is_empty(), "{query} : aucune occurrence");
        for method in [Method::Pairs, Method::Planes] {
            assert_eq!(by_method(&mut engine.index, &pattern, skip0, skip1, method), reference, "{query}, {method:?}");
        }
    }
    for (query, skip0, skip1) in [("אדם", 0, 2000), ("משיח", 10, 20_000), ("צדק", 0, 50_000)] {
        let pattern = codes(query);
        let reference = by_method(&mut engine.index, &pattern, skip0, skip1, Method::Planes);
        assert!(!reference.is_empty(), "{query} : aucune occurrence");
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
                assert_eq!(total, want.len() as u64, "total faux, plafond {cap}, {method:?}");
                assert_eq!(got, want[..cap.min(want.len())], "plafond {cap}, {method:?}");
            }
        }
    }
}

#[test]
fn an_empty_query_finds_nothing() {
    assert_eq!(records("", 0, 10).0, 0);
    assert_eq!(records("abc 123", 0, 3).0, 0, "une requete sans lettre hebraique trouve quelque chose");
    assert_eq!(records("אדם", 5, 4).0, 0, "une plage renversee trouve quelque chose");
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
    assert!(bare.1.iter().any(|r| r[4] == 25), "Gn 1,26 manque parmi les occurrences de אדם");
}

/// The classic skip: "תורה" spelled from the ת of בראשית, every fiftieth
/// letter — 49 letters jumped between two. A range that holds 49 finds it too.
#[test]
fn torah_is_read_every_fiftieth_letter_from_the_first_tav() {
    let first = records("תורה", 49, 49).1[0];
    assert_eq!(first, [5, 49, 0, 0, 0, 5, first[6]]);
    assert!(first[6] > 0, "l'occurrence devrait deborder du premier verset");
    assert!(records("תורה", 0, 100).1.contains(&first));
}

/// An occurrence ignores verse boundaries: the last letter of a verse and the
/// first two of the next are one hit, whose passage spans both.
#[test]
fn an_occurrence_may_span_two_verses() {
    let at = verse_starts()[1] as usize - 1;
    let query: String = stream()[at..at + 3].iter().map(|&c| ALEPH_BET[c as usize]).collect();
    let (_, recs) = records(&query, 0, 0);
    let hit = recs.iter().find(|r| r[0] as usize == at).expect("l'occurrence a cheval manque");
    assert_eq!((hit[4], hit[6]), (0, 1), "le passage doit couvrir les versets 0 et 1");
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
        assert!(!recs.is_empty(), "{query} : aucune occurrence");
        assert!(recs.len() as u64 <= total && recs.len() <= CAP);
        assert!(recs.windows(2).all(|w| (w[0][1], w[0][0]) < (w[1][1], w[1][0])), "{query} : ordre faux");
        for r in &recs {
            let [letter, skip, book, chapter, verse, rank, last] = r.map(|w| w as usize);
            assert!((skip0 as usize..=skip1 as usize).contains(&skip));
            let step = skip + 1;
            let span = (pattern.len() - 1) * step;
            let verse_of = |l: usize| starts.partition_point(|&x| x as usize <= l) - 1;
            assert_eq!(verse, verse_of(letter), "{query} : verset du debut faux");
            assert_eq!(last, verse_of(letter + span), "{query} : verset de la fin faux");
            assert_eq!(letter, starts[verse] as usize + rank);
            assert!(corpus::verses_of(chapter).contains(&verse));
            assert!(corpus::chapters_of(book).contains(&chapter));
            for (j, &c) in pattern.iter().enumerate() {
                assert_eq!(text[letter + j * step], c, "{query} : la lettre {j} ne correspond pas");
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
    assert!(total > CAP as u64, "le cas ne deborde pas : {total}");
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
    assert!(ms < 1500.0, "10 001 sauts demandent {ms:.1} ms");
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
        assert_eq!(keys, want, "{len} cles de {bits} bits");
    }
}

/// The placement tables give each letter its verse, each verse its chapter,
/// each chapter its book — the very answers of the corpus index.
#[test]
fn the_placement_tables_agree_with_the_corpus() {
    let engine = engine();
    let starts = verse_starts();
    for letter in (0..stream().len()).step_by(7).chain([stream().len() - 1]) {
        assert_eq!(engine.verse_of(letter), starts.partition_point(|&x| x as usize <= letter) - 1, "lettre {letter}");
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
    assert_eq!(engine.search("אָדָם", 0, 50, &mut again), total, "le total change");
    assert_eq!(again, first, "la reponse change");
    assert_eq!(engine.runs, 1, "la requete repetee a ete recherchee");
    for k in 0..MEMO as u32 {
        engine.search("אדם", 1, 1 + k, &mut again);
    }
    engine.search("אדם", 0, 50, &mut again);
    assert_eq!(again, first);
    assert_eq!(engine.runs, 2 + MEMO as u64, "la plus ancienne reponse n'a pas ete oubliee");
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

