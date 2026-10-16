// Writes TESTS.md at the repository root: every test of the repository,
// laid out along its file tree — what feature each file covers, which
// module, every test with its nature and what it holds — then every
// function no test names, the list of what to strengthen next.
//
//   node scripts/tests-map.mjs           write TESTS.md
//   node scripts/tests-map.mjs --check   fail if TESTS.md is stale, or if a
//                                        test the runners know is missing
//
// Exhaustive by construction: in --check, the tests read from the sources
// are compared with what `cargo test -- --list` and `vitest list` report;
// one missing on either side fails. Deterministic: same sources, same file.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const OUT = join(root, 'TESTS.md');
const check = process.argv.includes('--check');

const files = execFileSync('git', ['ls-files', '-co', '--exclude-standard'], {
	cwd: root,
	encoding: 'utf8'
})
	.split('\n')
	.filter(
		(f) => f && !f.startsWith('archive/') && !f.includes('node_modules') && !f.startsWith('target/')
	)
	.sort();
const read = (f) => readFileSync(join(root, f), 'utf8').replace(/\r\n/g, '\n');

// ------------------------------------------------------------------- rust

/** The crate a Rust file belongs to, and its module path inside it. */
function rustPlace(file) {
	const strip = (p) =>
		p
			.replace(/\.rs$/, '')
			.replace(/\/mod$/, '')
			.split('/')
			.filter(Boolean);
	if (file.startsWith('server/src/')) {
		const path = strip(file.slice('server/src/'.length));
		return { crate: 'ethaor-server', target: 'server', path: path[0] === 'lib' ? [] : path };
	}
	if (file.startsWith('tests/')) return { crate: 'ethaor', target: 'builder', path: [] };
	if (file.startsWith('src/')) {
		const path = strip(file.slice('src/'.length));
		return { crate: 'ethaor', target: 'lib', path: path[0] === 'lib' ? [] : path };
	}
	return null;
}

/** Removes strings, chars and comments, so braces can be counted. */
function bare(line) {
	return line
		.replace(/\/\/.*$/, '')
		.replace(/"(?:[^"\\]|\\.)*"/g, '""')
		.replace(/'(?:[^'\\]|\\.)'/g, "''");
}

/** The first paragraph of a doc comment, as one line. */
function paragraph(lines) {
	const text = [];
	for (const l of lines) {
		if (!l.trim()) {
			if (text.length) break;
			continue;
		}
		text.push(l.trim());
	}
	return text.join(' ');
}

/** `//!` doc of a file: what it is for. */
function fileDoc(source) {
	const lines = [];
	for (const l of source.split('\n')) {
		const m = l.match(/^\s*\/\/!\s?(.*)$/);
		if (m) lines.push(m[1]);
		else if (lines.length || l.trim()) break;
	}
	return paragraph(lines);
}

