import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { map } from './tests/artifacts';
import {
	add,
	bands,
	barRow,
	canShow,
	COLORS,
	counted,
	grouped,
	HIDDEN,
	inkStyle,
	isShown,
	MAX_SHOWN,
	paint,
	recolor,
	remove,
	slotsOf,
	toggle,
	worn,
	type Marked
} from './marks';

const fill = (n: number): Marked[] => {
	let list: Marked[] = [];
	for (let k = 0; k < n; k++) list = add(list, 'Consecutive letters', `w${k}`);
	return list;
};

describe('COLORS', () => {
	it("are Tailwind's 500 steps, value for value", () => {
		const theme = readFileSync('node_modules/tailwindcss/theme.css', 'utf8');
		for (const [name, value] of Object.entries(COLORS)) {
			expect(theme).toContain(`--color-${name}-500: ${value};`);
		}
		expect(Object.keys(COLORS)).not.toContain('violet');
	});

	// Sixteen colours to pick from, more than the searches shown — as many as
	// the module has slots, ten: those on the map never share a colour.
	it('are sixteen, more than the searches shown, as many as the module marks', () => {
		expect(Object.keys(COLORS)).toHaveLength(16);
		expect(MAX_SHOWN).toBe(10);
		expect(map.marks_slots()).toBe(MAX_SHOWN);
	});

	it('paint adds the alpha inside the colour', () => {
		expect(paint('sky', 0.5)).toBe('oklch(68.5% 0.169 237.323 / 0.5)');
	});
});

describe('add', () => {
	it('keeps whether the search tells final forms apart', () => {
		expect(add([], 'Consecutive letters', 'אדם', true)[0].exact).toBe(true);
		expect(add([], 'Consecutive letters', 'אדם')[0].exact).toBe(false);
	});

	it('starts with number 0, in slot 0, sky, still searching', () => {
		expect(add([], 'Consecutive letters', 'תורה')).toEqual([
			{
				id: 0,
				slot: 0,
				kind: 'Consecutive letters',
				word: 'תורה',
				exact: false,
				color: 'sky',
				total: null
			}
		]);
	});

	// As many searches as the reader runs: the first ten on the map, each
	// its own slot and colour; the next kept hidden, holding no slot, and
	// every one its own number.
	it('keeps every search, the first ten shown, the rest hidden', () => {
		const list = fill(MAX_SHOWN + 7);
		expect(list).toHaveLength(MAX_SHOWN + 7);
		expect(new Set(list.map((m) => m.id)).size).toBe(list.length);
		const shown = list.filter(isShown);
		expect(shown.map((m) => m.slot)).toEqual(Array.from({ length: MAX_SHOWN }, (_, k) => k));
		expect(new Set(shown.map((m) => m.color)).size).toBe(MAX_SHOWN);
		expect(list.slice(MAX_SHOWN).every((m) => m.slot === HIDDEN)).toBe(true);
		expect(canShow(list)).toBe(false);
		expect(canShow(fill(MAX_SHOWN - 1))).toBe(true);
	});

	it('takes the lowest slot freed, and a colour no search wears', () => {
		const list = add(remove(fill(3), 1), 'Consecutive letters', 'x');
		const taken = list.at(-1)!;
		expect(taken.slot).toBe(1);
		expect(list.filter((m) => m.color === taken.color)).toHaveLength(1);
	});

	it('passes over a colour chosen for another search', () => {
		const list = add(recolor(fill(1), 0, 'rose'), 'Consecutive letters', 'x');
		expect(list.map((m) => m.color)).toEqual(['rose', 'sky']);
	});

	// Past the sixteen colours, a search kept hidden and shown once one is
	// hidden still wears a colour no other on the map wears: two on the map
	// never read as one.
	it('never shows two searches in one colour, every colour taken', () => {
		let list = fill(Object.keys(COLORS).length + 3);
		const onMap = () => list.filter(isShown).map((m) => m.color);
		expect(new Set(onMap()).size).toBe(MAX_SHOWN);
		list = toggle(toggle(list, 4), list.at(-1)!.id);
		expect(onMap()).toHaveLength(MAX_SHOWN);
		expect(new Set(onMap()).size).toBe(MAX_SHOWN);
	});
});

