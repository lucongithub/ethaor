//! Places of the Tanakh in the words a reader — or an agent — uses: a book's
//! English name and OSIS code, chapter and verse as numbers.
//!
//! The corpus speaks global indices and Hebrew numerals; this is the one
//! place they become `Gen.1.1`. Numbers are read off the corpus' own labels,
//! never recounted, so a joined verse (`א-ב`) keeps both of its numbers.

use std::ops::Range;
use std::sync::OnceLock;

use crate::{corpus, els};

/// A book's names outside Hebrew.
pub struct Book {
    /// OSIS code, the key Bible software and datasets share.
    pub osis: &'static str,
    pub english: &'static str,
}

/// In the corpus' order — `ORDER` in `build/mamre.rs`, the Tanakh's.
pub const BOOKS: [Book; 39] = [
    Book { osis: "Gen", english: "Genesis" },
    Book { osis: "Exod", english: "Exodus" },
    Book { osis: "Lev", english: "Leviticus" },
    Book { osis: "Num", english: "Numbers" },
    Book { osis: "Deut", english: "Deuteronomy" },
    Book { osis: "Josh", english: "Joshua" },
    Book { osis: "Judg", english: "Judges" },
    Book { osis: "1Sam", english: "1 Samuel" },
    Book { osis: "2Sam", english: "2 Samuel" },
    Book { osis: "1Kgs", english: "1 Kings" },
    Book { osis: "2Kgs", english: "2 Kings" },
    Book { osis: "Isa", english: "Isaiah" },
    Book { osis: "Jer", english: "Jeremiah" },
    Book { osis: "Ezek", english: "Ezekiel" },
    Book { osis: "Hos", english: "Hosea" },
    Book { osis: "Joel", english: "Joel" },
    Book { osis: "Amos", english: "Amos" },
    Book { osis: "Obad", english: "Obadiah" },
    Book { osis: "Jonah", english: "Jonah" },
    Book { osis: "Mic", english: "Micah" },
    Book { osis: "Nah", english: "Nahum" },
    Book { osis: "Hab", english: "Habakkuk" },
    Book { osis: "Zeph", english: "Zephaniah" },
    Book { osis: "Hag", english: "Haggai" },
    Book { osis: "Zech", english: "Zechariah" },
    Book { osis: "Mal", english: "Malachi" },
    Book { osis: "Ps", english: "Psalms" },
    Book { osis: "Prov", english: "Proverbs" },
    Book { osis: "Job", english: "Job" },
    Book { osis: "Song", english: "Song of Songs" },
    Book { osis: "Ruth", english: "Ruth" },
    Book { osis: "Lam", english: "Lamentations" },
    Book { osis: "Eccl", english: "Ecclesiastes" },
    Book { osis: "Esth", english: "Esther" },
    Book { osis: "Dan", english: "Daniel" },
    Book { osis: "Ezra", english: "Ezra" },
    Book { osis: "Neh", english: "Nehemiah" },
    Book { osis: "1Chr", english: "1 Chronicles" },
    Book { osis: "2Chr", english: "2 Chronicles" },
];

/// A verse, every way a caller may name it.
#[derive(Debug, Clone, PartialEq)]
pub struct Verse {
    /// Global indices, the corpus' own: book 0..39, chapter 0..929, verse.
    pub book: usize,
    pub chapter: usize,
    pub verse: usize,
    /// Chapter number inside its book, from 1.
    pub chapter_number: u32,
    /// Verse number inside its chapter, from 1; `verse_last` differs only
    /// where the source joins two verses under one label.
    pub verse_number: u32,
    pub verse_last: u32,
}

impl Verse {
    pub fn book_osis(&self) -> &'static str {
        BOOKS[self.book].osis
    }

    pub fn book_english(&self) -> &'static str {
        BOOKS[self.book].english
    }

    pub fn book_hebrew(&self) -> &'static str {
        corpus::book_name(self.book)
    }

    /// `Gen.1.1`, or `Gen.1.1-Gen.1.2` for a joined verse: OSIS, which
    /// tools parse without guessing.
    pub fn osis(&self) -> String {
        let (b, c) = (self.book_osis(), self.chapter_number);
        if self.verse_last == self.verse_number {
            format!("{b}.{c}.{}", self.verse_number)
        } else {
            format!("{b}.{c}.{}-{b}.{c}.{}", self.verse_number, self.verse_last)
        }
    }

    /// `Genesis 1:1`, or `Genesis 1:1-2`: what a reader writes.
    pub fn human(&self) -> String {
        let (b, c) = (self.book_english(), self.chapter_number);
        if self.verse_last == self.verse_number {
            format!("{b} {c}:{}", self.verse_number)
        } else {
            format!("{b} {c}:{}-{}", self.verse_number, self.verse_last)
        }
    }

    /// `בראשית א,א`: the corpus' own labels, as the map shows them.
    pub fn hebrew(&self) -> String {
        format!(
            "{} {},{}",
            self.book_hebrew(),
            corpus::chapter_label(self.chapter),
            corpus::verse_label(self.verse)
        )
    }
}

/// Value of a Hebrew numeral — `טו` 15, `קנ` 150. `None` if a character is
/// not a numeral letter.
pub fn numeral_value(label: &str) -> Option<u32> {
    const LETTERS: &str = "אבגדהוזחטיכלמנסעפצקרשת";
    const VALUES: [u32; 22] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 200, 300, 400];
    if label.is_empty() {
        return None;
    }
    label.chars().try_fold(0, |sum, c| Some(sum + VALUES[LETTERS.chars().position(|l| l == c)?]))
}

