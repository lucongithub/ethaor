/**
 * Bridge to the WebAssembly module.
 *
 * All the computation lives in Rust: corpus, layout, camera, zoom folding,
 * pruning, click detection. This file only forwards the gestures and paints
 * the command list it is given.
 *
 * Two things are its own, and there is a single reason for that: the shaping
 * of pointed Hebrew — bidi and combining marks — lives in the browser, and
 * nothing else does it as well.
 *
 *   1. measure the width of the fragments Rust asks for;
 *   2. draw the text.
 */

// The module is imported, not served from `static/`: Vite then emits it under
// `_app/immutable/` with a hash of its content in the name, like the scripts,
// so a browser may keep it for good and a new build is never served stale.
import moduleUrl from './ethaor.wasm?url';
// The verses travel apart from the module: the map draws without them, so
// the first image never waits for the bulk of the corpus. Same hashed name.
import booksUrl from './ethaor-text.books.bin?url';
import bookCuts from './ethaor-text.books.json';
import plainUrl from './ethaor-text.plain.bin?url';
import SearchWorker from './search-worker?worker';
import type { Answer, Request } from './search-worker';
import { escalate, Fault, readTraps, relay, reporter } from './fault';
import { SearchCore, type Found, type WordsOptions } from './search-core';
import { fragmentKey, inflate, lanes, range, retried } from './text-open';
import type { Command } from './keys';
import { PORTION_TEXT } from './portions';
import { bands, paint as markPaint, slotsOf, SQUARE_ROUND, type Color } from './marks';
import {
	missing,
	readChapter,
	readContents,
	type Chapter,
	type Contents,
	type Found as BookFound
} from './book-view';
import type { Found as Inked } from './passage';
import { readCrossings, readResults, type Crossing, type Passage } from './results';

/**
 * Books fetched at once in the background. One at a time paid a round trip
 * per book; a few together fill the link, and each is still chosen nearest
 * to the view when its turn comes.
 */
const BOOKS_IN_FLIGHT = 8;

/** Must cover Hebrew with niqqud. Noto Serif Hebrew is loaded by app.css. */
export const HEBREW_FONT =
	'"Noto Serif Hebrew Variable", "Taamey Frank CLM", "SBL Hebrew", "David", serif';

/** Reference size for measuring: width is proportional. */
const REFERENCE = 100;

/**
 * The one colour of the map, and it carries information: the reading of the
 * other publication of the edition, inserted after ours — worn only while the
 * pointer is on it or on our form, white otherwise. Everything else is a
 * shade of white. Tailwind's yellow-200, the same the index uses under the
 * canvas.
 */
const OTHER_READING = '254,240,138';

/**
 * What the module welds into the text around the other reading: the two index
 * marks and the brackets. They live in the Latin block, so the font's latin
 * subset must be loaded before the first measurement, like the Hebrew one — a
 * width taken on a fallback font would be cached and wrong for good.
 */
const MARKS = '¹²[]';

const KIND_RECT = 0;
const KIND_TEXT = 1;
const KIND_CLIP = 2;
const KIND_UNCLIP = 3;
/** A search's square by a name: its box, its slot in the ink. */
const KIND_SQUARE = 4;

/** Ink of a run: ours, the other edition's, then the searches' (`INK_MARK + bits`, a bit per slot). */
const INK_MARK = 2;

