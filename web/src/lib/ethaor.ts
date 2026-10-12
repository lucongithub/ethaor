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

export interface TanakhMap {
	/** Asks for a frame again. No effect if one is already scheduled. */
	draw(): void;
	/** Back to the overview. */
	reset(): void;
	/** Detaches the listeners and releases the resize observer. */
	destroy(): void;
	readonly books: number;
}

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
	/** Path to the module, prefixed by `base` in case of a subdirectory. */
	wasmUrl?: string;
}

export async function createTanakhMap(
	canvas: HTMLCanvasElement,
	options: MapOptions = {}
): Promise<TanakhMap> {
	const { onSelect, onFocus, onReadings, wasmUrl = '/ethaor.wasm' } = options;

	const context = canvas.getContext('2d', { alpha: false });
	if (!context) throw new Error('canvas 2d indisponible');
	const ctx = context;

	// The font must be loaded **before** the first measurement: a width taken on
	// a fallback font would be cached and wrong for good.
	if (document.fonts) {
		try {
			await document.fonts.load(`16px ${HEBREW_FONT}`, `א${MARKS}`);
			await document.fonts.ready;
		} catch {
			// Font unavailable: the system fallback will do.
		}
	}

	const { instance } = await WebAssembly.instantiateStreaming(fetch(wasmUrl), {});
	const wasm = instance.exports as unknown as Wasm;

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
		const key = offset * 1024 + length;
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
		// breadcrumb to know what one is pointing at. With no pointer on the
		// canvas — a touch screen, or the cursor gone — the module answers for
		// the centre, which is what the zoom alone says.
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

	let frame: number | null = null;
	function draw() {
		frame ??= requestAnimationFrame(() => {
			frame = null;
			paint();
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

	/** The pointer has left: the breadcrumb goes back to the centre of the view. */
	function onPointerLeave() {
		pointer = null;
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
				now - lastClick.at < DOUBLE_MS &&
				Math.hypot(x - lastClick.x, y - lastClick.y) < DOUBLE_PX;

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

	return {
		books,
		draw,
		reset() {
			wasm.reset();
			draw();
		},
		destroy() {
			observer.disconnect();
			canvas.removeEventListener('pointerdown', onPointerDown);
			canvas.removeEventListener('pointermove', onPointerMove);
			canvas.removeEventListener('pointerup', onPointerUp);
			canvas.removeEventListener('pointercancel', onPointerUp);
			canvas.removeEventListener('pointerleave', onPointerLeave);
			canvas.removeEventListener('wheel', onWheel);
			if (frame !== null) cancelAnimationFrame(frame);
		}
	};
}
