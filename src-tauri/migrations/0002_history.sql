-- 0002_history.sql
-- Milestone 2: provider identity, account metadata, snapshot history, and the
-- notification table stubs. Timestamps are UTC ISO 8601 with milliseconds and
-- a trailing 'Z' (lexicographic order equals chronological order).
-- Provenance columns: usage_snapshots.data_kind ('live' | 'mock') marks the
-- snapshot source; usage_windows.source and token_usage.source preserve
-- provider_reported vs locally_calculated per metric.

CREATE TABLE providers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    account_label TEXT,
    plan TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- NULL account labels (no connected account) still deduplicate to one row.
CREATE UNIQUE INDEX accounts_provider_label
    ON accounts(provider_id, COALESCE(account_label, ''));

CREATE TABLE usage_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    fetched_at TEXT NOT NULL,
    data_kind TEXT NOT NULL CHECK (data_kind IN ('live', 'mock')),
    auth_state TEXT NOT NULL,
    capabilities_json TEXT NOT NULL,
    credits REAL CHECK (credits IS NULL OR credits >= 0),
    balance REAL CHECK (balance IS NULL OR balance >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX usage_snapshots_provider_fetched
    ON usage_snapshots(provider_id, fetched_at);

CREATE TABLE usage_windows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id INTEGER NOT NULL REFERENCES usage_snapshots(id) ON DELETE CASCADE,
    window_key TEXT NOT NULL,
    label TEXT NOT NULL,
    used_percent REAL CHECK (used_percent IS NULL OR (used_percent >= 0 AND used_percent <= 100)),
    remaining_percent REAL CHECK (remaining_percent IS NULL OR (remaining_percent >= 0 AND remaining_percent <= 100)),
    used_value REAL CHECK (used_value IS NULL OR used_value >= 0),
    remaining_value REAL CHECK (remaining_value IS NULL OR remaining_value >= 0),
    limit_value REAL CHECK (limit_value IS NULL OR limit_value >= 0),
    unit TEXT,
    starts_at TEXT,
    reset_at TEXT,
    source TEXT NOT NULL CHECK (source IN ('provider_reported', 'locally_calculated'))
);

CREATE INDEX usage_windows_snapshot ON usage_windows(snapshot_id);

CREATE TABLE token_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id INTEGER NOT NULL UNIQUE REFERENCES usage_snapshots(id) ON DELETE CASCADE,
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cached_input_tokens INTEGER CHECK (cached_input_tokens IS NULL OR cached_input_tokens >= 0),
    cached_output_tokens INTEGER CHECK (cached_output_tokens IS NULL OR cached_output_tokens >= 0),
    reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    total_tokens INTEGER CHECK (total_tokens IS NULL OR total_tokens >= 0),
    request_count INTEGER CHECK (request_count IS NULL OR request_count >= 0),
    estimated_cost_usd REAL CHECK (estimated_cost_usd IS NULL OR estimated_cost_usd >= 0),
    source TEXT NOT NULL CHECK (source IN ('provider_reported', 'locally_calculated'))
);

-- Notification rule and state tables are created here per the milestone 2
-- migration target; their behavior lands with the notifications milestone.
CREATE TABLE notification_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER REFERENCES providers(id) ON DELETE CASCADE,
    window_key TEXT,
    threshold_percent REAL NOT NULL CHECK (threshold_percent > 0 AND threshold_percent <= 100),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (provider_id, window_key, threshold_percent)
);

CREATE TABLE notification_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    window_key TEXT NOT NULL,
    period_key TEXT NOT NULL,
    threshold_percent REAL NOT NULL,
    notified_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (provider_id, window_key, period_key, threshold_percent)
);