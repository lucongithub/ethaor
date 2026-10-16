//! The camera: framing, its floors, the zoom notch, the detail level and the
//! dive — zoom, pull towards the aimed cell, fold into the next Tanakh.

use super::*;

#[derive(Clone, Copy)]
pub(crate) struct Camera {
    pub(crate) z: f32,
    pub(crate) x: f32,
    pub(crate) y: f32,
}

impl Camera {
    pub(crate) fn project(&self, r: Rect) -> Rect {
        Rect { x: r.x * self.z + self.x, y: r.y * self.z + self.y, w: r.w * self.z, h: r.h * self.z }
    }

    pub(crate) fn to_world(self, px: f32, py: f32) -> (f32, f32) {
        ((px - self.x) / self.z, (py - self.y) / self.z)
    }

    /// Re-roots the camera in `cell`, which contains a copy of the world.
    /// The image does not move; only the scale comes back down, which keeps the
    /// floats from losing their precision.
    pub(crate) fn wrap_in(&mut self, cell: Rect) {
        let z = self.z;
        let z2 = cell.w * z / WORLD.w;
        self.x += cell.x * z;
        self.y = cell.y * z + self.y + (cell.h * z - WORLD.h * z2) / 2.0;
        self.z = z2;
    }
}

impl App {
    /// Scale of the initial framing.
    ///
    /// It is the whole world in the window — unless that made the chapters
    /// illegible at the first notch. Below `base`, we prefer to **crop the map**
    /// rather than push the levels back: on a phone, insisting on showing all
    /// 1 600 units of width at once gave a framing three times smaller than on a
    /// laptop, hence numbers at the fourth notch instead of the first, and book
    /// titles of 5 px.
    ///
    /// The price is known: on a narrow window, the starting view no longer shows
    /// the full width of the map. One gets it back by zooming out one notch —
    /// the three portions — and panning.
    pub(crate) fn fit_scale(&self) -> f32 {
        let contain = (self.width / WORLD.w).min(self.height / WORLD.h);
        contain.max(self.base)
    }

    /// Minimum framing: the one that puts a chapter number just above its floor
    /// at the first zoom notch.
    ///
    /// Cached, because `fit_scale` is called everywhere and cannot borrow `self`
    /// mutably; recomputed when the layout changes.
    pub(super) fn base_scale(&mut self) -> f32 {
        let widest = self.widest_numeral();
        let detail = if self.detail { &self.layout } else { &self.spare };
        // One per cent of margin: on a very flat window, the computed step drops
        // below the nominal step, and the number missed its floor by three
        // hundredths of a pixel at the first notch.
        MIN_NUMBER * 1.05 / (number_in(detail, widest) * ZOOM_STEP)
    }

    /// Factor of one wheel notch.
    ///
    /// **Computed, and not fixed at 1.4**: it is worth whatever it takes for the
    /// dive from the framing down to a full-screen chapter to always take
    /// `DIVE_NOTCHES` notches, whatever the window. On a landscape screen the
    /// framing is already proportional to the height, so the computation yields
    /// exactly 1.40; it is on narrow windows, where the framing has been
    /// floored, that it departs a little.
    pub(crate) fn zoom_step(&mut self) -> f32 {
        let cell = {
            let detail = if self.detail { &self.layout } else { &self.spare };
            detail.chapters[0].h.max(0.01)
        };
        let fit = self.fit_scale();
        let target = (self.height / cell).max(fit * 1.01);
        (target / fit).powf(1.0 / DIVE_NOTCHES)
    }

    /// Minimum zoom, **technical and not visible**.
    ///
    /// Zooming out no longer has a legibility bound: past the three-portions
    /// state, their size no longer depends on the zoom but on the screen, so
    /// carrying on zooming out does not shrink them. All that remains is to keep
    /// the scale from falling to zero, where world coordinates would lose all
    /// meaning.
    ///
    /// A legibility bound was tried — stopping the zoom-out as soon as the three
    /// titles reached `MIN_TEXT`: it froze the view on three words of 4 px,
    /// illegible, instead of letting them grow.
    pub(crate) fn floor_scale(&self) -> f32 {
        1.0 / WORLD.w
    }

    /// Current level of detail: are the chapters on screen?
    ///
    /// The two levels do not have the same layout — the portions' air is halved
    /// there — hence neither the same boxes nor the same pitch. We keep both side
    /// by side and swap them when the threshold is crossed; nothing is
    /// recomputed, and the verse layout, which depends on the cells, is thrown
    /// away.
    pub(crate) fn set_detail(&mut self, detail: bool) {
        if detail == self.detail {
            return;
        }
        std::mem::swap(&mut self.layout, &mut self.spare);
        self.detail = detail;
        self.lines.clear();
        self.number = None;
    }

    pub(super) fn sync_detail(&mut self) {
        let camera = self.camera;
        let detail = self.chapters_ready(camera);
        self.set_detail(detail);
    }

    /// Scale the map opens on: **one notch below the framing**.
    ///
    /// The three levels are one notch apart, and the map starts at the top of
    /// them: the three portions alone, then the books, then the chapters. One
    /// reads the Tanakh in three parts before reading it in thirty-nine books,
    /// and the opening image says so rather than showing everything at once.
    /// Price: the dive to the text takes one notch more than `DIVE_NOTCHES`.
    /// Accepted — it buys the first thing the map has to say.
    ///
    /// It is the framing divided by the notch, never a scale of its own: the
    /// thresholds of the three levels all hang off `fit_scale`, and a second
    /// reference would let them drift apart.
    fn overview_scale(&mut self) -> f32 {
        self.fit_scale() / self.zoom_step()
    }

