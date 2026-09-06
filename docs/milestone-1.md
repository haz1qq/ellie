# Milestone 1 verification

Status: complete on `feat/provider-framework`. Provider integrations have not started; the milestone delivers the provider framework and a clearly marked mock provider only.

## Implemented

- `UsageProvider` async trait with detection, authentication, and usage methods.
- `ProviderRegistry` with per-provider refresh results. A failure does not discard another provider's snapshot.
- Generic `UsageSnapshot`, `UsageWindow`, `TokenUsage`, capability, authentication, provenance, mock/live-state, detection, and typed error models.
- A single `Ellie Demo` provider with clearly marked illustrative data. It performs no I/O, requires no credentials, and does not represent any provider account.
- Bootstrap IPC returns provider overviews with settings. React receives no adapter or authentication implementation details and renders quota windows and token activity only when declared by capabilities.

## Trust behavior

The mock data carries `data_kind: mock`. Every displayed usage value is labeled Demo data or illustrative. The sample allowance uses `provider_reported` only as a model exercise and states that no account is connected. Sample token activity carries `locally_calculated`; it is separately labeled. No React component contains provider-specific API or authentication code.

## Automated checks

All passed on the final state before the completion commit:

- `npm run typecheck`, `npm run lint`, `npm test` (4 tests), `npm run build`
- `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `cargo test` (8 tests)

Smoke check: the native app launched successfully with the tray, dashboard, and settings flows.

## Deferred

Live OpenAI/Codex, Claude, and DeepSeek adapters; account detection; credentials; database history; polling; notifications; and the local API remain out of scope.
