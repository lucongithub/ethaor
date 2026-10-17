import { beforeAll, describe, expect, it } from 'vitest';
import {
	estimate,
	kept,
	missing,
	readChapter,
	SIZE,
	SIZE_MAX,
	SIZE_MIN,
	SIZE_STEP,
	zoomed,
	type Chapter
} from './book-view';
import { block, booksShuffled, read } from './tests/artifacts';

/** The map module's exports the book view reads through the bridge. */
interface Module {
	memory: WebAssembly.Memory;
	init(width: number, height: number): number;
	tile_buffer(length: number): number;
	tile_load(first: number, end: number): number;
	readings_load(): number;
	text_ptr(): number;
	chapter_count(): number;
	chapter_landed(chapter: number): number;
	chapter_record(chapter: number): number;
	chapter_record_ptr(): number;
}

function fresh(): Module {
	const module = new WebAssembly.Instance(new WebAssembly.Module(read('ethaor.wasm')), {})
		.exports as unknown as Module;
	module.init(1280, 800);
	return module;
}

function land(module: Module, first: number, end: number, codes: Uint8Array) {
	const at = module.tile_buffer(codes.length);
	new Uint8Array(module.memory.buffer, at, codes.length).set(codes);
	module.tile_load(first, end);
}

/** A chapter read as the bridge reads it: record copied, strings decoded by bytes. */
function chapterOf(module: Module, chapter: number): Chapter | null {
	const words = module.chapter_record(chapter);
	if (words === 0) return null;
	const record = new Uint32Array(module.memory.buffer, module.chapter_record_ptr(), words).slice();
	const decoder = new TextDecoder();
	const text = (offset: number, length: number) =>
		decoder.decode(new Uint8Array(module.memory.buffer, module.text_ptr() + offset, length));
	return readChapter(chapter, record, text);
}

describe('a chapter read from the module', () => {
	let module: Module;
	beforeAll(() => {
		module = fresh();
		for (const { first, end, codes } of booksShuffled()) land(module, first, end, codes);
		const readings = block(39);
		const at = module.tile_buffer(readings.length);
		new Uint8Array(module.memory.buffer, at, readings.length).set(readings);
		expect(module.readings_load()).toBe(1);
	});

	// Every chapter reads whole: its number, its book's and portion's names,
	// every verse numbered and holding text, Genesis opening the Torah.
	it('gives every chapter with its names and verses', () => {
		const count = module.chapter_count();
		expect(count).toBe(929);
		let opening = 0;
		for (let c = 0; c < count; c++) {
			const chapter = chapterOf(module, c)!;
			expect(chapter.label.length, `chapter ${c}`).toBeGreaterThan(0);
			expect(chapter.bookName.length).toBeGreaterThan(0);
			expect(chapter.verses.length).toBeGreaterThan(0);
			for (const verse of chapter.verses) {
				expect(verse.label.length).toBeGreaterThan(0);
				expect(verse.parts.map((p) => p.text).join('').length).toBeGreaterThan(0);
			}
			if (chapter.opens) opening++;
		}
		expect(opening).toBe(39);
		const genesis = chapterOf(module, 0)!;
		expect([genesis.bookName, genesis.sectionName, genesis.label]).toEqual(['בראשית', 'תורה', 'א']);
		const first = genesis.verses[0].parts.map((p) => p.text).join('');
		expect(first.replace(/[֑-ׇ]/g, '')).toMatch(/^בראשית ברא אלהים/);
	});

	// The other edition's forms are cut out of the verse as their own parts —
	// each carrying its index, never ours — and the parts put back together
	// are the verse as shown, with nothing lost at the cuts.
	it('cuts the other reading out of the verse as its own parts', () => {
		let theirs = 0;
		for (let c = 0; c < 929; c++) {
			const chapter = chapterOf(module, c)!;
			const holds = chapter.verses.some((v) => v.parts.some((p) => p.theirs));
			expect(chapter.readings, `chapter ${c}`).toBe(holds);
			for (const verse of chapter.verses) {
				for (const part of verse.parts.filter((p) => p.theirs)) {
					theirs++;
					expect(part.text).toContain('²');
					expect(part.text).not.toContain('¹');
				}
				const ours = verse.parts.filter((p) => !p.theirs).map((p) => p.text);
				if (verse.parts.some((p) => p.theirs)) expect(ours.join('')).toContain('¹');
				expect(verse.parts.every((p) => p.text.length > 0)).toBe(true);
			}
		}
		expect(theirs).toBe(94);
	});

	// Our form is cut out alone, just before theirs: one word carrying `¹`,
	// no space in it — what the pointer must be on for the index to open —
	// and every `¹` of a verse is in such a part, never left in our text.
	it('cuts our marked form out alone, before theirs', () => {
		let marked = 0;
		for (let c = 0; c < 929; c++) {
			for (const verse of chapterOf(module, c)!.verses) {
				verse.parts.forEach((part, k) => {
					if (part.theirs) {
						const before = verse.parts[k - 1];
						expect(before?.marked, `verse ${verse.index}`).toBe(true);
					}
					if (part.marked) {
						marked++;
						expect(part.text.endsWith('¹') && !part.text.includes(' ')).toBe(true);
						expect(verse.parts[k + 1]?.theirs).toBe(true);
					}
					if (!part.marked && !part.theirs) expect(part.text).not.toContain('¹');
				});
			}
		}
		expect(marked).toBe(94);
	});
});