    /// Whether the camera still sits on the scale the map opens on.
    ///
    /// Two places ask it — a resize, and a numeral that rebuilds the layout —
    /// and they must ask it the same way: a view believed to be at the opening
    /// in one and not in the other would be refitted by one and left behind by
    /// the other.
    pub(crate) fn at_opening(&mut self) -> bool {
        let opening = self.overview_scale();
        (self.camera.z - opening).abs() < opening * 1e-3
    }

    pub(crate) fn fit(&mut self) {
        self.camera.z = self.overview_scale();
        self.camera.x = (self.width - WORLD.w * self.camera.z) / 2.0;
        self.camera.y = (self.height - WORLD.h * self.camera.z) / 2.0;
    }

    pub(crate) fn nest_at(&mut self, x: f32, y: f32) -> Option<Rect> {
        self.locate(x, y)?.3.map(|cell| cell.shrink(NEST))
    }

    /// Point aimed at by the zoom, with the letter containing it. Tolerates the
    /// gaps between boxes: aiming between two books must stay productive.
    pub(crate) fn target_near(&mut self, x: f32, y: f32) -> Option<(Rect, Rect)> {
        let chapter = self.chapter_at(x, y).or_else(|| self.nearest_chapter(x, y))?;
        let cell = self.locate_in(chapter, x, y)?.3?;
        Some((cell, cell.shrink(NEST)))
    }

    pub(crate) fn zoom_at(&mut self, px: f32, py: f32, factor: f32) {
        let (bx, by) = self.camera.to_world(px, py);
        self.camera.z *= factor;
        let (ax, ay) = self.camera.to_world(px, py);
        self.camera.x += (ax - bx) * self.camera.z;
        self.camera.y += (ay - by) * self.camera.z;

        // Descent. The nest sits at the centre of the letter, while the zoom
        // keeps the cursor still: without help, the nest drifts off screen and
        // becomes impossible to reach. So we gently pull the view towards it as
        // soon as the letter has been passed. The gesture stays free — the
        // correction is weak and only applies on approach.
        if factor > 1.0 {
            let (wx, wy) = self.camera.to_world(px, py);

            // As long as a chapter does not fill the screen, it is **it** that
            // the view follows. The backgrounds being black, the void between
            // cells is in no way distinguishable from the rest: without this
            // pull, zooming off-target led to a black screen with no way out.
            let aimed = self
                .chapter_at(wx, wy)
                .or_else(|| self.nearest_chapter(wx, wy))
                .map(|c| self.camera.project(self.layout.chapters[c]));

            if let Some(cell) = aimed
                && cell.w < self.width
            {
                let k = pull(factor);
                self.camera.x += (px - (cell.x + cell.w / 2.0)) * k;
                self.camera.y += (py - (cell.y + cell.h / 2.0)) * k;
            }

            let (wx, wy) = self.camera.to_world(px, py);
            if let Some((letter, nest)) = self.target_near(wx, wy) {
                let cell = self.camera.project(letter);
                let screen = self.camera.project(nest);

                if cell.w > self.width * NEST_AFTER {
                    // The letter has been passed: the new Tanakh appears, and
                    // the old one disappears at the same instant.
                    //
                    // We re-root **here**, and not when the nest covers the
                    // screen. Waiting cost some thirty notches spent handling
                    // coordinates of the order of 1e8 in f32: the nest, a few
                    // pixels wide, then drifted further and further from the
                    // centre (up to 800 px measured). Once the camera is
                    // returned to the nest's world, every number becomes small
                    // again and the framing exact.
                    self.camera.wrap_in(nest);
                    self.level += 1;
                } else if screen.w > 2.0 {
                    // Before that, a gentle pull towards the cursor: the nest is
                    // at the centre of its letter and would otherwise drift off
                    // screen.
                    let closeness = (cell.w / self.width).clamp(0.0, 1.0);
                    let k = (0.14 + 0.62 * closeness).max(pull(factor));
                    self.camera.x += (px - (screen.x + screen.w / 2.0)) * k;
                    self.camera.y += (py - (screen.y + screen.h / 2.0)) * k;
                }
            }
        }

        // Floor: we stop at the last zoom where the three portion titles are
        // still legible.
        self.camera.z = self.camera.z.max(self.floor_scale());

        // Below the framing, we re-centre: the view converges towards a single
        // point instead of drifting out of frame.
        if self.camera.z < self.fit_scale() {
            self.camera.x = (self.width - WORLD.w * self.camera.z) / 2.0;
            self.camera.y = (self.height - WORLD.h * self.camera.z) / 2.0;
        }
    }
}

/// Strength of the pull towards the target, for a given zoom notch.
///
/// **It must beat the drift of the zoom.** One notch multiplies the distance
/// between cursor and target by `factor`: a correction of `1 - 1/factor`
/// compensates it exactly, anything below lets the target escape. At a fixed
/// 18 % and notches of 1.4, the aimed cell left the screen and the frame went
/// black: measured, four empty frames over fifty notches once the chapter block
/// was set at the top of its box, hence further from the centre of the view. We
/// add a margin to converge instead of merely following.
pub(crate) fn pull(factor: f32) -> f32 {
    ((1.0 - 1.0 / factor.max(1.0)) + 0.12).clamp(0.0, 0.75)
}
