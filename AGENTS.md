# Ellie - Agent Instructions

## Purpose and scope
- Build Ellie: a lightweight, local-first Windows tray app answering, "How much AI usage do I have left?"
- Use Ellie for the product name and `ellie` for repository, CLI, and executable identifiers (`ellie.exe`).
- Read `PROJECT.md` before architectural decisions and `AGENTS.md` before code changes. Inspect relevant code and milestone status; preserve working architecture unless a change is justified.
- Initial providers: OpenAI / Codex, Anthropic / Claude, and DeepSeek. Defer other providers and future features until their milestone is active.

## Personality and visual identity
- Ellie is named after the owner's black-and-white cat: affectionate, clingy, cute, funny, and a little mischievous. Make the app feel like a quiet companion while the user works.
- Apply this identity from the first dashboard shell: ink black, warm white, soft gray, restrained pink accents, a simple black-and-white cat mascot, and a legible cat tray icon. Keep dark mode first and preserve accessible contrast and recognizable status colors.
- Use brief, warm, playful copy sparingly. Pair personality with precise information: "Getting a little low" must accompany the actual provider, window, and remaining allowance. Never replace actionable errors, timestamps, provenance, or numeric labels with jokes or cat metaphors.
- Express clinginess through a welcoming presence, not repeated interruptions, guilt, attention demands, or extra notifications. Keep decorative elements away from primary controls and usage values.
- Include static identity and a small set of friendly messages in v0.1. Defer an animated desktop pet, screen-edge companion, and elaborate reactions until a separately authorized milestone; they must not delay reliable quota monitoring.
- Keep any motion subtle and infrequent, respect reduced-motion preferences, and make companion behavior optional. Do not add background activity solely to animate the mascot.
- Use a simple illustrative mascot until reference photos are provided; do not claim invented markings are Ellie's actual markings. Keep visual assets and copy separate from provider/domain logic.
- This identity extends the original specification's compact dashboard aesthetic. Carry it into `PROJECT.md` and design documentation when those files are maintained.

## Architecture
- Use Tauri 2, Rust, Tokio, reqwest, serde, React, TypeScript, Vite, and npm. Prefer rusqlite for SQLite, Axum for the local API, and tracing for logs. Follow existing dependency choices and lockfiles.
- Keep provider adapters, normalization, persistence, refresh scheduling, and presentation separate. Provider authentication and network communication belong in Rust; the frontend consumes normalized data and authentication state.
- Use a shared provider abstraction and capability declarations. Keep provider-specific parsing and authentication in that provider's module; isolate failures.
- Model generic usage windows, optional balances, credits, and token statistics. Do not hard-code universal five-hour, weekly, or monthly fields.
- Store UTC timestamps; convert only for display. Use versioned database migrations, with no secrets in SQLite. Default history retention is 90 days.
- Refresh asynchronously every five minutes by default, respecting provider minimum intervals. Prevent overlapping refreshes; use bounded timeouts and rate-limit-aware backoff. Preserve the last successful snapshot and expose its age after failures.
- Keep Pi integration separate from the core. When implementing the local API, use `127.0.0.1:9876` and versioned `/api/v1` routes.

## Security
- Store API keys and OAuth secrets in the OS credential store, using Windows Credential Manager or a reputable abstraction backed by it. Never persist plaintext secrets in SQLite, configuration, fixtures, or source control.
- Never log credentials, authorization headers, cookies, or sensitive response bodies. Redact diagnostic data and errors before they reach logs or UI.
- Prefer supported official authentication and secure reuse of existing official-tool authentication. Do not silently extract browser cookies, password databases, local storage, or session tokens.
- Keep undocumented integration research isolated until its security, legitimacy, and maintenance implications are understood. Do not assume example endpoints or authentication flows are production-ready.
- Minimize Tauri capabilities and IPC exposure; validate inputs at trust boundaries. Never return stored credentials through IPC or the local API.
- Keep the API on loopback; do not assume loopback alone prevents hostile browser requests. Validate origins and protect state-changing routes from unauthorized callers. Bound requests and refresh work.
- Keep user data local unless a requested provider operation requires transmission. Do not introduce telemetry or an Ellie cloud dependency.
- Use HTTPS for provider requests and avoid arbitrary shell execution. Sanitize stored responses; keep credentials and identifiable account data out of fixtures and exports.

