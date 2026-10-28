import { describe, expect, it } from 'vitest';
import { map } from './tests/artifacts';
import { MAX_SKIPS, runSkips, skipRange } from './skips';

// The largest skip of a three-letter word in the whole Tanakh, as the map
// module says it.
const MAX = map.skip_max(3);

describe('skipRange', () => {
	// 5 - 5 is one search, skip 5; 5 - 10 is six, skip 5 to skip 10.
	it('asks one skip per whole number between the two ends, both included', () => {
		expect(skipRange(5, 5, MAX)).toEqual([5]);
		expect(skipRange(5, 10, MAX)).toEqual([5, 6, 7, 8, 9, 10]);
	});

	// 10 - 5 asks what 5 - 10 does: the rows still come lowest first.
	it('takes the two ends in either order, lowest first', () => {
		expect(skipRange(10, 5, MAX)).toEqual([5, 6, 7, 8, 9, 10]);
	});

	// Skip 0 is the consecutive search; a fraction, a negative or a number
	// past the bound skips nothing.
	it('refuses skip 0, a fraction, a negative, NaN and a skip past the bound', () => {
		for (const [from, to] of [
			[0, 5],
			[5, 0],
			[1.5, 3],
			[-2, 3],
			[Number.NaN, 3],
			[3, Number.POSITIVE_INFINITY],
			[1, MAX + 1]
		]) {
			expect(skipRange(from, to, MAX), `${from} - ${to}`).toBeNull();
		}
		expect(skipRange(MAX, MAX, MAX)).toEqual([MAX]);
	});

	// The bound is the word's, not a number of our own: two letters may skip
	// almost the whole Tanakh, three half as far, and so on — and one letter
	// has no skip, so its range is always refused.
	it("is bounded by the word's length in the whole Tanakh", () => {
		const two = map.skip_max(2);
		expect(MAX).toBe(Math.floor((two + 1) / 2) - 1);
		expect(map.skip_max(4)).toBe(Math.floor((two + 1) / 3) - 1);
		expect(skipRange(two, two, two)).toEqual([two]);
		expect(skipRange(two, two, MAX)).toBeNull();
		expect(map.skip_max(1)).toBe(0);
		expect(skipRange(1, 1, map.skip_max(1))).toBeNull();
	});

	// Each skip is a search and a row: a run is bounded, one past it refused.
	it('holds MAX_SKIPS skips at most, one more refused', () => {
		expect(skipRange(1, MAX_SKIPS, MAX)).toHaveLength(MAX_SKIPS);
		expect(skipRange(1, MAX_SKIPS + 1, MAX)).toBeNull();
	});
});

describe('runSkips', () => {
	const totals: Record<number, number> = { 5: 3, 6: 1, 7: 0, 8: 2 };
	const search = (skip: number) => Promise.resolve({ skip, total: totals[skip] ?? 0 });

	// For 5 - 10 with nothing at skip 7, the rows are 5, 6 and 8: none for 7.
	it('keeps every skip that found something, lowest first, and drops one that found nothing', async () => {
		const kept: number[] = [];
		await runSkips([5, 6, 7, 8, 9, 10], search, (skip, found) => {
			expect(found.skip).toBe(skip);
			kept.push(skip);
		});
		expect(kept).toEqual([5, 6, 8]);
	});

	// One search at a time: the next is asked only once the one before has
	// answered, so the rows come in order however long each takes.
	it('asks each skip once the one before has answered', async () => {
		let running = 0;
		let most = 0;
		const asked: number[] = [];
		await runSkips(
			[5, 6, 7, 8],
			async (skip) => {
				asked.push(skip);
				most = Math.max(most, ++running);
				await new Promise((r) => setTimeout(r, 9 - skip));
				running--;
				return { total: 1 };
			},
			() => {}
		);
		expect(asked).toEqual([5, 6, 7, 8]);
		expect(most).toBe(1);
	});

	// A failure is the searcher's to tell; the skips after it still run.
	it('goes on past a search that fails', async () => {
		const kept: number[] = [];
		await runSkips(
			[5, 6, 8],
			(skip) => (skip === 6 ? Promise.reject(new Error('trap')) : search(skip)),
			(skip) => kept.push(skip)
		);
		expect(kept).toEqual([5, 8]);
	});

	// Reset while it searches: what answers after keeps no row, and nothing
	// more is asked.
	it('stops once the run is no longer wanted, keeping nothing that answers after', async () => {
		let live = true;
		const asked: number[] = [];
		const kept: number[] = [];
		await runSkips(
			[5, 6, 8],
			(skip) => {
				asked.push(skip);
				if (skip === 6) live = false;
				return search(skip);
			},
			(skip) => kept.push(skip),
			() => live
		);
		expect(asked).toEqual([5, 6]);
		expect(kept).toEqual([5]);
	});
});
