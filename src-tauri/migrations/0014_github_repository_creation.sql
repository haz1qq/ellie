CREATE TABLE github_repository_creation_attempts (
    id TEXT PRIMARY KEY CHECK (length(id) BETWEEN 16 AND 128),
    account_id INTEGER NOT NULL CHECK (account_id > 0),
    account_login TEXT NOT NULL CHECK (length(account_login) BETWEEN 1 AND 100),
    repository_name TEXT NOT NULL CHECK (length(repository_name) BETWEEN 1 AND 100),
    state TEXT NOT NULL CHECK (state IN ('dispatching', 'outcome_unknown')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX github_repository_creation_attempts_updated_idx
    ON github_repository_creation_attempts(updated_at);
