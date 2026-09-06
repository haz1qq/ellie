-- 0006_model.sql
-- Milestone 5 follow-up: model in use (or dominant alias) reported by the
-- provider, when the source exposes one. NULL when unknown — never invented.

ALTER TABLE usage_snapshots
    ADD COLUMN model TEXT;