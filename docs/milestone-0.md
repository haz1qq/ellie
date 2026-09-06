# Milestone 0 verification

Status: milestone 0 complete. Automated checks passed, and the owner confirmed all items in the Windows smoke checklist passed. Milestone 1 has not started.

## Automated checks

Verified on Windows on 2026-09-06:

| Check | Result |
| --- | --- |
| `npm run typecheck` | Passed |
| `npm run lint` | Passed, no warnings |
| `npm test` | Passed, 4 tests |
| `npm run build` | Passed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | Passed |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Passed, 4 tests |

Frontend coverage includes unavailable-provider presentation, browser preview isolation, failed/successful settings saves, redacted errors, and load retry. Rust coverage includes first-run migration, idempotence and persistence, UTC timestamps, rollback on migration failure, newer-schema rejection, and strict settings deserialization. `npm install` reported zero known vulnerabilities at installation time.

## Windows smoke checks

Agent-verified:

- `npm run tauri dev` compiles and launches the native Ellie window.
- Native dashboard loads settings through the restricted IPC contract.
- Settings navigation and a preference save succeed in the native webview.
- Disabling friendly messages changes the UI and persists after a webview reload and a development-process restart. The original setting was restored after testing.
- Closing the window removes it from the visible window list while the same Ellie process remains running.
- Browser preview layout inspected at desktop width and at 420px. No invented quota bars or values appear; controls and provider labels remain readable.

Owner-verified (reported “all pass” in response to the Windows smoke checklist):

- Minimize and restore through the taskbar.
- Left-click the cat tray icon to restore and focus Overview.
- Right-click the tray icon: Open Ellie and Settings work; Refresh is disabled.
- Hide to tray button and close-to-tray disabled behavior.
- Quit Ellie removes both the process and tray icon cleanly.
- Dashboard mascot and friendly-message preferences save and survive restarting.

The tray results above are owner-reported, not agent-automated. The Windows automation tool did not expose the notification area, and native screenshot capture showed an overlapping window. Browser layout verification does not count as native tray visual verification. Detailed keyboard focus/navigation and icon contrast on both light and dark taskbars were not individually covered by the reported checklist and remain additional accessibility review items.

During development, the frontend server uses `127.0.0.1:1420` while `npm run tauri dev` runs.

## Scope limits

Provider integrations, mock quotas, history/retention, polling, notifications, API, CLI, installers, and animated companion behavior are not implemented. Default future polling (5 minutes) and retention (90 days) remain specifications, not active background jobs.