/** Signature of the module's exports. Every float is CSS pixels. */
interface Wasm {
	memory: WebAssembly.Memory;
	fault_ptr(): number;
	fault_len(): number;
	init(width: number, height: number): number;
	resize(width: number, height: number): void;
	reset(): void;
	pan(dx: number, dy: number): void;
	zoom(x: number, y: number, factor: number): void;
	/** Factor of one wheel notch, computed by the module for this window. */
	zoom_step(): number;
	/** Chapter one can click on, or -1: the cursor shape follows it. */
	hover(x: number, y: number): number;
	/** Selects the passage under the point. 0 when nothing is clickable there. */
	select(x: number, y: number): number;
	/** Same, one step finer: the word under the point. */
	select_verse(verse: number): number;
	selection_ptr(): number;
	/** The selected verse's found letters: `(from, to, slots)` triples, slots as bits. */
	selection_marks_len(): number;
	selection_marks_ptr(): number;
	/** Breadcrumb from the centre of the view. */
	focus(): void;
	leave(mouse: number): void;
	/** Breadcrumb under a point: 1 if it names something, 0 if it falls back. */
	focus_at(x: number, y: number): number;
	focus_ptr(): number;
	build_frame(): number;
	frame_ptr(): number;
	frame_stride(): number;
	pending_ptr(): number;
	pending_len(): number;
	set_ratio(offset: number, length: number, ratio: number): void;
	clear_ratios(): void;
	text_ptr(): number;
	text_len(): number;
	/** The other reading under the pointer: one past its first byte, 0 for none. */
	readings_at(x: number, y: number): number;
	/** End (excluded) of the reading `readings_at` last answered. */
	readings_end(): number;
	/** Tiles the view needs now: the number of runs at `tiles_ptr`. */
	tiles_wanted(): number;
	/** Per run: first chapter, end chapter, byte range in the opened text. */
	tiles_ptr(): number;
	tile_buffer(length: number): number;
	tile_load(first: number, end: number): number;
	/** Lands the readings once every book is in: 1 then. */
	readings_load(): number;
	/** The next book to fetch in the background, nearest to the view; -1 when none. */
	next_book(): number;
	book_first_chapter(book: number): number;
	book_end_chapter(book: number): number;
	/** Probe: 1 when the last frame draws a line of a verse. */
	probe_verse_drawn(): number;
	/** The book view: chapters, their size before they land, their record. */
	chapter_count(): number;
	chapter_size(chapter: number): number;
	chapter_landed(chapter: number): number;
	chapter_tile_from(first: number, end: number): number;
	chapter_tile_to(first: number, end: number): number;
	/** Words of the chapter's record at `chapter_record_ptr`; 0 if not in. */
	chapter_record(chapter: number): number;
	chapter_record_ptr(): number;
	/** Where the view is, for the book view; -1 for no verse. */
	/** Words of the contents' record at `contents_ptr`: portions, books, chapters. */
	contents_record(): number;
	contents_ptr(): number;
	here_chapter(): number;
	here_verse(): number;
	/** How deep the image goes: 0 portions, 1 books, 2 chapters, 3 text. */
	here_depth(): number;
	/** Back to the framing: the portions and their books. */
	frame_books(): void;
	/** The book holding a chapter, at the chapters' level. */
	show_book(chapter: number): void;
	/** The size the book view gives the portions' names: the opening draws them at it. */
	set_portion_size(px: number): void;
	/** Back to a chapter at its reading level, the verse at the centre. */
	show(chapter: number, verse: number): void;
	/** The largest skip a word of `letters` letters can be read with: 0 for one letter. */
	skip_max(letters: number): number;
	/** Letters of the Tanakh's longest word: the last letter "every nth letter" reads. */
	longest_word(): number;
	/** Searches the map marks at once. */
	marks_slots(): number;
	/** Room for a search's hit records, in words: where they are copied. */
	marks_buffer(words: number): number;
	/** Marks those records in a slot, `span` letters each, in `form` (0 ELS, 1 letters); the runs placed. */
	marks_set(slot: number, span: number, form: number): number;
	marks_show(slot: number, on: number): void;
	marks_clear(slot: number): void;
	/** Joins the searches shown by any (0), each drawn whole, or by all (1), each only where every one is. */
	marks_join(all: number): void;
	/** A chapter's found letters as the book view shows it: `(from, to, slots)`, slots as bits. */
	chapter_marks(chapter: number): number;
	chapter_marks_ptr(): number;
	/** The shown searches holding a portion (0), book (1) or chapter (2), as bits. */
	marks_at(level: number, index: number): number;
	/** Lists the hits written at `marks_buffer` as passages, `span` letters each; how many. */
	results_set(span: number, form: number): number;
	/** Writes passages `from..from + count` at `results_ptr`; its length in words. */
	results_page(from: number, count: number): number;
	results_ptr(): number;
	results_clear(): void;
	/** Lists the searches shown together: of any (0) or where two cross (1); how many. */
	results_shown(all: number): number;
	/** The list's crossings, `(slots, passages)` pairs at `results_groups_ptr`; how many. */
	results_groups(): number;
	results_groups_ptr(): number;
	/** How many passages the searches shown make, of any (0) or of all (1). */
	passages_shown(all: number): number;
	/** How many passages the hits at `marks_buffer` make; the buffer taken. */
	passages_count(span: number, form: number): number;
}

/**
 * What a click selected.
 *
 * A click is only live on a passage one can read — the module refuses
 * elsewhere — so a selection always carries its verse. A word picked in the
 * passage adds it, with the rest of the verse split around it so the page
 * can show it in place (`passage.ts`).
 */
export interface Selection {
	book: number;
	chapter: number;
	/** The verse's number, in Hebrew letters. */
	number: string;
	verse: string;
	/** The word picked in the passage; empty until one is. */
	word: string;
	/** What precedes and follows the word inside the verse. */
	head: string;
	tail: string;
	/** The letters the searches shown found in the verse, by bytes of it. */
	found: Inked[];
}

/**
 * Where one is. Feeds the breadcrumb.
 *
 * The depth is the module's business: it fills the levels the map really shows
 * — the chapter with its number, the verse with its text — so an empty string
 * means "not at that depth yet", never "unknown".
 */
export interface Focus {
	section: string;
	book: string;
	chapter: string;
	/** Number of the verse, in Hebrew letters. Empty above the reading zoom. */
	verse: string;
	/** True when it comes from the pointer rather than the centre of the view. */
	hovered: boolean;
}

/** A place to hand between the map and the book view. */
export interface Place {
	chapter: number;
	/** Global index of the verse, or -1 for none. */
	verse: number;
}

/**
 * The book view's side of the module: the chapters as text, read as they
 * land, and fetched ahead of the whole text when the reader is there first.
 */
export interface Book {
	readonly count: number;
	/** Bytes of a chapter's verses, landed or not. */
	size(chapter: number): number;
	/** First chapters of the books: where a book's name is set. */
	readonly opens: ReadonlySet<number>;
	/** The chapter, or `null` while its verses are not in. */
	read(chapter: number): Chapter | null;
	/** Fetches the chapters of `from..to` not in yet; `onLanded` says when. */
	want(from: number, to: number): void;
	/** The portions, books and chapters' numbers: in from the start. */
	readonly contents: Contents;
	/**
	 * What a click in the book selects: the verse of global index `verse`,
	 * the map's selection, named the same way. `null` for a verse not in.
	 */
	select(verse: number): Selection | null;
	/** The slots of the shown searches holding a portion (0), a book (1) or a chapter (2). */
	marks(level: 0 | 1 | 2, index: number): number[];
}

