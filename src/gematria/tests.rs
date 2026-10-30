//! Gematria: each table is the chart the reference publishes and the rule
//! that defines it, the tally is the text read once, and every method
//! answers what its definition gives letter by letter — over every verse.

use super::*;
use crate::corpus;

/// The 22 letters, alef to tav, as the charts list them.
const LETTERS: &str = "אבגדהוזחטיכלמנסעפצקרשת";

/// The 27 forms in Unicode order, each final its own.
const FORMS_IN_ORDER: &str = "אבגדהוזחטיךכלםמןנסעףפץצקרשת";

/// The standard values, alef to tav, as the chart gives them.
const STANDARD_CHART: [u32; 22] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 200, 300, 400];

/// The final forms and the letter each closes.
const FINALS: [(char, char); 5] = [('ך', 'כ'), ('ם', 'מ'), ('ן', 'נ'), ('ף', 'פ'), ('ץ', 'צ')];

fn form(c: char) -> usize {
    letter::exact_code(c).expect("not a consonant") as usize
}

fn standard(c: char) -> u32 {
    let base = FINALS.iter().find(|(f, _)| *f == c).map_or(c, |&(_, b)| b);
    STANDARD_CHART[LETTERS.chars().position(|l| l == base).expect("not a letter")]
}

fn gadol(c: char) -> u32 {
    FINALS.iter().position(|(f, _)| *f == c).map_or_else(|| standard(c), |k| 500 + 100 * k as u32)
}

fn table(method: Method) -> &'static [u32; FORMS] {
    method.table().unwrap_or_else(|| panic!("{method:?} has no table"))
}

/// A table over the 22 letters, each final form worth its letter.
fn assert_letters(method: Method, chart: [u32; 22]) {
    let t = table(method);
    for (l, expected) in LETTERS.chars().zip(chart) {
        assert_eq!(t[form(l)], expected, "{method:?}: {l} is not the chart's");
    }
    for (f, base) in FINALS {
        assert_eq!(t[form(f)], t[form(base)], "{method:?}: final {f} is not worth {base}");
    }
}

/// A substitution: letter by letter, the value of the letter the chart puts
/// in its place, read in `values` — and that letter itself, the one the
/// page shows in its place (`substitute`).
fn assert_substitution(method: Method, order: &str, partners: &str, values: fn(char) -> u32) {
    let t = table(method);
    let read_as = method.substitute().unwrap_or_else(|| panic!("{method:?} replaces no letter"));
    assert_eq!(order.chars().count(), partners.chars().count(), "{method:?}: the chart is cut short");
    for (c, partner) in order.chars().zip(partners.chars()) {
        assert_eq!(t[form(c)], values(partner), "{method:?}: {c} does not take {partner}'s value");
        assert_eq!(read_as[form(c)] as usize, form(partner), "{method:?}: {c} is not read as {partner}");
    }
}

/// The value tables are the reference's charts, number for number: each
/// of the 22 letters, and each final form worth its letter — the tables
/// `table_of` builds, for the methods `has_table` says are one.
#[test]
fn the_value_tables_are_the_reference_s_charts() {
    assert_letters(Method::MisparHapanim, STANDARD_CHART);
    assert_letters(Method::MisparKatan, [1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4]);
    assert_letters(
        Method::MisparPerati,
        [1, 4, 9, 16, 25, 36, 49, 64, 81, 100, 400, 900, 1600, 2500, 3600, 4900, 6400, 8100, 10000, 40000, 90000, 160000],
    );
    assert_letters(
        Method::MisparShemi,
        [111, 412, 83, 434, 6, 12, 67, 418, 419, 20, 100, 74, 80, 106, 120, 130, 81, 104, 186, 510, 350, 406],
    );
    assert_letters(
        Method::MisparKidmi,
        [1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 75, 105, 145, 195, 255, 325, 405, 495, 595, 795, 1095, 1495],
    );
    assert_letters(
        Method::MisparMeshulash,
        [
            1, 8, 27, 64, 125, 216, 343, 512, 729, 1000, 8000, 27000, 64000, 125000, 216000, 343000, 512000, 729000, 1000000,
            8000000, 27000000, 64000000,
        ],
    );
    assert_letters(
        Method::MisparMispari,
        [13, 760, 636, 273, 348, 600, 372, 401, 770, 570, 620, 686, 323, 408, 660, 422, 446, 820, 46, 501, 1083, 720],
    );
}

