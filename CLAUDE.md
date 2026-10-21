# ethaor

Zoomable map of the Tanakh, no background. **Rust compiles to WebAssembly and
paints into a page.** Single criterion: speed.

Display reference: https://github.com/Peeter95/code-map — a box's size follows
its volume, and zooming reveals its contents inside it.

## How this file works

**Rules here, numbers in tests.** This file states invariants, intent and
rejected alternatives — the things a test cannot express. Every measurement,
ratio, count and threshold lives in `cargo test` (361 tests, 7 s), which is the
only place they stay true. Do not restate a measured value here; if a rule can
be asserted, assert it and leave only the *why* in prose.

When you add a rule: synthesize. One line per invariant, reason attached.
If a test already enforces it, the test is the documentation — the sole rule
that must stay written is **"no calculation without a test"** (see *Tests*),
because that is what keeps the rest verifiable long-term.

## Work split

**All computation is in Rust**: corpus, layout, camera, zoom folding, pruning,
hit testing. The module emits a command list; the page only paints it.

**Two things stay in the browser** — on the map — for one reason: pointed-Hebrew shaping
(bidi + combining marks) lives there and nothing else does it as well.

1. measure the widths Rust asks for;
2. draw the text.

Rust draws first with an estimate, records unknown fragments in `pending`, the
page measures them and asks for a new frame. Each fragment is measured once;
convergence takes one frame.

**The page needs no server**: a zoom gesture must answer under 16 ms; a round
trip, even local, is 20–100 ms. The browser never calls `server/` — its API
is for whoever has no browser: scripts, other apps, agents (see *WebAssembly
and API*).

**No three.js / WebGL**: it would force a glyph atlas and a home-made text
engine — no bidi, no combining marks, so the corpus' million niqqud points
would not render. Canvas 2D gives browser shaping for free.

