ALTER TABLE application_settings ADD COLUMN notifications_enabled INTEGER NOT NULL DEFAULT 1
    CHECK (notifications_enabled IN (0, 1));