// Before its verses land, a chapter has no record and the bridge reads
// nothing; once its book is in, it reads.
it('reads nothing of a chapter not landed, then reads it', () => {
	const module = fresh();
	expect(module.chapter_landed(0)).toBe(0);
	expect(chapterOf(module, 0)).toBeNull();
	const genesis = booksShuffled().find((b) => b.first === 0)!;
	land(module, genesis.first, genesis.end, genesis.codes);
	expect(module.chapter_landed(0)).toBe(1);
	expect(chapterOf(module, 0)?.verses.length).toBe(31);
	expect(chapterOf(module, 929)).toBeNull();
});

describe('estimate', () => {
	// The height a chapter not laid out is given grows with its text and its
	// size, shrinks with a wider column, and never falls under one line a
	// verse.
	it('follows the text, the size and the column', () => {
		const base = estimate(4000, 25, false, SIZE, 600);
		expect(estimate(8000, 25, false, SIZE, 600)).toBeGreaterThan(base);
		expect(estimate(4000, 25, false, SIZE * 2, 600)).toBeGreaterThan(base);
		expect(estimate(4000, 25, false, SIZE, 1200)).toBeLessThan(base);
		expect(estimate(4000, 25, true, SIZE, 600)).toBeGreaterThan(base);
		expect(estimate(10, 25, false, SIZE, 5000)).toBeGreaterThanOrEqual(25 * SIZE);
		expect(Number.isFinite(estimate(4000, 25, false, SIZE, 0))).toBe(true);
	});
});

describe('zoomed', () => {
	// One step is the browser's own, fractions come from a trackpad, and the
	// size stays within its bounds whatever the gesture.
	it('steps the size within its bounds', () => {
		expect(zoomed(SIZE, 0)).toBe(SIZE);
		expect(zoomed(SIZE, 1)).toBeCloseTo(SIZE * SIZE_STEP, 2);
		expect(zoomed(SIZE, -1)).toBeCloseTo(SIZE / SIZE_STEP, 2);
		expect(zoomed(SIZE, 0.5)).toBeGreaterThan(SIZE);
		expect(zoomed(SIZE, 1000)).toBe(SIZE_MAX);
		expect(zoomed(SIZE, -1000)).toBe(SIZE_MIN);
		expect(zoomed(SIZE, NaN)).toBe(SIZE);
		expect(zoomed(SIZE, Infinity)).toBe(SIZE);
	});
});

describe('missing', () => {
	// Chapters not in come as runs of neighbours, one request each; those in
	// break the runs.
	it('groups the chapters not in into runs', () => {
		const landed = new Set([3, 4, 8]);
		expect(missing(2, 10, (c) => landed.has(c))).toEqual([
			[2, 3],
			[5, 8],
			[9, 10]
		]);
		expect(missing(3, 5, (c) => landed.has(c))).toEqual([]);
		expect(missing(5, 5, () => false)).toEqual([]);
	});
});

describe('kept', () => {
	// The chapters laid out run a few each way past those on screen, never
	// past the Tanakh's ends, in either order of the two.
	it('reaches around the screen within the Tanakh', () => {
		expect(kept(10, 12, 2, 929)).toEqual({ from: 8, to: 15 });
		expect(kept(12, 10, 2, 929)).toEqual({ from: 8, to: 15 });
		expect(kept(0, 0, 2, 929)).toEqual({ from: 0, to: 3 });
		expect(kept(928, 928, 2, 929)).toEqual({ from: 926, to: 929 });
	});
});