/// Mispar Gadol and Mispar Siduri give the finals values of their own,
/// continuing the alphabet: 500 to 900, and ranks 23 to 27.
#[test]
fn gadol_and_siduri_continue_the_alphabet_with_the_finals() {
    let (gadol_t, siduri) = (table(Method::MisparGadol), table(Method::MisparSiduri));
    for (k, l) in LETTERS.chars().enumerate() {
        assert_eq!(gadol_t[form(l)], STANDARD_CHART[k], "gadol: {l}");
        assert_eq!(siduri[form(l)], k as u32 + 1, "siduri: {l}");
    }
    for (k, (f, _)) in FINALS.into_iter().enumerate() {
        assert_eq!(gadol_t[form(f)], 500 + 100 * k as u32, "gadol: {f}");
        assert_eq!(siduri[form(f)], 23 + k as u32, "siduri: {f}");
    }
}

/// Every substitution is the reference's chart: each letter takes the value
/// of the letter it names, and is read as that letter; final forms worth their letter — but for AtBach
/// and Ayak Bachar, which count the 27 forms, finals at 500 to 900. Ofanim's
/// is the last letter of each name (`last_letter`).
#[test]
fn the_substitutions_are_the_reference_s_charts() {
    assert_substitution(Method::Atbash, LETTERS, "תשרקצפעסנמלכיטחזוהדגבא", standard);
    assert_substitution(Method::Albam, LETTERS, "למנסעפצקרשתאבגדהוזחטיכ", standard);
    assert_substitution(Method::Achbi, LETTERS, "כיטחזוהדגבאתשרקצפעסנמל", standard);
    assert_substitution(Method::AchasBeta, LETTERS, "חטיכלמנסעפצקרשאבגדהוזת", standard);
    assert_substitution(Method::Ofanim, LETTERS, "פתלתאונתתדפדמנכנאיפשנו", standard);
    assert_substitution(Method::Avgad, LETTERS, "בגדהוזחטיכלמנסעפצקרשתא", standard);
    assert_substitution(Method::ReverseAvgad, LETTERS, "תאבגדהוזחטיכלמנסעפצקרש", standard);
    for method in [Method::Atbash, Method::Albam, Method::Achbi, Method::AchasBeta, Method::Ofanim, Method::Avgad, Method::ReverseAvgad] {
        for (f, base) in FINALS {
            assert_eq!(table(method)[form(f)], table(method)[form(base)], "{method:?}: final {f}");
        }
    }
    assert_substitution(Method::Atbach, FORMS_IN_ORDER, "טחזוהדגבאצךפעתסשנמלרכקיץףןם", gadol);
    assert_substitution(Method::AyakBachar, FORMS_IN_ORDER, "יכלמנסעפצקהרשותזךםןחףטץאבגד", gadol);
}

/// The value a substitution gives back, read as a letter: which letter of
/// the alphabet carries it.
fn partner(method: Method, l: usize) -> usize {
    let v = table(method)[form(LETTERS.chars().nth(l).unwrap())];
    STANDARD_CHART.iter().position(|&s| s == v).expect("a value no letter has")
}

