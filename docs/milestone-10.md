# Milestone 10 — Windows Packaging

Status: NSIS packaging configured and release artifact generated; installer install/uninstall smoke verification is pending.

Ellie is configured to produce a Windows NSIS installer from the existing Tauri application:

```powershell
npm run package:windows
```

The command runs the production frontend build, compiles the Rust application in release mode, and creates an installer under `src-tauri/target/release/bundle/nsis/`. The current build produced `Ellie_0.1.0_x64-setup.exe` (4,242,491 bytes; SHA-256 `8982be6d187488ba7ec77232e555177c1ca00ff18d91a9343be6d02b6a121498`). The generated `.exe` remains a local build artifact and is not committed.

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

- installer is generated as an NSIS `.exe`
- install completes in the current-user mode
- Ellie launches from the installed shortcut and shows the tray icon
- Overview, History, Settings, refresh, hide-to-tray, and quit still work
- provider credentials and local history remain available after install/update
- uninstall offers the expected application-data choice and preserves data by default
- no development runtime is needed to launch the installed app

Run the automated checks before packaging:

```powershell
npm run typecheck
npm run lint
npm test
npm run build
npm run check:rust
npm run package:windows
```
