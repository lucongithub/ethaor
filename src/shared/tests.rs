//! The shared layer: what both transports answer, checked once.

use super::els::{self as search, CAP, Engine, MAX_LIMIT, Query, Refusal, STRIDE};
use super::reference::{self, BOOKS, numeral_value};
use crate::{corpus, els, letter, pack, variant};

fn query(word: &str, skip_min: u32, skip_max: u32, offset: usize, limit: usize) -> Query {
    Query { word: word.into(), skip_min, skip_max, offset, limit, scope: None }
}

/// The English table walks the corpus' books in step: a book left out or
/// swapped would name every later verse wrong.
#[test]
fn the_book_table_follows_the_corpus() {
    const HEBREW: [&str; 39] = [
        "בראשית", "שמות", "ויקרא", "במדבר", "דברים", "יהושוע", "שופטים", "שמואל א", "שמואל ב", "מלכים א",
        "מלכים ב", "ישעיהו", "ירמיהו", "יחזקאל", "הושע", "יואל", "עמוס", "עובדיה", "יונה", "מיכה", "נחום",
        "חבקוק", "צפניה", "חגיי", "זכריה", "מלאכי", "תהילים", "משלי", "איוב", "שיר השירים", "רות", "איכה",
        "קוהלת", "אסתר", "דנייאל", "עזרא", "נחמיה", "דברי הימים א", "דברי הימים ב",
    ];
    assert_eq!(BOOKS.len(), corpus::book_count());
    for (b, name) in HEBREW.iter().enumerate() {
        assert_eq!(corpus::book_name(b), *name, "{} is not book {b}", BOOKS[b].english);
    }
    let osis: std::collections::HashSet<_> = BOOKS.iter().map(|b| b.osis).collect();
    assert_eq!(osis.len(), BOOKS.len(), "two books share an OSIS code");
    let chapters = |osis: &str| corpus::chapters_of(BOOKS.iter().position(|b| b.osis == osis).unwrap()).len();
    for (osis, n) in [("Gen", 50), ("Ps", 150), ("Isa", 66), ("1Sam", 31), ("2Sam", 24), ("Ezra", 10), ("Neh", 13), ("2Chr", 36)] {
        assert_eq!(chapters(osis), n, "{osis}");
    }
}

#[test]
fn a_numeral_reads_as_its_value() {
    assert_eq!(numeral_value("א"), Some(1));
    assert_eq!(numeral_value("טו"), Some(15));
    assert_eq!(numeral_value("טז"), Some(16));
    assert_eq!(numeral_value("קנ"), Some(150));
    assert_eq!(numeral_value(""), None);
    assert_eq!(numeral_value("a"), None);
    for n in 1..=200 {
        assert_eq!(numeral_value(&pack::hebrew_numeral(n)), Some(n as u32));
    }
}

/// Every label of the corpus is a numeral: chapters count 1, 2, 3… in each
/// book, and verses go on from where the previous one — joined or not —
/// stopped.
#[test]
fn the_numbers_are_the_labels() {
    for b in 0..corpus::book_count() {
        for (k, c) in corpus::chapters_of(b).enumerate() {
            let mut expected = 1;
            for v in corpus::verses_of(c) {
                let place = reference::verse(v);
                assert_eq!((place.book, place.chapter, place.verse), (b, c, v));
                assert_eq!(place.chapter_number, k as u32 + 1, "{}", place.hebrew());
                assert_eq!(place.verse_number, expected, "{}", place.hebrew());
                assert!(place.verse_last >= place.verse_number, "{}", place.hebrew());
                expected = place.verse_last + 1;
            }
        }
    }
}

