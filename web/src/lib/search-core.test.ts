import { beforeAll, describe, expect, it, vi } from 'vitest';
import { SearchCore } from './search-core';
import { block, bookCuts, booksShuffled, read, serveArtifacts } from './tests/artifacts';

beforeAll(() => vi.stubGlobal('fetch', serveArtifacts()));

describe('the books file', () => {
	it('holds the 39 books then the readings, in order', () => {
		expect(bookCuts.length - 1).toBe(40);
		for (let i = 1; i < bookCuts.length; i++) expect(bookCuts[i]).toBeGreaterThan(bookCuts[i - 1]);
		expect(bookCuts.at(-1)).toBe(read('ethaor-text.books.bin').length);
	});

	it('opens, block after block, to the plain text the tiles read', () => {
		const plain = read('ethaor-text.plain.bin');
		const blocks = Array.from({ length: bookCuts.length - 1 }, (_, i) => block(i));
		const whole = new Uint8Array(blocks.reduce((n, b) => n + b.length, 0));
		let at = 0;
		for (const b of blocks) {
			whole.set(b, at);
			at += b.length;
		}
		// The plain file starts with the chapters' spans, then the same bytes.
		expect(plain.subarray(plain.length - whole.length)).toEqual(whole);
		expect(whole.length / plain.length).toBeGreaterThan(0.9);
	});
});

describe('SearchCore', () => {
	it('answers -1 until the whole text is in, then finds', async () => {
		const core = await SearchCore.create();
		expect(core.els('תורה', 0, 0).total).toBe(-1);
		const books = booksShuffled();
		const readings = block(bookCuts.length - 2);
		books.slice(0, -1).forEach((b) => core.tile(b.first, b.end, b.codes));
		expect(core.readings(readings)).toBe(false);
		expect(core.els('תורה', 0, 0).total).toBe(-1);
		const last = books.at(-1)!;
		core.tile(last.first, last.end, last.codes);
		expect(core.readings(readings)).toBe(true);

		const found = core.els('תורה', 0, 0);
		expect(found.total).toBeGreaterThan(0);
		expect(found.stride).toBeGreaterThan(0);
		expect(found.hits.length).toBeGreaterThan(0);
		expect(found.hits.length % found.stride).toBe(0);
		// A copy, not a view: the next search does not rewrite it.
		const kept = found.hits.slice();
		core.els('אלהים', 0, 3);
		expect(found.hits).toEqual(kept);
	});

	it('tells final forms apart when asked: each spelling its share of the folded search', async () => {
		const core = await SearchCore.create();
		booksShuffled().forEach((b) => core.tile(b.first, b.end, b.codes));
		expect(core.readings(block(bookCuts.length - 2))).toBe(true);
		const folded = core.els('אדם', 0, 0).total;
		const final = core.els('אדם', 0, 0, true).total;
		const open = core.els('אדמ', 0, 0, true).total;
		expect(final).toBeGreaterThan(0);
		expect(open).toBeGreaterThan(0);
		expect(final + open).toBe(folded);
	});

	it('reads first and last letters: -1 before the text, then each letter by verse and rank', async () => {
		const core = await SearchCore.create();
		expect(core.words('בבא', { nth: 1 }).total).toBe(-1);
		booksShuffled().forEach((b) => core.tile(b.first, b.end, b.codes));
		expect(core.readings(block(bookCuts.length - 2))).toBe(true);

		// בראשית ברא אלהים: the first letters of Genesis' first words.
		const first = core.words('בבא', { nth: 1 });
		expect(first.form).toBe('letters');
		expect(first.stride).toBe(6);
		expect(Array.from(first.hits.subarray(0, 6))).toEqual([0, 0, 0, 6, 0, 9]);
		// Any order holds the word as written, and more.
		const any = core.words('אבב', { nth: 1, anyOrder: true });
		expect(Array.from(any.hits.subarray(0, 6))).toEqual([0, 0, 0, 6, 0, 9]);
		expect(any.total).toBeGreaterThan(core.words('אבב', { nth: 1 }).total);
		// The last letters, finals told apart: ת א ם.
		const last = core.words('תאם', { nth: 1, fromEnd: true, exact: true });
		expect(Array.from(last.hits.subarray(0, 6))).toEqual([0, 5, 0, 8, 0, 13]);
		expect(core.words('תאם', { nth: 1, exact: true }).hits.subarray(0, 2)).not.toEqual(
			new Uint32Array([0, 5])
		);
		// A letter no word holds finds nothing.
		expect(core.words('א', { nth: 99 }).total).toBe(0);
	});

	// The page searches first and last letters folded, never exact: a last
	// letter is final whenever it can be, so a word typed with or without its
	// final forms finds the same — and the words' last letters are not their
	// first.
	it('reads last letters folded: a final typed or not is the same search', async () => {
		const core = await SearchCore.create();
		booksShuffled().forEach((b) => core.tile(b.first, b.end, b.codes));
		expect(core.readings(block(bookCuts.length - 2))).toBe(true);
		const last = (word: string, nth = 1) => core.words(word, { nth, fromEnd: true }).total;
		expect(last('ומם')).toBeGreaterThan(0);
		expect(last('ומם')).toBe(last('וממ'));
		expect(last('ומם')).not.toBe(core.words('ומם', { nth: 1 }).total);
		expect(last('ומם', 2)).toBe(last('וממ', 2));
	});
});