function rustTests(file) {
	const source = read(file);
	if (!/#\[(tokio::)?test/.test(source)) return [];
	const place = rustPlace(file);
	const lines = source.split('\n');
	const stack = []; // inline modules: { name, depth }
	let depth = 0;
	let doc = [];
	let attrs = [];
	const tests = [];
	for (const line of lines) {
		const t = line.trim();
		const docLine = t.match(/^\/\/\/\s?(.*)$/);
		if (docLine) {
			doc.push(docLine[1]);
			continue;
		}
		if (t.startsWith('#[')) {
			attrs.push(t);
			continue;
		}
		const mod = t.match(/^(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*\{/);
		const fn = t.match(/^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)/);
		if (fn && attrs.some((a) => /^#\[(tokio::)?test\b/.test(a))) {
			const expected = attrs.join(' ').match(/should_panic\(expected\s*=\s*"([^"]*)"/);
			tests.push({
				name: fn[1],
				path: [...place.path, ...stack.map((m) => m.name), fn[1]].join('::'),
				doc: paragraph(doc),
				ignored: attrs.some((a) => a.startsWith('#[ignore')),
				panics: attrs.some((a) => a.includes('should_panic')) ? (expected?.[1] ?? '') : null,
				async: attrs.some((a) => a.startsWith('#[tokio::test')),
				threads: attrs.some((a) => a.includes('multi_thread'))
			});
		}
		if (mod) stack.push({ name: mod[1], depth });
		for (const c of bare(line)) {
			if (c === '{') depth++;
			if (c === '}') {
				depth--;
				if (stack.length && depth === stack.at(-1).depth) stack.pop();
			}
		}
		if (t && !t.startsWith('//')) {
			doc = [];
			attrs = [];
		}
	}
	return tests.map((test) => ({ ...test, target: place.target, crate: place.crate }));
}

// --------------------------------------------------------------------- ts

function tsTests(file) {
	const source = read(file);
	const lines = source.split('\n');
	const tests = [];
	const describes = []; // { name, indent }
	let doc = [];
	for (const line of lines) {
		const indent = line.match(/^\t*/)[0].length;
		const t = line.trim();
		while (describes.length && indent <= describes.at(-1).indent && t.startsWith('})')) {
			describes.pop();
			break;
		}
		const block =
			t.match(/^\/\*\*?\s?(.*?)\s*\*?\/?$/) && (t.startsWith('/**') || t.startsWith('*'));
		if (t.startsWith('//')) {
			doc.push(t.replace(/^\/\/\s?/, ''));
			continue;
		}
		if (block || t === '*/') {
			doc.push(t.replace(/^\/\*\*?|\*\/$|^\*/g, '').trim());
			continue;
		}
		const d = t.match(/^describe\((['"`])(.*?)\1/);
		if (d) {
			describes.push({ name: d[2], indent });
			doc = [];
			continue;
		}
		const it = t.match(/^(it|test)(\.skip|\.todo)?\((['"`])(.*?)\3/);
		if (it) {
			const name = [...describes.map((x) => x.name), it[4]].join(' > ');
			// A vitest name is a sentence already: it is the doc when no
			// comment says more.
			const said = paragraph(doc) || `${it[4]}.`;
			tests.push({ name: it[4], path: name, doc: said, ignored: Boolean(it[2]), target: 'web' });
		}
		if (t) doc = [];
	}
	return tests;
}

// ------------------------------------------------------------------ nature

function nature(file, test) {
	const tags = [];
	if (file.startsWith('web/')) tags.push('page logic, on the real artifacts');
	else if (file.startsWith('server/')) tags.push('service, called in memory');
	else if (file.startsWith('tests/')) tags.push('build: corpus reading');
	else if (file.startsWith('src/tests/')) tags.push('app: the module as the page drives it');
	else tags.push('unit, module-private');
	if (/parity/.test(file) || /parity|the_page_and_the_api/.test(test.name))
		tags.push('parity WebAssembly ↔ API');
	if (test.panics !== null && test.panics !== undefined)
		tags.push(`must panic${test.panics ? `: "${test.panics}"` : ''}`);
	if (test.ignored) tags.push('ignored: a measurement, run by hand');
	if (test.threads) tags.push('multi-thread');
	else if (test.async) tags.push('async');
	if (/blink|budget|in_time|fast/.test(test.name)) tags.push('time budget');
	return tags.join(' · ');
}

// ---------------------------------------------------------- what is tested

/** The source file a test file covers, and that file's purpose. */
function covered(file) {
	const pairs = [
		[/^src\/(.+)\/tests\.rs$/, (m) => [`src/${m[1]}.rs`]],
		[/^src\/tests\/(.+)\.rs$/, (m) => [`src/map/${m[1]}.rs`, `src/wasm/${m[1]}.rs`]],
		[
			/^server\/src\/tests\/(.+)\.rs$/,
			(m) => [
				`server/src/api/${m[1]}.rs`,
				`server/src/${m[1]}.rs`,
				`server/src/${m[1].replace(/s$/, '')}.rs`
			]
		],
		[/^web\/src\/lib\/(.+)\.test\.ts$/, (m) => [`web/src/lib/${m[1]}.ts`]]
	];
	for (const [rx, to] of pairs) {
		const m = file.match(rx);
		if (m) return to(m).find((target) => files.includes(target)) ?? null;
	}
	return null;
}

// ------------------------------------------------------------- the gather

const testFiles = files.filter((f) => /\.rs$/.test(f) || /\.test\.ts$/.test(f));
const all = [];
for (const file of testFiles) {
	const tests = file.endsWith('.rs') ? rustTests(file) : tsTests(file);
	if (tests.length) all.push({ file, doc: fileDoc(read(file)) || docOfTs(read(file)), tests });
}

function docOfTs(source) {
	const m = source.match(/^\/\*\*([\s\S]*?)\*\//);
	return m ? paragraph(m[1].split('\n').map((l) => l.replace(/^\s*\*\s?/, ''))) : '';
}

// Functions no test names: every function of the sources, looked up in the
// text of every test file.
// The text of the tests only — never the file that defines the function.
const testText = files
	.filter((f) => all.some((x) => x.file === f) || /^(src|server\/src)\/tests(\.rs|\/)/.test(f))
	.map(read)
	.join('\n');
const sourceFiles = files.filter(
	(f) =>
		((/^(src|server\/src|build)\/.*\.rs$/.test(f) || f === 'build.rs') &&
			!/\/tests?(\.rs|\/)/.test(f) &&
			!f.endsWith('/tests.rs')) ||
		(/^web\/src\/lib\/[^/]+\.ts$/.test(f) && !f.endsWith('.test.ts'))
);
const unnamed = [];
for (const f of sourceFiles) {
	const source = read(f);
	const names = new Set();
	const rx = f.endsWith('.rs')
		? /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"C"\s+)?fn\s+(\w+)/gm
		: /^export\s+(?:async\s+)?(?:function|class)\s+(\w+)/gm;
	for (const m of source.matchAll(rx)) names.add(m[1]);
	const missing = [...names].filter(
		(n) => n !== 'main' && n !== 'new' && !new RegExp(`\\b${n}\\b`).test(testText)
	);
	if (missing.length) unnamed.push({ file: f, missing: missing.sort() });
}

// ------------------------------------------------------------------ write

function tree() {
	const out = [];
	let opened = [];
	for (const { file, doc, tests } of all) {
		const dirs = file.split('/').slice(0, -1);
		let same = 0;
		while (same < opened.length && same < dirs.length && opened[same] === dirs[same]) same++;
		for (let k = same; k < dirs.length; k++) out.push(`${'  '.repeat(k)}- **${dirs[k]}/**`);
		opened = dirs;
		const pad = '  '.repeat(dirs.length);
		const target = covered(file);
		const counts = `${tests.length} test${tests.length > 1 ? 's' : ''}`;
		out.push(
			`${pad}- [\`${file.split('/').at(-1)}\`](${file}) — ${counts}${target ? ` — covers [\`${target}\`](${target})` : ''}`
		);
		if (doc) out.push(`${pad}  <br>${doc}`);
		const modules = new Map();
		for (const t of tests) {
			const module =
				t.target === 'web'
					? t.path.split(' > ').slice(0, -1).join(' > ')
					: t.path.split('::').slice(0, -1).join('::');
			if (!modules.has(module)) modules.set(module, []);
			modules.get(module).push(t);
		}
		for (const [module, list] of modules) {
			const label = module || (tests[0].target === 'web' ? 'top level' : 'crate root');
			const crate = list[0].crate
				? `${list[0].crate}${list[0].target === 'builder' ? ' (tests/builder)' : ''}`
				: 'vitest';
			out.push(`${pad}  - module \`${label}\` · ${crate}`);
			for (const t of list) {
				const what = t.doc ? ` — ${t.doc}` : ' — *(no doc: say what it holds)*';
				out.push(`${pad}    - \`${t.name}\`${what} <sub>${nature(file, t)}</sub>`);
			}
		}
	}
	return out.join('\n');
}

const count = (pred) => all.flatMap((f) => f.tests).filter(pred).length;
const areas = [
	['Rust core, module-private', (f) => /^src\//.test(f) && !/^src\/tests\//.test(f)],
	['Rust app (`src/tests/`)', (f) => /^src\/tests\//.test(f)],
	['Corpus build (`tests/`)', (f) => /^tests\//.test(f)],
	['Server (`server/`)', (f) => /^server\//.test(f)],
	['Page (`web/`)', (f) => /^web\//.test(f)]
];
const summary = areas.map(([label, pred]) => {
	const list = all.filter((x) => pred(x.file));
	const tests = list.flatMap((x) => x.tests);
	return `| ${label} | ${list.length} | ${tests.length} | ${tests.filter((t) => t.ignored).length} | ${tests.filter((t) => !t.doc).length} |`;
});
const total = all.flatMap((f) => f.tests);

const text = `# Tests

Every test of the repository, along its file tree. **Generated** by
\`web/scripts/tests-map.mjs\` (\`npm run tests:map\`) — do not edit by hand:
\`npm run verify\` fails when this file is stale, or when a test the runners
know is missing here. The \`tests\` skill (\`.claude/skills/tests\`) reads it to
find what to strengthen next.

For each test file: what it covers, then each module and each test — its
name, what it holds (its doc comment), and its nature.

## Summary

| area | files | tests | ignored | without a doc |
|---|---|---|---|---|
${summary.join('\n')}
| **total** | **${all.length}** | **${total.length}** | **${count((t) => t.ignored)}** | **${count((t) => !t.doc)}** |

## By file

${tree()}

## Functions no test names

Each function of the sources whose name appears in no test file: tested only
through others, if at all. Not all need a test of their own — a private
helper is often held by the test of its caller — but each is a question the
\`tests\` skill answers: held elsewhere, or a gap.

${unnamed.map(({ file, missing }) => `- [\`${file}\`](${file}): ${missing.map((n) => `\`${n}\``).join(', ')}`).join('\n')}
`;

// ------------------------------------------------------------------ check

function listed() {
	const cargo = (args) =>
		execFileSync('cargo', ['test', '-q', ...args, '--', '--list'], {
			cwd: root,
			encoding: 'utf8',
			stdio: ['ignore', 'pipe', 'ignore']
		})
			.split('\n')
			.filter((l) => / test$/.test(l) || /: test$/.test(l))
			.map((l) => l.replace(/: test$/, ''));
	const vitest = JSON.parse(
		execFileSync('npx', ['vitest', 'list', '--json'], {
			cwd: join(root, 'web'),
			encoding: 'utf8',
			shell: true,
			stdio: ['ignore', 'pipe', 'ignore']
		})
	);
	return {
		lib: cargo(['-p', 'ethaor', '--lib']),
		builder: cargo(['-p', 'ethaor', '--test', 'builder']),
		server: cargo(['-p', 'ethaor-server', '--lib']),
		web: vitest.map((t) => t.name)
	};
}

if (check) {
	const problems = [];
	const runners = listed();
	for (const target of ['lib', 'builder', 'server', 'web']) {
		const ours = new Set(total.filter((t) => t.target === target).map((t) => t.path));
		const theirs = new Set(runners[target]);
		for (const t of theirs)
			if (!ours.has(t)) problems.push(`${target}: ${t} is run but not mapped`);
		for (const t of ours)
			if (!theirs.has(t)) problems.push(`${target}: ${t} is mapped but not run`);
	}
	let current = '';
	try {
		current = read('TESTS.md');
	} catch {
		// No file yet: stale.
	}
	if (current !== text) problems.push('TESTS.md is stale: run `npm run tests:map`');
	if (problems.length) {
		console.error(problems.join('\n'));
		process.exit(1);
	}
	console.log(
		`TESTS.md is up to date: ${total.length} tests in ${all.length} files, every one mapped`
	);
} else {
	writeFileSync(OUT, text);
	console.log(
		`TESTS.md: ${total.length} tests in ${all.length} files, ${count((t) => !t.doc)} without a doc`
	);
}