#[test]
fn a_verse_is_named_every_way() {
    let first = reference::verse(0);
    assert_eq!((first.osis(), first.human(), first.hebrew()), ("Gen.1.1".into(), "Genesis 1:1".into(), "בראשית א,א".into()));
    let last = reference::verse(corpus::verse_count() - 1);
    assert_eq!(last.osis(), "2Chr.36.23");
    let joined = (0..corpus::verse_count()).map(reference::verse).find(|v| v.verse_last != v.verse_number);
    if let Some(v) = joined {
        let (b, c) = (v.book_osis(), v.chapter_number);
        assert_eq!(v.osis(), format!("{b}.{c}.{}-{b}.{c}.{}", v.verse_number, v.verse_last));
    }
}

/// Every letter of the stream is found back in its verse, at its rank there.
#[test]
fn every_letter_is_placed_in_its_verse() {
    let starts = els::verse_starts();
    for v in 0..corpus::verse_count() {
        for k in 0..(starts[v + 1] - starts[v]) as usize {
            assert_eq!(reference::letter(starts[v] as usize + k), (v, k));
        }
    }
}

#[test]
fn a_query_is_refused_with_its_reason() {
    let mut engine = Engine::new();
    let mut refused = |q: Query| search::search(&mut engine, &q).unwrap_err();
    assert_eq!(refused(query("abc", 0, 0, 0, 10)), Refusal::NoLetter);
    assert_eq!(refused(query("תורה", 5, 4, 0, 10)), Refusal::SkipRange { min: 5, max: 4 });
    assert_eq!(refused(query("תורה", 0, 0, 0, MAX_LIMIT + 1)), Refusal::Limit(MAX_LIMIT + 1));
    let codes: std::collections::HashSet<_> =
        [Refusal::NoLetter, Refusal::SkipRange { min: 1, max: 0 }, Refusal::Limit(0), Refusal::TextMissing]
            .iter()
            .map(Refusal::code)
            .collect();
    assert_eq!(codes.len(), 4, "two refusals share a code");
}

/// A page says what the engine's records say: the same hits in the same
/// order, each letter spelling the word in the verse the record names.
#[test]
fn a_page_places_the_engines_hits() {
    let mut engine = Engine::new();
    let page = search::search(&mut engine, &query("תּוֹרָה", 40, 60, 0, MAX_LIMIT)).unwrap();
    assert_eq!(page.letters, "תורה");
    let mut expected = Vec::new();
    assert_eq!(engine.search("תורה", 40, 60, &mut expected), page.total);
    assert_eq!(page.listed, expected.len() / STRIDE);
    assert!(page.hits.len() > 1);
    for (hit, r) in page.hits.iter().zip(expected.chunks(STRIDE)) {
        assert_eq!((hit.letters[0].rank, hit.skip), (r[0], r[1]));
        assert_eq!(hit.first_verse().book, r[2] as usize);
        assert_eq!(hit.first_verse().chapter, r[3] as usize);
        assert_eq!(hit.letters[0].verse, r[4] as usize);
        assert_eq!(hit.letters[0].position, r[5]);
        assert_eq!(hit.letters[3].verse, r[6] as usize);
        let word: String = hit.letters.iter().map(|l| l.char).collect();
        assert_eq!(word, "תורה");
        for pair in hit.letters.windows(2) {
            assert_eq!(pair[1].rank - pair[0].rank, hit.skip + 1);
        }
    }
}

/// `תורה` every fiftieth letter from the first tav of Genesis.
#[test]
fn torah_starts_in_the_first_verse() {
    let mut engine = Engine::new();
    let page = search::search(&mut engine, &query("תורה", 49, 49, 0, 1)).unwrap();
    let first = &page.hits[0];
    assert_eq!(first.first_verse().osis(), "Gen.1.1");
    assert_eq!(first.letters[0].position, 5);
}

