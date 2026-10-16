import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	CATALOGUE,
	escalate,
	Fault,
	incident,
	raise,
	readTraps,
	relay,
	Reporter,
	STEPS,
	TEXT,
	watch,
	type FaultData
} from './fault';
import { read } from './tests/artifacts';

const NUMBER = /^ETH-[0-9ABCDEFGHJKMNPQRSTVWXYZ]{8}$/;

afterEach(() => vi.unstubAllGlobals());

describe('a fault', () => {
	it('is numbered so it reads aloud and never repeats', () => {
		const seen = new Set<string>();
		for (let k = 0; k < 20_000; k++) {
			const n = incident();
			expect(n).toMatch(NUMBER);
			seen.add(n);
		}
		expect(seen.size).toBe(20_000);
	});

	it('is raised at its caller, with the words of its code and its number', () => {
		const fault = raise('tile-unfetched', 'fetching a tile');
		expect(fault.incident).toMatch(NUMBER);
		expect(fault.chain[0].at).toMatch(/fault\.test\.ts:\d+:\d+$/);
		expect(fault.chain[0].what).toBe('fetching a tile');
		expect(fault.message).toBe(CATALOGUE['tile-unfetched'].message);
		expect(fault.text).toBe(`${fault.message} (incident ${fault.incident})`);
		expect([fault.show, fault.fatal, fault.level]).toEqual([false, false, 'warn']);
	});

	it('keeps its number through every layer, each adding its step', () => {
		const fault = raise('search-failed', 'the origin', 'worker');
		const number = fault.incident;
		const passed = relay(escalate(fault, 'uncaught', 'a layer above'), 'the boundary');
		expect(passed).toBe(fault);
		expect(passed.incident).toBe(number);
		expect(passed.code).toBe('search-failed');
		expect(passed.chain.map((s) => s.what)).toEqual([
			'the origin',
			'a layer above',
			'the boundary'
		]);
		expect(passed.chain.every((s) => /fault\.test\.ts:\d+:\d+$/.test(s.at))).toBe(true);
	});

	it('is made of anything thrown, its own message and place as origin', () => {
		const thrown = new TypeError('x is not a function');
		const fault = escalate(thrown, 'map-broken', 'drawing a frame');
		expect(fault.chain[0].what).toBe('x is not a function');
		expect(fault.chain[0].at).toMatch(/fault\.test\.ts:\d+:\d+$/);
		expect(fault.chain[1].what).toBe('drawing a frame');
		expect(fault.origin).toContain('TypeError');
		expect(escalate('a string', 'uncaught', 'w').chain[0].what).toBe('a string');
		expect(escalate(undefined, 'uncaught', 'w').chain[0].what).toBe('undefined');
	});

	it('crosses from a worker unchanged', () => {
		const fault = escalate(
			new Error('inside'),
			'search-failed',
			'a search in the worker',
			'worker'
		);
		const back = Fault.from(structuredClone(fault.data('page')));
		expect([back.incident, back.code, back.source]).toEqual([fault.incident, fault.code, 'worker']);
		expect(back.chain).toEqual(fault.chain);
	});

	it('is sent as the service reads it, bounded as it bounds it', () => {
		const fault = raise('search-failed', 'w'.repeat(5000));
		for (let k = 0; k < 100; k++) relay(fault, `step ${k}`);
		const data: FaultData = fault.data('https://ethaor.example/');
		expect(Object.keys(data).sort()).toEqual(
			['chain', 'code', 'incident', 'level', 'message', 'source', 'stack', 'url'].sort()
		);
		expect(data.chain.length).toBe(STEPS);
		expect(data.chain[0].what.length).toBe(TEXT);
		expect(data.stack.length).toBeLessThanOrEqual(TEXT);
	});
});

