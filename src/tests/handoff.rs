//! The hand-off between the map and the book view: the map names where the
//! view is (`here`), never deeper than its image, and takes a reader back to
//! a chapter at its reading level, the verse at the centre (`show`).

use super::*;

/// Chapters a sweep over windows takes: both ends of the Tanakh, the
/// longest chapter, a one-chapter book, and a stride through the rest.
fn sample() -> Vec<usize> {
    let psalm_119 = corpus::chapters_of(26).start + 118;
    let obadiah = corpus::chapters_of(17).start;
    let mut chapters: Vec<usize> = (0..corpus::chapter_count()).step_by(97).collect();
    chapters.extend([psalm_119, obadiah, corpus::chapter_count() - 1]);
    chapters
}

/// Every chapter, shown with its last verse: the map then reads it — its
/// text, not its number — and names back that very chapter and verse. The
/// last verse is the farthest from where a chapter starts, so the one a
/// centring error would miss first.
#[test]
fn every_chapter_shown_is_read_and_named_back() {
    let mut app = reference_app();
    for chapter in 0..corpus::chapter_count() {
        let last = corpus::verses_of(chapter).end - 1;
        app.show(chapter, Some(last));
        assert!(app.readable(chapter), "chapter {chapter} shown but not read");
        assert_eq!(app.here(), (chapter, Some(last)), "chapter {chapter} named back elsewhere");
    }
}

/// On every window, once the page has measured what the frame asks: a
/// chapter shown again lands on its verse, and the frame draws verses.
#[test]
fn a_chapter_is_shown_on_every_window() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        for chapter in sample() {
            let verses = corpus::verses_of(chapter);
            let middle = verses.start + verses.len() / 2;
            app.show(chapter, Some(middle));
            measure_all(&mut app);
            app.show(chapter, Some(middle));
            app.build_frame();
            assert!(app.draws_a_verse(), "{w}x{h}: chapter {chapter} shown without its text");
            assert_eq!(app.here(), (chapter, Some(middle)), "{w}x{h}: chapter {chapter}");
        }
    }
}

/// Shown with no verse, a chapter is named back without one asked: the
/// view lands on it, and on its first lines when it is taller than the
/// screen.
#[test]
fn a_chapter_shown_without_a_verse_is_named_back() {
    let mut app = reference_app();
    for chapter in sample() {
        app.show(chapter, None);
        let (named, verse) = app.here();
        assert_eq!(named, chapter, "chapter {chapter} named back elsewhere");
        assert!(verse.is_some_and(|v| corpus::verses_of(chapter).contains(&v)), "chapter {chapter}: no verse of its own");
    }
}

/// `here` says no more than the image: the Tanakh's start at the opening,
/// a book's first chapter while only books show, and no verse until the
/// text is read.
#[test]
fn here_is_never_deeper_than_the_image() {
    let mut app = reference_app();
    assert_eq!(app.here(), (0, None), "the opening names a place");

    at_framing(&mut app);
    let (chapter, verse) = app.here();
    assert_eq!(verse, None, "a verse named with the books alone");
    assert!(
        (0..corpus::book_count()).any(|b| corpus::chapters_of(b).start == chapter),
        "with the books alone, chapter {chapter} opens no book"
    );

    // One notch in: the numbers, not the text.
    let step = app.zoom_step();
    app.zoom_at(app.width / 2.0, app.height / 2.0, step * 1.05);
    let camera = app.camera;
    assert!(app.chapters_ready(camera));
    let (chapter, verse) = app.here();
    assert_eq!(verse, None, "a verse named while chapter {chapter} is a number");
}

/// What `show` cannot place leaves the view as it was.
#[test]
fn a_chapter_out_of_range_moves_nothing() {
    let mut app = reference_app();
    let before = (app.camera.z, app.camera.x, app.camera.y);
    app.show(corpus::chapter_count(), None);
    app.show(usize::MAX, Some(0));
    assert_eq!((app.camera.z, app.camera.x, app.camera.y), before);
}

/// The depth the book view's levels read is the image's, on every window:
/// the opening is the three portions alone, the framing the books, a notch
/// in the chapters' numbers, a chapter shown its text — and `here` never
/// names deeper than that depth says.
#[test]
fn the_depth_is_the_image_on_every_window() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        app.fit();
        assert_eq!(app.depth(), 0, "{w}x{h}: the opening");
        assert_eq!(app.here(), (0, None), "{w}x{h}: the opening names a book");
        app.frame_books();
        assert_eq!(app.depth(), 1, "{w}x{h}: the framing");
        assert_eq!(app.here().1, None, "{w}x{h}: the framing names a verse");
        let mut notches = 0;
        while app.depth() == 1 && notches < 10 {
            let step = app.zoom_step();
            app.zoom_at(w / 2.0, h / 2.0, step);
            notches += 1;
        }
        assert_eq!(app.depth(), 2, "{w}x{h}: the chapters after {notches} notches");
        assert_eq!(app.here().1, None, "{w}x{h}: the numbers name a verse");
        for chapter in sample() {
            app.show(chapter, None);
            assert_eq!(app.depth(), 3, "{w}x{h}: chapter {chapter} shown");
        }
        app.frame_books();
        assert_eq!(app.depth(), 1, "{w}x{h}: back to the framing from a chapter");
    }
}

/// Every book shown at the chapters' level, on every window: its numbers
/// out, its text not read, the centre naming one of its own chapters — the
/// zoom the book view's chapters hand over to.
#[test]
fn every_book_is_shown_at_its_chapters() {
    for (w, h) in WINDOWS {
        let mut app = app_at(w, h);
        for book in 0..corpus::book_count() {
            let chapters = corpus::chapters_of(book);
            app.show_book(chapters.start + chapters.len() / 2);
            assert_eq!(app.depth(), 2, "{w}x{h}: book {book}");
            let (named, verse) = app.here();
            assert!(chapters.contains(&named), "{w}x{h}: book {book} names chapter {named}");
            assert_eq!(verse, None, "{w}x{h}: book {book} names a verse");
        }
        let before = (app.camera.x, app.camera.y, app.camera.z);
        app.show_book(corpus::chapter_count());
        assert_eq!((app.camera.x, app.camera.y, app.camera.z), before, "{w}x{h}: a chapter out of range moved the view");
    }
}