/// Pages cut the listed hits without gap or repeat, the last says there is
/// no next, and a limit of 0 counts without listing.
#[test]
fn pages_tile_the_listed_hits() {
    let mut engine = Engine::new();
    let mut ask = |offset, limit| search::search(&mut engine, &query("שלמה", 0, 0, offset, limit)).unwrap();
    let whole = ask(0, MAX_LIMIT);
    assert!(whole.listed > 30 && whole.listed <= MAX_LIMIT, "the case does not fit in one page");
    let mut walked = Vec::new();
    let mut offset = Some(0);
    while let Some(at) = offset {
        let page = ask(at, 7);
        walked.extend(page.hits.iter().cloned());
        offset = page.next_offset();
    }
    assert_eq!(walked, whole.hits);
    let count = ask(0, 0);
    assert_eq!((count.total, count.hits.len(), count.next_offset()), (whole.total, 0, None));
    assert!(ask(whole.listed + 5, 10).hits.is_empty());
}

/// A flood: more hits than can be listed. The total stays exact, the listed
/// stop at `CAP`, and the last listed page closes the walk.
#[test]
fn a_flood_is_counted_whole_and_listed_to_the_cap() {
    let mut engine = Engine::new();
    let page = search::search(&mut engine, &query("את", 0, 200, 0, 5)).unwrap();
    assert_eq!(page.listed, CAP);
    assert!(page.total > CAP as u64);
    let last = search::search(&mut engine, &query("את", 0, 200, CAP - 3, 10)).unwrap();
    assert_eq!((last.hits.len(), last.next_offset()), (3, None));
    assert_eq!(engine.runs, 1, "one more page ran the search again");
}

/// Bounds a caller may send, however far: never a panic, never a hit past
/// the stream.
#[test]
fn extreme_bounds_answer_without_panic() {
    let mut engine = Engine::new();
    let n = els::stream().len() as u32;
    let wide = search::search(&mut engine, &query("את", 0, u32::MAX, usize::MAX, MAX_LIMIT)).unwrap();
    assert!(wide.total > 0 && wide.hits.is_empty() && wide.next_offset().is_none());
    let far = search::search(&mut engine, &query("את", n, u32::MAX, 0, MAX_LIMIT)).unwrap();
    assert_eq!(far.total, 0, "a skip longer than the stream finds something");
    let page = search::search(&mut engine, &query("אב", 0, u32::MAX, 0, MAX_LIMIT)).unwrap();
    for hit in &page.hits {
        assert!(hit.letters.iter().all(|l| l.rank < n));
    }
}

/// Final forms and marks change nothing; a word of marks alone, or of
/// another script, has no letter to search.
#[test]
fn finals_and_marks_fold_into_one_search() {
    let mut engine = Engine::new();
    let bare = search::search(&mut engine, &query("אדמ", 2, 9, 0, 50)).unwrap();
    assert!(!bare.hits.is_empty());
    for word in ["אדם", "אָדָם", " א-ד-ם "] {
        assert_eq!(search::search(&mut engine, &query(word, 2, 9, 0, 50)).unwrap().hits, bare.hits, "{word}");
    }
    for word in ["", "\u{05B8}\u{05B7}", "abc 123"] {
        assert_eq!(search::search(&mut engine, &query(word, 0, 0, 0, 1)).unwrap_err(), Refusal::NoLetter, "{word:?}");
    }
}

/// One letter has no skip: each occurrence counts once, under `skip_min`.
#[test]
fn a_single_letter_is_each_of_its_occurrences() {
    let mut engine = Engine::new();
    let page = search::search(&mut engine, &query("ט", 4, 9, 0, MAX_LIMIT)).unwrap();
    let code = letter::code('ט').unwrap();
    assert_eq!(page.total, els::stream().iter().filter(|&&c| c == code).count() as u64);
    assert!(page.hits.iter().all(|h| h.skip == 4 && h.letters.len() == 1 && h.letters[0].char == 'ט'));
}

/// The portions are the map's: same books, same order.
#[test]
fn the_portions_are_the_maps() {
    for (k, (_, _, books)) in reference::PORTIONS.iter().enumerate() {
        let (name, first, end) = crate::treemap::SECTIONS[k];
        assert_eq!((books.start, books.end), (first, end));
        assert_eq!(reference::portion(name), Some(books.clone()));
    }
    for (name, k) in [("Torah", 0), ("nevi'im", 1), ("NEVIIM", 1), ("Prophets", 1), ("ketuvim", 2), ("writings", 2)] {
        assert_eq!(reference::portion(name), Some(reference::PORTIONS[k].2.clone()), "{name}");
    }
    assert_eq!(reference::portion("gospels"), None);
}

