import { describe, expect, it } from 'vitest';
import { erase, insert, KEYS, letters } from './letters';

describe('KEYS', () => {
	it('holds the 22 letters and the 5 finals once each, in alphabetical order, finals after their letter', () => {
		expect(KEYS).toHaveLength(27);
		expect(new Set(KEYS).size).toBe(27);
		// The Unicode block lists the alphabet with each final before its
		// letter; the keyboard puts it after.
		const block = Array.from({ length: 27 }, (_, k) => String.fromCharCode(0x05d0 + k));
		const finals = ['ך', 'ם', 'ן', 'ף', 'ץ'];
		for (const final of finals) {
			const at = block.indexOf(final);
			[block[at], block[at + 1]] = [block[at + 1], block[at]];
		}
		expect(KEYS).toEqual(block);
	});
});

describe('letters', () => {
	it('keeps the consonants, finals included, and drops marks, spaces, Latin and punctuation', () => {
		expect(letters('בְּרֵאשִׁית')).toBe('בראשית');
		expect(letters('אָדָם וְחַוָּה')).toBe('אדםוחוה');
		expect(letters('Gen 1:1 תורה!')).toBe('תורה');
		expect(letters('')).toBe('');
	});
});

describe('insert', () => {
	it('puts the letter at the caret and moves the caret past it', () => {
		expect(insert('', 0, 0, 'א')).toEqual({ word: 'א', caret: 1 });
		expect(insert('תרה', 1, 1, 'ו')).toEqual({ word: 'תורה', caret: 2 });
		expect(insert('תור', 3, 3, 'ה')).toEqual({ word: 'תורה', caret: 4 });
	});

	it('replaces the selection, whichever way it was drawn', () => {
		expect(insert('אבגד', 1, 3, 'ת')).toEqual({ word: 'אתד', caret: 2 });
		expect(insert('אבגד', 3, 1, 'ת')).toEqual({ word: 'אתד', caret: 2 });
	});

	it('keeps offsets past the word inside it', () => {
		expect(insert('אב', 9, 9, 'ג')).toEqual({ word: 'אבג', caret: 3 });
		expect(insert('אב', -4, -1, 'ג')).toEqual({ word: 'גאב', caret: 1 });
	});

	it('inserts nothing that is not a letter', () => {
		expect(insert('אב', 1, 1, 'x')).toEqual({ word: 'אב', caret: 1 });
	});
});

describe('erase', () => {
	it('erases the letter before the caret, nothing at the start', () => {
		expect(erase('תורה', 4, 4)).toEqual({ word: 'תור', caret: 3 });
		expect(erase('תורה', 2, 2)).toEqual({ word: 'תרה', caret: 1 });
		expect(erase('תורה', 0, 0)).toEqual({ word: 'תורה', caret: 0 });
	});

	it('erases the selection alone when there is one', () => {
		expect(erase('אבגד', 1, 3)).toEqual({ word: 'אד', caret: 1 });
		expect(erase('אבגד', 3, 1)).toEqual({ word: 'אד', caret: 1 });
	});
});
