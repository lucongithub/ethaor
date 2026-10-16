//! Verses laid out in their cell: one size per chapter, lines cut at words,
//! letters and words boxed right to left.

use super::*;

impl App {
    /// Layout of a chapter, in world coordinates.
    ///
    /// **All verses have the same character size.** A verse that is too long
    /// goes over several lines instead of being compressed: only the number of
    /// lines varies, never the size. A breathing space separates the verses, so
    /// that one sees where one ends and the next begins.
    ///
    /// Computed once per chapter, then cached: the layout depends only on the
    /// world, not on the zoom.
    pub(crate) fn chapter_lines(&mut self, chapter: usize) -> Vec<Line> {
        if let Some(lines) = self.lines.get(&chapter) {
            return lines.clone();
        }
        let laid = self.lay_out(chapter);
        self.lines.insert(chapter, laid.clone());
        laid
    }

    fn lay_out(&mut self, chapter: usize) -> Vec<Line> {
        let cell = column(self.layout.chapters[chapter]);
        let verses = corpus::verses_of(chapter);
        let count = verses.len().max(1);

        // We start from one line per verse, then tighten as long as the column
        // overflows. The number of lines can only grow as the size grows, so a
        // few passes are enough. The header row and the verse-number gutter
        // scale with the size, so they enter every pass.
        let mut size = cell.h / (count as f32 * LINE_K + (count - 1) as f32 * GAP_K + CHAPTER_MARK * LINE_K);
        for _ in 0..5 {
            let width = cell.w - self.gutter(chapter, size);
            let total = CHAPTER_MARK * LINE_K * size + self.column_height(chapter, width, size);
            if total <= cell.h {
                break;
            }
            size *= cell.h / total;
        }

        let gutter = self.gutter(chapter, size);
        let width = cell.w - gutter;
        let mut out = Vec::new();
        let mut y = cell.y + CHAPTER_MARK * LINE_K * size;

        for verse in verses {
            let text = drawn(verse);
            for slice in self.wrap(text, width, size) {
                out.push(Line {
                    rect: Rect { x: cell.x, y, w: width, h: size * LINE_K },
                    text: slice,
                    verse,
                    size,
                });
                y += size * LINE_K;
            }
            y += size * GAP_K;
        }

        out
    }

    /// Width of the verse-number gutter of a chapter, at a character size: its
    /// widest number, plus one space between it and the text.
    pub(crate) fn gutter(&mut self, chapter: usize, size: f32) -> f32 {
        let widest = corpus::verses_of(chapter)
            .map(|v| self.ratio(corpus::verse_label(v)))
            .fold(0.0_f32, f32::max);
        (widest * VERSE_MARK + self.space_ratio()) * size
    }

    /// Height the chapter's verses would take at this character size.
    pub(crate) fn column_height(&mut self, chapter: usize, width: f32, size: f32) -> f32 {
        let verses = corpus::verses_of(chapter);
        let count = verses.len().max(1);
        let mut lines = 0usize;

        for verse in verses {
            let drawn = self.width_of(drawn(verse), size);
            let needed = if width > 0.0 { (drawn / width).ceil() as usize } else { 1 };
            lines += needed.max(1);
        }

        lines as f32 * size * LINE_K + (count - 1) as f32 * size * GAP_K
    }

