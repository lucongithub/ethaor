//! The corpus in transport: only what cannot be recomputed.
//!
//! `build.rs` produces the corpus in its working form — `text.bin`,
//! `index.bin`, `variants.bin` — and `pack` reduces it to its information:
//!
//! - every string in one byte per character, through an alphabet of the few
//!   dozen characters the corpus uses (UTF-8 spends two per Hebrew character);
//! - no offset at all: the index is the strings' lengths laid end to end,
//!   which the strings themselves give back;
//! - chapter and verse labels as a rule — the Hebrew numeral of the rank —
//!   and the few that break it;
//! - the other edition's readings as their insertions alone, not as whole
//!   rewritten verses.
//!
//! `unpack` rebuilds the working form **byte for byte**, which is what lets
//! every other module go on reading it unchanged; the test suite holds the
//! two ends together on the real corpus.
//!
//! It comes in two parts. The **map** — names, counts, labels — is all the
//! overview draws; the **text** — verses and readings — is the bulk, and only
//! the reading level and the search need it.
//!
//! This file is compiled twice: into the module, which unpacks, and into
//! `build.rs` (`#[path]`), which packs. It therefore uses `std` only.

#![allow(dead_code)]

#[path = "pack/codec.rs"]
pub mod codec;

// ------------------------------------------------------------------ varints

fn put(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0 }
    }

    fn get(&mut self) -> u32 {
        let mut v = 0u32;
        let mut shift = 0;
        loop {
            let byte = self.bytes[self.at];
            self.at += 1;
            v |= ((byte & 0x7F) as u32) << shift;
            if byte & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }

    fn byte(&mut self) -> u8 {
        let b = self.bytes[self.at];
        self.at += 1;
        b
    }
}

// ----------------------------------------------------------------- alphabet

/// The characters of the corpus, each with its code and its UTF-8 bytes.
pub struct Alphabet {
    chars: Vec<char>,
    /// UTF-8 of each code, at most four bytes, and its length.
    utf8: Vec<([u8; 4], u8)>,
}

impl Alphabet {
    fn new(chars: Vec<char>) -> Alphabet {
        assert!(chars.len() <= 256, "trop de caracteres pour un octet");
        let utf8 = chars
            .iter()
            .map(|c| {
                let mut buf = [0u8; 4];
                let len = c.encode_utf8(&mut buf).len() as u8;
                (buf, len)
            })
            .collect();
        Alphabet { chars, utf8 }
    }

    fn code(&self, c: char) -> u8 {
        self.chars.binary_search(&c).expect("caractere hors alphabet") as u8
    }

    /// Appends the UTF-8 of `code` to `out`.
    #[inline]
    fn push(&self, out: &mut Vec<u8>, code: u8) {
        let (buf, len) = &self.utf8[code as usize];
        out.extend_from_slice(&buf[..*len as usize]);
    }

    fn write(&self, out: &mut Vec<u8>) {
        put(out, self.chars.len() as u32);
        for &c in &self.chars {
            put(out, c as u32);
        }
    }

    fn read(r: &mut Reader) -> Alphabet {
        let n = r.get() as usize;
        Alphabet::new((0..n).map(|_| char::from_u32(r.get()).expect("code invalide")).collect())
    }

    /// A string: its length in characters, then one code per character.
    fn put_str(&self, out: &mut Vec<u8>, s: &str) {
        put(out, s.chars().count() as u32);
        out.extend(s.chars().map(|c| self.code(c)));
    }

    fn get_str(&self, r: &mut Reader, out: &mut Vec<u8>) {
        let n = r.get() as usize;
        for _ in 0..n {
            let code = r.byte();
            self.push(out, code);
        }
    }

    fn get_string(&self, r: &mut Reader) -> String {
        let mut out = Vec::new();
        self.get_str(r, &mut out);
        String::from_utf8(out).expect("UTF-8 invalide")
    }
}

// ------------------------------------------------------------------- labels