/// A book is named by its OSIS code, exactly, and by nothing else: no
/// other case, no English, no Hebrew, no transliteration or abbreviation —
/// those are what answers give back, never what is read.
#[test]
fn a_book_is_named_by_its_osis_code_alone() {
    for (b, book) in BOOKS.iter().enumerate() {
        assert_eq!(reference::book(book.osis), Some(b), "{}", book.osis);
        assert_eq!(reference::book(&book.osis.to_lowercase()), None, "{} read in lower case", book.osis);
    }
    for other in ["Genesis", "בראשית", "Bereshit", "Gn.", "1 Samuel", "Song of Songs", "Matthew", "", " Gen", "Gen "] {
        assert_eq!(reference::book(other), None, "{other:?} read as a book");
    }
}

/// An OSIS reference names a book, a chapter or a verse; anything else —
/// another notation, a number past the text — names nothing.
#[test]
fn a_reference_names_its_verses() {
    let genesis = reference::book_verses(0);
    assert_eq!(reference::verses("Gen"), Some(genesis));
    assert_eq!(reference::verses("Gen.1"), Some(corpus::verses_of(0)));
    assert_eq!(reference::verses("Gen.1.1"), Some(0..1));
    let samuel = reference::verses("1Sam.3.4").unwrap();
    assert_eq!(reference::verse(samuel.start).osis(), "1Sam.3.4");
    assert_eq!(reference::verses("Ps.119").map(|v| v.len()), Some(176));
    for nothing in [
        "Gen.51", "Gen.0", "Gen.1.99", "Gen.1.1.1", "Gen.x", "Matt.1.1", "",
        "Genesis", "Genesis 1", "Gen 1:1", "Gen.1:1", "Gen.01", "Gen.+1", "Gen.1.", "gen.1", "Bereshit.1",
    ] {
        assert_eq!(reference::verses(nothing), None, "{nothing}");
    }
    // Every verse is found back from its own OSIS — joined ones by either
    // of their numbers.
    for v in 0..corpus::verse_count() {
        let place = reference::verse(v);
        for n in place.verse_number..=place.verse_last {
            let osis = format!("{}.{}.{n}", place.book_osis(), place.chapter_number);
            assert_eq!(reference::verses(&osis), Some(v..v + 1), "{osis}");
        }
    }
}

/// One way of scoping at a time, each naming the verses it says.
#[test]
fn a_scope_is_named_one_way() {
    let torah = reference::book_verses(0).start..reference::book_verses(4).end;
    let some = |r: std::ops::Range<usize>| Ok(Some((r.start, r.end)));
    let to_end = reference::book_verses(25).start..corpus::verse_count();
    assert_eq!(search::scope(None, None, None, None, None), Ok(None));
    assert_eq!(search::scope(Some("torah"), None, None, None, None), some(torah.clone()));
    assert_eq!(search::scope(None, Some("Gen"), None, None, None), some(reference::book_verses(0)));
    assert_eq!(search::scope(None, None, Some("Gen.2"), None, None), some(corpus::verses_of(1)));
    assert_eq!(search::scope(None, None, None, Some("Gen.1.1"), Some("Deut")), some(torah.clone()));
    assert_eq!(search::scope(None, None, None, Some("Mal"), None), some(to_end));
    assert_eq!(search::scope(None, None, None, None, Some("Gen.1.3")), some(0..3));
    assert_eq!(search::scope(Some("torah"), Some("Gen"), None, None, None), Err(Refusal::ScopeConflict));
    assert_eq!(search::scope(None, Some("Gen"), None, Some("Gen.2"), None), Err(Refusal::ScopeConflict));
    assert_eq!(search::scope(None, None, None, Some("Exod"), Some("Gen")), Err(Refusal::EmptyScope));
    for (chapter, why) in [("Gen", "a book"), ("Gen.1.1", "a verse"), ("Gen.60", "outside the book")] {
        assert_eq!(
            search::scope(None, None, Some(chapter), None, None),
            Err(Refusal::UnknownReference { param: "chapter", value: chapter.into() }),
            "{why}"
        );
    }
    let unknown = search::scope(Some("gospels"), None, None, None, None);
    assert!(matches!(unknown, Err(Refusal::UnknownReference { param: "portion", .. })));
}