**One exception: the book view** (see *Book view*). There the browser lays
the text out — wraps, aligns, selects, finds — as on any web page; the
module still gives what is laid (a chapter's record), and where the reader
is hands over between the two views through it.

## Structure

Same layout as check8ge: Rust crate at the root, SvelteKit app in `web/`.

```
Cargo.toml, build.rs, src/      WebAssembly module — all computation; also
                                the workspace root (`server/` is a member)
src/shared.rs, src/shared/      what both transports run: queries checked,
                                answers paged, places named (OSIS, English)
src/map.rs, src/map/            the map: `App`, one window's state, one file
                                per subject — camera, measure, text (verse
                                layout), hit (click, cursor, breadcrumb),
                                landing, frame, settings
src/wasm.rs, src/wasm/          WebAssembly exports only, no computation: els,
                                map, chapter (the book view's records),
                                landing (taken by both modules), fault
                                (what a panic leaves for the page)
build/mamre.rs                  source reading, included by build.rs
build/variants.rs               the other edition's readings, same inclusion
build/variants.tsv              their table, measured and versioned
src/pack.rs                     the corpus packed for transport, unpacked here
src/{corpus,els,letter,pack,treemap,variant}/tests.rs   module-private tests
src/tests/                      app tests: camera, frame, click, breadcrumb, …
tests/builder.rs                corpus build tests
TESTS.md                        every test along the file tree, generated
                                (`npm run tests:map`), checked by `verify`
server/                         axum: the static build, and the HTTP API
  src/api.rs                    `GET /api`: every endpoint as a tool (MCP-ready)
  src/api/<name>.rs             the HTTP twin of src/wasm/<name>.rs
  src/api/pool.rs               engines lent one per search, panics caught
  src/api/guard.rs              each client's budget, charged per request
  src/api/logs.rs               `GET /api/logs`: the logs, live, for the operator
  src/api/report.rs             `POST /api/report`: the page's faults, into the log
  src/fault.rs                  faults: incident, code, place, escalation, answer
  src/config.rs                 every setting, from the environment, defaulted
  src/logs.rs                   structured logs: JSON, off-thread, to rolling
                                files, read back and followed from them
  src/files.rs                  web/build/ as a CDN serves it
  src/tests/                    the service called in memory, parity with wasm
web/                            SvelteKit 2 + Svelte 5 + Tailwind 4
  src/lib/ethaor.ts             bridge: gestures, painting, measurements
  src/lib/components/TanakhMap.svelte   the page: sidebar, map, book view
  src/lib/components/Rail.svelte        the sidebar: a rail of icons, never opened
  src/lib/components/Logo.svelte        the mark: two nuns and a point, as `[·]`
  src/lib/components/BookView.svelte    the Tanakh read by scrolling
  src/lib/book-view.ts          that view's logic: records read, heights, zoom
  src/routes/+page.svelte
  src/routes/logs/              the operator's logs, live (`/logs/?token=…`)
  src/lib/logs-view.ts          that page's logic: token, events, bounds
  src/lib/ethaor.wasm           the map module, built and copied by `npm run wasm`
  src/lib/ethaor-search.wasm    the search module, likewise, loaded by the worker
  src/lib/ethaor-text.*         the corpus' text part: books gzipped (+ where they
                                lie, JSON), and plain for tiles
  src/lib/search-worker.ts      the search module off the frame loop
  src/lib/search-core.ts        the search module: land the text, search
  src/lib/fault.ts              the page's faults: numbered, escalated, reported
  src/lib/keys.ts               the keyboard: which key does what, Escape's order
  src/lib/text-open.ts          Range requests, gzip opened natively, the lanes
  src/lib/*.test.ts             the page's own logic, on the real artifacts
  src/lib/tests/artifacts.ts    those artifacts read from disk, for the tests
  static/fonts/Baloo2-Regular.woff   UI only, Latin subset (see Fonts)
  static/_headers               cache rules for the host (Cloudflare, Netlify)
  scripts/copy-wasm.mjs
  scripts/favicon.mjs           the tab's and home screen's icons, drawn from the
                                mark by Chrome in the real font (static/*.png)
  scripts/preload.mjs           preload links in build/index.html (postbuild)
  scripts/bench-load.mjs        the page timed on throttled networks
  scripts/bench-els.mjs         ELS methods timed in V8 (see ELS)
  scripts/tune-els.mjs          ELS cost model tuned on those times
  scripts/tests-map.mjs         writes TESTS.md; `--check` holds it exhaustive
```

**`adapter-static`**: `build/` is plain files, served by any host or CDN.
The app has no server code — map and search both run in the visitor's
browser — so a Node process would only have sent files, and would have had
to be sized, kept up and patched for it; static, any number of visitors costs
the server one download each. Rejected: `adapter-node`, check8ge's choice,
which had nothing to run here. Server code goes in `server/`, a separate
crate: the build stays static, and `server/` serves it beside its API —
or a CDN serves it and `server/` keeps `/api` alone.
`ssr = false` — canvas and module have no business on the server.

CSS is Tailwind everywhere. `app.css` holds only the Tailwind import, the fonts
and two theme tokens (`--font-sans`, `--font-hebrew`).

## Build

```sh
cargo test         # 361 tests, 7 s, crate and server — before any commit
cargo run -p ethaor-server --release   # web/build + /api on 127.0.0.1:8080
                   # run from the repository root; settings from the
                   # environment, else from `.env` there (see Settings)
cd web
npm install
npm run wasm       # both modules (map, search) built and copied into src/lib/
npm run dev        # runs `wasm` first (predev), then 127.0.0.1:8095
npm run build      # runs `wasm` first (prebuild), then writes build/
npm run eth        # build, then ethaor-server --release from the root: the app as in
                   # production, page and /api on 127.0.0.1:8080, `.env` read
npm run eth:run    # the same without the page build: start again after Ctrl-C
npm run preview    # serves build/ on the dev port — to test, not to host
npm run host       # same, on 0.0.0.0:8080
npm run deploy     # build, then upload build/ to Cloudflare Pages (ethaor)
npm test           # the page's own logic (vitest), modules rebuilt first
npm run tests:map  # rewrite TESTS.md after adding or changing a test
npm run verify     # the whole gate: cargo test, clippy on all three builds,
                   # npm test, svelte-check, prettier, TESTS.md up to date
                   # and exhaustive — before any commit
```

`build` recompiles the module itself: Vite only takes the file it finds, so a
build after a Rust change would otherwise ship the previous module. The
modules and text parts in `web/src/lib/` are therefore **not versioned**
(`web/.gitignore`): every build makes them, and a corpus change would
otherwise add megabytes to the history. Building needs Rust.

**The module is imported, not served from `static/`** (`ethaor.wasm?url` in
`ethaor.ts`): Vite emits it under `_app/immutable/` with a hash of its
content in its name, like the scripts. So `_headers` can let a browser keep
everything there for a year, and a new build is never served stale — under a
fixed name, a browser would either ask again at every visit or keep an old
module. **Only `build/` is deployed**; `deploy` needs `wrangler login` once.
Elsewhere, `build/` goes behind any static server — `.wasm` must be served as
`application/wasm`, or the page cannot stream it.

The map is welded into the module; the text is one more file, fetched once
the first image is drawn (see *Transport*). No request afterwards.

## Settings

**Every setting is an environment variable, and a local `.env` may hold
them** (`server/src/config.rs`). The process' environment always wins —
a host's secrets override a local file — and the file is only read, never
written into the environment. It is `.env` in the directory the server
runs in (`ETHAOR_ENV` names another), **never versioned** (`.gitignore`):
it holds `ETHAOR_LOGS_TOKEN`. Missing, the environment alone; malformed, it
is ignored whole and the log says why — half a file is worse than none.

| variable | default | what |
|---|---|---|
| `ETHAOR_ADDR` | `127.0.0.1:8080` | where it listens |
| `ETHAOR_ROOT` | `web/build` | the static build it serves |
| `ETHAOR_LOG` | `info` | log filter, `RUST_LOG` syntax |
| `ETHAOR_LOG_FORMAT` | JSON | `pretty` for a terminal (standard output; files stay JSON) |
| `ETHAOR_LOG_DIR` | `logs` | where the log files go, `off` for none |
| `ETHAOR_LOG_MB` | 256 | the disk all log files take together |
| `ETHAOR_LOGS_TOKEN` | unset: off | bearer token of `GET /api/logs`, 16 characters at least |
| `ETHAOR_ENGINES` | one per core | searches at once |
| `ETHAOR_WAIT_MS` | 2000 | queueing for an engine before `busy` |
| `ETHAOR_CACHE_MB` | 256 | JSON answers kept |
| `ETHAOR_MAX_COST_MS` | 300 | heaviest search admitted, estimated |
| `ETHAOR_IP_BURST_MS`, `ETHAOR_IP_RATE_MS` | 3000, 300 | each client's budget, and per second back |
| `ETHAOR_TRUST_PROXY` | off | client named by the proxy's headers |
| `ETHAOR_VERBOSE` | off | faults log their whole escalation and stack |

A new setting goes in this table, in `Config`, in `env.md` (every value and its effect, for whoever sets them), and in a test that reads it.

## WebAssembly and API

**A WebAssembly module generally gets its HTTP twin.** What the page computes
in the browser, a script or an agent without a browser asks `server/` for:
`src/wasm/<name>.rs` and `server/src/api/<name>.rs` go in pairs, and a new
export comes with its endpoint unless there is a reason not to — the map's
frames, which only a canvas reads, have none.

- **The computation is written once, in `src/shared/`.** A transport moves
  bytes and nothing else: the module records across its memory, the API
  JSON. Checking a query, paging, naming a place — all in `shared`, so the
  two can never disagree. The engines themselves (`els::Engine`, `corpus`)
  stay in their modules; `shared` re-exports what a transport needs.
- **The page keeps its module.** Once the text is in, nothing beats a search
  with no round trip, and the page must not depend on a process. The API is
  for callers with no browser; it answers at once, the text being welded
  into the native build.
- **Both sides are tested, always**: `shared` in `src/shared/tests.rs`, the
  module's exports in `src/tests/api.rs`, the endpoint in
  `server/src/tests/`. And **a parity test** drives the export exactly as
  the page does and the endpoint as a client does, and asserts the same
  hits — the one guarantee that the twin is a twin.
- **The API's JSON is designed for a future MCP server**, so an agent can
  call it with no guide. `GET /api` describes each endpoint as an MCP tool:
  `name`, `description`, `input_schema` (a JSON Schema of its parameters,
  `additionalProperties: false`) and its error codes (`errors`) — what an MCP
  server declares, as is, so wrapping the API in MCP is a mapping, not a
  rewrite. Unknown parameters are refused, never ignored. A refusal
  is `{"error": {"code", "message"}}` with a stable code. A place is named
  every way at once: OSIS (`Gen.1.1`), English (`Genesis 1:1`), Hebrew
  (`בראשית א,א`), global index. Counts are exact (`total`), pages follow
  `next_offset`, `limit=0` only counts. The description is tested against
  what the endpoint really takes.
- **`server/` serves `web/build/` as a CDN would**: Range honoured (tiles),
  nothing compressed on the fly (the books are gzip already, and a
  compressed range is no longer the range asked), `_app/immutable/` cached a
  year as `_headers` says — and there a missing name is a 404, never the
  page, or a stale page would keep HTML for a script for a year.
- **Under `/api` every answer is JSON**: a wrong path is a `not_found`, a
  wrong method a `method_not_allowed` (with `Allow`), `/api/` describes as
  `/api` does — never the page, which an agent could not parse.
- **The API places the page it returns, nothing more** (`Engine::hits`):
  the engine keeps up to `CAP` hits, and placing all of them to hand back a
  hundred made a repeated query cost more than its recall. The module, which
  reads every record, still gets them all through `Engine::search`.
- **Built for any number of callers, and the number is not ours to
  guess.** One process scales up; replicas scale out. The service keeps no
  state worth sharing — the corpus is frozen into the binary — so any
  number of copies run behind a load balancer, and a CDN in front answers
  repeated queries without reaching any of them.
- **Fully async, never blocked** (`api/pool.rs`): a caller waits on a
  semaphore, not a thread; the search itself runs on the blocking pool,
  off the async threads. Rejected: one engine behind one lock — measured, a
  wide search held every light query behind it for more than a hundred
  milliseconds.
- **Engines are lent, not shared**: an `Engine` caches what it builds, so a
  search needs one alone. One per core (`ETHAOR_ENGINES`), built on first
  need, lent to one search at a time and taken back **inside** the blocking
  task — a caller gone mid-search does not take it along. Rejected: one
  index behind concurrent caches — it would have reached into the method
  code the page runs, for memory the pool does not spend (measured: the
  whole service stays under a hundred megabytes, every engine warm).
- **An answer is searched once for everyone** (`moka`): the corpus is
  frozen, so a JSON answer is the same forever. Kept by query, weighed in
  bytes (`ETHAOR_CACHE_MB`); the same query asked by many at once is
  searched once while the others await it. A refusal is never kept, and a
  bad query costs no engine.
- **A full queue turns callers away**: past `ETHAOR_WAIT_MS` waiting for an
  engine, a 503 `busy` with `Retry-After` — under a flood a quick refusal a
  client retries beats a queue that never drains.
- **Answers say they may be kept**: `Cache-Control: public` on every
  success (an hour in a browser, a day in a shared cache), `no-store` on
  every refusal.
- `server/src/tests/load.rs` times the service in memory against one engine
  with no cache, a pool, and a pool with its cache — run it after any change
  to the pool or the cache:
  `cargo test -p ethaor-server --release load -- --ignored --nocapture`.
- **A search may be kept to a part of the text**: `portion` (`torah`,
  `neviim`, `ketuvim`), `book` (`Gen`), `chapter` (`Gen.1`), or
  `from`/`to` (any book, chapter or verse, both ends included, either
  open) — one way at a time. An occurrence counts only if **every** letter
  lies there: the engine searches that stretch alone (`Engine::hits_in`),
  so the first `CAP` kept are the stretch's own, not the remains of the
  whole text's. The module's twin is `els_search_in`, over global verse
  indices.
- **A caller names a place in OSIS, and in OSIS alone** (`Gen`, `Gen.1`,
  `1Sam.3.4`; `shared::reference`): one spelling per book, case exact,
  numbers from 1. **Answers name it every way** — OSIS, English, Hebrew.
  Rejected, by the owner: English names, transliterations (`Bereshit`),
  abbreviations and a trigram per book as input — every extra spelling is
  one more to guess or to collide. `GET /api` declares the 39 codes (an
  enum, a pattern), so an agent needs no other list.
- **Many hits stream** (`GET /api/els/stream`): NDJSON, a head line, one
  line per hit up to `CAP`, a closing `{"done": true}`. The engine is free
  once the pairs are copied out; the placing runs apart and stops as soon
  as the caller goes. A page stays bounded by `MAX_LIMIT`.
- **A chapter reads as the book view reads it** (`GET /api/chapter?ref=`):
  every verse named every way, our text, and where the other edition reads
  otherwise the verse as shown with its forms apart; `previous` and `next`
  across books. No engine, no cache: a few lookups cost less than either.
- **The contents list every portion, book and chapter**
  (`GET /api/contents`, no parameter): each named every way, so an agent
  finds any chapter's OSIS without a guide. Twin of `contents_record`.
- **A WebAssembly build names `-p ethaor`**: the workspace's default members
  take the server, which has no WebAssembly build.

## Robustness: degraded, never down

**By design, nothing a caller sends takes the service down, and nothing
takes it down at all short of losing its port.** Under too much it serves
less — refuses, sheds, drops a log line — never nothing. Every one of these
is a rule with a test; a new feature states which of them it leans on.

- **A panic is a 500, not a crash.** `release` unwinds (`panic =
  "unwind"`); the WebAssembly modules alone abort, in their own `wasm`
  profile. `main.rs` refuses to compile with `panic = "abort"`. A handler's
  panic is caught (`CatchPanicLayer`) and answered `internal`; a search's
  is caught in its task, its engine dropped — half-written caches are never
  lent again — and built anew at the next need.
- **Every search is admitted on its cost before it runs**: the cost model
  that picks the method also estimates its time (`els::cost`, V8
  milliseconds); past `ETHAOR_MAX_COST_MS` a 400 `too_costly` says the
  estimate and how to narrow it. The heaviest query is bounded, so the
  worst a caller can do is known.
- **Every client has a budget** (`api/guard.rs`): in the same milliseconds,
  `ETHAOR_IP_BURST_MS` at once, `ETHAOR_IP_RATE_MS` earned back per
  second; a request costs one, a search the cache does not hold its
  estimate too. Past it, 429 `rate_limited` with `Retry-After`; another
  client is not held. The client is the socket's address, or the proxy's
  first one only with `ETHAOR_TRUST_PROXY` — else anyone picks a fresh
  budget.
- **Everything that grows is bounded**: answers by bytes, budgets by
  clients (and forgotten when idle), engines by cores, the queue by
  `ETHAOR_WAIT_MS` (then `busy`), the log files by `ETHAOR_LOG_MB` (then
  the oldest is deleted), the log output by its queue (then lines are
  dropped), what a log follower reads by a chunk at a time.
- **A bad setting falls back**: unreadable, negative or infinite, it takes
  its default and the log says so; a burst below the heaviest admitted
  search is raised to it. Only a port that cannot be bound stops the
  process.
- **Missing pieces degrade the service, not stop it**: no `web/build` and
  the API still answers; no `ETHAOR_LOGS_TOKEN` and `/api/logs` is off; a
  log directory that cannot be written and the logs go to standard output
  alone, `/api/logs` a 503. A log write that fails loses its line, never
  the request.
- **In the browser too**: a search the module fails on is answered by its
  id with the error — the page's promise rejects, never hangs — and the
  worker replaces the module, feeding it again the text it kept.
- **Stopping lets requests finish** (Ctrl-C, SIGTERM).

## Faults and incidents

**Every error is numbered, escalated, logged once and told** — at every
layer, Rust, server, page, worker, UI. The skill `.claude/skills/faults`
is the playbook, and **it applies to every change**: read it before
writing code that can fail.

- **An incident number** (`ETH-7K3F92QD`) is given where an error is raised
  and follows it to the user's message. Quoted, it finds the one log line.
- **A code names its place**: each is in a catalogue (`CODES` in
  `server/src/fault.rs`, `CATALOGUE` in `web/src/lib/fault.ts`) and raised
  at one place only — a test on each side holds it. **`at`** gives the
  `file:line` too: `#[track_caller]`, the JS stack, or the panic's own.
- **Each layer adds a step** (`escalate`, `relay`); the fault is logged once,
  where it is answered. Normal: incident, code, `at`, what the user read.
  **Verbose** (`ETHAOR_VERBOSE=1`, the page with `?verbose`): every step and
  the stack.
- **The user reads plain words and the number**: `internal` and a sentence
  for our faults (the precise code stays in the log), the API's code for a
  refusal; in the page, the catalogue's message, a closable notice,
  or — when the map cannot go on — the whole screen with a way back.
- **The page's faults reach the same log** (`POST /api/report`, bounded,
  charged to the client, never throwing in the page): one incident, one log,
  whichever side raised it. Hosted without the service, reports are lost
  and the page goes on.
