---
name: tests
description: Use whenever a feature is added or changed in ethaor, a bug is fixed, or the test suite is to be strengthened — at any layer (Rust core, shared, WebAssembly exports, server, page, worker, UI). Inventories every behaviour of the feature, maps each to a test in TESTS.md or names the gap, writes the missing tests in the project's style, proves each one red on a deliberate break, regenerates TESTS.md and reads it back, and loops until the feature has no gap. Also use for a "strengthen" pass driven by TESTS.md's undocumented tests and functions no test names.
---

# Tests: every behaviour held, every test proven, the map kept

`TESTS.md` at the repository root is the map of every test, along the file
tree: what each test file covers, each module, each test with its nature
and what it holds, then **the functions no test names**. It is generated
(`npm run tests:map`, from `web/scripts/tests-map.mjs`) and checked by
`npm run verify`: stale, or missing a test the runners know, it fails.

This skill is the loop that keeps it honest. It works with the `faults`
skill: every failure path a feature has is a behaviour to test.

## The loop, for a feature

1. **Read the map.** `cd web && npm run tests:map`, then open `TESTS.md` at
   the feature's files: its tests, their docs, the functions not named.
2. **Inventory the behaviours** of the feature — write the list down before
   any test. For each category below, what does the feature promise?
   - **Contract**: what it gives for a normal input.
   - **Inputs and bounds**: empty, one, many, the maximum, past the maximum,
     the limits of the type (`u32::MAX`, `usize::MAX`, `NaN`, negative),
     malformed (bad UTF-8, unknown names, wrong order), Hebrew specifics
     (final forms, points, maqaf, joined verses).
   - **Invariants over the whole corpus**: every one of the 929 cells, every
     verse, every book; every window of `WINDOWS`; every notch of a sweep.
   - **Order and determinism**: same input, same output; the order promised.
   - **Memory and idempotence**: a repeat answered from memory, a second
     landing changing nothing.
   - **Concurrency**: many callers at once get what each would alone;
     nothing shared left half-written.
   - **Degradation and faults** (`faults` skill): each refusal with its code
     and incident; each bound refused past; the part surviving a failure;
     the log line once, under the number.
   - **Parity**: a computation with a WebAssembly twin and an HTTP twin
     answers the same on both.
   - **Budget**: time or size the feature must stay within.
   - **Regressions**: each bug fixed leaves a test written on its exact case.
3. **Map each behaviour** to a test already in `TESTS.md`, by name — or
   mark it a gap. A test that holds nothing on the list is a question too:
   what is it for?
4. **Write the missing tests** (rules below).
5. **Prove each one**: break the rule it holds in the code, see it red, put
   the code back. A test green on both is prose — rewrite it until it bites.
   A test that hangs instead of failing is a broken test: bound it.
6. **Regenerate and read back**: `npm run tests:map`, then read the
   feature's section of `TESTS.md`:
   - every test has a doc saying what it holds (no `(no doc …)` left);
   - every name says a behaviour, not a function;
   - the nature tags are right (a parity test, a measurement, a panic);
   - every function of the touched files under *Functions no test names*
     is either held by a named test of its caller — say which, in that
     test's doc — or gets a test now.
7. **Gate**: `npm run verify`.
8. **Loop** to step 2 until the inventory has no gap.

## A strengthen pass (no feature in hand)

Pick from `TESTS.md`, in this order, and run the loop on it:
1. tests marked `(no doc: say what it holds)` — read them, write the doc;
   a test whose purpose cannot be written down is rewritten or removed;
2. functions under *Functions no test names*, public ones first;
3. a feature whose tests cover only the contract: add bounds, faults,
   concurrency, parity.

## How a test is written here

- **Invariants, not snapshots**: a ratio, an order, an equality across all
  cells — never a list of expected coordinates.
- **A doc comment** (`///`, or the comment above `it(`) saying what the test
  holds, in one or two sentences; the name says the behaviour
  (`a_flood_is_counted_whole_and_listed_to_the_cap`).
- **Messages in English**, saying what went wrong (`"the engine left with
  the caller"`), never just restating the values.
- **Where**: a module's private test in `src/<mod>/tests.rs`; the app in
  `src/tests/`; the corpus build in `tests/builder.rs`; the server in
  `server/src/tests/`; the page in `web/src/lib/*.test.ts`, on the real
  artifacts.
- **Global state under its lock** (`ONE_AT_A_TIME`); a test never leans on
  another's order.
- **Real data**: fragments from `corpus::text()`, artifacts from
  `tests/artifacts.ts`, real WebAssembly traps (`probe_panic`).
- **Bounded**: a stream or event source read under a timeout; no sleep
  where a condition can be awaited.
- **Measurements** are `#[ignore]` and run by hand; a time budget asserted
  in the suite is loose enough for a debug build and still catches the day
  the work grows with the corpus.

## Never

- A feature merged with a behaviour on its inventory and no test.
- A test not seen red.
- `TESTS.md` edited by hand — regenerate it.
- A test without a doc, or with a French message.
- Deleting or weakening a test to make a change pass: the test is the
  documentation; change it only when the rule it holds changes, and say so.