/// A scoped search keeps every letter of every hit in its verses, finds no
/// more than the whole text, and is refused when it holds no verse.
#[test]
fn a_scoped_search_stays_in_its_verses() {
    let mut engine = Engine::new();
    let whole = search::search(&mut engine, &query("תורה", 49, 49, 0, MAX_LIMIT)).unwrap();
    let mut torah = query("תורה", 49, 49, 0, MAX_LIMIT);
    torah.scope = search::scope(Some("torah"), None, None, None, None).unwrap();
    let (first, end) = torah.scope.unwrap();
    let page = search::search(&mut engine, &torah).unwrap();
    assert!(page.total > 0 && page.total < whole.total);
    assert_eq!(page.hits[0].first_verse().osis(), "Gen.1.1");
    assert!(page.hits.iter().all(|h| h.letters.iter().all(|l| (first..end).contains(&l.verse))));
    for empty in [(5, 5), (9, 3), (0, corpus::verse_count() + 1)] {
        let mut q = query("תורה", 0, 0, 0, 1);
        q.scope = Some(empty);
        assert_eq!(search::search(&mut engine, &q).unwrap_err(), Refusal::EmptyScope, "{empty:?}");
    }
}

/// Past the allowed cost a query is refused before it runs, with its
/// estimate; under it, it is admitted.
#[test]
fn a_query_too_costly_is_refused_before_it_runs() {
    let mut engine = Engine::new();
    let estimate = search::admit(&mut engine, &query("משה", 49, 49, 0, 10), 50.0).unwrap();
    assert!(estimate < 50.0);
    match search::admit(&mut engine, &query("את", 0, u32::MAX, 0, 10), 50.0) {
        Err(Refusal::TooCostly { estimate_ms, limit_ms }) => assert!(estimate_ms > limit_ms && limit_ms == 50.0),
        other => panic!("{other:?}"),
    }
    let mut scoped = query("את", 0, 2000, 0, 10);
    let whole = search::admit(&mut engine, &scoped, f64::MAX).unwrap();
    scoped.scope = search::scope(None, None, Some("Gen.1"), None, None).unwrap();
    let narrowed = search::admit(&mut engine, &scoped, f64::MAX).unwrap();
    assert!(narrowed < whole / 100.0, "the passage does not lighten the estimate: {narrowed} {whole}");
    assert_eq!(engine.runs, 0);
}

// ------------------------------------------------------------------ passage

use super::passage;

/// Every chapter reads back whole: its verses in order, our text as the
/// corpus holds it, its book, portion and number the reference's, opening
/// its book exactly when it is the first, and found again by its OSIS name.
#[test]
fn every_chapter_reads_back_its_verses() {
    for c in 0..corpus::chapter_count() {
        let read = passage::chapter(c).expect("a landed chapter reads");
        let verses: Vec<usize> = read.verses.iter().map(|v| v.place.verse).collect();
        assert_eq!(verses, corpus::verses_of(c).collect::<Vec<_>>(), "chapter {c}: verses out of order");
        for v in &read.verses {
            assert_eq!(v.text, corpus::verse_text(v.place.verse), "chapter {c}: a verse is not ours");
            assert_eq!(v.place.chapter, c);
        }
        assert!(corpus::chapters_of(read.book).contains(&c), "chapter {c} in the wrong book");
        assert!(reference::PORTIONS[read.section].2.contains(&read.book), "chapter {c} in the wrong portion");
        assert_eq!(read.opens, corpus::chapters_of(read.book).start == c, "chapter {c}: opening");
        assert_eq!(read.number as usize, c - corpus::chapters_of(read.book).start + 1, "chapter {c}: number");
        assert_eq!(passage::named(&read.osis()), Ok(c), "{} names another chapter", read.osis());
        assert!(passage::named(&read.human()).is_err(), "{} read though not OSIS", read.human());
    }
}

