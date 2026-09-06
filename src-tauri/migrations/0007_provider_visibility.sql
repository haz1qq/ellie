-- Presentation preference only: provider fetching, credentials, and history remain unchanged.
ALTER TABLE application_settings ADD COLUMN hidden_provider_ids TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(hidden_provider_ids) AND json_type(hidden_provider_ids) = 'array');
