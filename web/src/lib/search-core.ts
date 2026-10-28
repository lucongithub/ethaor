/**
 * The search module (`ethaor-search.wasm`, the crate's `search` feature):
 * a corpus of its own and the searches — ELS first, others to come.
 *
 * It runs in a worker (`search-worker.ts`), so no search — up to a second
 * over every skip — ever holds a frame of the map; and in the page itself
 * only should no worker be available.
 */

import searchUrl from './ethaor-search.wasm?url';

interface Exports {
	memory: WebAssembly.Memory;
	tile_buffer(length: number): number;
	tile_load(first: number, end: number): number;
	readings_load(): number;
	els_query_ptr(): number;
	els_query_cap(): number;
	/** `exact` 1 tells final forms apart; 0 folds them. */
	els_search(length: number, skip0: number, skip1: number, exact: number): number;
	els_hits_ptr(): number;
	els_hits_len(): number;
	els_stride(): number;
	/** One letter of each word: the `nth`, from its end with `fromEnd` 1, in any order with `anyOrder` 1. */
	words_search(
		length: number,
		nth: number,
		fromEnd: number,
		anyOrder: number,
		exact: number
	): number;
	words_hits_ptr(): number;
	words_hits_len(): number;
	words_stride(): number;
	fault_ptr(): number;
	fault_len(): number;
	probe_panic(): void;
}

/** What a search found: the exact total, and the first records. */
export interface Found {
	/** Every hit, counted; -1 while the text has not landed. */
	total: number;
	/** `stride` words per kept hit — see `els::STRIDE` in Rust, or for
	 *  `letters` `words::LETTER_WORDS` per letter. */
	hits: Uint32Array;
	stride: number;
	/** How a record names its letters: an ELS hit's first and its skip
	 *  (absent), or each letter's verse and rank (`letters`). */
	form?: 'letters';
}

/** Which letter of each word a first- or last-letters search reads. */
export interface WordsOptions {
	/** 1 the first letter of each word; counted from its end with `fromEnd`. */
	nth: number;
	fromEnd?: boolean;
	/** The word's letters in any order. */
	anyOrder?: boolean;
	exact?: boolean;
}

const encoder = new TextEncoder();

export class SearchCore {
	private constructor(private readonly wasm: Exports) {}

	static async create(): Promise<SearchCore> {
		const { instance } = await WebAssembly.instantiateStreaming(fetch(searchUrl), {});
		return new SearchCore(instance.exports as unknown as Exports);
	}

	/** Lands a run of chapters — a book — in this module's corpus. */
	tile(first: number, end: number, codes: Uint8Array) {
		const at = this.wasm.tile_buffer(codes.length);
		new Uint8Array(this.wasm.memory.buffer, at, codes.length).set(codes);
		this.wasm.tile_load(first, end);
	}

	/** Lands the readings once every book is in: the searches can run. */
	readings(bytes: Uint8Array): boolean {
		const at = this.wasm.tile_buffer(bytes.length);
		new Uint8Array(this.wasm.memory.buffer, at, bytes.length).set(bytes);
		return this.wasm.readings_load() === 1;
	}

	/**
	 * What the module's last panic left — its Rust place, a new line, its
	 * message — or null. Read after a trap (`src/wasm/fault.rs`).
	 */
	panic(): string | null {
		try {
			const length = this.wasm.fault_len();
			if (length === 0) return null;
			return new TextDecoder().decode(
				new Uint8Array(this.wasm.memory.buffer, this.wasm.fault_ptr(), length)
			);
		} catch {
			return null;
		}
	}

	/** Probe: makes the module panic, for the tests of what a trap leaves. */
	probePanic() {
		this.wasm.probe_panic();
	}

	/**
	 * ELS: `query` read with `skip0..=skip1` letters jumped between its
	 * letters; with `exact`, its final forms told apart.
	 */
	els(query: string, skip0: number, skip1: number, exact = false): Found {
		const bytes = this.write(query);
		const total = this.wasm.els_search(bytes.length, skip0, skip1, exact ? 1 : 0);
		const stride = this.wasm.els_stride();
		const words = this.wasm.els_hits_len() * stride;
		const hits = new Uint32Array(this.wasm.memory.buffer, this.wasm.els_hits_ptr(), words).slice();
		return { total, hits, stride };
	}

	/**
	 * First or last letters: `query` read one letter from each word, side
	 * by side — the `nth` of each word, from its end with `fromEnd` — in
	 * order or, with `anyOrder`, in any.
	 */
	words(query: string, options: WordsOptions): Found {
		const bytes = this.write(query);
		const { nth, fromEnd = false, anyOrder = false, exact = false } = options;
		const total = this.wasm.words_search(bytes.length, nth, +fromEnd, +anyOrder, +exact);
		const stride = this.wasm.words_stride();
		const hits = new Uint32Array(
			this.wasm.memory.buffer,
			this.wasm.words_hits_ptr(),
			this.wasm.words_hits_len()
		).slice();
		return { total, hits, stride, form: 'letters' };
	}

	/** Writes a query where every search reads it; its bytes. */
	private write(query: string): Uint8Array {
		const bytes = encoder.encode(query).subarray(0, this.wasm.els_query_cap());
		new Uint8Array(this.wasm.memory.buffer, this.wasm.els_query_ptr(), bytes.length).set(bytes);
		return bytes;
	}
}
