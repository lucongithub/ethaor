//! Builds the corpus at compile time.
//!
//! Reads the Mechon-Mamre HTML sources (cp1255), extracts books / chapters /
//! verses from them, cleans up the punctuation and the masoretic markers, then
//! writes two files into OUT_DIR:
//!
//!   text.bin   all the UTF-8 text, concatenated
//!   index.bin  flat array of u32: header + offsets
//!
//! At run time the program parses nothing: it slices those two blocks.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const SRC_DIR: &str = "archive/t002/t";

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let src = Path::new(SRC_DIR);

    println!("cargo:rerun-if-changed={SRC_DIR}");
    println!("cargo:rerun-if-changed=build.rs");

    let mut files: Vec<PathBuf> = fs::read_dir(src)
        .unwrap_or_else(|e| panic!("source introuvable: {}: {e}", src.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            // t01.htm .. t35.htm: one file per book (t0101.htm = chapter, ignored)
            n.len() == 7 && n.starts_with('t') && n.ends_with(".htm") && n[1..3].chars().all(|c| c.is_ascii_digit())
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "aucun fichier source dans {SRC_DIR}");

    let mut corpus = Corpus::default();
    for f in &files {
        let raw = fs::read(f).unwrap();
        parse_book_file(&decode_cp1255(&raw), &mut corpus);
    }

    split_books(&mut corpus);
    reorder(&mut corpus);
    assert_eq!(corpus.books.len(), ORDER.len(), "nombre de livres inattendu");
    write_output(&corpus, &out);
}

// ---------------------------------------------------------------- decoding

/// cp1255 -> UTF-8.
///
/// Mechon-Mamre uses byte 0xCA, unassigned in standard cp1255, for
/// U+05BA HOLAM HASER FOR VAV (e.g. עֲוֺן). Without this fix, 414 signs are lost
/// in the Tanakh.
fn decode_cp1255(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let c = match b {
            0x00..=0x7F => b as char,
            0xA0 => ' ',                                    // NBSP
            0xC0..=0xC9 => char::from_u32(0x05B0 + (b - 0xC0) as u32).unwrap(),
            0xCA => '\u{05BA}',                             // Mechon-Mamre fix
            0xCB..=0xCF => char::from_u32(0x05B0 + (b - 0xC0) as u32).unwrap(),
            0xD0..=0xD8 => char::from_u32(0x05C0 + (b - 0xD0) as u32).unwrap(),
            0xE0..=0xFA => char::from_u32(0x05D0 + (b - 0xE0) as u32).unwrap(),
            _ => ' ',                                       // the rest: irrelevant
        };
        s.push(c);
    }
    s
}

// ------------------------------------------------------------------- model

#[derive(Default)]
struct Corpus {
    books: Vec<Book>,
}

struct Book {
    name: String,
    chapters: Vec<Chapter>,
}

struct Chapter {
    label: String,
    verses: Vec<Verse>,
}

struct Verse {
    label: String,
    text: String,
}

// ------------------------------------------------------- canonical order

/// The 39 books, in order: 5 of the Torah, 21 of the Prophets, 13 of the
/// Writings. That is the order displayed, not the one of the source files.
const ORDER: [&str; 39] = [
    // תורה
    "בראשית", "שמות", "ויקרא", "במדבר", "דברים",
    // נביאים
    "יהושוע", "שופטים", "שמואל א", "שמואל ב", "מלכים א", "מלכים ב", "ישעיהו",
    "ירמיהו", "יחזקאל", "הושע", "יואל", "עמוס", "עובדיה", "יונה", "מיכה",
    "נחום", "חבקוק", "צפניה", "חגיי", "זכריה", "מלאכי",
    // כתובים
    "תהילים", "משלי", "איוב", "שיר השירים", "רות", "איכה", "קוהלת", "אסתר",
    "דנייאל", "עזרא", "נחמיה", "דברי הימים א", "דברי הימים ב",
];

/// Books that the source presents as one piece, with continuous numbering, and
/// that usage separates in two.
///
/// Mechon-Mamre numbers Samuel from 1 to 55, Kings from 1 to 47, Chronicles
/// from 1 to 65 and Ezra/Nehemiah from 1 to 23: no restart at `א` marks the
/// break. So we cut at the traditional rank, and the totals match exactly.
const SPLITS: [(&str, usize, &str, &str); 4] = [
    ("שמואל", 31, "שמואל א", "שמואל ב"),
    ("מלכים", 22, "מלכים א", "מלכים ב"),
    ("דברי הימים", 29, "דברי הימים א", "דברי הימים ב"),
    ("עזרא / נחמיה", 10, "עזרא", "נחמיה"),
];

