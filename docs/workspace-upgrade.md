# Ellie workspace upgrade — design proposal

## Status and approved scope

**Implemented on `feat/workspace-github` and merged to `main`.** W1–W6 are complete. The owner selected:

- GitHub **track + create**: view authorized public/private repositories, track pushed commits, and create repositories.
- **Local to-do board**: private Work/Personal tasks with details, completion, priority, due dates, optional Work repository links, and one desktop sticky-note pin; no issue synchronization.
- **Expand the existing floating bar**: quota, commit activity, and a current task in an optional compact HUD that opens dashboard details.
- Preserve existing token/quota monitoring and improve the detailed dashboard.

The owner subsequently authorized the full workspace phase. The implementation preserves the scope limits below; live repository creation was performed by the owner separately, and automated tests never create remote repositories.

## Design documents

- [Detailed dashboard and HUD design](workspace-interface.md): page layouts, interactions, states, accessibility and native-window constraints.
- [Detailed backend design](workspace-backend.md): GitHub/service boundaries, persistence, creation recovery, IPC and security.
- [W1 GitHub authentication and permissions record](workspace-github-auth.md): verified official facts and the approved sign-in mechanism (GitHub App + PKCE).
- [W2 GitHub App registration and PKCE flow](workspace-github-app-registration.md): owner registration checklist, permission request, consent copy, and PKCE notes.

These companion documents consolidate the frontend and backend specialist proposals and the W1 verification. This plan owns scope and W1–W6 sequencing; the companion documents refine proposed details without authorizing implementation.

## 1. Product direction

Ellie remains a lightweight, local-first Windows tray companion. The expanded dashboard answers three questions:

1. How much AI usage do I have left?
2. What have I committed to my tracked GitHub repositories?
3. What should I work on next?

AI monitoring remains a first-class feature, usable without connecting GitHub or creating tasks. Preserve the black-and-white cat identity, warm white text, dark surfaces, restrained pink selection accents, accessible status colors, and optional friendly copy. No animated pet, new AI calls, telemetry, or Ellie cloud service.

### Scope boundaries

| Included in this design | Not included |
| --- | --- |
| Authorized GitHub repository browsing | Unrestricted GitHub account administration |
| Pushed commit history for selected repositories | Local folder scanning, unpushed commits, Git staging/push/pull |
| Explicitly confirmed repository creation | Repository deletion, code editing, merges, settings management |
| One local task board and one bounded pinned-task sticky note | GitHub Issues, PR management, task sync, team collaboration |
| Dashboard redesign and expanded mini bar | A second command-center panel, game injection, animated companion |
| Existing quota, history, settings, API and CLI compatibility | New providers or new API/CLI routes |

“Full access” is narrowed by the owner's choice to **track + create**, not blanket write permission. Access is always limited by the authenticated account, granted permissions, repository selection, and organization policy.

## 2. Dashboard information architecture

Proposed navigation: **Overview · AI Usage · GitHub · To-do · History · Settings**. History retains the existing AI analytics rather than silently mixing commit activity into its metrics.

Illustrative layout only; all values below are placeholders, not live data:

```text
┌──────────────────────────────────────────────────────────────────────┐
│ Ellie                            [Add task] [New repository]         │
│ Overview · AI Usage · GitHub · To-do · History · Settings             │
├──────────────────────────────────────────────────────────────────────┤
│ AI allowance                         │ Current task                  │
│ Provider / window / remaining        │ Task title                    │
│ Reset timestamp · freshness          │ List · priority · due date    │
│ [Open AI Usage]                      │ [Open task]                   │
├──────────────────────────────────────┼───────────────────────────────┤
│ GitHub activity                      │ To-do                         │
│ Selected repositories · date range  │ Open / completed counts       │
│ Loaded commit count · coverage      │ Due and upcoming tasks        │
│ Recent commit rows · last sync      │ [Add task] [Open lists]       │
│ [Open GitHub]                        │                               │
└──────────────────────────────────────┴───────────────────────────────┘
```

- At narrow widths, stack sections in reading order; never squeeze numeric labels or require horizontal page scrolling.
- Each network-backed section has its own freshness and refresh state. A global “updated” label must not imply all sources succeeded.
- New repository is enabled only when the verified connection supports creation; otherwise explain how to connect or resolve permissions.
- Disconnected GitHub shows a compact connection invitation, not invented activity or an empty zero count. Local tasks remain usable offline.
- Empty to-do state offers “Add your first task.” Do not create sample tasks automatically.
- Retain keyboard navigation, visible focus, text alongside status color, reduced-motion/transparency behavior, and the existing settings controls.

### AI Usage

