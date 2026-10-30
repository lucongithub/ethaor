import { beforeAll, describe, expect, it, vi } from 'vitest';
import { readContents, type Contents } from './book-view';
import { add, slotsOf, toggle, type Marked } from './marks';
import { countOf, ranked } from './excerpt';
import {
	colorsOf,
	crossed,
	countedWords,
	crossingsTitled,
	leadingFor,
	numberClass,
	numbered,
	numberedLetters,
	OWN,
	PAGE,
	readCrossings,
	readResults,
	VERSE_HEAD,
	stretches,
	type Passage
} from './results';
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
	results_groups(): number;
	results_groups_ptr(): number;
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

	// Three searches: ∩ lists every crossing of two at least, the widest
	// first, each crossing's passages holding exactly its searches' letters;
	// the crossings sum to the list. ∪ has none: one list, the text's order.
	it('are listed by crossing, the widest first', () => {
		const words = [
			['משה', 0],
			['אהרן', 1],
			['פרעה', 2]
		] as const;
		let list: Marked[] = [];
		for (const [word, slot] of words) {
			hand(core.els(word, 0, 0));
			module.marks_set(slot, word.length, 0);
			list = add(list, 'Consecutive letters', word);
		}
		const total = module.results_shown(1);
		const n = module.results_groups();
		const crossings = readCrossings(
			new Uint32Array(module.memory.buffer, module.results_groups_ptr(), 2 * n).slice()
		);
		const widths = crossings.map((c) => slotsOf(c.slots).length);
		expect(widths).toEqual([...widths].sort((a, b) => b - a));
		expect(widths[0]).toBe(3);
		expect(widths.includes(1)).toBe(false);
		expect(crossings.reduce((s, c) => s + c.passages, 0)).toBe(total);
		for (const c of crossings) {
			expect(crossed(list, c.slots).map((m) => m.word)).toEqual(
				slotsOf(c.slots).map((s) => words[s][0])
			);
			for (const passage of page(c.from, c.passages)) {
				const bits = passage.flatMap((v) => v.found).reduce((b, f) => b | f.slots, 0);
				expect(bits).toBe(c.slots);
			}
		}

		const any = module.results_shown(0);
		expect(module.results_groups()).toBe(0);
		const firsts = page(0, any).map((p) => p[0].verse);
		expect(firsts).toEqual([...firsts].sort((a, b) => a - b));
		module.results_clear();
		for (const [, slot] of words) module.marks_clear(slot);
	});

	// The module's pairs read as crossings, each knowing where it starts.
	it('read their crossings with where each starts', () => {
		expect(readCrossings([7, 2, 3, 5, 1, 4])).toEqual([
			{ slots: 7, passages: 2, from: 0 },
			{ slots: 3, passages: 5, from: 2 },
			{ slots: 1, passages: 4, from: 7 }
		]);
		expect(readCrossings([])).toEqual([]);
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

describe("a count's passages", () => {
	// The letters a selection holds — here verse 1's from its 4th letter on
	// and verse 2's first ten, as the book names them by rank — kept as a
	// search: its one passage holds both verses, every letter of the count
	// numbered from 1 in reading order, the numbers running on from one
	// verse to the next, and only those letters.
	it('number every letter counted, in order, across its verses', () => {
		// Three letters before the selection, twenty selected in verse 1.
		const ones = ranked(1, 'אבג', 'דהוזחטיכלמנסעפצקרשתא');
		const twos = ranked(2, '', 'אבגדהוזחטי');
		const found = countOf([...ones, ...twos]);
		expect(found.total).toBe(1);
		hand(found);
		expect(module.results_set(found.hits.length / 2, 1)).toBe(1);
		const [passage] = page(0, PAGE);
		module.results_clear();
		expect(passage.map((v) => v.verse)).toEqual([1, 2]);

		const pieces = numbered(passage);
		const numbers = pieces.flat(2).filter((p) => p.n > 0);
		expect(numbers.map((p) => p.n)).toEqual(numbers.map((_, k) => k + 1));
		expect(numbers.length).toBe(30);
		// Each numbered piece is one consonant with its marks, ours.
		for (const piece of numbers) expect(piece.text.match(/[א-ת]/gu)).toHaveLength(1);
		// Verse 1's 4th consonant is the first numbered; verse 2's first, the 21st.
		const consonants = (v: number) => passage[v].text.match(/[א-ת]/gu) ?? [];
		expect(numbers[0].text[0]).toBe(consonants(0)[3]);
		expect(numbers[20].text[0]).toBe(consonants(1)[0]);
		// Cut into pieces, every verse still reads whole.
		pieces.forEach((words, v) =>
			expect(words.map((w) => w.map((p) => p.text).join('')).join(' ')).toBe(passage[v].text)
		);
	});
});

describe('countedWords', () => {
	// A gematria's method shows the counted letters alone: a word keeps its
	// numbered letters, in order, across verses; a word with none goes, and
	// what is not a counted letter in a word goes too.
	it('keeps the counted letters alone, word by word, across verses', () => {
		const run = (text: string, n: number) => ({ text, slots: 0, n });
		const verses = [
			[
				[run('ב', 0), run('ר', 0)],
				[run('ב', 1), run('ְ', 0), run('ר', 2)]
			],
			[[run('א', 3)], [run('ת', 0)], [run('ו', 4), run('ה', 5)]]
		];
		expect(countedWords(verses).map((w) => w.map((p) => p.n))).toEqual([[1, 2], [3], [4, 5]]);
		expect(countedWords([])).toEqual([]);
	});
});

describe('numberedLetters', () => {
	// Each consonant with its marks is one piece, numbered on from `after`;
	// spaces stay between, unnumbered, and the pieces read the text whole.
	it('numbers each letter with its marks, on from where it is told', () => {
		const pieces = numberedLetters('בְּרֵאשִׁית בָּרָא', 4);
		expect(pieces.map((p) => p.text).join('')).toBe('בְּרֵאשִׁית בָּרָא');
		const letters = pieces.filter((p) => p.n > 0);
		expect(letters.map((p) => p.n)).toEqual([5, 6, 7, 8, 9, 10, 11, 12, 13]);
		expect(letters[0].text).toBe('בְּ');
		expect(pieces.find((p) => p.n === 0)?.text).toBe(' ');
		expect(numberedLetters('')).toEqual([]);
	});
});

describe('numberClass', () => {
	// Neighbours never share a tier: odd numbers just over their letter,
	// even ones a tier higher; neither selected nor taking the flow.
	it('sets odd and even numbers on two tiers, out of the flow and the selection', () => {
		const [odd, even] = [numberClass(1), numberClass(2)];
		expect(odd).not.toBe(even);
		expect(numberClass(3)).toBe(odd);
		expect(numberClass(446)).toBe(even);
		for (const c of [odd, even]) {
			expect(c).toContain('absolute');
			expect(c).toContain('select-none');
		}
	});

	// Longer numbers take more tiers: n neighbours in a row, each its own,
	// the lines made tall enough for all of them.
	it('takes as many tiers as asked, each neighbour its own, the lines tall enough', () => {
		const three = [1, 2, 3].map((n) => numberClass(n, 3));
		expect(new Set(three).size).toBe(3);
		expect(numberClass(4, 3)).toBe(three[0]);
		expect(numberClass(4, 4)).not.toBe(three[0]);
		expect(new Set([2, 3, 4].map(leadingFor)).size).toBe(3);
		expect(leadingFor(2)).toBe('leading-[2.6]');
	});
});

describe('crossingsTitled', () => {
	// A crossing's own title only among several: a single one would repeat
	// the list's title.
	it('titles the crossings only when there are several', () => {
		const one = { slots: 0b111, passages: 4, from: 0 };
		expect(crossingsTitled([one])).toBe(false);
		expect(crossingsTitled([])).toBe(false);
		expect(crossingsTitled([one, { slots: 0b11, passages: 2, from: 4 }])).toBe(true);
	});
});
