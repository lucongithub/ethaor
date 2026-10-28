//! Builds the corpus at compile time.
//!
//! Reads the Mechon-Mamre HTML sources (cp1255), extracts books / chapters /
//! verses from them, cleans up the punctuation and the masoretic markers, then
//! writes two files into OUT_DIR:
//!
//!   text.bin     all the UTF-8 text, concatenated
//!   index.bin    flat array of u32: header + offsets
//!   map.pack     the corpus' map, packed — what the module embeds
//!   text.plain   the corpus' text, packed: what the page fetches, a book at a
//!                time (gzipped) and a chapter at a time (tiles)
//!   text.books   where each book lies in it, as u32: what cuts it into books
//!
//! The first three are the working form, the reference the packed form is
//! checked against; `src/pack.rs` packs here and unpacks in the module.
//!
//! At run time the program parses nothing: it slices those two blocks.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const SRC_DIR: &str = "archive/t002/t";
const VARIANTS: &str = "build/variants.tsv";

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let src = Path::new(SRC_DIR);

    println!("cargo:rerun-if-changed={SRC_DIR}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/mamre.rs");
    println!("cargo:rerun-if-changed=build/variants.rs");
    println!("cargo:rerun-if-changed=src/pack.rs");
    println!("cargo:rerun-if-changed={VARIANTS}");

    let mut files: Vec<PathBuf> = fs::read_dir(src)
        .unwrap_or_else(|e| panic!("source not found: {}: {e}", src.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            // t01.htm .. t35.htm: one file per book (t0101.htm = chapter, ignored)
            n.len() == 7 && n.starts_with('t') && n.ends_with(".htm") && n[1..3].chars().all(|c| c.is_ascii_digit())
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no source file in {SRC_DIR}");

    let mut corpus = Corpus::default();
    for f in &files {
        let raw = fs::read(f).unwrap();
        parse_book_file(&decode_cp1255(&raw), &mut corpus);
    }

    split_books(&mut corpus);
    reorder(&mut corpus);
    assert_eq!(corpus.books.len(), ORDER.len(), "unexpected number of books");
    write_output(&corpus, &out);
}

// Reading the sources lives in its own file, included here: that is what lets
// `tests/builder.rs` check it on the very code the build runs.
include!("build/mamre.rs");
include!("build/variants.rs");

#[path = "src/pack.rs"]
mod pack;

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

    // The consonants of every verse, as the ELS stream will hold them: known
    // before the text lands, so a skip can be bounded before any search.
    let letter_count: usize = corpus
        .books
        .iter()
        .flat_map(|b| &b.chapters)
        .flat_map(|c| &c.verses)
        .map(|v| v.text.chars().filter(|c| ('\u{05D0}'..='\u{05EA}').contains(c)).count())
        .sum();
    // The consonants of the longest word: how many letters of each word a
    // first- or last-letters search can read, known before the text lands.
    let longest_word: usize = corpus
        .books
        .iter()
        .flat_map(|b| &b.chapters)
        .flat_map(|c| &c.verses)
        .flat_map(|v| v.text.split_whitespace())
        .map(|w| w.chars().filter(|c| ('\u{05D0}'..='\u{05EA}').contains(c)).count())
        .max()
        .unwrap_or(0);
    fs::write(
        out.join("letters.rs"),
        format!(
            "/// Consonants in the Tanakh, counted by `build.rs`: the ELS stream's length.\npub const LETTER_COUNT: u32 = {letter_count};\n\
             /// Consonants of the Tanakh's longest word, counted by `build.rs`.\npub const LONGEST_WORD: u32 = {longest_word};\n"
        ),
    )
    .unwrap();

    // region 5: display strings — our verse with the other edition's reading
    // inserted. Only the verses of `variants.tsv` have one, so the region is a
    // tail of a hundred strings, and a drawn line stays a slice of `text.bin`.
    let (variants, report_variants) = weld_variants(corpus, &mut text);

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

    let mut variant_bytes = Vec::with_capacity(variants.len() * 4);
    for v in &variants {
        variant_bytes.extend_from_slice(&v.to_le_bytes());
    }

    fs::write(out.join("text.bin"), text.as_bytes()).unwrap();
    fs::write(out.join("index.bin"), &bytes).unwrap();
    fs::write(out.join("variants.bin"), &variant_bytes).unwrap();

    // The transport form: `pack` asserts that it unpacks to the above.
    let packed = pack::pack(&pack::Blobs { text: text.clone().into_bytes(), index: idx.clone(), variants: variants.clone() });
    fs::write(out.join("map.pack"), &packed.map).unwrap();
    fs::write(out.join("text.plain"), &packed.text).unwrap();
    let books: Vec<u8> = packed.books.iter().flat_map(|o| o.to_le_bytes()).collect();
    fs::write(out.join("text.books"), &books).unwrap();

    let mut report = String::new();
    let _ = writeln!(
        report,
        "corpus: {n_books} books, {n_chapters} chapters, {n_verses} verses, {} KB of text, {} KB of index, {report_variants}; pack {} KB of map + {} KB of text",
        text.len() / 1024,
        bytes.len() / 1024,
        packed.map.len() / 1024,
        packed.text.len() / 1024
    );
    println!("cargo:warning={}", report.trim());
}

/// Appends the display strings to the text and returns `variants.bin`.
///
/// Layout, in u32:
///
///   [0]  n_display   verses carrying a display string
///   [1]  n_variant   differences, all verses together
///   then n_display  x 3: verse, offset in text.bin, length
///   then n_variant  x 4: verse, start of the insertion inside the display
///                        string, length of the mark on our form, length of the
///                        other reading
///
/// Both series are sorted by verse, and the insertions of one verse come in the
/// order of the text: the module looks an entry up by dichotomy and reads the
/// ranges in one pass.
fn weld_variants(corpus: &Corpus, text: &mut String) -> (Vec<u32>, String) {
    let specs = parse_variants(
        &fs::read_to_string(VARIANTS).unwrap_or_else(|e| panic!("{VARIANTS} illisible : {e}")),
    );

    // Indexed by place, so the walk below costs one lookup per verse instead of
    // a scan of the whole table.
    let mut wanted: BTreeMap<(usize, usize, String), Vec<Variant>> = BTreeMap::new();
    for s in &specs {
        wanted
            .entry((s.book, s.chapter, s.verse.clone()))
            .or_default()
            .push(s.clone());
    }

    let mut display: Vec<u32> = Vec::new();
    let mut inserts: Vec<u32> = Vec::new();
    let mut matched = 0usize;
    let mut verse = 0u32;

    for (bi, b) in corpus.books.iter().enumerate() {
        for (ci, c) in b.chapters.iter().enumerate() {
            for v in &c.verses {
                if let Some(entries) = wanted.get(&(bi, ci + 1, v.label.clone())) {
                    let (shown, spans) = display_of(&v.text, entries);
                    display.extend_from_slice(&[verse, text.len() as u32, shown.len() as u32]);
                    text.push_str(&shown);
                    for s in &spans {
                        inserts.extend_from_slice(&[verse, s.at as u32, s.mark as u32, s.other as u32]);
                    }
                    matched += entries.len();
                }
                verse += 1;
            }
        }
    }

    assert_eq!(
        matched,
        specs.len(),
        "{} entries of {VARIANTS} fall on no verse",
        specs.len() - matched
    );

    let mut out = vec![(display.len() / 3) as u32, (inserts.len() / 4) as u32];
    out.extend_from_slice(&display);
    out.extend_from_slice(&inserts);

    let report = format!("{} differences over {} verses", inserts.len() / 4, display.len() / 3);
    (out, report)
}
