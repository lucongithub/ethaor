import { describe, expect, it } from 'vitest';
import { command, escape, PAN, PAN_FAR } from './keys';

const key = (
	name: string,
	held: Partial<Record<'shift' | 'ctrl' | 'meta' | 'alt', boolean>> = {}
) => ({
	key: name,
	shiftKey: held.shift ?? false,
	ctrlKey: held.ctrl ?? false,
	metaKey: held.meta ?? false,
	altKey: held.alt ?? false
});

describe('command', () => {
	it('zooms one notch in on + or =, out on - or _', () => {
		expect(command(key('+'))).toEqual({ kind: 'zoom', steps: 1 });
		expect(command(key('='))).toEqual({ kind: 'zoom', steps: 1 });
		expect(command(key('-'))).toEqual({ kind: 'zoom', steps: -1 });
		expect(command(key('_', { shift: true }))).toEqual({ kind: 'zoom', steps: -1 });
	});

	it('moves the view the way the arrow points, by a small share, a large one with Shift', () => {
		expect(command(key('ArrowLeft'))).toEqual({ kind: 'pan', dx: -PAN, dy: 0 });
		expect(command(key('ArrowRight'))).toEqual({ kind: 'pan', dx: PAN, dy: 0 });
		expect(command(key('ArrowUp'))).toEqual({ kind: 'pan', dx: 0, dy: -PAN });
		expect(command(key('ArrowDown', { shift: true }))).toEqual({ kind: 'pan', dx: 0, dy: PAN_FAR });
		expect(PAN_FAR).toBeGreaterThan(PAN);
	});

	it('selects the passage at the centre on Enter', () => {
		expect(command(key('Enter'))).toEqual({ kind: 'select' });
	});

	it('leaves every key held with Ctrl, Cmd or Alt to the browser, Ctrl + included', () => {
		for (const held of ['ctrl', 'meta', 'alt'] as const) {
			for (const name of ['+', '-', 'ArrowLeft', 'Enter']) {
				expect(command(key(name, { [held]: true }))).toBeNull();
			}
		}
	});

	it('gives nothing for a key it does not know, Escape and Tab included', () => {
		for (const name of ['a', ' ', 'Tab', 'Escape', 'Home']) expect(command(key(name))).toBeNull();
	});
});

describe('escape', () => {
	// A count or gematria told once lies over everything: Escape closes it
	// before anything else, in the book too.
	it('closes a count told once before anything else', () => {
		const all = { results: true, selection: true, notice: true, searches: true };
		expect(escape({ info: true, ...all })).toBe('info');
		expect(escape({ info: true, selection: false, notice: false, book: true })).toBe('info');
		expect(escape({ info: false, ...all })).toBe('selection');
	});

	// A search's passages lie over everything but the passage a click in
	// them opens, which lies over them: Escape closes that one, then them.
	it('closes the passages of a search first, after the passage opened from them', () => {
		expect(escape({ results: true, selection: true, notice: true, searches: true })).toBe(
			'selection'
		);
		expect(escape({ results: true, selection: false, notice: true, searches: true })).toBe(
			'results'
		);
		expect(escape({ results: true, selection: false, notice: false, book: true })).toBe('results');
		expect(escape({ results: false, selection: false, notice: false, searches: true })).toBe(
			'searches'
		);
	});

	it('closes an open passage before anything else, the view kept', () => {
		expect(escape({ selection: true, notice: true })).toBe('selection');
		expect(escape({ selection: true, notice: false })).toBe('selection');
	});

	it('closes a notice next', () => {
		expect(escape({ selection: false, notice: true })).toBe('notice');
	});

	// The searches' panel lying over the view is closed after the passage and
	// the notice, before the overview — in the book view too.
	it('closes the searches lying over the view next', () => {
		expect(escape({ selection: true, notice: false, searches: true })).toBe('selection');
		expect(escape({ selection: false, notice: true, searches: true })).toBe('notice');
		expect(escape({ selection: false, notice: false, searches: true })).toBe('searches');
		expect(escape({ selection: false, notice: false, searches: true, book: true })).toBe(
			'searches'
		);
	});

	// A stretch selected, its menu beside it: after the passage and the
	// list it may be read from, before the notice — in the book view too.
	it('puts a stretch selected away after the passage and the list, before the notice', () => {
		expect(escape({ selection: true, excerpt: true, notice: true })).toBe('selection');
		expect(escape({ results: true, selection: false, excerpt: true, notice: true })).toBe(
			'results'
		);
		expect(escape({ selection: false, excerpt: true, notice: true, searches: true })).toBe(
			'excerpt'
		);
		expect(escape({ selection: false, excerpt: true, notice: false, book: true })).toBe('excerpt');
		expect(escape({ selection: false, excerpt: false, notice: false })).toBe('overview');
	});

	it('goes back to the overview only with nothing open', () => {
		expect(escape({ selection: false, notice: false })).toBe('overview');
	});

	// The book view has no overview: its place is the scroll, never thrown away.
	it('leaves the book view where it is with nothing open', () => {
		expect(escape({ selection: false, notice: false, book: true })).toBe('none');
	});
});
