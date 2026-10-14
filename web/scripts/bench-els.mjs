// Times the three ELS methods in V8 — the engine the page runs in — through
// the very exports the page calls.
//
// The grid and the cost model's inputs come from the native benchmark's table
// (`bench_methods`, see CLAUDE.md), which also marks the runs too long to
// time: those are skipped here too. Prints the same table with V8 times, for
// `tune-els.mjs`.
//
//   node scripts/bench-els.mjs <ethaor.wasm> <native.csv> > v8.csv
import { readFileSync } from 'node:fs';

const [, , wasm, table] = process.argv;
if (!wasm || !table) {
	console.error('usage: node scripts/bench-els.mjs <ethaor.wasm> <native.csv>');
	process.exit(1);
}
const { instance } = await WebAssembly.instantiate(readFileSync(wasm), {});
const e = instance.exports;
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
