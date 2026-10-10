//! Acces au corpus, sans copie ni allocation.
//!
//! Le texte et l'index sont produits par `build.rs` et soudes au binaire. Ils
//! vivent dans le segment en lecture seule de l'executable, que le systeme
//! mappe directement depuis le fichier : rien n'est lu, parse ou alloue au
//! demarrage. Toutes les fonctions ci-dessous sont des indexations `O(1)`.

/// Enveloppe garantissant l'alignement sur 4 octets.
///
/// `include_bytes!` ne promet aucun alignement ; sans cela, reinterpreter les
/// octets en `u32` serait un comportement indefini.
#[repr(C)]
struct Align4<T: ?Sized> {
    _align: [u32; 0],
    bytes: T,
}

static TEXT: &str = include_str!(concat!(env!("OUT_DIR"), "/text.bin"));

static INDEX: &Align4<[u8]> = &Align4 {
    _align: [],
    bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/index.bin")),
};

#[cfg(target_endian = "big")]
compile_error!("index.bin est ecrit en little-endian");

fn index() -> &'static [u32] {
    // SAFETY: `Align4` impose l'alignement sur 4 ; `index.bin` est un multiple
    // de 4 octets ecrit en little-endian par build.rs, et l'on a verifie
    // ci-dessus que la cible est little-endian.
    unsafe {
        core::slice::from_raw_parts(INDEX.bytes.as_ptr().cast::<u32>(), INDEX.bytes.len() / 4)
    }
}

/// Decoupage de l'index en six series de decalages. Voir `write_output` dans
/// `build.rs` pour la disposition.
#[allow(dead_code)]
struct Layout {
    section_name: &'static [u32],
    book_name: &'static [u32],
    book_chap: &'static [u32],
    chap_label: &'static [u32],
    chap_verse: &'static [u32],
    verse_label: &'static [u32],
    verse_text: &'static [u32],
}

fn layout() -> Layout {
    let idx = index();
    let (nb, nc, nv) = (idx[0] as usize, idx[1] as usize, idx[2] as usize);
    let rest = &idx[3..];

    let (section_name, rest) = rest.split_at(4);
    let (book_name, rest) = rest.split_at(nb + 1);
    let (book_chap, rest) = rest.split_at(nb + 1);
    let (chap_label, rest) = rest.split_at(nc + 1);
    let (chap_verse, rest) = rest.split_at(nc + 1);
    let (verse_label, rest) = rest.split_at(nv + 1);
    let (verse_text, _) = rest.split_at(nv + 1);

    Layout { section_name, book_name, book_chap, chap_label, chap_verse, verse_label, verse_text }
}

#[inline]
fn slice(offsets: &[u32], i: usize) -> &'static str {
    &TEXT[offsets[i] as usize..offsets[i + 1] as usize]
}

/// Le corpus complet, en UTF-8. La page y lit directement les fragments a
/// tracer, sans copie : seuls un decalage et une longueur traversent la
/// frontiere.
pub fn text() -> &'static str {
    TEXT
}

/// Decalage d'un fragment dans `text()`. Tous les fragments rendus sont des
/// tranches de cette chaine, donc la difference de pointeurs suffit.
pub fn offset_of(fragment: &str) -> usize {
    fragment.as_ptr() as usize - TEXT.as_ptr() as usize
}

/// Nombre de livres (35 pour le Tanakh Mechon-Mamre, Ezra et Nehemie separes).
pub fn book_count() -> usize {
    index()[0] as usize
}

/// Nombre total de chapitres, tous livres confondus.
pub fn chapter_count() -> usize {
    index()[1] as usize
}

/// Nombre total de versets.
#[allow(dead_code)]
pub fn verse_count() -> usize {
    index()[2] as usize
}

pub fn section_name(section: usize) -> &'static str {
    slice(layout().section_name, section)
}

pub fn book_name(book: usize) -> &'static str {
    slice(layout().book_name, book)
}

/// Indices globaux des chapitres d'un livre.
pub fn chapters_of(book: usize) -> core::ops::Range<usize> {
    let l = layout();
    l.book_chap[book] as usize..l.book_chap[book + 1] as usize
}

pub fn chapter_label(chapter: usize) -> &'static str {
    slice(layout().chap_label, chapter)
}

/// Indices globaux des versets d'un chapitre.
pub fn verses_of(chapter: usize) -> core::ops::Range<usize> {
    let l = layout();
    l.chap_verse[chapter] as usize..l.chap_verse[chapter + 1] as usize
}

#[allow(dead_code)]
pub fn verse_label(verse: usize) -> &'static str {
    slice(layout().verse_label, verse)
}

pub fn verse_text(verse: usize) -> &'static str {
    slice(layout().verse_text, verse)
}
