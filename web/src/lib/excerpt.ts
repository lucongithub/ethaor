// A stretch of a passage, selected by the reader: the passage's selection
// mode lets the browser select its text as on any web page, and once the
// selection is made a menu beside it offers what can be done with it.
// What decides lives here — what the selection reads, where the menu
// stands, how a copy fails — and the passage only wires it.

import { escalate, type Fault } from '$lib/fault';
import { letters } from '$lib/letters';
import type { Found } from '$lib/search-core';

/** A box on the screen, in CSS pixels. */
export interface Box {
	left: number;
	top: number;
	width: number;
	height: number;
}

/**
 * What a selection reads, as it is copied: its runs of white space — the
 * line breaks and indents of the markup between words included — made one
 * space each, nothing before or after. Empty when nothing but white space
 * is selected.
 */
export function excerpt(selected: string): string {
	return selected.replace(/\s+/g, ' ').trim();
}

/**
 * The text of a selection over several verses of the book view, as the map
 * gives it: each verse's piece read by `excerpt`, our forms' index `¹` taken
 * out — theirs are taken out whole before — each verse on its own line, the
 * verses holding nothing selected left out.
 */
export function joined(pieces: readonly string[]): string {
	return pieces
		.map((piece) => excerpt(piece.replace(/[¹²]/g, '')))
		.filter(Boolean)
		.join('\n');
}

/**
 * The level of a piece of the book view, from its markup — `data-verse` on
 * a verse, `data-title` on a name or a number with its rank: the portions'
 * names 0, the books' 1, the chapters' numbers 2, the verses 3, as the map
 * counts them (`Unit::level`). A selection keeps to the level of its first
 * press; `null` for anything else.
 */
export function levelIn(verse: string | undefined, title: string | undefined): number | null {
	if (verse !== undefined) return 3;
	const rank = Number(title);
	return title !== undefined && title !== '' && Number.isInteger(rank) && rank >= 0 && rank < 3
		? rank
		: null;
}

/** Room left between the selection, the menu and the edges of their box. */
export const MENU_GAP = 8;

/**
 * Where the menu stands, relative to `within`: centred under the selected
 * text, or over it when there is no room under; always whole inside
 * `within`, even when the selection is scrolled half out of it.
 */
export function menuAt(
	selected: Box,
	within: Box,
	menu: { width: number; height: number }
): { left: number; top: number } {
	const clamp = (v: number, low: number, high: number) => Math.max(low, Math.min(v, high));
	const centre = selected.left + selected.width / 2 - within.left;
	const left = clamp(
		centre - menu.width / 2,
		MENU_GAP,
		Math.max(MENU_GAP, within.width - menu.width - MENU_GAP)
	);
	const under = selected.top + selected.height - within.top + MENU_GAP;
	const over = selected.top - within.top - MENU_GAP - menu.height;
	const top = under + menu.height + MENU_GAP <= within.height ? under : over;
	return {
		left,
		top: clamp(top, MENU_GAP, Math.max(MENU_GAP, within.height - menu.height - MENU_GAP))
	};
}

/**
 * The text written to the clipboard. Resolves once it is there; a refusal —
 * no clipboard, no permission, a page not secure — rejects with a numbered
 * fault, the text still selected for the keyboard's own copy.
 */
export async function copy(text: string, write: (text: string) => Promise<void>): Promise<void> {
	try {
		await write(text);
	} catch (cause) {
		throw escalate(cause, 'copy-failed', 'copying a selection') as Fault;
	}
}

/**
 * A verse's letters a selection in the book holds, as a letters record
 * names them — the verse, then each letter's rank among its consonants:
 * `before`, our text of the verse ahead of the selection, gives the first
 * rank; `piece`, our text selected, how many follow. Theirs and the
 * verses' numbers are out of both, so the ranks are the module's.
 */
export function ranked(verse: number, before: string, piece: string): number[] {
	const first = letters(before).length;
	return Array.from({ length: letters(piece).length }, (_, k) => [verse, first + k]).flat();
}

/**
 * Whether a count is kept in the panel, as a search: only when it holds a
 * verse's letter to ink and list — the owner's rule. A count of names and
 * numbers alone is told once, in a modal, and kept nowhere.
 */
export const keepsCount = (found: Found): boolean => found.hits.length > 0;

/**
 * What a count of letters keeps, as a search's answer: one hit, its letters
 * the selection's verses' letters by verse and rank — so the map inks them,
 * the panel lists them, and they join ∪ and ∩ as any search's. No hit when
 * the selection holds no verse's letter.
 */
export function countOf(pairs: ArrayLike<number>): Found {
	const hits = Uint32Array.from(pairs);
	return { total: hits.length ? 1 : 0, hits, stride: Math.max(hits.length, 1), form: 'letters' };
}
