# OpenAI / Codex provider

Provider id: `openai-codex`. Display name: `OpenAI / Codex`.

## Source

Ellie reads ChatGPT plan quota through the **Codex CLI's own app-server**:
`codex app-server --stdio` speaks an MCP-like JSON-RPC 2.0 protocol over
newline-delimited JSON (the `jsonrpc` header is omitted on the wire). Ellie
performs the documented handshake and calls `account/rateLimits/read`, which
returns the ChatGPT rate-limit snapshot used by the Codex CLI itself. It also
best-effort calls `account/usage/read` for Codex's daily token-activity
buckets:

```json
{ "method": "account/rateLimits/read", "id": 2 }
{ "id": 2, "result": {
    "accountId": "…",
    "ordinaryUsageAllowed": true,
    "rateLimits": {
      "planType": "plus",
      "primary":  { "usedPercent": 96, "windowDurationMins": 300,  "resetsAt": 1788714040 },
      "secondary": { "usedPercent": 39, "windowDurationMins": 10080, "resetsAt": 1789199891 },
      "credits": { "balance": "0", "hasCredits": false, "unlimited": false }
    },
    "rateLimitResetCredits": { "availableCount": 0, "credits": null }
} }
```

The method name, request/response shapes, and handshake (`initialize` with
`clientInfo`, then `notifications/initialized`) come from the open-source
Codex CLI repository (`codex-rs/app-server-protocol/...` and the
`codex app-server` README). The wire behavior follows the installed CLI
version rather than a pinned copy, so scheme drift is tied to the user's
actual tool.

Rejected alternatives, recorded here for traceability:

- **`chatgpt.com/backend-api/wham/usage`** (used by several community
  scripts with the `~/.codex/auth.json` bearer token) is undocumented
  internal infrastructure, flagged by its own users as subject to change
  without notice. Not used: Ellie does not call undocumented backend
  endpoints or handle ChatGPT tokens itself.
- **`api.openai.com` organization usage API** returns separately billed org
  token counts by date, not ChatGPT plan 5-hour/weekly quota, reset times, or
  credits. Ellie now exposes it through the separate [OpenAI API billing
  provider](openai-api.md), never by combining it with this card.

## Authentication

- Ellie **reuses the Codex CLI login**: the app-server resolves and refreshes
  the user's ChatGPT OAuth credentials using its own `codex login` flows.
  Ellie never receives, stores, or logs a token.
- `detect()` is heuristic: it checks that a codex launcher exists (`where
  codex`, so npm `.cmd` shims count) and that `$CODEX_HOME/auth.json` (or
  `~/.codex/auth.json`) exists. Codex may store credentials in the OS
  keyring instead, in which case the file check is a false negative; the
  authoritative state comes from the fetch itself.
- Ellie performs no logins. Users authenticate with the official
  `codex login` flows (browser, `--device-auth`, or `--with-api-key`).
- API-key authentication mode has no ChatGPT plan quota; the fetch then
  fails with an `authentication_required`-class error.

## Fields

| Codex field | Ellie field | Notes |
| --- | --- | --- |
| `rateLimits.planType` | `plan` | e.g. `plus`, `pro`, `team` |
| `accountId` | `accountLabel` | Opaque backend account id |
| `rateLimits.primary` | window `id: "primary"` | 5-hour window when `windowDurationMins` ≈ 300 |
| `rateLimits.secondary` | window `id: "secondary"` | Weekly window when `windowDurationMins` ≈ 10080 |
| `window.usedPercent` | `usedPercent` | Provider-reported percent; validated 0–100 |
| `window.resetsAt` | `resetAt` | Unix seconds → `DateTime<Utc>`; converted to local time only for display |
| `window.windowDurationMins` | display label | `300` → `5-hour limit`, `10080` → `Weekly limit`, `1440` → `Daily limit`, `43200` → `Monthly limit`, `525600` → `Annual limit`, else `Quota window (N minutes)` |
| `credits.balance` | `credits` | Display string such as `"$766.76"`; parsed only when it is a numeric amount with currency/digit/separator characters, else `None` |
| `account/usage/read.dailyUsageBuckets[].tokens` | token activity | Provider-reported daily Codex activity; Ellie sums the trailing 30 days |

Hidden behind `rateLimits` (the backward-compatible single-bucket view).
The multi-bucket `rateLimitsByLimitId`, `individualLimit`, `spendControlReached`,
`ordinaryUsageAllowed`, and `rateLimitResetCredits` are not surfaced in v0.1.

## Interpretation

- `usedPercent` comes directly from the provider and is stored with
  `source: provider_reported`. `remainingPercent` is Ellie's complement
  (`100 − usedPercent`); the window's single provenance field remains
  `provider_reported` because the authoritative value is provider-reported,
  and the derived complement is documented here and in the UI copy.
- The primary/secondary windows keep the provider's unit semantics (percent
  of a 5-hour or weekly allowance) and reset identities. No window identity
  is fabricated when fields are absent.
- Missing, null, and zero are kept distinct: a `null` `secondary` yields a
  single window; `credits.balance` `"0"` yields `credits: 0`; missing
  fields yield `None`.
- `data_kind` is `live`; the dashboard shows no mock labels for this card.
- The `account/usage/read` bucket total is an explicit trailing 30-day sum,
  so it is labeled `locally_calculated`; the source daily buckets are
  provider-reported. It has no input/output/cached-token breakdown or API
  billing cost, and it is not an estimate of ChatGPT quota consumption.
- Reset timestamps remain provider-authoritative UTC instants and are rendered
  as a full local date and time, not a time-only label.

## Limitations

- Token activity is best-effort. Older installed Codex versions can reply
  that `account/usage/read` is unavailable; Ellie still renders quota and
  omits the token summary. `cost_tracking` remains `false`.
- `account_balance` is always `false`: the only monetary surface is `credits`.
- One app-server process is spawned per fetch (startup ~1 s); keeping a
  long-lived app-server/daemon connection is deferred to the polling
  milestone.
- Read-only: reset-credit redemption (`account/rateLimitResetCredit/consume`,
  an idempotent mutation) is intentionally not exposed.
- If the installed CLI lacks `account/rateLimits/read`, the server replies
  with a JSON-RPC "method not found", surfaced as provider unavailable.

## Refresh and failures

- The fetch is bounded: a 30 s per-step timeout and a 60 s total timeout;
  the child process is killed and stdout drained so no zombie thread or
  process lingers.
- Errors map to `ProviderError`:
  - `unavailable` — codex launcher missing, spawn failure, timeout,
    malformed reply, out-of-range percent;
  - `authentication_required` — server message mentions login/authentication;
  - `authentication_expired` — server message mentions expiry/unauthorized.
- Detection of failure classes is message-based and conservative; anything
  unrecognized becomes `unavailable` rather than a guessed class.
- Per-account failures are isolated by the registry: a codex failure never
  discards the demo provider's snapshot.

## Windows launcher resolution

npm installs expose `codex` only as `.cmd`/`.ps1`/extensionless shims that
`CreateProcess` cannot execute directly. Resolution order:

1. a native `codex.exe` entry from `where codex` — spawned directly;
2. the vendored native binary inside the npm platform package
   (`node_modules/@openai/codex-win32-x64/vendor/<triple>/bin/codex.exe`),
   discovered next to a `.cmd` shim — spawned directly;
3. `cmd /C codex app-server --stdio` with fixed, literal arguments only;
4. otherwise the provider reports `unavailable`.

All spawned processes use `CREATE_NO_WINDOW` and piped stdio; stderr is
drained and never logged.