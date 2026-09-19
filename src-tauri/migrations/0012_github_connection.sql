CREATE TABLE github_connection (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    client_id TEXT NOT NULL CHECK (length(client_id) BETWEEN 1 AND 128),
    account_id INTEGER CHECK (account_id IS NULL OR account_id > 0),
    account_login TEXT,
    updated_at TEXT NOT NULL,
    CHECK ((account_id IS NULL) = (account_login IS NULL))
);
