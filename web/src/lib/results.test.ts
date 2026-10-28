import { beforeAll, describe, expect, it, vi } from 'vitest';
import { readContents, type Contents } from './book-view';
import { add, toggle, type Marked } from './marks';
import { colorsOf, OWN, PAGE, readResults, VERSE_HEAD, stretches, type Passage } from './results';
import { SearchCore, type Found } from './search-core';
import { block, bookCuts, booksShuffled, map, read, serveArtifacts } from './tests/artifacts';

/** The map module's exports the list reads through the bridge. */
interface Module {
	memory: WebAssembly.Memory;
	init(width: number, height: number): number;
	tile_buffer(length: number): number;
	tile_load(first: number, end: number): number;
	readings_load(): number;
	text_ptr(): number;
	contents_record(): number;
	contents_ptr(): number;
	marks_buffer(words: number): number;
	marks_set(slot: number, span: number, form: number): number;
	results_set(span: number, form: number): number;
	results_page(from: number, count: number): number;
	results_ptr(): number;
	results_clear(): void;
	results_shown(all: number): number;
	passages_shown(all: number): number;
	passages_count(span: number, form: number): number;
	marks_clear(slot: number): void;
}

const decoder = new TextDecoder();
let module: Module;
let core: SearchCore;
let contents: Contents;

const text = (offset: number, length: number) =>
	decoder.decode(new Uint8Array(module.memory.buffer, module.text_ptr() + offset, length));

/** A search's hits handed to the map module, as the bridge hands them. */
function hand(found: Found) {
	const at = module.marks_buffer(found.hits.length);
	new Uint32Array(module.memory.buffer, at, found.hits.length).set(found.hits);
}

/** A page of passages read as the bridge reads it. */
function page(from: number, count: number): Passage[] {
	const words = module.results_page(from, count);
	return readResults(
		new Uint32Array(module.memory.buffer, module.results_ptr(), words).slice(),
		text
	);
}

/** Letters of a run of our verse: its consonants, finals folded. */
const letters = (s: string) =>
	(s.match(/[א-ת]/g) ?? [])
		.map((c) => ({ ך: 'כ', ם: 'מ', ן: 'נ', ף: 'פ', ץ: 'צ' })[c] ?? c)
		.join('');

const cut = (verse: string, from: number, to: number) =>
	decoder.decode(new TextEncoder().encode(verse).subarray(from, to));

beforeAll(async () => {
	vi.stubGlobal('fetch', serveArtifacts());
	module = new WebAssembly.Instance(new WebAssembly.Module(read('ethaor.wasm')), {})
		.exports as unknown as Module;
	module.init(1280, 800);
	core = await SearchCore.create();
	const readings = block(bookCuts.length - 2);
	for (const { first, end, codes } of booksShuffled()) {
		const at = module.tile_buffer(codes.length);
		new Uint8Array(module.memory.buffer, at, codes.length).set(codes);
		module.tile_load(first, end);
		core.tile(first, end, codes);
	}
	const at = module.tile_buffer(readings.length);
	new Uint8Array(module.memory.buffer, at, readings.length).set(readings);
	expect(module.readings_load()).toBe(1);
	expect(core.readings(readings)).toBe(true);
	const words = module.contents_record();
	contents = readContents(
		new Uint32Array(module.memory.buffer, module.contents_ptr(), words).slice(),
		text
	);
});

