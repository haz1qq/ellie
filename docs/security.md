# Security

Milestone 0 has no provider requests, authentication, credentials, telemetry, or local HTTP API. Vite's loopback development server is a development tool and is not part of the built application.

Capabilities are split by local window label. Custom commands must be registered in both the runtime handler and the build-time app manifest, then explicitly allowed by the relevant capability. `main` may read bootstrap/history data, save typed preferences, hide itself, refresh providers, manage local tasks, perform bounded GitHub reads/creation, save/remove provider keys, and report credential sources (never stored values). `mini` may only listen/unlisten for normalized updates, read a sanitized cached mini bootstrap, start native dragging, and restore the main window. `task-note` may listen for its targeted update, drag itself, read/complete/unpin only the current pinned task, and restore the main window; it cannot enumerate or edit other tasks. Auxiliary windows cannot save settings, refresh providers, query analytics, manage GitHub, or access credential commands. No window can execute shell commands or access files, SQL, HTTP plugins, or stored secrets. Windows notifications are dispatched by the Rust-side notification plugin; no frontend notification permission is granted. No remote-origin capability is granted.

The credential commands were initially omitted from the manifest/capability, blocking Save, Remove, and status before the credential store was reached. The credential IPC permission regression tests check these declarations together without accessing real credentials. Native Save/Remove still require a Windows smoke check; mocked frontend calls alone do not verify native permissions.

Provider keys are provider-scoped: `OPENAI_ADMIN_KEY` is only for the separately billed OpenAI Organization Usage API, while the OpenAI / Codex provider reuses `codex login` and never receives a key. An ordinary OpenAI API key must not be substituted for the Admin key. Environment keys take precedence over Credential Manager; key source status is exposed without a secret value.

Settings use a fixed SQL statement with parameters. The IPC settings object rejects unknown fields and invalid types. Appearance preferences include typed booleans and a finite mini-bar opacity bounded to 50–100%; paired physical coordinates are non-sensitive and Rust-owned. Editable settings saves preserve the latest stored coordinates transactionally so a stale frontend draft cannot overwrite a recent drag. `hiddenProviderIds` is a bounded list of unique, nonempty ASCII provider identifiers (maximum 64 IDs, each at most 64 bytes). SQLite stores these presentation preferences and a UTC update timestamp; provider history is stored separately. Visibility only filters cards and never removes credentials or history, changes provider fetching, or grants IPC access. API keys remain in Windows Credential Manager, never the settings table.

Production CSP restricts scripts to bundled code and connections to Tauri IPC. Development CSP additionally allows the loopback Vite server and its hot-reload connection. No external resources or web fonts are loaded. Native errors map to static typed categories; raw SQLite errors, paths, and input values are not logged or returned. Frontend failures show fixed actionable copy, never raw IPC error bodies.

The app fails startup if storage or the tray cannot initialize, avoiding a running app hidden without a working tray. Migrations are transactional and reject newer database versions. Settings saves commit before changing native window behavior. No database-reset fallback deletes user data.

## GitHub and local workspace

GitHub authentication, refresh, API calls, repository creation, and credentials are Rust-owned. The refresh token and runtime-supplied GitHub App Client Secret use separate Windows Credential Manager identities. The Client ID and account identity are non-secret SQLite metadata. React receives only configured flags, connection/account summaries, sanitized repositories/commits, immutable creation reviews, and redacted categories. The low-level code/state completion commands are not exposed through IPC; only `github_sign_in` owns the loopback callback. Requests disable redirects and proxies, use trusted GitHub HTTPS hosts, bounded bodies/pages/timeouts, and never log headers, tokens, private names, or raw bodies.

Personal repository creation is private by default. Rust binds a short-lived review to the exact account/session/inputs, persists minimal non-secret dispatch metadata, and consumes confirmation once. A timeout or lost response becomes an unknown outcome instead of an automatic retry. The user must inspect GitHub and resolve the local warning while connected to the same account; resolution never creates or deletes anything remotely. A confirmed 201 remains success even if local cleanup fails, preventing duplicate retries.

