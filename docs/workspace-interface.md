# Workspace interface design

**Status: implemented for the main window; expanded HUD pending.** This document expands the [workspace upgrade plan](workspace-upgrade.md). GitHub browsing/creation, local lists/tasks, the six-view command-center hierarchy, and the integrated Overview are implemented on `feat/workspace-github` and merged to `main`. The expanded HUD remains future work. It incorporates the frontend specialist's design, with parent synthesis to resolve conflicting HUD proposals.

## Preserve the existing application

Keep AI usage, History, Settings, provider visibility, refresh, credentials, notifications, local API onboarding, and tray behavior functional. GitHub connection and task creation are optional. Reuse the existing dark-first glass surfaces, warm text, restrained pink accents, static cat, and reduced-motion/transparency behavior.

Implementation anchors: `src/App.tsx`, `src/styles.css`, `src/lib/desktop.ts`, `src/components/MiniBar.tsx`, `src/lib/miniQuota.ts`, `src-tauri/src/tray.rs`, and `src-tauri/capabilities/mini.json`. These are integration points, not authorization for a broad refactor.

## Navigation and page hierarchy

Proposed navigation: **Overview · AI Usage · GitHub · To-do · History · Settings**.

| Page | Main content | Primary actions |
| --- | --- | --- |
| Overview | AI allowance rows, current task, scoped GitHub activity, task counts | Add task; open detail pages; new repository |
| AI Usage | Existing detailed provider cards and supported metrics | Refresh; hide provider |
| GitHub | Connection state, repositories, bounded commit history and scope | Connect; select tracked repositories; sync; new repository |
| To-do | One local task board, Work/Personal filters, due dates, current-task pin | Add/edit; complete/reopen; pin as sticky note; confirmed deletion |
| History | Existing AI analytics and source labels | Existing date-range controls |
| Settings | Existing preferences plus GitHub connection and HUD content settings | Existing controls; connection management; HUD configuration |

Use one page heading and meaningful section headings. Overview shows provider/window rows rather than a single cross-provider score: unrelated quota windows are not comparable. A summary links to full detail without hiding freshness or provenance.

## Wireframes

All names and values below are illustrative placeholders, not account data. A release version must come from application metadata, never a hard-coded mock version.