    /// Splits a verse into lines that fit in `width`, at word boundaries.
    ///
    /// Every line stays a slice of the corpus: only an offset and a length cross
    /// the boundary with the page.
    pub(crate) fn wrap(&mut self, verse: &'static str, width: f32, size: f32) -> Vec<&'static str> {
        if width <= 0.0 || size <= 0.0 || self.width_of(verse, size) <= width {
            return vec![verse];
        }

        let words: Vec<&'static str> = verse.split(' ').collect();
        let space = self.space_ratio();

        let mut out = Vec::new();
        let mut start = 0usize; // start of the current line, in bytes
        let mut at = 0usize; // start of the current word
        let mut run = 0.0f32;

        for word in &words {
            let w = self.ratio(word) * size;
            let first = at == start;
            let advance = if first { w } else { w + space * size };

            if !first && run + advance > width {
                out.push(&verse[start..at - 1]); // without the trailing space
                start = at;
                run = w;
            } else {
                run += advance;
            }
            at += word.len() + 1;
        }

        out.push(&verse[start..]);
        out
    }

    /// Cells of a line's letters, right to left.
    ///
    /// The text is drawn in one piece; this split only serves to locate a letter
    /// — for the nest it contains, and for clicks.
    pub(crate) fn letter_cells(&mut self, text: &'static str, row: Rect, size: f32) -> Vec<Rect> {
        let drawn = self.width_of(text, size);
        if drawn <= 0.0 {
            return Vec::new();
        }

        let parts: Vec<&'static str> = text.split(' ').flat_map(letter::letters).collect();
        let total: f32 = parts.iter().map(|p| self.ratio(p)).sum();
        if total <= 0.0 {
            return Vec::new();
        }

        let mut right = row.x + row.w;
        parts
            .iter()
            .map(|part| {
                let w = drawn * self.ratio(part) / total;
                right -= w;
                Rect { x: right, w, ..row }
            })
            .collect()
    }

    /// Cells of a line's words, right to left.
    ///
    /// Same split as `letter_cells`, one step coarser: it is what a double
    /// click reads. The spaces are counted — they are what separates two words
    /// on screen — so a word's box holds its ink and nothing else, and the gaps
    /// between boxes are the spaces.
    pub(crate) fn word_cells(&mut self, text: &'static str, row: Rect, size: f32) -> Vec<(Rect, &'static str)> {
        let drawn = self.width_of(text, size);
        if text.is_empty() || drawn <= 0.0 {
            return Vec::new();
        }

        let words: Vec<&'static str> = text.split(' ').collect();
        let space = self.space_ratio();
        let ink: f32 = words.iter().map(|w| self.ratio(w)).sum();
        let total = ink + words.len().saturating_sub(1) as f32 * space;
        if total <= 0.0 {
            return Vec::new();
        }

        // The drawn width is the measured one; the ratios only share it out.
        let scale = drawn / total;
        let mut right = row.x + row.w;
        let mut out = Vec::with_capacity(words.len());
        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                right -= space * scale;
            }
            let w = self.ratio(word) * scale;
            right -= w;
            out.push((Rect { x: right, w, ..row }, *word));
        }
        out
    }

    /// Line of a chapter nearest to a point, vertically.
    ///
    /// Nearest and not containing: the breathing spaces between verses leave
    /// gaps where the cursor often falls, and a strict search would find
    /// nothing there.
    pub(crate) fn line_at(&mut self, chapter: usize, y: f32) -> Option<Line> {
        self.chapter_lines(chapter)
            .into_iter()
            .min_by(|a, b| vertical_gap(a.rect, y).total_cmp(&vertical_gap(b.rect, y)))
    }

    /// Is a chapter **read** at this zoom, or merely numbered?
    ///
    /// That is the whole condition for a click: one only selects a passage one
    /// can see. Above that zoom a click would name a verse chosen by the
    /// geometry alone, which the reader has no way of checking.
    ///
    /// It is the condition of the **drawing**, taken in the same order: the
    /// cheap estimate first — a line of at least `READABLE_LINE`, which costs
    /// no layout — then the laid-out size, which is what `draw_line` really
    /// compares to `MIN_TEXT`. The estimate alone was not enough: a chapter of
    /// long verses wraps, hence lays out smaller than the estimate, and its
    /// cell fell back on its number while a click still believed the text was
    /// there.
    pub(crate) fn readable(&mut self, chapter: usize) -> bool {
        if self.line_height(chapter, self.camera) < READABLE_LINE {
            return false;
        }
        let z = self.camera.z;
        // All the lines of a chapter share one size.
        self.chapter_lines(chapter).first().is_some_and(|line| line.size * z >= MIN_TEXT)
    }
}

/// The text of a verse **as it is drawn**.
///
/// For the eighty-eight verses where another publication of the edition reads
/// differently, that is the welded display string: our verse with the other
/// form inserted right after ours. Everything downstream works on this string
/// and nothing else — wrapping, measuring, word boxes — so the other reading
/// takes room in the flow instead of being painted over the text. The corpus
/// keeps the verse untouched, which is what the breadcrumb and the click name.
pub(crate) fn drawn(verse: usize) -> &'static str {
    variant::display(verse).unwrap_or_else(|| corpus::verse_text(verse))
}

/// Text column of a cell: the cell minus its margin.
pub(crate) fn column(cell: Rect) -> Rect {
    cell.inset(cell.w * CELL_PAD)
}
