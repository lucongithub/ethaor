//! Splitting a word into letters: a consonant and the marks that follow it.
//!
//! Everything is computed on demand, only for what is visible, and always
//! **right to left**. Hebrew alignment is therefore exact by construction: we
//! never ask the text engine for its opinion.
/// A letter = one consonant and the marks that follow it (niqqud,
/// cantillation). Marks are combining characters: detaching them from their
/// consonant would break the rendering.
pub fn letters(word: &str) -> Letters<'_> {
    Letters { rest: word }
}
/// `Clone` is required: we walk twice, once for the sum of the widths, once to
/// place the cells.
#[derive(Clone)]
pub struct Letters<'a> {
    rest: &'a str,
}
impl<'a> Iterator for Letters<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        let end = self
            .rest
            .char_indices()
            .skip(1)
            .find(|(_, c)| is_base(*c))
            .map_or(self.rest.len(), |(i, _)| i);
        let (head, tail) = self.rest.split_at(end);
        self.rest = tail;
        Some(head)
    }
}
/// Hebrew consonant, final forms included.
fn is_base(c: char) -> bool {
    ('\u{05D0}'..='\u{05EA}').contains(&c)
}
pub fn letter_count(s: &str) -> usize {
    s.chars().filter(|&c| is_base(c)).count()
}

/// Letters of the alphabet once final forms are folded: alef to tav.
pub const ALPHABET: usize = 22;

/// Rank of a consonant in the alphabet, 0 = alef … 21 = tav, **final forms
/// folded onto their letter** — the code `letters.bin` is written in. `None`
/// for a mark, a space, anything that is not a consonant.
///
/// Mirrors `FOLD` in `build.rs`; the search tests read every verse through it
/// and compare with the welded stream, so the two cannot drift apart.
pub fn code(c: char) -> Option<u8> {
    const FOLD: [u8; 27] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 12, 13, 13, 14, 15, 16, 16, 17, 17, 18, 19, 20, 21,
    ];
    is_base(c).then(|| FOLD[(c as u32 - 0x05D0) as usize])
}

#[cfg(test)]
mod tests;
