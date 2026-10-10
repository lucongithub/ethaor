// Copie le module compile dans static/, d'ou SvelteKit le sert tel quel.
import { copyFileSync, mkdirSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const from = resolve(here, '../../target/wasm32-unknown-unknown/release/ethaor.wasm');
const to = resolve(here, '../static/ethaor.wasm');

mkdirSync(dirname(to), { recursive: true });
copyFileSync(from, to);
console.log(`ethaor.wasm -> static/ (${Math.round(statSync(to).size / 1024)} Ko)`);
