// The readings where another publication of the edition differs, welded into
// the binary like the corpus itself.
//
// `build/variants.tsv` holds the table, measured word by word against Miqra
// according to the Masorah. This file turns it into what the module draws: for
// each verse concerned, one **display string** — our verse with the other
// reading inserted after ours — and the ranges that say which part is which.
//
// The display string is built here and not at run time for the usual reason:
// the frame loop allocates nothing. It is appended to `text.bin`, so a drawn
// line stays a slice of the one welded string.
//
// This half touches no file, which lets `tests/builder.rs` check it on the very
// code the build runs. Plain comments, not inner doc comments: the file is
// included in the middle of another one.

// The two marks, right after the form they number: ours first, then the other
// reading. They are the index the page explains at the bottom of the screen.
// Both live in the Latin-1 block, which the Hebrew font's own latin subset
// covers.
const MARK_OURS: &str = "\u{00B9}";
const MARK_OTHER: &str = "\u{00B2}";

// The other reading is **bracketed**, which is how both editions print an
// alternative reading themselves — MAM writes `(ketiv) [qere]`. Side by side
// with a space alone, the two forms read as two words of the verse; the
// brackets say they are one place in two versions, which no colour says.
const OPEN: &str = "[";
const CLOSE: &str = "]";

/// One line of the table.
#[derive(Clone, Debug, PartialEq)]
struct Variant {
    book: usize,
    chapter: usize,
    verse: String,
    /// Index of the first word concerned, inside its verse.
    word: usize,
    /// How many of our words the other reading answers — a ketiv and its qere
    /// make two.
    count: usize,
    ours: String,
    other: String,
    kind: String,
}

/// What one difference becomes inside a display string: where the insertion
/// starts, how long the mark on our form is, how long the other reading is.
///
/// The two are kept apart because they are not painted the same: the mark
/// belongs to our form, the rest is the other edition's.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Insert {
    at: usize,
    mark: usize,
    other: usize,
}

/// Reads the table. Blank lines and `#` comments are skipped; anything else
/// must have the eight columns, or the build stops.
fn parse_variants(tsv: &str) -> Vec<Variant> {
    let mut out = Vec::new();
    for (n, line) in tsv.lines().enumerate() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 8, "variants.tsv line {}: {} columns", n + 1, f.len());
        let number = |i: usize| -> usize {
            f[i].trim()
                .parse()
                .unwrap_or_else(|e| panic!("variants.tsv line {} column {}: {e}", n + 1, i + 1))
        };
        out.push(Variant {
            book: number(0),
            chapter: number(1),
            verse: f[2].trim().to_string(),
            word: number(3),
            count: number(4),
            ours: f[5].trim().to_string(),
            other: f[6].trim().to_string(),
            kind: f[7].trim().to_string(),
        });
    }
    assert!(!out.is_empty(), "variants.tsv is empty");
    out
}

/// Byte range of `count` words starting at `word`, inside a verse.
///
/// The words of a verse are separated by one space — `clean` guarantees it — so
/// the span is found by counting spaces, with no allocation and no ambiguity:
/// an index, never a search for a form that may appear twice in the verse.
fn word_span(text: &str, word: usize, count: usize) -> (usize, usize) {
    assert!(count > 0, "an anchor covers at least one word");
    let mut start = None;
    let mut seen = 0usize;
    let mut at = 0usize;
    for (i, part) in text.split(' ').enumerate() {
        if i == word {
            start = Some(at);
        }
        if i >= word {
            seen += 1;
            if seen == count {
                let from = start.unwrap();
                return (from, at + part.len() - from);
            }
        }
        at += part.len() + 1; // the space
    }
    panic!("words {word}..{} outside the verse", word + count);
}

/// Our verse with the other readings inserted, and where each insertion sits.
///
/// Entries must come in the order of the verse: the display string is built in
/// one pass, which is also what makes the ranges monotonic for the module.
fn display_of(text: &str, entries: &[Variant]) -> (String, Vec<Insert>) {
    let mut out = String::with_capacity(text.len() + 64 * entries.len());
    let mut inserts = Vec::with_capacity(entries.len());
    let mut cursor = 0usize;

    for e in entries {
        let (at, len) = word_span(text, e.word, e.count);
        assert_eq!(&text[at..at + len], e.ours, "anchor missing: {} {},{}", e.book, e.chapter, e.verse);
        assert!(at >= cursor, "entries out of order: {} {},{}", e.book, e.chapter, e.verse);

        out.push_str(&text[cursor..at + len]);
        let mark_at = out.len();
        out.push_str(MARK_OURS);
        let mark = MARK_OURS.len();
        let other_at = out.len();
        out.push(' ');
        out.push_str(OPEN);
        out.push_str(&e.other);
        out.push_str(CLOSE);
        out.push_str(MARK_OTHER);
        inserts.push(Insert { at: mark_at, mark, other: out.len() - other_at });
        cursor = at + len;
    }
    out.push_str(&text[cursor..]);
    (out, inserts)
}
