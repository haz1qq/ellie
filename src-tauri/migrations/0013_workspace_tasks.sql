CREATE TABLE task_lists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
        CHECK (length(name) BETWEEN 1 AND 80 AND name = trim(name)),
    name_key TEXT NOT NULL UNIQUE CHECK (length(name_key) >= 1),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    list_id INTEGER NOT NULL REFERENCES task_lists(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200 AND title = trim(title)),
    notes TEXT CHECK (notes IS NULL OR length(notes) <= 4000),
    priority TEXT NOT NULL DEFAULT 'none'
        CHECK (priority IN ('none', 'low', 'medium', 'high')),
    due_date TEXT CHECK (due_date IS NULL OR length(due_date) = 10),
    repository_id INTEGER CHECK (repository_id IS NULL OR repository_id > 0),
    repository_full_name TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((repository_id IS NULL) = (repository_full_name IS NULL))
);

CREATE INDEX tasks_list_order_idx
    ON tasks(list_id, completed_at, due_date, created_at, id);
CREATE INDEX tasks_priority_idx ON tasks(priority);
CREATE INDEX tasks_due_date_idx ON tasks(due_date) WHERE completed_at IS NULL;

CREATE TABLE workspace_task_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    pinned_task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL
);

INSERT INTO workspace_task_state (id, pinned_task_id) VALUES (1, NULL);

CREATE TRIGGER workspace_pinned_task_must_be_incomplete_insert
BEFORE INSERT ON workspace_task_state
WHEN NEW.pinned_task_id IS NOT NULL
     AND EXISTS (SELECT 1 FROM tasks WHERE id = NEW.pinned_task_id AND completed_at IS NOT NULL)
BEGIN
    SELECT RAISE(ABORT, 'pinned task must be incomplete');
END;

CREATE TRIGGER workspace_pinned_task_must_be_incomplete_update
BEFORE UPDATE OF pinned_task_id ON workspace_task_state
WHEN NEW.pinned_task_id IS NOT NULL
     AND EXISTS (SELECT 1 FROM tasks WHERE id = NEW.pinned_task_id AND completed_at IS NOT NULL)
BEGIN
    SELECT RAISE(ABORT, 'pinned task must be incomplete');
END;

CREATE TRIGGER workspace_clear_pin_on_task_completion
AFTER UPDATE OF completed_at ON tasks
WHEN NEW.completed_at IS NOT NULL
BEGIN
    UPDATE workspace_task_state
    SET pinned_task_id = NULL
    WHERE id = 1 AND pinned_task_id = NEW.id;
END;