/// Number in Hebrew letters. `15` and `16` are written `טו` and `טז`, and not
/// `יה` / `יו`, which would form a divine name. The very rule of
/// `build/mamre.rs`, which names the chapters it renumbers.
pub fn hebrew_numeral(n: usize) -> String {
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
        out.push_str(UNITS[8]);
        out.push_str(UNITS[rest - 10]);
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

/// Labels of consecutive items — chapters of a book, verses of a chapter —
/// as the numeral of a counter, except where one breaks the rule: the source
/// joins two verses under one label (`א-ב`), and the count resumes further on.
///
/// An exception is (rank in the whole series, label, next value of the
/// counter). It is written for every label the rule does not give, whatever
/// the reason, so the rule never has to be right — only usually right.
fn label_exceptions(groups: &[Vec<&str>]) -> Vec<(usize, String, usize)> {
    let mut out = Vec::new();
    let mut rank = 0;
    for labels in groups {
        let mut counter = 1;
        for (k, label) in labels.iter().enumerate() {
            if *label == hebrew_numeral(counter) {
                counter += 1;
            } else {
                // Resume at the value the next label carries, if it is a
                // numeral within reach; otherwise simply one further.
                let next = labels
                    .get(k + 1)
                    .and_then(|n| (1..=1000).find(|&c| hebrew_numeral(c) == *n))
                    .unwrap_or(counter + 1);
                out.push((rank, label.to_string(), next));
                counter = next;
            }
            rank += 1;
        }
    }
    out
}

fn write_exceptions(out: &mut Vec<u8>, abc: &Alphabet, list: &[(usize, String, usize)]) {
    put(out, list.len() as u32);
    let mut last = 0;
    for (rank, label, next) in list {
        put(out, (rank - last) as u32);
        last = *rank;
        abc.put_str(out, label);
        put(out, *next as u32);
    }
}

fn read_exceptions(r: &mut Reader, abc: &Alphabet) -> Vec<(usize, String, usize)> {
    let n = r.get() as usize;
    let mut last = 0;
    (0..n)
        .map(|_| {
            last += r.get() as usize;
            let label = abc.get_string(r);
            (last, label, r.get() as usize)
        })
        .collect()
}

/// Lays the labels of consecutive groups end to end into `text`, pushing each
/// one's start into `offsets`.
fn lay_labels(
    sizes: impl Iterator<Item = usize>,
    exceptions: &[(usize, String, usize)],
    text: &mut Vec<u8>,
    offsets: &mut Vec<u32>,
) {
    let mut next = exceptions.iter().peekable();
    let mut rank = 0;
    for size in sizes {
        let mut counter = 1;
        for _ in 0..size {
            offsets.push(text.len() as u32);
            match next.peek() {
                Some((r, label, after)) if *r == rank => {
                    text.extend_from_slice(label.as_bytes());
                    counter = *after;
                    next.next();
                }
                _ => {
                    text.extend_from_slice(hebrew_numeral(counter).as_bytes());
                    counter += 1;
                }
            }
            rank += 1;
        }
    }
}

// --------------------------------------------------------------------- form

/// The working form of the corpus, as `build.rs` writes it and `corpus`,
/// `variant` read it. See `write_output` in `build.rs` for the layouts.
#[derive(PartialEq, Debug, Default)]
pub struct Blobs {
    pub text: Vec<u8>,
    pub index: Vec<u32>,
    pub variants: Vec<u32>,
}

/// The two parts of the transport form, and the text part before the codec.
pub struct Packed {
    pub map: Vec<u8>,
    pub text: Vec<u8>,
    pub plain: Vec<u8>,
}

/// Reduces the working form to its information. Asserts, rather than
/// trusts, that `unpack` gives it back.
pub fn pack(blobs: &Blobs) -> Packed {
    let text = std::str::from_utf8(&blobs.text).expect("texte non UTF-8");
    let idx = &blobs.index;
    let (nb, nc, nv) = (idx[0] as usize, idx[1] as usize, idx[2] as usize);
    let rest = &idx[3..];
    let (section_name, rest) = rest.split_at(4);
    let (book_name, rest) = rest.split_at(nb + 1);
    let (book_chap, rest) = rest.split_at(nb + 1);
    let (chap_label, rest) = rest.split_at(nc + 1);
    let (chap_verse, rest) = rest.split_at(nc + 1);
    let (verse_label, rest) = rest.split_at(nv + 1);
    let (verse_text, _) = rest.split_at(nv + 1);
    let slice = |offsets: &[u32], i: usize| &text[offsets[i] as usize..offsets[i + 1] as usize];

    let mut chars: Vec<char> = text.chars().collect();
    chars.sort_unstable();
    chars.dedup();
    let abc = Alphabet::new(chars);

    // ------------------------------------------------------------- the map
    let mut map = Vec::new();
    abc.write(&mut map);
    for s in 0..3 {
        abc.put_str(&mut map, slice(section_name, s));
    }
    put(&mut map, nb as u32);
    for b in 0..nb {
        abc.put_str(&mut map, slice(book_name, b));
        put(&mut map, book_chap[b + 1] - book_chap[b]);
    }
    for c in 0..nc {
        put(&mut map, chap_verse[c + 1] - chap_verse[c]);
    }
    let chapters: Vec<Vec<&str>> = (0..nb)
        .map(|b| (book_chap[b] as usize..book_chap[b + 1] as usize).map(|c| slice(chap_label, c)).collect())
        .collect();
    write_exceptions(&mut map, &abc, &label_exceptions(&chapters));
    let verses: Vec<Vec<&str>> = (0..nc)
        .map(|c| (chap_verse[c] as usize..chap_verse[c + 1] as usize).map(|v| slice(verse_label, v)).collect())
        .collect();
    write_exceptions(&mut map, &abc, &label_exceptions(&verses));
    // The full size of the working text, so the text part can land in place.
    put(&mut map, text.len() as u32);

    // The verses get an alphabet of their own — the codec's symbols: space
    // first, the verse separator last — and travel as one stream.
    let mut verse_chars: Vec<char> = (0..nv).flat_map(|v| slice(verse_text, v).chars()).collect();
    verse_chars.sort_unstable();
    verse_chars.dedup();
    assert!(verse_chars.len() <= codec::SEP as usize, "trop de caracteres dans les versets pour le codec");
    assert!(verse_chars[0] == ' ', "l'espace doit etre le symbole 0 du codec");
    let verse_abc = Alphabet::new(verse_chars);
    let mut stream = Vec::with_capacity(text.len() / 2);
    for v in 0..nv {
        stream.extend(slice(verse_text, v).chars().map(|c| verse_abc.code(c)));
        stream.push(codec::SEP);
    }
    let mut body = Vec::with_capacity(stream.len() + 4096);
    verse_abc.write(&mut body);
    put(&mut body, stream.len() as u32);
    let stream_base = body.len();
    body.extend_from_slice(&stream);

    // What lets one chapter land on its own, a tile: the verses' alphabet,
    // where the stream starts in the opened text part, and per chapter its
    // stream bytes and its UTF-8 bytes — where it lies in both.
    verse_abc.write(&mut map);
    put(&mut map, stream_base as u32);
    for c in 0..nc {
        let (first, last) = (chap_verse[c] as usize, chap_verse[c + 1] as usize);
        let codes: usize = (first..last).map(|v| slice(verse_text, v).chars().count() + 1).sum();
        put(&mut map, codes as u32);
        put(&mut map, verse_text[last] - verse_text[first]);
    }

    // ------------------------------------------------------------ the text
    // Readings: per insertion, the verse, where it falls in our verse, and
    // the two strings inserted — the mark that numbers our form and the
    // other edition's.
    let vars = &blobs.variants;
    let (n_display, n_insert) = (vars[0] as usize, vars[1] as usize);
    let inserts = &vars[2 + n_display * 3..2 + n_display * 3 + n_insert * 4];
    put(&mut body, n_insert as u32);
    let (mut last_verse, mut shift) = (0usize, 0usize);
    for i in 0..n_insert {
        let (verse, at, mark, other) =
            (inserts[i * 4] as usize, inserts[i * 4 + 1] as usize, inserts[i * 4 + 2] as usize, inserts[i * 4 + 3] as usize);
        if i == 0 || verse != last_verse {
            shift = 0;
        }
        let d = (0..n_display).find(|&k| vars[2 + k * 3] as usize == verse).expect("insertion sans verset");
        let display = &text[vars[2 + d * 3 + 1] as usize..][..vars[2 + d * 3 + 2] as usize];
        put(&mut body, (verse - last_verse) as u32);
        put(&mut body, (at - shift) as u32);
        abc.put_str(&mut body, &display[at..at + mark]);
        abc.put_str(&mut body, &display[at + mark..at + mark + other]);
        shift += mark + other;
        last_verse = verse;
    }

    let sealed = seal(&body);
    assert!(open(&sealed) == body, "le codec ne redonne pas le texte");
    let packed = Packed { map, text: sealed, plain: body };
    assert!(unpack(&packed.map, &packed.text) == *blobs, "le paquet ne redonne pas le corpus");
    packed
}

/// Splits a text part around its verse stream: the alphabet before, the
/// stream's length field and bytes, the readings after.
fn around_stream(part: &[u8]) -> (&[u8], usize, &[u8]) {
    let mut r = Reader::new(part);
    let n = r.get() as usize;
    for _ in 0..n {
        r.get();
    }
    let head = r.at;
    let len = r.get() as usize;
    let at = r.at;
    (&part[..head], len, &part[at..])
}

/// The text part as it travels: its verse stream through the codec, the rest
/// as is — the alphabet and the readings are a few kilobytes.
pub fn seal(plain: &[u8]) -> Vec<u8> {
    let (head, len, rest) = around_stream(plain);
    let packed = codec::encode(&rest[..len]);
    let mut out = head.to_vec();
    put(&mut out, packed.len() as u32);
    out.extend_from_slice(&packed);
    out.extend_from_slice(&rest[len..]);
    out
}

/// The text part as `unpack_text` reads it: the verse stream decoded.
/// The heavy step — the page runs it in a worker, as the bytes arrive.
pub fn open(sealed: &[u8]) -> Vec<u8> {
    let mut opening = Opening::default();
    opening.push(sealed);
    opening.finish()
}

/// Opens a text part as it arrives: the alphabet, then the codec's stream —
/// decoded as far as the bytes received allow — then the readings.
#[derive(Default)]
pub struct Opening {
    /// Bytes received before the stream starts.
    head: Vec<u8>,
    /// Once the head is read: where the stream's length field starts in it,
    /// and the stream's length.
    split: Option<(usize, usize)>,
    stream: codec::Stream,
    /// Bytes of the stream received so far.
    seen: usize,
    /// What follows the stream: the readings.
    tail: Vec<u8>,
}

/// The head of a text part, if whole: where the stream's length field
/// starts, the stream's length, where the stream starts.
fn read_head(bytes: &[u8]) -> Option<(usize, usize, usize)> {
    // A varint at `at`, if whole, and where the next one starts.
    let get = |at: usize| -> Option<(u32, usize)> {
        let (mut v, mut shift, mut at) = (0u32, 0, at);
        loop {
            let byte = *bytes.get(at)?;
            at += 1;
            v |= ((byte & 0x7F) as u32) << shift;
            if byte & 0x80 == 0 {
                return Some((v, at));
            }
            shift += 7;
        }
    };
    let (n, mut at) = get(0)?;
    for _ in 0..n {
        at = get(at)?.1;
    }
    let field = at;
    let (len, start) = get(at)?;
    Some((field, len as usize, start))
}

impl Opening {
    /// Appends bytes of the text part, in order, and decodes what they allow.
    pub fn push(&mut self, chunk: &[u8]) {
        if self.split.is_none() {
            self.head.extend_from_slice(chunk);
            let Some((field, len, start)) = read_head(&self.head) else { return };
            self.split = Some((field, len));
            // What came past the head is the start of the stream.
            let rest = self.head.split_off(start);
            self.head.truncate(field);
            return self.push(&rest);
        }
        let (_, len) = self.split.unwrap();
        let take = chunk.len().min(len - self.seen);
        self.stream.push(&chunk[..take]);
        self.seen += take;
        self.tail.extend_from_slice(&chunk[take..]);
        self.stream.run(false);
    }

    /// Symbols of the verse stream decoded so far.
    pub fn decoded(&self) -> usize {
        self.stream.decoded()
    }

    /// Decodes the rest — no more input is coming — and gives the opened text.
    pub fn finish(self) -> Vec<u8> {
        let stream = self.stream.finish();
        let mut out = Vec::with_capacity(self.head.len() + 5 + stream.len() + self.tail.len());
        out.extend_from_slice(&self.head);
        put(&mut out, stream.len() as u32);
        out.extend_from_slice(&stream);
        out.extend_from_slice(&self.tail);
        out
    }
}

/// The map part, read: what the overview needs, and where the text part will
/// land. The text buffer already holds the names and labels; the index
/// already holds everything but the verse offsets.
pub struct Map {
    abc: Alphabet,
    verse_abc: Alphabet,
    pub text: Vec<u8>,
    pub index: Vec<u32>,
    /// Size of the whole working text, text part included.
    pub total: usize,
    /// Where the verse stream starts in the opened text part.
    pub stream_base: usize,
    /// Per chapter: where it lies in the verse stream and in the text.
    pub chapters: Vec<Span>,
}

/// One chapter's place: its codes in the verse stream — separators included
/// — and its verses' bytes in the working text, each as (start, length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub stream: (usize, usize),
    pub text: (usize, usize),
}

