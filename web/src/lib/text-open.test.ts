import { afterEach, describe, expect, it, vi } from 'vitest';
import { gzipSync } from 'node:zlib';
import { fragmentKey, inflate, lanes, range, retried, RETRY_WAITS } from './text-open';

afterEach(() => vi.unstubAllGlobals());

const bytes = Uint8Array.from({ length: 5000 }, (_, i) => (i * 31) % 251);

describe('inflate', () => {
	it('opens what gzip closed, byte for byte', async () => {
		expect(await inflate(new Uint8Array(gzipSync(bytes)))).toEqual(bytes);
	});

	it('reads a view into a larger buffer, not the whole buffer', async () => {
		const packed = gzipSync(bytes);
		const around = new Uint8Array(packed.length + 20);
		around.set(packed, 10);
		expect(await inflate(around.subarray(10, 10 + packed.length))).toEqual(bytes);
	});
});

describe('range', () => {
	it('asks for the bytes from..to, end excluded, at the given priority', async () => {
		const fetch = vi.fn(async () => new Response(bytes.slice(100, 200), { status: 206 }));
		vi.stubGlobal('fetch', fetch);
		expect(await range('/t', 100, 200, 'high')).toEqual(bytes.slice(100, 200));
		expect(fetch).toHaveBeenCalledWith('/t', {
			headers: { Range: 'bytes=100-199' },
			priority: 'high'
		});
	});

	it('reads a whole-file answer only up to what it needs', async () => {
		let pulled = 0;
		const body = new ReadableStream<Uint8Array>({
			pull(controller) {
				const at = pulled++ * 1000;
				if (at >= bytes.length) controller.close();
				else controller.enqueue(bytes.slice(at, at + 1000));
			}
		});
		vi.stubGlobal('fetch', async () => new Response(body, { status: 200 }));
		expect(await range('/t', 1500, 2500)).toEqual(bytes.slice(1500, 2500));
		expect(pulled).toBeLessThan(5);
	});

	it('refuses a failed answer', async () => {
		vi.stubGlobal('fetch', async () => new Response(null, { status: 404 }));
		await expect(range('/t', 0, 10)).rejects.toThrow('404');
	});
});

describe('retried', () => {
	// A download that fails is tried again after each wait, and the first
	// success is the answer: a dropped request or a server restarting no
	// longer gives up the whole text.
	it('tries again after each wait until one succeeds', async () => {
		const waited: number[] = [];
		let calls = 0;
		const got = await retried(
			async () => {
				if (++calls < 3) throw new TypeError('Failed to fetch');
				return 'book';
			},
			[10, 20, 30],
			async (ms) => void waited.push(ms)
		);
		expect(got).toBe('book');
		expect(calls).toBe(3);
		expect(waited).toEqual([10, 20]);
	});

	// Bounded: one try more than there are waits, then the last failure
	// stands — the fault is told once, not retried forever.
	it('gives up after its waits with the last failure', async () => {
		const waited: number[] = [];
		let calls = 0;
		const failing = retried(
			async () => {
				throw new Error(`try ${++calls}`);
			},
			RETRY_WAITS,
			async (ms) => void waited.push(ms)
		);
		await expect(failing).rejects.toThrow(`try ${RETRY_WAITS.length + 1}`);
		expect(waited).toEqual([...RETRY_WAITS]);
	});

	// A page left mid-wait never tries again — its timers are gone — so a
	// reload that aborted its downloads reports nothing.
	it('waits on its sleep before trying again, never at once', async () => {
		let calls = 0;
		let wake: () => void = () => {};
		const pending = retried(
			async () => {
				if (++calls === 1) throw new TypeError('Failed to fetch');
				return 1;
			},
			[1000],
			() => new Promise<void>((done) => (wake = done))
		);
		await new Promise((done) => setTimeout(done, 10));
		expect(calls).toBe(1);
		wake();
		expect(await pending).toBe(1);
		expect(calls).toBe(2);
	});
});

describe('lanes', () => {
	it('takes each item once, never more at once than the lanes', async () => {
		const queue = Array.from({ length: 40 }, (_, i) => i);
		const taken: number[] = [];
		let busy = 0;
		let most = 0;
		await lanes(
			8,
			() => queue.shift() ?? -1,
			async (item) => {
				most = Math.max(most, ++busy);
				await new Promise((done) => setTimeout(done, (item * 7) % 5));
				taken.push(item);
				busy--;
			}
		);
		expect(taken.sort((a, b) => a - b)).toEqual(Array.from({ length: 40 }, (_, i) => i));
		expect(most).toBe(8);
	});

	it('chooses each item when a lane frees, so a later choice sees the view move', async () => {
		let nearest = 0;
		const left = new Set([0, 1, 2, 3, 4, 5]);
		const asked: number[] = [];
		await lanes(
			2,
			() => {
				if (!left.size) return -1;
				const pick = [...left].sort((a, b) => Math.abs(a - nearest) - Math.abs(b - nearest))[0];
				left.delete(pick);
				asked.push(pick);
				return pick;
			},
			async (item) => {
				await Promise.resolve();
				// The visitor jumps to the far end as the first item lands.
				if (item === 0) nearest = 5;
			}
		);
		expect(asked.slice(0, 3)).toEqual([0, 1, 5]);
	});

	it('ends at once when there is nothing to take', async () => {
		const take = vi.fn(async () => {});
		await lanes(3, () => -1, take);
		expect(take).not.toHaveBeenCalled();
	});
});

describe('fragmentKey', () => {
	// Two fragments never share a key: the cache once keyed on
	// `offset * 1024 + length`, and a verse of 1100 bytes collided with the
	// 76 bytes one further on, so the page could show another's text.
	it('keys every fragment apart, long verses included', () => {
		expect(fragmentKey(500, 1100)).not.toBe(fragmentKey(501, 76));
		const seen = new Set<number>();
		for (const offset of [0, 1, 1023, 1024, 4_700_000]) {
			for (const length of [0, 1, 76, 1023, 1024, 1100, 4000, 65535]) {
				const key = fragmentKey(offset, length);
				expect(Number.isSafeInteger(key)).toBe(true);
				seen.add(key);
			}
		}
		expect(seen.size).toBe(5 * 8);
	});
});
