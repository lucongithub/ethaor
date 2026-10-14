// Copies the compiled module next to the bridge that imports it. Vite emits it
// at build time under `_app/immutable/`, its content hash in the name.
import { copyFileSync, mkdirSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const from = resolve(here, '../../target/wasm32-unknown-unknown/release/ethaor.wasm');
const to = resolve(here, '../src/lib/ethaor.wasm');

mkdirSync(dirname(to), { recursive: true });
copyFileSync(from, to);
console.log(`ethaor.wasm -> src/lib/ (${Math.round(statSync(to).size / 1024)} Ko)`);
