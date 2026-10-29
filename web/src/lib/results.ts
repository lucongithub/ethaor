/**
 * A search's hits listed as passages, the module's record read
 * (`results_page`, `App::results_page` in `src/map/marks.rs`): each passage
 * every verse its hits reach, each verse its place and our text, its found
 * letters as byte ranges of it with the slots that found them — and the
 * search's own letters under `OWN`, whether it is shown on the map or not.
 */

import type { Contents } from './book-view';
import { MAX_SHOWN, slotsOf, worn, type Color, type Marked } from './marks';
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

/**
 * A crossing of the searches shown together: the slots holding a hit in
 * each of its passages, as bits, how many passages, and where they start
 * in the list.
 */
export interface Crossing {
	slots: number;
	passages: number;
	from: number;
}

/**
 * The list's crossings read from the module's `(slots, passages)` pairs
 * (`results_groups`), each with the index of its first passage.
 */
export function readCrossings(record: ArrayLike<number>): Crossing[] {
	const out: Crossing[] = [];
	let from = 0;
	for (let at = 0; at + 1 < record.length; at += 2) {
		out.push({ slots: record[at], passages: record[at + 1], from });
		from += record[at + 1];
	}
	return out;
}

/** The searches a crossing holds, lowest slot first: the words its title reads. */
export const crossed = (list: readonly Marked[], slots: number): Marked[] =>
	slotsOf(slots).flatMap((slot) => list.filter((m) => m.slot === slot));

/** Where verses lie, every level in Hebrew, as the breadcrumb names it. */
export interface Place {
	portion: string;
	book: string;
	chapter: string;
}

/** A run of a passage's verses in one chapter, and where it lies. */
export interface Stretch {
	place: Place;
	verses: ResultVerse[];
}

/**
 * A passage cut into runs, one per chapter, each named where it lies. A
 * word read far apart may reach into another chapter, book or portion: the
 * place is said again wherever it changes, as it is for the first, so each
 * verse keeps its number alone.
 */
export function stretches(passage: Passage, contents: Contents): Stretch[] {
	const out: Stretch[] = [];
	for (const verse of passage) {
		const open = out[out.length - 1];
		if (open && open.verses[0].chapter === verse.chapter) {
			open.verses.push(verse);
			continue;
		}
		const book = contents.books[verse.book];
		out.push({
			place: {
				portion: contents.portions[book?.section] ?? '',
				book: book?.name ?? '',
				chapter: contents.labels[verse.chapter] ?? ''
			},
			verses: [verse]
		});
	}
	return out;
}

/**
 * The colours of letters found, as the list paints them: the shown
 * searches' among `bits`, lowest slot first — and the search listed, if
 * the list is one search's, in its own colour wherever its letters are,
 * shown or hidden. Never one twice.
 */
export function colorsOf(list: readonly Marked[], search: Marked | null, bits: number): Color[] {
	const colors = worn(list, bits & (OWN - 1));
	if (search && bits & OWN && !colors.includes(search.color)) colors.unshift(search.color);
	return [...new Set(colors)];
}
