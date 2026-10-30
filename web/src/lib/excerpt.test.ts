import { describe, expect, it } from 'vitest';
import {
	copy,
	countOf,
	excerpt,
	joined,
	keepsCount,
	levelIn,
	MENU_GAP,
	menuAt,
	ranked
} from './excerpt';
import { Fault } from './fault';

describe('excerpt', () => {
	it('reads the selection with each run of white space made one space, nothing around it', () => {
		expect(excerpt('בְּרֵאשִׁית בָּרָא')).toBe('בְּרֵאשִׁית בָּרָא');
		expect(excerpt('\n\t בְּרֵאשִׁית\n\t\tבָּרָא  אֱלֹהִים \n')).toBe(
			'בְּרֵאשִׁית בָּרָא אֱלֹהִים'
		);
	});

	it('keeps every mark of a pointed letter, a selection cut inside a word included', () => {
		const word = 'בְּרֵאשִׁית';
		expect(excerpt(word.slice(0, 5))).toBe(word.slice(0, 5));
		expect(excerpt(word)).toHaveLength(word.length);
	});

	it('is empty for a selection of white space alone', () => {
		expect(excerpt('')).toBe('');
		expect(excerpt(' \n\t ')).toBe('');
	});
});

describe('joined', () => {
	it('puts each verse on its own line, read as one selection reads', () => {
		expect(joined(['\n בְּרֵאשִׁית  בָּרָא\n', 'וְהָאָרֶץ הָיְתָה '])).toBe(
			'בְּרֵאשִׁית בָּרָא\nוְהָאָרֶץ הָיְתָה'
		);
	});

	it('takes our forms index out, and leaves out the verses holding nothing', () => {
		expect(joined(['אֶת¹ הַשָּׁמַיִם', ' \n', 'וְאֵת²'])).toBe('אֶת הַשָּׁמַיִם\nוְאֵת');
		expect(joined([])).toBe('');
	});
});

describe('menuAt', () => {
	const within = { left: 100, top: 50, width: 600, height: 400 };
	const menu = { width: 160, height: 80 };
	const inside = (at: { left: number; top: number }) => {
		expect(at.left).toBeGreaterThanOrEqual(MENU_GAP);
		expect(at.top).toBeGreaterThanOrEqual(MENU_GAP);
		expect(at.left + menu.width).toBeLessThanOrEqual(within.width - MENU_GAP);
		expect(at.top + menu.height).toBeLessThanOrEqual(within.height - MENU_GAP);
	};

	it('stands centred under the selection when there is room', () => {
		const selected = { left: 300, top: 150, width: 120, height: 30 };
		const at = menuAt(selected, within, menu);
		expect(at.left + menu.width / 2).toBe(selected.left + selected.width / 2 - within.left);
		expect(at.top).toBe(selected.top + selected.height - within.top + MENU_GAP);
		inside(at);
	});

	it('stands over the selection when there is no room under it', () => {
		const selected = { left: 300, top: 380, width: 120, height: 30 };
		const at = menuAt(selected, within, menu);
		expect(at.top + menu.height + MENU_GAP).toBe(selected.top - within.top);
		inside(at);
	});

	it('stays whole inside its box, whatever the selection', () => {
		const edges = [
			{ left: 90, top: 40, width: 10, height: 10 },
			{ left: 690, top: 60, width: 30, height: 20 },
			{ left: 0, top: -200, width: 2000, height: 1000 },
			{ left: 400, top: 900, width: 50, height: 30 }
		];
		for (const selected of edges) inside(menuAt(selected, within, menu));
	});
});

describe('copy', () => {
	it('writes the text, and only that', async () => {
		const written: string[] = [];
		await copy('בָּרָא', async (text) => void written.push(text));
		expect(written).toEqual(['בָּרָא']);
	});

	it('turns a refused write into a numbered fault the user is told', async () => {
		const refused = copy('בָּרָא', () => Promise.reject(new DOMException('denied')));
		const fault = await refused.catch((error: unknown) => error);
		expect(fault).toBeInstanceOf(Fault);
		expect((fault as Fault).code).toBe('copy-failed');
		expect((fault as Fault).show).toBe(true);
		expect((fault as Fault).incident).toMatch(/^ETH-/);
	});

	it('turns a write that throws at once into the same fault', async () => {
		const fault = await copy('א', () => {
			throw new TypeError('no clipboard');
		}).catch((error: unknown) => error);
		expect((fault as Fault).code).toBe('copy-failed');
	});
});

describe('ranked', () => {
	it("names a verse's selected letters by their rank among its consonants", () => {
		// Two consonants before, four selected: ranks 2 to 5.
		expect(ranked(7, 'בְּרֵ', 'אשִׁית')).toEqual([7, 2, 7, 3, 7, 4, 7, 5]);
		expect(ranked(0, '', 'אֶת¹ הַ')).toEqual([0, 0, 0, 1, 0, 2]);
		expect(ranked(3, 'אב', ' ')).toEqual([]);
	});
});

describe('countOf', () => {
	it('keeps the letters as one hit of a letters record, none for no letter', () => {
		const found = countOf([1, 0, 1, 1, 2, 0]);
		expect(found).toMatchObject({ total: 1, stride: 6, form: 'letters' });
		expect([...found.hits]).toEqual([1, 0, 1, 1, 2, 0]);
		expect(countOf([])).toMatchObject({ total: 0, form: 'letters' });
		expect(countOf([]).hits).toHaveLength(0);
	});
});

describe('keepsCount', () => {
	// Only a count holding a verse's letter is kept in the panel; names and
	// numbers alone are told once.
	it('keeps a count holding a verse letter, and no other', () => {
		expect(keepsCount(countOf([4, 0]))).toBe(true);
		expect(keepsCount(countOf([]))).toBe(false);
	});
});

describe('levelIn', () => {
	// The book view's pieces counted as the map counts its levels: a verse
	// 3, a name or a number by its rank, anything else none.
	it("reads a piece's level from its markup", () => {
		expect(levelIn('12', undefined)).toBe(3);
		expect(levelIn(undefined, '0')).toBe(0);
		expect(levelIn(undefined, '1')).toBe(1);
		expect(levelIn(undefined, '2')).toBe(2);
		expect(levelIn(undefined, '')).toBeNull();
		expect(levelIn(undefined, '3')).toBeNull();
		expect(levelIn(undefined, undefined)).toBeNull();
	});
});
