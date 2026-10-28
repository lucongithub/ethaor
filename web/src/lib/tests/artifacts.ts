/**
 * The build's artifacts, read from disk as the page would fetch them, for
 * tests: the two modules, the text cut into books, the plain text.
 */

import { readFileSync } from 'node:fs';
import { basename } from 'node:path';
import { gunzipSync } from 'node:zlib';
import cuts from '../ethaor-text.books.json';

const lib = new URL('../', import.meta.url);

export const read = (name: string) => new Uint8Array(readFileSync(new URL(name, lib)));

/** `fetch` answering any URL with the artifact of the same file name. */
export function serveArtifacts(): typeof fetch {
	return (async (input: RequestInfo | URL) => {
		const url = input instanceof Request ? input.url : String(input);
		const name = basename(url).split('?')[0];
		const type = name.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream';
		return new Response(read(name), { headers: { 'content-type': type } });
	}) as typeof fetch;
}

export const bookCuts: number[] = cuts;
const books = read('ethaor-text.books.bin');

/** Block `i` of the books' file, opened: a book, or the readings last. */
export const block = (i: number) =>
	new Uint8Array(gunzipSync(books.subarray(bookCuts[i], bookCuts[i + 1])));

interface MapExports {
	book_first_chapter(book: number): number;
	book_end_chapter(book: number): number;
	marks_slots(): number;
	skip_max(letters: number): number;
}

/** The map module, for where each book's chapters lie, how many searches it marks and how far a word may skip. */
export const map = new WebAssembly.Instance(new WebAssembly.Module(read('ethaor.wasm')), {})
	.exports as unknown as MapExports;

/** The books, in an order of their own, each with its run of chapters. */
export function booksShuffled() {
	const count = bookCuts.length - 2;
	const order = Array.from({ length: count }, (_, i) => (i * 17) % count);
	return order.map((b) => ({
		first: map.book_first_chapter(b),
		end: map.book_end_chapter(b),
		codes: block(b)
	}));
}
