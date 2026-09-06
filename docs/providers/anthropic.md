# Anthropic / Claude provider

Provider id: `anthropic-claude`. Display name: `Anthropic / Claude`.

## Scope decision

Milestone 4 originally targeted Claude Code subscription quota (the
5-hour/weekly usage ring). That path was researched and then **paused by the
owner after the Claude subscription was cancelled**; this provider is the
**API-key variant**: pay-as-you-go usage and cost through the *documented*
Anthropic Admin API, with no subscription-window tracking. `has_subscription`
is `null` for this provider (not subscription-based).

## Rejected subscription paths (research record)

- **Claude Code `/usage`**: the only subscription-usage surface in the CLI is
  the interactive TUI slash command. It has no machine-readable CLI or file
  output; open feature requests (anthropics/claude-code #21943, #30764) ask
  for exactly that and are unimplemented.
- **`api.anthropic.com/api/oauth/usage`** (the endpoint Claude's HUD uses
  internally): undocumented, and multiple community tools treat it as
  unstable. On this machine the stored `claudeAiOauth` token was expired and
  Anthropic answered with 429 before a schema could be captured. Not used.
- **`claude.ai/api/...` usage variants**: rejected (403 on this account;
  undocumented).
- **Local cost capture** (`~/.claude/history.jsonl` usage recaps): opt-in
  (`usageStats`), currently disabled on this machine, and gives
  locally-estimated cost only — no quota. Not used.

## Source (implemented)

Reads the **Usage and Cost Admin API** (documented at
`docs.anthropic.com/en/docs/manage-claude/usage-cost-api`):

```
GET https://api.anthropic.com/v1/organizations/usage_report/messages?starting_at=…&ending_at=…&bucket_width=1d
GET https://api.anthropic.com/v1/organizations/cost_report?starting_at=…&ending_at=…&bucket_width=1d&limit=31
```

Headers: `x-api-key: <admin key>`, `anthropic-version: 2023-06-01`,
`User-Agent: ellie/<version>`. Both endpoints paginate via
`next_page`/`page`; Ellie follows up to 4 pages per report (bounded).

Response shapes (parsed leniently so unknown fields never crash a fetch):

- usage: `data[].results[]` with `uncached_input_tokens`,
  `cache_read_input_tokens`, `cache_creation.ephemeral_{1h,5m}_input_tokens`,
  `output_tokens` (plus grouping fields ignored here).
- cost: `data[].results[].amount` — a decimal string in lowest currency
  units (cents); `"123.45"` in USD means `$1.23`. USD only.

## Authentication

- The API key comes from the `ANTHROPIC_API_KEY` environment variable and
  must be an **Admin API key** (`sk-ant-admin`) whose user has the
  roles/scopes to read usage and cost reports. The Admin API is unavailable
  for accounts that are not organizations with members.
- Ellie never stores, writes, or logs the key; it is kept in memory for the
  request and dropped.
- Key-entry UI and keyring/Windows Credential Manager storage are deferred;
  `detect()` reports `authentication_required` (with instructions) until the
  variable is set. Token-refresh handling is not applicable (no OAuth here).

## Fields

| Anthropic field | Ellie field | Notes |
| --- | --- | --- |
| summed `uncached_input_tokens` + `cache_read_input_tokens` + `cache_creation.*` | `tokenUsage.inputTokens` | Aggregate over the 30-day window |
| summed `cache_read_input_tokens` + `cache_creation.*` | `tokenUsage.cachedInputTokens` | The cached portion of input |
| summed `output_tokens` | `tokenUsage.outputTokens` | |
| input + output | `tokenUsage.totalTokens` | |
| summed `cost_report` amounts ÷ 100 | `tokenUsage.estimatedCostUsd` | Cents → dollars |
| — | `plan`, `hasSubscription` | `null`/`null`: API billing has no subscription |
| — | `windows` | empty; `quota_windows` capability false |

## Interpretation and provenance

- The Admin API reports per-bucket totals that are authoritative
  (`provider_reported` semantics), but Ellie chooses the 30-day window,
  sums buckets, and converts units; the aggregated `TokenUsage` is therefore
  labeled `source: locally_calculated` and the UI copy marks it as such.
- Missing report fields contribute zero to the sum; non-numeric values are
  treated as missing; negative values reject the report
  (`invalid_snapshot`) rather than being silently included. An empty report
  means zero usage for the window.
- No quota windows, resets, credits, or balances are fabricated; the
  provider renders only token activity and cost.

## Refresh and failures

- Two bounded requests per fetch (30-day trailing window); per-call client
  timeout 20 s with a 60 s overall fetch bound.
- `401` → `authentication_expired`; `403` → `authentication_required`
  (key lacks scope or is not an admin key); network/timeout/`429`/`5xx` →
  `unavailable`; malformed bodies → `unavailable`.
- A failed Anthropic fetch never disturbs the OpenAI/Codex or demo cards
  (registry isolation).

## Limitations

- Shows usage and spend only — no "how much left" windows, because API
  billing has none.
- Requires an org (Admin API unavailable for single-user accounts) and an
  admin key; the owner's current account does not qualify.
- `request_count`, reasoning tokens, and per-model breakdowns are not
  surfaced.
- Desktop app must be restarted if `ANTHROPIC_API_KEY` changes; no settings
  UI yet (deferred per milestone decision).
- Subscription quota (if you resubscribe later) remains out of scope for
  this provider; it would need an official surface Anthropic has not shipped.