/**
 * The search module, off the frame loop.
 *
 * The page forwards the corpus' text as it gets it — book after book
 * (`tile`), then the readings — and the searches it wants (`els`). The worker
 * keeps a corpus of its own, so a search never touches the map's module, and
 * answers each search by its id. Searches asked before the whole text has
 * landed wait for it.
 *
 * A search that fails — the module trapped — is answered by its id with its
 * fault, numbered, its origin the Rust place the panic hook left; never left
 * unanswered. The module, whose state a trap may have left half-written, is
 * replaced by a new one fed the text again: the worker keeps it for that.
 */

import { escalate, fromTrap, type FaultData } from './fault';
import { SearchCore, type Found, type WordsOptions } from './search-core';

export type Request =
	| { tile: { first: number; end: number; codes: Uint8Array } }
	| { readings: Uint8Array }
	| { els: { id: number; query: string; skip0: number; skip1: number; exact?: boolean } }
	| { words: { id: number; query: string; options: WordsOptions } };

/**
 * A search's answer, or its failure by its id; or a failure feeding the
 * text, by no id. A failure carries its fault, for the page to escalate and
 * report.
 */
export type Answer =
	{ id: number; found: Found } | { id?: number; error: string; fault: FaultData };

type Text = Exclude<Request, { els: unknown } | { words: unknown }>;

/** What a search request runs on the module, by its id. */
function searchOf(message: Request): { id: number; run: (c: SearchCore) => Found } | null {
	if ('els' in message) {
		const { id, query, skip0, skip1, exact } = message.els;
		return { id, run: (c) => c.els(query, skip0, skip1, exact) };
	}
	if ('words' in message) {
		const { id, query, options } = message.words;
		return { id, run: (c) => c.words(query, options) };
	}
	return null;
}

let core = SearchCore.create();
let landed: () => void = () => {};
const text = new Promise<void>((done) => (landed = done));
// Text messages are handled in order, even across the instantiation.
let queue: Promise<void> = Promise.resolve();
// The text as it came, for a new module after a failure.
const fed: Text[] = [];

function feed(c: SearchCore, message: Text): boolean {
	if ('tile' in message) {
		c.tile(message.tile.first, message.tile.end, message.tile.codes);
		return false;
	}
	return c.readings(message.readings);
}

/** A failure feeding the text: answered by no id, the page degrades. */
function failed(cause: unknown, what: string) {
	const fault = escalate(cause, 'worker-failed', what, 'worker');
	self.postMessage({ error: fault.message, fault: fault.data() });
}

/** A new module, fed again everything the old one had. */
function renew() {
	core = SearchCore.create();
	const replay = fed.slice();
	queue = queue
		.then(() => core)
		.then((c) => replay.forEach((m) => feed(c, m)))
		.catch((cause) => failed(cause, 'feeding a new module the text again'));
}

self.onmessage = (event: MessageEvent<Request>) => {
	const message = event.data;
	const search = searchOf(message);
	if (search) {
		const { id, run } = search;
		// The module the search runs on: the current one once the text queued
		// before it is in — not the one when it was asked, which may be gone.
		let searching = core;
		text
			.then(() => queue)
			.then(() => (searching = core))
			.then((c) => {
				const found = run(c);
				self.postMessage({ id, found }, { transfer: [found.hits.buffer] });
			})
			.catch(async (cause) => {
				// A trap left its Rust place in the module: read it before the
				// module is replaced.
				const left =
					cause instanceof WebAssembly.RuntimeError
						? (await searching.catch(() => null))?.panic()
						: null;
				const [code, what] = ['search-failed', 'a search in the worker'] as const;
				const fault = left
					? fromTrap(cause, left, code, what)
					: escalate(cause, code, what, 'worker');
				self.postMessage({ id, error: fault.message, fault: fault.data() });
				renew();
			});
		return;
	}
	const piece = message as Text;
	fed.push(piece);
	queue = queue
		.then(() => core)
		.then((c) => {
			if (feed(c, piece)) landed();
		})
		.catch((cause) => failed(cause, 'feeding the module the text'));
};
