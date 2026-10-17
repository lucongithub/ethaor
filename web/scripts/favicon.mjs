// Draws the site's icons — the tab's and a phone's home screen — from the
// mark (`src/lib/components/Logo.svelte`): two nuns facing each other and a
// point, `[·]`, set in Noto Serif Hebrew, the Tanakh's own face.
//
// Chrome draws them with the real font: an SVG icon could not load it, and
// outlines copied out of the font would be a second drawing to keep in step.
// Run again after any change to the mark:
//
//   node scripts/favicon.mjs        (Chrome at CHROME, or its usual place)
//
// Writes static/favicon-32.png and static/apple-touch-icon.png, versioned.
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const web = resolve(here, '..');
const chrome = process.env.CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const font = readFileSync(
	join(
		web,
		'node_modules/@fontsource-variable/noto-serif-hebrew/files/noto-serif-hebrew-hebrew-wght-normal.woff2'
	)
);

/**
 * The icons: their size, file, the font's weight and the mark's share of the
 * tile. The menu's mark is weight 200, the icons' own line; at 32 pixels that
 * line would fall under a pixel and vanish, so the tab's icon is drawn
 * heavier and larger in its tile — the same letters at their size, as type
 * is.
 */
const ICONS = [
	{ size: 32, file: 'favicon-32.png', weight: 600, scale: 0.9 },
	{ size: 180, file: 'apple-touch-icon.png', weight: 300, scale: 0.62 }
];

/**
 * The icon inverts the menu's mark: a white tile, and the two nuns and the
 * point in black — the owner's choice, so it stands out in a tab.
 */
const TILE = '#fff';
const INK = '#000';

const page = (size, weight, scale) => `<!doctype html><meta charset="utf-8">
<style>
@font-face { font-family: Mark; src: url(/font.woff2) format('woff2'); font-weight: 100 900; }
html, body { margin: 0; background: transparent; }
.tile { width: ${size}px; height: ${size}px; border-radius: ${size * 0.22}px; background: ${TILE};
        display: flex; align-items: center; justify-content: center; }
.mark { display: inline-flex; align-items: center; gap: 0.1em; font: ${weight} ${size * scale}px Mark;
        line-height: 1; color: ${INK}; direction: ltr; }
.mirror { display: inline-block; transform: scaleX(-1); }
.point { width: 0.15em; height: 0.15em; border-radius: 50%; background: ${INK}; flex: none; }
</style>
<div class="tile"><span class="mark"><span class="mirror">נ</span><span class="point"></span><span>נ</span></span></div>`;

const server = createServer((req, res) => {
	if (req.url === '/font.woff2')
		return res.writeHead(200, { 'content-type': 'font/woff2' }).end(font);
	const { size, weight, scale } = ICONS.find((i) => req.url === `/${i.size}`) ?? ICONS[0];
	res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(page(size, weight, scale));
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const site = `http://127.0.0.1:${server.address().port}`;

const profile = mkdtempSync(join(tmpdir(), 'ethaor-favicon-'));
const browser = spawn(chrome, [
	'--headless=new',
	'--remote-debugging-port=0',
	`--user-data-dir=${profile}`,
	'--no-first-run',
	'about:blank'
]);
try {
	const port = await new Promise((ok, fail) => {
		let err = '';
		browser.stderr.on('data', (d) => {
			err += d;
			const m = err.match(/DevTools listening on ws:\/\/[^:]+:(\d+)\//);
			if (m) ok(Number(m[1]));
		});
		browser.on('exit', () => fail(new Error('chrome exited')));
	});
	const target = await (
		await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
	).json();
	const ws = new WebSocket(target.webSocketDebuggerUrl);
	await new Promise((ok) => ws.addEventListener('open', ok, { once: true }));
	let id = 0;
	const waiting = new Map();
	ws.addEventListener('message', (event) => {
		const msg = JSON.parse(event.data);
		if (msg.id && waiting.has(msg.id)) waiting.get(msg.id)(msg);
	});
	const send = (method, params = {}) =>
		new Promise((ok) => {
			const n = ++id;
			waiting.set(n, ok);
			ws.send(JSON.stringify({ id: n, method, params }));
		});

	await send('Page.enable');
	await send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 0, g: 0, b: 0, a: 0 } });
	for (const { size, file } of ICONS) {
		await send('Page.navigate', { url: `${site}/${size}` });
		// The letters must be the font's, never a fallback's.
		const ready = await send('Runtime.evaluate', {
			expression: `document.fonts.load('200 20px Mark', 'נ').then(() => document.fonts.ready).then(() => document.fonts.check('20px Mark', 'נ'))`,
			awaitPromise: true,
			returnByValue: true
		});
		if (ready.result?.result?.value !== true) throw new Error('the Hebrew font did not load');
		const shot = await send('Page.captureScreenshot', {
			format: 'png',
			clip: { x: 0, y: 0, width: size, height: size, scale: 1 }
		});
		writeFileSync(join(web, 'static', file), Buffer.from(shot.result.data, 'base64'));
		console.log(`${file} (${size} px)`);
	}
	ws.close();
} finally {
	browser.kill();
	server.close();
	try {
		rmSync(profile, { recursive: true, force: true });
	} catch {
		// Chrome may still hold its profile for a moment; the system cleans it.
	}
}