- **A WebAssembly panic is traced to Rust**: the hook (`src/wasm/fault.rs`)
  leaves `file:line:col` and the message before the abort; the page reads
  them after the trap (`readTraps`). `probe_panic` gives the tests a real
  trap.
- **A panic in the server** is caught (handler: `CatchPanicLayer`; search:
  the pool) and traced the same way, by the hook `install_panic_hook`.

## Logs

**Professional logs are structured, correlated, cheap and followable.**

- **One JSON object per event** on standard output and in the log files:
  time, level, target, message, fields — no parsing needed by a log
  platform.
  `ETHAOR_LOG` filters (`RUST_LOG` syntax, `info` by default);
  `ETHAOR_LOG_FORMAT=pretty` for a terminal.
- **Every request has an id** (`x-request-id`, a UUID), carried by each of
  its lines — the search's included, though it runs on another thread —
  and one line once answered: method, path, status, milliseconds. 5xx are
  `error`, 429 and 503 `warn`.
- **Formatted once, written off-thread** (`tracing-appender`, lossy):
  written by the answering thread, measured, a slow console cut the
  throughput by most of itself; now a line waits for no output, and past
  `LINES` waiting it is dropped. `server/src/tests/load.rs` runs with the
  logs on, as in production.
- **Kept on disk, bounded** (`ETHAOR_LOG_DIR`, `ETHAOR_LOG_MB`): numbered
  files, a new one per start and per share of the budget written, the
  oldest deleted past `FILES`. Rejected: rotation by day — a flood fills a
  day's file without bound; by size the disk taken is known. Rejected: the
  history in memory — lost at each restart, and a second copy of what the
  files hold.
- **A file is written in batches** (`BufWriter`): the writing thread
  flushes once its queue is empty, so a line waits for no more than the
  lines queued with it, and a burst costs one system call, not one per
  line.
- **`GET /api/logs` reads the files** (server-sent events): the last
  `history` lines read back, then each new one as it lands, filtered by
  `level`; it starts after the last whole line, so none is lost or sent
  twice between the two. **Woken by the writer, not by a clock**: each
  flush says so (`WRITTEN`) and a waiting follower reads at once; it looks
  on its own only every `POLL`, the net under a word missed. Rejected:
  watching the directory (`notify`) — the only writer is this process, and
  a crate and the system's events to learn what it already knows. Off without
  `ETHAOR_LOGS_TOKEN` (16 characters at least), and then only with
  `Authorization: Bearer` — logs name clients by address. **`/logs/`** is
  the page that reads it (black, white, monospaced): the token comes once
  as `?token=`, is taken out of the address before anything else — the
  history and a bookmark keep `/logs/` alone — and is kept for the tab
  (`sessionStorage`); the stream is read with `fetch`, the token in the
  header, since an `EventSource` cannot send one. Each row has a number of
  its own (`rows`), so dropping the oldest moves no row kept — keyed by
  position, every row was rewritten for each line added — and a row off
  screen is neither laid out nor painted (`content-visibility`). The token is
  compared in constant time. A follower only reads: the service never
  waits for it; one so slow that files it had not read were deleted is
  told how many (`event: lagged`). It is listed in
  `GET /api` under `operator`, not among the tools: an agent has no use
  for it.

## Fonts

**Baloo 2 has no Hebrew glyph** (checked in its `cmap`: 0/27 consonants,
0/55 marks). UI only — a few Latin characters — so it ships as **one weight
subset to Latin, in woff**: the full TTFs carried all of Devanagari and were
most of what the first image waited for. Pointed Hebrew uses **Noto Serif
Hebrew** (`@fontsource-variable`), which covers niqqud and cantillation.

`ethaor.ts` awaits **the Hebrew font alone** before the first measurement: a
width taken on a fallback font would be cached and wrong for good. Not
`document.fonts.ready` — that waited for the interface font too, which the
canvas never uses; it only triggers a re-measure, should a font land late.

## Guiding principle

**Whatever can be computed at build time must not be computed at run time.**
The corpus is frozen: `build.rs` reads the HTML sources and builds text and
index. **One exception, and it is about the network**: what the module can
rebuild in milliseconds is not sent — downloading it costs more than
recomputing it (see *Transport*). `corpus.rs` does `O(1)` indexing once it is
unpacked.

## Transport (`pack.rs`)

The corpus travels **packed**: only what cannot be recomputed.

- **`build.rs` still writes the working form** (`text.bin`, `index.bin`,
  `variants.bin`) and `pack` reduces it; `unpack` rebuilds it **byte for
  byte**. The same file packs in the build (`#[path]`) and unpacks in the
  module, and `pack` itself asserts the round trip: a packing bug fails the
  build, it cannot ship. Every other module reads the unpacked form,
  unchanged.
- What is not sent: offsets (the strings' lengths give them back), chapter
  and verse labels (the numeral of the rank, plus the few that break it),
  the other edition's display strings (only the insertions travel), and the
  ELS letter stream (read off the text at first search — it weighed more
  than the text once both were compressed).
- Strings are **one byte per character**, through an alphabet of the few
  dozen the corpus uses; UTF-8 spends two on a Hebrew one.
- **Two parts.** The **map** (names, counts, label exceptions) is embedded in
  the module and is all the overview needs. The **text** (verses, readings)
  is a file of its own, hashed like the module.
- **Two modules from one crate** (Cargo features): `ethaor.wasm` (`map`)
  is what the page draws with — gestures, frames, tiles, the landing — and
  `ethaor-search.wasm` (`search`) the searches, ELS first and the ones to
  come. Each build exports its own set and link-time
  optimisation drops the rest, so the search's code is off the path of the
  first image; native builds and the tests take both. `copy-wasm.mjs`
  refuses a module exporting what is not its own.
- **The search module lives in a worker** (`search-worker.ts`), loaded once
  the first image is drawn, with a corpus of its own the page feeds book by
  book as the text lands: no search — up to a second over every skip — ever
  holds a frame. The page's `search` is therefore asynchronous, and waits
  for the text. Without a worker the same code (`search-core.ts`) runs in the page.
- **The text arrives as a map service's world does: where the visitor
  looks, first.** The overview — portions, books, chapter numbers — is all
  in the module. Once the first image is drawn, the text comes **book by
  book** (`ethaor-text.books.bin`, each book gzipped on its own, then the
  readings), always the book nearest to the centre of the view next
  (`next_book`), so it follows the visitor if they move; `BOOKS_IN_FLIGHT`
  travel together — one at a time paid a full round trip per book. Where
  the blocks lie comes with the page's code (`ethaor-text.books.json`): no
  request to learn it. Books are opened natively by the browser.
- **Gzip, not a codec of our own.** Rejected, after measuring: our own
  context-mixing codec. A
  fifth smaller than brotli, zstd or LZMA, it won seconds on slow links —
  but once tiles bring first the chapters the visitor dives into, the first
  readable verse came at the same time either way, and the codec only
  hastened the *whole* text, at the price of seconds of processor on a
  phone, a second text file and six hundred lines to keep. Rejected too:
  shipping brotli or zstd ourselves — the browser cannot decode them in
  script, so their decoder would ride in a module.
- **The text is fetched once the first image is drawn**, not alongside it:
  fetched together, it took bandwidth from what the first image waits for —
  a priority hint does not stop that across connections — and measured, the
  map came sooner and the text no later. Books go at low priority, tiles at
  high: a dive is never queued behind the background.
- **Verses land chapter by chapter, each straight into its final place**
  (`corpus::Landing`), in a buffer reserved at the corpus' full size and
  zero-filled — zeros are UTF-8, so it is a `str` at any time. Nothing
  moves, so every slice handed out stays valid, and the order of arrival
  does not matter. A chapter not landed reads as the empty string.
- **Tiles, as a map service serves them: the chapters the view dives into
  come first.** After each frame the module names the chapters on screen
  that will be read within `TILE_AHEAD` notches and are not in
  (`tiles_wanted`); neighbours make one run, and each run is one Range
  request on the opened, uncompressed text part (`ethaor-text.plain.bin`)
  — a few kilobytes, one round trip, for a dive faster than the books. The
  books fill everything else, and the ELS needs them all. Rejected: books
  as chapters — each compressed apart, starting cold, and each a request.
- **Until a chapter lands, its reading level stays closed** (through
  `line_height`, the one gate of the reading level, per chapter): it keeps
  its number, nothing draws, clicks or names its verses, and no frame is
  black however deep the dive. The other edition's readings come with the
  whole text; the chapters laid before are laid again then. The ELS answers
  -1 until the whole text is in: its letter stream is read off the verses.
- **`build/index.html` declares the module and the Hebrew font for preload**
  (`scripts/preload.mjs`, after the build — the names carry hashes): the
  browser fetches them while it reads the HTML, not once the scripts run.
  A host such as Cloudflare turns them into 103 Early Hints.
- **The page's marks** — `ethaor:map` (first image), `ethaor:text` (whole
  text landed), `ethaor:verse` (first verse drawn) — are what
  `scripts/bench-load.mjs` reads: the site served over HTTP/2 with
  on-the-fly compression, as a CDN serves it, Chrome on throttled networks
  and processors, cache empty, optionally a visitor diving at once (`DIVE`).
  The criterion is the first verse a visitor can read.

## Data

Source: `archive/t002/t/` — Mechon Mamre HTML in **cp1255**, not UTF-8;
`t01.htm`..`t35.htm`, one file per book. 39 books (35 source titles —
`עזרא / נחמיה` is one), 929 chapters, 23 200 verses. Counts are asserted; do
not re-estimate them.

Source traps, handled in `build.rs`:

- byte **0xCA** is unassigned in standard cp1255; Mechon-Mamre uses it for
  `U+05BA`. Without the fix, signs are lost.
