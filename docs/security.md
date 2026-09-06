# Security

Milestone 0 has no provider requests, authentication, credentials, telemetry, or local HTTP API. Vite's loopback development server is a development tool and is not part of the built application.

Only the local `main` window receives capabilities. Custom commands are registered in the build-time app manifest and explicitly allowed by the main capability: read bootstrap settings, save typed boolean settings, and hide the window. The frontend may listen/unlisten for events; it cannot execute shell commands or access files, SQL, HTTP plugins, or stored secrets. No remote-origin capability is granted.

Settings use a fixed SQL statement with parameters. The IPC settings object rejects unknown fields and non-boolean values. SQLite stores only the three implemented preferences and a UTC update timestamp. Credential storage will be designed in provider milestones, backed by the OS credential store, and will never use this settings table.

Production CSP restricts scripts to bundled code and connections to Tauri IPC. Development CSP additionally allows the loopback Vite server and its hot-reload connection. No external resources or web fonts are loaded. Native errors map to static typed categories; raw SQLite errors, paths, and input values are not logged or returned. Frontend failures show fixed actionable copy, never raw IPC error bodies.

The app fails startup if storage or the tray cannot initialize, avoiding a running app hidden without a working tray. Migrations are transactional and reject newer database versions. Settings saves commit before changing native window behavior. No database-reset fallback deletes user data.

Logs are structured lifecycle events written to stdout. File logging and log retention are not implemented. The application does not register Windows autostart or change system settings.
