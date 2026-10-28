/**
 * The searches the page keeps, as many as the reader runs — the owner's
 * rule — listed in the panel on the right. Those **shown** are marked on
 * the map, each in a slot of the module's (`MARK_SLOTS` in
 * `src/map/marks.rs`) and a colour no other shown one wears: its letters in
 * the text, its bars by the names above it. Ten at most are shown at
 * once, every kind together — past that the map reads as noise —
 * and a hidden search holds no slot.
 *
 * The module says which slots a run or a bar belongs to, as bits; which
 * colour each slot wears is decided here.
 */

/** Searches shown at once, every kind together: the module's slots. */
export const MAX_SHOWN = 10;

/** A search shown holds a slot; hidden, it holds this. */
export const HIDDEN = -1;

/**
 * The colours a search may wear: Tailwind's 500 steps, its values exactly —
 * sixteen to pick from, violet left out, too close to its neighbours.
 */
export const COLORS = {
	red: 'oklch(63.7% 0.237 25.331)',
	orange: 'oklch(70.5% 0.213 47.604)',
	amber: 'oklch(76.9% 0.188 70.08)',
	yellow: 'oklch(79.5% 0.184 86.047)',
	lime: 'oklch(76.8% 0.233 130.85)',
	green: 'oklch(72.3% 0.219 149.579)',
	emerald: 'oklch(69.6% 0.17 162.48)',
	teal: 'oklch(70.4% 0.14 182.503)',
	cyan: 'oklch(71.5% 0.143 215.221)',
	sky: 'oklch(68.5% 0.169 237.323)',
	blue: 'oklch(62.3% 0.214 259.815)',
	indigo: 'oklch(58.5% 0.233 277.117)',
	purple: 'oklch(62.7% 0.265 303.9)',
	fuchsia: 'oklch(66.7% 0.295 322.15)',
	pink: 'oklch(65.6% 0.241 354.308)',
	rose: 'oklch(64.5% 0.246 16.439)'
} as const;

export type Color = keyof typeof COLORS;

/**
 * The order new searches take the colours in: sky first, then the next not
 * worn, each far on the wheel from the ones before it, so the first few
 * searches never read as one.
 */
const FIRST: Color[] = [
	'sky',
	'rose',
	'lime',
	'amber',
	'purple',
	'teal',
	'orange',
	'blue',
	'pink',
	'green',
	'yellow',
	'indigo',
	'cyan',
	'red',
	'emerald',
	'fuchsia'
];

/** One search kept. */
export interface Marked {
	/** Its own number, never given twice: what the page names it by. */
	id: number;
	/** The module's slot while it is shown — where its marks are set and
	 *  cleared — `HIDDEN` otherwise. */
	slot: number;
	/** The search's name, as the sidebar's menu gives it. */
	kind: string;
	/** The word asked for, as typed. */
	word: string;
	/** Exact letters (final forms): its finals told apart. */
	exact: boolean;
	color: Color;
	/** Hits counted, `null` while searching. */
	total: number | null;
	/** Passages holding them, as its list shows them; `null` while searching. */
	passages: number | null;
}

export const isShown = (m: Marked): boolean => m.slot !== HIDDEN;

/** The lowest slot no shown search holds, `HIDDEN` when all are taken. */
function freeSlot(list: readonly Marked[]): number {
	const taken = new Set(list.map((m) => m.slot));
	for (let slot = 0; slot < MAX_SHOWN; slot++) if (!taken.has(slot)) return slot;
	return HIDDEN;
}

/** A colour for a search: one no shown search wears, and among those, first
 *  one no search kept wears at all. */
function freeColor(list: readonly Marked[], keep?: Color): Color {
	const shown = new Set(list.filter(isShown).map((m) => m.color));
	if (keep && !shown.has(keep)) return keep;
	const kept = new Set(list.map((m) => m.color));
	return FIRST.find((c) => !kept.has(c)) ?? FIRST.find((c) => !shown.has(c)) ?? 'sky';
}

/** Whether a hidden search could be shown: a slot is free. */
export const canShow = (list: readonly Marked[]): boolean => freeSlot(list) !== HIDDEN;

/**
 * The list with a new search, last. It is shown in the lowest free slot, or
 * kept hidden when ten are shown already: the reader picks which to hide.
 */