describe('a WebAssembly trap', () => {
	it('is traced to the Rust place its panic left', async () => {
		const instance = new WebAssembly.Instance(
			new WebAssembly.Module(read('ethaor-search.wasm')),
			{}
		);
		const wasm = instance.exports as unknown as {
			memory: WebAssembly.Memory;
			fault_ptr(): number;
			fault_len(): number;
			probe_panic(): void;
		};
		const unwatch = readTraps(() =>
			new TextDecoder().decode(
				new Uint8Array(wasm.memory.buffer, wasm.fault_ptr(), wasm.fault_len())
			)
		);
		let trap: unknown;
		try {
			wasm.probe_panic();
		} catch (cause) {
			trap = cause;
		}
		expect(trap).toBeInstanceOf(WebAssembly.RuntimeError);
		const fault = escalate(trap, 'search-failed', 'a search');
		unwatch();
		expect(fault.chain[0].at).toMatch(/^src\/wasm\/fault\.rs:\d+:\d+$/);
		expect(fault.chain[0].what).toBe('probe: a panic on purpose');
		expect(fault.source).toBe('wasm');
		expect(fault.chain[1].what).toBe('a search');
	});
});

describe('the reporter', () => {
	it('sends each incident once, a bounded number a minute, and never throws', () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const sent: string[] = [];
		let now = 0;
		const reporter = new Reporter(
			(body) => sent.push(body),
			3,
			() => now
		);
		const fault = raise('tile-unfetched', 'x');
		reporter.report(fault);
		reporter.report(fault);
		expect(sent.length).toBe(1);
		expect(JSON.parse(sent[0]).incident).toBe(fault.incident);
		for (let k = 0; k < 10; k++) reporter.report(raise('tile-unfetched', `${k}`));
		expect(sent.length).toBe(3);
		now += 61_000;
		reporter.report(raise('tile-unfetched', 'later'));
		expect(sent.length).toBe(4);

		const failing = new Reporter(() => {
			throw new Error('network down');
		});
		expect(() => failing.report(raise('tile-unfetched', 'x'))).not.toThrow();
		const rejecting = new Reporter(() => Promise.reject(new Error('404')));
		expect(() => rejecting.report(raise('tile-unfetched', 'x'))).not.toThrow();
	});
});

describe('what nothing caught', () => {
	it('becomes an uncaught fault, reported and shown', () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const target = new EventTarget() as unknown as Window;
		const reported: Fault[] = [];
		const shown: Fault[] = [];
		const undo = watch(target, (f) => shown.push(f), {
			report: (f: Fault) => void reported.push(f)
		});
		const error = Object.assign(new Event('error'), {
			error: new RangeError('out of range'),
			message: 'out of range'
		});
		target.dispatchEvent(error);
		const rejection = Object.assign(new Event('unhandledrejection'), {
			reason: new Error('nobody awaited')
		});
		target.dispatchEvent(rejection);
		expect(reported.map((f) => [f.code, f.chain[0].what])).toEqual([
			['uncaught', 'out of range'],
			['unawaited', 'nobody awaited']
		]);
		expect(shown.length).toBe(2);
		undo();
		target.dispatchEvent(error);
		expect(reported.length).toBe(2);
	});
});

/** Every source file of the page, tests aside. */
function sources(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return name === 'tests' ? [] : sources(path);
		return /\.(ts|svelte)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
	});
}

describe('the catalogue', () => {
	it('speaks English to the user, plainly', () => {
		for (const [code, entry] of Object.entries(CATALOGUE)) {
			// English needs no letter past ASCII, and none of the little words
			// French cannot do without — accented or not, French shows.
			expect(entry.message, code).toMatch(/^[ -~]+$/);
			expect(entry.message, code).not.toMatch(
				/\b(le|la|les|un|une|des|du|est|et|pas|pour|avec|dans|sur|ne|n|l|d|qu|que|qui)\b/i
			);
			expect(entry.message, code).toMatch(/^[A-Z].*\.$/);
		}
	});

	it('has each code raised at one place, which it therefore names', () => {
		const root = new URL('.', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
		const text = sources(root)
			.map((p) => readFileSync(p, 'utf8'))
			.join('\n');
		for (const code of Object.keys(CATALOGUE)) {
			// Declared once in the catalogue — quoted, or bare for a plain word.
			const quoted = text.split(`'${code}':`).length - 1;
			const bare = text.split(`	${code}: {`).length - 1;
			expect(quoted + bare, `${code} declare`).toBe(1);
			const raised = text.split(`'${code}'`).length - 1 - quoted;
			expect(raised, code).toBe(1);
		}
	});
});
