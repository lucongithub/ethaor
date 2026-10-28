import { describe, expect, it } from 'vitest';
import { inked, pick, picked, words } from './passage';

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

	it('cuts each word into the runs the searches found, by bytes, the word given back whole', () => {
		const bytes = (s: string) => new TextEncoder().encode(s).length;
		// The letters of בָּרָא, the second word: from its first byte to its end.
		const second = bytes('בְּרֵאשִׁית ');
		const found = [{ from: second, to: second + bytes('בָּרָא'), slots: 1 << 2 }];
		const runs = inked(verse, found);
		expect(runs.map((w) => w.map((r) => r.text).join(''))).toEqual(words(verse));
		expect(runs[1]).toEqual([{ text: 'בָּרָא', slots: 1 << 2 }]);
		expect(runs[0]).toEqual([{ text: 'בְּרֵאשִׁית', slots: 0 }]);

		// Part of a word, across a space into the next; a letter two slots
		// found, as the module hands it — one range with both bits.
		const ra = bytes('בְּרֵאשִׁית בָּ');
		const parts = inked(verse, [
			{ from: ra, to: ra + bytes('רָ'), slots: 0b1001 },
			{ from: ra + bytes('רָ'), to: second + bytes('בָּרָא') + 1 + bytes('אֱ'), slots: 0b0001 }
		]);
		expect(parts[1]).toEqual([
			{ text: 'בָּ', slots: 0 },
			{ text: 'רָ', slots: 0b1001 },
			{ text: 'א', slots: 0b0001 }
		]);
		expect(parts[2]).toEqual([
			{ text: 'אֱ', slots: 0b0001 },
			{ text: 'לֹהִים', slots: 0 }
		]);
		expect(inked(verse, []).every((w) => w.length === 1 && w[0].slots === 0)).toBe(true);
	});
});
