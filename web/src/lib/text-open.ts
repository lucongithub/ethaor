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
