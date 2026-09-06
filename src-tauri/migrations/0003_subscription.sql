-- 0003_subscription.sql
-- Milestone 4: record whether the account had an active subscription at
-- snapshot time. NULL = unknown or not subscription-based (API-billed);
-- 1 = subscribed; 0 = explicitly unsubscribed (card hidden downstream).

ALTER TABLE usage_snapshots
    ADD COLUMN has_subscription INTEGER
        CHECK (has_subscription IS NULL OR has_subscription IN (0, 1));