//! Gematria as both transports ask it: a text checked, read once, and each
//! method asked for named every way — its key, its English and Hebrew
//! names, the names it also goes by, what it does — beside its value.
//!
//! The computation is `gematria`'s: one tally of the text, every method
//! read off it. No corpus, no engine: nothing here waits for the text.

pub use crate::gematria::search::{self, STRIDE};
pub use crate::gematria::{MAX_LETTERS, Method, Tally};

use std::ops::Range;

use super::els::Refusal;
use super::reference;

/// What a method is called and what it does: what an agent reads to choose
/// one, and the page to name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// The key a caller names it by: snake case, its Hebrew name
    /// transliterated.
    pub key: &'static str,
    pub name: &'static str,
    pub hebrew: &'static str,
    /// Other names the reference gives it.
    pub also_known_as: &'static [&'static str],
    pub description: &'static str,
}

/// Every method's names, in the order of `Method::ALL`.
const INFO: [Info; Method::ALL.len()] = [
    Info {
        key: "mispar_hapanim",
        name: "Mispar haPanim",
        hebrew: "מספר הפנים",
        also_known_as: &["Mispar Hechrachi", "absolute value", "standard value", "normative value"],
        description: "The standard value: each of the 22 letters has its basic value, alef 1 to tet 9, yod 10 to \
            tsadi 90, qof 100 to tav 400. A final form is worth its letter.",
    },
    Info {
        key: "mispar_gadol",
        name: "Mispar Gadol",
        hebrew: "מספר גדול",
        also_known_as: &["large value"],
        description: "The standard value, but the five final forms continue the alphabet: final kaf 500, final \
            mem 600, final nun 700, final pe 800, final tsadi 900.",
    },
    Info {
        key: "mispar_siduri",
        name: "Mispar Siduri",
        hebrew: "מספר סידורי",
        also_known_as: &["ordinal value"],
        description: "Each letter's rank in the alphabet, alef 1 to tav 22; the five final forms continue it, \
            final kaf 23 to final tsadi 27.",
    },
    Info {
        key: "mispar_katan",
        name: "Mispar Katan",
        hebrew: "מספר קטן",
        also_known_as: &["Mispar Meugal", "reduced value"],
        description: "The standard value without its zeros: yod is 1 instead of 10, tav 4 instead of 400.",
    },
    Info {
        key: "mispar_perati",
        name: "Mispar Perati",
        hebrew: "מספר הפרטי",
        also_known_as: &["Mispar HaMerubah HaPerati", "squared values"],
        description: "Each letter's standard value squared: alef 1 × 1 = 1, bet 2 × 2 = 4.",
    },
    Info {
        key: "mispar_shemi",
        name: "Mispar Shemi",
        hebrew: "מספר שמי",
        also_known_as: &["Milui", "full name value"],
        description: "Each letter is worth its name spelled out: alef = אלף = 1 + 30 + 80 = 111. The names are \
            spelled אלף בית גימל דלת הא וו זין חית טית יוד כף למד מם נון סמך עין פא צדי קוף ריש שן תו.",
    },
    Info {
        key: "mispar_musafi",
        name: "Mispar Musafi",
        hebrew: "מספר מוספי",
        also_known_as: &[],
        description: "The standard value plus the number of letters.",
    },
    Info {
        key: "atbash",
        name: "AtBash",
        hebrew: "אתב״ש",
        also_known_as: &[],
        description: "Each letter takes the value of its opposite in the alphabet: alef that of tav, bet that \
            of shin.",
    },
    Info {
        key: "albam",
        name: "AlBam",
        hebrew: "אלב״ם",
        also_known_as: &[],
        description: "The alphabet split in half, each letter taking the value of its counterpart in the other \
            half: alef that of lamed, bet that of mem.",
    },
    Info {
        key: "mispar_boneeh",
        name: "Mispar Bone'eh",
        hebrew: "מספר בונה",
        also_known_as: &["building value"],
        description: "Each letter adds the value of every letter before it in the word or phrase to its own, \
            and the running totals are summed: אחד = 1 + (1 + 8) + (1 + 8 + 4) = 23.",
    },
    Info {
        key: "mispar_kidmi",
        name: "Mispar Kidmi",
        hebrew: "מספר קדמי",
        also_known_as: &["Mispar Meshulash", "triangular value"],
        description: "Each letter is worth its value plus those of every letter before it in the alphabet: \
            alef 1, bet 1 + 2 = 3, gimel 1 + 2 + 3 = 6.",
    },
    Info {
        key: "mispar_neelam",
        name: "Mispar Ne'elam",
        hebrew: "מספר נעלם",
        also_known_as: &["hidden value"],
        description: "Each letter is worth its name spelled out without the letter itself: alef = לף = 30 + 80 \
            = 110. The names are spelled as for Mispar Shemi.",
    },
    Info {
        key: "mispar_hamerubah_haklali",
        name: "Mispar HaMerubah HaKlali",
        hebrew: "מספר המרובע הכללי",
        also_known_as: &[],
        description: "The standard value of the whole, squared.",
    },
    Info {
        key: "mispar_meshulash",
        name: "Mispar Meshulash",
        hebrew: "מספר משולש",
        also_known_as: &["cubed value"],
        description: "Each letter's standard value cubed: alef 1 × 1 × 1 = 1, bet 2 × 2 × 2 = 8.",
    },
    Info {
        key: "mispar_haachor",
        name: "Mispar Ha'achor",
        hebrew: "מספר האחור",
        also_known_as: &[],
        description: "Each letter's standard value multiplied by its position in the word or phrase, from 1.",
    },
    Info {
        key: "mispar_mispari",
        name: "Mispar Mispari",
        hebrew: "מספר מספרי",
        also_known_as: &[],
        description: "Each letter's standard value spelled out as a Hebrew number, and that name's value \
            summed: alef = one = אחד = 1 + 8 + 4 = 13.",
    },
    Info {
        key: "mispar_katan_mispari",
        name: "Mispar Katan Mispari",
        hebrew: "מספר קטן מספרי",
        also_known_as: &["integral reduced value"],
        description: "The digital root of the standard value: its digits added until one digit is left \
            (אחד = 13 → 1 + 3 = 4).",
    },
    Info {
        key: "mispar_kolel",
        name: "Mispar Kolel",
        hebrew: "מספר כלל",
        also_known_as: &[],
        description: "The standard value plus the number of words.",
    },
    Info {
        key: "achbi",
        name: "AchBi",
        hebrew: "אכב״י",
        also_known_as: &[],
        description: "The alphabet split in half, each half mirrored: alef takes the value of kaf, bet that of \
            yod, lamed that of tav.",
    },
    Info {
        key: "atbach",
        name: "AtBach",
        hebrew: "אטב״ח",
        also_known_as: &[],
        description: "The alphabet, the five final forms at its end, split into three groups of nine, each \
            mirrored: alef takes the value of tet, bet that of chet, yod that of tsadi, qof that of final \
            tsadi. Final forms are worth 500 to 900.",
    },
    Info {
        key: "ayak_bachar",
        name: "Ayak Bachar",
        hebrew: "אי״ק בכ״ר",
        also_known_as: &["Ayak Bakar"],
        description: "The alphabet, the five final forms at its end, split into three groups of nine; each \
            letter takes the value of the one at its place in the next group: alef that of yod, yod that of \
            qof, qof that of alef. Final forms are worth 500 to 900.",
    },
    Info {
        key: "ofanim",
        name: "Ofanim",
        hebrew: "אופנים",
        also_known_as: &[],
        description: "Each letter is replaced by the last letter of its name: alef (אלף) by pe, bet (בית) by \
            tav.",
    },
    Info {
        key: "achas_beta",
        name: "Achas Beta",
        hebrew: "אח״ס בט״ע",
        also_known_as: &[],
        description: "The alphabet split into groups of seven, seven and eight letters; each letter takes the \
            value of the one at its place in the next group — alef that of chet, chet that of samekh, samekh \
            that of alef. Tav does not change.",
    },
    Info {
        key: "avgad",
        name: "Avgad",
        hebrew: "אבג״ד",
        also_known_as: &[],
        description: "Each letter is replaced by the next one: alef by bet, bet by gimel, tav by alef.",
    },
    Info {
        key: "reverse_avgad",
        name: "Reverse Avgad",
        hebrew: "אבג״ד הפוך",
        also_known_as: &[],
        description: "Each letter is replaced by the one before it: bet by alef, gimel by bet, alef by tav.",
    },
];