/// Chapter of each verse and book of each chapter, built once.
struct Tables {
    chapter_of_verse: Vec<u16>,
    book_of_chapter: Vec<u8>,
}

static TABLES: OnceLock<Tables> = OnceLock::new();

fn tables() -> &'static Tables {
    TABLES.get_or_init(|| {
        let mut chapter_of_verse = vec![0u16; corpus::verse_count()];
        for c in 0..corpus::chapter_count() {
            for v in corpus::verses_of(c) {
                chapter_of_verse[v] = c as u16;
            }
        }
        let mut book_of_chapter = vec![0u8; corpus::chapter_count()];
        for b in 0..corpus::book_count() {
            for c in corpus::chapters_of(b) {
                book_of_chapter[c] = b as u8;
            }
        }
        Tables { chapter_of_verse, book_of_chapter }
    })
}

/// A verse by its global index.
pub fn verse(verse: usize) -> Verse {
    let t = tables();
    let chapter = t.chapter_of_verse[verse] as usize;
    let label = corpus::verse_label(verse);
    let (first, last) = label.split_once('-').unwrap_or((label, label));
    // Every label of the corpus is a numeral: `the_numbers_are_the_labels`.
    let number = |l: &str| numeral_value(l).unwrap_or(0);
    Verse {
        book: t.book_of_chapter[chapter] as usize,
        chapter,
        verse,
        chapter_number: number(corpus::chapter_label(chapter)),
        verse_number: number(first),
        verse_last: number(last),
    }
}

/// A letter of the ELS stream: the verse it lies in, and its rank among
/// that verse's consonants, from 0.
pub fn letter(rank: usize) -> (usize, usize) {
    let starts = els::verse_starts();
    let verse = starts.partition_point(|&s| s as usize <= rank) - 1;
    (verse, rank - starts[verse] as usize)
}

/// Books, chapters and verses of the corpus.
pub fn counts() -> [usize; 3] {
    [corpus::book_count(), corpus::chapter_count(), corpus::verse_count()]
}

/// The three portions: a key, the English name, and their books.
pub const PORTIONS: [(&str, &str, Range<usize>); 3] =
    [("torah", "Torah", 0..5), ("neviim", "Nevi'im", 5..26), ("ketuvim", "Ketuvim", 26..39)];

/// A portion by any of its usual names, in any case: `torah`, `nevi'im`,
/// `prophets`, `כתובים`… Its books, as a range.
pub fn portion(name: &str) -> Option<Range<usize>> {
    let name = name.trim().to_lowercase().replace(['\'', '’', '-', ' '], "");
    let k = match name.as_str() {
        "torah" | "law" | "pentateuch" | "תורה" => 0,
        "neviim" | "nevim" | "prophets" | "נביאים" => 1,
        "ketuvim" | "kethuvim" | "writings" | "כתובים" => 2,
        _ => return None,
    };
    Some(PORTIONS[k].2.clone())
}

/// A book by its OSIS code or English name in any case (`gen`, `1 Samuel`,
/// `song of songs`), or its Hebrew name as the corpus writes it.
pub fn book(name: &str) -> Option<usize> {
    let name = name.trim();
    let folded = |s: &str| s.to_lowercase().replace(' ', "");
    let wanted = folded(name);
    BOOKS
        .iter()
        .position(|b| folded(b.osis) == wanted || folded(b.english) == wanted)
        .or_else(|| (0..corpus::book_count()).find(|&b| corpus::book_name(b) == name))
}

/// The verses of a book, as global indices.
pub fn book_verses(book: usize) -> Range<usize> {
    let chapters = corpus::chapters_of(book);
    corpus::verses_of(chapters.start).start..corpus::verses_of(chapters.end - 1).end
}

/// The verses a reference names, as global indices: a book (`Gen`,
/// `Genesis`), a chapter (`Gen.1`, `Genesis 1`) or a verse (`Gen.1.1`,
/// `Genesis 1:1`). A joined verse is named by either of its numbers.
pub fn verses(reference: &str) -> Option<Range<usize>> {
    let reference = reference.trim();
    // The longest prefix that names a book, cut at a dot or a space.
    let cuts = reference.char_indices().filter(|&(_, c)| c == '.' || c == ' ').map(|(i, _)| i);
    let (b, rest) = cuts
        .chain(std::iter::once(reference.len()))
        .rev()
        .find_map(|i| book(&reference[..i]).map(|b| (b, reference[i..].trim_start_matches(['.', ' ']))))?;
    if rest.is_empty() {
        return Some(book_verses(b));
    }
    let mut numbers = rest.split(['.', ':']).map(|n| n.trim().parse::<u32>());
    let chapter_number = numbers.next()?.ok()?;
    let chapter = corpus::chapters_of(b).nth((chapter_number as usize).checked_sub(1)?)?;
    let Some(verse_number) = numbers.next() else {
        return Some(corpus::verses_of(chapter));
    };
    let verse_number = verse_number.ok()?;
    if numbers.next().is_some() {
        return None;
    }
    let v = corpus::verses_of(chapter).find(|&v| {
        let place = verse(v);
        place.verse_number <= verse_number && verse_number <= place.verse_last
    })?;
    Some(v..v + 1)
}
