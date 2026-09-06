# Ellie

One place to see how much AI you have left.

Ellie is a lightweight, local-first Windows tray app with a quiet black-and-white cat personality. **Milestone 0 is complete**, with automated checks passed and Windows smoke checks confirmed by the owner. The shell does not connect to providers or show sample quota values.

## Development

Prerequisites: Windows 10/11, Microsoft C++ Build Tools with Desktop development with C++, WebView2, Rust (MSVC toolchain), Node.js 22.14 or later, and npm 10 or later. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```powershell
npm install
npm run tauri dev
```

If PowerShell blocks an unsigned `npm.ps1`, use `npm.cmd` in place of `npm`; no execution-policy changes are necessary. npm is the project package manager. Commit `package-lock.json` and `src-tauri/Cargo.lock`; use `npm ci` for reproducible installs.

`npm run dev` opens only the frontend server at `http://127.0.0.1:1420`. Browser preview is labeled and cannot save desktop settings or control the tray.

## Current scope

- Dark React dashboard with a static illustrative cat and optional friendly copy.
- Rust-owned Windows tray: Open Ellie, Settings, disabled Refresh, and Quit Ellie.
- Closing hides to tray by default; the Close to tray preference can disable this behavior. Minimizing uses the normal Windows taskbar. Left-click the cat tray icon to restore the overview; right-click for its menu.
- Local SQLite initialization with a transactional, versioned settings migration.
- Three persisted preferences: close to tray, dashboard mascot, and friendly messages.
- Structured JSON lifecycle logs to stdout. No provider calls, credentials, telemetry, history, polling, or local API in this milestone.

SQLite lives at the Tauri local application data directory (`%LOCALAPPDATA%\com.haz1qq.ellie\ellie.sqlite3` on Windows). It contains only non-sensitive preferences. A failed settings save keeps the previous settings active. Database initialization failures stop startup without overwriting the file.

## Checks

```powershell
npm run typecheck
npm run lint
npm test
npm run build
npm run check:rust
```

See [milestone 0 verification](docs/milestone-0.md) for checks actually completed and Windows smoke-test status. Packaging is milestone 10; bundling is intentionally disabled.

## Project documentation

- [Specification and milestone scope](PROJECT.md)
- [Agent instructions](AGENTS.md)
- [Architecture](docs/architecture.md)
- [Security](docs/security.md)

The cat is an illustrative interpretation, not a reproduction of the real Ellie's markings. Source artwork is in `assets/cat-icon.svg` and `src/components/Cat.tsx`.
