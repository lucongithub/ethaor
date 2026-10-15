// Times the three ELS methods in V8 — the engine the page runs in — through
// the very exports the page calls.
//
// The grid and the cost model's inputs come from the native benchmark's table
// (`bench_methods`, see CLAUDE.md), which also marks the runs too long to
// time: those are skipped here too. Prints the same table with V8 times, for
// `tune-els.mjs`.
//
//   node scripts/bench-els.mjs <native.csv> > v8.csv
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';

const [, , table] = process.argv;
if (!table) {
	console.error('usage: node scripts/bench-els.mjs <native.csv>   (after npm run wasm)');
	process.exit(1);
}
// The search module, and the text it searches, as the page hands it over:
// book after book, then the readings. Without them every search would answer
// -1 — the text not in — and the bench would time nothing.
const lib = new URL('../src/lib/', import.meta.url);
const { instance } = await WebAssembly.instantiate(
	readFileSync(new URL('ethaor-search.wasm', lib)),
	{}
);
const e = instance.exports;
const books = readFileSync(new URL('ethaor-text.books.bin', lib));
const cuts = JSON.parse(readFileSync(new URL('ethaor-text.books.json', lib), 'utf8'));
const block = (i) => gunzipSync(books.subarray(cuts[i], cuts[i + 1]));
// The allocation may grow the memory: the view is taken after it.
const write = (bytes) => {
	const at = e.tile_buffer(bytes.length);
	new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes);
};
const map = new WebAssembly.Instance(
	new WebAssembly.Module(readFileSync(new URL('ethaor.wasm', lib))),
	{}
).exports;
for (let b = 0; b < cuts.length - 2; b++) {
	write(block(b));
	e.tile_load(map.book_first_chapter(b), map.book_end_chapter(b));
}
write(block(cuts.length - 2));
if (e.readings_load() !== 1) throw new Error('le texte ne se pose pas dans le module de recherche');
e.els_prepare();
const encoder = new TextEncoder();

/** One search with a method imposed (1, 2, 3), memory of answers emptied. */
function once(word, skip0, skip1, method) {
	e.probe_els_method(method);
	e.probe_els_forget(0);
	const bytes = encoder.encode(word);
	new Uint8Array(e.memory.buffer, e.els_query_ptr(), bytes.length).set(bytes);
	const start = performance.now();
	e.els_search(bytes.length, skip0, skip1);
	return performance.now() - start;
}

console.log('word;skip0;skip1;anchors;steps;f2;lanes;total;direct;pairs;planes');
for (const line of readFileSync(table, 'utf8').split('\n')) {
	const f = line.trim().split(';');
	if (f.length < 11 || f[0] === 'word') continue;
	const [word, skip0, skip1] = [f[0], Number(f[1]), Number(f[2])];
	const out = f.slice(0, 8);
	for (let method = 1; method <= 3; method++) {
		const native = f[7 + method];
		if (native === '-' || Number(native) > 3000) {
			out.push('-');
			continue;
		}
		// Median of a few runs, within a small budget per case.
		const times = [];
		const begin = performance.now();
		while (times.length < 3 || (performance.now() - begin < 200 && times.length < 30)) {
			const t = once(word, skip0, skip1, method);
			times.push(t);
			if (t > 1500) break;
		}
		times.sort((a, b) => a - b);
		out.push(times[times.length >> 1].toFixed(4));
	}
	console.log(out.join(';'));
}
