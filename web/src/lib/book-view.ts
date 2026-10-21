// The book view's logic: the Tanakh read by scrolling, as a web page reads.
// The module gives each chapter as a record of offsets into its text
// (`src/wasm/chapter.rs`); this reads it, sizes the chapters not laid out
// yet, and steps the text's size. The component (`BookView.svelte`) only
// lays out what it is handed — the browser shapes and wraps pointed Hebrew
// there, as it does for any page.

/** Words of a record's head, before its verses — `HEAD` in the module. */
export const HEAD = 10;

/** One stretch of a verse as shown: ours, or the other edition's. */
export interface Part {
	text: string;
	theirs: boolean;
	/** Our form where theirs differs: the word carrying `¹`, cut out alone so
	 *  the pointer on it — and on theirs — opens the index, nothing else. */
	marked: boolean;
	/** Which of the verse's readings a form belongs to — ours and theirs of
	 *  one pair share it, so the pointer on either colours theirs alone;
	 *  -1 for the rest of the verse. */
	reading: number;
}

export interface Verse {
	/** Global index of the verse. */
	index: number;
	/** Its number, in Hebrew letters. */
	label: string;
	/** The verse as shown, cut where the other edition's forms begin and end. */
	parts: Part[];
}

export interface Chapter {
	index: number;
	book: number;
	section: number;
	/** The chapter's number, the book's name, the portion's: Hebrew. */
	label: string;
	bookName: string;
	sectionName: string;
	/** First chapter of its book: the book's name is set above it. */
	opens: boolean;
	verses: Verse[];
	/** Whether a verse here carries the other edition's reading. */
	readings: boolean;
}

/**
 * A chapter's record, read. `text(offset, length)` decodes a stretch of the
 * corpus: every string is cut there by **byte** offsets, never in a decoded
 * string, since one pointed letter is several UTF-16 units.
 */
export function readChapter(
	index: number,
	record: ArrayLike<number>,
	text: (offset: number, length: number) => string
): Chapter {
	const verses: Verse[] = [];
	let at = HEAD;
	let readings = false;
	for (let v = 0; v < record[9]; v++) {
		const [from, length] = [record[at + 3], record[at + 4]];
		const runs = record[at + 5];
		const parts: Part[] = [];
		let cursor = from;
		for (let k = 0; k < runs; k++) {
			const [start, size] = [record[at + 6 + 2 * k], record[at + 7 + 2 * k]];
			if (start > cursor) parts.push(...ours(text(cursor, start - cursor), k));
			parts.push({ text: text(start, size), theirs: true, marked: false, reading: k });
			cursor = start + size;
		}
		if (cursor < from + length || parts.length === 0) {
			parts.push(...ours(text(cursor, from + length - cursor), -1));
		}
		readings ||= runs > 0;
		verses.push({ index: record[at], label: text(record[at + 1], record[at + 2]), parts });
		at += 6 + 2 * runs;
	}
	return {
		index,
		book: record[0],
		section: record[1],
		label: text(record[2], record[3]),
		bookName: text(record[4], record[5]),
		sectionName: text(record[6], record[7]),
		opens: record[8] === 1,
		verses,
		readings
	};
}

/**
 * A stretch of ours. Before the other edition's run `reading` (-1 for none)
 * it ends with our form, marked `¹`: that last word is cut out as its own
 * part, of the same reading.
 */
function ours(stretch: string, reading: number): Part[] {
	const at = stretch.lastIndexOf(' ') + 1;
	if (reading < 0 || !stretch.endsWith('¹'))
		return [{ text: stretch, theirs: false, marked: false, reading: -1 }];
	const head = stretch.slice(0, at);
	const form = { text: stretch.slice(at), theirs: false, marked: true, reading };
	return head ? [{ text: head, theirs: false, marked: false, reading: -1 }, form] : [form];
}

/**
 * Width of one byte of pointed Hebrew, as a share of the text's size: a
 * letter and its marks take about four bytes and two fifths of an em.
 */
export const EM_PER_BYTE = 0.11;

/** Line height and the air between verses, as shares of the text's size. */
export const LINE = 1.9;
export const VERSE_GAP = 0.35;

/** Height of a chapter's header — its number, and its book's name above. */
export const HEADER = 4;
export const BOOK_HEADER = 6;

/**
 * Height a chapter not laid out yet is given, in pixels: its bytes wrapped
 * on the column's width, its verses' air, its header. An estimate — the
 * browser lays the real one — so it errs on the side a scroll can bear: the
 * page anchors on what is read, and a chapter's real height replaces its
 * estimate before anyone reaches it.
 */
export function estimate(
	bytes: number,
	verses: number,
	opens: boolean,
	size: number,
	column: number
): number {
	const perLine = Math.max(1, column / (size * EM_PER_BYTE));
	const lines = Math.max(verses, Math.ceil(bytes / perLine) + verses / 2);
	const header = opens ? BOOK_HEADER : HEADER;
	return Math.round(size * (lines * LINE + verses * VERSE_GAP + header));
}

