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
