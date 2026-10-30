//! The map module's gematria (`ethaor.wasm`, feature `map`): a stretch the
//! reader selected, read by every method at once — each value, and each
//! letter as the method reads it and its share of the value, which the page
//! draws over the passage.
//!
//! The browser's side of `server/src/api/gematria.rs`: both run
//! `shared::gematria`. In the map's module, not the search's: it reads no
//! corpus, so it answers at once — the search module waits for the whole
//! text — and the selection it reads is the map's.

use crate::shared::els::Refusal;
use crate::shared::gematria::{self, Method};

const METHODS: usize = Method::ALL.len();

/// The text written by the page, until `gematria_read` reads it.
static mut TEXT: Vec<u8> = Vec::new();
static mut VALUES: [f64; METHODS] = [0.0; METHODS];
static mut FLAGS: [u32; METHODS] = [0; METHODS];
/// Method after method, each letter's form as the method reads it.
static mut READ_AS: Vec<u8> = Vec::new();
/// Method after method, each letter's share of the value.
static mut SHARES: Vec<u32> = Vec::new();
static mut WORDS: u32 = 0;
static mut NAMES: String = String::new();

/// A method's value is the sum of its letters' shares.
pub const SUMMED: u32 = 1;
/// A method reads each letter as another.
pub const REPLACES: u32 = 2;

/// Room for a text of `len` bytes: where the page writes it.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_buffer(len: u32) -> *mut u8 {
    super::fault::guard();
    unsafe {
        TEXT.clear();
        TEXT.resize(len as usize, 0);
        TEXT.as_mut_ptr()
    }
}

/// Reads the `len` bytes written at `gematria_buffer` by every method, in
/// the order of `Method::ALL`: its value at `gematria_values_ptr` (f64),
/// what it is at `gematria_flags_ptr` (`SUMMED`, `REPLACES`), and for each
/// of the n letters, method after method, its form as the method reads it
/// (0 alef … 26 tav, finals their own) at `gematria_read_as_ptr` and its
/// share at `gematria_shares_ptr`. Returns n — or -1 for a text holding no
/// Hebrew letter, -2 past `MAX_LETTERS`: nothing is written then.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_read(len: u32) -> i32 {
    let text = unsafe { core::str::from_utf8(&TEXT[..(len as usize).min(TEXT.len())]) }.unwrap_or("");
    let reading = match gematria::read(text, &Method::ALL) {
        Ok(reading) => reading,
        Err(Refusal::TooManyLetters) => return -2,
        Err(_) => return -1,
    };
    let n = reading.letter_count as usize;
    unsafe {
        WORDS = reading.words;
        READ_AS.clear();
        SHARES.clear();
        let mut shares = Vec::with_capacity(n);
        for (m, (&(method, value), read_as)) in reading.values.iter().zip(&reading.read_as).enumerate() {
            VALUES[m] = value as f64;
            FLAGS[m] = if method.summed() { SUMMED } else { 0 } | if read_as.is_some() { REPLACES } else { 0 };
            let letters = read_as.as_deref().unwrap_or(&reading.letters);
            READ_AS.extend(letters.chars().filter_map(crate::letter::exact_code));
            gematria::shares(text, method, &mut shares);
            SHARES.extend_from_slice(&shares);
        }
    }
    n as i32
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_values_ptr() -> *const f64 {
    unsafe { VALUES.as_ptr() }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_flags_ptr() -> *const u32 {
    unsafe { FLAGS.as_ptr() }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_read_as_ptr() -> *const u8 {
    unsafe { READ_AS.as_ptr() }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_shares_ptr() -> *const u32 {
    unsafe { SHARES.as_ptr() }
}

/// The words of the text last read: what Mispar Kolel adds.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
pub extern "C" fn gematria_words() -> u32 {
    unsafe { WORDS }
}

/// Writes every method's names at `gematria_names_ptr`, one line each in the
/// order of `Method::ALL` — its key, English name and Hebrew name parted by
/// tabs; their length in bytes.
#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_names() -> u32 {
    unsafe {
        NAMES.clear();
        for method in Method::ALL {
            let info = gematria::info(method);
            NAMES.push_str(&format!("{}\t{}\t{}\n", info.key, info.name, info.hebrew));
        }
        NAMES.len() as u32
    }
}

#[cfg_attr(feature = "map", unsafe(no_mangle))]
#[allow(static_mut_refs)]
pub extern "C" fn gematria_names_ptr() -> *const u8 {
    unsafe { NAMES.as_ptr() }
}