/// The ciphers keep their shape: AtBash, AlBam and AchBi swap pairs (twice
/// is the letter back), Achas Beta comes back in three steps, Avgad and
/// Reverse Avgad undo each other, and each is a permutation of the
/// alphabet — no two letters take one value.
#[test]
fn the_ciphers_are_permutations_of_their_shape() {
    for method in [Method::Atbash, Method::Albam, Method::Achbi, Method::AchasBeta, Method::Avgad, Method::ReverseAvgad] {
        let mut taken = [false; 22];
        for l in 0..22 {
            let p = partner(method, l);
            assert!(!taken[p], "{method:?}: two letters take one value");
            taken[p] = true;
        }
    }
    for l in 0..22 {
        for method in [Method::Atbash, Method::Albam, Method::Achbi] {
            assert_eq!(partner(method, partner(method, l)), l, "{method:?} does not swap pairs");
        }
        let thrice = partner(Method::AchasBeta, partner(Method::AchasBeta, partner(Method::AchasBeta, l)));
        assert_eq!(thrice, l, "Achas Beta does not come back in three steps");
        assert_eq!(partner(Method::ReverseAvgad, partner(Method::Avgad, l)), l, "Avgad is not undone");
    }
}

/// The classic examples the reference gives: תורה is 611; alef's name
/// (אלף) is 111, without its alef 110; אחד builds to 23 and spells 13, whose
/// root is 4; and every value of תורה, each worked out by hand.
#[test]
fn the_reference_s_examples_are_found() {
    let value = |text: &str, method| tally(text).value(method);
    assert_eq!(value("א", Method::MisparShemi), 111);
    assert_eq!(value("א", Method::MisparNeelam), 110);
    assert_eq!(value("אחד", Method::MisparBoneeh), 23);
    assert_eq!(value("א", Method::MisparMispari), 13);
    assert_eq!(value("אחד", Method::MisparKatanMispari), 4);
    // ת 400, ו 6, ר 200, ה 5.
    let torah = [
        (Method::MisparHapanim, 611),
        (Method::MisparGadol, 611),
        (Method::MisparSiduri, 22 + 6 + 20 + 5),
        (Method::MisparKatan, 4 + 6 + 2 + 5),
        (Method::MisparPerati, 160000 + 36 + 40000 + 25),
        (Method::MisparShemi, 406 + 12 + 510 + 6),
        (Method::MisparMusafi, 611 + 4),
        (Method::Atbash, 1 + 80 + 3 + 90),
        (Method::Albam, 20 + 100 + 9 + 50),
        (Method::MisparBoneeh, 400 + 406 + 606 + 611),
        (Method::MisparKidmi, 1495 + 21 + 795 + 15),
        (Method::MisparNeelam, 6 + 6 + 310 + 1),
        (Method::MisparHamerubahHaklali, 611 * 611),
        (Method::MisparMeshulash, 64000000 + 216 + 8000000 + 125),
        (Method::MisparHaachor, 400 + 2 * 6 + 3 * 200 + 4 * 5),
        (Method::MisparMispari, 720 + 600 + 501 + 348),
        (Method::MisparKatanMispari, 8),
        (Method::MisparKolel, 612),
        (Method::Achbi, 30 + 6 + 50 + 7),
        (Method::Atbach, 600 + 4 + 800 + 5),
        (Method::AyakBachar, 4 + 60 + 2 + 50),
        (Method::Ofanim, 6 + 6 + 300 + 1),
        (Method::AchasBeta, 400 + 40 + 6 + 30),
        (Method::Avgad, 1 + 7 + 300 + 6),
        (Method::ReverseAvgad, 300 + 5 + 100 + 4),
    ];
    assert_eq!(torah.len(), Method::ALL.len(), "a method is left unworked");
    for (method, expected) in torah {
        assert_eq!(value("תורה", method), expected, "{method:?}");
    }
}

