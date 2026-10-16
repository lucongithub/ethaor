# Loading ethaor — report (15 October 2026)

Goal: cut download and display time as far as possible, ahead of the
dictionaries and precomputed searches to come.

## Result

Measured in headless Chrome, throttled network (DevTools presets), empty
cache, median of 3 runs, site served with on-the-fly Brotli compression
(`web/scripts/bench-load.mjs`). "Before" = the last commit (`7ee5c1e`), where
everything came in one block.

### On a PC

| network | before: everything | map shown | whole text |
|---|---|---|---|
| 3G | 20.99 s | **2.81 s** (×7.5) | **8.33 s** (×2.5) |
| slow 4G | 10.34 s | **1.39 s** (×7.4) | **4.13 s** (×2.5) |
| 4G | 2.11 s | **0.43 s** (×4.9) | **1.25 s** (×1.7) |
| fibre | 0.33 s | **0.13 s** (×2.6) | **0.24 s** (×1.4) |

### On a phone (processor throttled ×4)

| network | map shown | whole text |
|---|---|---|
| 3G | 2.84 s | 8.39 s |
| slow 4G | 1.43 s | 4.24 s |
| 4G | 0.50 s | 1.40 s |
| fibre | 0.25 s | 0.44 s |

The previous version was not measured on a phone.

## What was done

1. **Compact pack (step A)** — `src/pack.rs`.
   - Only what cannot be recomputed is sent. The browser rebuilds by itself:
     - the ELS letter stream (513 KB compressed);
     - the index offsets;
     - the chapter and verse numbers (one rule + 2 exceptions);
     - the other edition's rewritten verses (only the insertions travel).
   - Text at 1 byte per character instead of 2 in UTF-8.
   - The build checks that unpacking gives the corpus back **byte for byte**:
     an error fails the build.
2. **Loading in two steps (step B)**.
   - **The module**, code + map: 69 KB compressed instead of 1.7 MB. It shows
     the overview on its own.
   - **The text** is a file of its own, fetched **after** the first image.
     Measured: faster than downloading it alongside.
   - While the text is on its way the map stays whole (the numbers), with no
     black image. Checked on 9 screen sizes, up to 40 zoom notches.
3. **Our own codec** for the text — `src/pack/codec.rs`, built by a sub-agent
   after a full benchmark.
   - Context model, integer arithmetic only, no dependency.
   - **489 KB**, 20 % less than Brotli, zstd or LZMA.
   - Decoded in a **Web Worker**, reusing the module already compiled: the
     map never freezes.
   - Decoded **as it downloads**: on a slow network, decoding ends when the
     download does.
4. **Text form chosen automatically** from the throughput the page measures
   (`SLOW_LINK`, 300 KB/s).
   - **Slow network**: our codec, the fewest bytes possible.
   - **Fast network**: gzip (791 KB), decoded natively by the browser. On a
     phone, the processor rather than the network held the codec back: on 4G,
     1.40 s instead of 2.58 s.
   - The automatic choice lands on the best form in 7 cases of 8; the one gap:
     0.2 s on 4G on a PC.
5. **Fonts**.
   - Baloo cut to Latin, one weight, in woff: 21 KB instead of 3 × 410 KB.
   - The page waits only for the Hebrew font before its first image.
6. **Preloading** of the module and the Hebrew font in the HTML
   (`web/scripts/preload.mjs`), which Cloudflare turns into *103 Early Hints*.

### Codec measurements (verse text, decoded in V8, PC)

| codec | size | decoding |
|---|---|---|
| ours (context model) | 499 KB | ~340 ms |
| LZMA (lzma-rust2) | 626 KB | 29 ms |
| zstd (ruzstd) | 635 KB | 13 ms |
| Brotli (brotli-decompressor) | 635 KB | 13 ms |
| gzip, decoded natively | 771 KB | 11 ms |

Measured afterwards in Chrome: on a phone, our codec decodes about 165 KB
compressed per second, ~9 times slower than on a PC. That is what led to the
automatic choice between codec and gzip.

### Set aside, with measurements

- **wasm SIMD**: +10 % only, and a browser without SIMD would no longer load
  the module at all.
- **Embedded LZMA or zstd**: their decoder would add 24 to 43 KB to the
  module, on the path of the first image.
