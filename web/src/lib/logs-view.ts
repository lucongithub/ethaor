/**
 * The operator's log page (`/logs/`): what decides, apart from the page.
 *
 * - **The token** comes once in the address (`/logs/?token=…`), is taken
 *   out of it at once (`history.replaceState`), so the history and a
 *   bookmark keep `/logs/` alone, and is kept for the tab only
 *   (`sessionStorage`): a reload follows on, a closed tab forgets it.
 * - **The logs** are `GET /api/logs`, server-sent events, read with
 *   `fetch` — an `EventSource` cannot send `Authorization` — so the token
 *   never travels in a URL to the service.
 * - **Bounded**: the page keeps the last `MAX_LINES`; a stream that ends
 *   or fails is opened again after `RETRY_MS`, history replayed afresh.
 *   A refusal (wrong token, logs off) is not retried: it says why.
 */

/** Lines kept on the page; older ones go. */
export const MAX_LINES = 5000;

/** Lines replayed when the stream opens. */
export const HISTORY = 500;

/** Wait before opening again a stream that ended or failed. */
export const RETRY_MS = 2000;

/** Where the token is kept for the tab. */
export const KEY = 'ethaor.logs.token';

/**
 * The token in `url`, and the address to show instead with it taken out
 * (`null`: there was none, nothing to replace).
 */
export function takeToken(url: URL): { token: string | null; clean: string | null } {
	const token = url.searchParams.get('token');
	if (token === null) return { token: null, clean: null };
	const rest = new URL(url);
	rest.searchParams.delete('token');
	return { token: token.trim() || null, clean: rest.pathname + rest.search + rest.hash };
}

/** One server-sent event. */
export type SseEvent = { event: string; data: string };

/**
 * The whole events in `buffer`, and what is left of an event not yet
 * whole. Comments (`:`, the keep-alives) are skipped; `data` lines are
 * joined by newlines, as the format says.
 */
export function parseEvents(buffer: string): { events: SseEvent[]; rest: string } {
	// A `\r` at the very end may be half of a `\r\n` the next chunk ends:
	// held back, or it would count as a line of its own.
	const held = buffer.endsWith('\r') ? '\r' : '';
	const text = buffer.slice(0, buffer.length - held.length).replace(/\r\n?/g, '\n');
	const blocks = text.split('\n\n');
	const rest = (blocks.pop() ?? '') + held;
	const events: SseEvent[] = [];
	for (const block of blocks) {
		let event = 'message';
		const data: string[] = [];
		for (const line of block.split('\n')) {
			if (line === '' || line.startsWith(':')) continue;
			const colon = line.indexOf(':');
			const field = colon < 0 ? line : line.slice(0, colon);
			let value = colon < 0 ? '' : line.slice(colon + 1);
			if (value.startsWith(' ')) value = value.slice(1);
			if (field === 'event') event = value;
			else if (field === 'data') data.push(value);
		}
		if (data.length > 0) events.push({ event, data: data.join('\n') });
	}
	return { events, rest };
}

/** A log line as the page shows it. */
export type Shown = { time: string; level: string; message: string; fields: string };

/**
 * A line on the page: `id` is its own, never reused, so dropping the
 * oldest lines leaves every other one under its key — the page removes
 * those and adds the new, instead of rewriting every row.
 */
export type Row = Shown & { id: number };

/** `lines` read for the eye, numbered from `first`. */
export function rows(lines: string[], first: number): Row[] {
	return lines.map((line, i) => ({ id: first + i, ...show(line) }));
}

/** Fields shown in their own column, not among the rest. */
const OWN = new Set(['ts_ms', 'level', 'message']);