- the first `<H1>` of `t01.htm` is a cover page, no verse.
- cleaning strips `{פ} {ס} {ר} {ש}` and all punctuation. The maqaf joins two
  distinct words, so it becomes a **space**, not a deletion.
- an **editorial mark can sit inside a rank**: Psalm 107 closes its bracketed
  section in the label itself (`<B>קז,כג ]</B>`). A rank keeps Hebrew letters
  and the hyphen of the joined ranks (`ע,א-ב`, one text under two numbers) —
  anything else is markup, and the breadcrumb was reading it out.
- four books are split (`SPLITS`): Mechon-Mamre numbers Samuel, Kings,
  Chronicles and Ezra/Nehemiah straight through with **no restart at `א`**, so
  we cut at the traditional rank and renumber with `hebrew_numeral`, where 15
  and 16 are `טו` / `טז`, never `יה` / `יו`.

**Where another publication of the edition reads otherwise, the map shows
both** (`build/variants.tsv`, 94 readings over 88 verses). The table was
measured word by word against Miqra according to the Masorah, book by book —
numbering plays no part — and every entry is **anchored by rank**, never by
searching for a form: a form can appear twice in a verse, and only the index
says which one is meant. The build checks each anchor and stops if one has
moved, so the table cannot drift away from the source.

Setting apart what is not the text: the other edition's own apparatus (its
notes on Sefarad, Ashkenaz, Yemen or the Keter), its pointing of the divine
name, and the cantillation — which our source does not carry at all.

## Layout (`treemap.rs`)

**Canonical order beats tiling aesthetics.** A squarified treemap sorts by
size; that would be wrong here. Row cutting that preserves order.

- **Books sit on a 5-column grid**, the one the five books of the Torah impose.
  Rejected: 7 columns — bigger cells, but the grid states the order of the
  Tanakh, not a cell-size knob, and the overview must not move for a reason
  that is not its own.
- A section's height is not proportional to its text volume: it is computed so
  that **all book boxes are identical** (`band + n × row`).
- **A column's width comes from the chapter block, not from the world**
  (`BOOK_SLACK`): widest block (Tehilim) plus a tenth of air. So the vertical
  is solved first (row height, band, stride), then the column is cut to the
  stride — change the stride and the width follows, ratio intact. The book area
  is centred in the world, leaving black left and right rather than void inside
  every box. Price: smaller overview titles. Accepted.
- **Sections touch; what separates them is the air of their band**
  (`SECTION_LEAD`). The section title is flush to the bottom of its band, so
  the empty top of the band — and it alone — is the breathing space between
  portions. Setting it here rather than as a margin keeps book boxes identical
  across sections.
- **Portion air is the map's only knob, and it is paid in cells**: world height
  is fixed, so what the three bands take, the book rows do not get. Cost of the
  retained value: one more zoom notch to reach the verses. Accepted.
- **Air has two values, one per detail level** (`SECTION_LEAD_WIDE`,
  `SECTION_LEAD_TIGHT` = half). In overview it says the Tanakh is in three
  parts; once chapters show, the division is known and the same void only pushes
  the text away. Both layouts are built at startup and **swapped** at the
  threshold (`set_detail`); nothing is recomputed mid-zoom, only verse layout
  (which depends on cells) is dropped. The world keeps its proportions, so the
  camera stays valid: the map reorganizes, the view does not jump.
- The world's **bottom margin is `SECTION_LEAD`**, not the general margin, or
  the last row of the Writings looked cut off. The top needs nothing — the first
  section already carries its own lead above its title.
- **A book's title band is deduced, not set** (`chapters_fit`): band and stride
  are tied (title fills its band, number fills its cell, number is half the
  title, and what the band leaves splits between the rows). Fixing one fixes the
  other, so they are solved by fixed point — a few passes settle the pair. The
  bigger the cell, the sooner verses become readable, so the cell is what the
  trade-off protects.
- **Numeral width enters the computation**: a number is bounded by its cell's
  height *and* width, so if measured Hebrew digits are wider than the estimate,
  width decides and the band must tighten to keep the ratio. `build_frame`
  therefore redoes the layout **once**, when the page has measured the numerals
  — not per zoom.
- The band is **never clipped** to a share of the section: an `h / 2` bound gave
  the Torah (single row) much taller boxes and overlapping rows.

**One single stride for the 929 chapters.** A cell — and therefore a chapter
number — is exactly the same size everywhere in the corpus. **This is a rule,
not a setting**: no cell or numeral size may depend on the book, the zoom or
what is on screen. Everything follows:

- **rows are imposed** (`CHAPTER_ROWS`): the row count of the densest book, the
  150 Psalms. Everything else derives from it, so the whole Tanakh shares one
  grid. Few rows stretch a block into a ribbon and hit the width; many rows
  crush it in height; the retained count is at the crossing.
- **stride** = body height / those rows.
- **cells per row** = `ceil(densest / CHAPTER_ROWS)`: it comes from the densest
  book having to fill its rows, not from box width, which would take twice as
  many. In a wide, short box height is scarce, never width — so a block takes
  only half the box in width, and that is intended.
- the block is **flush top-right**, where reading starts. Centred vertically, a
  short book put its row in the middle of an empty box, far from its title; the
  emptiness belongs at the bottom.
- a book that does not fill its row stops where it ends, at everyone else's
  stride, leaving the slots to its left empty. Stretching three chapters across
  the full width gave three cells lost in void.
- more rows for Tehilim are **paid on the cell** (`CHAPTER_GAP` was tightened to
  give part of it back, and that was all there was to give).
- books and sections, unlike chapters, get their columns from `grid`: the **most
  balanced arrangement among those costing almost nothing in cell size**
  (`KEEP`), biggest cell breaking ties; cells fill right to left, then down —
  reading order. An imposed column count never falls back on the element count,
  or the Torah's five books would inherit wider boxes than the other sections
  and the chapter cell would no longer be unique.
- Rejected: **max cell per book** (one size per book — excluded by the rule
  above); **square packing** (25 chapters in 5×5) — tiny cells in a wide box,
  and making it big would need a portrait world that wastes a landscape screen.

### Verses

- **All verses of a chapter share one character size.** A long verse wraps
  instead of being compressed: only the line count varies. A breathing space
  (`gap_k`) separates verses so one can see where each begins.
- `lay_out` starts at one line per verse and tightens while the column
  overflows — line count can only grow with size, so a few passes suffice.
  Cached per chapter, invalidated when a measurement arrives from the page.
- The unit of rendering is the **line**, not the verse: `Line` carries its
  corpus slice, box and size. `wrap` cuts at words keeping contiguous slices, so
  only an offset and a length cross the boundary.
- Each line is flush right from the measured width; **alignment is never
  delegated**.
- **Once the text is out, its numbers sit beside it, never in it.** The
  chapter number takes the top-right corner of its column, on a header row of
  its own (`CHAPTER_MARK`): the centred number is gone and the reader must
  still know which chapter he reads. Each verse number sits at the right of
  its first line, in a gutter (`VERSE_MARK`) — Hebrew reads from the right,
  that is where a verse begins. Both are reserved by `lay_out` and paid on the
  character size, so nothing overlaps.
- **A cell's padding follows the cell** (`CELL_PAD`, a share of the side). Fixed
  in world units it threw away most of the ink of a small cell and delayed
  verses by several notches. All rendering is proportional, so padding must be.
- **A line's readability floor is the text's** (`READABLE_LINE`): at the given
  line height it yields exactly `MIN_TEXT` characters. Asking more gained no
  readability and pushed verses back two notches.
- **Goal: dive into the text fast.** Overview shows books, the first notch the
  chapter numbers, verses around the tenth notch (window-dependent). It used to
  take twice that.

### The other reading

- **It takes its place in the flow, it is never laid over the text.** The
  build welds, for each verse concerned, a **display string** — our verse with
  the other form inserted right after ours — and the verse is laid out from
  that string: wrapping, measuring, word boxes, all of it. Painting it on top
  would have broken the one rule the frame tests hold, that nothing covers
  anything.
- **The other reading is bracketed**, as both editions print an alternative
  reading themselves. Side by side with a space alone the two forms read as two
  words of the verse; the brackets say they are one place in two versions,
  which no colour says.
- Each form carries its index, `¹` ours and `²` theirs, and the page names the
  two editions in a row under the canvas, taking its own row so no pixel of map
  is lost. Each entry gives the edition, then who publishes it in parentheses,
  and **wears the colour of the form it names** — so the index is read twice
  over, by the number and by the ink. The row reads right to left, like the map
  it explains.
- **The index follows the pointer, not the frame** (`readings_at`), and it
  opens **on the two forms and nowhere else** — the owner's rule: the
  pointer on our marked word (`¹`) or on a word of theirs (`[…]²`), drawn
  at reading level. Rejected: the whole chapter holding a reading — the
  index then spoke while the reader was lines away from any mark. A number,
  the void of a cell, a book, a portion and the black say nothing, as the
  breadcrumb and the click say nothing there. In the book view the same:
  the pointer on either form (`data-form`), nothing for the chapter.
