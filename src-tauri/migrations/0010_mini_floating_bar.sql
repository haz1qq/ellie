ALTER TABLE application_settings ADD COLUMN mini_bar_enabled INTEGER NOT NULL DEFAULT 0 CHECK (mini_bar_enabled IN (0, 1));
ALTER TABLE application_settings ADD COLUMN mini_bar_opacity REAL NOT NULL DEFAULT 0.9 CHECK (mini_bar_opacity >= 0.5 AND mini_bar_opacity <= 1.0);
ALTER TABLE application_settings ADD COLUMN mini_bar_x INTEGER;
ALTER TABLE application_settings ADD COLUMN mini_bar_y INTEGER;
