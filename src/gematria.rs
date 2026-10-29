//! Gematria: a Hebrew word or phrase read as a number, by each of the
//! methods TorahCalc documents (https://www.torahcalc.com/info/gematria).
//!
//! **One pass over the text, then a few multiplications per method.** The
//! pass tallies how many times each of the 27 letter forms occurs, how many
//! letters and words there are, and one sum weighted by position (`Tally`).
//! Every method is then read off that tally alone: a method that gives each
//! form a value is the dot product of the counts with its table, and the
//! others — those that add the letter or word count, square the total, or
//! weigh a letter by its place — are a formula over the same few numbers.
//! Asking all of them costs the pass and 27 multiplications per table.
//!
//! **The tables are computed, not typed** — at compile time, from the three
//! things the methods are defined by: the standard value of the 22 letters,
//! the name of each letter, and the name of each number. Each table is the
//! rule written in code (`AtBash` mirrors the alphabet, `Ayak Bachar` shifts
//! the 27 forms by nine…), so a value cannot be mistyped; the tests hold
//! each table to its rule and to the values the reference publishes.
//!
//! The 27 forms are in Unicode order, alef to tav, each final its own
//! (`letter::exact_code`). The **sequence** is the order of the gadol
//! values: the 22 letters, then the five finals (ך ם ן ף ץ), so that value
//! `(j % 9 + 1) · 10^(j / 9)` holds for every rank `j`, 1 to 900.

use crate::letter::{self, ALPHABET, FORMS};

/// Every method, in the order of the reference's page. The wasm export and
/// the API answer them in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    MisparHapanim,
    MisparGadol,
    MisparSiduri,
    MisparKatan,
    MisparPerati,
    MisparShemi,
    MisparMusafi,
    Atbash,
    Albam,
    MisparBoneeh,
    MisparKidmi,
    MisparNeelam,
    MisparHamerubahHaklali,
    MisparMeshulash,
    MisparHaachor,
    MisparMispari,
    MisparKatanMispari,
    MisparKolel,
    Achbi,
    Atbach,
    AyakBachar,
    Ofanim,
    AchasBeta,
    Avgad,
    ReverseAvgad,
}

use Method::*;

impl Method {
    pub const ALL: [Method; 25] = [
        MisparHapanim,
        MisparGadol,
        MisparSiduri,
        MisparKatan,
        MisparPerati,
        MisparShemi,
        MisparMusafi,
        Atbash,
        Albam,
        MisparBoneeh,
        MisparKidmi,
        MisparNeelam,
        MisparHamerubahHaklali,
        MisparMeshulash,
        MisparHaachor,
        MisparMispari,
        MisparKatanMispari,
        MisparKolel,
        Achbi,
        Atbach,
        AyakBachar,
        Ofanim,
        AchasBeta,
        Avgad,
        ReverseAvgad,
    ];

    /// The table giving each form its value, for the methods that are one.
    pub fn table(self) -> Option<&'static [u32; FORMS]> {
        TABLES[self as usize].as_ref()
    }
}

/// Letters a text may hold to be read: past it, a value could pass 2^53
/// and lose its last digits in a JSON number or a JavaScript one — the
/// tests hold the worst case of every method under that bound.
pub const MAX_LETTERS: usize = 10_000;

/// What one pass over a text leaves: all any method needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Tally {
    /// Occurrences of each form, alef to tav, finals their own.
    pub forms: [u32; FORMS],
    pub letters: u32,
    /// Runs of letters and marks holding a consonant, parted by anything
    /// else: a space, a maqaf, a paseq, a sof pasuq, punctuation.
    pub words: u32,
    /// Σ standard value × place of the letter in the text, from 1.
    pub weighted: u64,
    /// Letters past `MAX_LETTERS` were left uncounted.
    pub truncated: bool,
}

