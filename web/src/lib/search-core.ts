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
	els_search(length: number, skip0: number, skip1: number): number;
	els_hits_ptr(): number;
	els_hits_len(): number;
	els_stride(): number;
	fault_ptr(): number;
	fault_len(): number;
	probe_panic(): void;
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
