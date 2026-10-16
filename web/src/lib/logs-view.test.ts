import { describe, expect, it } from 'vitest';
import {
	follow,
	HISTORY,
	keep,
	MAX_LINES,
	parseEvents,
	RETRY_MS,
	rows,
	show,
	takeToken,
	type State
} from './logs-view';

describe('takeToken', () => {
	// The token leaves the address, the rest of it stays: the history and a
	// bookmark keep `/logs/` alone.
	it('takes the token out of the address and keeps the rest', () => {
		expect(takeToken(new URL('https://x.test/logs/?token=abc&level=warn#end'))).toEqual({
			token: 'abc',
			clean: '/logs/?level=warn#end'
		});
		expect(takeToken(new URL('https://x.test/logs/?token=%20abc%20'))).toEqual({
			token: 'abc',
			clean: '/logs/'
		});
	});

	// No token, nothing to replace; an empty one is taken out but is no token.
	it('replaces nothing without a token, and an empty one is none', () => {
		expect(takeToken(new URL('https://x.test/logs/'))).toEqual({ token: null, clean: null });
		expect(takeToken(new URL('https://x.test/logs/?token='))).toEqual({
			token: null,
			clean: '/logs/'
		});
	});
});

describe('parseEvents', () => {
	// Events cut anywhere by the network come out whole, once, in order.
	it('gives whole events across any cut, and keeps the rest', () => {
		const stream =
			'data: {"n":1}\n\n: keep-alive\n\nevent: lagged\ndata: 3\n\ndata: {"n":2}\r\n\r\n';
		for (let cut = 0; cut <= stream.length; cut++) {
			const first = parseEvents(stream.slice(0, cut));
			const second = parseEvents(first.rest + stream.slice(cut));
			expect([...first.events, ...second.events], `cut at ${cut}`).toEqual([
				{ event: 'message', data: '{"n":1}' },
				{ event: 'lagged', data: '3' },
				{ event: 'message', data: '{"n":2}' }
			]);
			expect(second.rest).toBe('');
		}
	});

	// Several data lines are one event, joined by a newline, as the format says.
	it('joins the data lines of one event', () => {
		expect(parseEvents('data: a\ndata:b\n\n').events).toEqual([{ event: 'message', data: 'a\nb' }]);
	});
});

describe('show', () => {
	// The time, level and message apart, every other field after them.
	it('reads a log line for the eye', () => {
		const line = JSON.stringify({
			ts_ms: Date.UTC(2026, 9, 16, 12, 30, 5, 120),
			level: 'WARN',
			target: 'ethaor_server',
			message: 'answered',
			status: 429,
			path: '/api/els'
		});
		expect(show(line)).toEqual({
			time: '2026-10-16 12:30:05.120',
			level: 'WARN',
			message: 'answered',
			fields: 'target=ethaor_server status=429 path=/api/els'
		});
	});

	// A line that is not JSON is shown as it is, never thrown on.
	it('shows a line that is not JSON as it is', () => {
		expect(show('not json')).toEqual({ time: '', level: '', message: 'not json', fields: '' });
		expect(show('null').message).toBe('null');
	});
});

describe('rows', () => {
	// Each line has its own number, and keeping the newest leaves every
	// survivor under the number it had: the page moves nothing it keeps.
	it('numbers the lines once, and the survivors keep their numbers', () => {
		const first = rows(['{"message":"a"}', '{"message":"b"}'], 0);
		expect(first.map((r) => [r.id, r.message])).toEqual([
			[0, 'a'],
			[1, 'b']
		]);
		const many = rows(
			Array.from({ length: MAX_LINES }, (_, i) => `{"n":${i}}`),
			2
		);
		const kept = keep(first, many, false);
		expect(kept).toHaveLength(MAX_LINES);
		expect(kept[0].id).toBe(2);
		expect(kept.every((r, i) => r.id === i + 2)).toBe(true);
		expect(new Set(kept.map((r) => r.id)).size).toBe(kept.length);
	});
});

describe('keep', () => {
	// The page holds `MAX_LINES` at most, the newest; a fresh opening replaces.
	it('keeps the newest lines up to the bound, and a fresh opening replaces', () => {
		const many = Array.from({ length: MAX_LINES + 10 }, (_, i) => i);
		expect(keep([], many, false)).toEqual(many.slice(10));
		expect(keep([1, 2], [3], false)).toEqual([1, 2, 3]);
		expect(keep([1, 2], [3], true)).toEqual([3]);
	});
});

/** A stream of server-sent events, written in the chunks given. */
function sse(chunks: string[]): Response {
	const body = new ReadableStream<Uint8Array>({
		start(c) {
			for (const chunk of chunks) c.enqueue(new TextEncoder().encode(chunk));
			c.close();
		}
	});
	return new Response(body, { status: 200, headers: { 'content-type': 'text/event-stream' } });
}

