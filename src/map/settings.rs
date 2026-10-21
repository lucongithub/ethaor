//! The map's settings: every tuned value, with the reason for it. The
//! numbers they produce are asserted in the tests, not here.

/// Share of a letter's cell taken by the world it contains.
///
/// **Large, and that is what removes the white square.** At 0.004, the next
/// Tanakh measured only six pixels at the moment of folding: nothing was
/// legible in it, and a white fill was needed so the screen would not be black
/// during the fifteen or so notches it took to grow.
///
/// At 0.9, it is born nine tenths of the screen wide, so its book titles pass
/// `MIN_TEXT` immediately: the rendering goes from one text to the next without
/// ever painting anything in between. Measured: 1 382 px on a 1 536 window,
/// 351 px on a 390 phone — and in both cases zero frame without text. At 0.7,
/// the phone had a black frame at the fold.
pub(crate) const NEST: f32 = 0.9;

/// Folding only happens once the letter is wider than the screen: one can
/// therefore zoom fully into a letter without seeing anything arrive. Nothing
/// of the world it contains is drawn before that moment, and by then it is
/// already a thousand pixels wide: we go from one text to the next with no
/// intermediate state.
pub(super) const NEST_AFTER: f32 = 1.0;

/// Below this, text would be no more than a grey smear.
pub(crate) const MIN_TEXT: f32 = 4.0;

/// Share of its third of the world taken by a portion title, when the map is
/// nothing but three words, and the gap between two of those words as a share
/// of the world height.
///
/// Both are taken on the **projected** world: the three names are born large at
/// the first zoom-out notch, then shrink and draw closer at every following
/// notch, endlessly. Measured, 1 536 x 730 window: 61 px and a 130 px gap at
/// the first notch, then 43 and 93, 31 and 66, 22 and 48.
///
/// The gap was tried at a third of the world height — the three words then held
/// the corners of the screen and no longer read as a group — then at a sixth,
/// too tight. At a quarter, it is worth 2.1 times the size of the words.
pub(crate) const SECTION_CARD: f32 = 0.35;
pub(crate) const CARD_GAP: f32 = 1.0 / 4.0;
/// Share of the screen's height the three portions' names may take at most:
/// on a short window the floored framing crops the world, and the names
/// shrink with the room rather than be cut with the world.
pub(crate) const CARD_ROOM: f32 = 0.9;
/// Distance between two portions' names, centre to centre, as a share of
/// their size once the page gives that size: the book view's own spacing —
/// one size, and the `gap-[1.14em]` between two names.
pub(crate) const CARD_LEAD: f32 = 2.14;

/// Floor specific to chapter numbers, lower than the one for running text: a
/// number has one to three letters, known in advance and read as a shape, where
/// a line of verse has to be deciphered.
///
/// That is what guarantees the promise "chapters from the first notch": at the
/// notch above the framing, a number measures 4.7 px on a window 730 px tall
/// and 4.0 px on one of 620. Since the portions' air tightens at chapter level,
/// the floor no longer decides anything: it is only there as a guard rail.
pub(crate) const MIN_NUMBER: f32 = 3.5;

/// Hierarchy of whites: a section is pure white, a book one tone below, a
/// chapter number a shade lower still. Rank therefore reads by intensity,
/// without any frame being needed.
pub(crate) const BOOK_INK: f32 = 0.78;
pub(crate) const CHAPTER_INK: f32 = 0.58;

/// Share of the title band taken by the text.
///
/// The title is sized by **the band**, which is in world units: it therefore
/// grows with the zoom, exactly like the box and the chapters, and the space
/// between the top of the frame and the first chapter keeps the same look. The
/// band being of constant height, every book carries the same title size at a
/// given zoom.
pub(super) const TITLE_FILL: f32 = 1.0;

/// Ceiling, as a share of the window height. Past that point the band has left
/// the screen — we have entered the book — and the title has no need to grow
/// further: it is only there to recall where one is.
pub(crate) const TITLE_CEILING: f32 = 0.085;

