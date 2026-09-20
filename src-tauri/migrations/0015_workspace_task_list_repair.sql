CREATE UNIQUE INDEX IF NOT EXISTS task_lists_name_key_unique_idx
    ON task_lists(name_key);

-- Databases created during the workspace development phase may have received
-- name_key through the schema-15 Rust compatibility repair. Those columns
-- cannot gain NOT NULL/CHECK constraints via ALTER TABLE, so triggers keep the
-- same invariant as fresh databases created by migration 0013.
CREATE TRIGGER IF NOT EXISTS task_lists_name_key_required_insert
BEFORE INSERT ON task_lists
WHEN NEW.name_key IS NULL OR length(NEW.name_key) < 1
BEGIN
    SELECT RAISE(ABORT, 'task list name key is required');
END;

CREATE TRIGGER IF NOT EXISTS task_lists_name_key_required_update
BEFORE UPDATE OF name_key ON task_lists
WHEN NEW.name_key IS NULL OR length(NEW.name_key) < 1
BEGIN
    SELECT RAISE(ABORT, 'task list name key is required');
END;
