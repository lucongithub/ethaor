/**
 * The searches the page keeps on the map: each in a slot of the module's
 * (`MARK_SLOTS` in `src/map/marks.rs`), each wearing a colour of its own —
 * its letters in the text, its dots by the names above it. Every kind of
 * search shares the slots, five at most.
 *
 * The module says which slot a run or a dot belongs to; which colour that
 * slot wears is decided here.
 */

/** Searches kept at once, every kind together: the module's slots. */
export const MAX_SEARCHES = 5;

/** The colours a search may wear: Tailwind's 500 steps, its values exactly. */
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
	violet: 'oklch(60.6% 0.25 292.717)',
	purple: 'oklch(62.7% 0.265 303.9)',
	fuchsia: 'oklch(66.7% 0.295 322.15)',
	pink: 'oklch(65.6% 0.241 354.308)',
	rose: 'oklch(64.5% 0.246 16.439)'
} as const;

export type Color = keyof typeof COLORS;

/**
 * The colour a new search takes: sky first, then the next of these not worn,
 * far apart on the wheel so two searches never read as one.
 */
const FIRST: Color[] = ['sky', 'rose', 'lime', 'amber', 'violet'];

/** One search kept. */
export interface Marked {
	/** The module's slot: where its marks are set, shown, cleared. */
	slot: number;
	/** The search's name, as the sidebar's menu gives it. */
	kind: string;
	/** The word asked for, as typed. */
	word: string;
	/** Exact letters (final forms): its finals told apart. */
	exact: boolean;
	color: Color;
	/** Its marks drawn; hidden, they are kept. */
	shown: boolean;
	/** Hits counted, `null` while searching. */
	total: number | null;
}

/** The list with a new search, in the lowest free slot; `null` when full. */
export function add(
	list: readonly Marked[],
	kind: string,
	word: string,
	exact = false
): Marked[] | null {
	if (list.length >= MAX_SEARCHES) return null;
	const taken = new Set(list.map((m) => m.slot));
	let slot = 0;
	while (taken.has(slot)) slot++;
	const worn = new Set(list.map((m) => m.color));
	const all = Object.keys(COLORS) as Color[];
	const color = [...FIRST, ...all].find((c) => !worn.has(c)) ?? 'sky';
	return [...list, { slot, kind, word, exact, color, shown: true, total: null }];
}

/** The list with one search changed; the others untouched. */
function change(list: readonly Marked[], slot: number, to: Partial<Marked>): Marked[] {
	return list.map((m) => (m.slot === slot ? { ...m, ...to } : m));
}

export const remove = (list: readonly Marked[], slot: number): Marked[] =>
	list.filter((m) => m.slot !== slot);

export const toggle = (list: readonly Marked[], slot: number): Marked[] =>
	list.map((m) => (m.slot === slot ? { ...m, shown: !m.shown } : m));

export const recolor = (list: readonly Marked[], slot: number, color: Color): Marked[] =>
	change(list, slot, { color });

export const counted = (list: readonly Marked[], slot: number, total: number): Marked[] =>
	change(list, slot, { total });

/** A colour as the canvas paints it, at `alpha`. */
export function paint(color: Color, alpha: number): string {
	return COLORS[color].replace(')', ` / ${alpha})`);
}