/// A verse shown with the other reading is ours with the other forms
/// inserted: taking every insertion out gives our verse back byte for byte,
/// and each run said to be theirs is a non-empty stretch of the shown verse
/// on a character boundary, carrying their index mark and never ours.
/// Elsewhere the shown verse is ours, with no run.
#[test]
fn a_shown_verse_is_ours_with_the_other_forms_inserted() {
    let mut carrying = 0;
    for c in 0..corpus::chapter_count() {
        for v in passage::chapter(c).unwrap().verses {
            if v.others.is_empty() {
                assert_eq!(v.shown, v.text, "verse {} shown otherwise with no run", v.place.verse);
                continue;
            }
            carrying += 1;
            let mut back = String::new();
            let mut at = 0;
            for insert in variant::inserts_of(v.place.verse) {
                let (from, len) = insert.range();
                back.push_str(&v.shown[at..from]);
                at = from + len;
            }
            back.push_str(&v.shown[at..]);
            assert_eq!(back, v.text, "verse {}: the insertions out do not give ours back", v.place.verse);
            for run in &v.others {
                assert!(!run.is_empty() && run.end <= v.shown.len(), "verse {}: run {run:?}", v.place.verse);
                assert!(v.shown.is_char_boundary(run.start) && v.shown.is_char_boundary(run.end));
                // Theirs carries its own index and never ours: the mark that
                // numbers our form is ours to paint.
                let theirs = &v.shown[run.clone()];
                assert!(theirs.contains('²') && !theirs.contains('¹'), "verse {}: run {theirs:?}", v.place.verse);
            }
        }
    }
    assert_eq!(carrying, crate::variant::verse_count(), "a verse with another reading was not shown so");
}

/// A reference names a chapter, or the chapter of a verse; a book, a
/// chapter past its book, or nothing at all names none and is refused with
/// its code.
#[test]
fn a_reference_names_its_chapter_or_is_refused() {
    let ps = corpus::chapters_of(26).start;
    assert_eq!(passage::named("Gen.1"), Ok(0));
    assert_eq!(passage::named("Gen.50"), Ok(49));
    assert_eq!(passage::named("Gen.1.3"), Ok(0));
    assert_eq!(passage::named("Ps.119.176"), Ok(ps + 118));
    for bad in ["Gen", "Gen.51", "Gen.0", "", "Nowhere 1", "Gen.1.1.1"] {
        let refused = passage::named(bad).expect_err(bad);
        assert_eq!(refused.code(), "unknown_reference", "{bad}");
    }
}

/// Neighbours run across books and stop at the Tanakh's two ends; a chapter
/// out of range reads nothing.
#[test]
fn neighbours_run_across_books_and_stop_at_the_ends() {
    let last = corpus::chapter_count() - 1;
    assert_eq!(passage::chapter(0).unwrap().previous(), None);
    assert_eq!(passage::chapter(last).unwrap().next(), None);
    assert_eq!(passage::chapter(49).unwrap().next(), Some(50), "Genesis 50 does not lead to Exodus 1");
    assert_eq!(passage::chapter(50).unwrap().previous(), Some(49));
    assert_eq!(passage::chapter(last + 1), None);
    assert_eq!(passage::chapter(usize::MAX), None);
    assert_eq!(passage::read("Gen.51").unwrap_err().code(), "unknown_reference");
}