pub fn unpack_map(map: &[u8]) -> Map {
    let mut r = Reader::new(map);
    let abc = Alphabet::read(&mut r);
    let mut text = Vec::new();

    let mut section_name = Vec::with_capacity(4);
    for _ in 0..3 {
        section_name.push(text.len() as u32);
        abc.get_str(&mut r, &mut text);
    }
    section_name.push(text.len() as u32);

    let nb = r.get() as usize;
    let mut book_name = Vec::with_capacity(nb + 1);
    let mut book_chap = vec![0u32];
    for _ in 0..nb {
        book_name.push(text.len() as u32);
        abc.get_str(&mut r, &mut text);
        book_chap.push(book_chap.last().unwrap() + r.get());
    }
    book_name.push(text.len() as u32);
    let nc = *book_chap.last().unwrap() as usize;

    let mut chap_verse = vec![0u32];
    for _ in 0..nc {
        chap_verse.push(chap_verse.last().unwrap() + r.get());
    }
    let nv = *chap_verse.last().unwrap() as usize;

    let chapter_exceptions = read_exceptions(&mut r, &abc);
    let verse_exceptions = read_exceptions(&mut r, &abc);
    let total = r.get() as usize;

    let mut chap_label = Vec::with_capacity(nc + 1);
    lay_labels(book_chap.windows(2).map(|w| (w[1] - w[0]) as usize), &chapter_exceptions, &mut text, &mut chap_label);
    chap_label.push(text.len() as u32);
    let mut verse_label = Vec::with_capacity(nv + 1);
    lay_labels(chap_verse.windows(2).map(|w| (w[1] - w[0]) as usize), &verse_exceptions, &mut text, &mut verse_label);
    verse_label.push(text.len() as u32);

    let mut index = Vec::with_capacity(3 + 4 + 2 * (nb + 1) + 2 * (nc + 1) + 2 * (nv + 1));
    index.extend_from_slice(&[nb as u32, nc as u32, nv as u32]);
    index.extend_from_slice(&section_name);
    index.extend_from_slice(&book_name);
    index.extend_from_slice(&book_chap);
    index.extend_from_slice(&chap_label);
    index.extend_from_slice(&chap_verse);
    index.extend_from_slice(&verse_label);
    // The verse offsets come with the text part.
    index.resize(index.len() + nv + 1, 0);

    let verse_abc = Alphabet::read(&mut r);
    let stream_base = r.get() as usize;
    let mut chapters = Vec::with_capacity(nc);
    let (mut stream_at, mut text_at) = (0usize, text.len());
    for _ in 0..nc {
        let (codes, bytes) = (r.get() as usize, r.get() as usize);
        chapters.push(Span { stream: (stream_at, codes), text: (text_at, bytes) });
        stream_at += codes;
        text_at += bytes;
    }
    Map { abc, verse_abc, text, index, total, stream_base, chapters }
}

