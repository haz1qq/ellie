# Ellie

> One place to see how much AI you have left.

## Document status

This is Ellie's product and engineering specification. It preserves the full 78-section project specification and incorporates the subsequently agreed personality direction in the UI requirements, milestone 0, v0.1 definition, and section 79.

Use this file for product scope and architecture, and `AGENTS.md` for concise coding-agent instructions. Keep both aligned when requirements change.

Current implementation: milestones 0–2 are complete (`feat/bootstrap`, `feat/provider-framework`, `feat/sqlite-history`), and milestone 3 OpenAI/Codex is complete on `feat/openai-codex` — a live provider that reads ChatGPT plan quota through the codex CLI's own app-server, reusing `codex login` without handling tokens. The demo provider remains registered alongside it. See `docs/milestone-0.md` … `docs/milestone-3.md` and `docs/providers/openai.md` for scope, sources, and verification evidence.

The milestones and checklists describe intended deliverables, not completed implementation. Provider fields, API responses, and Rust models are conceptual examples until verified and implemented. Actual provider capabilities must be researched during the relevant integration milestone; never treat example quotas as evidence of live support.

Work on one explicitly agreed milestone or scoped task at a time. Do not proceed to the next milestone without instruction.

## Contents

- [1. Project Overview](#1-project-overview)
- [2. Primary Goals](#2-primary-goals)
- [3. Initial Supported Providers](#3-initial-supported-providers)
- [4. Technology Stack](#4-technology-stack)
- [5. High-Level Architecture](#5-high-level-architecture)
- [6. Provider Architecture](#6-provider-architecture)
- [7. Provider Registry](#7-provider-registry)
- [8. Provider Capabilities](#8-provider-capabilities)
- [9. Normalized Usage Model](#9-normalized-usage-model)
- [10. Usage Window Model](#10-usage-window-model)
- [11. Token Usage Model](#11-token-usage-model)
- [12. Data Classification](#12-data-classification)
- [13. Authentication Philosophy](#13-authentication-philosophy)
- [14. Existing Authentication Detection](#14-existing-authentication-detection)
- [15. API Keys](#15-api-keys)
- [16. OAuth](#16-oauth)
- [17. Browser Credentials](#17-browser-credentials)
- [18. Provider Reliability](#18-provider-reliability)
- [19. SQLite](#19-sqlite)
- [20. Example Database Design](#20-example-database-design)
- [21. Database Migrations](#21-database-migrations)
- [22. Usage History Retention](#22-usage-history-retention)
- [23. Polling](#23-polling)
- [24. Polling Requirements](#24-polling-requirements)
- [25. Stale Data](#25-stale-data)
- [26. System Tray](#26-system-tray)
- [27. Tray Behavior](#27-tray-behavior)
- [28. Main Dashboard](#28-main-dashboard)
- [29. UI Principles](#29-ui-principles)
- [30. Provider Cards](#30-provider-cards)
- [31. Mini Floating Bar](#31-mini-floating-bar)
- [32. Notifications](#32-notifications)
- [33. Notification Rules](#33-notification-rules)
- [34. Reset Detection](#34-reset-detection)
- [35. Local REST API](#35-local-rest-api)
- [36. API Example](#36-api-example)
- [37. Pi Integration](#37-pi-integration)
- [38. CLI](#38-cli)
- [39. Project Structure](#39-project-structure)
- [40. Logging](#40-logging)
- [41. Error Handling](#41-error-handling)
- [42. Frontend Error States](#42-frontend-error-states)
- [43. Security Requirements](#43-security-requirements)
- [44. Testing Strategy](#44-testing-strategy)
- [45. Provider Fixtures](#45-provider-fixtures)
- [46. Rust Coding Standards](#46-rust-coding-standards)
- [47. TypeScript Coding Standards](#47-typescript-coding-standards)
- [48. Git Workflow](#48-git-workflow)
- [49. Commit Style](#49-commit-style)
- [50. Development Philosophy](#50-development-philosophy)
- [51. Trustworthiness Principle](#51-trustworthiness-principle)
- [52. Milestone 0 - Bootstrap](#52-milestone-0---bootstrap)
- [53. Milestone 1 - Provider Framework](#53-milestone-1---provider-framework)
- [54. Milestone 2 - SQLite and History](#54-milestone-2---sqlite-and-history)
- [55. Milestone 3 - OpenAI / Codex](#55-milestone-3---openai--codex)
- [56. Milestone 4 - Claude](#56-milestone-4---claude)
- [57. Milestone 5 - DeepSeek](#57-milestone-5---deepseek)
- [58. Milestone 6 - Background Polling](#58-milestone-6---background-polling)
- [59. Milestone 7 - Notifications](#59-milestone-7---notifications)
- [60. Milestone 8 - Local API](#60-milestone-8---local-api)
- [61. Milestone 9 - Historical Analytics](#61-milestone-9---historical-analytics)
- [62. Milestone 10 - Windows Packaging](#62-milestone-10---windows-packaging)
- [63. Ellie v0.1 Definition](#63-ellie-v01-definition)
- [64. Explicit Non-Goals for v0.1](#64-explicit-non-goals-for-v01)
- [65. Privacy Philosophy](#65-privacy-philosophy)
- [66. Application Identity](#66-application-identity)
- [67. Long-Term Architecture](#67-long-term-architecture)
- [68. Future Extension System](#68-future-extension-system)
- [69. Performance Goals](#69-performance-goals)
- [70. Startup Behavior](#70-startup-behavior)
- [71. Settings](#71-settings)
- [72. Data Export](#72-data-export)
- [73. Documentation Requirements](#73-documentation-requirements)
- [74. README Requirements](#74-readme-requirements)
- [75. Instructions for Coding Agents](#75-instructions-for-coding-agents)
- [76. Definition of Done for a Task](#76-definition-of-done-for-a-task)
- [77. Product Principle](#77-product-principle)
- [78. Core Product Question](#78-core-product-question)
- [79. Personality and Visual Identity](#79-personality-and-visual-identity)

## 1. Project Overview

Ellie is a lightweight, local-first Windows desktop application for monitoring AI provider usage.

Ellie focuses on answering one question clearly:

> **How much AI usage do I have left?**

Most existing tools focus on:

- input tokens
- output tokens
- cached tokens
- estimated cost

Ellie should additionally focus on provider-level quota information such as:

- 5-hour usage limits
- weekly usage limits
- monthly limits
- remaining credits
- account balance
- reset timestamps
- provider-specific quotas
- locally calculated token statistics
- estimated cost history

Ellie must support providers with different quota systems without pretending that all providers expose the same limits.

---

## 2. Primary Goals

Ellie should:

1. Run primarily as a Windows system tray application.
2. Retrieve actual provider-reported quota information where available.
3. Track locally observable token and cost usage.
4. Store historical usage snapshots locally.
5. Display quota percentages and reset countdowns.
6. Support multiple AI providers through provider adapters.
7. Consume minimal CPU and memory while idle.
8. Keep authentication credentials secure.
9. Expose a local API for integrations such as Pi.
10. Remain local-first with no Ellie cloud dependency.
11. Eventually support macOS and Linux.
12. Clearly distinguish real provider data from locally estimated data.

---

## 3. Initial Supported Providers

Ellie v0.1 should focus only on:

1. OpenAI / Codex
2. Anthropic / Claude
3. DeepSeek

Do not add additional providers until the provider architecture is stable.

Potential future providers:

- Google Gemini
- GitHub Copilot
- OpenRouter
- Cursor
- Windsurf
- Z.ai
- Kimi
- MiniMax
- Groq
- Mistral
- xAI / Grok
- OpenCode

---

## 4. Technology Stack

### Desktop Framework

Tauri 2

### Backend

Rust

### Frontend

React

TypeScript

Vite

### Package Manager

npm

### Rust Async Runtime

Tokio

### HTTP Client

reqwest

### Serialization

serde

serde_json

### Local Database

SQLite

Prefer:

```text
rusqlite
```

unless another well-supported Rust SQLite integration provides a clear architectural advantage.

### Local HTTP API

Axum

### Date and Time

Use either:

```text
chrono
```

or:

```text
time
```

All timestamps must be stored internally in UTC.

Convert timestamps to the user's local timezone only for display.

### Credential Storage

Use operating-system secure credential facilities.

For Windows:

- Windows Credential Manager
- or a reputable Rust keyring abstraction backed by Windows Credential Manager

Never store plaintext credentials inside SQLite.

### Frontend State

Initially prefer:

- React hooks
- React Context where required

Do not introduce Redux unless complexity later justifies it.

### Charts

Prefer:

```text
Recharts
```

Charts are not required for the earliest MVP.

### Logging

Prefer:

```text
tracing
tracing-subscriber
```

---

## 5. High-Level Architecture

```text
                      AI Providers

             OpenAI      Claude      DeepSeek
                │           │            │
                └───────────┼────────────┘
                            │
                            ▼
                   Provider Adapters
                            │
                            ▼
                    Usage Normalizer
                            │
                 ┌──────────┴──────────┐
                 │                     │
                 ▼                     ▼
           Current State          SQLite History
                 │                     │
                 └──────────┬──────────┘
                            │
                            ▼
                        Rust Core
                            │
             ┌──────────────┼──────────────┐
             │              │              │
             ▼              ▼              ▼
        Tauri Dashboard  System Tray    Local API
                                           │
                                           ▼
                                     Integrations
                                      e.g. Pi
```

The provider layer must remain independent of the UI.

The frontend should never need to understand how OpenAI, Claude, or DeepSeek authentication or API communication works.

---

## 6. Provider Architecture

Every provider should implement a common abstraction.

Conceptual Rust trait:

```rust
#[async_trait]
pub trait UsageProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn display_name(&self) -> &'static str;

    fn capabilities(&self) -> ProviderCapabilities;

    async fn detect(
        &self,
    ) -> Result<DetectionResult, ProviderError>;

    async fn authenticate(
        &self,
    ) -> Result<AuthState, ProviderError>;

    async fn fetch_usage(
        &self,
    ) -> Result<UsageSnapshot, ProviderError>;
}
```

The exact trait may change if there is a better idiomatic Rust design.

Provider-specific logic must remain contained within that provider's module wherever practical.

---

## 7. Provider Registry

Ellie should eventually expose a centralized provider registry.

Conceptual example:

```rust
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn UsageProvider>>,
}
```

Responsibilities:

- register providers
- enable/disable providers
- retrieve providers by ID
- refresh one provider
- refresh all enabled providers
- expose provider capabilities
- isolate individual provider failures

One provider failing must never crash Ellie.

---

## 8. Provider Capabilities

Different providers expose different information.

Therefore providers should declare capabilities.

Example:

```rust
pub struct ProviderCapabilities {
    pub quota_windows: bool,
    pub token_usage: bool,
    pub account_balance: bool,
    pub credits: bool,
    pub cost_tracking: bool,
    pub local_history: bool,
}
```

The frontend must dynamically display supported information.

For example:

```text
OpenAI
├── 5-hour quota
├── weekly quota
└── credits

Claude
├── 5-hour quota
├── weekly quota
└── model-specific limits

DeepSeek
├── account balance
├── API usage
└── locally calculated tokens
```

Do not invent a weekly quota for a provider that does not expose one.

---

## 9. Normalized Usage Model

Do not design Ellie around fixed database columns such as:

```text
five_hour_usage
weekly_usage
monthly_usage
```

Providers use different quota models.

Use generic usage windows instead.

Conceptual model:

```rust
pub struct UsageSnapshot {
    pub provider_id: String,
    pub account_label: Option<String>,
    pub plan: Option<String>,

    pub windows: Vec<UsageWindow>,

    pub credits: Option<CreditState>,
    pub balance: Option<BalanceState>,
    pub token_usage: Option<TokenUsage>,

    pub fetched_at: DateTime<Utc>,
}
```

---

## 10. Usage Window Model

```rust
pub struct UsageWindow {
    pub id: String,
    pub label: String,

    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,

    pub used_value: Option<f64>,
    pub remaining_value: Option<f64>,
    pub limit_value: Option<f64>,

    pub unit: Option<String>,

    pub starts_at: Option<DateTime<Utc>>,
    pub reset_at: Option<DateTime<Utc>>,
}
```

Example OpenAI data:

```text
Provider:
OpenAI

Window:
5 Hour

Used:
63%

Remaining:
37%

Reset:
2h 14m
```

Another window:

```text
Weekly
Used: 42%
Remaining: 58%
Reset: September 9
```

---

## 11. Token Usage Model

Track where available:

- input tokens
- output tokens
- cached input tokens
- cached output tokens
- reasoning tokens
- total tokens
- request count
- estimated cost

Conceptual structure:

```rust
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,

    pub cached_input_tokens: Option<u64>,
    pub cached_output_tokens: Option<u64>,

    pub reasoning_tokens: Option<u64>,

    pub total_tokens: Option<u64>,
    pub request_count: Option<u64>,

    pub estimated_cost_usd: Option<f64>,
}
```

---

## 12. Data Classification

Ellie must distinguish between two fundamentally different categories of usage information.

### Provider Reported

Data directly reported by the provider.

Examples:

```text
5-hour usage = 63%
Weekly usage = 42%
Reset at 13:00
Remaining credits = 120
Account balance = $8.42
```

Internally classify this as:

```text
Provider Reported
```

### Locally Calculated

Data calculated by Ellie from:

- CLI logs
- Pi logs
- local request history
- provider request logs
- historical snapshots

Examples:

```text
2.3M tokens today
$4.72 estimated spend
487 requests this week
Claude Sonnet = 63% of local usage
```

Internally classify this as:

```text
Locally Calculated
```

Never present locally estimated usage as official provider quota information.

---

## 13. Authentication Philosophy

Authentication is one of Ellie's most security-sensitive systems.

Preference order:

```text
Existing official authentication
        ↓
Official provider API credentials
        ↓
Official CLI/local application data
        ↓
Documented account endpoints
        ↓
Other carefully researched mechanisms
```

---

## 14. Existing Authentication Detection

Where practical, Ellie should detect authentication already created by tools such as:

```text
Codex CLI
Claude Code
Gemini CLI
```

Provider adapters should be able to report:

```text
Authenticated
Authentication detected
Authentication required
Expired
Unsupported
```

Avoid requiring users to sign in twice where there is a secure and legitimate way to reuse existing authentication.

---

## 15. API Keys

If the user manually adds an API key:

Ellie must:

- store it in the OS keyring
- never store it in SQLite
- never log it
- never include it in analytics
- never expose it through the localhost API
- avoid unnecessarily sending it to the frontend

Incorrect:

```text
SQLite:

api_key = sk-xxxxxxxx
```

Correct:

```text
Windows Credential Manager
          │
          ▼
     Rust provider
```

---

## 16. OAuth

If a provider requires OAuth:

- use a secure OAuth flow
- securely store refresh tokens
- securely store access tokens where necessary
- never log tokens
- support token refresh
- detect expired authentication
- expose only authentication state to the frontend

---

## 17. Browser Credentials

Ellie should not silently scrape:

- browser cookies
- browser password databases
- browser local storage
- session tokens

Browser credential extraction should not be Ellie's default authentication strategy.

Any investigation of undocumented systems should remain isolated from production code until the security and maintenance implications are understood.

---

## 18. Provider Reliability

Provider integrations may rely on different sources.

Priority should generally be:

```text
Official documented API
          ↓
Official CLI/local data
          ↓
Documented account API
          ↓
Provider endpoint used by official tooling
          ↓
Local logs
```

Provider adapters must handle:

- authentication failure
- rate limiting
- network failure
- unexpected responses
- provider API changes
- missing fields
- unavailable quota information

A provider failure should never terminate Ellie.

---

## 19. SQLite

SQLite stores:

- application configuration
- providers
- account metadata
- historical usage snapshots
- usage windows
- token usage
- notification state
- non-sensitive settings

Potential tables:

```text
providers
accounts
usage_snapshots
usage_windows
token_usage
application_settings
notification_rules
notification_state
```

Do not store secrets.

---

## 20. Example Database Design

### providers

```text
id
provider_key
display_name
enabled
created_at
updated_at
```

### accounts

```text
id
provider_id
account_label
plan
created_at
updated_at
```

Do not include credentials.

### usage_snapshots

```text
id
provider_id
account_id
fetched_at
source_type
```

Possible source types:

```text
provider_reported
locally_calculated
```

### usage_windows

```text
id
snapshot_id
window_key
label
used_percent
remaining_percent
used_value
remaining_value
limit_value
unit
starts_at
reset_at
```

### token_usage

```text
id
snapshot_id
input_tokens
output_tokens
cached_input_tokens
cached_output_tokens
reasoning_tokens
total_tokens
request_count
estimated_cost_usd
```

---

## 21. Database Migrations

Use migrations from the beginning.

Never manually modify production schemas.

Migration files should be version-controlled.

Example:

```text
src-tauri/migrations/

0001_initial.sql
0002_notification_rules.sql
0003_token_usage.sql
```

Migration execution should happen safely during application startup.

---

## 22. Usage History Retention

Default retention:

```text
90 days
```

Future configurable options may include:

```text
30 days
90 days
180 days
1 year
Forever
```

History cleanup should happen periodically without blocking the UI.

---

## 23. Polling

Provider usage should refresh automatically.

Default interval:

```text
5 minutes
```

Future selectable intervals:

```text
1 minute
5 minutes
10 minutes
15 minutes
30 minutes
Manual only
```

Provider adapters may define their own minimum safe refresh interval.

Avoid excessive provider requests.

---

## 24. Polling Requirements

Polling must:

- run asynchronously
- prevent overlapping refreshes
- tolerate individual provider failures
- keep the most recent successful data
- update `last_successful_refresh`
- expose stale-data state
- use reasonable timeouts

A failed refresh must not replace valid previous data with empty values.

---

## 25. Stale Data

The UI should show:

```text
Updated 42 seconds ago
```

or:

```text
Updated 7 minutes ago
```

If the latest refresh fails:

```text
Claude

Weekly
54%

Unable to refresh.
Showing data from 17 minutes ago.
```

This is preferable to removing the usage information entirely.

---

## 26. System Tray

Ellie should primarily run from the Windows notification area.

Initial tray menu:

```text
Ellie
────────────────────────

OpenAI
5h        63%
Weekly    42%

Claude
5h        81%
Weekly    54%

DeepSeek
Balance   $8.42

────────────────────────

Open Ellie
Refresh
Settings
Quit
```

---

## 27. Tray Behavior

Requirements:

- Ellie launches normally.
- Closing the main window may hide it instead of terminating Ellie.
- Ellie continues running in the tray.
- `Open Ellie` restores and focuses the dashboard.
- `Refresh` triggers provider refresh.
- `Settings` opens the settings page.
- `Quit` cleanly terminates the application.

Tray behavior must be predictable and configurable later.

---

## 28. Main Dashboard

Initial design:

```text
┌──────────────────────────────────────────────────────┐
│ Ellie                                  Updated 10s ago│
│ One place to see how much AI you have left.          │
├──────────────────────────────────────────────────────┤
│                                                      │
│ OpenAI Codex                              Plus        │
│                                                      │
│ 5 Hour                                               │
│ █████████████░░░░░░░ 63% used                        │
│ 37% remaining · Reset in 2h 14m                      │
│                                                      │
│ Weekly                                               │
│ █████████░░░░░░░░░░░ 42% used                        │
│ 58% remaining · Reset Sep 9, 8:00 AM                 │
│                                                      │
├──────────────────────────────────────────────────────┤
│                                                      │
│ Claude                                    Max         │
│                                                      │
│ 5 Hour                                               │
│ ████████████████░░░░ 81% used                        │
│                                                      │
│ Weekly                                               │
│ ███████████░░░░░░░░░ 54% used                        │
│                                                      │
├──────────────────────────────────────────────────────┤
│                                                      │
│ DeepSeek API                                         │
│                                                      │
│ Balance                                  $8.42        │
│ Tokens today                             1.8M         │
│                                                      │
└──────────────────────────────────────────────────────┘
```

---

## 29. UI Principles

Ellie should have a compact developer-tool aesthetic shaped by the personality of the owner's black-and-white cat: affectionate, clingy, cute, funny, and a little mischievous. Apply the visual and interaction rules in section 79 from the first dashboard shell.

Priorities:

1. clarity
2. accuracy
3. information density
4. responsive interaction
5. minimal visual noise

Dark mode should be implemented first.

Light mode can follow.

Use ink black, warm white, and soft gray as the foundation, with restrained pink accents. Include a simple cat mascot, a legible cat tray icon, and brief friendly messages in v0.1. Preserve accessible contrast, semantic status colors, and precise usage labels.

Do not overdesign the initial UI. Animated companion features remain deferred.

---

## 30. Provider Cards

Each provider card may contain:

```text
Provider logo
Provider name
Account / plan
Authentication status
Quota windows
Balance
Credits
Token statistics
Last refresh
Refresh status
```

Only render supported values.

Do not show placeholder quota bars suggesting data exists when the provider does not expose it.

---

## 31. Mini Floating Bar

Not required for the first milestone, but the architecture should allow it later.

Example:

```text
┌────────────────────────────────────────────┐
│ OAI 5h 63% W 42% │ Claude 5h 81% W 54%   │
└────────────────────────────────────────────┘
```

Potential requirements:

- optional
- always-on-top
- movable
- compact
- configurable opacity
- click to open Ellie
- remember window position

---

## 32. Notifications

Ellie should support Windows notifications for quota thresholds.

Initial default thresholds:

```text
75%
90%
95%
```

Possible future threshold:

```text
50%
```

Example:

```text
Ellie

OpenAI Codex weekly usage reached 90%.

10% remaining.
Reset in 2 days 8 hours.
```

---

## 33. Notification Rules

Users should eventually be able to configure notification thresholds independently per provider and quota window.

Avoid duplicate notifications.

Track which thresholds have already fired during the current quota period.

Example:

```text
Weekly period #AB123

75% → notified
90% → notified
95% → pending
```

When the quota resets, notification state should reset.

---

## 34. Reset Detection

Prefer authoritative provider reset timestamps.

Potential reset detection signals:

1. provider-supplied reset timestamp
2. provider-supplied quota-period identifier
3. reset timestamp changing
4. usage falling significantly

Do not rely on percentage drops where authoritative provider metadata exists.

---

## 35. Local REST API

Ellie should eventually expose:

```text
http://127.0.0.1:9876
```

Bind to localhost only by default.

Initial endpoints:

```text
GET  /api/v1/health
GET  /api/v1/providers
GET  /api/v1/usage
GET  /api/v1/providers/:id/usage
POST /api/v1/refresh
```

Future:

```text
GET /api/v1/history
GET /api/v1/history/:provider
GET /api/v1/settings
```

---

## 36. API Example

Request:

```http
GET /api/v1/usage
```

Response:

```json
{
  "app": "ellie",
  "version": "0.1.0",
  "providers": [
    {
      "id": "openai",
      "name": "OpenAI",
      "plan": "Plus",
      "windows": [
        {
          "id": "session",
          "label": "5 Hour",
          "usedPercent": 63,
          "remainingPercent": 37,
          "resetAt": "2026-09-06T08:00:00Z"
        },
        {
          "id": "weekly",
          "label": "Weekly",
          "usedPercent": 42,
          "remainingPercent": 58,
          "resetAt": "2026-09-09T00:00:00Z"
        }
      ]
    }
  ]
}
```

The API must never expose provider credentials.

---

## 37. Pi Integration

Pi integration must remain separate from Ellie's core application.

Architecture:

```text
OpenAI ─────┐
Claude ─────┼──► Ellie ───► localhost REST API
DeepSeek ───┘                    │
                                 ▼
                           Pi Extension
```

A future Pi extension could call:

```text
GET http://127.0.0.1:9876/api/v1/usage
```

and display:

```text
OpenAI 5h 63% | Week 42%
Claude 5h 81% | Week 54%
```

This keeps authentication and provider logic centralized inside Ellie.

## 38. CLI

A future CLI should use the same Ellie core.

Executable:

```text
ellie
```

Example:

```powershell
ellie status
```

Output:

```text
Ellie

OpenAI Codex
  5 Hour    63% used     Reset 2h 14m
  Weekly    42% used     Reset 3d 7h

Claude
  5 Hour    81% used     Reset 3h 22m
  Weekly    54% used     Reset 4d 2h

DeepSeek
  Balance   $8.42
```

Possible future commands:

```text
ellie status
ellie refresh
ellie providers
ellie history
ellie version
```

The CLI is not required for v0.1.

## 39. Project Structure

Initial recommended structure:

```text
ellie/
│
├── src/
│   ├── components/
│   ├── pages/
│   ├── hooks/
│   ├── lib/
│   ├── types/
│   ├── styles/
│   │
│   ├── App.tsx
│   └── main.tsx
│
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   │
│   │   ├── providers/
│   │   │   ├── mod.rs
│   │   │   ├── registry.rs
│   │   │   ├── models.rs
│   │   │   ├── openai.rs
│   │   │   ├── anthropic.rs
│   │   │   └── deepseek.rs
│   │   │
│   │   ├── usage/
│   │   │   ├── mod.rs
│   │   │   ├── models.rs
│   │   │   ├── aggregator.rs
│   │   │   └── polling.rs
│   │   │
│   │   ├── storage/
│   │   │   ├── mod.rs
│   │   │   ├── database.rs
│   │   │   └── migrations.rs
│   │   │
│   │   ├── credentials/
│   │   │   └── mod.rs
│   │   │
│   │   ├── notifications/
│   │   │   └── mod.rs
│   │   │
│   │   ├── tray/
│   │   │   └── mod.rs
│   │   │
│   │   ├── api/
│   │   │   └── mod.rs
│   │   │
│   │   ├── settings/
│   │   │   └── mod.rs
│   │   │
│   │   └── logging/
│   │       └── mod.rs
│   │
│   ├── migrations/
│   │
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── scripts/
│   └── investigations/
│
├── tests/
│   └── fixtures/
│       ├── openai/
│       ├── anthropic/
│       └── deepseek/
│
├── docs/
│   ├── architecture.md
│   ├── security.md
│   │
│   └── providers/
│       ├── openai.md
│       ├── anthropic.md
│       └── deepseek.md
│
├── AGENTS.md
├── PROJECT.md
├── README.md
├── package.json
└── package-lock.json
```

Exact structure may evolve where justified.

## 40. Logging

Use structured Rust logging.

Suggested libraries:

```text
tracing
tracing-subscriber
```

Logs may include:

```text
timestamp
severity
provider
operation
duration
error category
```

Never log:

- API keys
- OAuth tokens
- refresh tokens
- cookies
- Authorization headers
- passwords
- session credentials

Sensitive values must be redacted before logging.

## 41. Error Handling

Avoid production use of:

```rust
.unwrap()
.expect()
```

where failure is realistically possible.

Prefer typed errors.

Potential libraries:

```text
thiserror
anyhow
```

Use typed provider/domain errors where practical.

Example categories:

```text
AuthenticationRequired
AuthenticationExpired
RateLimited
NetworkUnavailable
ProviderUnavailable
UnsupportedFeature
MalformedResponse
DatabaseError
CredentialStoreError
```

## 42. Frontend Error States

Convert backend failures into understandable UI messages.

Examples:

```text
Authentication required
```

```text
Unable to contact OpenAI
```

```text
Claude usage information is temporarily unavailable
```

```text
This account does not expose a weekly quota
```

```text
Showing usage from 12 minutes ago
```

Do not expose raw stack traces to normal users.

## 43. Security Requirements

Ellie handles sensitive authentication information.

The following rules are mandatory:

- Never store plaintext API keys in SQLite.
- Never commit credentials.
- Never print credentials to logs.
- Never expose credentials through Tauri commands.
- Never expose credentials through the localhost API.
- Bind the API to 127.0.0.1 by default.
- Minimize Tauri permissions.
- Validate frontend-to-Rust command arguments.
- Use HTTPS for provider requests.
- Treat provider responses as untrusted input.
- Avoid arbitrary shell command execution.
- Do not silently scrape browser credentials.
- Sanitize stored provider responses where appropriate.
- Redact sensitive fields in debugging output.

## 44. Testing Strategy

### Rust

Test:

- provider response parsing
- quota normalization
- percentage calculations
- reset detection
- database behavior
- migrations
- notification thresholds
- stale-data behavior
- provider registry behavior

### Frontend

Test:

- provider cards
- loading states
- failure states
- quota displays
- reset countdown displays
- settings interactions

## 45. Provider Fixtures

Provider parsers should use sanitized response fixtures where useful.

Example:

```text
tests/fixtures/openai/usage.json
tests/fixtures/openai/usage_missing_reset.json

tests/fixtures/anthropic/usage.json
tests/fixtures/anthropic/rate_limited.json

tests/fixtures/deepseek/balance.json
```

Never commit real credentials or identifiable account data into fixtures.

## 46. Rust Coding Standards

All Rust code should pass:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test
```

Before a milestone is considered complete.

Prefer:

- idiomatic Rust
- small modules
- explicit domain models
- meaningful error handling
- minimal unnecessary cloning
- async where appropriate
- traits where they improve testability
- dependency injection where useful

Avoid abstraction for abstraction's sake.

## 47. TypeScript Coding Standards

Enable strict TypeScript.

Avoid:

```typescript
any
```

unless absolutely necessary.

Prefer:

- typed Tauri command responses
- reusable domain interfaces
- small components
- explicit loading/error states
- frontend components that are provider-neutral

Keep provider-specific HTTP/auth logic out of React.

## 48. Git Workflow

Primary branch:

```text
main
```

Development should happen on feature branches.

Examples:

```text
feat/bootstrap
feat/provider-abstraction
feat/sqlite-history
feat/openai-provider
feat/claude-provider
feat/deepseek-provider
feat/system-tray
feat/local-api
feat/notifications
```

Fixes:

```text
fix/openai-reset-time
fix/claude-auth-detection
fix/tray-window-focus
```

## 49. Commit Style

Prefer conventional commits:

```text
feat: bootstrap Ellie desktop application

feat(providers): add provider registry

feat(openai): implement Codex quota retrieval

feat(claude): add Claude usage adapter

fix(storage): prevent duplicate snapshots

refactor(usage): normalize quota windows

docs: document provider authentication

test(openai): add quota parser fixtures
```

Do not automatically merge development branches into main.

## 50. Development Philosophy

Do not attempt to implement the entire application in one agent run.

Build Ellie milestone by milestone.

Each milestone should:

- compile
- run
- include relevant tests
- leave the repository usable
- update documentation where necessary

Never add fake provider implementations that display realistic-looking invented data.

Mock providers are permitted during architectural development only when clearly labeled as mock data.

## 51. Trustworthiness Principle

Correctness is more important than provider count.

If Ellie cannot retrieve a particular quota:

Display:

```text
Unavailable
```

or:

```text
Not supported by provider
```

Do not estimate an official quota unless the UI clearly describes it as an estimate.

## 52. Milestone 0 - Bootstrap

Goal:

Create the base Ellie application.

Implement:

- Tauri 2
- Rust backend
- React
- TypeScript
- Vite
- npm
- dark dashboard shell with Ellie's black-and-white identity
- simple static cat mascot and a small set of warm messages
- Windows system tray with a legible cat icon
- SQLite initialization
- migration mechanism
- structured logging
- settings infrastructure
- initial module structure

Do not implement provider integrations yet.

Completion criteria:

```powershell
npm run tauri dev
```

successfully launches Ellie.

Ellie can:

- open
- hide/minimize
- remain in tray
- reopen from tray
- quit cleanly

## 53. Milestone 1 - Provider Framework

Implement:

```text
UsageProvider
ProviderRegistry
ProviderCapabilities
UsageSnapshot
UsageWindow
TokenUsage
ProviderError
DetectionResult
AuthState
```

Add a clearly identified mock provider.

The frontend must receive mock data through the same provider-neutral architecture that real providers will use.

Completion criterion:

No React component contains provider API/auth implementation details.

## 54. Milestone 2 - SQLite and History

Implement migrations and tables for:

```text
providers
accounts
usage_snapshots
usage_windows
token_usage
settings
notification_rules
notification_state
```

Implement:

```text
insert snapshot
retrieve latest snapshot
retrieve history
cleanup history
```

Default retention:

```text
90 days
```

## 55. Milestone 3 - OpenAI / Codex

Research existing Codex authentication and usage sources.

Determine the safest legitimate way to retrieve:

- plan
- 5-hour usage
- weekly usage
- reset timestamps
- credits where available

Implement:

```text
OpenAiProvider
```

Prefer reuse of existing Codex authentication where securely possible.

Document findings and implementation in:

```text
docs/providers/openai.md
```

Do not silently scrape browser credentials.

## 56. Milestone 4 - Claude

Research:

- Claude Code authentication
- available local usage information
- provider quota endpoints where appropriate

Implement:

```text
AnthropicProvider
```

Retrieve where available:

- plan
- 5-hour usage
- weekly usage
- model-specific limits
- reset timestamps

Document:

```text
docs/providers/anthropic.md
```

## 57. Milestone 5 - DeepSeek

Prefer official DeepSeek API functionality.

Implement:

```text
DeepSeekProvider
```

Potential information:

- account balance
- available credits
- API availability
- locally observed token consumption
- estimated cost

Do not display a 5-hour or weekly quota unless DeepSeek actually exposes one.

Document:

```text
docs/providers/deepseek.md
```

## 58. Milestone 6 - Background Polling

Implement:

```text
automatic refresh
manual refresh
per-provider refresh
refresh timeout
failure handling
stale-data detection
```

Default:

```text
5 minutes
```

Prevent simultaneous overlapping refresh cycles.

## 59. Milestone 7 - Notifications

Implement Windows notifications.

Default thresholds:

```text
75%
90%
95%
```

Requirements:

- once per threshold per quota period
- reset notification state after quota reset
- configurable enable/disable
- provider and window information included

## 60. Milestone 8 - Local API

Implement Axum server bound to:

```text
127.0.0.1:9876
```

Initial routes:

```text
GET  /api/v1/health
GET  /api/v1/providers
GET  /api/v1/usage
GET  /api/v1/providers/:id/usage
POST /api/v1/refresh
```

Do not expose secrets.

## 61. Milestone 9 - Historical Analytics

Dashboard ranges:

```text
Today
7 days
30 days
90 days
```

Potential analytics:

- tokens over time
- estimated spend
- requests
- quota utilization
- provider distribution
- model distribution

Do not prioritize advanced analytics until quota collection is reliable.

## 62. Milestone 10 - Windows Packaging

Produce a distributable Windows application.

Target:

```text
ellie.exe
```

Installer:

```text
NSIS
```

Potential features:

- install Ellie
- uninstall Ellie
- optional Start with Windows
- shortcuts where appropriate
- preserve local user data appropriately

The installed application must not require:

```text
Rust
Cargo
Node.js
npm
Python
```

Those are development dependencies only.

## 63. Ellie v0.1 Definition

Ellie v0.1 is complete when it has:

```text
✓ Windows desktop application
✓ Windows system tray
✓ Tauri dashboard
✓ Rust core
✓ OpenAI / Codex integration
✓ Claude integration
✓ DeepSeek integration
✓ generic quota windows
✓ quota reset countdowns
✓ SQLite history
✓ automatic refresh
✓ manual refresh
✓ stale-data handling
✓ Windows notifications
✓ secure credential storage
✓ local REST API
✓ local token statistics where available
✓ dark mode
✓ Ellie's black-and-white visual identity
✓ static cat mascot and cat tray icon
✓ brief, affectionate interface copy with precise usage labels
✓ Windows installer
```

## 64. Explicit Non-Goals for v0.1

Do not prioritize:

```text
Ellie cloud account
cloud synchronization
remote Ellie server
mobile app
macOS release
Linux release
team collaboration
provider marketplace
browser extension
30+ providers
AI chat interface
automatic provider switching
advanced forecasting
enterprise dashboards
```

Keep v0.1 focused and local-first.

## 65. Privacy Philosophy

Ellie should be local-first by default.

```text
No Ellie account
No mandatory cloud service
No remote usage database
No advertising
No selling usage data
No default telemetry
```

Any future telemetry must be explicitly opt-in.

## 66. Application Identity

Project:

```text
Ellie
```

Repository:

```text
ellie
```

Default development folder:

```text
C:\dev\ellie
```

Executable:

```text
ellie.exe
```

Future CLI:

```text
ellie
```

Local API:

```text
http://127.0.0.1:9876
```

Tagline:

One place to see how much AI you have left.

## 67. Long-Term Architecture

```text
                         Ellie Core
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
   Desktop Dashboard        CLI             Local API
                                                 │
                           ┌─────────────────────┼──────────────┐
                           │                     │              │
                           ▼                     ▼              ▼
                      Pi Extension           VS Code       Other Tools
```

The core provider logic should be reusable across these interfaces.

## 68. Future Extension System

Provider plugins may eventually become desirable.

Do not implement a dynamic plugin runtime in v0.1.

However, provider modules should remain sufficiently isolated that adding a new provider requires minimal changes to unrelated systems.

Ideal future workflow:

```text
Create provider adapter
        ↓
Register provider
        ↓
Declare capabilities
        ↓
Frontend automatically renders supported data
```

## 69. Performance Goals

Ellie is intended to run all day.

Therefore:

- avoid constant polling
- avoid unnecessary frontend re-renders
- avoid heavyweight background tasks
- avoid loading full history when unnecessary
- use SQLite indexes where appropriate
- cache current provider state
- close unused network connections appropriately
- avoid memory growth over time

Idle resource usage should remain minimal.

## 70. Startup Behavior

Desired startup flow:

```text
Launch Ellie
    │
    ├── initialize logging
    ├── initialize SQLite
    ├── run migrations
    ├── load settings
    ├── initialize credential storage
    ├── register providers
    ├── load most recent snapshots
    ├── create system tray
    ├── start refresh scheduler
    └── display dashboard
```

Failures in individual providers must not prevent Ellie from launching.

## 71. Settings

Future settings categories:

```text
General
Providers
Notifications
Appearance
Data
Integrations
Advanced
About
```

### General

```text
Start with Windows
Close to tray
Refresh interval
Launch minimized
```

### Providers

```text
OpenAI
Claude
DeepSeek
```

Each provider should expose only its relevant configuration.

### Notifications

Configure thresholds.

### Appearance

```text
Dark
Light
System
```

### Data

```text
History retention
Export history
Clear local history
```

### Integrations

```text
Local API
Pi
Future integrations
```

## 72. Data Export

Not required for early v0.1 but design storage so future export is straightforward.

Potential formats:

```text
CSV
JSON
```

Examples:

```text
ellie-usage-2026-09.csv
ellie-history.json
```

Exports must never contain credentials.

## 73. Documentation Requirements

Maintain:

```text
README.md
PROJECT.md
AGENTS.md
docs/architecture.md
docs/security.md
docs/providers/openai.md
docs/providers/anthropic.md
docs/providers/deepseek.md
```

Provider documentation should explain:

- data source
- authentication mechanism
- fields available
- quota interpretation
- limitations
- refresh behavior
- failure cases

## 74. README Requirements

README should eventually include:

- what Ellie is
- screenshots
- features
- supported providers
- installation
- development prerequisites
- development instructions
- privacy model
- architecture overview
- contributing information
- license

## 75. Instructions for Coding Agents

Any coding agent working on Ellie must follow these rules.

- Read PROJECT.md before making architectural decisions.
- Read AGENTS.md before modifying code.
- Inspect the existing repository before writing code.
- Do not replace working architecture without strong justification.
- Work on one milestone or clearly scoped task at a time.
- Run appropriate tests after changes.
- Run Rust formatting.
- Run Rust linting.
- Run TypeScript checks.
- Update documentation for non-obvious behavior.
- Never fabricate provider quota information.
- Never expose credentials.
- Keep provider implementations isolated.
- Prefer provider-neutral domain models.
- Keep Windows as the primary v0.1 platform.
- Keep Ellie local-first.
- Do not add large dependencies without justification.
- Do not perform destructive database changes without careful migration design.
- Do not automatically merge or push code.
- Keep changes logically scoped.
- Prefer correctness over speed.
- Clearly state when provider functionality is unsupported.
- Never silently scrape browser credentials.
- Keep provider-reported and locally-calculated data distinct.
- Do not continue into the next milestone unless explicitly instructed.

## 76. Definition of Done for a Task

A coding task should not be considered complete merely because code was written.

Where applicable, completion means:

```text
✓ code compiles
✓ application runs
✓ cargo fmt passes
✓ cargo clippy passes
✓ cargo test passes
✓ TypeScript typecheck passes
✓ frontend lint passes
✓ relevant documentation updated
✓ no known credentials leaked
✓ errors handled reasonably
✓ implementation matches requested scope
```

## 77. Product Principle

Ellie's most important product principle is:

> Trust the number.

A usage monitor becomes useless if users cannot tell whether a number is:

- official
- calculated
- estimated
- stale
- unavailable

Ellie should communicate that distinction clearly.

## 78. Core Product Question

Every significant product decision should help Ellie answer:

> How much AI usage do I have left?

That is the core purpose of the application.


## 79. Personality and Visual Identity

### The story behind Ellie

Ellie is named after the owner's cat. She has black-and-white fur and an affectionate, clingy, cute, and funny personality. The application should feel like Ellie quietly keeping the user company while they work.

Her personality is part of the product from the first version. It should appear in the palette, mascot, tray icon, and writing. It does not require a conversational AI, an AI chat interface, or additional provider requests.

### Visual direction

- Use ink black, warm white, and soft gray as the main palette.
- Use restrained pink accents for small decorative details or selection treatments.
- Start with dark mode. A future light theme should preserve the same identity.
- Retain recognizable status colors for warnings and errors. Always pair color with text or an icon; color alone must not convey state.
- Maintain readable contrast, visible keyboard focus, and clear controls at normal Windows display scaling.
- Keep provider cards compact and provider-neutral. Usage values, remaining allowances, reset times, and freshness should be the most prominent information.
- Use cat and paw details sparingly. Decorative shapes must not obscure labels, reduce click targets, or replace standard navigation affordances.
- Establish reusable color, spacing, typography, and component tokens rather than scattering presentation values throughout the UI.

The palette is a design direction, not a fixed set of color values. Choose exact values during interface implementation and verify contrast.

### Mascot and tray icon

Use a simple black-and-white cat illustration. Until reference photos are supplied, treat the mascot as an illustrative interpretation; do not claim invented markings match the real Ellie.

The dashboard mascot may peek over a card or rest beside the usage summary. Keep it outside essential information and controls. Purely decorative artwork should be hidden from assistive technology; interactive mascot controls need meaningful accessible names.

The tray icon should use a clear cat silhouette that remains recognizable at small sizes and on light or dark Windows taskbars. Provide a useful text tooltip. Any warning state must also be available in text through the normal interface.

Keep visual assets and copy separate from provider authentication, parsing, and domain logic. Reuse the same normalized state across the dashboard, tray, and future companion.

### Voice and writing

Ellie's voice is warm, curious, affectionate, and mildly mischievous. Prefer short, natural messages. Avoid excessive cat puns, baby talk, repetitive greetings, or jokes in serious error explanations.

Personality accompanies factual information; it never replaces it.

| Situation | Optional personality line | Required factual behavior |
| --- | --- | --- |
| Opening the dashboard | There you are. I saved your spot. | Show current provider data and its freshness. |
| Refreshing | Let me check on that. | Show an accurate in-progress state without overlapping refreshes. |
| High usage | Getting a little low. | State the provider, quota window, remaining allowance, and reset when known. |
| Refresh failure | Couldn't fetch that. | Explain the failure and show the age of the last successful data. |
| Confirmed reset | Fresh allowance. I call the warm seat. | Announce a reset only when supported by the provider data. |

These lines are examples, not required messages on every interaction. Do not emit extra notifications simply to show personality. Keep technical details available where useful, but do not expose raw stack traces or secrets to ordinary users.

### Affection without interruption

Express clinginess as a welcoming presence. Do not demand attention, guilt the user for leaving, repeatedly open windows, steal focus, or generate unnecessary notifications.

The core workflow must remain fast: open Ellie, read the numbers, return to work. Personality must not slow refreshes or add idle background work.

### Scope and milestone placement

Milestone 0 includes the palette, static dashboard mascot, cat tray icon, and a small set of friendly messages as part of the existing dashboard shell and tray work. These are presentation requirements; milestone 0 still excludes provider integrations.

Milestone 1 uses clearly labeled mock data. Apply the same trust and writing rules to loading, unavailable, and mock states. Do not let playful presentation make invented values appear official.

As real providers are implemented, keep the same visual identity and add precise provider-neutral states. Milestone 7 notifications may include a short personality line while retaining the required provider, window, threshold, and period information.

The following require a separately authorized future milestone:

- animated desktop pet or screen-edge companion
- always-on-top companion behavior
- elaborate expressions and reactions
- floating-bar interactions beyond the already agreed scope

Do not advance to these features automatically. They must not delay reliable quota collection, secure credential storage, or Windows packaging.

### Motion and optional companion behavior

Any motion should be subtle and infrequent, respect reduced-motion preferences, and provide an equivalent static state. Do not use continuous animation solely to make Ellie seem alive.

A future companion must be optional and easy to hide. It should not block application controls or steal keyboard focus. Clicking it may open the existing dashboard. Closing or disabling it must leave quota monitoring usable.

### Acceptance criteria

- Ellie has a consistent black-and-white identity in the dashboard and tray from the first shell.
- The mascot is recognizable without dominating the usage information.
- The interface communicates official, calculated, estimated, stale, and unavailable values explicitly.
- Friendly copy accompanies precise labels and actionable errors.
- Color, decorative artwork, and motion are never the only way to understand state.
- Keyboard navigation, focus visibility, reduced-motion behavior where relevant, and tray readability are verified.
- Personality adds no provider calls and no unnecessary background work.
- Animated companion features remain deferred unless explicitly authorized.

The design principle remains: **Trust the number.** Ellie gives that trustworthy tool a familiar personality.