/** A log line, read for the eye: time, level, message, then the rest. */
export function show(line: string): Shown {
	let json: Record<string, unknown>;
	try {
		json = JSON.parse(line);
	} catch {
		return { time: '', level: '', message: line, fields: '' };
	}
	if (json === null || typeof json !== 'object')
		return { time: '', level: '', message: line, fields: '' };
	const ms = typeof json.ts_ms === 'number' ? json.ts_ms : NaN;
	const time = Number.isFinite(ms) ? new Date(ms).toISOString().replace('T', ' ').slice(0, 23) : '';
	const fields = Object.entries(json)
		.filter(([k]) => !OWN.has(k))
		.map(([k, v]) => `${k}=${typeof v === 'string' ? v : JSON.stringify(v)}`)
		.join(' ');
	return { time, level: String(json.level ?? ''), message: String(json.message ?? ''), fields };
}

/** What the page says about the stream. */
export type State =
	| { kind: 'connecting' }
	| { kind: 'live' }
	| { kind: 'refused'; status: number; code: string; message: string; incident?: string }
	| { kind: 'retrying'; why: string };

export interface Follow {
	fetch: typeof fetch;
	token: string;
	level: string;
	onLines: (lines: string[], fresh: boolean) => void;
	onLagged: (files: number) => void;
	onState: (state: State) => void;
	signal: AbortSignal;
	sleep?: (ms: number) => Promise<void>;
}

const wait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Follows the logs until `signal` aborts or the service refuses: opened,
 * read, opened again after `RETRY_MS` when it ends or fails. Never throws.
 */
export async function follow(f: Follow): Promise<void> {
	const sleep = f.sleep ?? wait;
	while (!f.signal.aborted) {
		f.onState({ kind: 'connecting' });
		let why: string;
		try {
			const response = await f.fetch(
				`/api/logs?history=${HISTORY}&level=${encodeURIComponent(f.level)}`,
				{ headers: { authorization: `Bearer ${f.token}` }, signal: f.signal }
			);
			if (!response.ok) {
				const refused = await refusal(response);
				// 503 with a code is the service saying no file is kept: not
				// worth asking again. Without one, a proxy in between: retried.
				if (response.status !== 503 || refused.code !== 'unknown') {
					f.onState(refused);
					return;
				}
				why = `the service answered ${response.status}`;
			} else if (!response.body) {
				why = 'the answer has no body';
			} else {
				f.onState({ kind: 'live' });
				// The history is replayed whole at every opening: what the page
				// showed goes first — even when the history is empty, a level
				// with no line yet.
				f.onLines([], true);
				await read(response.body, f);
				why = 'the stream ended';
			}
		} catch (e) {
			if (f.signal.aborted) return;
			why = e instanceof Error ? e.message : String(e);
		}
		if (f.signal.aborted) return;
		f.onState({ kind: 'retrying', why });
		await sleep(RETRY_MS);
	}
}

async function refusal(response: Response): Promise<State & { kind: 'refused' }> {
	const status = response.status;
	try {
		const body = await response.json();
		const error = body?.error ?? {};
		return {
			kind: 'refused',
			status,
			code: String(error.code ?? 'unknown'),
			message: String(error.message ?? response.statusText),
			incident: typeof error.incident === 'string' ? error.incident : undefined
		};
	} catch {
		return {
			kind: 'refused',
			status,
			code: 'unknown',
			message: response.statusText || `HTTP ${status}`
		};
	}
}

async function read(body: ReadableStream<Uint8Array>, f: Follow): Promise<void> {
	const reader = body.getReader();
	const decoder = new TextDecoder();
	let buffer = '';
	try {
		for (;;) {
			const { value, done } = await reader.read();
			if (done) return;
			buffer += decoder.decode(value, { stream: true });
			const { events, rest } = parseEvents(buffer);
			buffer = rest;
			const lines: string[] = [];
			for (const e of events) {
				if (e.event === 'lagged') {
					if (lines.length) f.onLines(lines.splice(0), false);
					f.onLagged(Number(e.data) || 0);
				} else lines.push(e.data);
			}
			if (lines.length) f.onLines(lines, false);
		}
	} finally {
		reader.releaseLock();
	}
}

/** `kept` with `added` after it, the oldest dropped past `MAX_LINES`. */
export function keep<T>(kept: T[], added: T[], fresh: boolean): T[] {
	const all = fresh ? added : kept.concat(added);
	return all.length > MAX_LINES ? all.slice(all.length - MAX_LINES) : all;
}
