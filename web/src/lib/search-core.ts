/**
 * The search module (`ethaor-search.wasm`, the crate's `search` feature):
 * the codec that opens the corpus' text, a corpus of its own, and the
 * searches — ELS first, others to come.
 *
 * It runs in a worker (`search-worker.ts`), so neither the decode nor a
 * search — up to a second over every skip — ever holds a frame of the map;
 * and in the page itself only should no worker be available.
 */

import searchUrl from './ethaor-search.wasm?url';

interface Exports {
	memory: WebAssembly.Memory;
	codec_chunk(length: number): number;
	codec_push(): void;
	codec_finish(): number;
	codec_output(): number;
	text_buffer(length: number): number;
	text_load(): number;
	els_query_ptr(): number;
	els_query_cap(): number;
	els_search(length: number, skip0: number, skip1: number): number;
	els_hits_ptr(): number;
	els_hits_len(): number;
	els_stride(): number;
}

/** What an ELS search found: the exact total, and the first records. */
export interface Found {
	/** Every hit, counted; -1 while the text has not landed. */
	total: number;
	/** `stride` words per kept hit — see `els::STRIDE` in Rust. */
	hits: Uint32Array;
	stride: number;
}

const encoder = new TextEncoder();

export class SearchCore {
	private constructor(private readonly wasm: Exports) {}

	static async create(): Promise<SearchCore> {
		const { instance } = await WebAssembly.instantiateStreaming(fetch(searchUrl), {});
		return new SearchCore(instance.exports as unknown as Exports);
	}

	/** One chunk of the text part as it downloads: decoded as far as it allows. */
	push(chunk: Uint8Array) {
		const at = this.wasm.codec_chunk(chunk.length);
		// Views taken after each call: the module's memory may have grown.
		new Uint8Array(this.wasm.memory.buffer, at, chunk.length).set(chunk);
		this.wasm.codec_push();
	}

	/** No more chunks: decodes the rest, lands it here, and gives it, opened. */
	finish(): Uint8Array {
		const length = this.wasm.codec_finish();
		const plain = new Uint8Array(this.wasm.memory.buffer, this.wasm.codec_output(), length).slice();
		this.land(plain);
		return plain;
	}

	/** Lands the opened text in this module's corpus: the searches can run. */
	land(plain: Uint8Array) {
		const at = this.wasm.text_buffer(plain.length);
		new Uint8Array(this.wasm.memory.buffer, at, plain.length).set(plain);
		this.wasm.text_load();
	}

	/** ELS: `query` read with `skip0..=skip1` letters jumped between its letters. */
	els(query: string, skip0: number, skip1: number): Found {
		const bytes = encoder.encode(query).subarray(0, this.wasm.els_query_cap());
		new Uint8Array(this.wasm.memory.buffer, this.wasm.els_query_ptr(), bytes.length).set(bytes);
		const total = this.wasm.els_search(bytes.length, skip0, skip1);
		const stride = this.wasm.els_stride();
		const words = this.wasm.els_hits_len() * stride;
		const hits = new Uint32Array(this.wasm.memory.buffer, this.wasm.els_hits_ptr(), words).slice();
		return { total, hits, stride };
	}
}