fn split_books(corpus: &mut Corpus) {
    let mut out: Vec<Book> = Vec::with_capacity(ORDER.len());

    for book in corpus.books.drain(..) {
        let split = SPLITS.iter().find(|(name, ..)| *name == book.name);

        let Some(&(_, at, first, second)) = split else {
            out.push(book);
            continue;
        };

        assert!(at < book.chapters.len(), "coupure hors limites pour {}", book.name);

        let mut chapters = book.chapters;
        let tail = chapters.split_off(at);

        out.push(Book { name: first.to_string(), chapters: renumber(chapters) });
        out.push(Book { name: second.to_string(), chapters: renumber(tail) });
    }

    corpus.books = out;
}

/// Renumbers the chapters from 1: after a cut, the second half would otherwise
/// inherit the numbers of the first.
fn renumber(mut chapters: Vec<Chapter>) -> Vec<Chapter> {
    for (i, chapter) in chapters.iter_mut().enumerate() {
        chapter.label = hebrew_numeral(i + 1);
    }
    chapters
}

/// Number in Hebrew letters. `15` and `16` are written `טו` and `טז`, and not
/// `יה` / `יו`, which would form a divine name.
fn hebrew_numeral(n: usize) -> String {
    const HUNDREDS: [&str; 4] = ["ק", "ר", "ש", "ת"];
    const TENS: [&str; 9] = ["י", "כ", "ל", "מ", "נ", "ס", "ע", "פ", "צ"];
    const UNITS: [&str; 9] = ["א", "ב", "ג", "ד", "ה", "ו", "ז", "ח", "ט"];

    let mut out = String::new();
    let mut rest = n;

    while rest >= 100 {
        let step = (rest / 100).min(4);
        out.push_str(HUNDREDS[step - 1]);
        rest -= step * 100;
    }

    if rest == 15 || rest == 16 {
        out.push_str(UNITS[8]); // ט
        out.push_str(UNITS[rest - 10 - 1]);
        return out;
    }

    if rest >= 10 {
        out.push_str(TENS[rest / 10 - 1]);
        rest %= 10;
    }
    if rest > 0 {
        out.push_str(UNITS[rest - 1]);
    }
    out
}

fn reorder(corpus: &mut Corpus) {
    let mut out: Vec<Book> = Vec::with_capacity(ORDER.len());

    for name in ORDER {
        let found = corpus
            .books
            .iter()
            .position(|book| book.name == name)
            .unwrap_or_else(|| panic!("livre absent de la source : {name}"));
        out.push(corpus.books.remove(found));
    }

    assert!(corpus.books.is_empty(), "livres non classes : {:?}", corpus.books.iter().map(|b| &b.name).collect::<Vec<_>>());
    corpus.books = out;
}

// ----------------------------------------------------------------- parsing

