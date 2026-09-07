# Milestone 7 — Notifications

Status: implementation in progress on `feat/deepseek`.

## Scope

Milestone 7 implements Windows notifications for provider-reported quota
windows. The initial fixed thresholds are 75%, 90%, and 95% used. Settings
provides one enable/disable control for all usage notifications; editing
thresholds and per-provider rules remain deferred.

## Implemented

- Added the `notifications_enabled` preference with a schema migration and
  Settings toggle. Existing installations default to enabled.
- Added Rust-owned Windows notification dispatch through
  `tauri-plugin-notification`; notification commands are not exposed to the
  frontend.
- Notifications are evaluated after startup, manual, per-provider, tray, and
  polling refreshes.
- Only live, non-stale snapshots with provider-reported usage percentages can
  trigger alerts. Demo data and locally calculated estimates are ignored.
- Notification text includes provider, quota window, threshold, remaining
  percentage when reported, and reset timestamp when available.
- `notification_state` transactionally deduplicates each threshold by provider,
  window, and quota period. Provider reset timestamps are preferred for period
  identity, with window starts as a fallback. Windows without either identity
  are skipped rather than assigned a fabricated period.

## Verification

Rust tests cover threshold claims, duplicate suppression, reset-period changes,
disabled notifications, and mock-data exclusion. Frontend tests cover saving
the notification preference. Full repository checks and native Windows smoke
verification are required before marking this milestone complete.

## Deferred

Threshold editing, per-provider/window notification rules, and reset
notifications remain future enhancements. The local API remains Milestone 8.
