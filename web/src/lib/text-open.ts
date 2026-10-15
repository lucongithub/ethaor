/**
 * Getting the corpus' text part: read as it downloads, inflated natively,
 * timed, or fetched one tile at a time.
 */

/** Reads a download chunk by chunk, handing each over as it arrives. */
export async function chunks(response: Response, each: (chunk: Uint8Array) => void) {
	if (!response.ok || !response.body) throw new Error(`texte indisponible : ${response.status}`);
	const reader = response.body.getReader();
	for (;;) {
		const { done, value } = await reader.read();
		if (done) return;
		each(value);
	}
}

/**
 * The other form of the text part: opened, then gzipped, which the browser
 * itself decodes — natively, as it downloads, at no cost worth measuring.
 * Larger than the codec's form, so it is for fast links.
 */
export async function inflate(response: Response): Promise<Uint8Array> {
	if (!response.ok || !response.body) throw new Error(`texte indisponible : ${response.status}`);
	const opened = response.body.pipeThrough(new DecompressionStream('gzip'));
	return new Uint8Array(await new Response(opened).arrayBuffer());
}

/**
 * Bytes per second of a resource's download, from the browser's own timing
 * of it — its body only, latency left out. `null` when it cannot tell: from
 * the cache, or too quick to time.
 */
export function bandwidth(url: string): number | null {
	const entry = performance.getEntriesByName(new URL(url, location.href).href)[0] as
		PerformanceResourceTiming | undefined;
	if (!entry || entry.encodedBodySize === 0) return null;
	const seconds = (entry.responseEnd - entry.responseStart) / 1000;
	return seconds > 0.02 ? entry.encodedBodySize / seconds : null;
}

/**
 * Bytes `from..to` of a file, by a Range request — a tile. Should the server
 * ignore the range and send the whole file, only what is needed is read
 * before the download is dropped.
 */
export async function range(url: string, from: number, to: number): Promise<Uint8Array> {
	const response = await fetch(url, { headers: { Range: `bytes=${from}-${to - 1}` } });
	if (response.status === 206) return new Uint8Array(await response.arrayBuffer());
	if (!response.ok || !response.body) throw new Error(`tuile indisponible : ${response.status}`);
	const out = new Uint8Array(to);
	const reader = response.body.getReader();
	let at = 0;
	while (at < to) {
		const { done, value } = await reader.read();
		if (done) break;
		out.set(value.subarray(0, to - at), at);
		at += value.length;
	}
	await reader.cancel();
	return out.subarray(from, to);
}
