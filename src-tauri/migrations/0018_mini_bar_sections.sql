-- W6: expanded HUD section opt-ins. Both default OFF so the existing
-- quota-only mini window behavior is preserved on upgrade.
ALTER TABLE application_settings ADD COLUMN mini_bar_show_github INTEGER NOT NULL DEFAULT 0 CHECK (mini_bar_show_github IN (0, 1));
ALTER TABLE application_settings ADD COLUMN mini_bar_show_task INTEGER NOT NULL DEFAULT 0 CHECK (mini_bar_show_task IN (0, 1));