## Provider-data trust
- Explicitly distinguish `provider_reported` from `locally_calculated` in storage and every consuming interface. Preserve provenance per metric when combining sources.
- Never present local token counts, estimated costs, or inferred resets as official quota information. Label estimates and their limitations.
- Missing, unsupported, stale, and zero are different states. Use optional values and explicit status; never substitute zero or fabricate a quota window.
- Preserve the provider's units, window identity, timestamps, and meaning of used versus remaining. Validate numeric values; derive complements only when the source semantics support them.
- Render only capabilities actually available for the account. Documentation examples and mock data are illustrative, never evidence of live provider support.
- Prefer authoritative reset timestamps or period IDs over percentage drops. Deduplicate notifications per provider, window, period, and threshold.
- Treat provider responses and imported logs as untrusted data. Handle malformed responses, missing fields, expiry, rate limits, and API changes without crashing Ellie or discarding good cached data.

## Coding standards
- Make small, cohesive changes in the existing style. Keep unrelated refactors out of the task; explain new dependencies and architecture changes.
- Use typed Rust errors and strict TypeScript types. Avoid unchecked panics, `unwrap` on external input, broad `any`, and blocking I/O on UI or async executor threads.
- Keep domain logic independent and testable. Prefer React hooks and Context initially; avoid unnecessary state frameworks and abstraction layers.
- Maintain accessible controls and explicit used/remaining labels. Prioritize a compact, clear dashboard with dark mode first.
- Preserve user edits. Never commit secrets or generated build products. Keep documentation and interface contracts aligned with implemented behavior.
- Develop on scoped `feat/*` or `fix/*` branches from `main`; prefer conventional commits. Do not automatically push or merge.

## Milestone discipline
- Work on one agreed milestone or scoped task at a time. Do not continue into the next milestone without explicit instruction.
- Follow the specification sequence: 0 bootstrap; 1 provider framework; 2 SQLite/history; 3 OpenAI/Codex; 4 Claude; 5 DeepSeek; 6 polling; 7 notifications; 8 local API; 9 analytics; 10 Windows packaging. Milestone 0 includes no provider integrations; the future CLI is outside v0.1.
- Deliver a working vertical slice before expanding scope. Keep mocks visibly marked; never mark a provider integration complete using mocks alone.
- Defer floating bars, charts, extra providers, and other future features until scheduled. Do not let optional features block the active milestone.
- Mark completion only when acceptance criteria and relevant checks pass. Record unresolved integration questions and blockers accurately.

## Versioning and releases
- Track Semantic Versioning (`MAJOR.MINOR.PATCH`). While version < 1.0.0, breaking changes bump the minor; from 1.0.0 onward they bump the major.
- `PATCH` fixes regressions without behavior or contract changes; `MINOR` adds backward-compatible features; breaking changes use a major (or a 0.x minor).
- Ellie's consumer-facing contract is the versioned local API (`/api/v1`), the SQLite schema with migrations, and the installer identity. Prefer additive releases; for breaking API changes, add a new versioned route and keep the old one rather than editing it.
- Keep `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` versions in lockstep, plus any expressed API version, before any release build.
- `1.0.0` is a compatibility commitment, not a feature threshold: declare it only when the feature set is stable and the versioned contract is locked, and document the release in `PROJECT.md`/`README.md`.
- Record release checks exactly as run; never claim smoke tests that were not executed.

## Testing and handoff
- Use repository-defined scripts. Milestone completion requires `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test` against the correct manifest, plus configured TypeScript, frontend lint, and relevant test/build checks with npm. Do not claim nonexistent scripts ran.
- Test normalization with sanitized fixtures: absent versus zero, mixed provenance, invalid values, multiple windows, reset boundaries, timezone display, and account-specific capabilities.
- Test meaningful failure paths: expired authentication, rate limits, malformed responses, timeouts, overlapping refresh prevention, and stale-cache preservation.
- Verify migrations and retention on temporary databases; test notification deduplication and API/IPC secret exclusion when touched.
- Smoke-test affected Windows behavior: hide/restore, tray refresh, settings, and clean quit. Live provider checks require available authorized credentials; never use real secrets in fixtures.
- Test affected frontend cards, loading/error states, quota/reset displays, and settings. Update `README.md`, `PROJECT.md`, and relevant architecture, security, or provider docs for non-obvious changes; provider docs must cover sources, authentication, fields, interpretation, limitations, refresh, and failures.
- Report what changed, checks actually run, and remaining limitations. Distinguish implemented, mocked, and unverified behavior; never report tests or milestones as passed without evidence.
