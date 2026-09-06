# Architecture

## Milestone 0

The Tauri 2 executable owns application lifecycle, the Windows tray, settings, and SQLite. React consumes three narrow Rust commands through `src/lib/desktop.ts`. The frontend does not receive filesystem paths or arbitrary database access.

| Module | Responsibility |
| --- | --- |
| `src-tauri/src/lib.rs` | Logging, startup, state, close-to-tray lifecycle |
| `src-tauri/src/tray.rs` | Native tray menu, restore/focus, settings navigation, quit |
| `src-tauri/src/commands.rs` | Typed IPC and serialized preference writes |
| `src-tauri/src/storage.rs` | Connection handling and transactional migrations |
| `src-tauri/src/settings.rs` | Non-sensitive, strictly typed preferences |
| `src-tauri/src/error.rs` | Redacted errors |
| `src/App.tsx` | Dashboard and settings views |
| `src/components/Cat.tsx`, `src/copy.ts` | Static identity and optional copy |

Startup initializes SQLite on a blocking worker and waits before exposing the application. Settings reads/writes run on blocking workers; writes are serialized by an async mutex. A native atomic preference controls close behavior and changes only after a successful database write. Connections have bounded busy timeouts and close after each operation.

Migration 1 creates only `application_settings`. Its schema version and SQL change in one transaction. Reopening is idempotent; a newer schema fails safely. Timestamps are UTC ISO 8601. History tables, retention processing, and provider models belong to later milestones.

Tray navigation sets the intended view in native state and emits a window-scoped navigation event. The frontend subscribes before reading initial state, supporting early tray interactions. Left-click restores the overview; the Settings menu opens the settings view. Refresh is disabled because there are no provider adapters.

## Dependencies

Tauri, React, TypeScript, Vite, and npm follow the specification. rusqlite uses bundled SQLite for a predictable Windows build. Tokio provides an async mutex, serde defines the IPC contract, thiserror defines redacted failures, and tracing emits structured lifecycle events. Vitest, Testing Library, and jsdom exercise frontend failure states; tempfile isolates Rust database tests. HTTP, keyring, Axum, and analytics dependencies are deferred until used.

No provider framework or integrations are implemented. Initial provider labels describe the roadmap only, not runtime capabilities. No invented quota windows, balances, estimates, or reset times are displayed.