export interface TanakhMap {
	/** Asks for a frame again. No effect if one is already scheduled. */
	draw(): void;
	/** Back to the overview. */
	reset(): void;
	/**
	 * Carries out a key's command (`keys.ts`) at the centre of the view; a
	 * selection it makes is told through `onSelect`, as a click's is.
	 */
	key(command: Command): void;
	/** Detaches the listeners and releases the resize observer. */
	destroy(): void;
	/**
	 * ELS across the whole Tanakh: `query` read with `skip0..=skip1` letters
	 * jumped between its letters, its final forms told apart with `exact`. Runs in the search module's worker, so it
	 * never holds a frame; answers once the text has landed. Rejected if the
	 * module failed on it — the next search runs on a new one.
	 */
	search(query: string, skip0: number, skip1: number, exact?: boolean): Promise<Found>;
	/**
	 * First or last letters: `query` read one letter from each word of the
	 * Tanakh, side by side — the `nth` of each, from its end with `fromEnd` —
	 * in order or, with `anyOrder`, in any. Runs where `search` runs, and
	 * answers or fails the same way.
	 */
	searchWords(query: string, options: WordsOptions): Promise<Found>;
	/** Letters of the Tanakh's longest word: the last letter of a word a search can read. */
	readonly longestWord: number;
	/**
	 * The largest skip a word of `letters` letters can be read with in the
	 * whole Tanakh — its first and last letters both in it; 0 for one letter.
	 * Known from the first image, before the text lands.
	 */
	maxSkip(letters: number): number;
	readonly books: number;
	/** Where the view is: the chapter at its centre, the verse once read. */
	here(): Place;
	/** How deep the image goes: 0 the portions alone, 1 the books, 2 the chapters, 3 the text. */
	depth(): number;
	/** Back to the framing, the portions and their books: one notch in from the overview. */
	frameBooks(): void;
	/** Takes the map to the book holding a chapter, its chapters' numbers out. */
	showBook(chapter: number): void;
	/** Takes the map to a chapter at its reading level, the verse at the centre. */
	show(place: Place): void;
	readonly book: Book;
	/** The searches marked on the map, one per slot. */
	readonly marks: Marks;
	/** One search's hits, listed as passages. */
	readonly results: Results;
}

/**
 * A search's hits as passages, for the reader to scroll: set when the list
 * opens, read a page at a time, forgotten when it closes.
 */
export interface Results {
	/** Lists a search's hits, `span` letters each; the number of passages. */
	set(found: Found, span: number): number;
	/** Lists the searches shown together: those holding any, or where two cross. */
	shown(all: boolean): number;
	/** The crossings of the list `shown` set, the widest first. */
	crossings(): Crossing[];
	/** How many passages a search's hits make, `span` letters each, as `set` would list them. */
	count(found: Found, span: number): number;
	/** How many passages the searches shown make: of any, of all. */
	together(): { any: number; all: number };
	/** Passages `from..from + count`; fewer at the end. */
	page(from: number, count: number): Passage[];
	clear(): void;
}

/**
 * A search's hits marked on the map: its letters inked in the text, a square by
 * each chapter, book and portion holding them — in the colour it is given.
 */
export interface Marks {
	readonly slots: number;
	/** Marks a search's hits in `slot`, `span` letters each, shown in `color`. */
	set(slot: number, found: Found, span: number, color: Color): void;
	color(slot: number, color: Color): void;
	clear(slot: number): void;
	/** Draws the searches shown joined by any (∪), each whole, or by all (∩), each only where every one is. */
	join(all: boolean): void;
}

export type { Found, Passage, WordsOptions };

export interface MapOptions {
	/** Called on click: the verse hit, or `null` if the click falls on nothing. */
	onSelect?: (selection: Selection | null) => void;
	/**
	 * Called on every frame: where one is. The title bands stay inside the map
	 * and therefore never cover anything; when they leave the screen, it is the
	 * breadcrumb that says where one is.
	 */
	onFocus?: (focus: Focus | null) => void;
	/**
	 * Called when the answer changes for what the pointer names: 1 when the
	 * chapter, the book or the portion under the cursor holds a reading of the
	 * other edition, 0 otherwise. The page shows the index under the canvas on
	 * that alone.
	 */
	onReadings?: (count: number) => void;
	/**
	 * A fault the user should hear of — the map stopped, a search failed.
	 * Every fault is reported to the log whether shown or not; those the page
	 * lives through degrade in silence.
	 */
	onFault?: (fault: Fault) => void;
	/** Verses have landed — a tile, a book, or the readings last. */
	onLanded?: () => void;
}

