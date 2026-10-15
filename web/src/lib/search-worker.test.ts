import { describe, expect, it, vi } from 'vitest';
import type { Answer, Request } from './search-worker';
import { block, bookCuts, booksShuffled, serveArtifacts } from './tests/artifacts';

describe('the search worker', () => {
	it('holds a search until the text is in, then answers it by its id', async () => {
		const answers: Answer[] = [];
		const scope = {
			postMessage: (answer: Answer) => answers.push(answer),
			onmessage: null as null | ((event: { data: Request }) => void)
		};
		vi.stubGlobal('fetch', serveArtifacts());
		vi.stubGlobal('self', scope);
		await import('./search-worker');
		const send = (data: Request) => scope.onmessage!({ data });
		const total = (answer: Answer) => ('found' in answer ? answer.found.total : NaN);

		send({ els: { id: 7, query: 'תורה', skip0: 0, skip1: 0 } });
		for (const b of booksShuffled()) send({ tile: b });
		await new Promise((done) => setTimeout(done, 50));
		expect(answers).toEqual([]);

		send({ readings: block(bookCuts.length - 2) });
		await vi.waitFor(() => expect(answers.length).toBe(1));
		expect('id' in answers[0] && answers[0].id).toBe(7);
		expect(total(answers[0])).toBeGreaterThan(0);

		send({ els: { id: 8, query: 'תורה', skip0: 0, skip1: 0 } });
		await vi.waitFor(() => expect(answers.length).toBe(2));
		expect('id' in answers[1] && answers[1].id).toBe(8);
		expect(total(answers[1])).toBe(total(answers[0]));
	});
});
