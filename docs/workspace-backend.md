# Workspace backend design

**Status: implemented through W4; expanded HUD backend pending.** Companion to the [workspace upgrade plan](workspace-upgrade.md) and [interface design](workspace-interface.md). The GitHub authentication/read service, personal repository prepare/confirm flow, and local task persistence are implemented on `feat/workspace-github`; live remote-creation verification remains outstanding.

## Existing boundaries and planned extension

Relevant integration points are `src-tauri/src/{storage,commands,refresh,mini_bar,credentials,local_api,tray}.rs` and the main/mini Tauri capabilities. Retain existing AI normalization, provider registry, history, polling, notification deduplication, credential storage, local API and CLI contracts.

GitHub is a separate workspace service, not a `UsageProvider` or GitHub Copilot integration. Give it independent connection state, HTTP client, sync lock, bounded retry state, cache and account/session generation. Do not route commits through usage snapshots or the existing provider update event. Local tasks have a separate persistence service and no network dependency.

All blocking database/keyring work stays off UI and async executor threads. GitHub failures cannot delay app startup or prevent AI refresh and task editing.

## Authentication feasibility gate (W1)

Before implementation, verify against official GitHub documentation:

- GitHub App versus OAuth suitability for an installed desktop application.
- Device authorization or PKCE support as applicable; registration/distribution and callback requirements.
- Exact read permissions for authorized public/private repositories and commits, and exact personal-repository creation permissions.
- Whether creation needs separate consent/elevation; account and organization restrictions.
- Token expiry, refresh, revocation, storage and recovery behavior.
- Supported API versions/media types, pagination/filtering, rate-limit guidance and creation responses.

Do not embed a client secret, silently reuse CLI/browser credentials, or choose broad classic-token permissions for convenience. An officially supported, owner-approved flow is required. The runtime-supplied GitHub App Client Secret and refresh token use separate Credential Manager identities; React receives only configured/not-configured status after the one-way main-window save command. No stored credential is returned through IPC, events, API, config, logs, or SQLite.

The W1 feasibility gate is recorded in [`workspace-github-auth.md`](workspace-github-auth.md): the owner approved **GitHub App with PKCE web flow and a `127.0.0.1` loopback redirect**, with verified official facts covering device flow vs PKCE, token lifetimes, permissions, and rate limits. The W2 registration checklist, exact permission request, and consent copy are in [`workspace-github-app-registration.md`](workspace-github-app-registration.md); its live-verification items belong to W2 before any implementation.

## Commit semantics and synchronization

Initial proposal: one GitHub.com account, explicitly tracked repositories, default branches for summaries and explicit branch selection in details. Organization repositories may be readable when authorized; organization **creation** remains deferred. Enterprise hosts and multiple connected accounts are outside this slice.

“Pushed commits” means remotely visible commit history within a stated repository/branch/date scope. It is not local Git monitoring or exact push-event history. Commit times must never be labeled push times.

Personal counts require GitHub-linked stable account identity matching the connected account. Do not infer identity from names/emails. Unattributed rows may appear in repository history without increasing the personal count. Co-author attribution and deleted/renamed-account behavior require verification in W1/W2.

Deduplicate by `(repository_id, full_sha)` and model branch membership separately. A shared commit counts once per repository across selected branches. Counts are locally calculated over returned GitHub records, not estimates or official contribution totals. Carry the scope and calculation basis into every consumer.

Every query result includes requested repositories/branches, date range, loaded count, partial reason if any, last successful observation, and coverage. Treat completeness and freshness as separate axes: a result can be both partial and stale. Never hide old gaps merely because a later request succeeded for a different scope.

Propose independent five-minute polling, conditional requests where supported, bounded pages/rows/response sizes/concurrency and deadlines. Exact bounds belong in W1/W2 decisions. Manual and scheduled sync use the same non-overlap gate and honor rate limits. Reaching a bound returns partial coverage, not a total.