export async function createTanakhMap(
	canvas: HTMLCanvasElement,
	options: MapOptions = {}
): Promise<TanakhMap> {
	const { onSelect, onFocus, onReadings, onFault, onLanded } = options;

	const context = canvas.getContext('2d', { alpha: false });
	if (!context) throw new Error('canvas 2d unavailable');
	const ctx = context;

	// The module downloads while the font loads. The text waits for the first
	// image: the overview does not need it, and fetched alongside it took
	// bandwidth from what the first image waits for — a priority hint does not
	// stop that over several connections. Measured, the map came sooner and
	// the text no later: the pipe is full either way.
	const loading = WebAssembly.instantiateStreaming(fetch(moduleUrl), {});
	let mapDrawn: () => void = () => {};
	const firstImage = new Promise<void>((drawn) => (mapDrawn = drawn));

	// The Hebrew font must be loaded **before** the first measurement: a width
	// taken on a fallback font would be cached and wrong for good. Only that
	// one: the interface font has nothing to do with the canvas, and waiting
	// for every font held the first image for nothing.
	if (document.fonts) {
		try {
			await document.fonts.load(`16px ${HEBREW_FONT}`, `א${MARKS}`);
		} catch {
			// Font unavailable: the system fallback will do.
		}
	}

	const { module, instance } = await loading;
	const wasm = instance.exports as unknown as Wasm;
	// A trap in the map module is traced to the Rust panic that caused it.
	const unwatch = readTraps(() => {
		const length = wasm.fault_len();
		if (length === 0) return null;
		return new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, wasm.fault_ptr(), length));
	});

	/** Reported always; told to the user when the code says so. */
	function told(raised: Fault): Fault {
		reporter.report(raised);
		if (raised.show) onFault?.(raised);
		return raised;
	}
	/** Raised here, from whatever was thrown. */
	function fault(cause: unknown, code: Parameters<typeof escalate>[1], what: string): Fault {
		return told(escalate(cause, code, what));
	}

	// If the final font arrives after the first measurements, those were taken on
	// a fallback font: they must be forgotten, otherwise the text stays badly
	// framed for the whole session.
	document.fonts?.ready.then(() => {
		wasm.clear_ratios();
		draw();
	});

	const decoder = new TextDecoder();
	const decoded = new Map<number, string>();

	/** Corpus fragment, read straight from the module's memory. */
	function fragment(offset: number, length: number): string {
		const key = fragmentKey(offset, length);
		let text = decoded.get(key);
		if (text === undefined) {
			const bytes = new Uint8Array(wasm.memory.buffer, wasm.text_ptr() + offset, length);
			text = decoder.decode(bytes);
			decoded.set(key, text);
		}
		return text;
	}

	// --------------------------------------------------------------- drawing

	// The colour each slot's search wears.
	const markColors = new Map<number, Color>();

	let width = 0;
	let height = 0;
	let dpr = 1;

	function measure() {
		const rect = canvas.getBoundingClientRect();
		dpr = Math.min(window.devicePixelRatio || 1, 2);
		width = Math.max(1, rect.width);
		height = Math.max(1, rect.height);
		canvas.width = Math.round(width * dpr);
		canvas.height = Math.round(height * dpr);
	}

	// Marks a benchmark — or a real visitor's timing — can read: the first
	// image on screen (`map`), the book nearest to the view landed (`near`),
	// the whole text landed (`text`), the first verse drawn (`verse`).
	let painted = false;
	let versed = false;

	function paint() {
		const words = wasm.build_frame();
		const stride = wasm.frame_stride();
		const commands = new Float32Array(wasm.memory.buffer, wasm.frame_ptr(), words);

		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.fillStyle = '#000';
		ctx.fillRect(0, 0, width, height);

		// The browser lays the text on its right edge and applies bidi.
		ctx.textAlign = 'right';
		ctx.textBaseline = 'alphabetic';
		ctx.direction = 'rtl';

		// The colours of the slots an ink names, as bits past `INK_MARK`.
		const inked = (ink: number): Color[] =>
			slotsOf(ink - INK_MARK).flatMap((slot) => markColors.get(slot) ?? []);

		// A square by a name, in its search's colour: one slot.
		const square = (i: number) => {
			const [color] = inked(commands[i + 9]);
			if (!color) return;
			ctx.fillStyle = markPaint(color, commands[i + 5]);
			const [x, y, w, h] = [commands[i + 1], commands[i + 2], commands[i + 3], commands[i + 4]];
			// Rounded as the panel's swatch; sharp where the browser has no
			// `roundRect`.
			if (!ctx.roundRect) return ctx.fillRect(x, y, w, h);
			ctx.beginPath();
			ctx.roundRect(x, y, w, h, w * SQUARE_ROUND);
			ctx.fill();
		};

		// The reading under the pointer, asked of the frame just built: only its
		// runs wear the colour, and only while it is pointed at.
		const lit = litReading();

		// Open clip depth, so it can be closed cleanly at the end of the frame if
		// a command were missing.
		let clips = 0;

		for (let i = 0; i < words; i += stride) {
			const kind = commands[i];
			const x = commands[i + 1];
			const y = commands[i + 2];
			const w = commands[i + 3];
			const h = commands[i + 4];

			// A chapter's text is locked inside its box: a badly measured width
			// would otherwise make it bite into its neighbours.
			if (kind === KIND_CLIP) {
				ctx.save();
				ctx.beginPath();
				ctx.rect(x, y, w, h);
				ctx.clip();
				clips++;
				continue;
			}

			if (kind === KIND_UNCLIP) {
				if (clips > 0) {
					ctx.restore();
					clips--;
				}
				continue;
			}

			if (kind === KIND_SQUARE) {
				square(i);
				continue;
			}

			// Fill without a frame: boxes are drawn by their tint alone.
			if (kind === KIND_RECT) {
				const fill = commands[i + 5];
				if (fill > 0.003) {
					ctx.fillStyle = `rgba(255,255,255,${fill})`;
					ctx.fillRect(x, y, w, h);
				}
				continue;
			}

			// The last slot says what the run is: everything is white but the
			// other edition's reading, which the module cut out as its own run —
			// and that one wears its colour only while either form is pointed at
			// — and the letters the searches found, in their colours: bands of
			// each, top to bottom, where several found the same letters.
			const offset = commands[i + 7];
			const ink = commands[i + 9];
			const alpha = commands[i + 5];
			const marked = ink >= INK_MARK ? inked(ink) : [];
			const tint =
				ink > 0 && ink < INK_MARK && offset >= lit.from && offset < lit.to
					? OTHER_READING
					: '255,255,255';
			const text = fragment(offset, commands[i + 8]);
			ctx.font = `${commands[i + 6]}px ${HEBREW_FONT}`;
			if (marked.length > 1) {
				// The glyphs' full height, as the book view's markup paints it.
				const m = ctx.measureText(text);
				const fill = ctx.createLinearGradient(
					0,
					y - m.fontBoundingBoxAscent,
					0,
					y + m.fontBoundingBoxDescent
				);
				for (const [at, color] of bands(marked)) fill.addColorStop(at, markPaint(color, alpha));
				ctx.fillStyle = fill;
			} else {
				ctx.fillStyle = marked.length ? markPaint(marked[0], alpha) : `rgba(${tint},${alpha})`;
			}
			ctx.fillText(text, x, y);
		}

		while (clips > 0) {
			ctx.restore();
			clips--;
		}

		reportFocus();
		reportReadings(lit.to > 0 ? 1 : 0);
		measurePending();
	}

	/**
	 * The other reading under the pointer, as a byte range of the text; empty
	 * with no pointer on the canvas — a touch screen or the cursor gone — since
	 * there is then nothing to explain.
	 */
	function litReading(): { from: number; to: number } {
		const start = pointer === null ? 0 : wasm.readings_at(pointer.x, pointer.y);
		return start === 0 ? { from: 0, to: 0 } : { from: start - 1, to: wasm.readings_end() };
	}

	let lastReadings = -1;
	function reportReadings(here: number) {
		if (!onReadings) return;

		// The index follows the pointer, not the frame: it opens on the two
		// forms under the cursor, not on whatever happens to be on screen.
		if (here === lastReadings) return;
		lastReadings = here;
		onReadings(here);
	}

	let lastFocus = '';
	function reportFocus() {
		if (!onFocus) return;

		// The pointer has priority over the centre of the view: one reads the
		// breadcrumb to know what one is pointing at. With no pointer, the
		// module answers for the centre — or for nothing, if a mouse left
		// (`leave`).
		const hovered = pointer !== null && wasm.focus_at(pointer.x, pointer.y) === 1;
		if (!hovered && pointer === null) wasm.focus();

		const view = new Uint32Array(wasm.memory.buffer, wasm.focus_ptr(), 8);
		const read = (i: number) => (view[i + 1] ? fragment(view[i], view[i + 1]) : '');

		const section = read(0);
		if (!section) {
			if (lastFocus) {
				lastFocus = '';
				onFocus(null);
			}
			return;
		}

		const next: Focus = {
			section,
			book: read(2),
			chapter: read(4),
			verse: read(6),
			hovered
		};
		const key = `${next.section}|${next.book}|${next.chapter}|${next.verse}|${next.hovered}`;
		if (key === lastFocus) return;

		lastFocus = key;
		onFocus(next);
	}

	/**
	 * Rust first draws with an estimate, then records the fragments whose width
	 * it does not know. We measure them and ask for a frame again: it will be
	 * right. Each fragment is measured only once.
	 */
	function measurePending() {
		const count = wasm.pending_len();
		if (count === 0) return;

		// Copy: `set_ratio` may grow the module's memory and invalidate the view.
		const pairs = Array.from(new Uint32Array(wasm.memory.buffer, wasm.pending_ptr(), count));

		ctx.font = `${REFERENCE}px ${HEBREW_FONT}`;
		for (let i = 0; i < pairs.length; i += 2) {
			const offset = pairs[i];
			const length = pairs[i + 1];
			const ratio = ctx.measureText(fragment(offset, length)).width / REFERENCE;
			wasm.set_ratio(offset, length, ratio);
		}

		draw();
	}

	// Tiles: after each frame the module says which chapters the view is about
	// to read without their verses in; each run is fetched by itself — a few
	// kilobytes — and lands as soon as it arrives, ahead of the whole text.
	function fetchTiles() {
		const runs = wasm.tiles_wanted();
		if (runs === 0) return;
		const asked = Array.from(new Uint32Array(wasm.memory.buffer, wasm.tiles_ptr(), runs * 4));
		for (let i = 0; i < asked.length; i += 4) {
			const [first, end, from, to] = asked.slice(i, i + 4);
			retried(() => range(plainUrl, from, to, 'high'))
				.then((bytes) => {
					const at = wasm.tile_buffer(bytes.length);
					// A view taken after the call: the allocation may grow the memory.
					new Uint8Array(wasm.memory.buffer, at, bytes.length).set(bytes);
					wasm.tile_load(first, end);
					draw();
					onLanded?.();
				})
				.catch((cause) =>
					fault(cause, 'tile-unfetched', `fetching the tile of chapters ${first}..${end}`)
				);
		}
	}

	// A place the book view handed back: shown again once the frame has
	// measured what it asked, since the widths move the lines it centred on.
	let showing: Place | null = null;

	let frame: number | null = null;
	// Once a frame has failed the module may be half-written: the map stops,
	// its user is told, and nothing is drawn again — no error in a loop.
	let broken = false;
	function draw() {
		if (broken) return;
		frame ??= requestAnimationFrame(() => {
			frame = null;
			try {
				paint();
			} catch (cause) {
				broken = true;
				fault(cause, 'map-broken', 'drawing a frame');
				return;
			}
			if (!painted) {
				painted = true;
				performance.mark('ethaor:map');
				mapDrawn();
			}
			fetchTiles();
			if (showing && wasm.pending_len() === 0) {
				wasm.show(showing.chapter, showing.verse);
				showing = null;
				draw();
			}
			if (!versed && wasm.probe_verse_drawn() === 1) {
				versed = true;
				performance.mark('ethaor:verse');
			}
		});
	}

	// --------------------------------------------------------------- gestures

	const pointers = new Map<number, { x: number; y: number }>();
	let pinch = 0;
	let moved = false;

	/**
	 * Last position of the pointer on the canvas, in CSS pixels, or `null` when
	 * it is gone. It is what the breadcrumb is read at: hovering a title, a
	 * chapter or a line of verse says where **that** is, not where the view is.
	 */
	let pointer: { x: number; y: number } | null = null;

	const local = (event: PointerEvent) => {
		const rect = canvas.getBoundingClientRect();
		return { x: event.clientX - rect.left, y: event.clientY - rect.top };
	};

	const spread = () => {
		const [a, b] = [...pointers.values()];
		return Math.hypot(a.x - b.x, a.y - b.y);
	};

	const midpoint = () => {
		const [a, b] = [...pointers.values()];
		return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
	};

	function onPointerDown(event: PointerEvent) {
		canvas.setPointerCapture(event.pointerId);
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
		moved = false;
		if (pointers.size === 2) pinch = spread();
	}

	function onPointerMove(event: PointerEvent) {
		pointer = local(event);

		if (!pointers.has(event.pointerId)) {
			canvas.style.cursor = wasm.hover(pointer.x, pointer.y) < 0 ? 'default' : 'pointer';
			draw();
			return;
		}

		const previous = pointers.get(event.pointerId)!;
		pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });

		if (pointers.size === 2) {
			const now = spread();
			if (pinch > 0 && now > 0) {
				const rect = canvas.getBoundingClientRect();
				const mid = midpoint();
				wasm.zoom(mid.x - rect.left, mid.y - rect.top, now / pinch);
			}
			pinch = now;
			moved = true;
			draw();
			return;
		}

		const dx = event.clientX - previous.x;
		const dy = event.clientY - previous.y;
		if (Math.abs(dx) + Math.abs(dy) > 3) moved = true;
		wasm.pan(dx, dy);
		draw();
	}

	/**
	 * The pointer has left. A finger lifted hands the breadcrumb back to the
	 * centre of the view; a mouse gone elsewhere empties it.
	 */
	function onPointerLeave(event: PointerEvent) {
		pointer = null;
		wasm.leave(event.pointerType === 'mouse' ? 1 : 0);
		draw();
	}

	function onPointerUp(event: PointerEvent) {
		if (!pointers.has(event.pointerId)) return;
		const single = pointers.size === 1;
		pointers.delete(event.pointerId);
		pinch = pointers.size === 2 ? spread() : 0;

		if (single && !moved) {
			const { x, y } = local(event);
			onSelect?.(selectionAt(x, y));
		}
		draw();
	}

	function onWheel(event: WheelEvent) {
		event.preventDefault();
		const rect = canvas.getBoundingClientRect();
		// The factor of one notch comes from the module: it is worth whatever it
		// takes for the dive from the framing down to a chapter's text to always
		// need the same number of notches, whatever the window. Around 1.4 on a
		// landscape screen. The trackpad sends fractions of a notch, so fine
		// tuning stays possible.
		const steps = event.deltaMode === 1 ? -event.deltaY : -event.deltaY / 100;
		pointer = { x: event.clientX - rect.left, y: event.clientY - rect.top };
		wasm.zoom(pointer.x, pointer.y, Math.pow(wasm.zoom_step(), steps));
		draw();
	}

	/**
	 * What a click selected: the passage. Its words are picked in the passage
	 * itself, once it is open.
	 *
	 * Strings are cut by **byte** offsets, read straight from the module's
	 * memory: slicing the decoded string would be wrong, since one Hebrew
	 * letter with its marks is several UTF-16 units.
	 */
	function selectionAt(x: number, y: number): Selection | null {
		return selected(wasm.select(x, y));
	}

	/** The selection the module just filled, if `hit` says it did. */
	function selected(hit: number): Selection | null {
		if (hit === 0) return null;

		const view = new Uint32Array(wasm.memory.buffer, wasm.selection_ptr(), 6);
		const [book, chapter, offset, length, label, labelSize] = view;
		if (length === 0) return null;
		const marks = new Uint32Array(
			wasm.memory.buffer,
			wasm.selection_marks_ptr(),
			wasm.selection_marks_len() * 3
		);
		const found: Inked[] = [];
		for (let i = 0; i < marks.length; i += 3) {
			found.push({ from: marks[i], to: marks[i + 1], slots: marks[i + 2] });
		}

		return {
			book,
			chapter,
			number: fragment(label, labelSize),
			verse: fragment(offset, length),
			word: '',
			head: '',
			tail: '',
			found
		};
	}

	/**
	 * A key, read for the centre of the view. The keyboard speaks for the
	 * centre as a lifted finger does: the breadcrumb then names where the keys
	 * went, not where a resting cursor was left. The next move of the mouse
	 * takes it back.
	 */
	function onKey(command: Command) {
		pointer = null;
		wasm.leave(0);
		if (command.kind === 'zoom') {
			wasm.zoom(width / 2, height / 2, Math.pow(wasm.zoom_step(), command.steps));
		} else if (command.kind === 'pan') {
			// `pan` moves the world; the view goes the other way.
			wasm.pan(-command.dx * width, -command.dy * height);
		} else {
			onSelect?.(selectionAt(width / 2, height / 2));
		}
		draw();
	}

	// The portions' names at the opening take the book view's size, read off
	// its own classes: a breakpoint crossed changes it.
	const portions = document.createElement('span');
	portions.className = PORTION_TEXT;
	portions.style.cssText = 'position:absolute;visibility:hidden;pointer-events:none';
	portions.setAttribute('aria-hidden', 'true');
	document.body.append(portions);
	function portionSize() {
		wasm.set_portion_size(parseFloat(getComputedStyle(portions).fontSize));
	}

	function onResize() {
		measure();
		portionSize();
		wasm.resize(width, height);
		draw();
	}

	// ----------------------------------------------------------------- start

	measure();
	const books = wasm.init(width, height);
	portionSize();

	canvas.addEventListener('pointerdown', onPointerDown);
	canvas.addEventListener('pointermove', onPointerMove);
	canvas.addEventListener('pointerup', onPointerUp);
	canvas.addEventListener('pointercancel', onPointerUp);
	canvas.addEventListener('pointerleave', onPointerLeave);
	canvas.addEventListener('wheel', onWheel, { passive: false });

	const observer = new ResizeObserver(onResize);
	observer.observe(canvas);

	draw();

	// The whole text is in once every book and the readings have landed. Should
	// part of it never come, the map stays whole — at the level of the
	// numbers, but for the chapters that did.
	let textLanded: () => void = () => {};
	const textIn = new Promise<void>((done) => (textLanded = done));
	// The search module runs in a worker, started once the first image is
	// drawn: no search — up to a second over every skip — ever holds a frame.
	// Should no worker be available, it runs here.
	//
	// The text comes as Google Earth's world does: what the visitor looks at
	// first. Once the first image is drawn, books are fetched one after the
	// other in the background, each time the one nearest to the view
	// (`next_book`), at low priority; a fast dive meanwhile asks for its
	// chapters' tiles at high priority (`fetchTiles`). Each book lands here and
	// in the search module; the readings last, and the whole text is in.
	let ask: (request: Request) => void = () => {};
	let here: Promise<SearchCore> | null = null;
	// Each search waiting for its answer — or its failure: a search the
	// module could not run is rejected, never left pending.
	const searches = new Map<
		number,
		{ done: (found: Found) => void; fail: (cause: Error) => void }
	>();
	let next = 0;
	function asked(request: (id: number) => Request): Promise<Found> {
		return new Promise((done, fail) => {
			const id = next++;
			searches.set(id, { done, fail });
			ask(request(id));
		});
	}
	const search = (query: string, skip0: number, skip1: number, exact = false) =>
		asked((id) => ({ els: { id, query, skip0, skip1, exact } }));
	const searchWords = (query: string, options: WordsOptions) =>
		asked((id) => ({ words: { id, query, options } }));
	function landHere(first: number, end: number, codes: Uint8Array) {
		const at = wasm.tile_buffer(codes.length);
		// A view taken after the call: the allocation may grow the memory.
		new Uint8Array(wasm.memory.buffer, at, codes.length).set(codes);
		wasm.tile_load(first, end);
		draw();
		onLanded?.();
	}
	firstImage
		.then(async () => {
			let worker: Worker | null = null;
			try {
				worker = new SearchWorker();
			} catch {
				worker = null;
			}
			if (worker) {
				worker.onmessage = (event: MessageEvent<Answer>) => {
					const message = event.data;
					if ('found' in message) searches.get(message.id)?.done(message.found);
					else if (message.id !== undefined) {
						const failed = Fault.from(message.fault);
						searches.get(message.id)?.fail(told(relay(failed, "answering the page's search")));
					} else told(relay(Fault.from(message.fault), 'the worker preparing the text'));
					if (message.id !== undefined) searches.delete(message.id);
				};
				ask = (request) => worker!.postMessage(request);
			} else {
				here = SearchCore.create();
				ask = (request) => {
					let id: number;
					let run: (core: SearchCore) => Found;
					if ('els' in request) {
						const { query, skip0, skip1, exact } = request.els;
						id = request.els.id;
						run = (core) => core.els(query, skip0, skip1, exact);
					} else if ('words' in request) {
						const { query, options } = request.words;
						id = request.words.id;
						run = (core) => core.words(query, options);
					} else return;
					// Searches wait for the text, as in the worker.
					textIn
						.then(() => here!)
						.then((core) => searches.get(id)?.done(run(core)))
						.catch((cause) =>
							searches.get(id)?.fail(fault(cause, 'search-failed-here', 'a search in the page'))
						)
						.finally(() => searches.delete(id));
				};
			}

			// Books in flight at once: one at a time paid a full round trip per
			// book — forty on a 3G link — so a few travel together, each still
			// the nearest to the view when its turn comes.
			const block = async (i: number) =>
				inflate(await retried(() => range(booksUrl, bookCuts[i], bookCuts[i + 1], 'low')));
			await lanes(BOOKS_IN_FLIGHT, wasm.next_book, async (book) => {
				const codes = await block(book);
				const [first, end] = [wasm.book_first_chapter(book), wasm.book_end_chapter(book)];
				landHere(first, end, codes);
				if (!performance.getEntriesByName('ethaor:near').length) performance.mark('ethaor:near');
				if (worker) ask({ tile: { first, end, codes } });
				else (await here!).tile(first, end, codes);
			});
			const readings = await block(bookCuts.length - 2);
			const atReadings = wasm.tile_buffer(readings.length);
			new Uint8Array(wasm.memory.buffer, atReadings, readings.length).set(readings);
			if (wasm.readings_load() === 1) {
				textLanded();
				performance.mark('ethaor:text');
				draw();
				onLanded?.();
			}
			if (worker) ask({ readings });
			else (await here!).readings(readings);
		})
		.catch((cause) => fault(cause, 'text-unfetched', 'fetching the books in the background'));

	// The book view's chapters: read from the module's record as they land,
	// and fetched as tiles when the reader is there before the books.
	const opens = new Set<number>();
	for (let b = 0; b < books; b++) opens.add(wasm.book_first_chapter(b));
	const askedChapters = new Set<number>();
	const contentsWords = wasm.contents_record();
	const contents = readContents(
		new Uint32Array(wasm.memory.buffer, wasm.contents_ptr(), contentsWords).slice(),
		(offset, length) =>
			decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.text_ptr() + offset, length))
	);
	const book: Book = {
		count: wasm.chapter_count(),
		opens,
		contents,
		size: (chapter) => wasm.chapter_size(chapter),
		select: (verse) => selected(wasm.select_verse(verse)),
		marks(level, index) {
			return slotsOf(wasm.marks_at(level, index));
		},
		read(chapter) {
			const words = wasm.chapter_record(chapter);
			if (words === 0) return null;
			// Copied: a later record is written over this one.
			const record = new Uint32Array(wasm.memory.buffer, wasm.chapter_record_ptr(), words).slice();
			// Decoded afresh, not through `fragment`'s cache: a verse is read
			// once per landing here, and the cache would end up holding the
			// whole text.
			const text = (offset: number, length: number) =>
				decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.text_ptr() + offset, length));
			// The letters the searches shown found here, cut out with their slots.
			const n = wasm.chapter_marks(chapter);
			const marks = new Uint32Array(wasm.memory.buffer, wasm.chapter_marks_ptr(), n * 3);
			const found: BookFound[] = [];
			for (let i = 0; i < marks.length; i += 3) {
				found.push({ from: marks[i], to: marks[i + 1], slots: marks[i + 2] });
			}
			return readChapter(chapter, record, text, found);
		},
		want(from, to) {
			const runs = missing(from, to, (c) => askedChapters.has(c) || wasm.chapter_landed(c) === 1);
			for (const [first, end] of runs) {
				for (let c = first; c < end; c++) askedChapters.add(c);
				const [start, stop] = [
					wasm.chapter_tile_from(first, end),
					wasm.chapter_tile_to(first, end)
				];
				retried(() => range(plainUrl, start, stop, 'high'))
					.then((bytes) => landHere(first, end, bytes))
					.catch((cause) => {
						// Asked again when the reader comes back to them.
						for (let c = first; c < end; c++) askedChapters.delete(c);
						fault(cause, 'book-unfetched', `fetching chapters ${first}..${end} for the book view`);
					});
			}
		}
	};

	// How the module reads a search's records: 0 ELS, 1 each letter's place.
	const formOf = (found: Found) => (found.form === 'letters' ? 1 : 0);

	const results: Results = {
		set(found, span) {
			const at = wasm.marks_buffer(found.hits.length);
			// A view taken after the call: the allocation may grow the memory.
			new Uint32Array(wasm.memory.buffer, at, found.hits.length).set(found.hits);
			return wasm.results_set(span, formOf(found));
		},
		shown: (all) => wasm.results_shown(all ? 1 : 0),
		crossings() {
			const n = wasm.results_groups();
			return readCrossings(new Uint32Array(wasm.memory.buffer, wasm.results_groups_ptr(), 2 * n));
		},
		count(found, span) {
			const at = wasm.marks_buffer(found.hits.length);
			new Uint32Array(wasm.memory.buffer, at, found.hits.length).set(found.hits);
			return wasm.passages_count(span, formOf(found));
		},
		together: () => ({ any: wasm.passages_shown(0), all: wasm.passages_shown(1) }),
		page(from, count) {
			const words = wasm.results_page(from, count);
			const record = new Uint32Array(wasm.memory.buffer, wasm.results_ptr(), words).slice();
			// Decoded afresh, as a chapter's verses: the list may run long.
			return readResults(record, (offset, length) =>
				decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.text_ptr() + offset, length))
			);
		},
		clear: () => wasm.results_clear()
	};

	const marks: Marks = {
		slots: wasm.marks_slots(),
		set(slot, found, span, color) {
			const at = wasm.marks_buffer(found.hits.length);
			// A view taken after the call: the allocation may grow the memory.
			new Uint32Array(wasm.memory.buffer, at, found.hits.length).set(found.hits);
			wasm.marks_set(slot, span, formOf(found));
			markColors.set(slot, color);
			draw();
		},
		color(slot, color) {
			markColors.set(slot, color);
			draw();
		},
		clear(slot) {
			wasm.marks_clear(slot);
			markColors.delete(slot);
			draw();
		},
		join(all) {
			wasm.marks_join(all ? 1 : 0);
			draw();
		}
	};

	return {
		books,
		book,
		marks,
		results,
		search,
		searchWords,
		longestWord: wasm.longest_word(),
		maxSkip: (letters) => wasm.skip_max(letters),
		here: () => ({ chapter: wasm.here_chapter(), verse: wasm.here_verse() }),
		depth: () => wasm.here_depth(),
		frameBooks() {
			wasm.frame_books();
			draw();
		},
		showBook(chapter) {
			wasm.show_book(chapter);
			draw();
		},
		show(place) {
			wasm.show(place.chapter, place.verse);
			showing = place;
			draw();
		},
		draw,
		reset() {
			wasm.reset();
			draw();
		},
		key: onKey,
		destroy() {
			observer.disconnect();
			portions.remove();
			canvas.removeEventListener('pointerdown', onPointerDown);
			canvas.removeEventListener('pointermove', onPointerMove);
			canvas.removeEventListener('pointerup', onPointerUp);
			canvas.removeEventListener('pointercancel', onPointerUp);
			canvas.removeEventListener('pointerleave', onPointerLeave);
			canvas.removeEventListener('wheel', onWheel);
			if (frame !== null) cancelAnimationFrame(frame);
			unwatch();
		}
	};
}