Retain existing provider cards, supported quota windows, used/remaining labels, reset timestamps, balances, token statistics, refresh actions, hidden-provider preferences, and stale-cache handling. Do not infer new quotas or combine subscription and API-billed activity. Overview is a summary; the detailed page retains all existing information.

### GitHub

- Connection header: account login, access status, last successful sync, retry state, and repository selection.
- Repository list: name/owner, visibility, description if present, default branch if present, tracked state, and safe external link.
- Commit list: repository, branch context, short SHA, plain-text subject, GitHub-linked author when available, committed timestamp displayed locally, and GitHub link.
- Filters: tracked repository, branch, and date range. Proposed first version uses each repository's default branch initially, with explicit branch selection for detail views.
- Activity summary: commits attributed by GitHub to the connected account in the selected scope. Never infer identity from an arbitrary name/email match. Unattributed commits can appear in repository history but do not count as “your commits.”
- Show “N loaded commits” when pagination or errors leave coverage incomplete. Never describe this as an account-wide contribution count or an exact push count.
- Commit time is not push time. A commit appearing on GitHub means it is remotely visible; the history does not reveal when every push occurred. Explain branch/date coverage next to summaries.

### To-do

- Present one local `My tasks` board. Rust creates its internal list automatically on the first bootstrap, so users do not choose a storage destination or manage list containers.
- Create/edit tasks with required name, optional plain-text details, explicit Work/Personal type, priority (`none|low|medium|high`), and optional due date.
- Work tasks may retain one optional validated GitHub repository identity snapshot. Personal tasks cannot retain a repository. This is metadata, not synchronization; removing GitHub access never deletes a task.
- Complete/reopen and delete tasks, with explicit task-delete confirmation. No notification/reminder system is introduced.
- Filter by completion, Work/Personal type, and priority. Incomplete tasks remain first, then due date and stable creation order.
- Pin one incomplete task as the current task for both Overview → Focus and a dedicated always-on-top, draggable sticky-note window. Complete/unpin/Open Ellie are its only task actions; it cannot enumerate or edit other tasks. Completing, deleting, or unpinning clears the pin and closes the note without selecting a replacement.
- Due dates are calendar dates (`YYYY-MM-DD`), not UTC instants. Created/updated/completed timestamps are UTC. Overdue means an incomplete task's due date precedes the user's current local date.
- Save failures preserve the user's draft and show retry; only report success after persistence succeeds. Tasks live in Ellie's ordinary local SQLite database under `%LOCALAPPDATA%`, not Credential Manager or GitHub.

## 3. Repository creation flow

Proposed first delivery supports personal-account repositories. Organization creation is deferred until separately approved and its permissions/policy behavior are verified.

1. Open **New repository** from Overview or GitHub.
2. Enter repository name, optional description, and visibility. Default to **private**; public visibility has a clear disclosure warning.
3. Optionally initialize a README. Templates, licenses, gitignore presets, local clone, and folder creation are deferred.
4. Review the exact owner, name, visibility, description, and initialization choice.
5. Select **Create repository** once. Rust validates inputs and permissions; disable duplicate submission while pending.
6. On confirmed success, show the repository link and offer to track it. Never claim a local clone exists.

Failures distinguish missing permissions, invalid name, name conflict, organization policy where relevant, rate limits, authentication expiry, and network failure. Do not expose raw response bodies.

Creation is a remote side effect: if the response is lost or times out, show **“Creation outcome unknown”**, then reconcile against the requested owner/name. Do not automatically repeat the create request. Finding a repository with that name is not proof Ellie created it; let the user inspect it before retrying. Canceling the UI cannot undo an already-submitted request. No repository-deletion permission is needed for rollback.

## 4. Expanded HUD

Evolve the existing mini window rather than running a second companion window.

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Ellie │ Provider · window · remaining │ Commits* │ Current task     │
└─────────────────────────────────────────────────────────────────────┘
* Loaded count for the configured repository/branch/date scope
```

- Optional and always on top; retain taskbar exclusion, drag handle, opacity, monitor-safe position persistence, and clean shutdown.
- Settings chooses visible sections. Existing users retain their quota-only configuration until opting into GitHub/task content; hiding the HUD does not stop core monitoring.
- Preserve full provider/window/remaining labels and explicit zero, unavailable, missing, and stale states. No inferred complements or new quota eligibility rules.
- Task text may truncate with accessible full text available; quota labels must remain readable. At constrained widths, use compact stacked rows instead of clipping essential labels. Exact dimensions are a UI implementation decision verified on Windows scaling.
- Activating quota, commits, or current task restores/focuses the appropriate main-dashboard view through the existing native navigation path. Keep dragging distinct from navigation.
- HUD is read-only: task editing/completion and repository creation happen in the main dashboard. No credentials, arbitrary URL opening, network requests, or general settings writes from this window.
- Shared cached state plus sanitized window-targeted updates supply content. Opening, moving, or repainting the HUD must not trigger GitHub/provider refreshes.
- GitHub/task titles may be sensitive on screen. Default new sections off for existing installations, offer quota-only mode, and explain that always-on-top content can appear during screen sharing. Do not promise capture exclusion.
- No rotating ticker, focus stealing, repeated prompts, or additional notifications.

## 5. Technical design and trust boundaries

Preserve Tauri 2/Rust/Tokio/reqwest/serde, React/TypeScript, and SQLite. No new dependency is approved by this document.

```text
AI adapters → existing normalization / refresh / history ─┐
GitHub API → separate GitHub service / cache / sync ──────┼→ Rust state
Local tasks → task service / SQLite ─────────────────────┘     │
                                     ┌───────────────────────┴──────┐
                                     ▼                              ▼
                              Main dashboard                 Read-only HUD
