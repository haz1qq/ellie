# Milestone 9 — Historical Analytics

Status: core analytics implemented.

Ellie now exposes local history through the `get_analytics` Tauri command and renders it on the dashboard. The range selector supports Today, 7 days, 30 days, and 90 days. Today begins at UTC midnight; timestamps remain UTC until display.

## Included analytics

- latest tracked token and request summaries for each live provider in the selected range
- daily token activity bars, using the latest observation per provider and day so repeated refreshes are not counted repeatedly
- estimated spend grouped by currency, including stored balance-decrease estimates and token-cost estimates
- latest provider-reported quota utilization for each provider window
- snapshot and connected-provider counts

Only `live` snapshots are included. Mock/demo snapshots, malformed timestamps, negative counters, and locally calculated quota windows are excluded from the corresponding analytics. Every token and spend result carries source metadata; the UI labels provider-reported values, Ellie-calculated estimates, and mixed results.

Spend and token totals are deliberately not presented as a universal billing total. Provider windows retain their original meaning, and balance-derived spend remains an estimate because top-ups and balance expiry can look like usage. Repeated refreshes are deduplicated rather than summed.

## Presentation

The dashboard's **History & insights** section has compact summary cards, a token trend, quota utilization bars, range controls, empty/loading/error states, and a methodology note. It is unavailable in the browser-only preview because browser preview cannot access the native SQLite database.

Advanced provider/model distributions remain deferred until their interpretation and account-specific semantics are defined.

## Verification

- Rust analytics aggregation tests cover live-only filtering, duplicate refresh deduplication, UTC-day boundaries, source metadata, quota filtering, and spend estimates.
- Frontend tests cover analytics rendering and range selection.
- Run the repository checks from the root:

```powershell
npm run typecheck
npm run lint
npm test
npm run build
npm run check:rust
```
