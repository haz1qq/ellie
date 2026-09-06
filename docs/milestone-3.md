# Milestone 3 verification

Status: complete on `feat/openai-codex`.

## Implemented

- `OpenAiProvider` (`src-tauri/src/providers/openai.rs`) reads ChatGPT plan
  quota through the Codex CLI's own `codex app-server --stdio` (MCP-like
  JSON-RPC over JSONL): `initialize` + `notifications/initialized`
  handshake, then `account/rateLimits/read`. Registered in the registry
  alongside the demo provider.
- **Authentication is reused, never handled**: the app-server resolves and
  refreshes the user's `codex login`; Ellie stores no token. `detect()`
  checks launcher presence and `~/.codex/auth.json`; `authenticate()` is
  reuse-only.
- Normalization: `planType` → plan; `primary`/`secondary` windows →
  `UsageWindow`s with `usedPercent` (provider-reported), `remainingPercent`
  (derived complement), `resetsAt` → UTC reset, and labels derived from
  `windowDurationMins` (5-hour / Weekly / …); `credits.balance` parsed only
  when numeric; `accountId` → account label. `data_kind: live`.
- Windows launch resolution: `where codex` `.exe` direct, npm `@openai/*`
  vendored native binary next to a `.cmd` shim, or `cmd /C codex` fallback
  with fixed literal args; `CREATE_NO_WINDOW`, drained stderr (never
  logged), 30 s step / 60 s total timeouts, child killed on exit.
- Frontend remains provider-neutral; the usage panel and window cards now
  distinguish mock vs live provenance, and the provider count/bottom note
  reflect live data.

## Research recorded

`docs/providers/openai.md` documents the source, the rejected alternatives
(undocumented `wham/usage`; documented platform usage API), authentication,
field mapping, interpretation, limitations, refresh, failures, and the
Windows launcher resolution. Research sources included the open-source
`openai/codex` repository (app-server protocol JSON schemas, `account.rs`,
`rate_limits.rs`, login manager) and the `codex app-server` README.

## Verification

- Offline: 26 Rust tests (8 new for this milestone) covering sanitized
  response normalization, null/missing handling, window labels, out-of-range
  rejection, credits parsing, error classification, npm/exe launcher
  resolution, and a real-process JSON-RPC exchange against an in-crate mock
  app-server over spawned pipes.
- Frontend: 5 tests, including live-quota rendering with provider
  provenance and no demo labels; typecheck, lint, build pass.
- **Live smoke (authorized local login)**: `ELLIE_LIVE_CODEX=1` provider
  fetch completed in ~1.2 s against codex-cli 0.153.4 on this machine:
  `planType: plus`, primary `96%` (300 min), secondary `39%` (10080 min),
  `credits.balance "0"`, account uuid — normalized to a live snapshot with
  two windows and persisting through the existing history pipeline.

## Automated checks

- `cargo fmt --check`, `cargo clippy --all-targets --all-features`,
  `cargo test` (26; 24 pre-existing + 2 new offline + 1 env-gated live test)
- `npm run typecheck`, `npm run lint`, `npm test` (5), `npm run build`

## Deferred

Polling, notifications, token-activity display (`account/usage/read`), API
key-originated org usage, reset-credit redemption, and the local API remain
out of scope.