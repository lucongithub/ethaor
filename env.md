# ethaor-server settings

Every setting is an environment variable. Locally they go in `.env` at the
repository root (never versioned). This file lists each variable, its
possible values and what they do.

## How they are read

- The server reads `.env` in the directory it runs from (the repository root
  for `cargo run -p ethaor-server`). `ETHAOR_ENV=path` names another file.
- **A variable already set in the environment always wins** over the file:
  in production, the host's secrets override a forgotten `.env`.
- The file is only read; nothing is written into the environment.
- **Nothing stops the start**:
  - no file: the environment alone, and the log says `no settings file`;
  - a malformed file: ignored whole (half a file is worse than none), and
    the log says `settings file ignored`;
  - a value unreadable, negative, zero or infinite: its default, and the log
    says so.
- At start, the `listening` line recalls the main settings applied (engines,
  maximum cost, verbose, logs endpoint).

A line is `NAME=value`, no spaces around `=`. `#` starts a comment. Quotes
are allowed: `NAME="value"`.

## Serving

| variable | default | values | effect |
|---|---|---|---|
| `ETHAOR_ADDR` | `127.0.0.1:8080` | `127.0.0.1:PORT` · `0.0.0.0:PORT` · `[::]:PORT` | where the server listens: this machine only, every interface (the network, a container), IPv6 too |
| `ETHAOR_ROOT` | `web/build` | a directory | the static build served beside the API; missing, the API still answers |
| `ETHAOR_ENV` | `.env` | a path | another settings file (set it in the environment, not in the file itself) |

**Relative paths start from the directory the server is started in, not
from the executable's**: `ETHAOR_ROOT`, `ETHAOR_LOG_DIR`, and `.env` itself
(or `ETHAOR_ENV`). Started elsewhere, the server finds no `.env` (defaults
only), no build (the API alone answers) and writes its logs there. In a
deployment with `.env` beside the executable, start it from that directory
(`WorkingDirectory=` for systemd, `cd` first on Windows), or give absolute
paths. `ETHAOR_ENV` is read from the environment only: a file cannot name
where it is itself.

## Logs

### `ETHAOR_LOG`: which lines are written

`RUST_LOG` syntax: one level for everything, then, if wanted, a level per
module after a comma. The most specific wins.

| level | what is seen (each level includes the ones above) |
|---|---|
| `error` | only what failed on our side: 5xx answers, panics, failures the page reports |
| `warn` | + what the service lived through: `busy`, `rate_limited`, a page degraded, the logs asked without the token, a bad setting |
| `info` | + every request answered, every search, every refusal — **default** |
| `debug` | + the libraries' own detail (hyper, tower): noisy |
| `trace` | everything: for a bug hunt, never in production |

Per-module examples:

| value | for |
|---|---|
| `warn,ethaor_server::api::els=info` | a quiet log that still keeps every search |
| `info,ethaor_server::api::guard=warn` | the requests, without the budgets' noise |
| `info,hyper=warn,tower=warn` | our detail, not the libraries' |
| `error,ethaor_server::api::report=warn` | only failures, and the page's reports |
| `debug,ethaor_server::api::pool=trace` | looking into the engine pool |

Modules:

| module | what it logs |
|---|---|
| `ethaor_server` | every request answered: method, path, status, time, `request_id` |
| `ethaor_server::fault` | every fault: incident, code, place, user message |
| `ethaor_server::api::els` | every search: word, total, estimate, time |
| `ethaor_server::api::pool` | the engines (panics caught) |
| `ethaor_server::api::guard` | the clients' budgets |
| `ethaor_server::api::report` | the faults the page sends |
| `ethaor_server::api::logs` | the logs endpoint |
| `ethaor_server::config` | settings ignored |

### The other log settings

| variable | default | values | effect |
|---|---|---|---|
| `ETHAOR_LOG_FORMAT` | JSON | `pretty` | standard output only. JSON: one object per line, for a log platform. `pretty`: readable and coloured, for a terminal. The files stay JSON |
| `ETHAOR_LOG_DIR` | `logs` | a directory · `off` | where the log files are written, beside standard output: `ethaor-00000001.log`, `ethaor-00000002.log`… Each start of the server opens a new file, and a full one rolls to the next. Relative paths start from where the server runs. `off`: standard output only, and `GET /api/logs` answers 503 `logs_unavailable`. A directory that cannot be written does the same, and the log says why |
| `ETHAOR_LOG_MB` | `256` | `64` small disk · `256` · `4096` | the disk all the files may take together, shared between 8 files at most: past that the oldest is deleted. Not a positive number: the default |
| `ETHAOR_VERBOSE` | off | `1` | a fault also logs **its whole escalation and its stack**. Without it: incident number, code, `file:line`, the message the user read |
| `ETHAOR_LOGS_TOKEN` | unset: endpoint off | 16 characters or more | the token of `GET /api/logs` (the log files, replayed then followed live), sent as `Authorization: Bearer <token>`. Shorter or unset, the endpoint answers 403: logs hold clients' IP addresses |

