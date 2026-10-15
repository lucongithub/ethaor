/**
 * The search module, off the frame loop.
 *
 * The page forwards the corpus' text as it gets it — book after book
 * (`tile`), then the readings — and the searches it wants (`els`). The worker
 * keeps a corpus of its own, so a search never touches the map's module, and
 * answers each search by its id. Searches asked before the whole text has
 * landed wait for it.
 */

import { SearchCore, type Found } from './search-core';

export type Request =
	| { tile: { first: number; end: number; codes: Uint8Array } }
	| { readings: Uint8Array }
	| { els: { id: number; query: string; skip0: number; skip1: number } };

export type Answer = { id: number; found: Found } | { error: string };

const core = SearchCore.create();
let landed: () => void = () => {};
const text = new Promise<void>((done) => (landed = done));
// Text messages are handled in order, even across the instantiation.
let queue: Promise<void> = Promise.resolve();

self.onmessage = (event: MessageEvent<Request>) => {
	const message = event.data;
	if ('els' in message) {
		text
			.then(() => core)
			.then((c) => {
				const found = c.els(message.els.query, message.els.skip0, message.els.skip1);
				self.postMessage({ id: message.els.id, found }, { transfer: [found.hits.buffer] });
			})
			.catch((cause) => self.postMessage({ error: String(cause) }));
		return;
	}
	queue = queue
		.then(() => core)
		.then((c) => {
			if ('tile' in message) c.tile(message.tile.first, message.tile.end, message.tile.codes);
			else if (c.readings(message.readings)) landed();
		})
		.catch((cause) => self.postMessage({ error: String(cause) }));
};