/// A method's names.
pub fn info(method: Method) -> &'static Info {
    &INFO[method as usize]
}

/// The method a key names, exactly as `info` spells it.
pub fn method(key: &str) -> Option<Method> {
    Method::ALL.into_iter().find(|&m| info(m).key == key)
}

/// The methods a caller asks for: keys parted by commas, answered in the
/// order of `Method::ALL`, each once. `None` asks for all of them.
pub fn methods(keys: Option<&str>) -> Result<Vec<Method>, Refusal> {
    let Some(keys) = keys else {
        return Ok(Method::ALL.to_vec());
    };
    let mut asked = [false; Method::ALL.len()];
    for key in keys.split(',') {
        let method = method(key).ok_or_else(|| Refusal::UnknownMethod(key.into()))?;
        asked[method as usize] = true;
    }
    Ok(Method::ALL.into_iter().filter(|&m| asked[m as usize]).collect())
}

/// A text read: what was read of it, and each method's value.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// The consonants read, in order: points, marks and anything not a
    /// Hebrew letter left out.
    pub letters: String,
    pub letter_count: u32,
    pub words: u32,
    pub values: Vec<(Method, u64)>,
}

/// Checks a text: one Hebrew letter at least, `MAX_LETTERS` at most.
pub fn check(tally: &Tally) -> Result<(), Refusal> {
    if tally.letters == 0 {
        return Err(Refusal::NoLetter);
    }
    if tally.truncated {
        return Err(Refusal::TooManyLetters);
    }
    Ok(())
}

