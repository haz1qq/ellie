-- Explicit opt-in for both new installs and upgrades; environment tokens do not enable the API.
ALTER TABLE application_settings ADD COLUMN local_api_enabled INTEGER NOT NULL DEFAULT 0 CHECK (local_api_enabled IN (0, 1));