function run(answers: (() => Response | Promise<Response>)[], level = 'info') {
	const controller = new AbortController();
	const calls: { url: string; auth: string | null }[] = [];
	const states: State[] = [];
	const shown: { lines: string[]; fresh: boolean }[] = [];
	const lagged: number[] = [];
	const slept: number[] = [];
	const done = follow({
		fetch: (async (url: string, init?: RequestInit) => {
			calls.push({ url, auth: new Headers(init?.headers).get('authorization') });
			const next = answers.shift();
			if (!next) {
				controller.abort();
				throw new DOMException('aborted', 'AbortError');
			}
			return next();
		}) as typeof fetch,
		token: 'the-token-1234567890',
		level,
		signal: controller.signal,
		onLines: (lines, fresh) => shown.push({ lines, fresh }),
		onLagged: (files) => lagged.push(files),
		onState: (s) => states.push(s),
		sleep: async (ms) => void slept.push(ms)
	});
	return { done, calls, states, shown, lagged, slept };
}

describe('follow', () => {
	// The token goes in the header, never the URL; the history and level are
	// asked; the lines arrive, the first batch replacing what was shown.
	it('sends the token in the header and hands the lines on', async () => {
		const r = run(
			[
				() =>
					sse(['data: {"n":1}\n\ndata: {"n', '":2}\n\nevent: lagged\ndata: 2\n\ndata: {"n":3}\n\n'])
			],
			'warn'
		);
		await r.done;
		expect(r.calls[0]).toEqual({
			url: `/api/logs?history=${HISTORY}&level=warn`,
			auth: 'Bearer the-token-1234567890'
		});
		expect(r.calls[0].url).not.toContain('the-token');
		expect(r.shown.flatMap((s) => s.lines)).toEqual(['{"n":1}', '{"n":2}', '{"n":3}']);
		// What was shown is cleared once, at the opening, before any line.
		expect(r.shown[0]).toEqual({ lines: [], fresh: true });
		expect(r.shown.slice(1).every((s) => !s.fresh)).toBe(true);
		expect(r.lagged).toEqual([2]);
	});

	// A stream that ends or a network failure is opened again after the
	// pause, each opening replaying the history afresh.
	it('opens again after an end or a failure, the history afresh', async () => {
		const r = run([
			() => sse(['data: a\n\n']),
			() => Promise.reject(new TypeError('network down')),
			() => sse(['data: b\n\n'])
		]);
		await r.done;
		expect(r.calls).toHaveLength(4);
		expect(r.slept).toEqual([RETRY_MS, RETRY_MS, RETRY_MS]);
		expect(
			r.states.filter((s) => s.kind === 'retrying').map((s) => s.kind === 'retrying' && s.why)
		).toEqual(['the stream ended', 'network down', 'the stream ended']);
		expect(r.shown).toEqual([
			{ lines: [], fresh: true },
			{ lines: ['a'], fresh: false },
			{ lines: [], fresh: true },
			{ lines: ['b'], fresh: false }
		]);
	});

	// A level with no line yet clears the page all the same: the lines of
	// the level before must not stay as if they were of this one.
	it('clears what was shown even when the history is empty', async () => {
		const r = run([() => sse([': keep-alive\n\n'])], 'error');
		await r.done;
		expect(r.shown[0]).toEqual({ lines: [], fresh: true });
		expect(keep(['an info of the level before'], r.shown[0].lines, r.shown[0].fresh)).toEqual([]);
	});

	// A refusal says the service's code, message and incident, and is not
	// asked again: a wrong token stays wrong.
	it('stops on a refusal and says why', async () => {
		for (const [status, code] of [
			[401, 'unauthorized'],
			[403, 'logs_disabled'],
			[503, 'logs_unavailable']
		] as const) {
			const r = run([
				() =>
					Response.json({ error: { code, message: 'no', incident: 'ETH-12345678' } }, { status })
			]);
			await r.done;
			expect(r.calls, code).toHaveLength(1);
			expect(r.states.at(-1)).toEqual({
				kind: 'refused',
				status,
				code,
				message: 'no',
				incident: 'ETH-12345678'
			});
		}
	});

	// Served without the service (`npm run dev`), the answer is not JSON:
	// refused, its status said. A 503 without a code is a proxy: retried.
	it('says a page served without the service, and retries a bare 503', async () => {
		const missing = run([
			() => new Response('<!doctype html>', { status: 404, statusText: 'Not Found' })
		]);
		await missing.done;
		expect(missing.states.at(-1)).toEqual({
			kind: 'refused',
			status: 404,
			code: 'unknown',
			message: 'Not Found'
		});
		const proxy = run([() => new Response('busy', { status: 503 })]);
		await proxy.done;
		expect(proxy.calls).toHaveLength(2);
	});
});