describe("a search's passages", () => {
	// The module's own bit is the page's: one past the slots it marks.
	it('carry its own letters under the bit past every slot', () => {
		expect(OWN).toBe(1 << map.marks_slots());
		expect(VERSE_HEAD).toBe(8);
		expect(PAGE).toBeGreaterThan(0);
	});

	// Read page by page, the list holds every hit whole: its own letters,
	// across all passages, spell the word once per hit — with no slot set,
	// the search hidden from the map — and every verse is ours, numbered,
	// its chapter in its book.
	it('spell the word once per hit, every verse whole, hidden or not', () => {
		const found = core.els('משה', 0, 0);
		hand(found);
		const count = module.results_set(3, 0);
		expect(count).toBeGreaterThan(PAGE);
		const all: Passage[] = [];
		for (let from = 0; from < count; from += PAGE) all.push(...page(from, PAGE));
		expect(all.length).toBe(count);
		expect(page(count, PAGE)).toEqual([]);

		const own = all
			.flat()
			.flatMap((v) => v.found.filter((f) => f.slots & OWN).map((f) => cut(v.text, f.from, f.to)))
			.map(letters)
			.join('');
		expect(own).toBe('משה'.repeat(found.hits.length / found.stride));
		for (const verse of all.flat()) {
			const book = contents.books[verse.book];
			expect(verse.chapter, `verse ${verse.verse}`).toBeGreaterThanOrEqual(book.first);
			expect(verse.chapter, `verse ${verse.verse}`).toBeLessThan(book.end);
			expect(verse.number.length).toBeGreaterThan(0);
			expect(verse.text.length).toBeGreaterThan(0);
		}
		module.results_clear();
		expect(page(0, PAGE)).toEqual([]);
	});

	// A first-letters search's records name each letter by place: listed in
	// that form, its own letters across all passages are, hit by hit, the
	// first letter of a word each, spelling the word; read as ELS records
	// they would say something else.
	it('list a first-letters search letter by letter where each lies', () => {
		const found = core.words('משה', { nth: 1 });
		expect(found.total).toBeGreaterThan(0);
		hand(found);
		const count = module.results_set(3, 1);
		hand(found);
		expect(module.passages_count(3, 1)).toBe(count);
		const all: Passage[] = [];
		for (let from = 0; from < count; from += PAGE) all.push(...page(from, PAGE));
		const own = all
			.flat()
			.flatMap((v) => v.found.filter((f) => f.slots & OWN).map((f) => cut(v.text, f.from, f.to)));
		expect(own.map(letters).join('')).toBe('משה'.repeat(found.hits.length / found.stride));
		for (const verse of all.flat())
			for (const f of verse.found.filter((f) => f.slots & OWN)) {
				// Each own letter opens a word: a space, or the verse, before it.
				const before = cut(verse.text, 0, f.from);
				expect(before === '' || before.endsWith(' '), `${verse.verse}: ${before}`).toBe(true);
			}
		hand(found);
		expect(module.results_set(3, 0)).not.toBe(count);
		module.results_clear();
	});

	// A hit read across a verse break is one passage of both verses, each
	// with its share of the word.
	it('show every verse a hit reaches', () => {
		const found = core.els('אב', 0, 0);
		const across = Array.from({ length: found.hits.length / found.stride }, (_, k) =>
			found.hits.subarray(k * found.stride, (k + 1) * found.stride)
		).find((r) => r[4] !== r[6])!;
		hand({ total: 1, hits: across, stride: found.stride });
		expect(module.results_set(2, 0)).toBe(1);
		const [passage] = page(0, PAGE);
		expect(passage.map((v) => v.verse)).toEqual([across[4], across[6]]);
		expect(passage.every((v) => v.found.some((f) => f.slots & OWN))).toBe(true);
		const spelled = passage
			.map((v) =>
				v.found
					.filter((f) => f.slots & OWN)
					.map((f) => letters(cut(v.text, f.from, f.to)))
					.join('')
			)
			.join('');
		expect(spelled).toBe('אב');
	});

	// Each passage names where it lies: Genesis 1 in the Torah. One that
	// reaches into another chapter, book or portion names each again where
	// it changes, its verses kept with their place.
	it('name where they lie, portion, book and chapter, again wherever it changes', () => {
		hand(core.els('בראשית', 0, 0));
		module.results_set(6, 0);
		const [first] = page(0, 1);
		expect(stretches(first, contents)).toEqual([
			{ place: { portion: 'תורה', book: 'בראשית', chapter: 'א' }, verses: first }
		]);
		// Deuteronomy's last chapter into Joshua's first: a new book and a
		// new portion; two verses of one chapter stay one run.
		const end = contents.books[4].end - 1;
		const next = contents.books[4].end;
		const verse = first[0];
		const crossing: Passage = [
			{ ...verse, book: 4, chapter: end, verse: 1 },
			{ ...verse, book: 4, chapter: end, verse: 2 },
			{ ...verse, book: 5, chapter: next, verse: 3 }
		];
		const runs = stretches(crossing, contents);
		expect(runs.map((r) => r.place)).toEqual([
			{ portion: 'תורה', book: contents.books[4].name, chapter: contents.labels[end] },
			{ portion: 'נביאים', book: contents.books[5].name, chapter: contents.labels[next] }
		]);
		expect(runs.map((r) => r.verses.map((v) => v.verse))).toEqual([[1, 2], [3]]);
		const within: Passage = [verse, { ...verse, chapter: 1, verse: 9 }];
		expect(stretches(within, contents).map((r) => r.place.chapter)).toEqual(['א', 'ב']);
	});

	// A word read every two hundred letters: its passages hold the verses its
	// letters lie in, the ones between left out.
	it('of a word read far apart hold its verses alone, the ones between left out', () => {
		hand(core.els('אדם', 200, 200));
		const count = module.results_set(3, 0);
		const passages = page(0, count);
		expect(passages.length).toBe(count);
		const lone = passages.filter((p) => p.length === 3);
		expect(lone.length).toBeGreaterThan(0);
		for (const passage of lone) {
			const next = passage.slice(1).map((v, k) => v.verse === passage[k].verse + 1);
			expect(next.every(Boolean)).toBe(false);
			for (const verse of passage) expect(verse.found.some((f) => f.slots & OWN)).toBe(true);
		}
	});
});

