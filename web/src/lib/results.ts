/**
 * A search's hits listed as passages, the module's record read
 * (`results_page`, `App::results_page` in `src/map/marks.rs`): each passage
 * every verse its hits reach, each verse its place and our text, its found
 * letters as byte ranges of it with the slots that found them — and the
 * search's own letters under `OWN`, whether it is shown on the map or not.
 */

import type { Contents } from './book-view';
import { MAX_SHOWN, slotsOf, worn, type Color, type Marked } from './marks';
import { inked, type Found, type Run } from './passage';

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

/**
 * Whether the list's crossings each get a title of their own: only when
 * there are several — the owner's rule. A single crossing is the list's own
 * title over again, which says it already.
 */
export const crossingsTitled = (crossings: readonly Crossing[]): boolean => crossings.length > 1;

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

/** A piece of a verse as a count's list draws it: a run, or one of the
 *  count's letters with its number from 1 — 0 for none. */
export interface Numbered extends Run {
	n: number;
}

/** A consonant and the marks riding on it. */
const LETTER = /[א-ת][^א-ת\s]*/gu;

/**
 * A text cut into its letters, each a consonant and the marks riding on it,
 * numbered from `after + 1` in reading order; what is not a letter — a
 * space, a mark before any consonant, punctuation — stays between them,
 * unnumbered (0). Every piece carries `slots`. The pieces read the text
 * back whole.
 */
export function numberedLetters(text: string, after = 0, slots = 0): Numbered[] {
	const pieces: Numbered[] = [];
	let n = after;
	let at = 0;
	for (const match of text.matchAll(LETTER)) {
		const start = match.index ?? 0;
		if (start > at) pieces.push({ text: text.slice(at, start), slots, n: 0 });
		pieces.push({ text: match[0], slots, n: ++n });
		at = start + match[0].length;
	}
	if (at < text.length) pieces.push({ text: text.slice(at), slots, n: 0 });
	return pieces;
}

/**
 * Only the letters a count holds, word by word across a passage's verses
 * (`numbered`): each word keeps its numbered letters alone, a word holding
 * none goes — what a gematria's method shows under its title, never the
 * whole passage, the owner's rule.
 */
export function countedWords(pieces: Numbered[][][]): Numbered[][] {
	return pieces
		.flat()
		.map((word) => word.filter((piece) => piece.n > 0))
		.filter((word) => word.length > 0);
}

/**
 * A passage's verses cut as the list draws them, each word into its runs —
 * and every letter of the search's own (`OWN`) apart, numbered in reading
 * order across the passage, from 1: a count's letters, each with its
 * place in the count.
 */
export function numbered(passage: Passage): Numbered[][][] {
	let n = 0;
	return passage.map((verse) =>
		inked(verse.text, verse.found).map((word) =>
			word.flatMap((run): Numbered[] => {
				if (!(run.slots & OWN)) return [{ ...run, n: 0 }];
				const pieces = numberedLetters(run.text, n, run.slots);
				n += pieces.filter((p) => p.n > 0).length;
				return pieces;
			})
		)
	);
}

/** Where each tier of numbers sits over its letter, the lowest first. */
const TIERS = [
	'bottom-[82%]',
	'bottom-[calc(82%+1.15em)]',
	'bottom-[calc(82%+2.3em)]',
	'bottom-[calc(82%+3.45em)]'
];

/**
 * The classes of a counted letter's number: small, centred over its letter,
 * out of the flow so the letters keep the text's own spacing, never
 * selected nor read aloud — a copy is the text alone. Neighbours take the
 * `tiers` in turn, the first just over the letter: two never share a line,
 * so a long number over a narrow letter still reads apart.
 */
export function numberClass(n: number, tiers = 2): string {
	return [
		'pointer-events-none absolute left-1/2 -translate-x-1/2 select-none',
		'font-sans text-[0.36em] leading-none tracking-tight whitespace-nowrap text-white/70 tabular-nums',
		TIERS[(n - 1) % Math.min(Math.max(tiers, 1), TIERS.length)]
	].join(' ');
}

/** The lines' height for numbers on `tiers` tiers: room over every letter for all of them. */
export function leadingFor(tiers: number): string {
	return tiers <= 2 ? 'leading-[2.6]' : tiers === 3 ? 'leading-[3.4]' : 'leading-[4.2]';
}