```

GitHub is a workspace integration, **not a UsageProvider** and not GitHub Copilot quota support. Do not force commits/tasks into usage snapshots or the AI refresh coordinator. Share safe infrastructure where appropriate, not domain semantics.

### Authentication decision gate

Prefer a documented interactive GitHub sign-in with credentials owned by Rust and Windows Credential Manager. Before implementation, verify GitHub App versus OAuth device-flow support, application registration/distribution requirements, exact read/create permissions, expiration/revocation behavior, and organization restrictions against official GitHub documentation.

Do not ship a client secret inside the desktop binary. Do not silently read browser sessions or existing CLI tokens. Do not default to a broad classic token merely to simplify creation. If minimal read access cannot create repositories, document a separate explicit permission upgrade or ask the owner to approve an alternative flow. The authentication mechanism is **unresolved**, not a tested integration claim.

The authentication mechanism is resolved by [`workspace-github-auth.md`](workspace-github-auth.md) (W1 decision record): GitHub App with PKCE web flow and a `127.0.0.1` loopback redirect is recommended, with confirmed official facts, credential lifetimes, permissions, rate limits, alternatives and owner decision pending. The official links there are verified research with cited passages, not a substitute for live W2 verification.

### Synchronization and failure handling

- Proposed scope: one connected GitHub.com account, explicitly selected repositories, bounded paginated reads. Enterprise hosts and multi-account switching are deferred.
- Proposed refresh target: five minutes, independently scheduled, subject to GitHub rate limits and bounded work. Explicit refresh also honors retry deadlines.
- Use request timeouts, bounded response sizes/concurrency, conditional requests where supported, and primary/secondary rate-limit backoff. Final page/work limits must be recorded during implementation; reaching a bound produces partial coverage, not a misleading total.
- Cache only required repository/commit metadata, never full raw responses, email addresses, code, patches, or credentials. Render untrusted text as text, not HTML.
- Deduplicate commit rows by repository identity plus SHA; track branch membership separately. Across selected branches, count a SHA once per repository, not once per appearance. Repository identity remains stable across renames.
- A complete successful branch refresh replaces that branch's covered membership so force-pushed-away commits do not inflate current counts. Failed/partial refreshes preserve prior cache and explicitly label coverage/staleness.
- Authentication failure stops sync and prompts reconnection. Permission loss or confirmed inaccessible repositories suppress their content in the dashboard/HUD; a transient network outage may show stale cache with age.
- Account identity must be checked before publishing data. Reject late results from disconnected/obsolete sessions. Disconnect clears credentials and GitHub cache, stops work, and removes HUD GitHub content; local tasks survive with links retained locally. If deletion fails, report it and allow retry, never claim removal succeeded.
- GitHub failure must not block task persistence, quota polling, notifications, or app startup.

### Persistence and IPC

Proposed entities, not a finalized SQL schema or implemented command contract:

| Entity | Minimal responsibilities |
| --- | --- |
| GitHub connection metadata | Account ID/login, non-secret credential reference, sync status |
| Tracked repositories | Stable ID, owner/name, visibility, chosen branch, last success |
| Cached commits and branch membership | Repository ID/SHA, subject, optional linked author ID/login, UTC commit time, coverage |
| Task lists | Local ID, name, UTC creation/update time |
| Tasks | Local ID/list ID, title/notes, priority, optional date/link, UTC lifecycle timestamps |
| Workspace preferences | Pinned task ID and HUD section visibility |

Use additive versioned migrations after the actual latest schema at implementation time. Keep AI history and settings intact; test upgrades on populated temporary databases. Proposed GitHub cache retention is 90 days; tasks persist until explicitly deleted and are never swept by AI/GitHub history cleanup. Ordinary local SQLite is not encrypted: disclose that task/private-repository metadata is protected by the Windows user environment, not Credential Manager.

Main-window IPC should expose typed operations for connection state, repository selection, bounded commit queries, confirmed creation, and task CRUD. Validate IDs, enum values, text lengths, dates, URLs, and ownership in Rust. Resolve repository actions from trusted IDs rather than accepting arbitrary API URLs. Pin authentication to trusted GitHub hosts and prevent redirects from leaking authorization.

HUD receives only the configured summary through narrow read/navigation permissions. Do not send task notes or unnecessary private repository details to it. New workspace data is not exposed by the existing `/api/v1` endpoints or CLI. Existing local API opt-in and token rules remain unchanged.

## 6. Delivery plan and acceptance gates

W1–W6 are implemented and merged to `main`; native DPI/interaction checks remain manual. Retain existing milestone history and do not treat automated fixtures as live GitHub verification.

| Phase | Deliverable | Gate |
| --- | --- | --- |
| W1 | Verify GitHub auth/API feasibility and finalize permission design | Official-source evidence; read/create permissions, secure desktop flow and registration decision documented; owner approval before implementation |
| W2 | GitHub connection, selection, commit list and cached read states | Authorized live public/private checks plus sanitized fixture tests; partial coverage, identity attribution, expiry, rate limits and disconnect verified |
| W3 | Personal repository creation — implemented, live create pending | Explicit review/confirmation; duplicate submission and ambiguous outcome tests pass; real authorized repository creation requires owner consent |
| W4 | Local lists/tasks and persistence — implemented | Offline CRUD, restart persistence, failed saves, date boundaries, pin clearing, populated migration, and repository-link retention tests pass |
| W5 | Integrated Overview and detailed navigation — implemented | Existing AI/history/settings behavior preserved; six-view library-backed shell, scoped commit pagination, account-wide profile contribution calendar, and accessible states implemented |
| W6 | Expand existing HUD — implemented | Shared cache only; no extra polling; scope labels, privacy toggles, navigation, content-aware logical (DPI-correct) sizing, and persisted user resizing verified in automated and contract tests; DPI/multi-monitor dragging and clean quit remain manual smoke checks |

Dashboard sketches and review can precede implementation phases. Each implemented phase must include usable main-window UI rather than accumulating backend-only work until W5.

### Required implementation verification (not run for this document)

- `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`.
- `cargo test --manifest-path src-tauri/Cargo.toml`.
- Tests for secret exclusion across IPC/API/events/logs; invalid input, hostile text/URLs, wrong account IDs, bounded pagination, cancellation and late results.
- Regression checks for usage normalization, stale cache, quota notifications, existing local API/CLI responses, settings and provider visibility.
- Owner-authorized live GitHub checks; mocks alone cannot complete W2/W3. Never delete a remote test repository automatically.
- Windows smoke checks: hide/restore, tray refresh, settings, HUD enabled/disabled, restart, DPI/monitor changes, keyboard focus, and clean quit.

No version bump, installer identity change, API contract expansion, code dependency, or migration is made in this documentation phase. Pick the release version under the existing SemVer policy when an implementation is ready for release.

## 7. Consolidated design decisions

Proposed choices from specialist synthesis. The W5 decisions are implemented; HUD-specific items were implemented in W6:

- Preserve per-provider/window rows on Overview rather than a misleading cross-provider quota score.
- Expand native typed navigation for section-specific HUD links; retain existing tray and main-window restore behavior.
- Use sibling HUD activation controls, not nested buttons. Expanded content requires bounded native sizing and contract-test updates, not just CSS wrapping inside the existing 480×96 window.
- Do not rely on horizontal scrolling, abbreviated quota labels, or unmeasured palette contrast to make the HUD usable.
- Count loaded GitHub records as a scoped local calculation, not an estimate or an official contribution total. Completeness and freshness are separate states.
- Bind repository-creation confirmation to reviewed inputs and account/session; preserve minimal pending-operation state for uncertain outcomes after restart.
- Keep separate W5 dashboard and W6 HUD gates. Authorized organization repository reads are not the same as organization repository creation.

## 8. Review checklist

Approvals for the implemented W1–W5 decisions were granted during delivery; HUD opt-in/privacy behavior was approved and implemented in W6.

- [x] Approve proposed dashboard navigation and layouts.
- [x] Approve default-branch commit coverage and personal-account-only repository creation for the first delivery.
- [x] Approve the W1 authentication mechanism (done: **GitHub App + PKCE**, see [decision record](workspace-github-auth.md) and [W2 registration doc](workspace-github-app-registration.md)); the app registration checklist in W2 was completed before implementation.
- [x] Approve task interaction defaults and HUD opt-in/privacy behavior (implemented in W6).
- [x] Authorize one implementation phase explicitly.