/// A file holds one or more `<H1>`; each one opens a book
/// (t35 = Ezra + Nehemiah). The cover-page `<H1>` of t01 has no verse at all
/// and is therefore dropped.
fn parse_book_file(html: &str, corpus: &mut Corpus) {
    let body = html.split("<BODY>").last().unwrap_or(html);

    for raw_book in body.split("<H1>").skip(1) {
        let Some((name, rest)) = raw_book.split_once("</H1>") else { continue };
        let name = strip_tags(name).trim().to_string();

        // <B>chapter,verse</B> text ... up to the next <B>
        let mut chapters: BTreeMap<String, Chapter> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();

        for seg in rest.split("<B>").skip(1) {
            let Some((label, text)) = seg.split_once("</B>") else { continue };
            let label = strip_tags(label);
            let Some((chap, verse)) = label.split_once(',') else { continue };
            let (chap, verse) = (chap.trim().to_string(), verse.trim().to_string());

            let text = clean(text);
            if text.is_empty() {
                continue;
            }

            if !chapters.contains_key(&chap) {
                order.push(chap.clone());
                chapters.insert(chap.clone(), Chapter { label: chap.clone(), verses: Vec::new() });
            }
            chapters.get_mut(&chap).unwrap().verses.push(Verse { label: verse, text });
        }

        if order.is_empty() {
            continue; // cover page
        }
        // `order` preserves the document order; an alphabetical sort on Hebrew
        // digits would give a wrong order.
        let chapters = order.into_iter().map(|k| chapters.remove(&k).unwrap()).collect();
        corpus.books.push(Book { name, chapters });
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Strips tags, masoretic markers `{פ} {ס} {ר} {ש}` and all punctuation. The
/// maqaf `-` joins two distinct words: it becomes a space, not a deletion.
/// Result: words separated by a single space.
fn clean(s: &str) -> String {
    let no_tags = strip_tags(s);

    let mut out = String::with_capacity(no_tags.len());
    let mut in_brace = false;
    for c in no_tags.chars() {
        match c {
            '{' => in_brace = true,
            '}' => in_brace = false,
            _ if in_brace => {}
            // Hebrew: letters, niqqud, cantillation
            '\u{0590}'..='\u{05F4}' => out.push(c),
            // everything else (, . ; : - -- ( ) [ ] spaces) = word separator
            _ => out.push(' '),
        }
    }

    // normalise the spaces
    let mut res = String::with_capacity(out.len());
    for w in out.split_whitespace() {
        if !res.is_empty() {
            res.push(' ');
        }
        res.push_str(w);
    }
    res
}

// -------------------------------------------------------------------- output

/// Layout of `index.bin`, in u32:
///
///   [0]  n_books
///   [1]  n_chapters
///   [2]  n_verses
///   then section_name (4 entries: three sections and the sentinel)
///   then, in order, six arrays of offsets (each with a trailing sentinel, so
///   `end = next`):
///     book_name    n_books+1       -> bytes in text.bin
///     book_chap    n_books+1       -> chapter indices
///     chap_label   n_chapters+1    -> bytes
///     chap_verse   n_chapters+1    -> verse indices
///     verse_label  n_verses+1      -> bytes
///     verse_text   n_verses+1      -> bytes
///
/// The text is written in homogeneous regions (book names, then chapter
/// labels, then verse labels, then texts), which makes every series of offsets
/// monotonic.
fn write_output(corpus: &Corpus, out: &Path) {
    let n_books = corpus.books.len();
    let n_chapters: usize = corpus.books.iter().map(|b| b.chapters.len()).sum();
    let n_verses: usize = corpus.books.iter().flat_map(|b| &b.chapters).map(|c| c.verses.len()).sum();

    let mut text = String::new();
    let mut book_name = Vec::with_capacity(n_books + 1);
    let mut book_chap = Vec::with_capacity(n_books + 1);
    let mut chap_label = Vec::with_capacity(n_chapters + 1);
    let mut chap_verse = Vec::with_capacity(n_chapters + 1);
    let mut verse_label = Vec::with_capacity(n_verses + 1);
    let mut verse_text = Vec::with_capacity(n_verses + 1);

    // region 0: names of the three sections
    let mut section_name = Vec::with_capacity(4);
    for name in ["תורה", "נביאים", "כתובים"] {
        section_name.push(text.len() as u32);
        text.push_str(name);
    }
    section_name.push(text.len() as u32);

    // region 1: book names
    for b in &corpus.books {
        book_name.push(text.len() as u32);
        text.push_str(&b.name);
    }
    book_name.push(text.len() as u32);

    // region 2: chapter labels
    for b in &corpus.books {
        for c in &b.chapters {
            chap_label.push(text.len() as u32);
            text.push_str(&c.label);
        }
    }
    chap_label.push(text.len() as u32);

    // region 3: verse labels
    for b in &corpus.books {
        for c in &b.chapters {
            for v in &c.verses {
                verse_label.push(text.len() as u32);
                text.push_str(&v.label);
            }
        }
    }
    verse_label.push(text.len() as u32);

    // region 4: verse texts
    for b in &corpus.books {
        for c in &b.chapters {
            for v in &c.verses {
                verse_text.push(text.len() as u32);
                text.push_str(&v.text);
            }
        }
    }
    verse_text.push(text.len() as u32);

    // hierarchical indices
    let (mut ci, mut vi) = (0u32, 0u32);
    for b in &corpus.books {
        book_chap.push(ci);
        for c in &b.chapters {
            chap_verse.push(vi);
            vi += c.verses.len() as u32;
            ci += 1;
        }
    }
    book_chap.push(ci);
    chap_verse.push(vi);

    let mut idx: Vec<u32> = Vec::with_capacity(3 + book_name.len() + book_chap.len() + chap_label.len() + chap_verse.len() + verse_label.len() + verse_text.len());
    idx.extend_from_slice(&[n_books as u32, n_chapters as u32, n_verses as u32]);
    idx.extend_from_slice(&section_name);
    idx.extend_from_slice(&book_name);
    idx.extend_from_slice(&book_chap);
    idx.extend_from_slice(&chap_label);
    idx.extend_from_slice(&chap_verse);
    idx.extend_from_slice(&verse_label);
    idx.extend_from_slice(&verse_text);

    let mut bytes = Vec::with_capacity(idx.len() * 4);
    for v in &idx {
        bytes.extend_from_slice(&v.to_le_bytes());
    }

    fs::write(out.join("text.bin"), text.as_bytes()).unwrap();
    fs::write(out.join("index.bin"), &bytes).unwrap();

    let mut report = String::new();
    let _ = writeln!(
        report,
        "corpus: {n_books} livres, {n_chapters} chapitres, {n_verses} versets, {} Ko de texte, {} Ko d'index",
        text.len() / 1024,
        bytes.len() / 1024
    );
    println!("cargo:warning={}", report.trim());
}