impl Tally {
    /// The text's value by `method`. Exact up to `MAX_LETTERS` letters.
    pub fn value(&self, method: Method) -> u64 {
        if let Some(table) = method.table() {
            return self.dot(table);
        }
        let standard = self.dot(&STANDARD);
        let letters = u64::from(self.letters);
        match method {
            MisparMusafi => standard + letters,
            // Each letter adds itself and every letter before it: the k-th
            // of n is counted n - k + 1 times, which is (n + 1) · total − Σ v · k.
            MisparBoneeh => (letters + 1) * standard - self.weighted,
            // The letter's name without the letter itself: every default
            // name starts with its letter.
            MisparNeelam => self.dot(&SHEMI) - standard,
            MisparHamerubahHaklali => standard * standard,
            MisparHaachor => self.weighted,
            // The digital root: the digits summed until one is left.
            MisparKatanMispari => match standard {
                0 => 0,
                v => 1 + (v - 1) % 9,
            },
            MisparKolel => standard + u64::from(self.words),
            _ => unreachable!("a method with a table is read above"),
        }
    }

    fn dot(&self, table: &[u32; FORMS]) -> u64 {
        self.forms.iter().zip(table).map(|(&n, &v)| u64::from(n) * u64::from(v)).sum()
    }
}

/// Reads a text once. Only the 27 Hebrew letters count; points,
/// cantillation, punctuation and anything else are skipped. Stops counting
/// past `MAX_LETTERS`: the caller refuses such a text.
pub fn tally(text: &str) -> Tally {
    let mut tally = Tally::default();
    let mut in_word = false;
    let mut holds_letter = false;
    for c in text.chars() {
        match class(c) {
            Class::Letter(form) => {
                if tally.letters as usize == MAX_LETTERS {
                    tally.truncated = true;
                } else {
                    tally.letters += 1;
                    tally.forms[form as usize] += 1;
                    tally.weighted += u64::from(STANDARD[form as usize]) * u64::from(tally.letters);
                }
                in_word = true;
                holds_letter = true;
            }
            Class::Mark => in_word = true,
            Class::Gap => {
                tally.words += u32::from(in_word && holds_letter);
                (in_word, holds_letter) = (false, false);
            }
        }
    }
    tally.words += u32::from(in_word && holds_letter);
    tally
}

/// The consonants a text is read as, in order: what `tally` counts.
pub fn letters(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().filter(|&c| letter::exact_code(c).is_some())
}

enum Class {
    /// One of the 27 forms.
    Letter(u8),
    /// Inside a word, but no letter: a point or a cantillation mark.
    Mark,
    /// Between words.
    Gap,
}

fn class(c: char) -> Class {
    if let Some(form) = letter::exact_code(c) {
        return Class::Letter(form);
    }
    match c as u32 {
        // Maqaf, paseq, sof pasuq: they part words.
        0x05BE | 0x05C0 | 0x05C3 => Class::Gap,
        0x0591..=0x05C7 => Class::Mark,
        _ => Class::Gap,
    }
}

// The tables, built at compile time from the rules.

/// A form's letter, finals folded: 0 alef … 21 tav (`letter::code`).
const FOLD: [usize; FORMS] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 12, 13, 13, 14, 15, 16, 16, 17, 17, 18, 19, 20, 21];

/// A form's rank in the sequence: the 22 letters, then ך ם ן ף ץ.
const SEQUENCE: [usize; FORMS] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 22, 10, 11, 23, 12, 24, 13, 14, 15, 25, 16, 26, 17, 18, 19, 20, 21];

/// The value of rank `j` of the sequence: 1 to 9, 10 to 90, 100 to 900.
/// The 22 letters' standard values are its first 22.
const fn value(j: usize) -> u32 {
    (j as u32 % 9 + 1) * 10u32.pow(j as u32 / 9)
}

/// The letters' names, as the reference spells them by default — the
/// values of Mispar Shemi, Ne'elam and Ofanim.
const NAMES: [&str; ALPHABET] = [
    "אלף", "בית", "גימל", "דלת", "הא", "וו", "זין", "חית", "טית", "יוד", "כף", "למד", "מם", "נון", "סמך", "עין", "פא", "צדי",
    "קוף", "ריש", "שן", "תו",
];

/// The numbers' names, each letter's standard value spelled out — Mispar
/// Mispari. Spaces are no letter.
const NUMBERS: [&str; ALPHABET] = [
    "אחד", "שתיים", "שלוש", "ארבע", "חמש", "שש", "שבע", "שמונה", "תשע", "עשר", "עשרים", "שלושים", "ארבעים", "חמישים", "שישים",
    "שבעים", "שמונים", "תשעים", "מאה", "מאתיים", "שלוש מאות", "ארבע מאות",
];

