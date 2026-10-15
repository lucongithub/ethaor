// Times the page as a visitor gets it: Chrome on a throttled network, cache
// empty, reading two marks the page sets — `ethaor:map` (first image) and
// `ethaor:text` (verses landed).
//
// It serves a built site itself, as a CDN does: over HTTP/2 — one connection,
// every request in parallel; over HTTP/1.1 six connections queue a page's
// dozen files and the map came seconds later than any real visitor sees —
// and compressing on the fly (brotli, `BROTLI` quality, 5 by default). The
// certificate is a throwaway one, made with `openssl`, which Chrome is told to
// accept. Chrome is driven through its debugging protocol.
//
//   node scripts/bench-load.mjs [build dir] [runs]
//
// CHROME overrides the browser path; CPU slows the processor down by that
// factor (DevTools' emulation: 4 is a mid-range phone).
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createSecureServer } from 'node:http2';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import zlib from 'node:zlib';

const root = resolve(process.argv[2] ?? 'build');
const runs = Number(process.argv[3] ?? 3);
const quality = Number(process.env.BROTLI ?? 5);
const cpu = Number(process.env.CPU ?? 1);
// DIVE=n: as soon as the map is drawn, a visitor in a hurry turns the wheel n
// notches at the centre, one every 150 ms, and the bench times the first verse
// drawn (`ethaor:verse`) — what the visitor waits for to read.
const dive = Number(process.env.DIVE ?? 0);
// WAIT=mark times that mark instead — `near`, the book nearest to the view.
const wait = process.env.WAIT;
const chrome = process.env.CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';

// DevTools' presets, in bytes per second.
const NETWORKS = [
	{ name: '3G', latency: 300, down: 780_000 / 8 },
	{ name: '4G lente', latency: 150, down: 1_600_000 / 8 },
	{ name: '4G', latency: 60, down: 9_000_000 / 8 },
	{ name: 'fibre', latency: 10, down: 100_000_000 / 8 }
];

const TYPES = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript',
	'.css': 'text/css',
	'.json': 'application/json',
	'.wasm': 'application/wasm',
	'.woff2': 'font/woff2',
	'.ttf': 'font/ttf',
	'.bin': 'application/octet-stream'
};
// What a host compresses: text-like types and wasm. Fonts and binaries are
// sent as they are, already compressed.
const COMPRESSIBLE = new Set(['.html', '.js', '.css', '.json', '.wasm', '.ttf']);

// A throwaway certificate for 127.0.0.1.
const certs = mkdtempSync(join(tmpdir(), 'ethaor-cert-'));
execFileSync(
	'openssl',
	[
		'req',
		'-x509',
		'-newkey',
		'rsa:2048',
		'-nodes',
		'-days',
		'1',
		'-subj',
		'/CN=127.0.0.1',
		'-keyout',
		join(certs, 'key.pem'),
		'-out',
		join(certs, 'cert.pem')
	],
	{ stdio: 'ignore' }
);

