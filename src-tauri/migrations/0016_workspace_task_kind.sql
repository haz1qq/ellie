ALTER TABLE tasks ADD COLUMN task_kind TEXT NOT NULL DEFAULT 'personal'
    CHECK (task_kind IN ('work', 'personal'));

-- Repository-linked tasks predate explicit task types and are work tasks.
UPDATE tasks SET task_kind = 'work' WHERE repository_id IS NOT NULL;

CREATE TRIGGER task_personal_repository_insert
BEFORE INSERT ON tasks
WHEN NEW.task_kind = 'personal' AND NEW.repository_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'personal task cannot link a repository');
END;

CREATE TRIGGER task_personal_repository_update
BEFORE UPDATE OF task_kind, repository_id ON tasks
WHEN NEW.task_kind = 'personal' AND NEW.repository_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'personal task cannot link a repository');
END;