/// The methods by their definitions, letter by letter, with no tally: the
/// consonants read in order, each method's sum built as it describes it.
fn by_definition(text: &str, words: u64) -> Vec<u64> {
    let letters: Vec<usize> = text.chars().filter_map(letter::exact_code).map(usize::from).collect();
    let std: Vec<u64> = letters.iter().map(|&f| u64::from(table(Method::MisparHapanim)[f])).collect();
    let total: u64 = std.iter().sum();
    Method::ALL
        .iter()
        .map(|&method| match method {
            Method::MisparMusafi => total + letters.len() as u64,
            Method::MisparBoneeh => (0..std.len()).map(|k| std[..=k].iter().sum::<u64>()).sum(),
            Method::MisparNeelam => letters.iter().map(|&f| u64::from(table(Method::MisparShemi)[f])).sum::<u64>() - total,
            Method::MisparHamerubahHaklali => total.pow(2),
            Method::MisparHaachor => std.iter().enumerate().map(|(k, v)| v * (k as u64 + 1)).sum(),
            Method::MisparKatanMispari => {
                let mut v = total;
                while v > 9 {
                    v = v.to_string().bytes().map(|b| u64::from(b - b'0')).sum();
                }
                v
            }
            Method::MisparKolel => total + words,
            m => letters.iter().map(|&f| u64::from(table(m)[f])).sum(),
        })
        .collect()
}

/// Over every verse of the Tanakh, pointed as it is, each method answers
/// what its definition gives letter by letter: one pass and a few
/// multiplications are the sums written out. Words are the verse's, parted
/// by spaces and the maqaf.
#[test]
fn every_verse_is_worth_what_each_definition_gives() {
    for verse in 0..corpus::verse_count() {
        let text = corpus::verse_text(verse);
        let words = text.replace('\u{05BE}', " ").split_whitespace().filter(|w| letter::letter_count(w) > 0).count();
        let read = tally(text);
        let values: Vec<u64> = Method::ALL.iter().map(|&m| read.value(m)).collect();
        assert_eq!(values, by_definition(text, words as u64), "verse {verse}: {text}");
        assert_eq!(read.words as usize, words, "verse {verse}: words miscounted");
    }
}

/// A cipher's letters read back its value: over every verse, the letters
/// each replaced as the method reads them are worth, by their large value,
/// what the method gives the verse. Only the ciphers and Ofanim replace
/// letters; every other method reads them as they are.
#[test]
fn a_cipher_s_letters_read_back_its_value() {
    let replacing = [
        Method::Atbash,
        Method::Albam,
        Method::Achbi,
        Method::Atbach,
        Method::AyakBachar,
        Method::Ofanim,
        Method::AchasBeta,
        Method::Avgad,
        Method::ReverseAvgad,
    ];
    for method in Method::ALL {
        assert_eq!(method.substitute().is_some(), replacing.contains(&method), "{method:?}");
    }
    let mut read = String::new();
    for verse in 0..corpus::verse_count() {
        let text = corpus::verse_text(verse);
        for method in replacing {
            let read_as = method.substitute().unwrap();
            read.clear();
            read.extend(letters(text).map(|c| FORMS_IN_ORDER.chars().nth(read_as[form(c)] as usize).unwrap()));
            assert_eq!(tally(&read).value(Method::MisparGadol), tally(text).value(method), "{method:?}, verse {verse}");
        }
    }
}

/// Each letter's share of a text's value, over every verse: the shares add
/// up to the value, but for Kolel, HaMerubah HaKlali and Katan Mispari,
/// whose letters each carry their standard value, read by the formula; one
/// share a letter, in order — אחד builds to 3 + 16 + 4, each letter in the
/// running totals from it on.
#[test]
fn each_letter_s_shares_add_up_to_the_value() {
    let mut shares_of = Vec::new();
    shares("אחד", Method::MisparBoneeh, &mut shares_of);
    assert_eq!(shares_of, [3, 16, 4]);
    shares("אחד", Method::MisparHaachor, &mut shares_of);
    assert_eq!(shares_of, [1, 16, 12]);
    let formulas = [Method::MisparHamerubahHaklali, Method::MisparKatanMispari, Method::MisparKolel];
    for verse in (0..corpus::verse_count()).step_by(7) {
        let text = corpus::verse_text(verse);
        let read = tally(text);
        for method in Method::ALL {
            shares(text, method, &mut shares_of);
            assert_eq!(shares_of.len(), read.letters as usize, "{method:?}, verse {verse}: one share a letter");
            let sum: u64 = shares_of.iter().map(|&s| u64::from(s)).sum();
            assert_eq!(method.summed(), !formulas.contains(&method), "{method:?}");
            let expected = if method.summed() { read.value(method) } else { read.value(Method::MisparHapanim) };
            assert_eq!(sum, expected, "{method:?}, verse {verse}: {text}");
        }
    }
}