To read them in a browser: `http://127.0.0.1:8080/logs/?token=<token>`
(the page served by `ethaor-server`, black on white text). The token is
taken out of the address at once, so the history keeps `/logs/` alone, and
kept for the tab: a reload goes on, a closed tab forgets it. Without
`?token=`, the page asks for it.

Generate a token: `openssl rand -base64 32`, or in PowerShell
`[Convert]::ToBase64String((1..32 | % { Get-Random -Max 256 }))`.

## Capacity

| variable | default | typical values | effect |
|---|---|---|---|
| `ETHAOR_ENGINES` | one per core | `1` … the core count | searches at once, one engine each (~15 MB per warm engine). Fewer to save memory; more is of no use |
| `ETHAOR_WAIT_MS` | `2000` | `500` strict · `2000` · `10000` patient | the longest wait for a free engine before a 503 `busy`. Short: load is shed early and the client retries. Long: fewer refusals, slower answers under load |
| `ETHAOR_CACHE_MB` | `256` | `64` small machine · `256` · `1024`+ large machine | megabytes of JSON answers kept, shared by every caller; a query already asked is served without an engine. `0` is **not** accepted: it falls back to the default |

## Guards

| variable | default | typical values | effect |
|---|---|---|---|
| `ETHAOR_MAX_COST_MS` | `300` | `100` strict · `300` · `100000` lifted | the heaviest search admitted, in milliseconds as the cost model estimates it (V8 time; native runs faster). Past it: 400 `too_costly`, with the estimate and how to narrow the search |
| `ETHAOR_IP_BURST_MS` | `3000` | `3000` · `1000000000` lifted | what one client may spend at once, in the same milliseconds. Always raised to at least the maximum cost admitted |
| `ETHAOR_IP_RATE_MS` | `300` | `300` · `1000000` lifted | what it earns back per second. A request costs 1; a search not in the cache costs its estimate too. Past it: 429 `rate_limited` with `Retry-After` |
| `ETHAOR_TRUST_PROXY` | off | `1` | name the client by the proxy's header (`CF-Connecting-IP`, else the first `X-Forwarded-For` address). **Only behind a proxy that sets them** (Cloudflare, a load balancer), or anyone could pick a fresh budget |

| `ETHAOR_OPEN` | off | `1` | the default browser opens the page once the server listens — for a copy started by a double click. A browser that cannot be opened is logged, and the server goes on |

On/off settings (`ETHAOR_VERBOSE`, `ETHAOR_TRUST_PROXY`, `ETHAOR_OPEN`): `1`, `true` or
`yes` turns them on; anything else leaves them off.

## Recipes

**Everyday development**
```sh
ETHAOR_LOG=info
ETHAOR_LOG_FORMAT=pretty
ETHAOR_LOGS_TOKEN=<a generated token>
```

**Heavy searches by hand** (without being held back)
```sh
ETHAOR_MAX_COST_MS=100000
ETHAOR_IP_BURST_MS=1000000000
ETHAOR_IP_RATE_MS=1000000
```

**Following an incident** (an `ETH-XXXXXXXX` a user quotes)
```sh
ETHAOR_VERBOSE=1
ETHAOR_LOG=debug,hyper=info,tower=info
```
then search the logs for the number, or follow them live:
```powershell
curl.exe -N -H "Authorization: Bearer <token>" "http://127.0.0.1:8080/api/logs?level=warn"
```
In the page, opening the URL with `?verbose` shows the whole escalation in
the console.

**Production behind Cloudflare** (variables set at the host, no `.env`)
```sh
ETHAOR_ADDR=0.0.0.0:8080
ETHAOR_TRUST_PROXY=1
ETHAOR_LOG=info
ETHAOR_LOGS_TOKEN=<the host's secret>
```

**Small machine** (2 vCPU, little memory)
```sh
ETHAOR_ENGINES=2
ETHAOR_CACHE_MB=64
ETHAOR_MAX_COST_MS=150
```
