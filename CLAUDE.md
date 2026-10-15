# ethaor

Zoomable map of the Tanakh, no background. **Rust compiles to WebAssembly and
paints into a page.** Single criterion: speed.

Display reference: https://github.com/Peeter95/code-map — a box's size follows
its volume, and zooming reveals its contents inside it.

## How this file works

**Rules here, numbers in tests.** This file states invariants, intent and
rejected alternatives — the things a test cannot express. Every measurement,
ratio, count and threshold lives in `cargo test` (251 tests, 5 s), which is the
only place they stay true. Do not restate a measured value here; if a rule can
be asserted, assert it and leave only the *why* in prose.

When you add a rule: synthesize. One line per invariant, reason attached.
If a test already enforces it, the test is the documentation — the sole rule
that must stay written is **"no calculation without a test"** (see *Tests*),
because that is what keeps the rest verifiable long-term.

## Work split

**All computation is in Rust**: corpus, layout, camera, zoom folding, pruning,
hit testing. The module emits a command list; the page only paints it.

**Two things stay in the browser**, for one reason: pointed-Hebrew shaping
(bidi + combining marks) lives there and nothing else does it as well.

1. measure the widths Rust asks for;
2. draw the text.

Rust draws first with an estimate, records unknown fragments in `pending`, the
page measures them and asks for a new frame. Each fragment is measured once;
convergence takes one frame.

**No server**: a zoom gesture must answer under 16 ms; a round trip, even
local, is 20–100 ms. Actix only makes sense once the dictionary needs data we
cannot embed.

**No three.js / WebGL**: it would force a glyph atlas and a home-made text
engine — no bidi, no combining marks, so the corpus' million niqqud points
would not render. Canvas 2D gives browser shaping for free.

## Structure

Same layout as check8ge: Rust crate at the root, SvelteKit app in `web/`.

```
Cargo.toml, build.rs, src/      WebAssembly module — all computation
build/mamre.rs                  source reading, included by build.rs
build/variants.rs               the other edition's readings, same inclusion
build/variants.tsv              their table, measured and versioned
src/pack.rs                     the corpus packed for transport, unpacked here
src/{corpus,els,letter,pack,treemap,variant}/tests.rs   module-private tests
src/tests/                      app tests: camera, frame, click, breadcrumb, …
tests/builder.rs                corpus build tests
web/                            SvelteKit 2 + Svelte 5 + Tailwind 4
  src/lib/ethaor.ts             bridge: gestures, painting, measurements
  src/lib/components/TanakhMap.svelte
  src/routes/+page.svelte
  src/lib/ethaor.wasm           the map module, built and copied by `npm run wasm`
  src/lib/ethaor-search.wasm    the search module, likewise, loaded by the worker
  src/lib/ethaor-text*.bin      the corpus' text part: codec, gzip, plain for tiles
  src/lib/search-worker.ts      the search module off the frame loop
  src/lib/search-core.ts        the search module: decode, land, search
  src/lib/text-open.ts          the text read as it downloads, gzip, tiles
  static/fonts/Baloo2-Regular.woff   UI only, Latin subset (see Fonts)
  static/_headers               cache rules for the host (Cloudflare, Netlify)
  scripts/copy-wasm.mjs
  scripts/preload.mjs           preload links in build/index.html (postbuild)
  scripts/bench-load.mjs        the page timed on throttled networks
  scripts/bench-els.mjs         ELS methods timed in V8 (see ELS)
  scripts/tune-els.mjs          ELS cost model tuned on those times
```

**`adapter-static`**: `build/` is plain files, served by any host or CDN.
The app has no server code — map and search both run in the visitor's
browser — so a Node process would only have sent files, and would have had
to be sized, kept up and patched for it; static, any number of visitors costs
the server one download each. Rejected: `adapter-node`, check8ge's choice,
which had nothing to run here. Server code, if the dictionary ever needs it,
goes in a separate service.
`ssr = false` — canvas and module have no business on the server.

CSS is Tailwind everywhere. `app.css` holds only the Tailwind import, the fonts
and two theme tokens (`--font-sans`, `--font-hebrew`).

## Build