impl Map {
    /// Verses of a chapter, as global ranks.
    pub fn verses_of(&self, chapter: usize) -> core::ops::Range<usize> {
        let nb = self.index[0] as usize;
        let nc = self.index[1] as usize;
        let chap_verse = 3 + 4 + 2 * (nb + 1) + (nc + 1);
        self.index[chap_verse + chapter] as usize..self.index[chap_verse + chapter + 1] as usize
    }

    /// Where the verses end in the working text: the display strings follow.
    pub fn verses_end(&self) -> usize {
        self.chapters.last().map_or(self.text.len(), |s| s.text.0 + s.text.1)
    }
}

/// Lays one chapter's codes — its slice of the verse stream — as UTF-8 into
/// `out`, its exact place in the working text, and records each verse's
/// bounds, as offsets in the working text, into `bounds`: (start, end) per
/// verse of the chapter.
///
/// What a tile lands, and what the whole text lands chapter after chapter.
pub fn lay_chapter(map: &Map, chapter: usize, codes: &[u8], out: &mut [u8], bounds: &mut [(u32, u32)]) {
    let span = map.chapters[chapter];
    assert_eq!(codes.len(), span.stream.1, "tuile de taille fausse");
    assert_eq!(out.len(), span.text.1, "place de taille fausse");
    let (mut at, mut verse, mut start) = (0usize, 0usize, span.text.0);
    for &code in codes {
        if code == codec::SEP {
            bounds[verse] = (start as u32, (span.text.0 + at) as u32);
            verse += 1;
            start = span.text.0 + at;
        } else {
            let (buf, len) = &map.verse_abc.utf8[code as usize];
            let len = *len as usize;
            out[at..at + len].copy_from_slice(&buf[..len]);
            at += len;
        }
    }
    assert_eq!((verse, at), (bounds.len(), out.len()), "tuile mal formee");
}