/// Only consonants count: points and cantillation change nothing, nor do
/// Latin letters, digits or punctuation; a final form stays itself.
#[test]
fn only_consonants_count() {
    let pointed = corpus::verse_text(0);
    let bare: String = pointed.chars().filter(|&c| letter::exact_code(c).is_some() || c == ' ').collect();
    let (a, b) = (tally(pointed), tally(&bare));
    assert_eq!((a.forms, a.letters, a.weighted), (b.forms, b.letters, b.weighted), "the points were counted");
    assert_eq!(tally("abc 12 ?! שלום"), tally("שלום"), "something not Hebrew was counted");
    assert_eq!(tally("ם").forms[form('ם')], 1, "a final form was folded");
    assert_eq!(letters("abc בְּרֵאשִׁית!").collect::<String>(), "בראשית");
}

/// Words are runs of letters and their marks holding a consonant, parted
/// by a space, a maqaf, a paseq, a sof pasuq or anything else — a
/// gershayim too; marks alone are no word. Each character as `class`
/// sorts it.
#[test]
fn words_are_parted_where_the_text_parts_them() {
    for (text, words) in [
        ("", 0),
        ("א", 1),
        ("  א  ב  ", 2),
        ("כָּל־הָאָרֶץ", 2),
        ("א׀ב", 2),
        ("א׃ ב", 2),
        ("א,ב.ג", 3),
        ("א-ב|ג", 3),
        ("\u{05B8} א", 1),
        ("ב״ה", 2),
    ] {
        assert_eq!(tally(text).words, words, "{text:?}");
    }
}

/// Only the 27 letters of the Unicode block are Hebrew letters: a
/// presentation form (a shin with its dot welded in, U+FB2A) or a Yiddish
/// ligature (U+05F0) is no letter of the Tanakh and counts for nothing.
#[test]
fn only_the_hebrew_block_s_letters_count() {
    for other in ['\u{FB2A}', '\u{FB31}', '\u{FB4F}', '\u{05F0}', '\u{05F2}'] {
        assert_eq!(tally(&other.to_string()).letters, 0, "{other:?} read as a letter");
        assert_eq!(letters(&other.to_string()).count(), 0, "{other:?} read as a letter");
    }
}

/// At `MAX_LETTERS` letters, the heaviest text each method can be given —
/// every letter the one it values most — stays exact in a JSON number and a
/// JavaScript one (2^53); one letter more is told, not counted.
#[test]
fn the_heaviest_text_stays_exact() {
    const EXACT: u64 = 1 << 53;
    let heaviest = |t: &[u32; FORMS]| (0..FORMS).max_by_key(|&f| t[f]).unwrap();
    let tav = form('ת');
    for method in Method::ALL {
        let f = method.table().map_or(tav, heaviest);
        let c = char::from_u32(0x05D0 + f as u32).unwrap();
        let text: String = std::iter::repeat_n(c, MAX_LETTERS).collect();
        let read = tally(&text);
        assert!(!read.truncated, "{method:?}: the bound itself was refused");
        assert!(read.value(method) < EXACT, "{method:?}: {} is past 2^53", read.value(method));
    }
    let over: String = std::iter::repeat_n('ת', MAX_LETTERS + 1).collect();
    let read = tally(&over);
    assert!(read.truncated, "a letter past the bound went unnoticed");
    assert_eq!(read.letters as usize, MAX_LETTERS);
}

