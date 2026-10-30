/**
 * A selection read by every gematria method, as the map module answers it
 * (`src/wasm/gematria.rs`): each method's value, and each letter as the
 * method reads it — AtBash reads alef as tav — with its share of the value.
 * The page draws them over the passage; it computes none of them.
 */

import { grouped } from './marks';
import type { Numbered } from './results';

/** The exports the reading takes, from the map module. */
export interface GematriaExports {
	memory: WebAssembly.Memory;
	gematria_buffer(length: number): number;
	gematria_read(length: number): number;
	gematria_values_ptr(): number;
	gematria_flags_ptr(): number;
	gematria_read_as_ptr(): number;
	gematria_shares_ptr(): number;
	gematria_words(): number;
	gematria_names(): number;
	gematria_names_ptr(): number;
}

/** A method's value is the sum of its letters' shares — `SUMMED`. */
const SUMMED = 1;
/** A method reads each letter as another — `REPLACES`. */
const REPLACES = 2;

/** One method's reading of the text. */
export interface Method {
	/** Its key, as the API names it: `mispar_hapanim`, `atbash`… */
	key: string;
	name: string;
	hebrew: string;
	value: number;
	/** The value is the sum of the shares; else a formula over them. */
	summed: boolean;
	/** Each letter is read as another. */
	replaces: boolean;
	/** Each letter's form as the method reads it: 0 alef … 26 tav, finals their own. */
	readAs: Uint8Array;
	/** Each letter's share of the value; its standard value for a formula. */
	shares: Uint32Array;
}

export interface Gematria {
	letters: number;
	words: number;
	/** Every method, Mispar haPanim — the standard value — first. */
	methods: Method[];
}

/** What a text may not be read for: no Hebrew letter, or too many. */
export type Unread = 'no-letter' | 'too-long';

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** `text` read by every method, or why it cannot be. */
export function readGematria(wasm: GematriaExports, text: string): Gematria | Unread {
	// Each call may grow the module's memory, which detaches the buffer read
	// before it: the buffer is read after the call, never as its argument.
	const bytes = encoder.encode(text);
	const room = wasm.gematria_buffer(bytes.length);
	new Uint8Array(wasm.memory.buffer, room, bytes.length).set(bytes);
	const letters = wasm.gematria_read(bytes.length);
	if (letters === -2) return 'too-long';
	if (letters < 0) return 'no-letter';
	const length = wasm.gematria_names();
	const at = wasm.gematria_names_ptr();
	const names = decoder
		.decode(new Uint8Array(wasm.memory.buffer, at, length))
		.split('\n')
		.filter(Boolean)
		.map((line) => line.split('\t'));
	const count = names.length;
	const { buffer } = wasm.memory;
	const values = new Float64Array(buffer, wasm.gematria_values_ptr(), count);
	const flags = new Uint32Array(buffer, wasm.gematria_flags_ptr(), count);
	const readAs = new Uint8Array(buffer, wasm.gematria_read_as_ptr(), count * letters);
	const shares = new Uint32Array(buffer, wasm.gematria_shares_ptr(), count * letters);
	return {
		letters,
		words: wasm.gematria_words(),
		methods: names.map(([key, name, hebrew], m) => ({
			key,
			name,
			hebrew,
			value: values[m],
			summed: (flags[m] & SUMMED) !== 0,
			replaces: (flags[m] & REPLACES) !== 0,
			readAs: readAs.slice(m * letters, (m + 1) * letters),
			shares: shares.slice(m * letters, (m + 1) * letters)
		}))
	};
}

/** A form's letter: 0 alef … 26 tav, in Unicode order. */
export const formLetter = (form: number): string => String.fromCharCode(0x05d0 + form);

/**
 * A numbered letter as a method shows it — the `n`th of the text, from 1:
 * the letter it is read as, bare, for a method replacing letters (its points
 * belong to the letter it replaced), the letter as written for any other;
 * and over it, its share of the value. Without a method, the letter and its
 * place in the count.
 */
export function letterAs(method: Method | null, piece: Numbered): { text: string; over: string } {
	if (!method) return { text: piece.text, over: grouped(piece.n) };
	const at = piece.n - 1;
	if (at >= method.shares.length) return { text: piece.text, over: '' };
	const text = method.replaces ? formLetter(method.readAs[at]) : piece.text;
	return { text, over: grouped(method.shares[at]) };
}

/**
 * How three methods that are no sum of their letters reach their value
 * from the standard one — the letters each carry their standard value:
 * Kolel adds the words, HaMerubah HaKlali squares, Katan Mispari takes the
 * root. Nothing for a method that is the sum of its letters.
 */
export function formula(reading: Gematria, method: Method): string {
	if (method.summed) return '';
	const standard = grouped(reading.methods[0].value);
	const value = grouped(method.value);
	if (method.key === 'mispar_kolel') {
		const words = `${grouped(reading.words)} ${reading.words === 1 ? 'word' : 'words'}`;
		return `${standard} + ${words} = ${value}`;
	}
	if (method.key === 'mispar_hamerubah_haklali') return `${standard}² = ${value}`;
	return `${standard} → ${value}`;
}

/**
 * How many tiers the numbers over the letters take, so the widest never
 * meets its neighbours' — a tier more for every few digits; never fewer
 * than two, the count's.
 */
export function tiersFor(shares: ArrayLike<number>): number {
	let widest = 0;
	for (let k = 0; k < shares.length; k++) widest = Math.max(widest, grouped(shares[k]).length);
	return widest <= 5 ? 2 : widest <= 7 ? 3 : 4;
}