/// One insertion of the other edition's reading: where it falls in our
/// verse, the mark that numbers our form, the other form.
type Insertion = (usize, Vec<u8>, Vec<u8>);

/// The other edition's readings — `rest`, what follows the verse stream in
/// the opened text part — laid once every verse is in: the display strings
/// into `out`, the working text past the verses, and the readings' table.
///
/// `verses` is the working text up to the end of the verses, where they are
/// read: two disjoint stretches, so a caller can lay the readings while
/// slices of the verses are still held.
pub fn lay_readings(map: &Map, rest: &[u8], verses: &[u8], out: &mut [u8], bounds: &[(u32, u32)]) -> Vec<u32> {
    let base = map.verses_end();
    debug_assert_eq!(verses.len(), base);
    let abc = &map.abc;
    let mut r = Reader::new(rest);
    let n_insert = r.get() as usize;
    let mut groups: Vec<(usize, Vec<Insertion>)> = Vec::new();
    let mut verse = 0usize;
    for _ in 0..n_insert {
        verse += r.get() as usize;
        let at = r.get() as usize;
        let mut mark = Vec::new();
        abc.get_str(&mut r, &mut mark);
        let mut other = Vec::new();
        abc.get_str(&mut r, &mut other);
        match groups.last_mut() {
            Some((v, list)) if *v == verse => list.push((at, mark, other)),
            _ => groups.push((verse, vec![(at, mark, other)])),
        }
    }
    let mut end = map.verses_end();
    let mut displays = Vec::with_capacity(groups.len() * 3);
    let mut inserts = Vec::with_capacity(n_insert * 4);
    for (verse, list) in &groups {
        let (from, to) = (bounds[*verse].0 as usize, bounds[*verse].1 as usize);
        let start = end;
        let mut cursor = from;
        let mut lay = |bytes: &[u8], end: &mut usize| {
            out[*end - base..*end - base + bytes.len()].copy_from_slice(bytes);
            *end += bytes.len();
        };
        for (at, mark, other) in list {
            // `at` is where the insertion falls in our verse.
            lay(&verses[cursor..from + at], &mut end);
            cursor = from + at;
            let mark_at = end - start;
            lay(mark, &mut end);
            lay(other, &mut end);
            inserts.extend_from_slice(&[*verse as u32, mark_at as u32, mark.len() as u32, other.len() as u32]);
        }
        lay(&verses[cursor..to], &mut end);
        displays.extend_from_slice(&[*verse as u32, start as u32, (end - start) as u32]);
    }
    let mut variants = vec![groups.len() as u32, n_insert as u32];
    variants.extend_from_slice(&displays);
    variants.extend_from_slice(&inserts);
    variants
}

