/**
 * Getting the corpus' text part: by Range requests — a book, gzipped and
 * opened natively, or a tile — from the files the build cut.
 */

/** Opens gzipped bytes, natively. */
export async function inflate(bytes: Uint8Array): Promise<Uint8Array> {
	const opened = new Blob([bytes.slice()]).stream().pipeThrough(new DecompressionStream('gzip'));
	return new Uint8Array(await new Response(opened).arrayBuffer());
}

/**
 * Bytes `from..to` of a file, by a Range request — a tile. Should the server
 * ignore the range and send the whole file, only what is needed is read
 * before the download is dropped.
 */
export async function range(
	url: string,
	from: number,
	to: number,
	priority: RequestPriority = 'auto'
): Promise<Uint8Array> {
	const response = await fetch(url, { headers: { Range: `bytes=${from}-${to - 1}` }, priority });
	if (response.status === 206) return new Uint8Array(await response.arrayBuffer());
	if (!response.ok || !response.body) throw new Error(`tile unavailable: ${response.status}`);
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

/**
 * Runs `lanes` loops at once, each taking the next item (`next`, negative
 * once there is none) and awaiting `take` before asking again. The item is
 * chosen when a lane frees, not up front: what is nearest to the view then
 * is what comes, even if the view moved meanwhile.
 */
export async function lanes(
	count: number,
	next: () => number,
	take: (item: number) => Promise<void>
): Promise<void> {
	const lane = async () => {
		for (let item = next(); item >= 0; item = next()) await take(item);
	};
	await Promise.all(Array.from({ length: count }, lane));
}

/**
 * Key of a corpus fragment in the page's cache of decoded strings: one per
 * (offset, length), whatever both. Lengths take sixteen bits, so a verse of
 * more than a kilobyte keys apart from its neighbour's fragments; with the
 * corpus' offsets under 2^37 the key stays an exact integer.
 */
export function fragmentKey(offset: number, length: number): number {
	return offset * 65536 + length;
}