describe('the searches shown together', () => {
	// A search's count is the length of its list; the passages of any and
	// of all are listed as they are counted, and each passage of all holds
	// letters of both searches, each in its slot.
	it('are counted as listed, each passage of all holding both', () => {
		const words = [
			['משה', 1],
			['אהרן', 2]
		] as const;
		for (const [word, slot] of words) {
			const found = core.els(word, 0, 0);
			hand(found);
			const count = module.passages_count(word.length, 0);
			hand(found);
			expect(module.results_set(word.length, 0)).toBe(count);
			hand(found);
			module.marks_set(slot, word.length, 0);
		}
		const [any, all] = [module.passages_shown(0), module.passages_shown(1)];
		expect(all).toBeGreaterThan(0);
		expect(all).toBeLessThan(any);
		expect(module.results_shown(1)).toBe(all);
		const listed: Passage[] = [];
		for (let from = 0; from < all; from += PAGE) listed.push(...page(from, PAGE));
		expect(listed.length).toBe(all);
		for (const passage of listed) {
			const bits = passage.flatMap((v) => v.found).reduce((b, f) => b | f.slots, 0);
			expect(bits & OWN).toBe(0);
			expect(bits).toBe((1 << 1) | (1 << 2));
		}
		expect(module.results_shown(0)).toBe(any);
		module.results_clear();
		module.marks_clear(1);
		module.marks_clear(2);
	});
});

describe("a passage's colours", () => {
	// The search listed wears its colour on its own letters whether it is
	// shown or hidden; the shown ones theirs, lowest slot first; never twice.
	it('always show the search listed, and the others shown', () => {
		let list: Marked[] = add(add([], 'Consecutive letters', 'משה'), 'Consecutive letters', 'אהרן');
		const [moses, aaron] = list;
		expect(colorsOf(list, moses, OWN | (1 << moses.slot))).toEqual([moses.color]);
		expect(colorsOf(list, moses, OWN | (1 << aaron.slot))).toEqual([moses.color, aaron.color]);
		expect(colorsOf(list, moses, 1 << aaron.slot)).toEqual([aaron.color]);
		list = toggle(list, moses.id);
		const hidden = list.find((m) => m.id === moses.id)!;
		expect(colorsOf(list, hidden, OWN)).toEqual([moses.color]);
		expect(colorsOf(list, hidden, OWN | (1 << aaron.slot))).toEqual([moses.color, aaron.color]);
		expect(colorsOf(list, hidden, 0)).toEqual([]);
		// The searches shown together: no search of its own, their colours alone.
		expect(colorsOf(list, null, OWN | (1 << aaron.slot))).toEqual([aaron.color]);
	});
});