/// The opened text part, cut: the verse stream, and what follows it.
pub fn split_text<'a>(map: &Map, plain: &'a [u8]) -> (&'a [u8], &'a [u8]) {
    let len: usize = map.chapters.iter().map(|s| s.stream.1).sum();
    let base = map.stream_base;
    (&plain[base..base + len], &plain[base + len..])
}

/// Lands the opened text part after the map's strings — every chapter in
/// its place, then the readings — and gives the working form.
pub fn unpack_text(map: Map, plain: &[u8]) -> Blobs {
    let mut text = map.text.clone();
    text.resize(map.total, 0);
    let nv = map.index[2] as usize;
    let mut bounds = vec![(0u32, 0u32); nv];
    let (stream, rest) = split_text(&map, plain);
    for (c, span) in map.chapters.iter().enumerate() {
        let verses = map.verses_of(c);
        let (at, len) = span.text;
        let codes = &stream[span.stream.0..span.stream.0 + span.stream.1];
        lay_chapter(&map, c, codes, &mut text[at..at + len], &mut bounds[verses]);
    }
    let (verses, out) = text.split_at_mut(map.verses_end());
    let variants = lay_readings(&map, rest, verses, out, &bounds);
    let verses_end = map.verses_end() as u32;
    let mut index = map.index;
    let at = index.len() - (nv + 1);
    for v in 0..nv {
        index[at + v] = bounds[v].0;
    }
    index[at + nv] = bounds.last().map_or(verses_end, |b| b.1);
    Blobs { text, index, variants }
}

/// Both parts at once, the text part as it travels.
pub fn unpack(map: &[u8], text: &[u8]) -> Blobs {
    unpack_text(unpack_map(map), &open(text))
}

#[cfg(test)]
mod tests;
