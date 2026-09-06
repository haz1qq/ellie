# Milestone 4 verification

Status: complete on `feat/anthropic-claude` (direction adjusted by the owner
mid-milestone).

## Direction

Milestone 4 started as Claude Code subscription-quota research. Mid-way the
owner cancelled the Claude subscription and chose to **implement the
API-key variant** (documented Anthropic Admin API usage/cost), with
unsubscribed providers **hidden dynamically** and provider configuration /
expired-token UX deferred. Research findings and the pivot are recorded in
`docs/providers/anthropic.md`.

## Implemented

- `AnthropicProvider` (`src-tauri/src/providers/anthropic.rs`): reads the
  documented Usage and Cost Admin API for the trailing 30 days
  (`usage_report/messages` token buckets + `cost_report` amounts in cents),
  bounded pagination (≤4 pages), `x-api-key` + `anthropic-version` headers,
  per-request and overall timeouts. Key from `ANTHROPIC_API_KEY` env only;
  never stored or logged. Normalizes to a `TokenUsage` with
  `source: locally_calculated` (window choice + summation by Ellie) and
  declares `token_usage`/`cost_tracking`, no quota windows.
- `hasSubscription` on `UsageSnapshot` (null = unknown/non-subscription,
  false = explicitly unsubscribed): OpenAI/Codex sets it from `planType`
  (`free` → false, other → true, missing → null); Anthropic and demo → null.
  Persisted via migration `0003_subscription.sql` (schema v3) and
  round-tripped through history.
- Dashboard: providers whose snapshot reports `hasSubscription: false` are
  hidden each render (state derived per refresh, so resubscribing — a new
  snapshot with a paid plan — brings the card back automatically). Count
  text shows "N shown · M unsubscribed" when providers are hidden; copy
  stays provenance-aware.
- New dependency: `reqwest` (rustls) for the Admin API calls — the first
  use of the HTTP layer the architecture deferred until now.

## Research recorded

`docs/providers/anthropic.md` documents: the cancelled-subscription pivot;
the rejected subscription paths (TUI-only `/usage` with open feature
requests, undocumented `api.anthropic.com/api/oauth/usage` + `claude.ai`
variants, opt-in local cost capture); the documented Usage/Cost Admin API
contract; authentication, fields, interpretation, limitations, failures,
and the deferred configuration/token UX.

## Verification

- Offline Rust: 35 tests (7 new) — usage-row summation and cache mapping,
  cents→dollars, missing/non-numeric tolerance, negative rejection, empty
  reports → zero, status classification, missing-key detection, and a
  full-fetch test against an in-process hermetic HTTP server that asserts
  the request URLs/params and the aggregated snapshot.
- Frontend: 6 tests — hidden-unsubscribed card, resubscribe-brings-it-back
  (fresh mount), live/mock provenance copy, and the existing suite.
- No live Admin API call was possible on this machine (no admin key; org is
  a single-user account where the Admin API is unavailable and the
  subscription is cancelled). The unused subscription endpoint
  (`api.anthropic.com/api/oauth/usage`) proved reachable-but-undocumented
  (429 with the OAuth token) and was deliberately not integrated.
  Live verification with a valid admin key is deferred and recorded as a
  limitation.

## Automated checks

- `cargo fmt --check`, `cargo clippy --all-targets --all-features`,
  `cargo test` (35)
- `npm run typecheck`, `npm run lint`, `npm test` (6), `npm run build`

## Deferred

Claude subscription quota (requires an official programmatic surface);
Anthropic key entry UI + keyring storage; management of other
unsubscribed/unconfigured states; polling; notifications; local API.