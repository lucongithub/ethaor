/**
 * The equidistant-letter search: a word read with `from..=to` letters
 * skipped between its letters, one search per skip — the owner's rule —
 * each its own row in the panel, and only those that found something.
 */

/** Skips one run may ask for: each is a search and a row of its own. */
export const MAX_SKIPS = 100;

/**
 * The skips `from..=to` asks for, lowest first — the two ends taken in
 * either order — or `null` when either is not a whole number in
 * `1..=max` (skip 0 is the consecutive search; `max` the largest skip the
 * word can be read with in the whole Tanakh, the module's `skip_max`), or
 * when the range holds more than `MAX_SKIPS`.
 */
export function skipRange(from: number, to: number, max: number): number[] | null {
	const fits = (n: number) => Number.isInteger(n) && n >= 1 && n <= max;
	if (!fits(from) || !fits(to)) return null;
	const [low, high] = from <= to ? [from, to] : [to, from];
	if (high - low + 1 > MAX_SKIPS) return null;
	return Array.from({ length: high - low + 1 }, (_, k) => low + k);
}

/**
 * Runs one search per skip, one after the other, lowest first, and hands
 * `keep` each that found at least one hit, in that order; a skip with none
 * is dropped. A search that fails is skipped — its fault is the searcher's
 * to tell — and the next runs. Stops as soon as `live` says the run is no
 * longer wanted (the searches were reset).
 */
export async function runSkips<F extends { total: number }>(
	skips: readonly number[],
	search: (skip: number) => Promise<F>,
	keep: (skip: number, found: F) => void,
	live: () => boolean = () => true
): Promise<void> {
	for (const skip of skips) {
		if (!live()) return;
		let found: F;
		try {
			found = await search(skip);
		} catch {
			continue;
		}
		if (!live()) return;
		if (found.total > 0) keep(skip, found);
	}
}
