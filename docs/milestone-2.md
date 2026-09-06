# Milestone 2 verification

Status: complete on `feat/sqlite-history`.

## Implemented

- Versioned migration runner: `src-tauri/migrations/0001_settings.sql` and
  `0002_history.sql` apply in order inside one transaction; reopening is
  idempotent, a newer schema fails safely, and a failed migration rolls the
  schema version and data back.
- Schema version 2 creates the milestone tables: `providers`, `accounts`,
  `usage_snapshots`, `usage_windows`, `token_usage`, `notification_rules`,
  and `notification_state`. The settings table already existed from
  migration 1. Notification tables are created per the milestone migration
  target; their rule/state behavior lands with the notifications milestone.
- `src-tauri/src/history.rs` implements:
  - `insert_snapshot` — provider/account upsert plus snapshot, windows, and
    token usage in one transaction. Invalid snapshots are rejected before
    any write.
  - `latest_snapshot` — most recent snapshot for a provider.
  - `snapshot_history` — newest-first history with optional provider filter
    and limit.
  - `cleanup_history` — deletes snapshots older than the retention window,
    cascading to windows and token usage.
- Default retention is 90 days. A background task on the Tokio runtime runs
  cleanup every 24 hours on a blocking worker (first tick at startup),
  bounded by a 30-second timeout; it never blocks the UI.
- Successful refreshes are persisted to history from the bootstrap IPC on a
  blocking worker. A failing history write logs and cannot fail bootstrap.

## Provenance in storage

- `usage_snapshots.data_kind` stores `live` or `mock`, so Ellie Demo history
  is visibly marked as illustrative at rest. Per the accepted design, all
  snapshots (including demo) are persisted with this marker preserved.
- `usage_windows.source` and `token_usage.source` store
  `provider_reported` vs `locally_calculated` per metric, and reads return
  them untouched.
- Capabilities are stored as JSON with each snapshot (what the provider
  declared at fetch time). Auth state is stored as a stable string.
- Timestamps are UTC ISO 8601 with millisecond precision and a trailing `Z`;
  lexicographic order equals chronological order. No secrets are stored.

## Runtime behavior verified

Launching the current build (including the user's live `tauri dev` session):
the real database at
`%LOCALAPPDATA%\com.haz1qq.ellie\ellie.sqlite3` migrated cleanly from
schema 1 to 2 with the existing settings row preserved. Each app start
persisted an Ellie Demo snapshot (3 observed) with `data_kind=mock`,
`provider_reported` window source, and `locally_calculated` token source.
The startup cleanup pass ran without blocking startup
(`history_cleanup` logged ~15 ms after `app_started`, deleting 0 rows since
all snapshots were fresh).

## Automated checks

All passed on the final state before the completion commits:

- `npm run typecheck`, `npm run lint`, `npm test` (4 tests), `npm run build`
- `cargo fmt --check`, `cargo clippy --all-targets --all-features`,
  `cargo test` (16 tests; 8 pre-existing + 8 new history/storage tests)

History tests cover: insert/read round-trip with provenance preserved, mock
snapshot round-trip with mock provenance, newest-first ordering with limit
and provider filter, retention cleanup deleting only expired snapshots,
account-row deduplication per label, invalid-snapshot rejection before any
write, empty history for unknown providers, migration preserving settings
and history tables from schema 1, and rollback of failed migrations.

## Deferred

Live OpenAI/Codex, Claude, and DeepSeek adapters; account detection;
credentials; polling; notifications; and the local API remain out of scope.
Browser history display lands with the analytics milestone.