Only a complete refresh may replace branch membership **within its successfully covered range**; do not erase older cached ranges. A force-push invalidates current membership only where the refreshed scope establishes it. Partial/error results retain old cache with explicit age/coverage. Never store source code, patches, full raw responses or author emails.

## Repository creation

Proposed first delivery: personal repositories, private by default, optional description and README initialization. No clone, local folder, template/license presets, deletion or automatic rollback.

Use a proposed prepare/confirm flow:

1. Validate owner, name, fields and verified permissions in Rust.
2. Prepare a short-lived immutable review record bound to the current account/session and exact inputs.
3. The main window displays those inputs and public-visibility disclosure.
4. Explicit confirmation consumes the record once. Changed fields or account state require a new review.
5. Prevent duplicate in-flight submissions; publish success only after a confirmed remote response.

Timeout, process interruption or a lost response can leave the outcome unknown. Persist minimal non-secret pending-attempt metadata before dispatch so restart can report uncertainty rather than encourage blind resubmission. Reconcile by reading the requested owner/name, but do not treat its existence as proof Ellie created it. Never automatically repeat the create request. Cancellation cannot undo an accepted remote request.

Bounded, redacted errors distinguish validation, conflict, permissions, authentication, rate limits, connectivity and uncertain outcomes. Never auto-delete a test or user repository.

## Local tasks

Proposed task fields: stable local ID/list ID, bounded required title, optional bounded plain-text notes, priority `none|low|medium|high`, optional due date, optional repository identity link, UTC created/updated/completed timestamps and stable ordering.

Due dates are validated calendar dates (`YYYY-MM-DD`), not UTC timestamps. Overdue means incomplete and earlier than today's local calendar date; timezone changes may alter this state. No reminders or notification scheduler are introduced.

Pin at most one incomplete task. Completing/deleting that task clears the pin in the same transaction. List deletion confirms the affected count; if membership changes before confirmation, request renewed confirmation. Save errors preserve frontend drafts. Repository link snapshots survive disconnect/cache cleanup without keeping the whole GitHub cache or deleting tasks; show disconnected/unavailable status.

Proposed duplicate-list-name policy and exact input limits remain decisions to finalize before W4.

## Conceptual persistence

Names below describe responsibilities, not committed SQL contracts. Use additive migrations after the actual latest schema at implementation time; never renumber existing migrations or rewrite AI tables.

| Entity | Purpose |
| --- | --- |
| GitHub connection | Non-secret account identity, host, status and timestamps |
| Repository cache | Stable remote ID, owner/name, visibility, branch metadata and freshness |
| Tracked branches | Repository/branch selection and covered query ranges |
| Commits + memberships | SHA, subject, optional linked account, timestamps, scoped branch membership |
| Sync coverage | Bounded run metadata, pagination counts, coverage and reasons |
| Creation attempts | Minimal review/pending/outcome state for duplicate suppression and recovery |
| Task lists/tasks | Local content, priority/date/lifecycle and indexes |
| Task repository links | Retained identity snapshot independent of cache deletion |
| Workspace preferences | Current-task pin and HUD content selection |

GitHub cache retention is proposed at 90 days, independent of AI history cleanup. Bound sync/attempt metadata retention as well. Tasks/links persist until explicit deletion. Ordinary SQLite is not encrypted: task notes and private repository metadata rely on the Windows user environment, not Credential Manager protection.

Migration tests must cover populated-database upgrades, transaction rollback, reopening, foreign keys, pin consistency, cleanup and unchanged existing settings/history/API enablement.

## IPC, events and HUD

Main-window-only typed commands cover connection lifecycle, repository selection, bounded commit queries/sync, prepare/confirm creation and local list/task operations. Validate window identity, IDs, lengths, dates, enum values, control characters and account ownership in Rust. Derive requests and external links from trusted host plus validated identities; never accept arbitrary API URLs.

Keep `/api/v1` and `ellie-cli` behavior unchanged. Workspace content is not automatically exposed to local integrations. Existing provider events retain their meanings.

