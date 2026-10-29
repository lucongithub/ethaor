// The keyboard's part of the map: which key does what. It only decides; the
// bridge (`ethaor.ts`) carries the decision out on the module, as it does a
// gesture's.

/** What a key asks of the map, or nothing. */
export type Command =
	/** Notches of zoom at the centre of the view: positive in, negative out. */
	| { kind: 'zoom'; steps: number }
	/** How far the view moves, as shares of its width and height: positive
	 *  right and down. */
	| { kind: 'pan'; dx: number; dy: number }
	/** The passage at the centre of the view, as a click there would give it. */
	| { kind: 'select' };

/** What Escape closes, in order: the nearest thing first. `none`: nothing
 *  left to close, and no overview to go back to — the book view's. */
export type Escape = 'info' | 'results' | 'selection' | 'notice' | 'searches' | 'overview' | 'none';

/**
 * Share of the view one arrow moves it by, and with Shift held. A small step
 * keeps the eye on the line it reads; Shift crosses a column at once.
 */
export const PAN = 1 / 8;
export const PAN_FAR = 1 / 2;

interface Key {
	key: string;
	shiftKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
	altKey: boolean;
}

/**
 * The command a key gives the map. Any key held with Ctrl, Cmd or Alt is the
 * browser's or the system's — Ctrl + zooms the page, not the map — and is left
 * to them.
 */
export function command(event: Key): Command | null {
	if (event.ctrlKey || event.metaKey || event.altKey) return null;
	const step = event.shiftKey ? PAN_FAR : PAN;
	switch (event.key) {
		case '+':
		case '=':
			return { kind: 'zoom', steps: 1 };
		case '-':
		case '_':
			return { kind: 'zoom', steps: -1 };
		case 'ArrowLeft':
			return { kind: 'pan', dx: -step, dy: 0 };
		case 'ArrowRight':
			return { kind: 'pan', dx: step, dy: 0 };
		case 'ArrowUp':
			return { kind: 'pan', dx: 0, dy: -step };
		case 'ArrowDown':
			return { kind: 'pan', dx: 0, dy: step };
		case 'Enter':
			return { kind: 'select' };
		default:
			return null;
	}
}

/**
 * What Escape does. **It closes before it leaves**: an open passage or
 * notice is closed and the view stays where the reader dived — the
 * passage first, which may lie over a search's list of passages, then that
 * list, lying over everything else. Only with
 * nothing open does it go back to the overview — closing a passage used to
 * throw away a dive of ten notches. The searches' panel is closed only
 * where it lies over the view (`searches`, a narrow screen): beside it, it
 * covers nothing and stays. In the book view there is no overview: the
 * reader's place is the scroll, and Escape never throws it away. What a
 * search did (`info`), opened from the panel over everything, closes first.
 */
export function escape(open: {
	info?: boolean;
	results?: boolean;
	selection: boolean;
	notice: boolean;
	searches?: boolean;
	book?: boolean;
}): Escape {
	if (open.info) return 'info';
	if (open.selection) return 'selection';
	if (open.results) return 'results';
	if (open.notice) return 'notice';
	if (open.searches) return 'searches';
	return open.book ? 'none' : 'overview';
}
