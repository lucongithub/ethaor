// Builds the two WebAssembly modules and copies them, with the corpus' text
// part, next to the code that imports them. Vite emits them at build time
// under `_app/immutable/`, each with its content hash in its name.
//
// One crate, two builds (see the features in Cargo.toml):
//   ethaor.wasm         `map`: what the page draws with — on the path of the
//                       first image, so as small as it can be
//   ethaor-search.wasm  `search`: the searches, which a worker
//                       loads once the first image is drawn
// Both builds write the same file, so each is copied as soon as it is built.
//
// The text part ships twice: cut into books, each gzipped on its own
// (`ethaor-text.books.bin`), which the page fetches in the background nearest
// to the view first and the browser opens natively; and uncompressed
// (`ethaor-text.plain.bin`), never downloaded whole — tiles ask it for one
// chapter's bytes at a time, the ones a fast dive needs at once.
import { execFileSync } from 'node:child_process';
import {
	copyFileSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
// The `wasm` profile: release, aborting on panic (see Cargo.toml).
const release = join(root, 'target/wasm32-unknown-unknown/wasm');
const lib = resolve(here, '../src/lib');
mkdirSync(lib, { recursive: true });

// What each module may export: the split is only worth its bytes if the
// search's code stays out of the map's module — a feature mixed up would
// bring it back onto the path of the first image without a sound.
const FORBIDDEN = {
	'ethaor.wasm': /^(els_|probe_els_|words_)/,
	'ethaor-search.wasm': /^(init|build_frame|zoom|tiles_wanted|next_book)$/
};

function copy(from, name) {
	if (FORBIDDEN[name]) {
		const exported = WebAssembly.Module.exports(new WebAssembly.Module(readFileSync(from))).map(
			(e) => e.name
		);
		const stray = exported.filter((n) => FORBIDDEN[name].test(n));
		if (stray.length) throw new Error(`${name} exports what is not its own: ${stray.join(', ')}`);
	}
	const to = join(lib, name);
	copyFileSync(from, to);
	console.log(`${name} -> src/lib/ (${Math.round(statSync(to).size / 1024)} KB)`);
}

for (const [feature, name] of [
	['map', 'ethaor.wasm'],
	['search', 'ethaor-search.wasm']
]) {
	execFileSync(
		'cargo',
		[
			'build',
			'--manifest-path',
			join(root, 'Cargo.toml'),
			// The workspace also holds the native server, which has no
			// WebAssembly build.
			'--package',
			'ethaor',
			'--target',
			'wasm32-unknown-unknown',
			'--profile',
			'wasm',
			'--no-default-features',
			'--features',
			feature
		],
		{ stdio: 'inherit' }
	);
	copy(join(release, 'ethaor.wasm'), name);
}

// The text part is written by build.rs into its OUT_DIR, whose name carries a
// hash: take the most recent one.
const builds = join(release, 'build');
const out = readdirSync(builds)
	.filter((name) => name.startsWith('ethaor-'))
	.map((name) => join(builds, name, 'out'))
	.filter((dir) => {
		try {
			return statSync(join(dir, 'text.plain')).isFile();
		} catch {
			return false;
		}
	})
	.sort(
		(a, b) => statSync(join(b, 'text.plain')).mtimeMs - statSync(join(a, 'text.plain')).mtimeMs
	)[0];
if (!out) throw new Error('text.plain not found: did the Rust build run?');

copy(join(out, 'text.plain'), 'ethaor-text.plain.bin');
// The text cut into books, each gzipped on its own, then the readings: what
// the page fetches in the background, books nearest to the view first. One
// file, so one hashed name; each block is a Range request. Where the blocks
// lie goes into a small JSON the page imports with its code: no request to
// learn it.
const plain = readFileSync(join(out, 'text.plain'));
const raw = readFileSync(join(out, 'text.books'));
const cuts = Array.from({ length: raw.length / 4 }, (_, i) => raw.readUInt32LE(i * 4));
const blocks = cuts
	.slice(0, -1)
	.map((from, i) => gzipSync(plain.subarray(from, cuts[i + 1]), { level: 9, memLevel: 9 }));
const starts = [0];
for (const block of blocks) starts.push(starts[starts.length - 1] + block.length);
writeFileSync(join(lib, 'ethaor-text.books.bin'), Buffer.concat(blocks));
writeFileSync(join(lib, 'ethaor-text.books.json'), JSON.stringify(starts));
console.log(
	`ethaor-text.books.bin -> src/lib/ (${blocks.length} blocks, ${Math.round(starts.at(-1) / 1024)} KB)`
);
