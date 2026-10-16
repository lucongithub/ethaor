// Declares in build/index.html what the first image waits for, so the
// browser fetches it while it reads the HTML instead of once the scripts have
// run: the module and the Hebrew font. Not the text: the page asks for it
// once the first image is drawn, so it never shares the pipe with it. A host
// such as Cloudflare also turns these links into 103 Early Hints, sent
// before the page itself.
//
// The names carry a content hash, known only after the build: hence a step
// after it rather than a line in app.html.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const build = resolve(dirname(fileURLToPath(import.meta.url)), '../build');
const assets = join(build, '_app/immutable/assets');
const names = readdirSync(assets);
const find = (pattern) => {
	const name = names.find((n) => pattern.test(n));
	if (!name) throw new Error(`preload: no file matches ${pattern}`);
	return `./_app/immutable/assets/${name}`;
};

// `crossorigin` on every link: the page fetches these through `fetch` and
// the font loader, both in CORS mode — a preload in another mode would be
// downloaded twice.
const links = [
	`<link rel="preload" href="${find(/^ethaor\.[\w-]+\.wasm$/)}" as="fetch" type="application/wasm" crossorigin>`,
	`<link rel="preload" href="${find(/^noto-serif-hebrew-hebrew-wght-normal\.[\w-]+\.woff2$/)}" as="font" type="font/woff2" crossorigin>`
];

const index = join(build, 'index.html');
const html = readFileSync(index, 'utf8');
if (html.includes('rel="preload"')) throw new Error('preload: index.html is already processed');
writeFileSync(index, html.replace('</head>', `\t${links.join('\n\t\t')}\n\t</head>`));
console.log(`preload: ${links.length} links in index.html`);