```sh
cargo test         # 251 tests, 5 s — before any commit
cd web
npm install
npm run wasm       # both modules (map, search) built and copied into src/lib/
npm run dev        # 127.0.0.1:8095
npm run build      # runs `wasm` first (prebuild), then writes build/
npm run preview    # serves build/ on the dev port — to test, not to host
npm run host       # same, on 0.0.0.0:8080
npm run deploy     # build, then upload build/ to Cloudflare Pages (ethaor)
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
- **The verses go through our own codec** (`pack/codec.rs`, search module): context mixing
  over a Huffman tree per previous symbol, integer-only so the build and the
  browser agree bit for bit, no dependency. Chosen on a measured bench
  against brotli, zstd, LZMA and gzip — a fifth smaller than the best of
  them. Rejected: shipping brotli or zstd ourselves (the browser cannot
  decode them in script, so their decoder would ride in the module) and
  leaving compression to the host (its level is not ours to set).
- **Its efficiency is held, not only its correctness**: a tight ceiling on
  the sealed text (`the_codec_keeps_its_measured_efficiency`), the build
  refusing a codec that no longer beats gzip by a third (`copy-wasm.mjs`),
  and its decode timed in release (`cargo test --release bench_codec --
  --ignored --nocapture`). The time a visitor sees is `bench-load.mjs`'s.
- **Two modules from one crate** (Cargo features): `ethaor.wasm` (`map`)
  is what the page draws with — gestures, frames, tiles, the landing — and
  `ethaor-search.wasm` (`search`) the codec and the searches, ELS first and
  the ones to come. Each build exports its own set and link-time
  optimisation drops the rest, so the search's code is off the path of the
  first image; native builds and the tests take both. `copy-wasm.mjs`
  refuses a module exporting what is not its own.
- **The search module lives in a worker** (`search-worker.ts`), loaded once
  the first image is drawn, with a corpus of its own: neither the text's
  decode nor a search — up to a second over every skip — ever holds a
  frame. The page's `search` is therefore asynchronous, and waits for the
  text. Without a worker the same code (`search-core.ts`) runs in the page.
- **It decodes as the bytes arrive** (`codec::Stream`, `pack::Opening`): a
  symbol never reads past what has arrived, so over a link slower than the
  decoder the decode is over when the download is. Without it a phone added
  its whole decode after the download.
- **The text ships in two forms and the page picks by the bandwidth it
  measured** on the module (`SLOW_LINK`): the codec's, fewest bytes, for slow
  links; gzipped, which the browser decodes natively at once, for fast ones —
  where a phone's processor, not the link, held the codec back. Measured both
  ways on throttled networks and processors.
- **The text is fetched once the first image is drawn**, not alongside it:
  fetched together, it took bandwidth from what the first image waits for —
  a priority hint does not stop that across connections — and measured, the
  map came sooner and the text no later.
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
  — a few kilobytes, one round trip. The whole text still travels in the
  background, compressed: it fills everything else and the ELS needs it.
  Rejected: tiles *instead of* the stream — the codec learns across the
  whole corpus, and cut into hundreds of pieces each would start cold.
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
  lives **at the chapter and nowhere above**: it appears when the chapter under
  the cursor holds another reading. A book or a portion would answer yes nearly
  everywhere — a reading sits in almost every book — and an index always lit
  says nothing; at the chapter the answer is actionable, there is a difference
  *here*. Above that level, and outside every box, it says nothing, as the
  breadcrumb and the click say nothing there. A reading can also be on screen
  while the reader looks elsewhere: explaining a mark he is not looking at is
  noise.
- **The ink is the last slot of a text record**, and it is all the page is
  told: a run is ours or theirs, and which colour stands for which is the
  page's business. Both are drawn at the same strength — the colour alone sets
  them apart. A line is therefore cut into runs at drawing time, laid from the
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
  node scripts/bench-els.mjs ../target/wasm32-unknown-unknown/release/ethaor.wasm ../native.csv > ../v8.csv
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
`yellow-200`, where its text and ours part ways. Rank, level and emphasis keep
to the white; the colour says "another voice", which no shade of white says.
Rejected: setting it apart by the intensity of the white alone — it read as a
faded text rather than as a second reading.

**No frame, no fill.** Neither outline nor plate: sections, books and chapters
share the same black, and only letters carry information. Rank reads in the
**intensity of the white**: section pure white, book one step down (`BOOK_INK`),
chapter number a touch lower (`CHAPTER_INK`). The three parts of the Tanakh are
told apart by the **air above their title** (`SECTION_LEAD`) and by that title
in pure white.

**Hover changes nothing in the drawing**; it only sets the cursor shape. The
command list is bit-identical with and without hover.

Structure labels (titles, numbers) are drawn in world coordinates, so they grow
with the zoom — but `label` drops them as soon as their box exceeds the screen;
beyond that it would only be a giant glyph floating over the current level.
**One exception, at the last zoom-out state**: when the map is just the three
portion names, their size and gap come from the projected world
(`SECTION_CARD`, `CARD_GAP`), not from their band — a section's band is worth
its volume of books, and once the books are gone it says nothing.

Gestures: wheel or pinch to zoom, drag to pan, click for the passage, double
click for the word, Esc for the overview. **One wheel notch is `ZOOM_STEP`**:
from overview to a chapter's text is about ten notches, already long; at a
smaller step it took twice as many and felt like never arriving. A trackpad
sends fractions of a notch, so fine control remains. `touch-action: none` on the
canvas, or the mobile browser intercepts everything.

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
here. `cargo test` holds 251 tests and answers in 5 s, so there is no reason not
to run it at every step.

```sh
cargo test                        # everything
cargo test -- tests::frame        # one panel
cargo test --test builder         # corpus build
```

### What a test asserts here

**Invariants, not snapshots.** A ratio, an order, an equality across the 929
cells — never a list of expected coordinates, which would break at the first
tweak having protected nothing. Numbers belong in tests: a measurement cited
anywhere must be an assertion.

| file | what it holds |
|---|---|
| `src/corpus/tests.rs` | chapters land in any order, each in its place, then the whole text over them gives the corpus byte for byte; a tile's range is its chapters' stretch; counts; monotone bounded index; the hierarchy partitions; traditional cut ranks; numerals avoid the divine name; cleaned text; the `U+05BA` bytes; a fragment is found back from its offset |
| `src/letter/tests.rs` | a letter = a consonant and its marks; lossless splitting over the corpus; marks never detached |
| `src/treemap/tests.rs` | book boxes all identical; **one stride for the 929 chapters**; nesting, non-overlap, reading order; block flush top-right; rows for Tehilim; portion air paid in cells; number/title ratio |
| `src/tests/geometry.rs` | distances, proportional cell padding, attraction beating zoom drift |
| `src/tests/camera.rs` | floored framing, dive in `DIVE_NOTCHES` notches, **numbers at the first notch on nine windows**, non-oscillating detail level, fold lowering the scale, one-way descent |
| `src/tests/frame.rs` | **no fill, no empty frame, no overlaid text** on nine windows and sixty notches; overview title count; three sizes at chapter level; chapter number in the corner and verse numbers at their right once text is out; no number drawn twice; one book-title size per notch, at the bottom of its band, under its ceiling; three portions on zoom-out, equal gaps, single right edge; measurements converge in one frame; record layout, a contract with `ethaor.ts`; time and command budget |
| `src/tests/click.rs` | the click live **exactly** where text is drawn, cell by cell on nine windows and twelve notches; dead at framing and chapter level; the cursor never lies about the click; the passage under the point; the word under the point, from the right verse; word boxes right-to-left, non-overlapping, covering the drawn width |
| `src/tests/breadcrumb.rs` | depth bounded by what is drawn; portion and book at framing, chapter at the first notch, verse with the text; the pointer names the book, cell or line it is on; a portion band names the portion alone; the air names nothing and the centre takes over |
| `src/tests/locate.rs` | every point of a book box names one of its chapters, none in the air between portions; hover says what the click will do |
| `src/tests/text.rs` | one size per chapter; lines recompose the verse **byte for byte**; every line stays a corpus slice; letters right-to-left |
| `src/tests/api.rs` | the interface the page really calls, global state under a lock: gestures, selection, breadcrumb at centre and under pointer, a mouse gone names nothing, forgetting measurements, probes; the search through the query buffer |
| `src/els/tests.rs` | the stream is every verse read letter by letter; postings, residue lists and planes say what the stream says; **every method equals trying every start and every skip**, on random streams of every size and range; the cap keeps the first hits in (skip, start) order with the exact total; two letters counted without listing; the cost model picks the measured winner; radix sort; placement tables; memo; finals and pointing fold; an occurrence across two verses; every record placed where its hit is; `תורה` every fiftieth letter from the first tav; time budget |
| `src/pack/tests.rs` | the packed corpus unpacks to the working form **byte for byte**; the codec round-trips, decodes as the bytes arrive (any cut, really overlapping) and keeps its measured size; the map alone holds names, labels and counts and reserves the text's room; the packed form is a fraction of the working one; Hebrew numerals; label exceptions carry only what breaks the rule; varints |
| `src/tests/landing.rs` | before the text lands the map stays whole — no black frame however deep, no verse drawn, clicked or named — and the verses come once it lands; the overview is identical with and without the text; a chapter's tile is asked two notches or more ahead of its reading level, once, only for chapters on screen, with its exact byte range |
| `src/variant/tests.rs` | the welded table: taking the insertions out gives the verse back byte for byte, every range is a slice on a letter boundary, an offset is mapped back to the verse or refused |
| `src/tests/readings.rs` | the other reading **in the flow, never over it**: the runs of a line tile it from the right and recompose it, the ink marks theirs and only theirs, a verse without a variant stays one command, a click never names the other edition |
| `tests/builder.rs` | `build/mamre.rs` and `build/variants.rs` included, not copied: cp1255 and byte 0xCA, Hebrew numerals, cover page, document order, the four cuts, canonical order; a reading anchored by rank, the display string that gives the verse back |

### Rules for what comes next

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
- **The TypeScript bridge is not covered** — it only measures and paints. Any
  logic appearing there must move down into Rust, or come with its own tests.

## Forbidden

- Computation added or changed without a test (see *Tests*).
- Text alignment delegated to the engine: positions are computed here.
- HTML parsing at run time: that is `build.rs`' job.
- Allocation per letter, word or verse in the frame loop.
- Layout recomputation on zoom.
- Restating a measured value in this file instead of asserting it in a test.
- **All code comments in English** — Rust, TypeScript, Svelte, build scripts.
