// Builds the two WebAssembly modules and copies them, with the corpus' text
// part, next to the code that imports them. Vite emits them at build time
// under `_app/immutable/`, each with its content hash in its name.
//
// One crate, two builds (see the features in Cargo.toml):
//   ethaor.wasm         `map`: what the page draws with — on the path of the
//                       first image, so as small as it can be
//   ethaor-search.wasm  `search`: the codec and the searches, which a worker
//                       loads once the first image is drawn
// Both builds write the same file, so each is copied as soon as it is built.
//
// The text part ships three times: through the codec (`ethaor-text.bin`),
// smallest, for slow links; opened then gzipped (`ethaor-text.gzip.bin`),
// which the browser decodes natively at once, for fast links — the page picks
// one by the bandwidth it measured; and opened, uncompressed
// (`ethaor-text.plain.bin`), never downloaded whole: tiles ask it for one
// chapter's bytes at a time.
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
const release = join(root, 'target/wasm32-unknown-unknown/release');
const lib = resolve(here, '../src/lib');
mkdirSync(lib, { recursive: true });

// What each module may export: the split is only worth its bytes if the
// search's code stays out of the map's module — a feature mixed up would
// bring it back onto the path of the first image without a sound.
const FORBIDDEN = {
	'ethaor.wasm': /^(els_|codec_|probe_els_)/,
	'ethaor-search.wasm': /^(init|build_frame|zoom|tiles_wanted|tile_load)$/
};

function copy(from, name) {
	if (FORBIDDEN[name]) {
		const exported = WebAssembly.Module.exports(new WebAssembly.Module(readFileSync(from))).map(
			(e) => e.name
		);
		const stray = exported.filter((n) => FORBIDDEN[name].test(n));
		if (stray.length)
			throw new Error(`${name} exporte ce qui ne lui revient pas : ${stray.join(', ')}`);
	}
	const to = join(lib, name);
	copyFileSync(from, to);
	console.log(`${name} -> src/lib/ (${Math.round(statSync(to).size / 1024)} Ko)`);
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
			'--target',
			'wasm32-unknown-unknown',
			'--release',
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
			return statSync(join(dir, 'text.pack')).isFile();
		} catch {
			return false;
		}
	})
	.sort(
		(a, b) => statSync(join(b, 'text.pack')).mtimeMs - statSync(join(a, 'text.pack')).mtimeMs
	)[0];
if (!out) throw new Error('text.pack introuvable : le build Rust a-t-il tourne ?');

copy(join(out, 'text.pack'), 'ethaor-text.bin');
copy(join(out, 'text.plain'), 'ethaor-text.plain.bin');
// Not named `.gz`: a host may take that for a precompressed sibling and send
// it with a `Content-Encoding`, which would open it before the page does.
const gzip = join(lib, 'ethaor-text.gzip.bin');
writeFileSync(gzip, gzipSync(readFileSync(join(out, 'text.plain')), { level: 9, memLevel: 9 }));
console.log(`ethaor-text.gzip.bin -> src/lib/ (${Math.round(statSync(gzip).size / 1024)} Ko)`);

// The codec is only worth its decode while it stays well below gzip: a third
// smaller at the least (measured: 38 %).
const sealed = statSync(join(lib, 'ethaor-text.bin')).size;
const gzipped = statSync(gzip).size;
if (sealed > gzipped * 0.65)
	throw new Error(`le codec (${sealed} o) ne bat plus gzip (${gzipped} o) d'un tiers`);