/// Reading the longest text allowed, by every method, stays far under a
/// frame — loose enough for a debug build, and still caught the day the
/// work grows with the methods.
#[test]
fn every_method_reads_the_longest_text_in_a_blink() {
    let text: String = corpus::text().chars().filter(|&c| letter::exact_code(c).is_some()).take(MAX_LETTERS).collect();
    let started = std::time::Instant::now();
    let read = tally(&text);
    let sum: u64 = Method::ALL.iter().map(|&m| read.value(m)).fold(0, u64::wrapping_add);
    let took = started.elapsed();
    assert!(sum > 0);
    assert!(took.as_millis() < 20, "{MAX_LETTERS} letters took {took:?}");
}

mod search {
    //! The search: every run of whole words of one verse worth the target,
    //! and nothing else — held against reading every run by its definition.

    use super::super::search::{STRIDE, stream};
    use super::super::*;
    use crate::corpus;

    /// A verse's words, each as the letters it is read as.
    fn words(verse: usize) -> Vec<String> {
        corpus::verse_text(verse).split(' ').map(|w| letters(w).collect::<String>()).filter(|w| !w.is_empty()).collect()
    }

    /// Every run `(verse, from, to)` — ranks of its letters — of the verses
    /// worth `target` by `method`, each run read by `tally` as a phrase:
    /// the definition, run by run, no window. Mispar Katan Mispari's are
    /// single words.
    fn every_run(method: Method, target: u64, verses: impl Iterator<Item = usize>) -> Vec<u32> {
        let mut out = Vec::new();
        for verse in verses {
            let words = words(verse);
            let mut starts = Vec::new();
            let mut at = 0;
            for w in &words {
                starts.push(at);
                at += w.chars().count();
            }
            for i in 0..words.len() {
                let longest = if method == Method::MisparKatanMispari { i + 1 } else { words.len() };
                for j in i + 1..=longest {
                    if tally(&words[i..j].join(" ")).value(method) == target {
                        let end = starts[j - 1] + words[j - 1].chars().count();
                        out.extend_from_slice(&[verse as u32, starts[i] as u32, end as u32]);
                    }
                }
            }
        }
        out
    }

    /// **Every method finds exactly the runs worth its target**, read one by
    /// one by their definition: over a stretch of verses from across the
    /// Tanakh, targets taken from one word, from two, from a whole verse,
    /// and one no run reaches — each verse's words as `words_of` cuts them.
    #[test]
    fn every_search_finds_exactly_the_runs_worth_its_target() {
        let stream = stream().expect("the text is in");
        let verses = 4000..4120;
        for method in Method::ALL {
            let sample = words(4010);
            let mut targets: Vec<u64> =
                [&sample[..1], &sample[..2], &sample[..]].iter().map(|run| tally(&run.join(" ")).value(method)).collect();
            targets.push(7_777_777);
            for target in targets {
                let mut found = Vec::new();
                let total = stream.search(method, target, verses.clone(), usize::MAX, &mut found);
                let expected = every_run(method, target, verses.clone());
                assert_eq!(found, expected, "{method:?} at {target}: not the runs worth it");
                assert_eq!(total as usize, found.len() / STRIDE, "{method:?}: miscounted");
            }
        }
    }

    /// A phrase finds itself where it is written: the first two words of
    /// Genesis, at the head of its first verse, by every method — Katan
    /// Mispari aside, whose hits are single words.
    #[test]
    fn a_phrase_finds_itself() {
        let stream = stream().expect("the text is in");
        let phrase = "בְּרֵאשִׁית בָּרָא";
        for method in Method::ALL.into_iter().filter(|&m| m != Method::MisparKatanMispari) {
            let mut found = Vec::new();
            stream.search(method, tally(phrase).value(method), 0..1, usize::MAX, &mut found);
            assert!(found.chunks(STRIDE).any(|h| h == [0, 0, 9]), "{method:?} does not find the first words of Genesis");
        }
    }

