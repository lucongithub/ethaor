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
    println!("cargo:rerun-if-changed=build/mamre.rs");

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

// Reading the sources lives in its own file, included here: that is what lets
// `tests/builder.rs` check it on the very code the build runs.
include!("build/mamre.rs");

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
