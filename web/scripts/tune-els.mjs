// Tunes the six constants of the ELS cost model (`choose` in src/els.rs) on a
// benchmark table, so that the method it picks is the fastest as often as
// possible — measured by the time actually spent over the whole grid, not by
// how well it predicts each time.
//
// Starts from the constants in the source, adjusts them one at a time by
// small factors while the total improves, and reports the picked total
// against the best possible one and the worst single miss. A second table,
// if given, is only scored: a check that the constants hold elsewhere.
//
//   node scripts/tune-els.mjs v8.csv [native.csv]
import { readFileSync } from 'node:fs';

const [, , tuneOn, checkOn] = process.argv;
if (!tuneOn) {
	console.error('usage: node scripts/tune-els.mjs <table.csv> [check.csv]');
	process.exit(1);
}

/** Rows of a table: inputs of the model and the three times (null = not run). */
function load(path) {
	const rows = [];
	for (const line of readFileSync(path, 'utf8').split('\n')) {
		const f = line.trim().split(';');
		if (f.length < 11 || f[0] === 'word') continue;
		rows.push({
			case: `${f[0]} ${f[1]}..${f[2]}`,
			anchors: Number(f[3]),
			steps: Number(f[4]),
			second: Number(f[5]),
			lanes: Math.min(Number(f[6]), 3),
			times: f.slice(8, 11).map((x) => (x === '-' ? null : Number(x)))
		});
	}
	return rows;
}

/** The model of `choose`: Direct, Pairs, Planes, in nanoseconds. */
function estimate(c, r) {
	return [
		c[0] * r.anchors + c[1] * r.steps,
		c[2] * r.anchors + c[3] * r.steps * r.second,
		c[4] * r.anchors + (c[5] * r.steps * r.lanes) / 64
	];
}

function score(c, rows) {
	let picked = 0;
	let best = 0;
	let worst = { ratio: 1, case: '' };
	for (const r of rows) {
		const e = estimate(c, r);
		const pick = e.indexOf(Math.min(...e));
		const time = r.times[pick] ?? Infinity;
		const fastest = Math.min(...r.times.filter((t) => t !== null));
		picked += time;
		best += fastest;
		if (time / fastest > worst.ratio) worst = { ratio: time / fastest, case: r.case };
	}
	return { picked, best, worst };
}

// The constants of `choose`, in its order.
let c = [1.89, 1.84, 19.0, 2.35, 9.9, 1.97];
const rows = load(tuneOn);
// The worst miss weighs a little, so a gain on the total does not buy a bad
// surprise on one query.
const cost = (k) => {
	const s = score(k, rows);
	return s.picked * (1 + 0.02 * s.worst.ratio);
};
let current = cost(c);
for (let pass = 0; pass < 6; pass++) {
	for (let k = 0; k < c.length; k++) {
		for (const factor of [0.5, 0.7, 0.85, 0.95, 1.05, 1.2, 1.4, 2]) {
			const d = [...c];
			d[k] *= factor;
			const v = cost(d);
			if (v < current) [current, c] = [v, d];
		}
	}
}

console.log(`constants: ${c.map((x) => x.toFixed(2)).join(', ')}`);
for (const [name, path] of [
	['tuned on', tuneOn],
	['checked on', checkOn]
]) {
	if (!path) continue;
	const s = score(c, load(path));
	console.log(
		`${name} ${path}: picked ${s.picked.toFixed(0)} ms, best ${s.best.toFixed(0)} ms ` +
			`(+${((s.picked / s.best - 1) * 100).toFixed(1)} %), worst x${s.worst.ratio.toFixed(2)} (${s.worst.case})`
	);
}
