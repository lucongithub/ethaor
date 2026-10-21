import { describe, expect, it } from 'vitest';
import { pick, picked, words } from './passage';

const verse = 'בְּרֵאשִׁית בָּרָא אֱלֹהִים';

describe('passage', () => {
	it('cuts a verse at its spaces, every pointed letter whole', () => {
		expect(words(verse)).toEqual(['בְּרֵאשִׁית', 'בָּרָא', 'אֱלֹהִים']);
		expect(words(verse).join(' ')).toBe(verse);
	});

	it('picks each word in place: head, word and tail give the verse back', () => {
		words(verse).forEach((word, k) => {
			const at = pick(verse, k)!;
			expect(at.word).toBe(word);
			expect(at.head + at.word + at.tail, `word ${k}`).toBe(verse);
			expect(picked({ ...at }), `word ${k} read back`).toBe(k);
		});
		expect(pick(verse, 0)!.head).toBe('');
		expect(pick(verse, 2)!.tail).toBe('');
	});

	it('picks nothing out of the verse, and reads no word picked as none', () => {
		expect(pick(verse, -1)).toBeNull();
		expect(pick(verse, 3)).toBeNull();
		expect(picked({ word: '', head: '' })).toBe(-1);
	});
});