- **Compression left to the host**: its level is out of our hands.
- **Consonants and marks apart**: larger (933 KB against 607 KB).
- **Blocking the zoom while the text is missing**: of no use, no image is
  empty without it.
- **Optimising the module for size** (`opt-level=s`): −7 KB only, for slower
  code.

## Tools left in the repository

- `web/scripts/bench-load.mjs`: loading in Chrome, network and processor
  throttled (`CPU=4`, `TEXT=codec|gzip`).
- `web/scripts/bench-els.mjs`, `web/scripts/tune-els.mjs`: calibration of the
  ELS search's cost model.

## Next: tiles per chapter (the Google Maps idea)

Criterion added to the benchmark: **the first readable verse**, for a visitor
who zooms 16 notches as soon as the map shows (one notch every 150 ms,
`DIVE=16`).

**Principle kept.**
- Tiles **complement** the whole stream, they do not replace it: the codec
  learns on the whole corpus, and cut into hundreds of pieces each piece
  would start from scratch.
- A tile is a chapter's stretch of the uncompressed text
  (`ethaor-text.plain.bin`), asked by a partial request (*Range*). An average
  chapter weighs ~2.6 KB: a single network round trip.
- Rust names the visible chapters that will become readable within 3 notches
  (`TILE_AHEAD`); neighbouring chapters are grouped into one request.
- Each chapter lands straight in its final place, in any order
  (`corpus::Landing`): a buffer at the final size, zero-filled, and atomic
  verse bounds.

**Result (PC, first readable verse)**

| network | without tiles | with tiles | limit (map + gesture) |
|---|---|---|---|
| 3G | 8.41 s | **4.30 s** | ~4.3 s |
| slow 4G | 4.23 s | **2.89 s** | ~2.9 s |
| 4G | 2.04 s | 1.99 s | ~1.95 s |
| fibre | 1.70 s | 1.61 s | ~1.6 s |

The visitor no longer waits for the text, only for their own gesture: that is
the physical limit.

**Benchmark moved to HTTP/2** (as a CDN serves). The map then shows in 2.61 s
on 3G, 1.29 s on slow 4G, 0.35 s on 4G and 0.11 s on fibre; the first verse
comes at 4.08 / 2.77 / 1.83 / 1.62 s.

**What remains on the critical path** (~122 KB transferred on 3G):

| item | size transferred |
|---|---|
| module | 67 KB |
| Hebrew font | 18 KB |
| JavaScript | ~33 KB |
| CSS | ~3 KB |

## Next: two modules, the search in a worker

The search modules to come will be added on: the search left the map's
module.

- **One crate, two builds** (Cargo features `map` and `search`).
  - `ethaor.wasm`, the map: **52 KB** gzip instead of 73.
  - `ethaor-search.wasm`, the search: the codec and the ELS, 48 KB.
- **The search module lives in a worker**, with its own copy of the corpus.
  It is loaded after the first image. Neither decoding nor a search ever
  freezes the map.
- **The page's search API is asynchronous**: `search(word, skip_min, skip_max)`.
- **The build refuses a module** that exports what is not its own.

| network (HTTP/2, PC) | map before | map after | first verse before | first verse after |
|---|---|---|---|---|
| 3G | 2.61 s | **2.42 s** | 4.08 s | **3.91 s** |
| slow 4G | 1.29 s | **1.21 s** | 2.77 s | **2.68 s** |
| 4G | 0.35 s | 0.39 s | 1.83 s | 1.83 s |
| fibre | 0.11 s | 0.14 s | 1.62 s | 1.61 s |

On 4G and fibre, the differences are measurement noise.

Checked in V8: the search module decodes the text as it comes and gives it
back identical; the ELS finds `תורה` (skip 49) at letter 5.

## Our codec removed

Since the tiles, our codec changed nothing for reading: the first verse came
at the same time with it or with gzip. It only brought the whole text sooner
on slow networks, at the price of:

- several seconds of processor on a phone;
- a second text file to publish;
- ~640 lines of specialised code.

It was removed: the whole text travels as gzip, decoded natively by the
browser. The search module goes from 124 to 95 KB raw.

| measurement (HTTP/2, PC) | with codec | gzip only |
|---|---|---|
| map, 3G | 2.42 s | 2.43 s |
| first verse, 3G | 3.91 s | 3.91 s |
| first verse, slow 4G | 2.68 s | 2.71 s |
| whole text, 3G | 7.96 s | 11.08 s |
| whole text, slow 4G | 3.95 s | 5.46 s |
| whole text, fibre | 0.25 s | 0.21 s |