export function add(list: readonly Marked[], kind: string, word: string, exact = false): Marked[] {
	const id = list.reduce((top, m) => Math.max(top, m.id + 1), 0);
	const slot = freeSlot(list);
	const color = freeColor(list);
	return [...list, { id, slot, kind, word, exact, color, total: null, passages: null }];
}

/** The list with one search changed; the others untouched. */
function change(list: readonly Marked[], id: number, to: Partial<Marked>): Marked[] {
	return list.map((m) => (m.id === id ? { ...m, ...to } : m));
}

export const remove = (list: readonly Marked[], id: number): Marked[] =>
	list.filter((m) => m.id !== id);

/**
 * The list with a search shown, or hidden. Hidden, it gives its slot back.
 * Shown, it takes the lowest free one — and a colour no shown search wears,
 * its own if it can — or nothing changes when ten are shown.
 */
export function toggle(list: readonly Marked[], id: number): Marked[] {
	const search = list.find((m) => m.id === id);
	if (!search) return [...list];
	if (isShown(search)) return change(list, id, { slot: HIDDEN });
	const slot = freeSlot(list);
	if (slot === HIDDEN) return [...list];
	return change(list, id, { slot, color: freeColor(list, search.color) });
}

export const recolor = (list: readonly Marked[], id: number, color: Color): Marked[] =>
	change(list, id, { color });

/** The list with a search's hits counted, and the passages holding them. */
export const counted = (
	list: readonly Marked[],
	id: number,
	total: number,
	passages: number
): Marked[] => change(list, id, { total, passages });

/** A colour as the canvas paints it, at `alpha`. */
export function paint(color: Color, alpha: number): string {
	return COLORS[color].replace(')', ` / ${alpha})`);
}

/**
 * A row of `n` bars by a name, as the map draws it (`BAR_WIDTH`,
 * `BAR_SPACE`, `BAR_ROW` in `src/map/frame.rs`), in the name's em: each
 * bar's width and the space between two — the nominal ones while the row
 * is narrower than its widest, both thinned alike past it.
 */
export function barRow(n: number): { bar: number; space: number; width: number } {
	const natural = n * 0.08 + Math.max(n - 1, 0) * 0.07;
	const k = Math.min(1, 1.4 / natural);
	return { bar: 0.08 * k, space: 0.07 * k, width: natural * k };
}

/**
 * A count as the panel shows it: thousands grouped by a no-break space
 * (U+00A0, written as itself below), never a comma — the owner's rule,
 * `22 413`. A narrow one vanished in the interface font.
 */
export const grouped = (n: number): string =>
	Math.trunc(n)
		.toString()
		.replace(/\B(?=(\d{3})+(?!\d))/g, ' ');

/** The slots set in `bits` — the module's way of naming them — lowest first. */
export function slotsOf(bits: number): number[] {
	const slots: number[] = [];
	for (let slot = 0; slot < MAX_SHOWN; slot++) if (bits & (1 << slot)) slots.push(slot);
	return slots;
}

/** The colours the shown searches among `bits` wear, lowest slot first. */
export function worn(list: readonly Marked[], bits: number): Color[] {
	return slotsOf(bits).flatMap((slot) => {
		const search = list.find((m) => m.slot === slot);
		return search ? [search.color] : [];
	});
}

/**
 * A letter several searches found wears every one of their colours: bands
 * of equal height, top to bottom, lowest slot first, cut sharp — each colour
 * a stop where its band starts and one where it ends. The points sit mostly
 * under the letter, so the body keeps the first colours.
 */
export function bands<T>(colors: readonly T[]): [number, T][] {
	const n = colors.length;
	return colors.flatMap((color, k): [number, T][] => [
		[k / n, color],
		[(k + 1) / n, color]
	]);
}

/**
 * The style of letters the searches found, in the page's markup: one
 * colour as it is, several in their bands painted into the glyphs — the
 * canvas draws the same bands. Empty for none.
 */
export function inkStyle(colors: readonly Color[]): string {
	if (colors.length === 0) return '';
	if (colors.length === 1) return `color: ${COLORS[colors[0]]}`;
	const stops = bands(colors.map((c) => COLORS[c]))
		.map(([at, color]) => `${color} ${at * 100}%`)
		.join(', ');
	return (
		`color: transparent; background-image: linear-gradient(to bottom, ${stops}); ` +
		'-webkit-background-clip: text; background-clip: text'
	);
}