Propose separate revisioned, window-targeted GitHub/task updates and cached bootstraps. Subscribe before bootstrap and reconcile revisions to avoid missing updates or replacing newer state with older bootstrap results. Revision/session changes must clear obsolete content. Neither event handling nor HUD opening triggers network work.

HUD projection includes only enabled sections: loaded personal count with scope/coverage/age, and current task title with minimal display metadata. Exclude notes, credentials, commit subjects and unnecessary private metadata. Disabled sections should not receive their payloads.

Extend native navigation with a bounded destination enum for Overview/AI Usage/GitHub/To-do/History/Settings while preserving tray restore and existing entry points. HUD permits only cached reads, relevant event subscriptions, dragging and bounded main-window navigation. No auth, task writes, creation, refresh, database access or arbitrary external URL opening.

## Disconnect and failure safety

Use account/session generations to reject late responses. Pause sync on auth failure. Confirmed permission loss suppresses inaccessible content; transient network failures may show labeled stale cache.

Disconnect immediately stops new work and invalidates the session, cancels best-effort in-flight reads, clears memory/HUD content, then removes saved credentials/cache with explicit partial-cleanup reporting. Failure to delete a credential must not silently reconnect on restart: persist disabled intent where possible and fail closed if state is inconsistent. Already-submitted remote creation may still finish; retain only the minimum uncertainty record needed to explain that outcome.

Local tasks and their retained repository links survive disconnect. Explain that these links still contain local repository metadata; provide removal through task editing/deletion rather than claiming all repository references vanished.

HTTPS host/redirect/proxy handling must not leak authorization. Logs exclude secrets, raw bodies, private repository names and task content. The existing same-user malware limitation still applies; no new sandbox or encryption guarantee is claimed.

## W2a implementation status (verified on branch `feat/workspace-github`)

Implemented and parent-verified: GitHub App PKCE authorization (verifier/challenge, authorize URL with `127.0.0.1` loopback redirect, strict `state` comparison, code exchange, refresh), refresh-token storage through the injected `SecretStore` under `github_refresh_token`, bounded paginated repository and commit reads with strict sanitization, and typed redacted error categories. The original W2a low-level `github_connect_start`/`github_connect_complete` IPC commands were removed after Rust-owned `github_sign_in` superseded them, so callback code/state cannot cross the WebView boundary. New code lives in `src-tauri/src/github/`; dependencies added are `sha2 0.10.9` and `base64 0.22.1`, both already present in the lockfile. Permission manifests under `src-tauri/permissions/autogenerated/` are build-generated and gitignored; `capabilities/mini.json` is unchanged.

Checks actually run by the parent and their results:

| Command | Result |
| --- | --- |
| `cd src-tauri && cargo fmt --check` | passed |
| `cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings` | passed |
| `cd src-tauri && cargo test` | passed: 106 library tests, 32 `ellie-cli` tests, 0 failed |
| `npm run typecheck` | passed |
| `npm run lint` | passed |
| `npm test` | passed: 5 files, 71 tests |
| `npm run build` | passed |

Tests use sanitized fixtures and local `127.0.0.1` mock servers only; only production code (`src/lib.rs`) constructs the real `https://api.github.com` base URL, and it is never exercised by tests.

**Known gaps (W2b scope), found during parent review:**

1. There is no resume-from-stored-refresh-token path. After an app restart the service starts `Disconnected` and the stored refresh token is unused until the user re-runs the browser sign-in flow. W2b must add startup session restoration from the stored refresh token.
2. The App `client_id` is not persisted anywhere, so W2b must add it as non-secret configuration (Settings/IPC) before restore can work.
3. The connection record (account id/login, status) is in memory only and is not persisted to SQLite; W2b adds the `github_connection` table and migration after the current latest schema.
4. `github_connect_start` leaves the service in `Authorizing` until completion or an explicit `github_disconnect`; there is no expiry timer for an abandoned authorization.
5. Token parsing initially required exact expiry values and fixed token prefixes. Live verification exposed a generic `MalformedResponse`; the parser now treats tokens as opaque bounded values, accepts positive bounded provider-reported lifetimes, still requires refresh-token fields, and returns redacted stage-specific categories for missing expiration fields, unsupported token metadata, or an unusable account response.
6. No GitHub events are emitted and no UI consumes these commands yet (W5 wiring).
7. Live verification remains outstanding: real GitHub App consent, real Windows Credential Manager writes, and refresh-token rotation behavior.