- **Theirs is yellow only while its pair is pointed at** — the owner's rule:
  every reading coloured at once lit the page with marks the reader was not
  looking at. `readings_at` answers *which* reading (theirs' byte range), and
  the page colours that one's runs alone; every other stays white, its
  brackets and index still saying what it is. Same in the book view, the two
  forms of a pair sharing their `reading`. No pointer — a touch screen — no
  colour.
- **The ink is the last slot of a text record**, and it is all the page is
  told: a run is ours or theirs, and which colour stands for which is the
  page's business. Both are drawn at the same strength — the colour alone sets
  them apart, when it is worn. A line is therefore cut into runs at drawing time, laid from the
  right like everything else; a verse without a variant stays one single
  command.
- **The breadcrumb and the click keep naming our text.** They read the corpus,
  which the display string leaves untouched. A word of ours is given back
  without the mark that numbers it; a word of the other edition names nothing,
  since the passage does not contain it.

### Labels

- **Titles are sized by their band**, which is in world units: they grow with
  the zoom, like the box and the chapters. A fixed pixel size opened an
  ever-growing gap between frame and first chapter — what is wanted is a
  constant *padding* in the web sense.
- `uniform_title` keeps the **median** on-screen band, never the smallest: one
  tiny book would crush every overview title. All titles share one size at a
  given zoom.
- **A book title keeps the right edge of its box, never the screen's.** It
  reads just after the last cell of its first chapter row; clamping it to the
  window made the name slide left while the book was dragged right.
- **Portion titles are the only ones pinned to the screen**: a portion is
  several screens wide once chapters show, and its name must stay readable while
  you are inside it. The caller clamps its box to the window.
- A title is flush to the **bottom of its band**, i.e. just above the first
  chapter: that is what keeps the gap stable, up to `TITLE_CEILING` — past that
  the band has left the screen, you are inside the book, and the title no longer
  needs to grow.
- **A title stays in its band, never pinned over content.** The band is reserved
  for it, so it covers nothing. Pinning made titles bite into chapters, verses
  and other titles; a dark veil made them readable but still masked.
- When the band leaves the screen, the **breadcrumb** takes over: `focus`
  returns the position under the view centre and the page shows it in its own
  row above the canvas. Nothing is covered, nothing is lost.
- **The breadcrumb goes down to the verse, and its depth is the zoom's.** Four
  levels — portion, book, chapter, verse — each appearing only with what the map
  already shows (`chapters_ready` for the chapter, `READABLE_LINE` for the
  verse). Announcing an invisible chapter would be an invented position; the
  breadcrumb never says more than the image.
- **Its point is the pointer when there is one** (`focus_at`), the view centre
  otherwise: one reads the breadcrumb to know what is under the cursor. The
  level stays the zoom's — hovering a book in overview gives portion and book,
  not a chapter.
- A **portion's name** names the portion and stops there: it belongs to no
  book.
- **The pointer on nothing clears the breadcrumb.** In the black around the
  map, in the air between portions, in the void of a box — `focus_at` returns 0
  and the row empties. Falling back to the centre of the view was tried: it
  kept the last position on screen, which read as a place the pointer had
  already left. Price: the breadcrumb blinks out between two letters. Accepted
  — naming somewhere one is not is worse than naming nowhere.
- With **no pointer at all** — a touch screen — the page asks `focus` instead
  and the centre of the view speaks: there is no hover for it to contradict.
- **A mouse gone from the canvas empties the breadcrumb.** Falling back to the
  centre made the row name a portion while the cursor sat on the row itself,
  read as a selection. Only a lifted finger hands over to the centre. The
  page only reports the leave (`leave`); the decision is the module's.
- The verse is given by its **number**, not its text: it is a position and must
  fit one navbar row. Text comes from a click (`select`).
- Each title is clipped **in width only**. A small book's band is lower than the
  text line; clipping in height cut the tops of letters and faked different
  sizes. Overflow goes up, into the gutter between books, never onto the
  chapters — the title is anchored to the bottom of its band.
- The band reserved by `treemap` is of **constant height** (section:
  `lead + SECTION_TITLE`; book: deduced by `chapters_fit`, clipped by `header`
  only when the box is too low to hold it), not proportional: a proportional
  band is nonexistent for a small book and its title then landed on the
  chapters.
- **A chapter number is the same size everywhere** (`number_size`), since all
  cells are. One size per book was tried: ten different sizes on screen at once.
  Excluded.
- **A number is half a book title** (`CHAPTER_OVER_BOOK`). From that ratio
  `book_band` derives the band height, hence the stride, hence the cell size:
  the cell is cut for that body, not the other way round.
- **And it reads whole or not at all**: the height/width fill bounds only hold
  the invariant if a measurement from the page makes digits wider than the
  estimate. Overflow is impossible by construction.
- Numbers appear **from the first notch** (`MIN_NUMBER`, below the running-text
  floor: a numeral is one to three known letters read as a shape, where a verse
  line must be deciphered). The floor no longer decides anything on usual
  windows.
- The number is **centred in its cell**: `emit_text` is flush right, so it gets
  a box stopping at the middle of the numeral.
- **At startup the map shows only sections and books.** While chapter numbers
  are hidden, a book's box is free: its title fills it, width and height, and
  centres in it.
- Numbers arrive **at the first zoom notch** (`CHAPTER_FROM`, expressed as a
  multiple of the initial framing, not in pixels, so the threshold falls at the
  same notch on any window and after a fold as before).
- **The threshold is not enough on a small screen**: `chapters_ready` also
  requires the numeral to clear its floor (on a phone the first notch crossed
  the threshold and the frame was entirely black). The switch is always decided
  on the *chapter* layout, the one that will be in place after; judging it on
  the current layout made the level oscillate frame to frame.
- **The framing has a floor so levels fall at the same notch everywhere**
  (`base_scale`). Natural framing is the whole world in the window; on a narrow
  window that was three times smaller than on a phone. We prefer to **crop the
  map**: framing never goes below the scale that puts a numeral just above its
  floor at the first notch. Price: a narrow window does not show the full map
  width at start — one notch out and a drag brings it back.
- **The wheel notch is computed, not fixed** (`zoom_step`, exported to the
  page): whatever makes the dive from framing to a full-screen chapter take
  `DIVE_NOTCHES` notches. Landscape gives the nominal step; floored framings
  give slightly more.
- The one size step over the whole zoom range is the detail-level change, and it
  is intended. The title then **fills its band** (`TITLE_FILL`): a smaller share
  deepened the step and gained nothing, the glyph slab being shorter than its
  line anyway.
- **A section title is a quarter bigger than a book title**
  (`SECTION_OVER_BOOK`) — just enough to read as a title above the books, pure
  white against `BOOK_INK` doing the rest. Bigger, it ate the air between
  portions. It has no ceiling of its own; the book ceiling bounds both, so the
  ratio holds at every zoom.
- **A cell always shows something**: its text as soon as it is readable, its
  number otherwise. Two holes gave empty cells (a minimum-size threshold on the
  cell, and a text-mode cell whose lines all fell under `MIN_TEXT`), so we check
  afterwards that text was emitted and fall back on the number.

## Endless zoom

At the heart of every letter, `NEST` reserves the place of the whole Tanakh —
a large share of its cell, not a point.

**No fill is ever painted.** Rendering emits text and clips only: no white
square, zooming in or out. That was the patch over a hole, and the hole was
plugged on both sides — a newborn world a thousand pixels wide on one side, a
zoom-out floor on the other.

- Nothing of the contained world is drawn before the fold: a letter can be
  zoomed fully with nothing appearing.
- **Once the letter is exceeded, `wrap_in` re-roots the camera** and the level
  increases. There are never two worlds on screen: `build_frame` draws the
  current world, full stop. No cross-fade, no point list, no nested camera.
- At that instant the new Tanakh takes most of the screen, so its portion titles
  are already readable: no intermediate state. With a tiny `NEST` it was born a
  few pixels wide and took a dozen notches to become readable — hence the old
  white fill.
- **Re-rooting early is about precision, not looks.** Waiting for the point to
  cover the screen left ~30 notches where the camera handled coordinates near
  1e8 in f32: the ulp exceeds the pixel and the point drifted hundreds of pixels
  off centre. Folded into the point's world, everything is small and exact.
- Zoom follows **the chapter** while it does not fill the screen, then the
  letter. Backgrounds being black, the void between cells looks like everything
  else: without that attraction, zooming beside a cell led to a black screen with
  no way out.
- The contained world then sits at the centre of the letter while zoom keeps the
  cursor still, so an **attraction** pulls the view toward it, strengthened on
  approach — otherwise the point would have to grow far past the screen to cover
  its corners, losing float precision.
- **The attraction must beat the drift of the zoom** (`pull`): one notch
  multiplies the cursor-to-target distance by its factor, so a correction of
  `1 - 1/factor` compensates exactly and anything below lets the target escape.
  A fixed percentage let the aimed cell leave the screen. A margin is added on
  top to converge instead of merely following.
- After the fold the world is smaller than the framing, so the zoom-out rule
  **centres** it and holds it there while it grows. Free framing returns when
  the world fills the screen.
- **The descent is one-way.** Once the new Tanakh fills the screen the old one
  ceases to exist: no return path is kept, and zooming out does not come back.

**The map opens on the three portions alone, large, evenly spaced**
(`PORTIONS_UNDER`, `overview_scale`). The three levels are one notch apart, and
the opening sits on the first of them — the Tanakh is read in three parts
before being read in thirty-nine books, and the first image says so instead of
showing everything at once:

| notch from the opening | what is shown |
|---|---|
| the opening, one notch under the framing | the three portions alone, large |
| one notch in — the framing | the portions and the 39 books |
| two notches in | the chapters |

Price: the dive to the text takes one notch more than `DIVE_NOTCHES`. Accepted
— it buys the first thing the map has to say. The opening is **the framing
divided by the notch**, never a scale of its own: every threshold hangs off
`fit_scale`, and a second reference would let them drift apart. Esc comes back
here, since this is the overview.

Two places ask whether the camera is still there — a resize, and a measured
numeral that rebuilds the layout — and they ask it through one function
(`at_opening`): a view believed to be at the opening by one and not by the
other would be refitted by one and left behind by the other.

The threshold sits just under 1, not at 1: the framing itself must stay on the
books' side, rounding included. Waiting for book titles to fall under the text
floor took five notches of ever-paler word grid.

Two rules change at that level:

- **Position**: in the world it comes from section volume, which spaces them very
  unevenly. Once the books are gone that inequality says nothing, so
  `draw_world` spreads them in equal parts over the screen height.
- **Size**: taken from a share of the projected world (`SECTION_CARD`), not from
  the section band. Following the band made them minuscule exactly when they
  became the whole image. So they are born large, then **shrink and close in at
  every notch out, endlessly**: the map recedes instead of disappearing.
  Rejected: a fixed pixel size — readable, but zooming out then did nothing.
- Their **gap** is a share of the world height (`CARD_GAP`). Larger, the three
  words held the screen corners and stopped reading as a group; smaller, too
  tight.
- **The three share one right edge**, that of the widest word; three separately
  centred words pulled in three directions. The resulting slab is centred on the
  world.

The level is recognized **by zoom alone, and on the right side of the framing**.
Deep inside a book the title bands have also left the screen; judging on them
painted the three portions giant over the verses.

**Zoom-out is therefore no longer bounded by readability** (`floor_scale`),
since the three titles no longer shrink to nothing: only a technical guard keeps
the scale off zero. The map stays **centred** — below the framing the camera
recentres at each notch.

`locate` tolerates gaps — verse breathing, inter-cell spacing, book margins — by
keeping the nearest line, cell or chapter rather than the containing one.
Without it, aiming between two boxes never descended.
**Tolerance stops at the book**: a point outside every book box designates
nothing. `hover` follows the same rule, so the pointer announces what a click
will do.

**That tolerance is the zoom's, not the pointer's.** A box is mostly void: a
cell is far wider than its number — the stride is the densest book's — and a
book's title falls back into its band as soon as the numbers are out. Naming
from anywhere in a box made the breadcrumb answer for letters the pointer was
an inch away from.

So **the pointer names what it is drawn on, level by level, and nothing else**.
Each level has its own ink, taken from the very geometry the drawing uses — a
portion's name (`section_ink`, two geometries: the cards of the opening, the
band above the books), a book's name (`book_ink`), a chapter's number
(`names_chapter`), a line of verse. Deepest first, and they never overlap,
since a band is reserved for its title. **On nothing, nothing**: the pointer
answers 0 and the centre of the view takes over, the rule the air between
portions already followed.

