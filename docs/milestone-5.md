# Milestone 5 verification

Status: complete on `feat/deepseek`.

## Implemented

- `DeepSeekProvider` (`src-tauri/src/providers/deepseek.rs`): reads the
  account balance through the documented `GET /user/balance` endpoint
  (`https://api.deepseek.com`, `Authorization: Bearer $DEEPSEEK_API_KEY`),
  bounded by per-request (20 s) and overall (60 s) timeouts.
- Balance normalization: USD preferred over other reported currencies,
  `total_balance` parsed strictly (finite, non-negative), unparseable /
  missing / negative → `None` (never fabricated zero). Carries the real
  currency code (`USD`/`CNY`) with the value.
- Declares `account_balance` capability only; no quota windows, credits,
  token totals, or cost are fabricated — per the milestone rule, DeepSeek
  exposes no `5-hour`/`weekly` quota and none is shown.
- New neutral model field `balanceCurrency` (ISO-4217) persisted via
  migration `0004_balance_currency.sql` (schema v4) and round-tripped
  through history; the dashboard renders the balance with its real currency
  (`Intl.NumberFormat`).
- Registered in the registry; missing key → `authentication_required`,
  hidden by the dashboard's unconfigured-provider rule until configured.

## Research recorded

`docs/providers/deepseek.md` documents the official surface, the explicit
decision not to show non-existent quota windows, authentication, field
mapping, interpretation/provenance, failures, and limitations.

## Verification

- Offline Rust: 42 tests (7 new) — USD currency preference, first-currency
  fallback, missing/invalid/negative/zero balance handling, status
  classification, missing-key detection, snapshot shape, and a hermetic
  local-server fetch test asserting the request path and Bearer header.
- Frontend: 8 tests (1 new) — DeepSeek balance card renders the
  provider-reported balance with its currency and no mock/demo labels;
  header detail joins now handle null account/plan.
- No live call was possible on this machine (no `DEEPSEEK_API_KEY` at
  milestone time); live verification with a real key is deferred and
  recorded as a limitation.

## Automated checks

- `cargo fmt --check`, `cargo clippy --all-targets --all-features`,
  `cargo test` (42)
- `npm run typecheck`, `npm run lint`, `npm test` (8), `npm run build`

## Follow-up: credentials UI, model, and spend estimate

Added after the milestone commit on the same branch:

- **Provider credentials in Settings**: Anthropic and DeepSeek API keys are
  saved to Windows Credential Manager (`keyring`) and honored with the env
  vars as fallback; new IPC commands `save_provider_key`,
  `delete_provider_key`, `provider_key_status` (status only — secrets are
  never returned or logged). Codex row explains the reused `codex login`.
- **Spend estimate**: `spendEstimate` on snapshots (schema v5) — the
  positive balance decrease between the oldest stored balance in the
  trailing 30 days and the current balance, computed in
  `attach_spend_estimates` before persistence; requires two prior balance
  points and matching currency; labeled as an estimate.
- **Model**: `model` on snapshots (schema v6) — codex quota-alias model and
  Anthropic dominant model in the report; never fabricated.
- **Token display**: provenance-aware card labels with input/output/cached
  breakdown for Anthropic.
- Tests: 47 Rust (5 new: spend-estimate math + requirements, key-store
  precedence/validation), 10 frontend (token/model/spend rendering,
  settings credential save flow).

## Deferred

Local token/cost accounting in Ellie (analytics milestone); polling;
notifications; local API; packaging.