General task IPC is main-window-only and Rust validates IDs, bounded text, explicit Work/Personal type, strict calendar dates, repository identities, and deletion counts. Personal tasks cannot retain repository metadata; repository URLs are derived from validated `owner/name`, so the WebView cannot supply arbitrary URLs. The separate sticky-note commands enforce the `task-note` label and operate only on the current pinned row—no task ID is accepted from that window. Task writes and pin clearing are transactional and run on blocking workers. Task notes, due dates, private repository link snapshots, and sticky-note coordinates are ordinary unencrypted SQLite data protected only by the Windows user boundary, not Credential Manager. Tasks are never exposed through the existing local API/CLI and survive GitHub disconnect.

## Local API authentication (0.3.0)

The API is explicitly opt-in: migration 11 adds `local_api_enabled = 0` for fresh
installs and upgrades. The persisted setting wins over `ELLIE_API_TOKEN`. Disabled
startup does not bind a socket or read the credential store. Only the main window
may invoke `local_api_status` or `configure_local_api` (typed enable/disable/rotate);
both also enforce the `main` window label in Rust. Generic settings cannot set
API enablement. Neither command accepts or returns a token. The mini capability
has no new permissions and cannot control authentication.

Enabling reserves `127.0.0.1:9876`, resolves authentication, commits enablement,
then authorizes requests. Without an override, a missing token is generated from
32 OS-random bytes (`getrandom`) and stored only in Windows Credential Manager
using the existing keyring Windows backend (`ellie` / `local_api_token`). No
provider credential account is reused. Restart never silently replaces a missing
or unreadable token. Credential-store and persistence failures are redacted and
fail closed. In-memory authentication is not Debug/Serialize; only four status
fields cross IPC: enabled preference, listening, token source, and static error.

`ELLIE_API_TOKEN` is an explicit process override, not an enable switch. The shared
Rust resolver checks it before touching the store. Empty/non-Unicode, whitespace,
non-visible-ASCII, or over-512-byte overrides are rejected without fallback;
syntactically valid but mismatched overrides get unauthorized, without retrying
with stored credentials. Override users must choose a strong secret themselves.
The app captures its process override at startup. Unset and restart to switch to
managed credentials. Rotation is blocked while an override exists.

A lifecycle mutex serializes controls, including startup, and operations finish
even if an IPC caller disconnects. Blocking keyring/SQLite calls run on blocking
workers. Windows CredWrite atomically replaces the credential; the new active
token is published only after a successful write, with no subsequent persistence
step to fail. Failed rotation preserves both the old stored and active token.
Each HTTP request checks the current token, including retained keep-alive
connections. Disable revokes authorization before fallible persistence, closes
the listener, and retains the credential. If saving disable fails, runtime access
stays denied but the old enabled preference remains on disk: the UI reports the
failure and asks the user to retry before restart. Requests already authorized
before a successful disable/rotation may finish; subsequent requests are denied
or require the replacement token.

All `/api/v1` routes require bearer authentication, and browser Origin (including
`null`) or Fetch Metadata requests are explicitly rejected, with no CORS layer.
Status distinguishes listening from preference and bind/server/credential errors;
Refresh API status re-reads runtime status. Responses never contain auth secrets,
provider credentials, or raw error bodies. Auth failures return static 401/403
errors; when disabled there normally is no listener (retained connections still
fail the authorization gate). This does not defend against malware already able
to read the same Windows user's Credential Manager.

The HTTP-only `ellie-cli` shares token resolution and the fixed credential account,
not SQLite/provider-core access. It reads stored credentials automatically on a
blocking worker for each invocation unless explicitly overridden. It sends the
token only in Authorization, disables system/environment proxy routing, never
prints tokens or raw error bodies, and uses static redacted credential errors.
Offline help/version do not read credentials or contact the app. See `docs/cli.md`.
Automated tests use only injected mock stores, temporary SQLite files, and
loopback mock servers; native credential operations remain a Windows smoke check.

Logs are structured lifecycle events written to stdout. File logging and log retention are not implemented. The application does not register Windows autostart or change system settings.
