# ellie-cli — command-line usage

`ellie-cli` is the Route B command-line companion to the Ellie tray app. It is a
thin second Cargo binary inside `src-tauri` that speaks only to Ellie's
loopback local REST API (`http://127.0.0.1:9876/api/v1`) using the
`ELLIE_API_TOKEN` bearer token. It never touches credentials, SQLite, or Ellie
core internals, and it does not work while the tray app is stopped.

## Executable naming

The tray application owns the `ellie` executable name and its Windows
installer identity (`ellie.exe`). Cargo cannot emit a second binary with the
same name, so the CLI ships as `ellie-cli` (`ellie-cli.exe` on Windows). This
is a deliberate naming decision recorded in PROJECT.md §38.

## Build

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --bin ellie-cli
```

The binary is produced at `src-tauri/target/debug/ellie-cli.exe`. Inclusion of
the CLI in the Windows installer is a Milestone 10 packaging decision and is
not implemented yet; for now the CLI is a developer-built cargo binary.

## Prerequisites

- The Ellie tray app must be running so its local API is reachable at
  `127.0.0.1:9876`.
- The `ELLIE_API_TOKEN` environment variable must be set in the CLI's process
  to the same bearer token the running Ellie app was started with. If the app
  was started without `ELLIE_API_TOKEN`, every API route returns HTTP 503 and
  the CLI reports `Ellie's local API has no token configured`.

The token is sent only in the `Authorization: Bearer` header. The CLI disables
reqwest's automatic system/environment proxy routing so the fixed loopback
request and bearer token cannot be delegated to a configured proxy. It never
prints the token, never logs it, never persists it, and never prints raw API
response bodies or error bodies.

## Commands

```text
ellie-cli status     Show current quota usage per provider
ellie-cli refresh    Trigger a refresh in the running Ellie app, then show usage
ellie-cli version    Print the ellie-cli version
ellie-cli help       Show this help
```

Running `ellie-cli` with no arguments prints the help text. An unknown
command or extra arguments print the help text to stderr and exit with code 2.

### `ellie-cli status`

Fetches `GET /api/v1/usage` and prints one report. Output follows the §38
example shape; provider names are the ones the API reports (for example
`OpenAI / Codex`, `Anthropic / Claude`, `DeepSeek`), the used percentage comes
from each quota window's `usedPercent`, and the reset countdown is computed
locally with chrono from the authoritative `resetAt` timestamp:

```text
Ellie

OpenAI / Codex
  5 Hour    63% used     Reset 2h 14m
  Weekly    42% used     Reset 3d 7h

Anthropic / Claude
  5 Hour    81% used     Reset 3h 22m
  Weekly    54% used     Reset 4d 2h

DeepSeek
  Balance   $8.42
```

Rendering policy:

- The demo provider (`dataKind: "mock"`, `ellie-demo`) is omitted, matching the
  §38 example, which shows only live providers. Providers that have never
  produced data and carry no error are omitted too (the API/dashboard hide
  unconfigured providers the same way). If every provider is omitted, the
  report prints `No provider usage data available`.
- Each quota window prints its label, used percent, and reset countdown.
  Windows whose `source` is `locally_calculated` are labelled
  `(Ellie estimate)`; provider-reported windows carry no suffix.
- A window with no `usedPercent` prints `used: Unavailable`; a window with no
  `resetAt` prints `Reset unknown`.
- A provider with data but no windows and no balance prints
  `No usage data available`. A balance whose currency is missing or empty is
  printed with `(currency unavailable)` rather than as an unlabeled number.
- A provider whose refresh failed while older data exists is shown with its
  data, the current friendly recovery reason (for example,
  `Authentication has expired`), and `Stale — showing data from a previous
  refresh (data from 2h 0m)` style copy. Equivalent auth-state and error copy
  is printed only once, and raw errors are never shown.
- Providers that are not authenticated (auth state or a failed refresh)
  print friendly copy such as `Authentication not configured`,
  `Authentication has expired`, or `Data unavailable` — never raw error
  strings.

Token usage, credits, and spend estimates are not rendered by the CLI yet; the
report covers quota windows and account balance only.

### `ellie-cli refresh`

Posts to `/api/v1/refresh`, which triggers a serialized full refresh in the
running app, then prints the refreshed report:

```text
Refreshed.

Ellie
...
```

If Ellie was already busy refreshing, the heading becomes
`Refresh already in progress.` and the current cached report is printed. A
status request has a 30-second deadline; refresh has a separate six-minute
deadline to cover the coordinator's bounded sequential provider work. Both
retain a short five-second connection deadline for the fixed loopback API.

### `ellie-cli version`

Prints the CLI version from `CARGO_PKG_VERSION` (in lockstep with the app
version), for example:

```text
0.2.0
```

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | Ellie is unreachable: not running, connection refused, timeout, or network error. |
| 2 | Usage or environment error: unknown command/extra arguments, or `ELLIE_API_TOKEN` missing/empty. |
| 3 | Unauthorized: the `ELLIE_API_TOKEN` does not match the running app (HTTP 401/403). |
| 4 | The app's local API has no token configured (HTTP 503 `api_token_not_configured`). |
| 5 | Requested provider not found (HTTP 404; reserved — the current commands do not take a provider argument). |
| 6 | Ellie returned an unexpected response or a server error (other non-2xx status, or a malformed/unparseable body). |

## Error handling and redaction

- The CLI maps HTTP status codes to the friendly copy above; it never reads
  out or prints an API error body (`{"error": "..."}` content is never
  surfaced).
- Transport failures (connection refused, timeout) become friendly copy with
  no underlying error strings.
- The bearer token never appears in CLI output, error messages, or the exit
  code, and it is never echoed to stderr.
- Malformed JSON responses are reported as
  `Ellie returned an unexpected response` (code 6); the CLI keeps working
  without a retry loop or partial data.

## Limitations

- Route B requires the tray app to be running with the token set; there is no
  offline mode (direct-core reuse remains a possible Route A follow-up).
- Provider display names and window labels are copied verbatim from the API;
  the §38 illustration uses shortened names (`OpenAI Codex`, `Claude`).
- Reset countdowns are local chrono computations from the provider-reported
  `resetAt`; they are display-only and not authoritative quota data.
- Token usage, credits, spend estimates, model, and provider history are not
  rendered yet.
- The CLI only reads the cached snapshot for `status`; run `refresh` first if
  you want the freshest values.
- Windows packaging/installer inclusion of `ellie-cli` is not implemented.

## Verification status

Automated checks ran locally (see the git worktree for exact commands and
outputs):

- `cargo fmt --manifest-path src-tauri/Cargo.toml` (and `--check`).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` — clean.
- `cargo test --manifest-path src-tauri/Cargo.toml` — 73 library tests and 29
  `ellie-cli` tests pass.
- `git diff --check` — clean.

`ellie-cli` tests cover serde fixtures (multiple windows, balance rows,
missing/optional fields, empty provider lists), §38-shaped formatting and
reset countdowns, stale/unavailable/empty/mock rendering, balance currency
handling, provenance labels, the error-to-exit-code mapping, independent
status/refresh deadlines, proxy bypass, and end-to-end paths against a tiny
local mock HTTP server (GET/POST success, 401/403, 503, malformed body, and
connection refused when nothing is listening).

Not yet verified: a live `ellie-cli status` / `ellie-cli refresh` run against a
running Ellie tray app with a real `ELLIE_API_TOKEN` still needs a manual
Windows smoke check (requires an owner-configured app instance; never use real
secrets in automated tests).
