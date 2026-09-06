-- 0004_balance_currency.sql
-- Milestone 5: ISO-4217 currency code for the account balance, so monetary
-- values are displayed with their actual currency instead of a fabricated
-- one. NULL when no balance is reported.

ALTER TABLE usage_snapshots
    ADD COLUMN balance_currency TEXT;