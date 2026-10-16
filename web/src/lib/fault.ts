/**
 * Faults in the page: numbered, escalated, reported, told in plain words.
 *
 * The same model as the service's (`server/src/fault.rs`), so an incident a
 * user quotes is found in one log whichever side raised it:
 *
 * - **An incident number** (`ETH-7K3F92QD`) given where the error is raised,
 *   kept through every escalation, shown to the user, sent to the log.
 * - **A code** from `CATALOGUE`, raised at one place only (a test holds it),
 *   so it names that place; **`at`**, the `file:line` it was raised at — or,
 *   for a WebAssembly trap, the Rust place the panic hook left (`fromTrap`).
 * - **The escalation**: each layer adds what it was doing (`escalate`).
 * - **Reported** to `POST /api/report`, bounded, never throwing — hosted
 *   without the service, a report is lost and the page goes on.
 * - **The user reads** the code's message and the number, nothing more.
 *   Verbose (`?verbose` or `localStorage['ethaor.verbose'] = '1'`), the
 *   console shows the whole escalation too.
 */

export type Step = { at: string; what: string };
export type Source = 'page' | 'worker' | 'wasm';
export type Level = 'error' | 'warn';

/**
 * Every fault the page raises, each at one place (a test holds it), so the
 * code names that place. `show`: told to the user — otherwise the page
 * degrades in silence and only the log knows. `fatal`: the map cannot go
 * on, and the user is offered the way back. `warn`: a degradation lived
 * through; `error`: something the user lost. `message`: what the user
 * reads, in English like the rest of the interface — plain, saying what
 * still works.
 */
export const CATALOGUE = {
	'map-unloaded': {
		show: true,
		fatal: true,
		level: 'error',
		message: 'The map could not be loaded.'
	},
	'map-broken': {
		show: true,
		fatal: true,
		level: 'error',
		message: 'The map ran into an error and stopped. Reload the page to start it again.'
	},
	'search-failed': {
		show: true,
		level: 'error',
		message: 'The search failed. The next one will run normally.'
	},
	'search-failed-here': {
		show: true,
		level: 'error',
		message: 'The search failed. The next one will run normally.'
	},
	'tile-unfetched': {
		show: false,
		level: 'warn',
		message: 'A passage could not be loaded ahead; it will come with the whole text.'
	},
	'text-unfetched': {
		show: false,
		level: 'warn',
		message: 'The whole text could not be loaded; the map stays at the level of the chapters.'
	},
	'worker-failed': {
		show: false,
		level: 'warn',
		message: 'The search could not prepare the text; it will try again.'
	},
	uncaught: { show: true, level: 'error', message: 'Something unexpected went wrong.' },
	unawaited: { show: true, level: 'error', message: 'Something unexpected went wrong.' }
} as const satisfies Record<
	string,
	{ show: boolean; fatal?: boolean; level: Level; message: string }
>;

export type Code = keyof typeof CATALOGUE;

/** Steps, and characters of any text, a report carries: the service's bounds. */
export const STEPS = 32;
export const TEXT = 2000;

const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

/** `ETH-` and eight characters of Crockford's base 32: read aloud without doubt. */
export function incident(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(8));
	return 'ETH-' + Array.from(bytes, (b) => ALPHABET[b & 31]).join('');
}

/** What a report carries: the service reads exactly these. */
export interface FaultData {
	incident: string;
	code: Code;
	message: string;
	source: Source;
	level: Level;
	chain: Step[];
	stack: string;
	url: string;
}

export class Fault extends Error {
	readonly incident: string;
	readonly code: Code;
	readonly source: Source;
	readonly chain: Step[];
	readonly origin: string;
	constructor(code: Code, origin: Step, source: Source, stack: string, number = incident()) {
		super(CATALOGUE[code].message);
		this.name = 'Fault';
		this.incident = number;
		this.code = code;
		this.source = source;
		this.chain = [origin];
		this.origin = stack;
	}

	get show(): boolean {
		return CATALOGUE[this.code].show;
	}

	/** The map cannot go on. */
	get fatal(): boolean {
		return 'fatal' in CATALOGUE[this.code];
	}

	get level(): Level {
		return CATALOGUE[this.code].level;
	}

	/** What the user reads: the words, and the number to quote. */
	get text(): string {
		return `${this.message} (incident ${this.incident})`;
	}

	/** Bounded, as the service takes it. */
	data(url = globalThis.location?.href ?? ''): FaultData {
		const cut = (s: string) => s.slice(0, TEXT);
		return {
			incident: this.incident,
			code: this.code,
			message: cut(this.message),
			source: this.source,
			level: this.level,
			chain: this.chain.slice(0, STEPS).map((s) => ({ at: cut(s.at), what: cut(s.what) })),
			stack: cut(this.origin),
			url: cut(url)
		};
	}

	/** Back from a worker's message: same number, same steps. */
	static from(data: FaultData): Fault {
		const fault = new Fault(data.code, data.chain[0], data.source, data.stack, data.incident);
		fault.chain.push(...data.chain.slice(1));
		return fault;
	}
}

/** The first frame of a stack outside this file, as `file:line:column`. */
function place(stack: string | undefined, skip: RegExp | null = /[\\/]fault\.ts[:?]/): string {
	for (const line of (stack ?? '').split('\n').slice(1)) {
		const match = line.match(/\(?((?:[a-z]+:\/\/|\/|[A-Za-z]:)[^()\s]*?:\d+:\d+)\)?\s*$/);
		if (match && !skip?.test(match[1])) return match[1].replace(/\?[^:]*(?=:\d+:\d+$)/, '');
	}
	return 'unknown';
}

