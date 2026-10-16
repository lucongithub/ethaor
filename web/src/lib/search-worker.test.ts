import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CATALOGUE } from './fault';
import type { Answer, Request } from './search-worker';
import { block, bookCuts, booksShuffled, serveArtifacts } from './tests/artifacts';

/** A worker scope of our own, and the worker loaded into it afresh. */
async function start() {
	const answers: Answer[] = [];
	const scope = {
		postMessage: (answer: Answer) => answers.push(answer),
		onmessage: null as null | ((event: { data: Request }) => void)
	};
	vi.stubGlobal('fetch', serveArtifacts());
	vi.stubGlobal('self', scope);
	await import('./search-worker');
	const send = (data: Request) => scope.onmessage!({ data });
	const feed = () => {
		for (const b of booksShuffled()) send({ tile: b });
		send({ readings: block(bookCuts.length - 2) });
	};
	return { answers, send, feed };
}

const total = (answer: Answer) => ('found' in answer ? answer.found.total : NaN);

describe('the search worker', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.doUnmock('./search-core');
	});

	it('holds a search until the text is in, then answers it by its id', async () => {
		const { answers, send } = await start();
		send({ els: { id: 7, query: 'תורה', skip0: 0, skip1: 0 } });
		for (const b of booksShuffled()) send({ tile: b });
		await new Promise((done) => setTimeout(done, 50));
		expect(answers).toEqual([]);

		send({ readings: block(bookCuts.length - 2) });
		await vi.waitFor(() => expect(answers.length).toBe(1));
		expect(answers[0].id).toBe(7);
		expect(total(answers[0])).toBeGreaterThan(0);

		send({ els: { id: 8, query: 'תורה', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(2));
		expect(answers[1].id).toBe(8);
		expect(total(answers[1])).toBe(total(answers[0]));
	});

	it('answers a failed search by its id, and searches on with a new module', async () => {
		// A module that traps on one query, as a panicking search would.
		const created = vi.fn();
		vi.doMock('./search-core', async () => {
			const actual = await vi.importActual<typeof import('./search-core')>('./search-core');
			return {
				SearchCore: {
					async create() {
						created();
						const core = await actual.SearchCore.create();
						const els = core.els.bind(core);
						core.els = (query, skip0, skip1) => {
							if (query === 'בום') throw new Error('unreachable executed');
							// A real trap: the module panics, its hook leaves the Rust place.
							if (query === 'פאניקה') core.probePanic();
							return els(query, skip0, skip1);
						};
						return core;
					}
				}
			};
		});
		const { answers, send, feed } = await start();
		feed();
		send({ els: { id: 1, query: 'תורה', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(1));

		send({ els: { id: 2, query: 'בום', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(2));
		const failed = answers[1];
		if (!('fault' in failed)) throw new Error('not a fault');
		expect(failed.id).toBe(2);
		expect(failed.error).toBe(CATALOGUE['search-failed'].message);
		expect(failed.fault.incident).toMatch(/^ETH-[0-9A-Z]{8}$/);
		expect([failed.fault.code, failed.fault.source]).toEqual(['search-failed', 'worker']);
		expect(failed.fault.chain[0].what).toBe('unreachable executed');
		expect(failed.fault.chain.at(-1)?.what).toBe('a search in the worker');

		send({ els: { id: 3, query: 'תורה', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(3));
		expect(answers[2].id).toBe(3);
		expect(total(answers[2])).toBe(total(answers[0]));
		expect(created).toHaveBeenCalledTimes(2);

		send({ els: { id: 4, query: 'פאניקה', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(4));
		const trapped = answers[3];
		if (!('fault' in trapped)) throw new Error('not a fault');
		expect(trapped.fault.source).toBe('wasm');
		expect(trapped.fault.chain[0].at).toMatch(/^src\/wasm\/fault\.rs:\d+:\d+$/);
		expect(trapped.fault.chain[0].what).toBe('probe: a panic on purpose');
		send({ els: { id: 5, query: 'תורה', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(5));
		expect(total(answers[4])).toBe(total(answers[0]));
	});
});
