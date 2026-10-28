// The passage opened by a click, read word by word: each word of the verse
// is one to point at and pick, and a picked word is shown in place, the
// rest of the verse around it. The verse is our text — the other edition's
// forms are not in it — cut at its spaces as the module cuts a line into
// words. A space is one UTF-16 unit and never inside a pointed letter, so
// the cut keeps every letter whole.

/** The words of a verse, in reading order. */
export function words(verse: string): string[] {
	return verse.split(' ');
}

/** The verse with its `k`-th word picked: the word, and what precedes and follows it. */
export function pick(
	verse: string,
	k: number
): { word: string; head: string; tail: string } | null {
	const all = words(verse);
	if (k < 0 || k >= all.length || all[k] === '') return null;
	const before = all.slice(0, k).join(' ');
	const after = all.slice(k + 1).join(' ');
	return {
		word: all[k],
		head: k > 0 ? `${before} ` : '',
		tail: k < all.length - 1 ? ` ${after}` : ''
	};
}

/** Which word of the verse is picked, from what precedes it; -1 for none. */
export function picked(selection: { word: string; head: string }): number {
	return selection.word ? words(selection.head).length - 1 : -1;
}

/** Letters the searches found in the verse: a byte range of it, and the
 *  slots that found them, as bits. */
export interface Found {
	from: number;
	to: number;
	slots: number;
}

/** A piece of a word: its text, and the slots that found it as bits, 0 for none. */
export interface Run {
	text: string;
	slots: number;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * The verse's words, each cut into the runs the searches found and those
 * between: what the passage draws, the found letters in their colour. The
 * module gives byte ranges of the verse; a run is cut by bytes, never in the
 * decoded string, and a word without a find stays one run.
 */
export function inked(verse: string, found: readonly Found[]): Run[][] {
	let at = 0;
	return words(verse).map((word) => {
		const bytes = encoder.encode(word);
		const [start, end] = [at, at + bytes.length];
		at = end + 1;
		const cuts = found
			.filter((f) => f.to > start && f.from < end)
			.map((f) => ({
				from: Math.max(f.from, start) - start,
				to: Math.min(f.to, end) - start,
				slots: f.slots
			}))
			.sort((a, b) => a.from - b.from);
		const runs: Run[] = [];
		let cursor = 0;
		const piece = (from: number, to: number, slots: number) =>
			runs.push({ text: decoder.decode(bytes.subarray(from, to)), slots });
		for (const cut of cuts) {
			const from = Math.max(cut.from, cursor);
			if (cut.to <= from) continue;
			if (from > cursor) piece(cursor, from, 0);
			piece(from, cut.to, cut.slots);
			cursor = cut.to;
		}
		if (cursor < bytes.length || runs.length === 0) piece(cursor, bytes.length, 0);
		return runs;
	});
}
