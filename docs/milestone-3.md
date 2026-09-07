# Milestone 3 verification

Status: complete on `feat/openai-codex`.

## Implemented

- `OpenAiProvider` (`src-tauri/src/providers/openai.rs`) reads ChatGPT plan
  quota through the Codex CLI's own `codex app-server --stdio` (MCP-like
  JSON-RPC over JSONL): `initialize` + `notifications/initialized`
  handshake, then `account/rateLimits/read` and best-effort
  `account/usage/read`. Registered in the registry alongside the demo
  provider and the separate OpenAI API billing provider.
- **Authentication is reused, never handled**: the app-server resolves and
  refreshes the user's `codex login`; Ellie stores no token. `detect()`
  checks launcher presence and `~/.codex/auth.json`; `authenticate()` is
  reuse-only.
- Normalization: `planType` → plan; `primary`/`secondary` windows →
  `UsageWindow`s with `usedPercent` (provider-reported), `remainingPercent`
  (derived complement), `resetsAt` → UTC reset, and labels derived from
  `windowDurationMins` (5-hour / Weekly / …); `credits.balance` parsed only
  when numeric; `accountId` → account label. Codex daily activity buckets
  are summed into an explicit trailing 30-day locally-calculated aggregate.
  `data_kind: live`.
- Windows launch resolution: `where codex` `.exe` direct, npm `@openai/*`
  vendored native binary next to a `.cmd` shim, or `cmd /C codex` fallback
  with fixed literal args; `CREATE_NO_WINDOW`, drained stderr (never
  logged), 30 s step / 60 s total timeouts, child killed on exit.
- Frontend remains provider-neutral; the usage panel and window cards now
  distinguish mock vs live provenance, and the provider count/bottom note
  reflect live data.

## Research recorded

`docs/providers/openai.md` documents Codex quota/activity, authentication,
field mapping, interpretation, limitations, refresh, failures, and the
Windows launcher resolution. `docs/providers/openai-api.md` documents the
separate documented Organization Usage API, Admin-key authentication, API
billing fields, and limitations. Research sources included the open-source
`openai/codex` repository (app-server protocol JSON schemas, `account.rs`,
`rate_limits.rs`, login manager) and the `codex app-server` README.

## Verification

The first two bullets record the original milestone-3 completion evidence.
The later OpenAI enhancements have separate current verification below.

- Offline: 26 Rust tests (8 new for this milestone) covering sanitized
  response normalization, null/missing handling, window labels, out-of-range
  rejection, credits parsing, error classification, npm/exe launcher
  resolution, and a real-process JSON-RPC exchange against an in-crate mock
  app-server over spawned pipes.
- Frontend: 5 tests, including live-quota rendering with provider
  provenance and no demo labels; typecheck, lint, build pass.
- **Live smoke (authorized local login)**: `ELLIE_LIVE_CODEX=1` provider
  fetch completed in ~1.9 s against the installed Codex CLI, with quota and
  an `account/usage/read` token-activity result normalized into the live
  snapshot. No token or account value was logged.
- **OpenAI API billing**: local-server tests cover its documented response
  shape and pagination. No real OpenAI Admin API key was available, so live
  Organization Usage access remains owner verification.

## Automated checks

Original milestone checks were `cargo fmt --check`,
`cargo clippy --all-targets --all-features`, `cargo test`, `npm run typecheck`,
`npm run lint`, `npm test`, and `npm run build`. Post-milestone OpenAI
extensions currently pass the same commands; exact run evidence is recorded
in the implementation handoff rather than retroactively changing the original
milestone count.

## Deferred

Polling, notifications, reset-credit redemption, and the local API remain
out of scope. Token activity and separately billed OpenAI API usage were
added later as documented post-milestone enhancements.