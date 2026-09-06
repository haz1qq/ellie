-- 0005_spend_estimate.sql
-- Milestone 5 follow-up: locally-calculated spend estimate derived from
-- provider-reported balance changes across stored snapshots. All columns
-- are NULL when no estimate is computable (fewer than two snapshots, no
-- matching currency, or the balance did not decrease).

ALTER TABLE usage_snapshots
    ADD COLUMN spend_estimate_amount REAL
        CHECK (spend_estimate_amount IS NULL OR spend_estimate_amount >= 0);

ALTER TABLE usage_snapshots
    ADD COLUMN spend_estimate_currency TEXT;

ALTER TABLE usage_snapshots
    ADD COLUMN spend_estimate_window_days INTEGER
        CHECK (spend_estimate_window_days IS NULL OR spend_estimate_window_days >= 1);