The whole text only serves the ELS search and reading elsewhere than where
one dives: on 3G, the search is ready ~3 s later. Reading is untouched.

Checked in V8: the gzip opened gives the text back byte for byte, and the
search still finds `תורה` (skip 49) at letter 5. 245 tests green.

## The text book by book, nearest to the view first

The user's idea, as Google Earth does.
- The map, down to the chapter numbers, is immediate: it is in the module.
- Then the text comes **by books**, always the book nearest to the centre of
  the view first (`next_book`). The order is recomputed at each send, so it
  follows the visitor if they move.
- Dive tiles go at high priority, books at low priority.

**Implementation.**
- One file, `ethaor-text.books.bin`: 39 books each compressed on its own,
  then the readings. 821 KB in all, against 791 KB in one block (+4 %).
- Each block is asked by a partial request (*Range*). Where they lie comes
  with the page's code (JSON): no request to learn it.
- The search worker gets the same books.

**Tuning loop: books in flight.** One at a time paid a network round trip per
book: 24 s on 3G for the whole text.

| whole text | 2 | 4 | 6 | **8 (kept)** | gzip in one block |
|---|---|---|---|---|---|
| 3G | 15.1 s | 12.3 s | 12.1 s | **12.0 s** | 11.1 s |
| slow 4G | 7.8 s | 6.2 s | 6.0 s | **5.9 s** | 5.5 s |
| 4G | 2.6 s | 1.6 s | 1.4 s | **1.36 s** | 1.17 s |
| fibre | 0.68 s | 0.40 s | 0.33 s | **0.26 s** | 0.21 s |

**The idea's criterion: the book looked at arrives almost at once.**

| network | map | nearest book to the view in |
|---|---|---|
| 3G | 2.42 s | **2.76 s** (+0.34 s) |
| slow 4G | 1.21 s | **1.38 s** (+0.17 s) |
| 4G | 0.36 s | **0.44 s** |
| fibre | 0.13 s | **0.14 s** |

With the single file, that book could wait up to 11 s on 3G, depending on its
place in the Tanakh. The first verse of a dive stays bound by the gesture
(3.9 s on 3G), whatever the setting. The whole text, which the ELS search
needs, comes ~0.9 s later than in one block on 3G: the price of compressing
book by book.

247 tests green, two of them new:
- books land in any order, and the readings wait until all are in;
- the next book asked is the nearest to the view, it follows the view as it
  moves, and each book is asked once.

## Last phase — delivered

1. **Check.**
   - 251 tests green; `npm run check`, clippy (on the three targets) and
     Prettier: nothing to say.
   - The codec's efficiency locked:
     - a tight size ceiling (502,000 bytes);
     - the build refuses a codec that no longer beats gzip by a third;
     - a release speed benchmark: ~380 ms of native decoding.
   - Deliberate breaks: removing the margin of decoding-as-it-comes turns red.
     With a degraded model setting (learning rate 1), the text grows past the
     ceiling, which stops it.
   - The tuning loop found better: mixer learning rate 6 instead of 3, i.e.
     −1.2 KB.
2. **Check for real.**
   - In Chrome, a dive down to the verses then a click: the text of
     Zephaniah 2–3 shown, the breadcrumb down to the verse, the passage
     opened on click.
   - Both text forms (`codec`, `gzip`) give the first verse in the same time.
3. **Readable benchmark**: the text form chosen shows in the table.
4. **Documentation**: CLAUDE.md up to date (transport, tiles, two modules,
   codec efficiency, tools, test table).
5. **Cleaning**: the temporary copy of the old version deleted, after checking
   its links to `node_modules` and `archive` were removed and the originals
   intact.
6. **Still to decide**
   - The commit.
   - Should the generated files in `web/src/lib/` be versioned?
     - which: `ethaor*.wasm` and `ethaor-text*.bin`, ~4 MB of which 2.4 MB of
       uncompressed text;
     - `npm run build` regenerates them anyway;
     - but a deployment building from git without Rust would need them.
   - The first Cloudflare deployment (`npx wrangler login`, then
     `npm run deploy`), followed by a real measurement on the public URL.
