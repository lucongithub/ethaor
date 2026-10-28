/**
 * A search's hits listed as passages, the module's record read
 * (`results_page`, `App::results_page` in `src/map/marks.rs`): each passage
 * every verse its hits reach, each verse its place and our text, its found
 * letters as byte ranges of it with the slots that found them — and the
 * search's own letters under `OWN`, whether it is shown on the map or not.
 */

import type { Contents } from './book-view';
import { MAX_SHOWN, worn, type Color, type Marked } from './marks';
import type { Found } from './passage';

/** The bit a search's own letters carry, past every slot's: `OWN` in the module. */
export const OWN = 1 << MAX_SHOWN;

/** Words before a verse's runs — `RESULT_VERSE_HEAD` in the module. */
export const VERSE_HEAD = 8;

/** Passages read at a time, as the reader scrolls down to them. */
export const PAGE = 30;

/** One verse of a passage. */
export interface ResultVerse {
	book: number;
	chapter: number;
	/** Global index. */
	verse: number;
	/** Its number, in Hebrew letters. */
	number: string;
	/** Our text: the other edition's forms are not in it. */
	text: string;
	/** Letters found, by bytes of `text`, `OWN` for the search's own. */
	found: Found[];
}

export type Passage = ResultVerse[];

/** A page of passages read from the module's record; strings cut by byte offsets. */
export function readResults(
	record: ArrayLike<number>,
	text: (offset: number, length: number) => string
): Passage[] {
	const passages: Passage[] = [];
	let at = 0;
	while (at < record.length) {
		const count = record[at++];
		const passage: Passage = [];
		for (let v = 0; v < count; v++) {
			const runs = record[at + 7];
			const found: Found[] = [];
			for (let r = 0; r < runs; r++) {
				const k = at + VERSE_HEAD + 3 * r;
				found.push({ from: record[k], to: record[k + 1], slots: record[k + 2] });
			}
			passage.push({
				book: record[at],
				chapter: record[at + 1],
				verse: record[at + 2],
				number: text(record[at + 3], record[at + 4]),
				text: text(record[at + 5], record[at + 6]),
				found
			});
			at += VERSE_HEAD + 3 * runs;
		}
		passages.push(passage);
	}
	return passages;
}

/** Where a passage lies, every level in Hebrew: its first verse's, and its last's where they part. */
export interface Where {
	portion: string;
	book: string;
	chapter: string;
	/** The passage reaches into another chapter: each verse says its chapter. */
	crosses: boolean;
}

export function where(passage: Passage, contents: Contents): Where {
	const first = passage[0];
	const last = passage[passage.length - 1];
	const book = (b: number) => contents.books[b]?.name ?? '';
	const span = (a: string, b: string) => (a === b ? a : `${a}–${b}`);
	const crosses = first.chapter !== last.chapter;
	return {
		portion: contents.portions[contents.books[first.book]?.section] ?? '',
		book: span(book(first.book), book(last.book)),
		chapter:
			first.book === last.book
				? span(contents.labels[first.chapter], contents.labels[last.chapter])
				: contents.labels[first.chapter],
		crosses
	};
}

/**
 * The colours of letters found, as the list paints them: the shown
 * searches' among `bits`, lowest slot first — and the search listed, in its
 * own colour, wherever its letters are, shown or hidden. Never one twice.
 */
export function colorsOf(list: readonly Marked[], search: Marked, bits: number): Color[] {
	const colors = worn(list, bits & (OWN - 1));
	if (bits & OWN && !colors.includes(search.color)) colors.unshift(search.color);
	return [...new Set(colors)];
}