/** Raised here: the caller's place is its origin. */
export function raise(code: Code, what: string, source: Source = 'page'): Fault {
	const stack = new Error().stack ?? '';
	return new Fault(code, { at: place(stack), what }, source, stack);
}

/**
 * What each live WebAssembly module's panic hook left, if any: a trap
 * anywhere in the page is then traced to its Rust place (`fromTrap`).
 */
const traps: (() => string | null)[] = [];

/**
 * A fault raised elsewhere — in the worker — passed on here: same number,
 * one more step, no new code.
 */
export function relay(fault: Fault, what: string): Fault {
	fault.chain.push({ at: place(new Error().stack), what });
	return fault;
}

/** Registers a module whose last panic `read` gives. Returns the undo. */
export function readTraps(read: () => string | null): () => void {
	traps.push(read);
	return () => traps.splice(traps.indexOf(read), 1);
}

/**
 * Passed on by a layer, saying what it was doing. A fault keeps its number
 * and gains the step; a WebAssembly trap becomes one whose origin is the
 * Rust panic's place; anything else thrown becomes one, its own message and
 * place as origin.
 */
export function escalate(error: unknown, code: Code, what: string, source: Source = 'page'): Fault {
	const here = place(new Error().stack);
	if (error instanceof Fault) {
		error.chain.push({ at: here, what });
		return error;
	}
	if (typeof WebAssembly !== 'undefined' && error instanceof WebAssembly.RuntimeError) {
		const left = traps
			.map((read) => {
				try {
					return read();
				} catch {
					return null;
				}
			})
			.find(Boolean);
		if (left) return fromTrap(error, left, code, what);
	}
	const cause = error instanceof Error ? error : new Error(String(error));
	const fault = new Fault(
		code,
		{ at: place(cause.stack, null), what: cause.message },
		source,
		cause.stack ?? ''
	);
	fault.chain.push({ at: here, what });
	return fault;
}

/**
 * A WebAssembly trap: the panic hook left its Rust place and message in the
 * module (`src/wasm/fault.rs`) — read them, and the Rust place is the origin.
 * Without one, the trap itself is.
 */
export function fromTrap(
	error: unknown,
	left: string | null,
	code: Code,
	what: string,
	source: Source = 'wasm'
): Fault {
	if (!left) return escalate(error, code, what, source);
	const cut = left.indexOf('\n');
	const origin = {
		at: left.slice(0, cut < 0 ? left.length : cut),
		what: cut < 0 ? '' : left.slice(cut + 1)
	};
	const fault = new Fault(
		code,
		origin,
		source,
		error instanceof Error ? (error.stack ?? '') : String(error)
	);
	fault.chain.push({ at: place(new Error().stack), what });
	return fault;
}

/** Whether the console shows each fault's whole escalation. */
export function verbose(): boolean {
	try {
		if (new URLSearchParams(globalThis.location?.search ?? '').has('verbose')) return true;
		return globalThis.localStorage?.getItem('ethaor.verbose') === '1';
	} catch {
		return false;
	}
}

/**
 * Sends faults to the log. Each incident once, at most `limit` a minute —
 * a fault in a loop must not flood the service — and never a throw: losing
 * a report degrades the log, never the page.
 */
export class Reporter {
	private readonly sent = new Set<string>();
	private window: number[] = [];
	constructor(
		private readonly send: (body: string) => unknown,
		private readonly limit = 20,
		private readonly now = () => Date.now()
	) {}

	report(fault: Fault) {
		try {
			if (verbose()) console.error(`[${fault.incident}] ${fault.code}`, fault.chain, fault.origin);
			else console.warn(`[${fault.incident}] ${fault.code} at ${fault.chain[0].at}`);
			if (this.sent.has(fault.incident)) return;
			const t = this.now();
			this.window = this.window.filter((s) => t - s < 60_000);
			if (this.window.length >= this.limit) return;
			this.window.push(t);
			this.sent.add(fault.incident);
			const sending = this.send(JSON.stringify(fault.data()));
			if (sending instanceof Promise) sending.catch(() => {});
		} catch {
			// A report lost: the page goes on.
		}
	}
}

/** The page's reporter: a beacon, which survives the page closing. */
export const reporter = new Reporter((body) => {
	const blob = new Blob([body], { type: 'application/json' });
	if (navigator.sendBeacon?.('/api/report', blob)) return;
	return fetch('/api/report', {
		method: 'POST',
		body,
		keepalive: true,
		headers: { 'content-type': 'application/json' }
	});
});

/**
 * Catches what nothing else did — an error or a rejection left unhandled —
 * as an `uncaught` fault, reported and handed to `shown` for the user.
 * Returns what undoes it.
 */
export function watch(
	target: Window,
	shown: (fault: Fault) => void,
	report: Pick<Reporter, 'report'> = reporter
): () => void {
	const onError = (event: ErrorEvent) => {
		const fault =
			event.error instanceof Fault
				? event.error
				: escalate(event.error ?? event.message, 'uncaught', 'an error nothing caught');
		report.report(fault);
		if (fault.show) shown(fault);
	};
	const onRejection = (event: PromiseRejectionEvent) => {
		const fault = escalate(event.reason, 'unawaited', 'a promise nothing awaited');
		report.report(fault);
		if (fault.show) shown(fault);
	};
	target.addEventListener('error', onError);
	target.addEventListener('unhandledrejection', onRejection);
	return () => {
		target.removeEventListener('error', onError);
		target.removeEventListener('unhandledrejection', onRejection);
	};
}