const cache = new Map();
const server = createSecureServer(
	{
		key: readFileSync(join(certs, 'key.pem')),
		cert: readFileSync(join(certs, 'cert.pem')),
		allowHTTP1: true
	},
	(req, res) => {
		let path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
		if (path.endsWith('/')) path += 'index.html';
		const file = join(root, path);
		let body;
		try {
			if (!statSync(file).isFile()) throw new Error();
			body = readFileSync(file);
		} catch {
			res.writeHead(404).end();
			return;
		}
		const ext = extname(file);
		const headers = {
			'content-type': TYPES[ext] ?? 'application/octet-stream',
			'cache-control': 'no-store'
		};
		if (COMPRESSIBLE.has(ext) && /\bbr\b/.test(req.headers['accept-encoding'] ?? '')) {
			if (!cache.has(file)) {
				cache.set(
					file,
					zlib.brotliCompressSync(body, {
						params: { [zlib.constants.BROTLI_PARAM_QUALITY]: quality }
					})
				);
			}
			body = cache.get(file);
			headers['content-encoding'] = 'br';
		}
		// Range requests, as any static host answers them — what tiles ask for.
		const asked = /^bytes=(\d+)-(\d+)$/.exec(req.headers.range ?? '');
		if (asked && !headers['content-encoding']) {
			const [from, to] = [Number(asked[1]), Math.min(Number(asked[2]), body.length - 1)];
			headers['content-range'] = `bytes ${from}-${to}/${body.length}`;
			headers['content-length'] = to - from + 1;
			res.writeHead(206, headers).end(body.subarray(from, to + 1));
			return;
		}
		headers['content-length'] = body.length;
		res.writeHead(200, headers).end(body);
	}
);
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const site = `https://127.0.0.1:${server.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'ethaor-bench-'));
const browser = spawn(chrome, [
	'--headless=new',
	'--remote-debugging-port=0',
	`--user-data-dir=${profile}`,
	'--no-first-run',
	'--ignore-certificate-errors',
	'--window-size=1536,730',
	'about:blank'
]);
const port = await new Promise((ok, fail) => {
	let err = '';
	browser.stderr.on('data', (d) => {
		err += d;
		const m = err.match(/DevTools listening on ws:\/\/[^:]+:(\d+)\//);
		if (m) ok(Number(m[1]));
	});
	browser.on('exit', () => fail(new Error('chrome est sorti')));
});

/** A page target and a tiny client of the debugging protocol over it. */
async function page() {
	const target = await (
		await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
	).json();
	const ws = new WebSocket(target.webSocketDebuggerUrl);
	await new Promise((ok) => ws.addEventListener('open', ok, { once: true }));
	let id = 0;
	const waiting = new Map();
	ws.addEventListener('message', (event) => {
		const msg = JSON.parse(event.data);
		if (msg.id && waiting.has(msg.id)) {
			waiting.get(msg.id)(msg);
			waiting.delete(msg.id);
		}
	});
	const send = (method, params = {}) =>
		new Promise((ok) => {
			const n = ++id;
			waiting.set(n, ok);
			ws.send(JSON.stringify({ id: n, method, params }));
		});
	return { send, close: () => ws.close(), id: target.id };
}

async function once(network) {
	const p = await page();
	await p.send('Network.enable');
	await p.send('Network.setCacheDisabled', { cacheDisabled: true });
	await p.send('Network.emulateNetworkConditions', {
		offline: false,
		latency: network.latency,
		downloadThroughput: network.down,
		uploadThroughput: network.down
	});
	await p.send('Emulation.setCPUThrottlingRate', { rate: cpu });
	await p.send('Page.enable');
	await p.send('Page.navigate', { url: site });
	const begin = Date.now();
	let marks = {};
	let dived = dive === 0;
	const wanted = wait ? `ethaor:${wait}` : dive ? 'ethaor:verse' : 'ethaor:text';
	while (Date.now() - begin < 120_000) {
		const r = await p.send('Runtime.evaluate', {
			expression: `JSON.stringify(Object.fromEntries(performance.getEntriesByType('mark').map(m => [m.name, m.startTime])))`,
			returnByValue: true
		});
		marks = JSON.parse(r.result?.result?.value ?? '{}');
		if (!dived && marks['ethaor:map'] !== undefined) {
			dived = true;
			for (let i = 0; i < dive; i++) {
				await p.send('Input.dispatchMouseEvent', {
					type: 'mouseWheel',
					x: 768,
					y: 400,
					deltaX: 0,
					deltaY: -100
				});
				await new Promise((ok) => setTimeout(ok, 150));
			}
		}
		if (marks['ethaor:map'] !== undefined && marks[wanted] !== undefined) break;
		await new Promise((ok) => setTimeout(ok, 50));
	}
	p.close();
	await fetch(`http://127.0.0.1:${port}/json/close/${p.id}`);
	return [marks['ethaor:map'] ?? NaN, marks[wanted] ?? NaN];
}

const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1];
console.log(
	`site ${root}, brotli ${quality}, processeur /${cpu}, ` +
		`${dive ? `plongee de ${dive} crans, ` : ''}${runs} essais par reseau (mediane)`
);
console.log(`reseau      carte (ms)  ${(wait ?? (dive ? 'verse' : 'text')).padStart(6)} (ms)`);
for (const network of NETWORKS) {
	const map = [];
	const text = [];
	for (let i = 0; i < runs; i++) {
		const [m, t] = await once(network);
		map.push(m);
		text.push(t);
	}
	console.log(
		`${network.name.padEnd(10)} ${median(map).toFixed(0).padStart(10)} ${median(text).toFixed(0).padStart(12)}`
	);
}

browser.kill();
server.close();
try {
	rmSync(profile, { recursive: true, force: true });
	rmSync(certs, { recursive: true, force: true });
} catch {
	// Chrome may still hold a file: the temporary folder is left behind.
}