describe('shown and hidden', () => {
	// Hidden, a search gives its slot back; shown again, it takes the lowest
	// free one. With five shown, a hidden one stays hidden.
	it('gives a slot back when hidden, takes the lowest free when shown', () => {
		let list = fill(MAX_SHOWN + 1);
		const last = list.at(-1)!.id;
		expect(toggle(list, last)).toEqual(list);
		list = toggle(list, 2);
		expect(list.find((m) => m.id === 2)!.slot).toBe(HIDDEN);
		expect(canShow(list)).toBe(true);
		list = toggle(list, last);
		expect(list.find((m) => m.id === last)!.slot).toBe(2);
		expect(toggle(list, 2)).toEqual(list);
	});

	// Shown again, a search keeps its colour unless a search on the map took
	// it meanwhile; then it wears one no shown search wears.
	it('keeps its colour when shown again, unless one on the map wears it', () => {
		let list = toggle(fill(2), 0);
		const own = toggle(list, 0);
		expect(own.find((m) => m.id === 0)!.color).toBe('sky');
		list = recolor(list, 1, 'sky');
		const shown = toggle(list, 0);
		const color = shown.find((m) => m.id === 0)!.color;
		expect(color).not.toBe('sky');
		expect(shown.filter((m) => isShown(m) && m.color === color)).toHaveLength(1);
	});

	it('changes nothing for a number no search has', () => {
		const list = fill(2);
		expect(toggle(list, 99)).toEqual(list);
	});
});

describe('a count', () => {
	// Thousands grouped by a no-break space — a narrow one vanished in the
	// interface font — never a comma or a point: 22 413.
	it('groups its thousands by a space', () => {
		const space = String.fromCharCode(0xa0);
		expect(grouped(0)).toBe('0');
		expect(grouped(942)).toBe('942');
		expect(grouped(1000)).toBe(`1${space}000`);
		expect(grouped(22413)).toBe(`22${space}413`);
		expect(grouped(1234567)).toBe(`1${space}234${space}567`);
		expect(grouped(22413)).not.toMatch(/[,.]/);
	});
});

describe('the bars by a name', () => {
	// Each search shown adds a bar at the nominal width, until the row is as
	// wide as it may grow; past it, every bar thins so they all still fit.
	it('grow one per search, then thin to fit the row', () => {
		const one = barRow(1);
		expect(barRow(2).bar).toBe(one.bar);
		let last = { bar: Infinity, width: 0 };
		for (let n = 1; n <= 24; n++) {
			const row = barRow(n);
			expect(row.width).toBeCloseTo(n * row.bar + (n - 1) * row.space, 9);
			expect(row.width).toBeLessThanOrEqual(1.4 + 1e-9);
			expect(row.width).toBeGreaterThanOrEqual(last.width - 1e-9);
			expect(row.bar).toBeLessThanOrEqual(last.bar);
			last = row;
		}
		expect(last.bar).toBeLessThan(one.bar);
	});
});

describe('a letter several searches found', () => {
	// The module names slots as bits: each bit read back, lowest first, none
	// past the last slot.
	it('reads its slots off the bits, lowest first', () => {
		expect(slotsOf(0)).toEqual([]);
		expect(slotsOf(0b10110)).toEqual([1, 2, 4]);
		expect(slotsOf(1 << MAX_SHOWN)).toEqual([]);
	});

	// Only the shown searches among the bits lend their colour, in slot order:
	// a hidden one, or a slot no search holds, adds none.
	it('wears the colours of the shown searches among its bits', () => {
		const list = toggle(fill(3), 1);
		expect(worn(list, 0b111)).toEqual(['sky', 'lime']);
		expect(worn(list, 0b1010)).toEqual([]);
	});

	// Bands of equal height cover the letter top to bottom, cut sharp: each
	// colour starts where the previous one ends, at the same offset.
	it('cuts equal bands, top to bottom, with sharp edges', () => {
		for (const n of [1, 2, 3, MAX_SHOWN]) {
			const colors = Array.from({ length: n }, (_, k) => k);
			const stops = bands(colors);
			expect(stops).toHaveLength(2 * n);
			expect(stops[0][0]).toBe(0);
			expect(stops.at(-1)![0]).toBe(1);
			for (let k = 0; k < n; k++) {
				expect(stops[2 * k]).toEqual([k / n, k]);
				expect(stops[2 * k + 1]).toEqual([(k + 1) / n, k]);
			}
		}
	});

	// In the markup: one colour as it is, several as bands painted into the
	// glyphs, none as nothing.
	it('is styled in its colour, or in bands painted into its glyphs', () => {
		expect(inkStyle([])).toBe('');
		expect(inkStyle(['rose'])).toBe(`color: ${COLORS.rose}`);
		const two = inkStyle(['sky', 'rose']);
		expect(two).toContain('color: transparent');
		expect(two).toContain('background-clip: text');
		expect(two).toContain(
			`linear-gradient(to bottom, ${COLORS.sky} 0%, ${COLORS.sky} 50%, ${COLORS.rose} 50%, ${COLORS.rose} 100%)`
		);
	});
});

describe('changes', () => {
	it('each touches its search alone', () => {
		const list = fill(3);
		expect(toggle(list, 1).map(isShown)).toEqual([true, false, true]);
		expect(toggle(toggle(list, 1), 1)).toEqual(list);
		expect(recolor(list, 2, 'red').map((m) => m.color)).toEqual(['sky', 'rose', 'red']);
		expect(counted(list, 0, 42).map((m) => m.total)).toEqual([42, null, null]);
		expect(remove(list, 0).map((m) => m.id)).toEqual([1, 2]);
	});
});
