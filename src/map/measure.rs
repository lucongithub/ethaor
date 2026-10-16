//! Widths: what the page has measured, an estimate until it has, and the
//! fragments still to ask for (`pending`).

use super::*;

impl App {
    /// Width of the fragment for a font of size 1.
    ///
    /// As long as the page has not measured it, we answer an estimate and record
    /// the fragment in `pending`: the next frame will be right.
    pub(crate) fn ratio(&mut self, fragment: &'static str) -> f32 {
        let key = key_of(fragment);
        if let Some(&ratio) = self.ratios.get(&key) {
            return ratio;
        }
        if self.pending.len() < 4096 {
            self.pending.push(key.0);
            self.pending.push(key.1);
        }
        letter::letter_count(fragment).max(1) as f32 * GUESS
    }

    /// Width of a space, measured once on a real space from the corpus.
    pub(crate) fn space_ratio(&mut self) -> f32 {
        let space = *self.space.get_or_insert_with(|| {
            let text = corpus::text();
            let at = text.find(' ').unwrap_or(0);
            &text[at..at + 1]
        });
        self.ratio(space)
    }

    /// Width of a fragment, **deduced from its words** and not measured as
    /// such.
    ///
    /// That is what makes the layout stable: measuring the whole line would make
    /// the fragments to measure depend on the wrapping, which depends on the
    /// measurements. Every new layout would call for new measurements, which
    /// would invalidate the layout — endlessly. Words, for their part, never
    /// change.
    pub(crate) fn width_of(&mut self, text: &'static str, size: f32) -> f32 {
        let space = self.space_ratio();
        let mut total = 0.0;
        let mut count = 0usize;

        for word in text.split(' ') {
            total += self.ratio(word);
            count += 1;
        }

        (total + count.saturating_sub(1) as f32 * space) * size
    }

    /// Width of the widest number of the Tanakh, for a font of size 1.
    pub(crate) fn widest_numeral(&mut self) -> f32 {
        (0..corpus::chapter_count())
            .map(|c| self.ratio(corpus::chapter_label(c)))
            .fold(0.3_f32, f32::max)
    }
}

pub(crate) fn key_of(fragment: &'static str) -> (u32, u32) {
    (corpus::offset_of(fragment) as u32, fragment.len() as u32)
}
