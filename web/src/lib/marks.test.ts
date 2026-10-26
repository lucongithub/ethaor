import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	add,
	COLORS,
	counted,
	MAX_SEARCHES,
	paint,
	recolor,
	remove,
	toggle,
	type Marked
} from './marks';

const fill = (n: number): Marked[] => {
	let list: Marked[] = [];
	for (let k = 0; k < n; k++) list = add(list, 'Consecutive letters', `w${k}`)!;
	return list;
};

describe('COLORS', () => {
	it("are Tailwind's 500 steps, value for value", () => {
		const theme = readFileSync('node_modules/tailwindcss/theme.css', 'utf8');
		for (const [name, value] of Object.entries(COLORS)) {
			expect(theme).toContain(`--color-${name}-500: ${value};`);
		}
		expect(Object.keys(COLORS)).toHaveLength(17);
	});

	it('paint adds the alpha inside the colour', () => {
		expect(paint('sky', 0.5)).toBe('oklch(68.5% 0.169 237.323 / 0.5)');
	});
});

describe('add', () => {
	it('keeps whether the search tells final forms apart', () => {
		expect(add([], 'Consecutive letters', 'אדם', true)![0].exact).toBe(true);
		expect(add([], 'Consecutive letters', 'אדם')![0].exact).toBe(false);
	});

	it('starts in slot 0, sky, shown, still searching', () => {
		expect(add([], 'Consecutive letters', 'תורה')).toEqual([
			{
				slot: 0,
				kind: 'Consecutive letters',
				word: 'תורה',
				exact: false,
				color: 'sky',
				shown: true,
				total: null
			}
		]);
	});

	it('gives each search a slot and a colour of its own, and refuses a sixth', () => {
		const list = fill(MAX_SEARCHES);
		expect(new Set(list.map((m) => m.slot)).size).toBe(MAX_SEARCHES);
		expect(new Set(list.map((m) => m.color)).size).toBe(MAX_SEARCHES);
		expect(list.every((m) => m.slot < MAX_SEARCHES)).toBe(true);
		expect(add(list, 'Consecutive letters', 'x')).toBeNull();
	});

	it('takes the lowest slot freed, and a colour no search wears', () => {
		const list = add(remove(fill(3), 1), 'Consecutive letters', 'x')!;
		const taken = list.at(-1)!;
		expect(taken.slot).toBe(1);
		expect(list.filter((m) => m.color === taken.color)).toHaveLength(1);
	});

	it('passes over a colour chosen for another search', () => {
		const list = add(recolor(fill(1), 0, 'rose'), 'Consecutive letters', 'x')!;
		expect(list.map((m) => m.color)).toEqual(['rose', 'sky']);
	});
});

describe('changes', () => {
	it('each touches its search alone', () => {
		const list = fill(3);
		expect(toggle(list, 1).map((m) => m.shown)).toEqual([true, false, true]);
		expect(toggle(toggle(list, 1), 1)).toEqual(list);
		expect(recolor(list, 2, 'red').map((m) => m.color)).toEqual(['sky', 'rose', 'red']);
		expect(counted(list, 0, 42).map((m) => m.total)).toEqual([42, null, null]);
		expect(remove(list, 0).map((m) => m.slot)).toEqual([1, 2]);
	});
});