It follows that **a level that is not drawn cannot be named**. At the opening
the map is three words, so no book can appear in the breadcrumb — not even from
the view centre, which is gated on `books_drawn` for the same reason. A
portion's name still names its portion alone: it belongs to no book.

Where the text is drawn the rule stays the click's, which the cursor must not
contradict — and there the lines fill the column anyway. The view centre keeps
the tolerance of `locate`: it is not a pointer, and the breadcrumb must answer
for it wherever it falls.

## ELS (`els.rs`, `els/`)

**ELS = equidistant letter sequence**: a word read across the whole Tanakh
with the same number of letters skipped between each of its letters —
skip 0 is the word as written, skip 49 takes one letter in fifty — over
**one skip or a whole range of them**. No display
yet; the page only has the exports, all prefixed `els_` (`els_search`,
`els_hits_ptr`, `els_hits_len`, …).

- **It reads letters, not text.** `build.rs` welds `letters.bin`: every
  consonant in reading order, one byte (its alphabet rank), no space, no mark,
  no verse break — so an occurrence crosses verses and books freely, as asked.
  Searching the UTF-8 text would mean decoding marks at every step, and a skip
  counted in letters cannot be taken in bytes.
- **Final forms are folded** (`letter::code`, mirrored by `FOLD` in `build.rs`):
  a skip reads letters, not word endings, so `אדמ` finds `אדם`. Marks in the
  query are dropped, so a pointed query is the same search.
- **A hit is the global rank of its first letter, and its skip**; book,
  chapter, verse, rank in the verse and verse of the last letter derive from
  the rank through tables built once (`Engine`). That rank is also what the
  page will highlight from: the letter's place in its verse, then
  `letter::letters` over the drawn line.
- **The indices are derived in memory, never welded** (`els/index.rs`):
  postings per letter at the first query, bit planes and residue lists per
  letter and gap on demand, cached. Welding them would add megabytes to the
  download for milliseconds of build — the one place the guiding principle
  yields, because the trade is download against a one-time cost.
- **Every method walks the anchors** — occurrences of the pattern's rarest
  letter — and differs in how it finds the steps that fit (`els/method.rs`):
  `Direct` tries each step (a handful of skips), `Pairs` lists the second
  rarest letter's occurrences on the anchor's residue (a very rare second
  letter), `Planes` ANDs bit planes decimated by each letter's gap, 64 steps
  per word (everything else). Rejected: one scan per skip — a range of
  skips multiplied it, which is what made ranges unaffordable.
- **A cost model picks the method per query** (`choose`), linear in what
  each walks, its constants fitted on `bench_methods` — in V8, the real
  target. A fixed method is several times off on part of the grid whichever
  is picked.
- **`Planes` ANDs three planes at most** (`LANES`): past three, a fourth
  plane costs more than the few survivors it would remove; one lets too many
  through to the text checks.
- **Counting never needs listing.** Results are capped (`CAP`) in (skip,
  start) order, the total is exact: past the cap a sink's threshold
  (`Keep`) tells `Planes` to only popcount — a branch-free loop — and two
  letters are counted from ranks on their plane (`count_pairs`), constant
  work per anchor. A flood of billions of hits is counted, not walked.
- **The model is recalibrated after any change to a method**, in three
  steps, from the repository root:

  ```sh
  cargo test --release bench_methods -- --ignored --nocapture 2> native.csv
  cd web && npm run wasm
  node scripts/bench-els.mjs ../native.csv > ../v8.csv
  node scripts/tune-els.mjs ../v8.csv ../native.csv
  ```

  `bench_methods` times each method natively on the grid (12 words, 17
  ranges) and writes what the model reads beside each time; `bench-els.mjs`
  times the same grid in V8, through the page's exports; `tune-els.mjs`
  tunes the constants on V8 and checks them natively. The new constants go
  into `choose` — its doc and `the_cost_model_picks_the_winner` say which
  cases decide. The tables are output, not sources: not versioned.
- **Repeated queries answer from memory** (`Engine::memo`): a redraw or a
  step back costs nothing.
- **No SIMD**: `simd128` gained little on the one loop it vectorises, and a
  browser without it would refuse the whole module — the map with it.
- Price: the stream adds to the download (one byte per letter), the indices
  to the memory. Accepted: speed is the criterion.

## Interface

Monochrome: black background, structures in shades of white — with **one
exception, and it carries information**: the other publication's reading, in
`yellow-200`, where its text and ours part ways — worn while the pointer is on
one of the two forms, white otherwise (see *The other reading*). Rank, level and emphasis keep
to the white; the colour says "another voice", which no shade of white says.
Rejected: setting it apart by the intensity of the white alone — it read as a
faded text rather than as a second reading.

**No frame, no fill.** Neither outline nor plate: sections, books and chapters
share the same black, and only letters carry information. Rank reads in the
**intensity of the white**: section pure white, book one step down (`BOOK_INK`),
chapter number a touch lower (`CHAPTER_INK`). The three parts of the Tanakh are
told apart by the **air above their title** (`SECTION_LEAD`) and by that title
in pure white.

**Hover changes nothing in the drawing**; it sets the cursor shape, and the
page colours the reading pointed at. The command list is bit-identical with
and without hover.

Structure labels (titles, numbers) are drawn in world coordinates, so they grow
with the zoom — but `label` drops them as soon as their box exceeds the screen;
beyond that it would only be a giant glyph floating over the current level.
**One exception, at the last zoom-out state**: when the map is just the three
portion names, their size and gap come from the projected world
(`SECTION_CARD`, `CARD_GAP`), not from their band — a section's band is worth
its volume of books, and once the books are gone it says nothing.

Gestures: wheel or pinch to zoom, drag to pan, click for the passage, double
click for the word, Esc for the overview. **The keyboard does the same at the
centre of the view** (`keys.ts`): `+`/`-` zoom, arrows pan (Shift farther),
Enter reads the passage. **Esc closes before it leaves**: the open passage,
then a notice, and only with nothing open the overview — closing a passage
used to throw away the dive. A key held with Ctrl, Cmd or Alt is the
browser's.

**The sidebar** (`Rail.svelte`) is a rail of icons on the left, in a
column of its own — the map is never covered — and it **does not open**:
the owner removed its open/close button, so nothing takes more of the row.
An icon shows its **name** on hover or keyboard focus (`Book mode`, `Map
mode`): a name, not an instruction — the owner's choice, and the one
tooltip there is.

**No tutorial, in any form.** No hint line, tooltip, tour or overlay
teaching the gestures: the map is the first thing to read, and its users
learn it by diving. Tried — one line at the end of the breadcrumb row, then
gone once used — and refused by the owner. The keys are named on the canvas
for the screen reader (`aria-label`), and that is all the instruction there
is. **The mark and its name, `Ethaor`, lead home** from the left of the top
bar, over the sidebar's icons — the bar runs the whole width and the
sidebar starts under it, so no two rules cross: the map's opening,
as Escape gives it with nothing open — the passage closed, the book left.
**One wheel notch is `ZOOM_STEP`**:
from overview to a chapter's text is about ten notches, already long; at a
smaller step it took twice as many and felt like never arriving. A trackpad
sends fractions of a notch, so fine control remains. `touch-action: none` on the
canvas, or the mobile browser intercepts everything.

### Book view

**The same Tanakh, read by scrolling** — a button in the sidebar switches,
and the button names where it goes (`Book mode`, `Map mode`, in its tip).

- **The browser lays the text out.** A web page's reading is what is asked
  for — reflow, selection, copy, find in page, the screen reader — and that
  is the engine's job. The exception to *positions are computed here* is this
  view's alone; the map keeps the rule.
- **The module still says what is read** (`chapter_record`, a twin of
  `GET /api/chapter`): a chapter's names, its verses as shown, the other
  edition's runs by byte offset. The page cuts by bytes, never in a decoded
  string.
- **The map stays alive beneath**, inert: its text keeps landing, and the
  book reads what has landed. A chapter not in is fetched as a tile, at once.
- **The place is handed over, never lost.** Map to book: the chapter at the
  centre, never deeper than the image (`here`) — the Tanakh's start at the
  opening, a book's first chapter with the books alone, the verse once read.
  Book to map: the verse at the middle of the column, brought to its reading
  level (`show`), shown again once the frame has measured what it asked.
- **Only the chapters near the screen are laid out**; the others keep a
  height — measured once laid, estimated before — so the scroll never moves
  under the reader. Rejected: every verse in the page at once — tens of
  thousands of nodes of pointed Hebrew to lay before the first line.
- **Zoom is the text's size, not a camera**: Ctrl and the wheel, a pinch,
  `+`/`-`. The column reflows and the verse under the eye stays put.
- **The whole book view is a traditional web page** — the owner's rule:
  the verses take Tailwind's type scale (`text-lg`, `text-xl` from `sm`,
  `text-2xl` from `lg`) times the reader's zoom (`--zoom`, a scale kept in
  `localStorage`), the browser computing the pixels the page then reads.
  The column is 34 of that size, so the zoom widens it with the text.
- **A button at the bottom right goes up a level** (`up`), four in all:
  the text; its book's chapters alone; the portions with every book's name;
  the three portions alone, as the map opens. Each goes back down where it
  is touched — a portion to its books, a book to its chapters, a chapter to
  its text. Gone at the top. The names come from the module's map part
  (`contents_record`, twin of `GET /api/contents`): no text waited for.
