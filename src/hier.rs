//! Splitting the text below the chapter: verses, words, letters.
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
