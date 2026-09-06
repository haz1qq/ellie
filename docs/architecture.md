# Architecture

The Tauri 2 executable owns application lifecycle, the Windows tray, settings, SQLite, and the provider framework. React consumes three narrow Rust commands through `src/lib/desktop.ts`. The frontend does not receive filesystem paths, raw database access, or provider authentication.

| Module | Responsibility |
| --- | --- |
| `src-tauri/src/lib.rs` | Logging, startup, state, close-to-tray lifecycle |
| `src-tauri/src/tray.rs` | Native tray menu, restore/focus, settings navigation, quit |
| `src-tauri/src/commands.rs` | Typed IPC and serialized preference writes |
| `src-tauri/src/storage.rs` | Connection handling and transactional migrations |
| `src-tauri/src/history.rs` | Snapshot persistence, retrieval, and 90-day retention |
| `src-tauri/src/providers/` | Provider abstraction, registry, models, and adapters (mock, OpenAI/Codex, Anthropic/Claude, DeepSeek) |
| `src-tauri/src/settings.rs` | Non-sensitive, strictly typed preferences |
| `src-tauri/src/error.rs` | Redacted errors |
| `src/App.tsx` | Dashboard and settings views |
| `src/components/Cat.tsx`, `src/copy.ts` | Static identity and optional copy |

Startup initializes SQLite on a blocking worker and waits before exposing the application. Settings reads/writes run on blocking workers; writes are serialized by an async mutex. A native atomic preference controls close behavior and changes only after a successful database write. Connections have bounded busy timeouts and close after each operation.

Migrations apply in order inside one transaction: 1 creates `application_settings`, 2 creates `providers`, `accounts`, `usage_snapshots`, `usage_windows`, `token_usage`, and notification table stubs. Reopening is idempotent; a newer schema fails safely and a failed migration rolls back. Timestamps are UTC ISO 8601. Every successful provider refresh is persisted with provenance (`data_kind` per snapshot; `provider_reported`/`locally_calculated` per metric); the latest snapshot, filtered/limited history, and retention cleanup (90 days default, run periodically on a background worker) live in `history.rs`.

Tray navigation sets the intended view in native state and emits a window-scoped navigation event. The frontend subscribes before reading initial state, supporting early tray interactions. Left-click restores the overview; the Settings menu opens the settings view. Refresh is disabled because there are no provider adapters.

## Dependencies

Tauri, React, TypeScript, Vite, and npm follow the specification. rusqlite uses bundled SQLite for a predictable Windows build. Tokio provides an async mutex and timers, serde defines the IPC contract, thiserror defines redacted failures, and tracing emits structured lifecycle events. Chrono handles UTC timestamps. Reqwest (rustls) serves the Anthropic Admin API calls. Vitest, Testing Library, and jsdom exercise frontend failure states; tempfile isolates Rust database tests. Keyring, Axum, and analytics dependencies are deferred until used.

The provider framework (trait, registry, capabilities, provenance models, typed errors) is implemented with a clearly marked Ellie Demo mock adapter plus live adapters for OpenAI / Codex (codex app-server over stdio), Anthropic / Claude (documented Admin API usage and cost), and DeepSeek (documented balance endpoint). Authentication is reused (`codex login`) or env-key-based (`ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`); Ellie never stores tokens or logs secrets. Snapshots carry an explicit subscription flag so unsubscribed providers hide and reappear on resubscription, and unconfigured providers collapse until configured. No other real provider integrations exist, and other initial provider labels describe the roadmap only, not runtime capabilities. No invented quota windows, balances, estimates, or reset times are displayed as account data.