Nothing in this section is a claim about live GitHub behavior.

## W2b implementation status (verified on branch `feat/workspace-github`)

Implemented and parent-verified: schema 12 `github_connection` singleton migration (non-secret `client_id`, optional account id/login, UTC timestamp) with a populated schema-11 upgrade test; `GitHubConnectionStore` trait with strict SQLite and in-memory implementations; lazy session restore from the stored refresh token (no network and no startup delay — the first API call performs the refresh); `github_save_client_id` main-window-only IPC that resets the session and deletes the refresh token when the client ID changes; connection-record persistence after `connect_complete` with fail-closed semantics; 15-minute authorization expiry with an injected clock; and a `clientIdConfigured` status field. `mini.json`, frontend production code, docs, and Cargo dependencies are unchanged.

Checks actually run by the parent and their results:

| Command | Result |
| --- | --- |
| `cd src-tauri && cargo fmt --check` | passed |
| `cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings` | passed |
| `cd src-tauri && cargo test` | passed: 116 library tests, 32 `ellie-cli` tests, 0 failed |
| `npm run typecheck` / `npm run lint` | passed |
| `npm test` | passed: 5 files, 72 tests |
| `npm run build` | passed |

A scan confirmed the owner's real Client ID appears nowhere in the repository; only sanitized values such as `Iv1.sanitized-client` are used. The real Client ID lives only in the user's runtime settings via `github_save_client_id`.

**Remaining gaps (recorded, not yet implemented):** repository selection/cache persistence and commit coverage tables from the design are not yet built (commits are fetched on demand); no GitHub events exist; no UI consumes these commands (W5); live GitHub App authorization, real Windows Credential Manager writes, and refresh-token rotation are still unverified; and a connection-persistence failure keeps the in-memory session and refresh token while reporting `Disconnected` so the next `connect_complete` retry can succeed (tested).

## W5a implementation status (verified on branch `feat/workspace-github`)

Implemented and parent-verified: Rust-owned `github_sign_in` / `github_cancel_sign_in` orchestration. The command binds an ephemeral `127.0.0.1` listener, starts the PKCE authorization, opens the authorize URL in the default browser through `tauri-plugin-opener` with a **URL-scoped capability** (only `https://github.com/login/oauth/authorize?*` from the main window), awaits exactly one loopback GET, rejects any non-callback path and any non-loophost peer, parses exactly one `code` + `state` and an optional validated GitHub RFC 9207 `iss`, completes the connection, and serves a minimal `no-store` page to the browser. Cancellation is generation-aware and preserves the refresh token; timeout matches the 15-minute authorization lifetime. The callback code/state never cross IPC. New dependency: `tauri-plugin-opener` v2 (official).