- **The map's opening draws the three portions at the book view's size**
  (`set_portion_size`): the size is the book's Tailwind steps
  (`PORTION_TEXT` in `portions.ts`, one string for both), measured by the
  bridge at each resize and handed to the module, which spaces them as the
  book does (`CARD_LEAD`) and lets them shrink as the map recedes. The
  map's zoom levels stay where they were: only the words change size.
  Switching views shows the same three words, to the pixel.
- **Above the text, too, it is a traditional web page**; the map stays
  as it is. Its levels are sized with Tailwind's type scale, stepped up at
  each breakpoint, never with sizes of their own. **The portions alone
  render as the map's opening**: its size read off the map on common
  screens and rounded to the nearest step (`text-5xl` to `text-9xl`, with
  `min-[120rem]:` and `min-[160rem]:` past `2xl` — in `rem`, or Tailwind
  sorts them before `2xl` and they never apply), one right edge, the three
  centred, the space between two a little more than their size. The books
  and a book's chapters step down from there, a portion's name a step above
  its books', as the map's are. They sit on the verses' column, centred; a
  long name wraps, a screen too short scrolls (its scrollbar hidden, its
  gutter kept so the column stands where the verses' does) — nothing is
  ever cut.
- **A book's chapters scroll on into the books before and after it, one
  book a screen**: each alone, centred, a scroll snapping to the next (a
  book taller than the screen scrolls through first); opened on the book
  asked for. A wheel turns one book a gesture: snapping alone took a notch
  back to the book it left, and the wheel seemed stuck. A book taller than
  the screen (the Psalms) is scrolled by the wheel itself, never past its
  edges — left to the browser, the notch that crossed its end landed on the
  next book and its last chapters were out of reach; only a new gesture at
  an edge turns the book, and coming back up enters it by its end. Its end
  is a snap point too, for touch and keys. The book at the middle of the screen is the one open, the one
  the levels above mark and the map is handed.
- **Switching views keeps the level** (`fromMap`, `toMap`, the map's
  `here_depth`): the map's text opens the book's text, its chapters' numbers
  and its framing the books — the owner's rule: the map's numbers span many
  books, and so does that level — its opening the portions; and
  back: the text on its passage (`show`), a book's chapters on that book at
  the chapters' zoom (`show_book`: numbers out, text not read, its chapters
  at the centre), the books on the framing (`frame_books`), the portions on
  the opening.
- **The breadcrumb speaks for the verse at the middle of the column**:
  there is no pointer to read in a scroll. The readings' legend opens on the
  two forms under the pointer alone, as on the map.
- **Escape never leaves the book**: there is no overview to go back to, and
  the reader's place is the scroll.

### The click

**The click is only live at reading level.** A chapter one cannot read is just a
number in a cell: clicking it would name a verse chosen by geometry, which the
reader has no way to check. `select` returns 0 above the reading zoom and the
selection is cleared.

The threshold is not one more setting: **it is the drawing's**, taken in the
same order as `draw_book` — the estimate first (a line of at least
`READABLE_LINE`, costing no layout), then the actually laid-out size, the one
`draw_line` compares to `MIN_TEXT`. The estimate alone was not enough: a chapter
with long verses wraps, so it lays out smaller than the estimate, and its cell
fell back on its number while a click still believed there was text. The click
is thus live **exactly where the text is drawn**.

The cursor says the same: `hover` answers on the click's condition, not on
geometry alone.

**A click gives the passage, a double click the word** (`select_word`). Both
share the condition: there is no word to show where there is no text.
`word_cells` cuts a line into words as `letter_cells` cuts it into letters, one
step coarser, counting spaces — a word's box carries its ink and nothing else,
and the gaps between boxes are the spaces. Same tolerance as elsewhere: keep the
nearest word, text being flush right.

The first click never delays anything: it shows the passage at once, and the
second click (within the double-click window) refines it to a word. Waiting for
that window before answering would make every click feel sticky.

`select` fills six slots: book, chapter, verse, word — the last two as
(offset, length). The page finds the word **inside** its verse by cutting at
byte offsets, never in the decoded string: one pointed letter is several UTF-16
units.

## Tests

**Nothing is added to the computation without a test.** That is the counterpart
of the guiding principle: if everything is computed here, everything is verified
here. `cargo test` holds 361 tests and answers in 7 s, so there is no reason not
to run it at every step.

```sh
cargo test                        # everything
cargo test -- tests::frame        # one panel
cargo test --test builder         # corpus build
cd web && npm test                # the page's logic, on the built artifacts
cd web && npm run verify          # everything, Rust and page, plus lints
```

### The map of the tests

**`TESTS.md` lists every test, along the file tree** — what each test file
covers, each module, each test with its nature and what it holds (its doc
comment) — then **the functions no test names**. It is generated, never
edited (`web/scripts/tests-map.mjs`), and `npm run verify` fails when it is
stale or when a test that `cargo test -- --list` or `vitest list` reports is
missing from it: it is exhaustive by construction.

**The `tests` skill (`.claude/skills/tests`) applies to every change**: it
inventories a feature's behaviours, maps each to a test or names the gap,
writes and proves the missing tests, regenerates the map, reads it back,
and loops. Without a feature in hand, it strengthens: undocumented tests
first, then the functions no test names.

### What a test asserts here

**Invariants, not snapshots.** A ratio, an order, an equality across the 929
cells — never a list of expected coordinates, which would break at the first
tweak having protected nothing. Numbers belong in tests: a measurement cited
anywhere must be an assertion.