/// The forms of a Hebrew string, in order, each its standard value summed —
/// UTF-8 read by hand, since this runs at compile time: a consonant is
/// `D7 90` to `D7 AA`.
const fn spelled(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let (mut i, mut sum) = (0, 0);
    while i + 1 < bytes.len() {
        if bytes[i] == 0xD7 && bytes[i + 1] >= 0x90 && bytes[i + 1] <= 0xAA {
            sum += value(FOLD[(bytes[i + 1] - 0x90) as usize]);
            i += 2;
        } else {
            i += 1;
        }
    }
    sum
}

/// The standard value of a name's last consonant — Ofanim.
const fn last_letter(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let mut i = bytes.len();
    while i >= 2 {
        if bytes[i - 2] == 0xD7 && bytes[i - 1] >= 0x90 && bytes[i - 1] <= 0xAA {
            return value(FOLD[(bytes[i - 1] - 0x90) as usize]);
        }
        i -= 1;
    }
    0
}

/// Every method's table, form by form, `None` for a method that is a
/// formula over the tally.
static TABLES: [Option<[u32; FORMS]>; Method::ALL.len()] = {
    let mut tables = [None; Method::ALL.len()];
    let mut m = 0;
    while m < Method::ALL.len() {
        tables[m] = table_of(Method::ALL[m]);
        m += 1;
    }
    tables
};

const STANDARD: [u32; FORMS] = match table_of(MisparHapanim) {
    Some(table) => table,
    None => unreachable!(),
};

const SHEMI: [u32; FORMS] = match table_of(MisparShemi) {
    Some(table) => table,
    None => unreachable!(),
};

const fn table_of(method: Method) -> Option<[u32; FORMS]> {
    if !has_table(method) {
        return None;
    }
    let mut table = [0; FORMS];
    let mut f = 0;
    while f < FORMS {
        table[f] = entry(method, FOLD[f], SEQUENCE[f]);
        f += 1;
    }
    Some(table)
}

const fn has_table(method: Method) -> bool {
    !matches!(
        method,
        MisparMusafi | MisparBoneeh | MisparNeelam | MisparHamerubahHaklali | MisparHaachor | MisparKatanMispari | MisparKolel
    )
}

/// The value a method gives a form whose letter is `l` (0..22) and whose
/// rank in the sequence is `j` (0..27).
const fn entry(method: Method, l: usize, j: usize) -> u32 {
    let v = value(l);
    match method {
        MisparHapanim => v,
        MisparGadol => value(j),
        MisparSiduri => j as u32 + 1,
        // The value without its zeros: 1 to 9 again in each nine.
        MisparKatan => l as u32 % 9 + 1,
        MisparPerati => v * v,
        MisparMeshulash => v * v * v,
        MisparShemi => spelled(NAMES[l]),
        MisparMispari => spelled(NUMBERS[l]),
        Ofanim => last_letter(NAMES[l]),
        // Each letter adds every one before it in the alphabet.
        MisparKidmi => {
            let (mut sum, mut k) = (0, 0);
            while k <= l {
                sum += value(k);
                k += 1;
            }
            sum
        }
        // The alphabet mirrored: alef with tav.
        Atbash => value(ALPHABET - 1 - l),
        // Its halves swapped: alef with lamed.
        Albam => value((l + 11) % ALPHABET),
        // Each half mirrored: alef with kaf, lamed with tav.
        Achbi => value(if l < 11 { 10 - l } else { 32 - l }),
        // The 27 forms in three nines, each mirrored: alef with tet.
        Atbach => value(j / 9 * 9 + 8 - j % 9),
        // The three nines shifted: alef takes yod's place, yod qof's, qof alef's.
        AyakBachar => value((j + 9) % FORMS),
        // Groups of seven, seven and eight shifted the same way; tav, the
        // eighth of the last, unmoved.
        AchasBeta => value(if l < 14 {
            l + 7
        } else if l < 21 {
            l - 14
        } else {
            l
        }),
        // The next letter, tav to alef; and the one before.
        Avgad => value((l + 1) % ALPHABET),
        ReverseAvgad => value((l + ALPHABET - 1) % ALPHABET),
        _ => 0,
    }
}

pub mod search;

#[cfg(test)]
mod tests;
