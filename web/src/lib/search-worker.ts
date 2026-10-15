/**
 * The search module, off the frame loop.
 *
 * The page sends the corpus' text as it gets it — through the codec chunk by
 * chunk (`chunk`, then `done`: decoded as the bytes arrive, the opened text
 * goes back to the page), or already opened (`plain`) — and the searches it
 * wants (`els`). The worker keeps a corpus of its own, so a search never
 * touches the map's module, and answers each search by its id. Searches asked
 * before the text has landed wait for it.
 */

import { SearchCore, type Found } from './search-core';

export type Request =
	| { chunk: Uint8Array }
	| { done: true }
	| { plain: Uint8Array }
	| { els: { id: number; query: string; skip0: number; skip1: number } };

export type Answer = { plain: Uint8Array } | { id: number; found: Found } | { error: string };

const core = SearchCore.create();
let landed: () => void = () => {};
const text = new Promise<void>((done) => (landed = done));
// Messages are handled one after the other, in order, even across awaits.
let queue: Promise<void> = Promise.resolve();

function answer(message: Answer, transfer: Transferable[] = []) {
	self.postMessage(message, { transfer });
}

self.onmessage = (event: MessageEvent<Request>) => {
	const message = event.data;
	if ('els' in message) {
		// Not in the queue: a search waits for the text, not for other messages.
		text
			.then(() => core)
			.then((c) => {
				const found = c.els(message.els.query, message.els.skip0, message.els.skip1);
				answer({ id: message.els.id, found }, [found.hits.buffer]);
			})
			.catch((cause) => answer({ error: String(cause) }));
		return;
	}
	queue = queue
		.then(async () => {
			const c = await core;
			if ('chunk' in message) {
				c.push(message.chunk);
			} else if ('done' in message) {
				const plain = c.finish();
				landed();
				answer({ plain }, [plain.buffer]);
			} else {
				c.land(message.plain);
				landed();
			}
		})
		.catch((cause) => answer({ error: String(cause) }));
};
