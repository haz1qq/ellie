# Ellie

One place to see how much AI you have left.

Ellie is a lightweight, local-first Windows tray app with a quiet black-and-white cat personality. It started from a confirmed **milestone 0** shell and now tracks live AI allowance from OpenAI/Codex, OpenAI API, Anthropic/Claude, and DeepSeek through a command-center dashboard, together with GitHub repository tracking/creation and a local task board.

## Workspace upgrade

The workspace upgrade was delivered in phases: GitHub App authentication, bounded repository/commit reads, secure credential storage, personal repository creation, local lists/tasks, and the library-backed command-center dashboard are implemented and merged to `main` (via PRs #17–#24 from `feat/workspace-github`). The expanded mini-bar HUD (W6) is implemented with optional GitHub-activity and current-task sections that default off. Existing AI monitoring keeps its prior contracts.

Start with the [upgrade plan](docs/workspace-upgrade.md), then the [dashboard/HUD design](docs/workspace-interface.md), [backend/security design](docs/workspace-backend.md), and [GitHub authentication design](docs/workspace-github-auth.md). Live GitHub sign-in, an owner-authorized repository creation, and multi-repository commit loading have succeeded; restart restoration, refresh-token rotation, and disconnect cleanup still require recorded live verification.

## Development

Prerequisites: Windows 10/11, Microsoft C++ Build Tools with Desktop development with C++, WebView2, Rust (MSVC toolchain), Node.js 22.14 or later, and npm 10 or later. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```powershell
npm install
npm run tauri dev
```

Build the Windows installer with:

```powershell
npm run package:windows
```

If PowerShell blocks an unsigned `npm.ps1`, use `npm.cmd` in place of `npm`; no execution-policy changes are necessary. npm is the project package manager. Commit `package-lock.json` and `src-tauri/Cargo.lock`; use `npm ci` for reproducible installs.

`npm run dev` opens only the frontend server at `http://127.0.0.1:1420`. Browser preview is labeled and cannot save desktop settings or control the tray.

If a rebuild reports `failed to remove ... ellie.exe` / `Access is denied (os error 5)`, stop the dev watcher with Ctrl+C, then choose **Quit Ellie** from the tray menu for any remaining Ellie instance. Closing the window normally only hides it. Run `npm run tauri dev` again once Ellie has exited; no database or credential deletion is needed. Native permission changes require a successful rebuild/restart, not just Vite hot reload.

## Current scope

- Library-backed React personal command center across Overview, AI Usage, GitHub, To-do, History, and Settings. Overview derives KPI cards, provider health, token charts, Focus task, upcoming work, scoped commits, the account-wide GitHub contribution calendar, and attention items from typed local/provider data only. Radix UI supplies accessible interaction primitives, Lucide supplies icons, Recharts supplies real-data charts, and Sonner supplies outcome toasts. Opaque fallbacks, reduced transparency/motion, forced colors, keyboard navigation, and narrow layouts remain supported.
- OpenAI / Codex provider: reads ChatGPT plan quota (5-hour and weekly windows, full local reset date/time, plan, credits) and best-effort trailing 30-day Codex token activity through the codex CLI's own `codex app-server` over stdio, reusing `codex login` — Ellie never stores a token. Needs the Codex CLI installed and logged in; gracefully unavailable otherwise. See `docs/providers/openai.md`.
- OpenAI API provider: separately reads 30-day API-billed completion token activity (input/output/cached tokens, requests, dominant model) through the documented Organization Usage API. It requires an OpenAI **Admin API key** (`OPENAI_ADMIN_KEY` or Settings); it does not represent ChatGPT/Codex subscription usage. See `docs/providers/openai-api.md`.
- Anthropic / Claude provider: reads pay-as-you-go usage and cost (30-day window) through the documented Admin API with an `ANTHROPIC_API_KEY` admin key. No subscription windows; unconfigured keys show a clear state. See `docs/providers/anthropic.md`.
- DeepSeek provider: shows the account balance with its real currency and an
  estimated spend, via the documented `GET /user/balance` endpoint
  (`DEEPSEEK_API_KEY` or a saved key). No quota windows exist on DeepSeek, so
  none are shown. See `docs/providers/deepseek.md`.
- Provider credentials in Settings: OpenAI Admin, Anthropic, and DeepSeek API keys are
  saved to Windows Credential Manager (never echoed back); Codex uses your
  `codex login` session directly. Cards show each provider's model in use,
  and live token activity where the provider reports it.
- Unsubscribed **and unconfigured** providers are hidden automatically and reappear when resubscribed or configured (state derived per refresh; `hasSubscription` in snapshots; `authentication_required` errors collapse until configured). Transient failures and expired auth still show their error card.
- **Hide** on any provider card removes only its display, including Ellie Demo. Restore it in **Settings → Provider visibility → Show … on dashboard**. Changes save immediately and survive restarts; fetching, credentials, and history are unchanged. Providers still need an active/configured account before their cards can appear.
- GitHub workspace: runtime GitHub App credentials, Rust-owned loopback sign-in, refresh-token restoration, secure Windows Credential Manager storage, repository browsing, paginated bounded commit history, a Rust-fetched account-wide GitHub profile contribution calendar on Overview (GraphQL, not a local commit total), and private-by-default personal repository creation with Rust-owned prepare/confirm semantics. Unknown creation outcomes require inspection and explicit same-account local resolution; Ellie never retries or deletes a repository automatically. Tokens, callback codes, state, and the saved Client Secret never cross back into the WebView.
- Rust-owned Windows tray: Open Ellie, Refresh, Settings, and Quit Ellie. Refresh uses the same serialized coordinator as dashboard and background refreshes.
- Closing hides to tray by default; with Close to tray disabled, closing the dashboard exits Ellie and closes auxiliary windows. Minimizing uses the normal Windows taskbar. Left-click the cat tray icon to restore the overview; right-click for its menu.
- Optional mini floating bar: a separate always-on-top, taskbar-free window showing live provider-reported quota remaining. It omits providers without quota windows and keeps zero, missing, unavailable, and stale states distinct. Settings controls enablement, 50–100% translucent surface opacity while labels stay opaque, section opt-ins (GitHub activity and current task, default off), and a reset-position recovery; dragging persists a monitor-validated local position, and clicking restores the main Overview. The window sizes itself to the enabled sections (quota 96px, task +52px, GitHub +76px), clamped to the monitor work area. It reuses cached normalized data and provider update events plus the shared GitHub commit cache, so opening it does not trigger provider or GitHub network work.
- Local to-do workspace: one **My tasks** board for explicit Work or Personal tasks, with task name/details, priority, calendar due date, completion, filters, and optional validated GitHub repository snapshots for Work tasks. Pinning an incomplete task shows it both in Overview → Focus and in Ellie's dedicated always-on-top, draggable sticky-note window; completing, deleting, or unpinning it closes the note. Tasks never synchronize to GitHub Issues.
- Task content is stored locally in the versioned SQLite database at `%LOCALAPPDATA%\com.haz1qq.ellie\ellie.sqlite3` and persists until explicit deletion. It is ordinary unencrypted SQLite protected by the Windows user boundary—not Credential Manager. Schema 15 repairs task-list databases created by an earlier workspace development build without losing lists/tasks; schemas 16–17 add explicit task type and sticky-note position.
- Snapshot history: every successful provider refresh is persisted with provenance (`live`/`mock`, `provider_reported`/`locally_calculated`); latest/ history/cleanup storage functions; 90-day retention cleaned up periodically on a background worker.
- **Historical analytics:** the dedicated History tab can query Today, 7-day, 30-day, and 90-day local ranges for latest token/request summaries, daily token activity, currency-grouped spend estimates, and latest provider-reported quota utilization. Mock snapshots and repeated refreshes are excluded or deduplicated, and source labels distinguish provider data from Ellie estimates. See `docs/milestone-9.md`.
- Persisted preferences: close to tray, dashboard mascot, friendly messages, hidden provider cards, and mini floating bar enablement, opacity, and position.
- Structured JSON lifecycle logs to stdout. Background polling runs every five minutes with bounded per-provider backoff. Windows usage notifications use configurable global thresholds (defaulting to 75%, 90%, and 95%) and can be disabled in Settings. The loopback local API on `127.0.0.1:9876` is opt-in through Settings → Integrations (OFF by default, including upgrades).
- `ellie-cli` command-line interface: `status`, `refresh`, and `version` consumed through the loopback local API with an automatically shared Windows Credential Manager token, or an explicit `ELLIE_API_TOKEN` override. It requires the tray app running with Local API enabled, never prints credentials or raw API bodies, and maps failures to documented exit codes. The tray app keeps the `ellie.exe` name; the CLI ships as `ellie-cli`. See `docs/cli.md`.

SQLite lives at the Tauri local application data directory (`%LOCALAPPDATA%\com.haz1qq.ellie\ellie.sqlite3` on Windows). It contains preferences, usage history (including clearly marked demo snapshots), local task content, retained task repository links, and minimal uncertain repository-creation metadata. SQLite is not encrypted; this local workspace data relies on the Windows user account boundary. Credentials never touch it. Failed saves keep prior durable state, and database initialization failures stop startup without overwriting the file.

## Checks

```powershell
npm run typecheck
npm run lint
npm test
npm run build
npm run check:rust
```

See [milestone 3 verification](docs/milestone-3.md), [Milestone 9 analytics](docs/milestone-9.md), and [Milestone 10 packaging](docs/milestone-10.md) for checks actually completed and live smoke-test status. The mini floating bar passed owner-confirmed Windows smoke checks: always-on-top and taskbar-free behavior, 480px sizing, content visibility, dragging and restart persistence, click-to-restore, opacity, and clean exit when Close to tray is disabled. The `ellie-cli` binary is covered by the same `npm run check:rust` flow — fmt, clippy `--all-targets --all-features -D warnings`, and `cargo test` (including 29 `ellie-cli` unit/mock-server tests). A live `ellie-cli status`/`refresh` against a running app with a real token still needs a manual Windows smoke check.

## Project documentation

- [Specification and milestone scope](PROJECT.md)
- [Agent instructions](AGENTS.md)
- [Architecture](docs/architecture.md)
- [Security](docs/security.md)

The cat is an illustrative interpretation, not a reproduction of the real Ellie's markings. Source artwork is in `assets/cat-icon.svg` and `src/components/Cat.tsx`.

## Local API onboarding (0.3.0)

1. Open Ellie → **Settings → Integrations → Enable Local API**. New installations
   and upgrades start **OFF**, even if `ELLIE_API_TOKEN` is already set.
2. Without an override, Ellie generates a 256-bit cryptographically random token
   and stores it only in Windows Credential Manager (`ellie` / `local_api_token`).
   Run `ellie-cli status` or `ellie-cli refresh` as the same Windows user; no token
   copying or environment configuration is needed.
3. Settings reports the persisted preference, actual listening status, token
   source, and redacted errors. **Refresh API status** rechecks the runtime.
   **Rotate API token** replaces the stored token; the next CLI invocation reads
   it automatically. Old tokens stop working for subsequent requests.
4. **Disable Local API** revokes access and closes the listener, including when
   an environment override exists. The stored token is retained for re-enabling.

`ELLIE_API_TOKEN` is an advanced explicit override in each process: it takes
precedence over the stored token, but never enables the API. Empty, non-Unicode,
whitespace-containing, non-visible-ASCII, and over-512-byte overrides fail closed;
there is no silent fallback on invalid or rejected overrides. Valid overrides
must match between app and client. Rotation is unavailable under an override;
unset it and restart Ellie/the CLI environment to restore automatic credentials.
Tokens are never displayed, copied through IPC, or saved in SQLite/configuration.
Already-authorized work may finish after disable/rotation. If disabling cannot
save its preference, access is denied for this run, but retry successfully before
restarting because the previous enabled preference remains on disk.

Automated tests use mock credentials and temporary databases only. Native Windows
Credential Manager onboarding, restart/rotation, and live CLI/provider behavior
remain manual verification; no owner credentials were accessed for these tests.
See [CLI usage](docs/cli.md) and [security](docs/security.md).