### Overview — desktop

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Ellie                                   [Add task] [New repository] │
│ Overview · AI Usage · GitHub · To-do · History · Settings             │
├───────────────────────────────────┬─────────────────────────────────┤
│ AI allowance                      │ Current task                    │
│ Provider · window                 │ Review dashboard copy           │
│ Remaining value · reset time      │ Work · High · due date          │
│ Source · last success             │ [Open task]                     │
│ [Open AI Usage]                   ├─────────────────────────────────┤
├───────────────────────────────────┤ To-do                           │
│ GitHub activity                   │ Open count · overdue count      │
│ Selected repositories / branches │ Due and upcoming tasks          │
│ Loaded count · range · coverage  │ [Add task] [Open lists]          │
│ Recent commits · last sync        │                                 │
│ [Open GitHub]                     │                                 │
└───────────────────────────────────┴─────────────────────────────────┘
```

### Narrow dashboard

```text
Ellie                     [Add task]
Overview · AI Usage · GitHub
To-do · History · Settings
─────────────────────────────────
AI allowance
Provider / full window label
Remaining · reset · freshness
[Open AI Usage]
─────────────────────────────────
Current task
Title · list · priority · due date
[Open task]
─────────────────────────────────
GitHub activity
Loaded count · exact scope
Coverage · last sync
[Open GitHub]
─────────────────────────────────
To-do summary
[Open lists]
```

Stack sections in semantic reading order. Wrap navigation and labels; never require horizontal page scrolling. Validate at the actual native minimum width and at 100%, 150%, and 200% Windows scaling; CSS breakpoints alone are not proof of native-window fit.

### GitHub

```text
GitHub                                            [New repository]
Connection: account · permissions · last successful sync
[Manage connection] [Choose repositories] [Sync]
Repositories | Commits
[Repository filter] [Branch] [Date range]
──────────────────────────────────────────────────────────────────
owner/repository · private · tracked · default branch
SHA       Plain-text subject          Linked author · commit time
[Open repository on GitHub]                          [Open commit]
──────────────────────────────────────────────────────────────────
N loaded commits · selected branches · date range
Partial coverage: page limit reached
```

Do not imply that a commit timestamp is a push timestamp. Personal counts use verified account association; repository history may include unattributed rows. A completed empty query can show zero; disconnected, inaccessible, partial, and failed states cannot masquerade as zero.

### To-do

```text
My tasks                                               [New task]
Open · Completed · Overdue · Sticky
[All / Open / Completed] [Work + Personal] [Priority]
──────────────────────────────────────────────────────────────────
[ ] Task title   Work · High · due date · optional repo [⋯]
[ ] Study Rust   Personal · due date                    [⋯]
[x] Completed task                                      [Reopen]
```

Ellie creates one internal `My tasks` list on first bootstrap and presents it as a single local board; users do not configure a storage destination or manage list containers. The editor includes task name, optional details, explicit Work/Personal type, priority, optional calendar due date, and an optional repository link shown only for Work. Labels remain visible; required/invalid states include text. Save success follows durable SQLite persistence and failed saves retain drafts. Task deletion requires confirmation.

Pinning one incomplete task promotes it to Overview → Focus and opens Ellie's custom `task-note` window: always on top, taskbar-free, draggable, and position-persistent. It shows only the pinned task with Complete, Unpin, and Open Ellie actions. Completing, deleting, or unpinning clears the pin and closes the note without choosing a replacement. This dedicated sticky note is separate from the unchanged quota-only mini bar and does not implement the pending expanded HUD.

## Repository creation interaction

Use a form and a separate review/confirmation step. Show owner, name, description, private/public selection, and README initialization. Default private. Public visibility requires an explicit warning. Personal-only creation is a proposed first-delivery boundary, not a verified GitHub capability claim.

While submitted, disable duplicate creation. A timeout yields **Creation outcome unknown**, not “Failed, try again.” Offer reconciliation/inspection without automatically issuing another create. Closing the form cannot undo a remote operation. A confirmed result offers a safe GitHub link and a separate track action; no implicit clone or deletion.

## Expanded HUD

Preserve the existing mini window identity and quota-only mode. Add optional GitHub and current-task sections; default both off on upgrade. W6 does not add a second command-center panel; the separately owner-authorized `task-note` window is a bounded sticky note for one pinned local task, not the expanded HUD.

```text
┌────────────────────────────────────────────────────────────────┐
│ Drag │ Provider · full quota window · remaining value          │
│      │ N loaded commits · scope/range · partial or stale state │
│      │ Current task title                                     │
└────────────────────────────────────────────────────────────────┘
```

The existing mini window is fixed at 480×96 and has a native contract test. The proposed expanded layout requires bounded content-aware sizing and corresponding native/test changes during W6; it is not achievable by promising CSS wrapping alone. Keep quota-only dimensions unless implementation evidence justifies a change. Clamp the whole window to the monitor work area after sizing and dragging.

Use independently focusable, sibling section buttons to open AI Usage, GitHub, or the pinned task. Do not nest buttons inside the existing whole-surface button. Keep a distinct drag handle; no task completion, repository creation, arbitrary link opening, or refresh action in the HUD.

Protect full quota labels and remaining values; only task titles may truncate, with accessible full text. Do not use a rotating ticker or horizontal scrolling as the normal way to read essential information. Bound the displayed selection to what fits; show an explicit “Open Ellie for more” affordance rather than silently hiding content. Exact size/row bounds are W6 validation decisions.

Show scope/coverage/freshness alongside commit counts, not solely in a tooltip. For no pin, say “No current task”; disconnected GitHub remains distinct from zero commits. Use a sufficiently opaque backdrop for text readability at every allowed opacity. Exact contrast must be tested on composite surfaces; palette contrast alone does not prove HUD accessibility.

Settings provides HUD hide/show, quota-only mode, section selection, opacity, and a reset-position action for recovery without precise dragging. Warn that task/repository content may appear in screen sharing; no capture-exclusion guarantee.

## State and accessibility contract

| State | Behavior |
| --- | --- |
| Loading without cache | Label what is loading; never substitute sample metrics |
| Loading with cache | Keep values and last-success age visible |
| Disconnected GitHub | Connect invitation; tasks and AI monitoring unaffected |
| No tasks | Add-first-task action; no automatic sample content |
| Partial GitHub coverage | Loaded count, scope, and reason; no account-wide total |
| Refresh failure | Preserve eligible stale cache with age; offer safe retry |
| Permission revoked | Suppress inaccessible GitHub content; explain reconnect/access action |
| Failed local save | Preserve draft, show actionable error, no success toast |

Use real navigation controls with current-page state, a skip-to-content link, visible focus, labeled fields, keyboard-operable dialogs with restored focus, and text alongside colors. Announce meaningful outcomes once; avoid announcing countdowns every second. External text is plain text, never executable HTML. Browser preview must remain clearly labeled and must not imply native persistence or live account access.

## Command-center dashboard status (implemented on `feat/workspace-github`)

The main app now uses a command-center shell with a left navigation rail (Overview, AI Usage, GitHub, To-do, History, Settings), a top action bar with a Ctrl+K quick-action palette, and a library-backed React UI. Overview derives every displayed value from existing typed AI, GitHub, task, and analytics data — no invented quota values, commit counts, or charts. It includes KPI cards, a provider summary sheet, a pinned Focus task card, a token-trend chart (Recharts), a bounded multi-repository recent-commit feed with per-row repository labels, attention items, upcoming tasks, and a local activity feed. The feed checks up to 12 loaded repositories with at most four concurrent requests, requests at most three commits per repository, merges the newest six by authored time, and reports partial or omitted coverage instead of implying an account-wide total. Disconnected GitHub shows an invitation rather than a zero; empty task state invites the first task.

Libraries adopted and where used: `lucide-react` (icons across the shell, dashboard, GitHub, tasks, and sticky note), `@radix-ui/react-dialog` (task editor, repository creation review, confirmations, command palette), `@radix-ui/react-select` (task priority/repository and page filters), `@radix-ui/react-dropdown-menu` (row actions), `@radix-ui/react-checkbox` (task completion), `recharts` (token trend area chart and quota utilization bars on History and Overview), `clsx` (variant composition in UI primitives), and `sonner` (save/key/delete toasts). No remote assets, telemetry, or runtime CDNs are used.

The GitHub page now paginates the bounded loaded commit set with explicit loaded-count scope labels and a bounded-not-account-total footnote. The account-wide contribution calendar moved to Overview: Rust fetches the connected account's profile `contributionsCollection.contributionCalendar` through the GitHub GraphQL API (bounded, authenticated, validated, redacted), and Overview renders it with monthly labels, weekday hints, the profile total, an explicit date range, and a note that it includes all contribution types GitHub counts — it is not a local commit total. The per-repository activity grid was removed.

Frontend checks after the task-board/sticky-note integration: `npm run typecheck`, `npm run lint`, and `npm test` (10 files, 117 tests) passed; the final production build and native Windows sticky-note smoke check remain to be recorded. A safe 1200×900 browser-preview capture was inspected; native Windows interaction/DPI smoke checks and a release install remain to be recorded.

Multi-repository commit-feed verification: `npm run typecheck`, `npm run lint`, `npm test` (11 files, 121 tests), `npm run build`, `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test` (153 library tests and 32 CLI tests) passed. Live native loading against multiple GitHub repositories was subsequently used by the owner; restart restoration, refresh-token rotation, and disconnect cleanup still require recorded smoke verification.

## W5b implementation status (verified on branch `feat/workspace-github`)

Implemented and parent-verified (all work under `src/`): additive typed GitHub bindings in `src/lib/desktop.ts`, a `useGitHubConnection` hook with generation-ordered mutation boundaries and friendly error-category copy, an additive `github` view (`GitHubPanel`) with connection banner, repository list, and commit list (repository picker + optional branch, plain-text subjects, author login or unattributed label, local-time commit dates, explicit loaded-count scope labels and a bounded-not-account-total footnote), a Settings → GitHub section (`GitHubSettings`) for the non-secret Client ID plus a masked, one-way-save App Client Secret, connect with in-progress/cancel, and disconnect with confirmation (`ConfirmDialog`), plus a skip-to-content link and additive token-based styles covering reduced-motion/reduced-transparency/forced-colors. `MiniBar`, `src/lib/miniQuota.ts`, and everything under `src-tauri/` are untouched.

Checks actually run by the parent for W5b: `npm run typecheck`, `npm run lint`, `npm test`, and `npm run build` — all passed at that phase. Live browser authorization and repository/commit loading subsequently succeeded; restart restoration, refresh-token rotation, and disconnect cleanup remain to be recorded.

**Wire contract note:** the Rust `GitHubConnectionState` enum serializes unit variants verbatim (`Disconnected`/`Authorizing`/`Connected`), so the frontend binds those exact strings; the status struct and summary structs use camelCase fields. Any future `rename_all` change on that enum must be coordinated across both sides. After live verification exposed GitHub's required App secret, the status contract gained only `clientSecretConfigured: boolean`; the secret itself is accepted only by the main-window save command, immediately cleared from the masked field, and never returned.

## Acceptance checklist for later implementation

- Every page and HUD section has loading, empty, unavailable, and applicable stale/partial states.
- Existing quota values, hidden providers, analytics, refresh, settings, and credential flows regress neither visually nor semantically.
- Narrow layouts and native DPI/monitor changes preserve labels and controls.
- Keyboard users can reach pages, editors, confirmations, HUD sections, and reset-position controls.
- Reduced motion, reduced transparency, forced colors, composite contrast, and focus restoration are verified.
- HUD navigation uses typed, bounded native destinations; no added network work or general write permissions.
- Extend frontend and native-window contract tests; record the commands and Windows smoke checks actually executed.

The implemented sections above record automated checks actually run. Contrast measurements and native Windows interaction/DPI smoke checks remain outstanding. Only the expanded HUD proposal remains documentation-only in this interface plan.