/** The text's size, in pixels: its default, and how far zoom takes it. */
export const SIZE = 22;
export const SIZE_MIN = 14;
export const SIZE_MAX = 72;

/** Factor of one step of zoom on the text: a browser's own step. */
export const SIZE_STEP = 1.1;

/** The text's size after `steps` of zoom — fractions from a trackpad. */
export function zoomed(size: number, steps: number): number {
	if (!Number.isFinite(steps)) return size;
	const next = size * Math.pow(SIZE_STEP, steps);
	return Math.min(SIZE_MAX, Math.max(SIZE_MIN, Math.round(next * 100) / 100));
}

/**
 * The runs of chapters in `from..to` whose verses are not in, as
 * `[first, end)`: each run is one stretch of the text part, so one Range
 * request.
 */
export function missing(from: number, to: number, landed: (chapter: number) => boolean) {
	const runs: [number, number][] = [];
	for (let c = from; c < to; c++) {
		if (landed(c)) continue;
		const last = runs[runs.length - 1];
		if (last && last[1] === c) last[1] = c + 1;
		else runs.push([c, c + 1]);
	}
	return runs;
}

/** Verses a chapter of `bytes` is reckoned to hold, before it lands. */
export const BYTES_PER_VERSE = 170;

/**
 * The chapters kept laid out around those on screen: `around` before and
 * after the first and last, within the Tanakh. A chapter left out keeps its
 * height, so the scroll does not move.
 */
export function kept(first: number, last: number, around: number, count: number) {
	const from = Math.max(0, Math.min(first, last) - around);
	const to = Math.min(count, Math.max(first, last) + around + 1);
	return { from, to };
}

/** A book of the contents: its portion, its name, its chapters. */
export interface ContentsBook {
	index: number;
	section: number;
	name: string;
	/** Global indices of its first chapter and of the one past its last. */
	first: number;
	end: number;
}

/** The Tanakh's contents: what the book view goes up to, to go elsewhere. */
export interface Contents {
	/** The portions' names, in order. */
	portions: string[];
	books: ContentsBook[];
	/** Every chapter's number, in Hebrew letters, by global index. */
	labels: string[];
}

/** Words of the contents record's head — `CONTENTS_HEAD` in the module. */
export const CONTENTS_HEAD = 3;

/** The contents' record, read; strings cut by byte offsets, as a chapter's. */
export function readContents(
	record: ArrayLike<number>,
	text: (offset: number, length: number) => string
): Contents {
	const [portionCount, bookCount, chapterCount] = [record[0], record[1], record[2]];
	let at = CONTENTS_HEAD;
	const portions: string[] = [];
	for (let p = 0; p < portionCount; p++, at += 2) portions.push(text(record[at], record[at + 1]));
	const books: ContentsBook[] = [];
	for (let b = 0; b < bookCount; b++, at += 5) {
		books.push({
			index: b,
			section: record[at],
			name: text(record[at + 1], record[at + 2]),
			first: record[at + 3],
			end: record[at + 4]
		});
	}
	const labels: string[] = [];
	for (let c = 0; c < chapterCount; c++, at += 2) labels.push(text(record[at], record[at + 1]));
	return { portions, books, labels };
}

/**
 * Where the book view is, from the text up: the text itself, then its
 * book's chapters, then the portions with every book's name, then the three
 * portions alone.
 */
export type Level = 'text' | 'chapters' | 'books' | 'portions';

const LEVELS: Level[] = ['text', 'chapters', 'books', 'portions'];

/** One level up from `level`; `null` from the portions, the top. */
export function up(level: Level): Level | null {
	return LEVELS[LEVELS.indexOf(level) + 1] ?? null;
}

/**
 * The map's depth (`here_depth`: 0 the portions alone, 1 the books, 2 the
 * chapters' numbers, 3 the text) as the book view's level it opens on.
 * The map's chapters open the books, not one book's chapters — the owner's
 * rule: the map shows many books' numbers at once, and the book view's
 * level for many books is the books.
 */
export function fromMap(depth: number): Level {
	return depth >= 3 ? 'text' : depth >= 1 ? 'books' : 'portions';
}

/**
 * The map's depth a level hands the reader back to: the text to its
 * passage, a book's chapters to that book's chapters, the books to the
 * framing — the portions and their books — and the portions alone to the
 * opening. The inverse of `fromMap`.
 */
export function toMap(level: Level): number {
	return ['portions', 'books', 'chapters', 'text'].indexOf(level);
}

/** The book holding a chapter; -1 out of the Tanakh. */
export function bookOf(contents: Contents, chapter: number): number {
	return contents.books.findIndex((b) => b.first <= chapter && chapter < b.end);
}