    /// Mispar HaMerubah HaKlali's runs are Mispar haPanim's at the target's
    /// root; a target that is no square has none.
    #[test]
    fn a_square_s_runs_are_its_root_s() {
        let stream = stream().expect("the text is in");
        let (mut squared, mut plain) = (Vec::new(), Vec::new());
        let all = 0..corpus::verse_count();
        let total = stream.search(Method::MisparHamerubahHaklali, 611 * 611, all.clone(), usize::MAX, &mut squared);
        stream.search(Method::MisparHapanim, 611, all.clone(), usize::MAX, &mut plain);
        assert!(total > 0);
        assert_eq!(squared, plain);
        assert_eq!(stream.search(Method::MisparHamerubahHaklali, 611 * 611 + 1, all, usize::MAX, &mut squared), 0);
        assert!(squared.is_empty(), "a target no square found something");
    }

    /// Past the cap the total stays exact and the first hits are kept, in
    /// the order of the text; a scope keeps the hits lying in it, the same
    /// as the whole Tanakh's filtered; an empty one finds nothing.
    #[test]
    fn the_cap_keeps_the_first_hits_and_a_scope_its_own() {
        let stream = stream().expect("the text is in");
        let all = 0..corpus::verse_count();
        let (mut every, mut first) = (Vec::new(), Vec::new());
        let total = stream.search(Method::MisparKatan, 7, all.clone(), usize::MAX, &mut every);
        assert_eq!(stream.search(Method::MisparKatan, 7, all.clone(), 5, &mut first), total);
        assert_eq!(first, every[..5 * STRIDE], "not the first hits");
        let hits: Vec<&[u32]> = every.chunks(STRIDE).collect();
        assert!(hits.windows(2).all(|w| w[0] < w[1]), "out of the order of the text");
        let scope = 1533..4000;
        let mut kept = Vec::new();
        stream.search(Method::MisparKatan, 7, scope.clone(), usize::MAX, &mut kept);
        let filtered: Vec<u32> =
            hits.iter().filter(|h| scope.contains(&(h[0] as usize))).flat_map(|h| h.iter().copied()).collect();
        assert_eq!(kept, filtered);
        assert_eq!(stream.search(Method::MisparKatan, 7, std::ops::Range { start: 9, end: 3 }, usize::MAX, &mut kept), 0, "an empty scope found something");
    }

    /// The stream holds every letter of the Tanakh, verse by verse; a
    /// stretch past the end reads up to the end.
    #[test]
    fn the_stream_reads_every_letter() {
        let stream = stream().expect("the text is in");
        let letters: usize = (0..corpus::verse_count()).map(|v| letter::letter_count(corpus::verse_text(v))).sum();
        assert_eq!(stream.letters_in(0..corpus::verse_count()), letters);
        assert_eq!(stream.letters_in(5..5), 0);
        assert_eq!(stream.letters_in(0..usize::MAX), letters, "past the end is the end");
    }

    /// Every method over the whole Tanakh, one after the other, stays well
    /// under two seconds even in a debug build.
    #[test]
    fn every_method_searches_the_whole_tanakh_in_a_blink() {
        let stream = stream().expect("the text is in");
        let started = std::time::Instant::now();
        let mut out = Vec::new();
        for method in Method::ALL {
            stream.search(method, tally("תורה").value(method), 0..corpus::verse_count(), 100_000, &mut out);
        }
        let took = started.elapsed();
        assert!(took.as_millis() < 2000, "25 searches took {took:?}");
    }

    /// Measures a search natively, per letter of the stream read: what
    /// `NS_PER_LETTER` is set from.
    /// `cargo test --release bench_search -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_search() {
        let stream = stream().expect("the text is in");
        let n = stream.letters_in(0..corpus::verse_count());
        let mut out = Vec::new();
        for method in Method::ALL {
            let started = std::time::Instant::now();
            let mut total = 0;
            for _ in 0..10 {
                total = stream.search(method, tally("תורה").value(method), 0..corpus::verse_count(), 100_000, &mut out);
            }
            let ns = started.elapsed().as_nanos() as f64 / 10.0 / n as f64;
            eprintln!("{method:?}\t{total} hits\t{ns:.2} ns a letter");
        }
    }
}