/// Reads a text by the methods asked for.
pub fn read(text: &str, methods: &[Method]) -> Result<Reading, Refusal> {
    let tally = crate::gematria::tally(text);
    check(&tally)?;
    Ok(Reading {
        letters: crate::gematria::letters(text).collect(),
        letter_count: tally.letters,
        words: tally.words,
        values: methods.iter().map(|&m| (m, tally.value(m))).collect(),
    })
}

/// A gematria search as both transports ask it: every run of consecutive
/// words of one verse worth, by `method`, what `text` is worth.
/// Hashable: a server keys its answers by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    pub text: String,
    pub method: Method,
    /// Rank of the first hit returned, in the order of the text.
    pub offset: usize,
    /// Hits returned at most; 0 only counts.
    pub limit: usize,
    /// The verses searched, `[first, end)`; `None` is the whole Tanakh.
    pub scope: Option<(usize, usize)>,
}

/// One run found: whole words of one verse.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub verse: usize,
    /// Ranks of its first letter and past its last among the verse's
    /// consonants: what the map marks.
    pub from: u32,
    pub to: u32,
}

impl Hit {
    pub fn place(&self) -> reference::Verse {
        reference::verse(self.verse)
    }

    /// The words found, as the verse writes them — pointed.
    pub fn words(&self) -> &'static str {
        let text = crate::corpus::verse_text(self.verse);
        let (mut start, mut end) = (text.len(), text.len());
        let mut at = 0;
        for (rank, chunk) in crate::letter::letters(text).enumerate() {
            if rank == self.from as usize {
                start = at;
            }
            at += chunk.len();
            if rank + 1 == self.to as usize {
                end = at;
            }
        }
        text[start.min(end)..end].trim_end()
    }
}

/// A page of runs and the exact count behind them.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The consonants of the text, as read.
    pub letters: String,
    /// What the text is worth by the method: what every run is worth.
    pub value: u64,
    pub total: u64,
    /// Runs that can be paged through: the first `CAP` in the order of the
    /// text. Past them the total is still exact.
    pub listed: usize,
    pub offset: usize,
    pub hits: Vec<Hit>,
}

impl Page {
    /// The offset of the next page, if one remains among the listed hits.
    pub fn next_offset(&self) -> Option<usize> {
        let next = self.offset + self.hits.len();
        (!self.hits.is_empty() && next < self.listed).then_some(next)
    }
}

/// Checks a search and gives the value its runs must have.
pub fn check_search(query: &Query) -> Result<u64, Refusal> {
    let tally = crate::gematria::tally(&query.text);
    check(&tally)?;
    if query.limit > super::els::MAX_LIMIT {
        return Err(Refusal::Limit(query.limit));
    }
    if let Some((first, end)) = query.scope
        && (first >= end || end > crate::corpus::verse_count())
    {
        return Err(Refusal::EmptyScope);
    }
    if !crate::corpus::has_text() {
        return Err(Refusal::TextMissing);
    }
    Ok(tally.value(query.method))
}

/// The verses a scope covers: the whole Tanakh for none.
fn verses_of(scope: Option<(usize, usize)>) -> Range<usize> {
    scope.map_or(0..crate::corpus::verse_count(), |(first, end)| first..end)
}

/// What a search is expected to cost, in V8 milliseconds, without running
/// it: the letters of its stretch, read once. Refuses it past `limit_ms`.
pub fn admit(query: &Query, limit_ms: f64) -> Result<f64, Refusal> {
    let stream = search::stream().ok_or(Refusal::TextMissing)?;
    let estimate_ms = search::cost(stream.letters_in(verses_of(query.scope))) / 1e6;
    if estimate_ms > limit_ms {
        return Err(Refusal::TooCostly { estimate_ms, limit_ms });
    }
    Ok(estimate_ms)
}

/// Searches, then keeps `limit` runs from `offset`.
pub fn search(query: &Query) -> Result<Page, Refusal> {
    let value = check_search(query)?;
    let stream = search::stream().ok_or(Refusal::TextMissing)?;
    let mut found = Vec::new();
    let total = stream.search(query.method, value, verses_of(query.scope), super::els::CAP, &mut found);
    let runs = found.chunks_exact(search::STRIDE);
    let listed = runs.len();
    let hits = runs
        .skip(query.offset)
        .take(query.limit)
        .map(|r| Hit { verse: r[0] as usize, from: r[1], to: r[2] })
        .collect();
    Ok(Page {
        letters: crate::gematria::letters(&query.text).collect(),
        value,
        total,
        listed,
        offset: query.offset,
        hits,
    })
}