/// Minimum line height for a chapter to show its verses.
///
/// Just above the text floor: at a leading of 1.30, a 5.5 px line carries
/// characters of 4.2 px, i.e. `MIN_TEXT`. Demanding more gained nothing in
/// legibility and pushed the verses two zoom notches further away.
pub(crate) const READABLE_LINE: f32 = 5.5;

/// Inner margin of a cell, as a **share of its side**.
///
/// Fixed at 2 world units, it left only 5.9 units of text in a cell of 9.9 —
/// 40 % of the ink thrown away, in width as in height, and verses that only
/// appeared six zoom notches later. A cell's margin must follow the cell, like
/// all the rest of the rendering.
pub(crate) const CELL_PAD: f32 = 0.06;

/// Nominal factor of one wheel notch, and length of the dive in notches: from
/// the framing to a full-screen chapter. `zoom_step` deduces the real factor
/// from it, so that this length is the same on every window.
pub(crate) const ZOOM_STEP: f32 = 1.4;
pub(crate) const DIVE_NOTCHES: f32 = 14.0;

/// Beyond this multiple of the initial framing, chapter numbers appear.
///
/// At launch the map therefore shows only the sections and the books, large;
/// the numbers arrive from the first zoom notch. The threshold is relative to
/// the framing and not to a cell size: it thus falls in the same place whatever
/// the window, and after a fold as before.
pub(super) const CHAPTER_FROM: f32 = 1.1;

/// Below this multiple of the initial framing, the map is nothing but the three
/// portion names.
///
/// The three levels are therefore **one notch from each other**, on either side
/// of the framing: one zoom-out notch gives the three portions, the framing
/// gives the portions and the books, one zoom-in notch gives the chapters. Like
/// `CHAPTER_FROM`, the threshold is relative to the framing, not to a size: it
/// falls in the same place whatever the window.
///
/// It is just below 1 and not at 1: the framing itself must stay on the books'
/// side, rounding included.
pub(crate) const PORTIONS_UNDER: f32 = 0.95;

/// How many wheel notches ahead of the reading level a chapter's tile is
/// asked for: while the zoom closes in on a few chapters, their verses travel,
/// so they are there by the time the lines are legible.
pub(super) const TILE_AHEAD: i32 = 3;

/// A chapter the book view hands back to the map (`show`): the share of the
/// view it fills, and the margin its lines keep over the drawing's floors —
/// on the floor itself, a rounding would leave it numbered and not read.
pub(super) const SHOW_FILL: f32 = 0.92;
pub(super) const SHOW_MARGIN: f32 = 1.25;

/// Line height and breathing space between verses, as multiples of the
/// character size of a chapter's text.
pub(crate) const LINE_K: f32 = 1.30;
pub(super) const GAP_K: f32 = 0.55;

/// Marks of a chapter laid out as text, as multiples of its character size.
///
/// **The chapter number sits in the top-right corner of its column**, where
/// reading starts, on a row of its own above the first verse: once the text is
/// out, the centred number has gone, and the reader still has to know which
/// chapter he is reading. **A verse number sits at the right of its first
/// line**, in a gutter of its own: Hebrew reads from the right, so that is
/// where a verse begins. Both are reserved by the layout, never laid over the
/// text — the gutter and the header row are paid on the character size.
pub(crate) const CHAPTER_MARK: f32 = 2.0;
pub(crate) const VERSE_MARK: f32 = 0.8;

/// Section titles are two thirds bigger than book titles.
pub(crate) const SECTION_OVER_BOOK: f32 = 1.25;


/// Estimated width of a letter, until the browser really measures.
pub(crate) const GUESS: f32 = 0.42;

/// Estimated width of the widest number — three letters — that the layout
/// starts from. `build_frame` rebuilds it as soon as the page has measured the
/// real one.
pub(crate) const ESTIMATED_NUMERAL: f32 = 3.0 * GUESS;
