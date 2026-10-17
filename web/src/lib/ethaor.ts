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
import { SearchCore, type Found } from './search-core';
import { fragmentKey, inflate, lanes, range } from './text-open';
import type { Command } from './keys';
import { missing, readChapter, type Chapter } from './book-view';

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
 * other publication of the edition, inserted after ours. Everything else is a
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
	select_word(x: number, y: number): number;
	selection_ptr(): number;
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
	/** 1 when what the pointer names holds another reading. */
	readings_at(x: number, y: number): number;
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
	here_chapter(): number;
	here_verse(): number;
	/** Back to a chapter at its reading level, the verse at the centre. */
	show(chapter: number, verse: number): void;
}

/**
 * What a click selected.
 *
 * A click is only live on a passage one can read — the module refuses
 * elsewhere — so a selection always carries its verse. A double click adds the
 * word, with the rest of the verse split around it so the page can show it in
 * place.
 */
export interface Selection {
	book: number;
	chapter: number;
	verse: string;
	/** Word under the point, on a double click only. */
	word: string;
	/** What precedes and follows the word inside the verse. */
	head: string;
	tail: string;
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
	 * jumped between its letters. Runs in the search module's worker, so it
	 * never holds a frame; answers once the text has landed. Rejected if the
	 * module failed on it — the next search runs on a new one.
	 */
	search(query: string, skip0: number, skip1: number): Promise<Found>;
	readonly books: number;
	/** Where the view is: the chapter at its centre, the verse once read. */
	here(): Place;
	/** Takes the map to a chapter at its reading level, the verse at the centre. */
	show(place: Place): void;
	readonly book: Book;
}

export type { Found };

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
			// other edition's reading, which the module cut out as its own run.
			const tint = commands[i + 9] > 0 ? OTHER_READING : '255,255,255';
			ctx.font = `${commands[i + 6]}px ${HEBREW_FONT}`;
			ctx.fillStyle = `rgba(${tint},${commands[i + 5]})`;
			ctx.fillText(fragment(commands[i + 7], commands[i + 8]), x, y);
		}

		while (clips > 0) {
			ctx.restore();
			clips--;
		}

		reportFocus();
		reportReadings();
		measurePending();
	}

	let lastReadings = -1;
	function reportReadings() {
		if (!onReadings) return;

		// The index follows the pointer, not the frame: it explains the
		// ensemble the reader has under the cursor — a chapter, a book, a
		// portion — and not whatever happens to be on screen. With no pointer
		// on the canvas, a touch screen or the cursor gone, there is nothing to
		// explain.
		const here = pointer === null ? 0 : wasm.readings_at(pointer.x, pointer.y);
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
			range(plainUrl, from, to, 'high')
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
	 * Last click that selected something, to tell a double click from two
	 * clicks. The first click already shows the passage; the second one, close
	 * enough in time and in space, refines it down to the word. Nothing is ever
	 * delayed waiting for it.
	 */
	let lastClick = { at: 0, x: 0, y: 0 };
	const DOUBLE_MS = 350;
	const DOUBLE_PX = 24;

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
			const now = event.timeStamp;
			const again =
				now - lastClick.at < DOUBLE_MS && Math.hypot(x - lastClick.x, y - lastClick.y) < DOUBLE_PX;

			const hit = selectionAt(x, y, again);
			onSelect?.(hit);
			lastClick = hit ? { at: now, x, y } : { at: 0, x: 0, y: 0 };
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
	 * What a click selected: the passage, and the word too on a double click.
	 *
	 * The word is split out of the verse by **byte** offsets, read straight from
	 * the module's memory: slicing the decoded string would be wrong, since one
	 * Hebrew letter with its marks is several UTF-16 units.
	 */
	function selectionAt(x: number, y: number, word: boolean): Selection | null {
		const hit = word ? wasm.select_word(x, y) : wasm.select(x, y);
		if (hit === 0) return null;

		const view = new Uint32Array(wasm.memory.buffer, wasm.selection_ptr(), 6);
		const [book, chapter, offset, length, at, size] = view;
		if (length === 0) return null;

		const inside = size > 0 && at >= offset && at + size <= offset + length;
		return {
			book,
			chapter,
			verse: fragment(offset, length),
			word: inside ? fragment(at, size) : '',
			head: inside ? fragment(offset, at - offset) : '',
			tail: inside ? fragment(at + size, offset + length - at - size) : ''
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
			onSelect?.(selectionAt(width / 2, height / 2, false));
		}
		draw();
	}

	function onResize() {
		measure();
		wasm.resize(width, height);
		draw();
	}

	// ----------------------------------------------------------------- start

	measure();
	const books = wasm.init(width, height);

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
	function search(query: string, skip0: number, skip1: number): Promise<Found> {
		return new Promise((done, fail) => {
			const id = next++;
			searches.set(id, { done, fail });
			ask({ els: { id, query, skip0, skip1 } });
		});
	}
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
					if (!('els' in request)) return;
					const { id, query, skip0, skip1 } = request.els;
					// Searches wait for the text, as in the worker.
					textIn
						.then(() => here!)
						.then((core) => searches.get(id)?.done(core.els(query, skip0, skip1)))
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
				inflate(await range(booksUrl, bookCuts[i], bookCuts[i + 1], 'low'));
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
	const book: Book = {
		count: wasm.chapter_count(),
		opens,
		size: (chapter) => wasm.chapter_size(chapter),
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
			return readChapter(chapter, record, text);
		},
		want(from, to) {
			const runs = missing(from, to, (c) => askedChapters.has(c) || wasm.chapter_landed(c) === 1);
			for (const [first, end] of runs) {
				for (let c = first; c < end; c++) askedChapters.add(c);
				const [start, stop] = [
					wasm.chapter_tile_from(first, end),
					wasm.chapter_tile_to(first, end)
				];
				range(plainUrl, start, stop, 'high')
					.then((bytes) => landHere(first, end, bytes))
					.catch((cause) => {
						// Asked again when the reader comes back to them.
						for (let c = first; c < end; c++) askedChapters.delete(c);
						fault(cause, 'book-unfetched', `fetching chapters ${first}..${end} for the book view`);
					});
			}
		}
	};

	return {
		books,
		book,
		search,
		here: () => ({ chapter: wasm.here_chapter(), verse: wasm.here_verse() }),
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
