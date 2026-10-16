//! The text as it lands: which chapters may open to the reading level, and
//! which tiles and books to fetch next.

use super::*;

impl App {
    /// Height of a line of this chapter, on screen, without laying the chapter
    /// out: 929 chapters per frame would be as many vectors thrown away.
    ///
    /// It is the one measure that says whether a chapter is **read** or merely
    /// numbered, so the drawing and the breadcrumb both take it from here.
    ///
    /// Zero until the text has landed: this is the one gate of the reading
    /// level — the drawing, the click and the breadcrumb all go through it.
    pub(crate) fn line_height(&self, chapter: usize, camera: Camera) -> f32 {
        if !self.verses_in(chapter) {
            return 0.0;
        }
        self.line_height_once_in(chapter, camera)
    }

    /// The line height the chapter has — or will have once its verses land.
    fn line_height_once_in(&self, chapter: usize, camera: Camera) -> f32 {
        let count = corpus::verses_of(chapter).len().max(1);
        column(self.layout.chapters[chapter]).h / count as f32 * camera.z
    }

    /// Whether a chapter's verses are there to read.
    fn verses_in(&self, chapter: usize) -> bool {
        self.text && corpus::chapter_ready(chapter)
    }

    /// Whether the last frame draws a line of a verse: a text command past
    /// the map's strings — names and labels lie before the verses.
    pub(crate) fn draws_a_verse(&self) -> bool {
        let start = corpus::verses_offset() as f32;
        self.commands.chunks(STRIDE).any(|c| c[0] == KIND_TEXT && c[7] >= start && c[8] > 0.0)
    }

    /// Asks for the tiles of the chapters the view is about to read: those on
    /// screen whose lines will pass `READABLE_LINE` within `TILE_AHEAD`
    /// notches, not in and not asked yet. Neighbours come in one run — one
    /// request — since a run of chapters is one stretch of the text part.
    /// Returns the number of runs, left in `tiles`.
    pub(crate) fn want_tiles(&mut self) -> usize {
        self.tiles.clear();
        let ahead = self.zoom_step().powi(TILE_AHEAD);
        let camera = self.camera;
        let mut run: Option<(usize, usize)> = None;
        for chapter in 0..corpus::chapter_count() {
            let wanted = !self.asked[chapter]
                && !self.verses_in(chapter)
                && self.visible(camera.project(self.layout.chapters[chapter]))
                && self.line_height_once_in(chapter, camera) * ahead >= READABLE_LINE;
            match (wanted, run) {
                (true, Some((first, end))) if end == chapter => run = Some((first, chapter + 1)),
                (true, _) => {
                    if let Some(r) = run {
                        self.push_tile(r);
                    }
                    run = Some((chapter, chapter + 1));
                }
                (false, _) => {}
            }
            if wanted {
                self.asked[chapter] = true;
            }
        }
        if let Some(r) = run {
            self.push_tile(r);
        }
        self.tiles.len() / 4
    }

    /// Of the books not asked for yet, the nearest to the centre of the view,
    /// now asked for. Distance is the world's, from the centre to the box.
    pub(crate) fn next_book(&mut self) -> Option<usize> {
        let (cx, cy) = self.camera.to_world(self.width / 2.0, self.height / 2.0);
        let book = (0..corpus::book_count())
            .filter(|&b| !self.books_asked[b])
            .min_by(|&a, &b| {
                let d = |b: usize| gap_to(self.layout.books[b], cx, cy);
                d(a).total_cmp(&d(b))
            })?;
        self.books_asked[book] = true;
        Some(book)
    }

    fn push_tile(&mut self, (first, end): (usize, usize)) {
        let (from, to) = corpus::tile_range(first..end);
        self.tiles.extend_from_slice(&[first as u32, end as u32, from as u32, to as u32]);
    }
}
