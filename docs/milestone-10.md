# Milestone 10 — Windows Packaging

Status: complete. NSIS packaging, automated checks, and owner-confirmed Windows installer smoke verification passed.

Ellie is configured to produce a Windows NSIS installer from the existing Tauri application:

```powershell
npm run package:windows
```

The command runs the production frontend build, compiles the Rust application in release mode, and creates an installer under `src-tauri/target/release/bundle/nsis/`. The current build produced `Ellie_0.1.0_x64-setup.exe` (4,242,491 bytes; SHA-256 `8982be6d187488ba7ec77232e555177c1ca00ff18d91a9343be6d02b6a121498`). The generated `.exe` remains a local build artifact and is not committed.

A temporary-directory Windows smoke run installed the package, launched the installed `ellie.exe` successfully, completed silent uninstall, removed the installed files after the uninstaller's cleanup delay, and confirmed the existing `%LOCALAPPDATA%\com.haz1qq.ellie\ellie.sqlite3` remained present. The owner also confirmed the complete manual installer checklist on Windows, including shortcuts, tray behavior, all tabs, refresh, hide-to-tray, quit, credential/history retention, and the interactive uninstall data choice.

## Installer policy

- Product: `Ellie`
- Installer target: NSIS
- Install mode: current user, so administrator access is not required by default
- Start Menu folder: `Ellie`
- Language selector: disabled for the single-language v0.1 interface
- Application icons: `src-tauri/icons/icon.png` and `src-tauri/icons/icon.ico`
- User data: Tauri's default uninstaller preserves application data unless the user explicitly chooses the data-deletion option. Ellie stores its SQLite database under `%LOCALAPPDATA%\com.haz1qq.ellie`; credentials remain in Windows Credential Manager.

The installed executable does not require Rust, Cargo, Node.js, npm, or Python. WebView2 remains a Windows runtime prerequisite for Tauri and is handled by the platform/installer behavior rather than bundled development tooling.

## Verification checklist

On a Windows machine, verify the generated installer and installed app:

- [x] installer is generated as an NSIS `.exe`
- [x] install completes in the current-user mode
- [x] the installed `ellie.exe` launches and remains running during the smoke interval
- [x] silent uninstall completes and removes installed files after cleanup
- [x] existing local SQLite data remains present after uninstall
- [x] Ellie launches from the installed shortcut and shows the tray icon
- [x] Overview, History, Settings, refresh, hide-to-tray, and quit still work in the installed build
- [x] provider credentials and local history remain available after install/update
- [x] uninstall's interactive application-data choice is manually confirmed
- [x] no development runtime is needed to launch the installed app

Run the automated checks before packaging:

```powershell
npm run typecheck
npm run lint
npm test
npm run build
npm run check:rust
npm run package:windows
```
