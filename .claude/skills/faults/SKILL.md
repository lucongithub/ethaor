---
name: faults
description: Use for ANY code written or changed in ethaor, at every layer — Rust core (src/), shared layer, WebAssembly exports, the axum server (server/), the page's TypeScript (web/src/lib), the search worker, the Svelte UI. Makes sure each part is sealed off so the app never crashes but degrades or, at worst, stops gracefully; that every error is numbered (incident), raised under a catalogue code naming its place, escalated layer by layer, logged once with its stack in verbose mode, and told to the user in plain words with the same number. Also use when debugging an incident number (ETH-XXXXXXXX), adding an endpoint, an export, a fetch, a worker message or a UI state.
---

# Faults: sealed, numbered, escalated, told

The contract, at every layer: **nothing a caller, a network or a bug does
takes the application down.** It degrades — refuses, sheds, serves less —
or, when a part cannot go on, stops that part gracefully and says so. Every
error is caught, numbered, passed up with what each layer was doing, logged
once, and told to the user in words they can read, with the number to quote.

## The model (same on both sides)

| piece | what | where |
|---|---|---|
| **incident** | `ETH-` + 8 Crockford base-32 chars, given where the error is raised, never changed | `Incident::new()` · `incident()` |
| **code** | stable, from a catalogue, **raised at one place only** — so it names that place | `CODES` in `server/src/fault.rs` · `CATALOGUE` in `web/src/lib/fault.ts` |
| **at** | `file:line` of the raise: `#[track_caller]`, the JS stack, or the Rust panic's own place | automatic |
| **chain** | one step per layer that passed it on: `{at, what}` | `.escalate(what)` · `escalate()` / `relay()` |
| **message** | what the user reads: plain, no internals; the log records it too | `FAILED` / refusal text · catalogue `message` (English, like the interface) |
| **stack** | backtrace / JS stack, logged **only in verbose** | `ETHAOR_VERBOSE=1` · `?verbose` |

Normal log line: `incident, code, at, status, user_message`. Verbose adds
`chain` and `backtrace`/`stack`. **One line per fault**, written where it is
answered — never once per layer.

Faults from the page reach the same log through `POST /api/report`, under
the number the user saw, with `source: page | worker | wasm`.

## Layer by layer

### Rust core and shared (`src/`, `src/shared/`)
- Bad input is a **`Refusal`**, never a panic: every caller-supplied value is
  checked (`check`, `scope`, `admit`). New refusal → new `Refusal` variant
  with `code()` and `message()`, added to `ERRORS` in `server/src/api/els.rs`.
- No `unwrap`/`expect`/indexing on anything a caller or the network controls.
  Clamp ranges (`min`, `saturating_*`), bound every loop by the data.
- Bound the work before doing it (cost estimate) and the output (`CAP`).
- A panic that still happens is caught by the layer above (pool, wasm hook):
  it must leave nothing shared half-written — or the holder drops it.

### WebAssembly exports (`src/wasm/`)
- Call `super::fault::guard()` first in every export a page calls first
  (`init`, `tile_buffer`, `els_search_in`…): it sets the hook that leaves the
  panic's `file:line:col` and message for the page (`fault_ptr`/`fault_len`).
- Bad arguments answer a sentinel (`0`, `-1`, empty), never panic.
- An `extern "C"` that panics aborts its whole process natively: keep
  exports thin, all logic in `shared` where it is testable.

### Server (`server/`)
- Raise with `Fault::refused` / `Fault::new` / `Fault::failure` /
  `Failure::ours` / `fault::from_panic` — they take the caller's place.
  A failure code goes in `CODES`; `every_code_is_raised_at_one_place` holds
  it. A refusal answers through `refuse(...)`.
- Each layer passing a fault up: `.escalate("what I was doing")`.
- Answer it once, at the boundary: `.respond()` — logs, then answers
  `{"error": {"code", "message", "incident"}}` (`internal` for ours).
- Blocking work in the pool (`pool.run`): panics there are caught, the
  engine dropped, the chain kept. Never block an async thread.
- Bound everything that grows; refuse past it (`too_costly`, `busy`,
  `rate_limited`, `bad_report`) with `Retry-After` where it helps.
- Never `unwrap`/`expect` on a request path. `main` may stop only when the
  port cannot be bound; everything else degrades.

### Page (`web/src/lib/*.ts`)
- New failure → new `CATALOGUE` entry: `show` (told or silent), `fatal`
  (the map cannot go on), `level` (`warn` lived through, `error` lost),
  `message` in English, plain. Raise it at **one** place.
- `raise(code, what)` for a fault born here; `escalate(cause, code, what)`
  for anything caught (a WebAssembly trap becomes its Rust place through
  `readTraps`); `relay(fault, what)` for one born elsewhere (the worker).
- Every promise has a `.catch` that raises; every event handler and frame
  callback is wrapped. Report with `reporter.report(fault)`; show through
  `onFault` when `fault.show`. Never `console.warn` and move on.
- A new WebAssembly module: register `readTraps(...)` for it.
- After a failure that may leave state half-written, stop that part
  (`broken` in the frame loop) rather than loop on the error.

### Worker (`search-worker.ts`)
- Answer **every** request by its id: the result, or `{id, error, fault}`.
  A request left unanswered hangs the page.
- After a trap, read the module's panic before replacing it, then rebuild
  it from what it was fed (`renew`).

### UI (Svelte)
- `fatal` → the whole screen: the message, `incident ETH-…`, a way back
  (reload). Otherwise a notice the user can close. Never a raw error, a
  stack, or a blank screen.

## Before calling a change done

- [ ] Every failure path of the change is listed: what is refused, what is
      bounded, what degrades, what stops — and how the user hears of it.
- [ ] Each new failure has its code in the catalogue, raised once.
- [ ] Each layer it crosses adds a step; it is answered and logged once.
- [ ] The user message is plain, says what still works, carries the number.
- [ ] No panic/unwrap/unbounded wait/unbounded store on a request or frame
      path; logging never on the answering thread.
- [ ] Tests (below) written, and each **seen red** on a deliberate break.
- [ ] `npm run verify` green.

## Tests a fault needs

- The refusal or failure answers with a valid incident, and **the log line
  under that incident** has the code, the `at` of the raise and the message
  the user got (`server/src/tests/faults.rs` shows how: `logged_lines()`, read from the log files).
- Escalation: same incident, steps in order, each at its own line.
- Normal vs verbose fields: chain and stack only in verbose.
- The part survives: after the fault, the next request / search / frame is
  served (or the part is stopped on purpose and says so).
- A panic or WebAssembly trap is traced to its own place
  (`probe_panic` gives a real trap in the page's tests).
- Anything read endlessly (streams, SSE) is read under a timeout: a broken
  rule must fail the suite, not hang it.
- Catalogue tests stay green: one raise site per code, both sides.

## Following an incident

1. The user quotes `ETH-7K3F92QD`.
2. Search the logs for it (`/api/logs` with the token, or the platform):
   one line — code, `at`, what the user read.
3. Need the steps and stack? Run with `ETHAOR_VERBOSE=1` (server) or open
   the page with `?verbose`; the same incident then logs its whole chain.

## Never

- A raw error, a stack or a blank screen shown to a user.
- An error swallowed without a report (`catch {}` with nothing raised) —
  except a report itself failing, which only degrades the log.
- The same fault logged at every layer.
- A code raised at two places, or a failure without a catalogue code.
- A fix of a fault without the test that would have caught it.
