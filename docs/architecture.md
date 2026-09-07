# Architecture

The Tauri 2 executable owns application lifecycle, the Windows tray, settings, SQLite, and the provider framework. React consumes narrow Rust commands through `src/lib/desktop.ts`. The frontend does not receive filesystem paths, raw database access, or provider authentication.

| Module | Responsibility |
| --- | --- |
| `src-tauri/src/lib.rs` | Logging, startup, state, close-to-tray lifecycle |
| `src-tauri/src/tray.rs` | Native tray menu, restore/focus, settings navigation, quit |
| `src-tauri/src/commands.rs` | Typed IPC, serialized preference writes, and refresh commands |
| `src-tauri/src/storage.rs` | Connection handling and transactional migrations |
| `src-tauri/src/credentials.rs` | Windows Credential Manager (keyring) key storage and key IPC status |
| `src-tauri/src/history.rs` | Snapshot persistence, retrieval, and 90-day retention |
| `src-tauri/src/refresh.rs` | Serialized refresh coordinator, cache, polling, and provider backoff |
| `src-tauri/src/providers/` | Provider abstraction, registry, models, and adapters (mock, OpenAI/Codex, Anthropic/Claude, DeepSeek) |
| `src-tauri/src/settings.rs` | Non-sensitive, strictly typed preferences |
| `src-tauri/src/error.rs` | Redacted errors |
| `src/App.tsx` | Dashboard and settings views |
| `src/components/Cat.tsx`, `src/copy.ts` | Static identity and optional copy |

Startup initializes SQLite on a blocking worker and waits before exposing the application. Settings reads/writes run on blocking workers; writes are serialized by an async mutex. A native atomic preference controls close behavior and changes only after a successful database write. Connections have bounded busy timeouts and close after each operation.

Migrations apply in order inside one transaction: 1 creates `application_settings`, 2 creates `providers`, `accounts`, `usage_snapshots`, `usage_windows`, `token_usage`, and notification tables, 7 adds provider visibility, 8 adds the notification enable/disable preference, and 9 adds global notification thresholds. Reopening is idempotent; a newer schema fails safely and a failed migration rolls back. Timestamps are UTC ISO 8601. Every successful provider refresh is persisted with provenance (`data_kind` per snapshot; `provider_reported`/`locally_calculated` per metric); the latest snapshot, filtered/limited history, and retention cleanup (90 days default, run periodically on a background worker) live in `history.rs`.

Tray navigation sets the intended view in native state and emits a window-scoped navigation event. The frontend subscribes before reading initial state, supporting early tray interactions. Left-click restores the overview; the Settings menu opens the settings view. Refresh is enabled and routes through the same refresh coordinator as the dashboard and automatic poller.

## Provider visibility

`Settings.hiddenProviderIds` is a bounded, unique list of provider IDs. Migration 7 adds a JSON-array column to `application_settings`, defaulting to `[]` (no manually hidden cards) for existing users. The existing `save_settings` command persists it with the appearance preferences, using the same serialized, blocking-worker write path. Invalid IDs, duplicates, and overlong lists are rejected before storage.

Each `ProviderOverview` includes registry-owned `providerId` and `displayName` even on fetch failure. This lets the UI offer visibility controls for every registered provider without hard-coding a provider catalog or depending on a successful snapshot. **Hide** on a card and **Show … on dashboard** in Settings save immediately. UI state changes only after a successful write; changing visibility preserves unsaved appearance edits.

Visibility is presentation-only: adapters still fetch, successful snapshots still persist, and no credentials/history are deleted. All providers, including Ellie Demo and unconfigured/failed providers, remain listed in Settings. Enabling display does not override automatic hiding for `hasSubscription === false` or `authentication_required`. No new IPC commands or permissions are needed.

Temporary-database tests cover migration from schema 6, default visibility, persistence after reopening, restoration without deleting history, and invalid-input rejection. UI tests cover hide/restore, persisted preferences, failed saves/restores, in-flight disabled controls, failed-provider identity, and automatic authentication hiding. Native Windows visibility/restart smoke verification passed.

## Dependencies

Tauri, React, TypeScript, Vite, and npm follow the specification. rusqlite uses bundled SQLite for a predictable Windows build. Tokio provides an async mutex and timers, serde defines the IPC contract, thiserror defines redacted failures, and tracing emits structured lifecycle events. Chrono handles UTC timestamps. Reqwest (rustls) serves the Anthropic Admin API calls; keyring persists provider API keys in Windows Credential Manager. Vitest, Testing Library, and jsdom exercise frontend failure states; tempfile isolates Rust database tests. Axum and analytics dependencies are deferred until used.

The provider framework (trait, registry, capabilities, provenance models, typed errors) is implemented with a clearly marked Ellie Demo mock adapter plus live adapters for OpenAI / Codex (codex app-server quota plus best-effort daily token activity over stdio), OpenAI API (documented Organization Usage API billing activity), Anthropic / Claude (documented Admin API usage and cost), and DeepSeek (documented balance endpoint). Authentication is reused (`codex login`) or key-based (`OPENAI_ADMIN_KEY`, `ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`); Ellie never stores tokens or logs secrets. OpenAI API billing is a separate provider/card so it is never combined with ChatGPT/Codex subscription quota or activity. Snapshots carry an explicit subscription flag so unsubscribed providers hide and reappear on resubscription, and unconfigured providers collapse until configured. No invented quota windows, balances, estimates, or reset times are displayed as account data.

## Notifications

Milestone 7 uses the existing `notification_state` table to claim one notification per provider, quota window, threshold, and quota period. The initial global thresholds are 75%, 90%, and 95% used; users can adjust all three values in Settings. A period key prefers the provider's authoritative reset timestamp and falls back to the window start timestamp; windows with neither are not notified rather than receiving fabricated reset identity. Only live, non-stale, provider-reported percentage data can trigger a notification, so demo snapshots and locally calculated estimates never appear as official alerts.

Notification state is claimed transactionally before dispatch to Windows through the Rust-side Tauri notification plugin. Repeated polling and manual refreshes therefore do not duplicate alerts. The Settings toggle controls all usage notifications and defaults to enabled for existing and new installations. Settings also persists three globally shared thresholds, requiring them to remain strictly ascending between 1% and 100%. Notification text includes the provider, window, threshold, remaining percentage when reported, and reset timestamp when available. No frontend notification IPC permission is granted because notification dispatch remains Rust-owned.

## Refresh coordinator

`RefreshCoordinator` owns one async critical section, a per-provider cached `ProviderOverview`, and bounded exponential retry state. Startup performs a forced refresh. The background poller waits five minutes between cycles; each cycle skips providers whose retry deadline has not elapsed. Dashboard Refresh, per-card Refresh, and the tray Refresh all share the same non-overlapping coordinator. A concurrent request returns the cached results with `busy: true` rather than starting another provider request.

A successful fetch replaces the cached snapshot and clears that provider's backoff. A failed fetch retains the last successful snapshot, marks the overview `stale`, preserves the typed error, records `lastSuccessfulRefresh`, and exposes `nextRetryAt`. If no in-memory value exists after restart, the coordinator attempts to load the latest persisted snapshot before showing an error-only result. Only fresh successful snapshots are written to history.

Provider fetches already have adapter-level bounded timeouts. The coordinator adds retry delays of 30 seconds, 60 seconds, 120 seconds, and so on up to 30 minutes. Explicit dashboard/tray refresh bypasses a provider's backoff, while polling honors it. Provider failures remain isolated: one failed provider cannot prevent other providers from refreshing or updating the UI.
