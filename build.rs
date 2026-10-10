//! Construit le corpus a la compilation.
//!
//! Lit les sources HTML Mechon-Mamre (cp1255), en extrait livres / chapitres /
//! versets, nettoie la ponctuation et les marqueurs massoretiques, puis ecrit
//! deux fichiers dans OUT_DIR :
//!
//!   text.bin   tout le texte UTF-8 concatene
//!   index.bin  tableau plat de u32 : en-tete + decalages
//!
//! A l'execution le programme ne parse rien : il decoupe ces deux blocs.

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
            // t01.htm .. t35.htm : un fichier par livre (t0101.htm = chapitre, ignore)
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

// ---------------------------------------------------------------- decodage

/// cp1255 -> UTF-8.
///
/// Mechon-Mamre utilise l'octet 0xCA, non attribue dans cp1255 standard, pour
/// U+05BA HOLAM HASER FOR VAV (p.ex. עֲוֺן). Sans ce correctif, 414 signes sont
/// perdus dans le Tanakh.
fn decode_cp1255(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let c = match b {
            0x00..=0x7F => b as char,
            0xA0 => ' ',                                    // NBSP
            0xC0..=0xC9 => char::from_u32(0x05B0 + (b - 0xC0) as u32).unwrap(),
            0xCA => '\u{05BA}',                             // correctif Mechon-Mamre
            0xCB..=0xCF => char::from_u32(0x05B0 + (b - 0xC0) as u32).unwrap(),
            0xD0..=0xD8 => char::from_u32(0x05C0 + (b - 0xD0) as u32).unwrap(),
            0xE0..=0xFA => char::from_u32(0x05D0 + (b - 0xE0) as u32).unwrap(),
            _ => ' ',                                       // reste : hors sujet
        };
        s.push(c);
    }
    s
}

// ------------------------------------------------------------------ modele

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

// ------------------------------------------------------- ordre canonique

/// Les 39 livres, dans l'ordre : 5 de la Torah, 21 des Prophetes, 13 des
/// Ecrits. C'est cet ordre qui est affiche, pas celui des fichiers sources.
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

/// Livres que la source presente d'un seul tenant, avec une numerotation
/// continue, et que l'usage separe en deux.
///
/// Mechon-Mamre numerote Samuel de 1 a 55, Rois de 1 a 47, Chroniques de 1 a 65
/// et Ezra/Nehemie de 1 a 23 : aucun redemarrage a `א` ne marque la coupure.
/// On coupe donc au rang traditionnel, et les totaux concordent exactement.
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

/// Renumerote les chapitres a partir de 1 : apres une coupure, la seconde
/// moitie heriterait sinon des numeros de la premiere.
fn renumber(mut chapters: Vec<Chapter>) -> Vec<Chapter> {
    for (i, chapter) in chapters.iter_mut().enumerate() {
        chapter.label = hebrew_numeral(i + 1);
    }
    chapters
}

/// Nombre en lettres hebraiques. `15` et `16` s'ecrivent `טו` et `טז`, et non
/// `יה` / `יו`, qui formeraient un nom divin.
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

// ------------------------------------------------------------------ parsing

/// Un fichier contient un ou plusieurs `<H1>` ; chacun ouvre un livre
/// (t35 = Ezra + Nehemie). Le `<H1>` de page de garde de t01 n'a aucun verset
/// et se trouve donc elimine.
fn parse_book_file(html: &str, corpus: &mut Corpus) {
    let body = html.split("<BODY>").last().unwrap_or(html);

    for raw_book in body.split("<H1>").skip(1) {
        let Some((name, rest)) = raw_book.split_once("</H1>") else { continue };
        let name = strip_tags(name).trim().to_string();

        // <B>chapitre,verset</B> texte ... jusqu'au <B> suivant
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
            continue; // page de garde
        }
        // `order` preserve l'ordre du document ; un tri alphabetique sur des
        // chiffres hebraiques donnerait un ordre faux.
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

/// Retire balises, marqueurs massoretiques `{פ} {ס} {ר} {ש}` et toute
/// ponctuation. Le maqaf `-` joint deux mots distincts : il devient une espace,
/// pas une suppression. Resultat : des mots separes par une seule espace.
fn clean(s: &str) -> String {
    let no_tags = strip_tags(s);

    let mut out = String::with_capacity(no_tags.len());
    let mut in_brace = false;
    for c in no_tags.chars() {
        match c {
            '{' => in_brace = true,
            '}' => in_brace = false,
            _ if in_brace => {}
            // hebreu : lettres, niqqud, cantillation
            '\u{0590}'..='\u{05F4}' => out.push(c),
            // tout le reste (, . ; : - -- ( ) [ ] espaces) = separateur de mots
            _ => out.push(' '),
        }
    }

    // normalise les espaces
    let mut res = String::with_capacity(out.len());
    for w in out.split_whitespace() {
        if !res.is_empty() {
            res.push(' ');
        }
        res.push_str(w);
    }
    res
}

// ------------------------------------------------------------------- sortie

/// Disposition de `index.bin`, en u32 :
///
///   [0]  n_books
///   [1]  n_chapters
///   [2]  n_verses
///   puis section_name (4 entrees : trois sections et la sentinelle)
///   puis, dans l'ordre, six tableaux de decalages (chacun avec sentinelle
///   finale, donc `fin = suivant`) :
///     book_name    n_books+1       -> octets dans text.bin
///     book_chap    n_books+1       -> indices de chapitres
///     chap_label   n_chapters+1    -> octets
///     chap_verse   n_chapters+1    -> indices de versets
///     verse_label  n_verses+1      -> octets
///     verse_text   n_verses+1      -> octets
///
/// Le texte est ecrit par regions homogenes (noms de livres, puis etiquettes de
/// chapitres, puis etiquettes de versets, puis textes), ce qui rend chaque
/// serie de decalages monotone.
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

    // region 0 : noms des trois sections
    let mut section_name = Vec::with_capacity(4);
    for name in ["תורה", "נביאים", "כתובים"] {
        section_name.push(text.len() as u32);
        text.push_str(name);
    }
    section_name.push(text.len() as u32);

    // region 1 : noms de livres
    for b in &corpus.books {
        book_name.push(text.len() as u32);
        text.push_str(&b.name);
    }
    book_name.push(text.len() as u32);

    // region 2 : etiquettes de chapitres
    for b in &corpus.books {
        for c in &b.chapters {
            chap_label.push(text.len() as u32);
            text.push_str(&c.label);
        }
    }
    chap_label.push(text.len() as u32);

    // region 3 : etiquettes de versets
    for b in &corpus.books {
        for c in &b.chapters {
            for v in &c.verses {
                verse_label.push(text.len() as u32);
                text.push_str(&v.label);
            }
        }
    }
    verse_label.push(text.len() as u32);

    // region 4 : textes des versets
    for b in &corpus.books {
        for c in &b.chapters {
            for v in &c.verses {
                verse_text.push(text.len() as u32);
                text.push_str(&v.text);
            }
        }
    }
    verse_text.push(text.len() as u32);

    // index hierarchiques
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
