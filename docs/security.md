# Security

Milestone 0 has no provider requests, authentication, credentials, telemetry, or local HTTP API. Vite's loopback development server is a development tool and is not part of the built application.

Only the local `main` window receives capabilities. Custom commands must be registered in both the runtime handler and the build-time app manifest, then explicitly allowed by `capabilities/main.json`. The allowed commands read bootstrap data, save typed boolean settings, hide the window, save/remove provider keys, and report credential sources (never stored key values). The frontend may listen/unlisten for events; it cannot execute shell commands or access files, SQL, HTTP plugins, or stored secrets. No remote-origin capability is granted.

The credential commands were initially omitted from the manifest/capability, blocking Save, Remove, and status before the credential store was reached. The credential IPC permission regression tests check these declarations together without accessing real credentials. Native Save/Remove still require a Windows smoke check; mocked frontend calls alone do not verify native permissions.

Settings use a fixed SQL statement with parameters. The IPC settings object rejects unknown fields and invalid types. Appearance preferences are booleans; `hiddenProviderIds` is a bounded list of unique, nonempty ASCII provider identifiers (maximum 64 IDs, each at most 64 bytes). SQLite stores these presentation preferences and a UTC update timestamp; provider history is stored separately. Visibility only filters cards and never removes credentials or history, changes provider fetching, or grants IPC access. API keys remain in Windows Credential Manager, never the settings table.

Production CSP restricts scripts to bundled code and connections to Tauri IPC. Development CSP additionally allows the loopback Vite server and its hot-reload connection. No external resources or web fonts are loaded. Native errors map to static typed categories; raw SQLite errors, paths, and input values are not logged or returned. Frontend failures show fixed actionable copy, never raw IPC error bodies.

The app fails startup if storage or the tray cannot initialize, avoiding a running app hidden without a working tray. Migrations are transactional and reject newer database versions. Settings saves commit before changing native window behavior. No database-reset fallback deletes user data.

Logs are structured lifecycle events written to stdout. File logging and log retention are not implemented. The application does not register Windows autostart or change system settings.
