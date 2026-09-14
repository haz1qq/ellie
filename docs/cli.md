# ellie-cli — command-line usage

`ellie-cli` is the Route B command-line companion to the Ellie tray app. It is a
thin second Cargo binary inside `src-tauri` that speaks only to Ellie's
loopback local REST API (`http://127.0.0.1:9876/api/v1`) using the
shared Windows Credential Manager token or explicit `ELLIE_API_TOKEN` override.
It shares only narrow Rust credential resolution, never SQLite or provider-core
internals, and it does not work while the tray app is stopped.

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

## Onboarding and prerequisites (0.3.0)

- Start Ellie and select **Settings → Integrations → Enable Local API**.
  The API is **OFF by default on fresh installs and upgrades**, even when
  `ELLIE_API_TOKEN` is already configured. Persisted enablement is authoritative.
- Without an override, Ellie securely generates a missing token and saves it only
  in Windows Credential Manager (`ellie` / `local_api_token`). Run the CLI as the
  same Windows user: it automatically reads the stored token on a blocking worker
  for each invocation. There is no token display, copy, export, or clipboard step.
- Settings displays actual listening/error status separately from preference.
  A missing/unreadable token on restart fails closed. Explicit Enable recovers a
  missing token; store errors require fixing credential access before retrying.
- **Rotate API token** atomically replaces the managed token. Subsequent requests
  with the old token are unauthorized; the next CLI invocation reads the new one.
  Failed replacement preserves the old credential and running authentication.
- **Disable Local API** revokes access and closes the listener even under override.
  It retains the credential. If persistence fails, access stays denied for the
  current run but the previous preference remains on disk; retry before restart.
  Already-authorized work may finish after disable/rotation.

### Advanced explicit override

`ELLIE_API_TOKEN` takes precedence in each process, but never enables the API.
When present it must match the running app's override. Empty, non-Unicode,
whitespace-containing, non-visible-ASCII, or over-512-byte values are invalid;
neither app nor CLI silently falls back to stored authentication. A syntactically
valid but mismatched override fails HTTP authorization, without a stored-token
retry. Choose a strong secret if using this advanced path. App environment is
captured at startup; unset the override and restart Ellie/the CLI environment to
return to automatic credentials. Rotation is unavailable under an app override.

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
`OpenAI / Codex`, `Anthropic / Claude`, `DeepSeek`), the remaining percentage
comes from each quota window's `remainingPercent` (derived as `100 - usedPercent`
when only used is reported), and the reset countdown is computed locally with
chrono from the authoritative `resetAt` timestamp:

```text
Ellie

OpenAI / Codex
  5 Hour    37% remaining     Reset 2h 14m
  Weekly    58% remaining     Reset 3d 7h

Anthropic / Claude
  5 Hour    19% remaining     Reset 3h 22m
  Weekly    46% remaining     Reset 4d 2h

DeepSeek
  Balance   $8.42
```

Rendering policy:

- The demo provider (`dataKind: "mock"`, `ellie-demo`) is omitted, matching the
  §38 example, which shows only live providers. Providers that have never
  produced data and carry no error are omitted too (the API/dashboard hide
  unconfigured providers the same way). If every provider is omitted, the
  report prints `No provider usage data available`.
- Each quota window prints its label, remaining percent, and reset countdown.
  Windows whose `source` is `locally_calculated` are labelled
  `(Ellie estimate)`; provider-reported windows carry no suffix.
- A window with neither remaining percent nor used percent prints
  `remaining: Unavailable`; a window with no `resetAt` prints `Reset unknown`.
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
0.3.0
```

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | Ellie is unreachable: not running, connection refused, timeout, or network error. |
| 2 | Usage or credential error: unknown command/extra arguments, invalid explicit override, missing token, or credential-store failure. |
| 3 | Unauthorized: the resolved token does not match the running app, or the request is forbidden (HTTP 401/403). |
| 4 | The app's local API is unavailable (HTTP 503; retained for older servers). Disabled 0.3.0 normally has no listener (code 1); retained connections are denied (code 3). |
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

- Route B requires the tray app to be running with Local API enabled; there is no
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

Automated 0.3.0 validation uses mock credential stores, temporary databases, and
loopback HTTP servers only. Tests cover opt-in upgrade defaults, persisted enable
and restart, disabled-with-override denial, invalid override no-fallback, missing
and failing stores, rotation/replacement resolution, old/new HTTP authorization,
failed rotation rollback, failed preference writes, bind errors, serialized
controls, Origin rejection, secret exclusion, and main-only IPC scope. Existing
CLI HTTP/error/timeout/proxy-bypass and provider normalization tests remain active.
Frontend tests cover onboarding/status, override-disabled rotation, pending and
failed controls, secret-redacted errors, and inert browser preview.

Final automated results: Rust **84 library + 32 CLI tests passed**; frontend
**59 tests across 5 files passed**. Format check, warning-denying clippy, lint,
typecheck, frontend build, CLI build, and diff check passed. Offline CLI help
exited successfully and version printed `0.3.0`. Native/live checks below were
not run.

Commands run for this change:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run lint
npm run typecheck
npm run build
cargo build --manifest-path src-tauri/Cargo.toml --bin ellie-cli
./src-tauri/target/debug/ellie-cli.exe help
./src-tauri/target/debug/ellie-cli.exe version
git diff --check
```

Windows manual verification remains outstanding: native main-only IPC controls,
Credential Manager enable/restart/rotation (including denial of the old token),
CLI status/refresh using automatic authentication and an authorized override,
disable denial with/without override, occupied port recovery, and normal tray
hide/restore/refresh/settings/quit. No real owner credentials were read or changed
by automated tests, and no live provider verification is claimed. Help/version
are offline smokes only. Installer/PATH/autostart changes are outside this task.