Checks actually run by the parent and their results: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test` (122 library + 32 CLI, 0 failed), `npm run typecheck`, `npm run lint`, `npm test` (5 files, 74 tests), `npm run build` — all passed. A scan confirmed no owner client id or secret appears in the repository. Live authorization and real default-browser opening remain unverified until the W5b UI exists.

## Live-auth correction: required GitHub App Client Secret

The first live callback reached Ellie, but GitHub rejected the token exchange. Official documentation review confirmed the implementation's earlier assumption was wrong: for a GitHub App web flow, `client_secret` remains required at both authorization-code exchange and refresh even with PKCE. The owner chose to keep the loopback + PKCE flow and store the App secret securely.

Implemented: dedicated `github_app_client_secret` Credential Manager entry; main-window-only `github_save_client_secret`; `clientSecretConfigured` boolean status (never the value); missing-secret preflight before opening the browser; Client Secret included in both code exchange and refresh forms; redacted `app_credentials_invalid` error category; disconnect and Client-ID replacement delete the App secret and refresh token; masked Settings input that clears after save; Connect disabled until both Client ID and Secret are configured. The secret is never stored in SQLite/config, returned through IPC, logged, or committed. Tests use sanitized values only.

Parent verification passed: `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test` (123 library + 32 CLI tests); `npm run typecheck`; `npm run lint`; `npm test` (7 files, 103 tests); and `npm run build`.

Live verification subsequently reached token/account response validation but surfaced only the generic `malformed_response` category. The parser now returns `token_expiration_required`, `token_response_invalid`, or `account_response_invalid` without logging bodies, tokens, or field values. Token strings are treated as opaque bounded credentials rather than relying on provider prefixes; provider-reported expirations are accepted only when positive and within bounded access/refresh limits. A fresh sign-in will therefore either complete if the issue was harmless prefix/lifetime drift or identify the failing stage safely. Parent verification passed: `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test` (124 library + 32 CLI tests); `npm run typecheck`; `npm run lint`; `npm test` (7 files, 106 tests); and `npm run build`.

Repository/commit loading, restart restore, refresh-token rotation, and disconnect remain live-verification items until sign-in succeeds.

## W3/W4 implementation status (branch `feat/workspace-github`)

Schema 13 adds local task lists, tasks, repository identity snapshots, and one transactional incomplete-task pin. Rust validates bounded titles/notes/list names, priorities, calendar due dates, repository IDs/names, and count-guarded list deletion. Main-window-only commands run SQLite work through blocking workers. Completing/deleting a pinned task clears the pin in the same transaction; disconnect never deletes tasks or repository-link snapshots.

Schema 14 stores only minimal non-secret metadata for repository-creation requests whose outcome may be unknown. Creation is personal-account-only, private by default, and uses a short-lived account/session-bound prepare review followed by one consumed confirmation. Ellie persists dispatch intent before `POST /user/repos`, never auto-retries, and treats timeout/lost successful responses as `creation_outcome_unknown`. Users inspect GitHub and explicitly resolve the local warning while connected to the same account; resolution never performs another create. A confirmed HTTP 201 remains success even if local attempt cleanup fails, preventing a dangerous duplicate retry.

The low-level `github_connect_start` and `github_connect_complete` IPC commands and permissions were removed; only Rust-owned `github_sign_in` handles callback values. Main capability permissions include task and creation operations; the mini capability remains byte-for-byte unchanged. Automated gates currently pass with 142 Rust library tests and 32 CLI tests. No owner-authorized live repository was created; native restart, interaction, and live creation remain verification items.

The Overview contribution calendar fetches the connected account's `contributionsCollection.contributionCalendar` through the GitHub GraphQL API (`POST /graphql`, same authenticated bearer path with 401-refresh retry). Rust validates the response: bounded weeks, unique strictly increasing dates, weekday alignment with `firstDay`, day-of-week bounds, known contribution levels, and sanitized counts, returning redacted categories for GraphQL errors. The frontend never computes or claims these totals. Private/internal contributions depend on the GitHub App's access, and the card labels the range and meaning explicitly.

## Acceptance and remaining decisions

Follow W1–W6 in the parent plan; this document does not renumber or combine them. Each phase requires explicit implementation authorization and the repository-defined checks.

Tests should exercise hostile text/URLs, wrong-account results, branch deduplication, partial + stale coverage, rate limits, expiry, bounded work, confirmed permission loss, disconnect failures, restart with unknown creation outcome, duplicate confirmation, offline task writes, date boundaries and populated migrations. Later Windows smoke checks cover tray/restore/settings/HUD/DPI/quit. Live GitHub tests require authorized credentials and explicit permission for remote creation.

Remaining decisions are limited to repository-selection/cache persistence, final expanded-HUD sizing/content controls, and any future organization-creation policy. Personal-only creation, bounded input rules, and case-insensitive unique list names are implemented.

Automated frontend and Rust checks were run as recorded above. No live repository creation, installer smoke check, or native Windows DPI/interaction pass was performed.
