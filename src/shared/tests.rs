//! The shared layer: what both transports answer, checked once.

use super::els::{self as search, CAP, Engine, MAX_LIMIT, Query, Refusal, STRIDE};
use super::reference::{self, BOOKS, numeral_value};
use super::words;
use crate::{corpus, els, letter, pack, variant};

fn query(word: &str, skip_min: u32, skip_max: u32, offset: usize, limit: usize) -> Query {
    Query { word: word.into(), exact: false, skip_min, skip_max, offset, limit, scope: None }
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

/// The contents walk the Tanakh whole and in order: the 39 books each once,
/// each under the portion `PORTIONS` puts it in, named as the corpus names
/// it, its chapters running on from the last book's with no gap or overlap
/// to the 929th, each numbered as the corpus numbers it.
#[test]
fn the_contents_walk_every_book_and_chapter_in_order() {
    use super::contents::{books, portion_hebrew};
    let books = books();
    assert_eq!(books.len(), corpus::book_count());
    let mut next = 0;
    for (b, book) in books.iter().enumerate() {
        assert_eq!(book.book, b, "book {b} out of order");
        assert!(reference::PORTIONS[book.section].2.contains(&b), "book {b} under portion {}", book.section);
        assert_eq!(book.hebrew, corpus::book_name(b));
        assert_eq!(book.chapters, corpus::chapters_of(b));
        assert_eq!(book.chapters.start, next, "a gap or an overlap before book {b}");
        assert!(!book.chapters.is_empty(), "book {b} has no chapter");
        next = book.chapters.end;
        let labels: Vec<_> = book.labels().collect();
        assert_eq!(labels.len(), book.chapters.len());
        for (label, c) in labels.iter().zip(book.chapters.clone()) {
            assert_eq!(*label, corpus::chapter_label(c));
        }
    }
    assert_eq!(next, corpus::chapter_count());
    assert_eq!([portion_hebrew(0), portion_hebrew(1), portion_hebrew(2)], ["תורה", "נביאים", "כתובים"]);
}

/// **Exact letters keep the finals the word writes**: folded, `אָדָם` is
/// searched as `אדמ`; exact, as `אדם` — every letter placed is then the form
/// the text writes there, and it finds a part of what folded finds. The
/// estimate and the search read the same stream.
#[test]
fn an_exact_query_keeps_its_finals() {
    let folded = query("אָדָם", 0, 0, 0, 50);
    let exact = Query { exact: true, ..folded.clone() };
    assert_eq!(search::check(&folded).unwrap(), "אדמ");
    assert_eq!(search::check(&exact).unwrap(), "אדם");

    let mut engine = Engine::new();
    search::admit(&mut engine, &exact, f64::INFINITY).unwrap();
    assert!(engine.exact, "admitted on the folded stream");
    let page = search::search(&mut engine, &exact).unwrap();
    assert!(!page.hits.is_empty());
    for hit in &page.hits {
        let word: String = hit.letters.iter().map(|l| l.char).collect();
        assert_eq!(word, "אדם", "a letter placed as another form");
    }
    let all = search::search(&mut engine, &folded).unwrap();
    assert!(!engine.exact, "the folded search ran exact");
    assert!(page.total < all.total, "exact found as many as folded");
}

// ------------------------------------------------------- first and last letters

fn words_query(word: &str, pick: words::Pick, any_order: bool, offset: usize, limit: usize) -> words::Query {
    words::Query { word: word.into(), exact: false, pick, any_order, offset, limit, scope: None }
}

/// A words query is refused for no letter, a letter no word holds — none
/// before the first, none past the longest word — a page too big, or an
/// empty scope; each with a code of its own.
#[test]
fn a_words_query_is_refused_with_its_reason() {
    let mut engine = Engine::new();
    let first = words::Pick::FIRST;
    let mut refused = |q: words::Query| words::search(&mut engine, &q).unwrap_err();
    assert_eq!(refused(words_query("abc", first, false, 0, 10)), Refusal::NoLetter);
    for nth in [0, words::LONGEST_WORD + 1] {
        let pick = words::Pick { nth, from_end: true };
        assert_eq!(refused(words_query("אב", pick, false, 0, 10)), Refusal::LetterRange(nth));
    }
    assert_eq!(refused(words_query("אב", first, false, 0, MAX_LIMIT + 1)), Refusal::Limit(MAX_LIMIT + 1));
    let mut q = words_query("אב", first, false, 0, 10);
    q.scope = Some((10, 10));
    assert_eq!(refused(q), Refusal::EmptyScope);
    let codes: std::collections::HashSet<_> =
        [Refusal::LetterRange(0), Refusal::NoLetter, Refusal::Limit(0), Refusal::EmptyScope, Refusal::SkipRange { min: 1, max: 0 }]
            .iter()
            .map(Refusal::code)
            .collect();
    assert_eq!(codes.len(), 5, "two refusals share a code");
}

/// A page is the engine's hits, in the order of the text, each letter the
/// picked one of its word — first or last — spelling the word in order, or
/// its letters in any; pages tile the listed hits and `limit` 0 counts.
#[test]
fn a_words_page_places_the_engines_hits() {
    let mut engine = Engine::new();
    let mut seen_any = false;
    for (pick, any_order) in [(words::Pick::FIRST, false), (words::Pick::LAST, false), (words::Pick::FIRST, true)] {
        let whole = words::search(&mut engine, &words_query("משה", pick, any_order, 0, MAX_LIMIT)).unwrap();
        assert_eq!(whole.letters, "משה");
        assert!(whole.total > 0 && whole.hits.len() as u64 == whole.total.min(MAX_LIMIT as u64));
        for hit in &whole.hits {
            let mut word: Vec<char> = hit.letters.iter().map(|l| l.char).collect();
            if any_order {
                seen_any |= word != ['מ', 'ש', 'ה'];
                word.sort_unstable();
                assert_eq!(word, ['ה', 'מ', 'ש']);
            } else {
                assert_eq!(word, ['מ', 'ש', 'ה']);
            }
            assert!(hit.letters.windows(2).all(|p| p[0].rank < p[1].rank), "out of reading order");
            for l in &hit.letters {
                assert_eq!(reference::letter(l.rank as usize), (l.verse, l.position as usize));
                // The letter opens — or closes — a word of its verse.
                let text = corpus::verse_text(l.verse);
                let (mut at, mut bounds) = (0usize, Vec::new());
                for word in text.split_whitespace() {
                    let n = letter::letter_count(word);
                    bounds.push(if pick.from_end { at + n - 1 } else { at });
                    at += n;
                }
                assert!(bounds.contains(&(l.position as usize)), "{} not at a word's edge", l.position);
            }
        }
        assert!(whole.hits.windows(2).all(|p| p[0].letters[0].rank < p[1].letters[0].rank));
        let counted = words::search(&mut engine, &words_query("משה", pick, any_order, 0, 0)).unwrap();
        assert_eq!((counted.total, counted.hits.len(), counted.next_offset()), (whole.total, 0, None));
        let (a, b) = (
            words::search(&mut engine, &words_query("משה", pick, any_order, 0, 7)).unwrap(),
            words::search(&mut engine, &words_query("משה", pick, any_order, 7, 7)).unwrap(),
        );
        assert_eq!(a.next_offset(), Some(7));
        assert_eq!([a.hits, b.hits].concat(), whole.hits[..14].to_vec());
    }
    assert!(seen_any, "any order found only the word as written");
}

/// A words search costs a pass over its stretch: the heaviest — the first
/// letters of the whole Tanakh — is admitted under the API's default
/// ceiling, and a ceiling below it refuses it before it runs.
#[test]
fn a_words_search_is_admitted_on_its_cost() {
    let mut engine = Engine::new();
    let q = words_query("א", words::Pick::FIRST, true, 0, 10);
    let estimate = words::admit(&mut engine, &q, 300.0).unwrap();
    assert!(estimate > 0.0 && estimate < 300.0);
    assert!(matches!(words::admit(&mut engine, &q, estimate / 2.0), Err(Refusal::TooCostly { .. })));
    let mut scoped = q.clone();
    scoped.scope = Some((0, 31));
    assert!(words::admit(&mut engine, &scoped, 300.0).unwrap() < estimate / 100.0);
}

/// Every gematria method is named every way, by a key of its own in snake
/// case that names it back; its Hebrew name is Hebrew, its English name and
/// description are there.
#[test]
fn every_gematria_method_is_named_every_way() {
    use super::gematria::{Method, info, method};
    let mut keys = std::collections::HashSet::new();
    for m in Method::ALL {
        let named = info(m);
        assert!(keys.insert(named.key), "{} names two methods", named.key);
        assert!(named.key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'), "{} is not snake case", named.key);
        assert_eq!(method(named.key), Some(m), "{} does not name its method back", named.key);
        assert!(!named.name.is_empty() && !named.description.is_empty(), "{} is unnamed or undescribed", named.key);
        assert!(named.hebrew.chars().any(|c| letter::exact_code(c).is_some()), "{}: no Hebrew name", named.key);
        assert!(named.hebrew.chars().all(|c| !c.is_ascii_alphabetic()), "{}: Latin in the Hebrew name", named.key);
    }
    assert_eq!(info(Method::MisparHapanim).key, "mispar_hapanim");
    assert_eq!(info(Method::MisparHapanim).hebrew, "מספר הפנים");
    for other in ["Mispar_Hapanim", "standard", "", "mispar_hapanim "] {
        assert_eq!(method(other), None, "{other:?} names a method");
    }
}

/// The methods asked for: all without a list; a list parted by commas
/// answered in the catalogue's order, each once; a key it does not hold —
/// an empty one too — refused with the key and the list of good ones.
#[test]
fn gematria_methods_are_asked_by_key() {
    use super::gematria::{Method, methods};
    assert_eq!(methods(None), Ok(Method::ALL.to_vec()));
    assert_eq!(methods(Some("atbash,mispar_hapanim,atbash")), Ok(vec![Method::MisparHapanim, Method::Atbash]));
    for bad in ["", "atbash,", "AtBash", "atbash,nope", "atbash, albam"] {
        let refused = methods(Some(bad)).unwrap_err();
        assert_eq!(refused.code(), "unknown_method", "{bad:?}");
        assert!(refused.message().contains("mispar_hapanim"), "{bad:?}: the message lists no key");
    }
}

/// A text is read by the methods asked, the consonants it was read as
/// beside them; one with no Hebrew letter, or past the bound, is refused
/// with its own code.
#[test]
fn a_gematria_text_is_read_or_refused() {
    use super::gematria::{MAX_LETTERS, Method, read};
    let reading = read("תּוֹרָה, abc", &[Method::MisparHapanim, Method::MisparKolel]).unwrap();
    assert_eq!(reading.letters, "תורה");
    assert_eq!((reading.letter_count, reading.words), (4, 1));
    assert_eq!(reading.values, vec![(Method::MisparHapanim, 611), (Method::MisparKolel, 612)]);
    assert_eq!(read("שָׁלוֹם, abc", &[]).unwrap().letters, "שלום", "the letters read are not the text's consonants");
    for empty in ["", "abc", "\u{05B8}\u{0591}"] {
        assert_eq!(read(empty, &Method::ALL).unwrap_err(), Refusal::NoLetter, "{empty:?}");
    }
    let longest: String = std::iter::repeat_n('א', MAX_LETTERS).collect();
    assert!(read(&longest, &Method::ALL).is_ok(), "the bound itself is refused");
    let over = longest + "א";
    assert_eq!(read(&over, &Method::ALL).unwrap_err(), Refusal::TooManyLetters);
    let codes: std::collections::HashSet<_> =
        [Refusal::NoLetter, Refusal::TooManyLetters, Refusal::UnknownMethod(String::new()), Refusal::LetterRange(0), Refusal::Limit(0)]
            .iter()
            .map(Refusal::code)
            .collect();
    assert_eq!(codes.len(), 5, "two refusals share a code");
}

fn gematria_query(text: &str, method: super::gematria::Method, offset: usize, limit: usize) -> super::gematria::Query {
    super::gematria::Query { text: text.into(), method, offset, limit, scope: None }
}

/// A gematria search's page is the stream's runs, in the order of the
/// text: pages tile the listed runs, `limit` 0 counts, every run's words
/// are the verse's own and worth the text's value, the first words of
/// Genesis among those of `בראשית ברא`.
#[test]
fn a_gematria_page_lists_the_runs_worth_the_text() {
    use super::gematria::{Method, search};
    let full = search(&gematria_query("בראשית ברא", Method::MisparHapanim, 0, MAX_LIMIT)).unwrap();
    assert_eq!(full.value, 913 + 203);
    assert!(full.total > 0 && full.listed as u64 == full.total.min(CAP as u64));
    assert_eq!(full.hits[0].place().verse, 0, "Genesis 1:1 not first");
    assert_eq!(crate::gematria::letters(full.hits[0].words()).collect::<String>(), "בראשיתברא");
    assert_eq!(full.hits[0].words().split(' ').count(), 2, "not the two words, whole");
    for hit in &full.hits {
        assert_eq!(crate::gematria::tally(hit.words()).value(Method::MisparHapanim), full.value, "{}: not worth the text", hit.words());
        assert!(corpus::verse_text(hit.verse).contains(hit.words()), "{}: not its verse's words", hit.words());
    }
    let mut walked = Vec::new();
    let mut offset = 0;
    loop {
        let page = search(&gematria_query("בראשית ברא", Method::MisparHapanim, offset, 7)).unwrap();
        walked.extend(page.hits.clone());
        match page.next_offset() {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(walked, full.hits, "pages do not tile the runs");
    let counted = search(&gematria_query("בראשית ברא", Method::MisparHapanim, 0, 0)).unwrap();
    assert_eq!((counted.total, counted.hits.len()), (full.total, 0));
}

/// A gematria search is refused with its reason before it runs: no Hebrew
/// letter, a limit past the most, an empty scope, a cost past the ceiling;
/// a scope keeps its runs inside it.
#[test]
fn a_gematria_search_is_refused_or_kept_to_its_scope() {
    use super::gematria::{Method, admit, check_search, search};
    let refused = |q: super::gematria::Query| check_search(&q).unwrap_err();
    assert_eq!(refused(gematria_query("abc", Method::Atbash, 0, 10)), Refusal::NoLetter);
    assert_eq!(refused(gematria_query("א", Method::Atbash, 0, MAX_LIMIT + 1)), Refusal::Limit(MAX_LIMIT + 1));
    let mut q = gematria_query("א", Method::Atbash, 0, 10);
    q.scope = Some((10, 10));
    assert_eq!(refused(q.clone()), Refusal::EmptyScope);
    q.scope = Some((1533, 4000));
    let page = search(&q).unwrap();
    assert!(page.total > 0);
    assert!(page.hits.iter().all(|h| (1533..4000).contains(&h.verse)), "a run outside its scope");
    let whole = gematria_query("א", Method::Atbash, 0, 10);
    let estimate = admit(&whole, f64::MAX).unwrap();
    assert!(estimate > 0.0);
    assert!(admit(&q, f64::MAX).unwrap() < estimate, "a scope costs as much as the whole");
    assert!(matches!(admit(&whole, estimate / 2.0), Err(Refusal::TooCostly { .. })));
}

