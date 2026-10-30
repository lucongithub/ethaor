import { describe, expect, it } from 'vitest';
import {
	formLetter,
	formula,
	letterAs,
	readGematria,
	tiersFor,
	type Gematria,
	type GematriaExports
} from './gematria';
import { numberedLetters } from './results';
import { map } from './tests/artifacts';

const wasm = map as unknown as GematriaExports;

/** A reading, or the test fails with why it is none. */
function reading(text: string): Gematria {
	const read = readGematria(wasm, text);
	if (typeof read === 'string') throw new Error(`not read: ${read}`);
	return read;
}

describe('readGematria', () => {
	// The real map module reads תורה: every method, the standard value first
	// at 611; AtBash reading it אפגצ, 1 + 80 + 3 + 90; the words counted.
	it('reads a text by every method, each letter as the method reads it and its share', () => {
		const torah = reading('תּוֹרָה');
		expect(torah.letters).toBe(4);
		expect(torah.words).toBe(1);
		expect(torah.methods).toHaveLength(25);
		const [hapanim] = torah.methods;
		expect([hapanim.key, hapanim.name, hapanim.hebrew, hapanim.value]).toEqual([
			'mispar_hapanim',
			'Mispar haPanim',
			'מספר הפנים',
			611
		]);
		expect(hapanim.replaces).toBe(false);
		expect([...hapanim.readAs].map(formLetter).join('')).toBe('תורה');
		const atbash = torah.methods.find((m) => m.key === 'atbash')!;
		expect(atbash.replaces).toBe(true);
		expect([...atbash.readAs].map(formLetter).join('')).toBe('אפגצ');
		expect([...atbash.shares]).toEqual([1, 80, 3, 90]);
		for (const m of torah.methods) {
			const sum = m.shares.reduce((a, b) => a + b, 0);
			expect(sum, m.key).toBe(m.summed ? m.value : 611);
		}
		expect(torah.methods.filter((m) => !m.summed).map((m) => m.key)).toEqual([
			'mispar_hamerubah_haklali',
			'mispar_katan_mispari',
			'mispar_kolel'
		]);
	});

	// No Hebrew letter, or more than the module reads exactly: told why.
	it('tells a text with no letter, or too long, from a reading', () => {
		expect(readGematria(wasm, 'abc')).toBe('no-letter');
		expect(readGematria(wasm, '')).toBe('no-letter');
		expect(readGematria(wasm, 'א'.repeat(10_001))).toBe('too-long');
		expect(reading('א'.repeat(10_000)).letters).toBe(10_000);
	});
});

describe('letterAs', () => {
	// A replacing method shows the letter it reads, bare; another the letter
	// as written; each its share over it — without a method, its number.
	it('shows each letter as its method reads it, its share over it', () => {
		const torah = reading('תּוֹרָה');
		const pieces = numberedLetters('תּוֹרָה').filter((p) => p.n > 0);
		const atbash = torah.methods.find((m) => m.key === 'atbash')!;
		expect(pieces.map((p) => letterAs(atbash, p).text).join('')).toBe('אפגצ');
		expect(pieces.map((p) => letterAs(atbash, p).over)).toEqual(['1', '80', '3', '90']);
		const meshulash = torah.methods.find((m) => m.key === 'mispar_meshulash')!;
		expect(letterAs(meshulash, pieces[0])).toEqual({ text: 'תּ', over: '64 000 000' });
		expect(letterAs(null, pieces[3])).toEqual({ text: 'ה', over: '4' });
		expect(letterAs(atbash, { text: 'א', slots: 0, n: 5 }).over).toBe('');
	});
});

describe('formula', () => {
	// The three methods no sum of their letters say how they reach their
	// value from the standard one; the others say nothing.
	it('says how Kolel, HaMerubah HaKlali and Katan Mispari reach their value', () => {
		const torah = reading('תורה אחת');
		const of = (key: string) =>
			formula(
				torah,
				torah.methods.find((m) => m.key === key)!
			);
		expect(of('mispar_kolel')).toBe('1 020 + 2 words = 1 022');
		expect(of('mispar_hamerubah_haklali')).toBe('1 020² = 1 040 400');
		expect(of('mispar_katan_mispari')).toBe('1 020 → 3');
		expect(of('atbash')).toBe('');
		expect(formula(reading('א'), reading('א').methods.at(-8)!)).toContain('1 word =');
	});
});

describe('tiersFor', () => {
	// Short shares keep the count's two tiers; longer ones take more.
	it('takes a tier more for longer numbers', () => {
		expect(tiersFor([1, 400])).toBe(2);
		expect(tiersFor([1, 4_000])).toBe(2);
		expect(tiersFor([160_000])).toBe(3);
		expect(tiersFor([64_000_000])).toBe(4);
	});
});
