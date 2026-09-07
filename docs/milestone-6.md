# Milestone 6 — Background polling

Status: implementation in progress on `feat/deepseek`.

## Implemented

- `RefreshCoordinator` owns a single async refresh critical section, a cached
  `ProviderOverview` for each registered provider, and per-provider retry state.
- Startup performs a forced refresh through the coordinator.
- A background task waits five minutes between refresh cycles and honors each
  provider's retry deadline.
- Dashboard **Refresh now**, provider-card **Refresh**, and tray **Refresh**
  use the same coordinator. A concurrent call returns cached results with
  `busy: true` rather than starting overlapping provider work.
- Fresh successful snapshots replace cached data, clear that provider's
  backoff, and are persisted to history.
- Failed refreshes retain the last successful snapshot, set `stale`, preserve
  the typed error, and expose `lastSuccessfulRefresh` and `nextRetryAt`.
  After restart, the latest persisted snapshot is used as a fallback when a
  first refresh fails.
- The dashboard receives `providers-updated` events from background and tray
  refreshes, and displays refresh age/stale status.
- Backoff starts at 30 seconds and doubles to a 30-minute maximum. Explicit
  refresh bypasses backoff; automatic polling honors it.

## Provider behavior

The coordinator does not invent provider data or replace failures with zero.
Individual adapter timeouts remain in force. A provider failure is isolated;
other providers can refresh and update normally. Only fresh successful
snapshots are inserted into history, avoiding duplicate stale rows during
backoff skips.

## Verification

Rust unit tests cover:

- stale snapshot preservation after a failed refresh;
- bounded exponential retry delay;
- skipping a provider during backoff;
- rejecting overlapping refreshes while allowing the first refresh to finish.

Frontend tests cover:

- dashboard-wide refresh;
- per-provider refresh without dropping other cards;
- stale-data messaging;
- background provider update events.

Full repository checks and Windows native smoke verification are required
before marking this milestone complete. The native smoke check must confirm
startup refresh, dashboard Refresh, per-card Refresh, tray Refresh, and
stale-data display after a provider failure.

## Deferred

Notifications remain Milestone 7. The local API remains Milestone 8.