| file | what it holds |
|---|---|
| `src/corpus/tests.rs` | chapters land in any order, each in its place, then the whole text over them gives the corpus byte for byte; books land in any order, the readings refuse to land before them and then complete the corpus; a tile's range is its chapters' stretch; counts; monotone bounded index; the hierarchy partitions; traditional cut ranks; numerals avoid the divine name; cleaned text; the `U+05BA` bytes; a fragment is found back from its offset |
| `src/letter/tests.rs` | a letter = a consonant and its marks; lossless splitting over the corpus; marks never detached |
| `src/treemap/tests.rs` | book boxes all identical; **one stride for the 929 chapters**; nesting, non-overlap, reading order; block flush top-right; rows for Tehilim; portion air paid in cells; number/title ratio |
| `src/tests/geometry.rs` | distances, proportional cell padding, attraction beating zoom drift |
| `src/tests/camera.rs` | floored framing, dive in `DIVE_NOTCHES` notches, **numbers at the first notch on nine windows**, non-oscillating detail level, fold lowering the scale, one-way descent |
| `src/tests/frame.rs` | **no fill, no empty frame, no overlaid text** on nine windows and sixty notches; overview title count; three sizes at chapter level; chapter number in the corner and verse numbers at their right once text is out; no number drawn twice; one book-title size per notch, at the bottom of its band, under its ceiling; three portions on zoom-out, equal gaps, single right edge; measurements converge in one frame; record layout, a contract with `ethaor.ts`; time and command budget |
| `src/tests/click.rs` | the click live **exactly** where text is drawn, cell by cell on nine windows and twelve notches; dead at framing and chapter level; the cursor never lies about the click; the passage under the point; the word under the point, from the right verse; word boxes right-to-left, non-overlapping, covering the drawn width |
| `src/tests/breadcrumb.rs` | depth bounded by what is drawn; portion and book at framing, chapter at the first notch, verse with the text; the pointer names the book, cell or line it is on; a portion band names the portion alone; the air names nothing and the centre takes over |
| `src/tests/locate.rs` | every point of a book box names one of its chapters, none in the air between portions; hover says what the click will do |
| `src/tests/text.rs` | one size per chapter; lines recompose the verse **byte for byte**; every line stays a corpus slice; letters right-to-left |
| `src/tests/api.rs` | the interface the page really calls, global state under a lock: gestures, selection, breadcrumb at centre and under pointer, a mouse gone names nothing, forgetting measurements, probes; the search through the query buffer; the landing exports take a tile as the page writes it; the page searches a stretch of verses (`els_search_in`); a panic leaves its place and message for the page; every chapter read from its record as the book view reads it; the place handed back and forth |
| `src/tests/handoff.rs` | every chapter shown is read and named back, its verse at the centre; on nine windows once measured; `here` never deeper than the image; a chapter out of range moves nothing |
| `web/src/lib/book-view.test.ts` | every chapter read from the real module with its names and verses, the other reading cut out as its own parts, the two forms of a pair sharing their reading; nothing read before a chapter lands; heights estimated from text, size and column; the text's size stepped within bounds; chapters not in grouped into runs; the window around the screen within the Tanakh |
| `src/els/tests.rs` | the stream is every verse read letter by letter; postings, residue lists and planes say what the stream says; **every method equals trying every start and every skip**, on random streams of every size and range; the cap keeps the first hits in (skip, start) order with the exact total; two letters counted without listing; the cost model picks the measured winner; radix sort; placement tables; memo; finals and pointing fold; an occurrence across two verses; every record placed where its hit is; `תורה` every fiftieth letter from the first tav; time budget; `hits` is the records unplaced, from the same memory; a scoped search finds exactly what lies wholly in its stretch, tried one by one; the whole stretch is the plain search; the cost is estimated without searching and tells a flood from a light query |
| `src/pack/tests.rs` | the packed corpus unpacks to the working form **byte for byte**; the map alone holds names, labels and counts and reserves the text's room; the packed form is a fraction of the working one; Hebrew numerals; label exceptions carry only what breaks the rule; varints |
| `web/src/lib/text-open.test.ts` | gzip opened byte for byte, from a view too; a Range asks `from..to` end excluded, and a server ignoring it is read only as far as needed; the lanes take each item once, never more at once than their count, and choose each item when a lane frees — so the view's move is seen; every decoded fragment keyed apart, a verse past a kilobyte included |
| `web/src/lib/search-core.test.ts` | the books file: 39 books then the readings, in order, and opened they give back the plain text the tiles read; the search module answers -1 until the last book and the readings are in, in any order, then finds, its hits a copy |
| `web/src/lib/fault.test.ts` | an incident reads aloud and never repeats; a fault raised at its caller, keeping its number through every layer, made of anything thrown, crossing from a worker unchanged, sent bounded as the service reads it; a real WebAssembly trap traced to its Rust place; the reporter sends each incident once, a bounded number a minute, never throwing; what nothing caught becomes a fault, reported and shown — but the browser's ResizeObserver loop notice, which is none; each code raised at one place |
| `web/src/lib/logs-view.test.ts` | the token leaves the address, the rest of it stays; events whole across any cut of the stream; a log line read for the eye, a line not JSON shown as it is; each row numbered once, the survivors keeping their numbers; the page bounded to its newest lines; what was shown cleared at each opening, an empty history too; the token sent in the header, never the URL; a stream that ends or fails opened again, the history afresh; a refusal not retried and said with its code and incident; a page served without the service said, a bare 503 retried |
| `web/src/lib/keys.test.ts` | each key its command, Shift farther; a key held with Ctrl, Cmd or Alt left to the browser; Escape closes the passage, then the notice, and only then returns to the overview — never in the book view |
| `web/src/lib/search-worker.test.ts` | a search asked before the text is held, then answered by its id; the next one answers the same; a failed search answered by its id, the next on a new module; a failure answered with its numbered fault, a real trap traced to its Rust place |
| `src/tests/landing.rs` | the next book fetched is the nearest to the view, follows it when it moves, each book once; before the text lands the map stays whole — no black frame however deep, no verse drawn, clicked or named — and the verses come once it lands; the overview is identical with and without the text; a chapter's tile is asked two notches or more ahead of its reading level, once, only for chapters on screen, with its exact byte range |
| `src/variant/tests.rs` | the welded table: taking the insertions out gives the verse back byte for byte, every range is a slice on a letter boundary, an offset is mapped back to the verse or refused |
| `src/tests/readings.rs` | the other reading **in the flow, never over it**: the runs of a line tile it from the right and recompose it, the ink marks theirs and only theirs, a verse without a variant stays one command, a click never names the other edition ; the index opens on the two forms alone, never on the rest of their chapter nor on its number; the answer is the pointed pair's reading, theirs alone, the very runs the frame cuts out |
| `src/shared/tests.rs` | the English book table walks the corpus' books; numerals read as their values; every label is a numeral, chapters 1, 2, 3… and verses on from the last, joined ones included; a verse named every way; every letter placed back in its verse; refusals with distinct codes; a page is the engine's hits, letters spelling the word; pages tile the listed hits, `limit` 0 counts; a flood counted whole and listed to the cap, its last page closing the walk without searching again; bounds past any range (skips past the stream, offset at the top of its type) answer without panic; finals and marks fold, a word of marks alone has no letter; one letter is each of its occurrences; the portions are the map's; a book by its exact OSIS code alone, no other case, name or notation; an OSIS reference names its book, chapter or verse, every verse found back from its OSIS, another notation naming nothing; one scope way at a time; a scoped search stays in its verses; a query too costly refused before it runs; every chapter reads back its verses, its book, portion and number; a shown verse is ours with the other forms inserted; a reference names its chapter or is refused; neighbours across books |
| `server/src/tests/` | the ELS answer names its place every way and is the shared page field for field; `next_offset` walks every listed hit; each refusal its code and a 400; `GET /api` lists tools whose schema is what the endpoint takes; the build served with ranges, no compression, a year of cache on hashed names and a 404 for a missing one, `/api` never shadowed; **the page's export and the API find the same hits**; a flood answers its total and last page; numbers past their type refused as `bad_param`; a missing text is a 503, a caller's mistake a 400; under `/api` a wrong path or method answers JSON, `/api/` describes, HEAD has no body; callers at once each get their own answer; no path climbs out of the build; a file asked again is a 304; searches run side by side, one engine each, never more than the pool, all back after; an answer searched once for everyone, at once or later; a refusal takes no engine; a full queue answers `busy` with `Retry-After` and is not kept; a caller gone gives its engine back; successes say `public`, refusals `no-store`; a scope keeps every letter inside, each scoping mistake its code; the page and the API keep to the same verses; a stream is the pages in one go, goes past a page, stops when left; a search too heavy refused before it runs; a client past its budget waits alone, a cached answer costs one; the proxy names the client only when trusted; a panic answered and the next request served; an engine that panicked replaced; a bad setting falls back; the logs closed without the token, a request found by its id, followed live and filtered by level; the log settings read and falling back; the files rolling within their budget, a new one per process, an unwritable directory refused at start; the history read back across files; a half-written line sent once whole; a follower reading across files, told what it missed; a flush writing the lines and waking a follower at once; no log file a 503; every answer has an id; a page of its own served at its directory, `/logs` sent there with its query whole; a settings file read, the environment winning over it and untouched, none the environment alone, a broken one ignored whole; an incident reads aloud and never repeats; an answer and its log share the incident, the log naming the code, the place and the user message; every refusal numbered; an escalation keeps its number and steps, verbose only adding chain and stack; a panic numbered at its own line; every code raised at one place; a page report logged under its number, bounded, a bad one refused; a chapter read verse by verse and named every way, carrying the other reading, its neighbours across books; each chapter refusal its code; **the book view's record and `/api/chapter` read the same chapter** |
| `tests/builder.rs` | `build/mamre.rs` and `build/variants.rs` included, not copied: cp1255 and byte 0xCA, Hebrew numerals, cover page, document order, the four cuts, canonical order; a reading anchored by rank, the display string that gives the verse back |

### Rules for what comes next

- **A new WebAssembly export comes with its endpoint**, its tests on both
  sides and a parity test (see *WebAssembly and API*); what both compute
  goes in `src/shared/` first.
- **Every change follows the `tests` skill**: its behaviours inventoried,
  each held by a test seen red, `TESTS.md` regenerated and read back.
- **Every test has a doc comment** saying what it holds: `TESTS.md` shows
  it, and flags the ones without.
- **Every change follows the `faults` skill**: its failures are listed,
  catalogued, escalated, told, tested — and each test seen red.
- **A new feature says how it degrades**: what it refuses, what it bounds,
  what it drops — and a test holds each (see *Robustness*).
- **A test never waits on an endless answer**: a stream or event source read
  in a test is read under a timeout, or a broken rule hangs the suite
  instead of failing it.
- **A new endpoint is declared in `GET /api` as an MCP tool** — `name`,
  `description`, `input_schema`, `errors` — and a test holds that
  description to what the endpoint really takes.
- **One step = one test.** A computation function added or changed arrives with
  the test that holds it, in the same commit. `cargo test` must be green before
  the next step.
- **A measurement is a test, not a line of prose.** If a ratio, notch or count
  matters, assert it; prose keeps the reason, not the number.
- **A fixed bug leaves a test behind**, written on the exact failing case.
- **A test that walks the zoom builds the frame at every notch.** `zoom_at`
  only moves the camera; `sweep` once read the opening frame sixty times over,
  and the overlap, fill and empty-frame guarantees held nothing.
- **A new test must fail on a deliberate break of the rule it holds** before it
  counts: break the code, see it red, restore. A test green on both is prose.
- **`App` is split by subject, not by layer** (`src/map/`): a file holds one
  subject's methods and keeps private what no other file calls. What a
  sibling needs is `pub(super)`; `pub(crate)` only for what `wasm/map.rs`
  or `src/tests/` reach — the compiler decides, not the habit. Moving code
  between these files changes no computation: a refactor of the map must
  leave every frame, click and breadcrumb bit-identical and the frame time
  unchanged — check both before and after.
- **Private-module tests live in the module** (`src/<mod>/tests.rs`, declared by
  `#[cfg(test)] mod tests;`) — the only place its private functions are visible.
  Everything else goes in `src/tests/`.
- **Nothing touching global state is tested in parallel**: `src/tests/api.rs`
  takes a lock, because `APP` is unique.
- **Fragments passed to `ratio` or `key_of` always come from the corpus.**
  `offset_of` is a pointer difference; a literal overflows it. In a test, take a
  slice of `corpus::text()`.
- **A record's layout is a contract**: changing it in Rust forces changing it in
  `ethaor.ts`. `a_record_keeps_the_layout_the_page_reads` is the alarm, not the
  guarantee.
- **The page's logic is tested in `web/`, the rest of the bridge is not.**
  Measuring and painting stay uncovered; anything that decides — what to
  fetch, in what order, how bytes are opened, when a search answers — lives
  in a small module of its own (`text-open.ts`, `search-core.ts`,
  `search-worker.ts`) with its `*.test.ts` beside it, run on the real
  artifacts. Logic growing inside `ethaor.ts` must move down into Rust or out
  into such a module first.
- **`npm run verify` is the gate**: green before any commit. A script in
  `scripts/` that reads the module's exports is run once after any change to
  them — `bench-els.mjs` measured nothing for a while because the text it
  needed no longer landed.

## Forbidden

- Computation added or changed without a test (see *Tests*).
- Computation in a transport (`src/wasm/`, `server/src/api/`): it belongs in
  `src/shared/`, or the twins drift apart.
- On a request's path: a panic that is not caught, a wait that is not
  bounded, a store that grows without bound, a log write on the answering
  thread (see *Robustness*).
- A search admitted without its estimated cost, or charged to no one.
- Text alignment delegated to the engine on the map: positions are computed
  here. The book view alone is laid out by the browser (see *Book view*).
- HTML parsing at run time: that is `build.rs`' job.
- Allocation per letter, word or verse in the frame loop.
- Layout recomputation on zoom.
- Restating a measured value in this file instead of asserting it in a test.
- **A comment or a note in another language than English** — whatever
  language the conversation is held in. English, always: code comments and
  doc comments (Rust, TypeScript, Svelte, CSS, scripts), **every Markdown
  file** (`CLAUDE.md`, `env.md`, `rapports/`, skills, any note), comments in
  configuration (`Cargo.toml`, `.env.example`, `.gitignore`, `*.tsv`), log
  lines and API messages, and **every message a developer reads**: assertion
  messages, `panic!`/`expect` texts, build and script output, test data
  written as prose. A `should_panic(expected = …)` is translated with the
  message it expects, or it stops matching.
- **What the user is shown in another language than Hebrew or English.**
  Hebrew is the Tanakh's own: its text, and the names that belong to it —
  portions, books, chapter and verse numerals, the editions' names. English
  is everything else the interface says: loading, buttons, labels, error
  messages (`CATALOGUE` in `web/src/lib/fault.ts`). An English element of
  the page carries `lang="en"` and `dir="ltr"`, the page itself being
  `lang="he"`.
