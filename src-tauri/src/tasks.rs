use std::{fmt, path::PathBuf};

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

const MAX_LIST_NAME_CHARS: usize = 80;
const MAX_TASK_TITLE_CHARS: usize = 200;
const MAX_TASK_NOTES_CHARS: usize = 4_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskErrorCategory {
    WindowDenied,
    InvalidInput,
    NotFound,
    Conflict,
    CountChanged,
    Storage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    category: TaskErrorCategory,
}

impl TaskError {
    pub const fn new(category: TaskErrorCategory) -> Self {
        Self { category }
    }

    pub const fn category(self) -> TaskErrorCategory {
        self.category
    }

    pub const fn window_denied() -> Self {
        Self::new(TaskErrorCategory::WindowDenied)
    }

    fn invalid_input() -> Self {
        Self::new(TaskErrorCategory::InvalidInput)
    }

    fn not_found() -> Self {
        Self::new(TaskErrorCategory::NotFound)
    }

    fn conflict() -> Self {
        Self::new(TaskErrorCategory::Conflict)
    }

    fn count_changed() -> Self {
        Self::new(TaskErrorCategory::CountChanged)
    }

    fn storage() -> Self {
        Self::new(TaskErrorCategory::Storage)
    }
}

impl fmt::Display for TaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.category {
            TaskErrorCategory::WindowDenied => "the command is not available to this window",
            TaskErrorCategory::InvalidInput => "the task input is invalid",
            TaskErrorCategory::NotFound => "the requested task item was not found",
            TaskErrorCategory::Conflict => "a task item with that identity already exists",
            TaskErrorCategory::CountChanged => "the list changed before deletion was confirmed",
            TaskErrorCategory::Storage => "local task storage is unavailable",
        })
    }
}

impl std::error::Error for TaskError {}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    #[default]
    None,
    Low,
    Medium,
    High,
}

impl TaskPriority {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    fn parse(value: &str) -> Result<Self, TaskError> {
        match value {
            "none" => Ok(Self::None),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(TaskError::storage()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Work,
    #[default]
    Personal,
}

impl TaskKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Personal => "personal",
        }
    }

    fn parse(value: &str) -> Result<Self, TaskError> {
        match value {
            "work" => Ok(Self::Work),
            "personal" => Ok(Self::Personal),
            _ => Err(TaskError::storage()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRepositoryInput {
    pub repository_id: u64,
    pub full_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRepositoryLink {
    pub repository_id: u64,
    pub full_name: String,
    pub html_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskList {
    pub id: i64,
    pub name: String,
    pub task_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: i64,
    pub list_id: i64,
    pub title: String,
    pub notes: Option<String>,
    pub kind: TaskKind,
    pub priority: TaskPriority,
    pub due_date: Option<String>,
    pub repository: Option<TaskRepositoryLink>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskBootstrap {
    pub lists: Vec<TaskList>,
    pub tasks: Vec<TaskItem>,
    pub pinned_task_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TaskNoteSnapshot {
    pub task: Option<TaskItem>,
    pub position: Option<(i32, i32)>,
}

/// Minimal HUD task projection: pinned state plus the task itself.
pub struct HudTask {
    pub task: TaskItem,
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDeletePreview {
    pub list_id: i64,
    pub task_count: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub list_id: i64,
    pub title: String,
    pub notes: Option<String>,
    #[serde(default)]
    pub kind: TaskKind,
    #[serde(default)]
    pub priority: TaskPriority,
    pub due_date: Option<String>,
    pub repository: Option<TaskRepositoryInput>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionFilter {
    #[default]
    All,
    Open,
    Completed,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskQuery {
    pub list_id: Option<i64>,
    #[serde(default)]
    pub completion: CompletionFilter,
    pub priority: Option<TaskPriority>,
}

#[derive(Clone)]
pub struct TaskService {
    database_path: PathBuf,
}

impl TaskService {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }

    pub fn bootstrap(&self) -> Result<TaskBootstrap, TaskError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        ensure_default_list(&transaction)?;
        let bootstrap = TaskBootstrap {
            lists: read_lists(&transaction)?,
            tasks: read_tasks(&transaction, &TaskQuery::default())?,
            pinned_task_id: read_pinned_task_id(&transaction)?,
        };
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(bootstrap)
    }

    pub fn list_tasks(&self, query: TaskQuery) -> Result<Vec<TaskItem>, TaskError> {
        validate_optional_id(query.list_id)?;
        read_tasks(&self.connect()?, &query)
    }

    /// Least-privilege HUD projection: the pinned task when it is still open,
    /// otherwise the next open task in the same order the To-do board shows
    /// (soonest due date, then oldest). Never returns completed tasks.
    pub fn hud_task(&self) -> Result<Option<HudTask>, TaskError> {
        let connection = self.connect()?;
        if let Some(task_id) = read_pinned_task_id(&connection)? {
            if let Ok(task) = read_task(&connection, task_id) {
                if task.completed_at.is_none() {
                    return Ok(Some(HudTask { task, pinned: true }));
                }
            }
        }
        let next = read_tasks(
            &connection,
            &TaskQuery {
                list_id: None,
                completion: CompletionFilter::Open,
                priority: None,
            },
        )?
        .into_iter()
        .next();
        Ok(next.map(|task| HudTask {
            task,
            pinned: false,
        }))
    }

    pub fn create_list(&self, name: String) -> Result<TaskList, TaskError> {
        let name = validate_required_text(&name, MAX_LIST_NAME_CHARS)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        let name_key = normalize_list_name(&name);
        ensure_list_name_available(&transaction, &name_key, None)?;
        let now = utc_now();
        transaction
            .execute(
                "INSERT INTO task_lists (name, name_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params![name, name_key, now],
            )
            .map_err(|_| TaskError::storage())?;
        let id = transaction.last_insert_rowid();
        let list = read_list(&transaction, id)?;
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(list)
    }

    pub fn rename_list(&self, list_id: i64, name: String) -> Result<TaskList, TaskError> {
        validate_id(list_id)?;
        let name = validate_required_text(&name, MAX_LIST_NAME_CHARS)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        ensure_list_exists(&transaction, list_id)?;
        let name_key = normalize_list_name(&name);
        ensure_list_name_available(&transaction, &name_key, Some(list_id))?;
        let changed = transaction
            .execute(
                "UPDATE task_lists
                 SET name = ?1, name_key = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![name, name_key, utc_now(), list_id],
            )
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::not_found());
        }
        let list = read_list(&transaction, list_id)?;
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(list)
    }

    pub fn list_delete_preview(&self, list_id: i64) -> Result<ListDeletePreview, TaskError> {
        validate_id(list_id)?;
        let connection = self.connect()?;
        ensure_list_exists(&connection, list_id)?;
        Ok(ListDeletePreview {
            list_id,
            task_count: task_count(&connection, list_id)?,
        })
    }

    pub fn delete_list(&self, list_id: i64, expected_task_count: u64) -> Result<(), TaskError> {
        validate_id(list_id)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        ensure_list_exists(&transaction, list_id)?;
        if task_count(&transaction, list_id)? != expected_task_count {
            return Err(TaskError::count_changed());
        }
        let changed = transaction
            .execute("DELETE FROM task_lists WHERE id = ?1", [list_id])
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::not_found());
        }
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(())
    }

    pub fn create_task(&self, input: TaskInput) -> Result<TaskItem, TaskError> {
        let input = validate_task_input(input)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        ensure_list_exists(&transaction, input.list_id)?;
        let now = utc_now();
        let (repository_id, repository_full_name) = repository_columns(input.repository.as_ref())?;
        transaction
            .execute(
                "INSERT INTO tasks
                    (list_id, title, notes, task_kind, priority, due_date, repository_id,
                     repository_full_name, completed_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?9)",
                params![
                    input.list_id,
                    input.title,
                    input.notes,
                    input.kind.as_str(),
                    input.priority.as_str(),
                    input.due_date,
                    repository_id,
                    repository_full_name,
                    now,
                ],
            )
            .map_err(|_| TaskError::storage())?;
        let id = transaction.last_insert_rowid();
        let task = read_task(&transaction, id)?;
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(task)
    }

    pub fn update_task(&self, task_id: i64, input: TaskInput) -> Result<TaskItem, TaskError> {
        validate_id(task_id)?;
        let input = validate_task_input(input)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        ensure_task_exists(&transaction, task_id)?;
        ensure_list_exists(&transaction, input.list_id)?;
        let (repository_id, repository_full_name) = repository_columns(input.repository.as_ref())?;
        let changed = transaction
            .execute(
                "UPDATE tasks
                 SET list_id = ?1, title = ?2, notes = ?3, task_kind = ?4, priority = ?5,
                     due_date = ?6, repository_id = ?7, repository_full_name = ?8,
                     updated_at = ?9
                 WHERE id = ?10",
                params![
                    input.list_id,
                    input.title,
                    input.notes,
                    input.kind.as_str(),
                    input.priority.as_str(),
                    input.due_date,
                    repository_id,
                    repository_full_name,
                    utc_now(),
                    task_id,
                ],
            )
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::not_found());
        }
        let task = read_task(&transaction, task_id)?;
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(task)
    }

    pub fn set_completed(&self, task_id: i64, completed: bool) -> Result<TaskItem, TaskError> {
        validate_id(task_id)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        ensure_task_exists(&transaction, task_id)?;
        let completed_at = completed.then(utc_now);
        transaction
            .execute(
                "UPDATE tasks SET completed_at = ?1, updated_at = ?2 WHERE id = ?3",
                params![completed_at, utc_now(), task_id],
            )
            .map_err(|_| TaskError::storage())?;
        if completed {
            transaction
                .execute(
                    "UPDATE workspace_task_state SET pinned_task_id = NULL
                     WHERE id = 1 AND pinned_task_id = ?1",
                    [task_id],
                )
                .map_err(|_| TaskError::storage())?;
        }
        let task = read_task(&transaction, task_id)?;
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(task)
    }

    pub fn delete_task(&self, task_id: i64) -> Result<(), TaskError> {
        validate_id(task_id)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        let changed = transaction
            .execute("DELETE FROM tasks WHERE id = ?1", [task_id])
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::not_found());
        }
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(())
    }

    pub fn set_pinned(&self, task_id: Option<i64>) -> Result<Option<i64>, TaskError> {
        validate_optional_id(task_id)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        if let Some(task_id) = task_id {
            let completed: Option<String> = transaction
                .query_row(
                    "SELECT completed_at FROM tasks WHERE id = ?1",
                    [task_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|_| TaskError::storage())?
                .ok_or_else(TaskError::not_found)?;
            if completed.is_some() {
                return Err(TaskError::invalid_input());
            }
        }
        let changed = transaction
            .execute(
                "UPDATE workspace_task_state SET pinned_task_id = ?1 WHERE id = 1",
                [task_id],
            )
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::storage());
        }
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(task_id)
    }

    pub fn pinned_task(&self) -> Result<Option<TaskItem>, TaskError> {
        Ok(self.task_note_snapshot()?.task)
    }

    pub fn complete_pinned(&self) -> Result<TaskItem, TaskError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| TaskError::storage())?;
        let task_id = read_pinned_task_id(&transaction)?.ok_or_else(TaskError::not_found)?;
        let now = utc_now();
        let changed = transaction
            .execute(
                "UPDATE tasks SET completed_at = ?1, updated_at = ?1 WHERE id = ?2",
                params![now, task_id],
            )
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::not_found());
        }
        transaction
            .execute(
                "UPDATE workspace_task_state SET pinned_task_id = NULL WHERE id = 1",
                [],
            )
            .map_err(|_| TaskError::storage())?;
        let task = read_task(&transaction, task_id)?;
        transaction.commit().map_err(|_| TaskError::storage())?;
        Ok(task)
    }

    pub(crate) fn task_note_snapshot(&self) -> Result<TaskNoteSnapshot, TaskError> {
        let connection = self.connect()?;
        let (task_id, x, y): (Option<i64>, Option<i64>, Option<i64>) = connection
            .query_row(
                "SELECT pinned_task_id, sticky_x, sticky_y
                 FROM workspace_task_state WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| TaskError::storage())?;
        let task = task_id.map(|id| read_task(&connection, id)).transpose()?;
        let position = match (x, y) {
            (None, None) => None,
            (Some(x), Some(y)) => Some((
                i32::try_from(x).map_err(|_| TaskError::storage())?,
                i32::try_from(y).map_err(|_| TaskError::storage())?,
            )),
            _ => return Err(TaskError::storage()),
        };
        Ok(TaskNoteSnapshot { task, position })
    }

    pub(crate) fn save_sticky_position(&self, x: i32, y: i32) -> Result<(), TaskError> {
        let changed = self
            .connect()?
            .execute(
                "UPDATE workspace_task_state SET sticky_x = ?1, sticky_y = ?2 WHERE id = 1",
                params![x, y],
            )
            .map_err(|_| TaskError::storage())?;
        if changed != 1 {
            return Err(TaskError::storage());
        }
        Ok(())
    }

    fn connect(&self) -> Result<Connection, TaskError> {
        crate::storage::connect(&self.database_path).map_err(|_| TaskError::storage())
    }
}

fn validate_task_input(input: TaskInput) -> Result<TaskInput, TaskError> {
    validate_id(input.list_id)?;
    let title = validate_required_text(&input.title, MAX_TASK_TITLE_CHARS)?;
    let notes = validate_notes(input.notes)?;
    let due_date = input.due_date.map(validate_due_date).transpose()?;
    let repository = input.repository.map(validate_repository).transpose()?;
    if input.kind == TaskKind::Personal && repository.is_some() {
        return Err(TaskError::invalid_input());
    }
    Ok(TaskInput {
        list_id: input.list_id,
        title,
        notes,
        kind: input.kind,
        priority: input.priority,
        due_date,
        repository,
    })
}

fn normalize_list_name(value: &str) -> String {
    value.to_lowercase()
}

fn validate_required_text(value: &str, max_chars: usize) -> Result<String, TaskError> {
    if value.chars().count() > max_chars || value.chars().any(char::is_control) {
        return Err(TaskError::invalid_input());
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(TaskError::invalid_input());
    }
    Ok(value.to_string())
}

fn validate_notes(notes: Option<String>) -> Result<Option<String>, TaskError> {
    let Some(notes) = notes else {
        return Ok(None);
    };
    let normalized = notes.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.chars().count() > MAX_TASK_NOTES_CHARS
        || normalized
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(TaskError::invalid_input());
    }
    if normalized.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized))
    }
}

fn validate_due_date(value: String) -> Result<String, TaskError> {
    if value.len() != 10
        || NaiveDate::parse_from_str(&value, "%Y-%m-%d")
            .ok()
            .is_none_or(|date| date.format("%Y-%m-%d").to_string() != value)
    {
        return Err(TaskError::invalid_input());
    }
    Ok(value)
}

fn validate_repository(input: TaskRepositoryInput) -> Result<TaskRepositoryInput, TaskError> {
    if input.repository_id == 0 || i64::try_from(input.repository_id).is_err() {
        return Err(TaskError::invalid_input());
    }
    if input.full_name != input.full_name.trim() || input.full_name.contains('\\') {
        return Err(TaskError::invalid_input());
    }
    let Some((owner, repository)) = input.full_name.split_once('/') else {
        return Err(TaskError::invalid_input());
    };
    let owner_is_valid = owner.len() <= 100
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if repository.contains('/')
        || !owner_is_valid
        || crate::github::models::validate_new_repository_name(repository).is_err()
    {
        return Err(TaskError::invalid_input());
    }
    Ok(input)
}

fn repository_columns(
    repository: Option<&TaskRepositoryInput>,
) -> Result<(Option<i64>, Option<&str>), TaskError> {
    repository
        .map(|repository| {
            Ok((
                Some(
                    i64::try_from(repository.repository_id)
                        .map_err(|_| TaskError::invalid_input())?,
                ),
                Some(repository.full_name.as_str()),
            ))
        })
        .unwrap_or(Ok((None, None)))
}

fn validate_id(id: i64) -> Result<(), TaskError> {
    if id <= 0 {
        return Err(TaskError::invalid_input());
    }
    Ok(())
}

fn validate_optional_id(id: Option<i64>) -> Result<(), TaskError> {
    if let Some(id) = id {
        validate_id(id)?;
    }
    Ok(())
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_timestamp(value: String) -> Result<DateTime<Utc>, TaskError> {
    let parsed = DateTime::parse_from_rfc3339(&value).map_err(|_| TaskError::storage())?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err(TaskError::storage());
    }
    Ok(parsed.with_timezone(&Utc))
}

fn ensure_default_list(connection: &Connection) -> Result<(), TaskError> {
    let has_list = connection
        .query_row("SELECT EXISTS(SELECT 1 FROM task_lists)", [], |row| {
            row.get::<_, bool>(0)
        })
        .map_err(|_| TaskError::storage())?;
    if has_list {
        return Ok(());
    }
    let now = utc_now();
    connection
        .execute(
            "INSERT INTO task_lists (name, name_key, created_at, updated_at)
             VALUES ('My tasks', 'my tasks', ?1, ?1)",
            [now],
        )
        .map_err(|_| TaskError::storage())?;
    Ok(())
}

fn ensure_list_exists(connection: &Connection, list_id: i64) -> Result<(), TaskError> {
    if connection
        .query_row("SELECT 1 FROM task_lists WHERE id = ?1", [list_id], |_| {
            Ok(())
        })
        .optional()
        .map_err(|_| TaskError::storage())?
        .is_none()
    {
        return Err(TaskError::not_found());
    }
    Ok(())
}

fn ensure_task_exists(connection: &Connection, task_id: i64) -> Result<(), TaskError> {
    if connection
        .query_row("SELECT 1 FROM tasks WHERE id = ?1", [task_id], |_| Ok(()))
        .optional()
        .map_err(|_| TaskError::storage())?
        .is_none()
    {
        return Err(TaskError::not_found());
    }
    Ok(())
}

fn ensure_list_name_available(
    connection: &Connection,
    name_key: &str,
    excluding: Option<i64>,
) -> Result<(), TaskError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM task_lists
                WHERE name_key = ?1 AND (?2 IS NULL OR id <> ?2)
             )",
            params![name_key, excluding],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| TaskError::storage())?;
    if exists {
        return Err(TaskError::conflict());
    }
    Ok(())
}

fn task_count(connection: &Connection, list_id: i64) -> Result<u64, TaskError> {
    let count = connection
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id = ?1",
            [list_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| TaskError::storage())?;
    u64::try_from(count).map_err(|_| TaskError::storage())
}

fn read_pinned_task_id(connection: &Connection) -> Result<Option<i64>, TaskError> {
    connection
        .query_row(
            "SELECT pinned_task_id FROM workspace_task_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| TaskError::storage())
}

fn read_lists(connection: &Connection) -> Result<Vec<TaskList>, TaskError> {
    let mut statement = connection
        .prepare(
            "SELECT task_lists.id, task_lists.name, COUNT(tasks.id),
                    task_lists.created_at, task_lists.updated_at
             FROM task_lists
             LEFT JOIN tasks ON tasks.list_id = task_lists.id
             GROUP BY task_lists.id
             ORDER BY task_lists.created_at, task_lists.id",
        )
        .map_err(|_| TaskError::storage())?;
    let rows = statement
        .query_map([], map_list_row)
        .map_err(|_| TaskError::storage())?;
    rows.map(|row| {
        row.map_err(|_| TaskError::storage())
            .and_then(parse_list_row)
    })
    .collect()
}

fn read_list(connection: &Connection, list_id: i64) -> Result<TaskList, TaskError> {
    connection
        .query_row(
            "SELECT task_lists.id, task_lists.name, COUNT(tasks.id),
                    task_lists.created_at, task_lists.updated_at
             FROM task_lists
             LEFT JOIN tasks ON tasks.list_id = task_lists.id
             WHERE task_lists.id = ?1
             GROUP BY task_lists.id",
            [list_id],
            map_list_row,
        )
        .optional()
        .map_err(|_| TaskError::storage())?
        .ok_or_else(TaskError::not_found)
        .and_then(parse_list_row)
}

type ListRow = (i64, String, i64, String, String);

fn map_list_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ListRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

fn parse_list_row(row: ListRow) -> Result<TaskList, TaskError> {
    Ok(TaskList {
        id: row.0,
        name: row.1,
        task_count: u64::try_from(row.2).map_err(|_| TaskError::storage())?,
        created_at: parse_timestamp(row.3)?,
        updated_at: parse_timestamp(row.4)?,
    })
}

fn read_tasks(connection: &Connection, query: &TaskQuery) -> Result<Vec<TaskItem>, TaskError> {
    let completion = match query.completion {
        CompletionFilter::All => 0,
        CompletionFilter::Open => 1,
        CompletionFilter::Completed => 2,
    };
    let priority = query.priority.map(TaskPriority::as_str);
    let mut statement = connection
        .prepare(
            "SELECT id, list_id, title, notes, task_kind, priority, due_date, repository_id,
                    repository_full_name, completed_at, created_at, updated_at
             FROM tasks
             WHERE (?1 IS NULL OR list_id = ?1)
               AND (?2 = 0 OR (?2 = 1 AND completed_at IS NULL)
                           OR (?2 = 2 AND completed_at IS NOT NULL))
               AND (?3 IS NULL OR priority = ?3)
             ORDER BY (completed_at IS NOT NULL), (due_date IS NULL), due_date,
                      created_at, id",
        )
        .map_err(|_| TaskError::storage())?;
    let rows = statement
        .query_map(params![query.list_id, completion, priority], map_task_row)
        .map_err(|_| TaskError::storage())?;
    rows.map(|row| {
        row.map_err(|_| TaskError::storage())
            .and_then(parse_task_row)
    })
    .collect()
}

fn read_task(connection: &Connection, task_id: i64) -> Result<TaskItem, TaskError> {
    connection
        .query_row(
            "SELECT id, list_id, title, notes, task_kind, priority, due_date, repository_id,
                    repository_full_name, completed_at, created_at, updated_at
             FROM tasks WHERE id = ?1",
            [task_id],
            map_task_row,
        )
        .optional()
        .map_err(|_| TaskError::storage())?
        .ok_or_else(TaskError::not_found)
        .and_then(parse_task_row)
}

type TaskRow = (
    i64,
    i64,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    String,
    String,
);

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
    ))
}

fn parse_task_row(row: TaskRow) -> Result<TaskItem, TaskError> {
    let due_date = row
        .6
        .map(validate_due_date)
        .transpose()
        .map_err(|_| TaskError::storage())?;
    let repository = match (row.7, row.8) {
        (None, None) => None,
        (Some(id), Some(full_name)) if id > 0 => {
            let repository_id = u64::try_from(id).map_err(|_| TaskError::storage())?;
            validate_repository(TaskRepositoryInput {
                repository_id,
                full_name: full_name.clone(),
            })
            .map_err(|_| TaskError::storage())?;
            Some(TaskRepositoryLink {
                repository_id,
                html_url: format!("https://github.com/{full_name}"),
                full_name,
            })
        }
        _ => return Err(TaskError::storage()),
    };
    Ok(TaskItem {
        id: row.0,
        list_id: row.1,
        title: row.2,
        notes: row.3,
        kind: TaskKind::parse(&row.4)?,
        priority: TaskPriority::parse(&row.5)?,
        due_date,
        repository,
        completed_at: row.9.map(parse_timestamp).transpose()?,
        created_at: parse_timestamp(row.10)?,
        updated_at: parse_timestamp(row.11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (tempfile::TempDir, TaskService) {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("ellie.sqlite3");
        crate::storage::initialize(&path).expect("initialize database");
        let service = TaskService::new(path);
        (temp, service)
    }

    fn task_input(list_id: i64) -> TaskInput {
        TaskInput {
            list_id,
            title: "Review workspace".to_string(),
            notes: Some("Check the dashboard\r\nthen tests".to_string()),
            kind: TaskKind::Work,
            priority: TaskPriority::High,
            due_date: Some("2026-10-01".to_string()),
            repository: Some(TaskRepositoryInput {
                repository_id: 42,
                full_name: "octo-cat/ellie".to_string(),
            }),
        }
    }

    #[test]
    fn hud_task_prefers_the_pinned_open_task_and_falls_back_to_the_next_one() {
        let (_temp, service) = service();
        let list = service.bootstrap().expect("bootstrap").lists[0].id;
        let mut earlier = task_input(list);
        earlier.title = "Earlier due task".to_string();
        earlier.due_date = Some("2026-09-27".to_string());
        let mut later = task_input(list);
        later.title = "Later due task".to_string();
        later.due_date = Some("2026-12-01".to_string());

        let earlier = service.create_task(earlier).expect("earlier");
        let later = service.create_task(later).expect("later");

        // Nothing pinned: the next open task by due date, flagged as a fallback.
        let next = service.hud_task().expect("hud").expect("a task");
        assert_eq!(next.task.title, "Earlier due task");
        assert!(!next.pinned);

        // Pinning wins over the due-date order.
        service.set_pinned(Some(later.id)).expect("pin");
        let pinned = service.hud_task().expect("hud").expect("a task");
        assert_eq!(pinned.task.title, "Later due task");
        assert!(pinned.pinned);

        // Completing the pinned task falls back again instead of showing it.
        service
            .set_completed(later.id, true)
            .expect("complete pinned");
        let fallback = service.hud_task().expect("hud").expect("a task");
        assert_eq!(fallback.task.title, "Earlier due task");
        assert!(!fallback.pinned);

        // No open tasks at all means no HUD task, never a completed one.
        service
            .set_completed(earlier.id, true)
            .expect("complete earlier");
        assert!(service.hud_task().expect("hud").is_none());
    }

    #[test]
    fn first_bootstrap_creates_the_single_default_list() {
        let (_temp, service) = service();
        let first = service.bootstrap().expect("bootstrap");
        assert_eq!(first.lists.len(), 1);
        assert_eq!(first.lists[0].name, "My tasks");
        assert!(first.tasks.is_empty());

        let reopened = TaskService::new(service.database_path.clone())
            .bootstrap()
            .expect("restart");
        assert_eq!(reopened.lists.len(), 1);
        assert_eq!(reopened.lists[0].id, first.lists[0].id);
    }

    #[test]
    fn offline_crud_filters_and_repository_snapshot_survive_reopen() {
        let (_temp, service) = service();
        let list = service.create_list("Work".into()).expect("create list");
        let task = service
            .create_task(task_input(list.id))
            .expect("create task");
        assert_eq!(
            task.notes.as_deref(),
            Some("Check the dashboard\nthen tests")
        );
        assert_eq!(task.kind, TaskKind::Work);
        assert_eq!(
            task.repository.as_ref().map(|link| link.html_url.as_str()),
            Some("https://github.com/octo-cat/ellie")
        );
        assert_eq!(
            service
                .list_tasks(TaskQuery {
                    list_id: Some(list.id),
                    completion: CompletionFilter::Open,
                    priority: Some(TaskPriority::High),
                })
                .expect("query"),
            vec![task.clone()]
        );
        service.set_pinned(Some(task.id)).expect("pin");
        service
            .save_sticky_position(120, -40)
            .expect("save sticky position");
        let reopened = TaskService::new(service.database_path.clone());
        let bootstrap = reopened.bootstrap().expect("bootstrap");
        assert_eq!(bootstrap.pinned_task_id, Some(task.id));
        assert_eq!(bootstrap.tasks[0].repository, task.repository);
        let note = reopened.task_note_snapshot().expect("sticky note");
        assert_eq!(note.task.as_ref().map(|item| item.id), Some(task.id));
        assert_eq!(note.position, Some((120, -40)));
        let completed = reopened.complete_pinned().expect("complete pinned");
        assert!(completed.completed_at.is_some());
        assert_eq!(
            reopened
                .bootstrap()
                .expect("after completion")
                .pinned_task_id,
            None
        );
        let reopened = reopened.set_completed(task.id, false).expect("reopen");
        assert!(reopened.completed_at.is_none());
    }

    #[test]
    fn rename_update_and_completion_filters_are_durable() {
        let (_temp, service) = service();
        let list = service.create_list("Inbox".into()).expect("list");
        let renamed = service
            .rename_list(list.id, "Focused work".into())
            .expect("rename");
        assert_eq!(renamed.name, "Focused work");
        let task = service.create_task(task_input(list.id)).expect("task");
        let updated = service
            .update_task(
                task.id,
                TaskInput {
                    list_id: list.id,
                    title: "Ship workspace".to_string(),
                    notes: None,
                    kind: TaskKind::Personal,
                    priority: TaskPriority::Medium,
                    due_date: Some("2026-12-31".to_string()),
                    repository: None,
                },
            )
            .expect("update");
        assert_eq!(updated.title, "Ship workspace");
        assert_eq!(updated.priority, TaskPriority::Medium);
        assert_eq!(updated.repository, None);
        service.set_completed(task.id, true).expect("complete");
        assert!(service
            .list_tasks(TaskQuery {
                completion: CompletionFilter::Open,
                ..TaskQuery::default()
            })
            .expect("open tasks")
            .is_empty());
        assert_eq!(
            service
                .list_tasks(TaskQuery {
                    completion: CompletionFilter::Completed,
                    ..TaskQuery::default()
                })
                .expect("completed tasks")
                .len(),
            1
        );
        let reopened = TaskService::new(service.database_path.clone());
        assert_eq!(
            reopened.bootstrap().expect("restart").lists[0].name,
            "Focused work"
        );
    }

    #[test]
    fn validates_absent_empty_dates_names_and_repository_identity() {
        let (_temp, service) = service();
        for name in ["", "   ", &"x".repeat(MAX_LIST_NAME_CHARS + 1), "bad\nname"] {
            assert_eq!(
                service
                    .create_list(name.to_string())
                    .expect_err("invalid")
                    .category(),
                TaskErrorCategory::InvalidInput
            );
        }
        let list = service.create_list("Personal".into()).expect("list");
        assert_eq!(
            service
                .create_list("personal".into())
                .expect_err("duplicate")
                .category(),
            TaskErrorCategory::Conflict
        );
        let mut input = task_input(list.id);
        input.notes = Some(" \t\n".to_string());
        input.due_date = None;
        input.repository = None;
        assert_eq!(service.create_task(input).expect("normalized").notes, None);
        for date in ["2026-02-30", "01-02-2026", "2026-1-02", ""] {
            let mut input = task_input(list.id);
            input.due_date = Some(date.to_string());
            assert_eq!(
                service
                    .create_task(input)
                    .expect_err("invalid date")
                    .category(),
                TaskErrorCategory::InvalidInput
            );
        }
        let mut personal_repository = task_input(list.id);
        personal_repository.kind = TaskKind::Personal;
        assert_eq!(
            service
                .create_task(personal_repository)
                .expect_err("personal task with repository")
                .category(),
            TaskErrorCategory::InvalidInput
        );
        let mut invalid_repository = task_input(list.id);
        invalid_repository.repository = Some(TaskRepositoryInput {
            repository_id: 1,
            full_name: "https://evil.example/repo".to_string(),
        });
        assert_eq!(
            service
                .create_task(invalid_repository)
                .expect_err("arbitrary URL")
                .category(),
            TaskErrorCategory::InvalidInput
        );
    }

    #[test]
    fn guarded_list_delete_detects_changed_membership_and_clears_pin() {
        let (_temp, service) = service();
        let list = service.create_list("Work".into()).expect("list");
        let preview = service.list_delete_preview(list.id).expect("preview");
        assert_eq!(preview.task_count, 0);
        let task = service.create_task(task_input(list.id)).expect("task");
        assert_eq!(
            service
                .delete_list(list.id, preview.task_count)
                .expect_err("changed count")
                .category(),
            TaskErrorCategory::CountChanged
        );
        service.set_pinned(Some(task.id)).expect("pin");
        service.delete_list(list.id, 1).expect("delete list");
        let bootstrap = service.bootstrap().expect("bootstrap");
        assert!(bootstrap.tasks.is_empty());
        assert_eq!(bootstrap.pinned_task_id, None);
    }

    #[test]
    fn deleting_a_pinned_task_clears_the_pin_transactionally() {
        let (_temp, service) = service();
        let list = service.create_list("Work".into()).expect("list");
        let task = service.create_task(task_input(list.id)).expect("task");
        service.set_pinned(Some(task.id)).expect("pin");
        service.delete_task(task.id).expect("delete");
        assert_eq!(service.bootstrap().expect("bootstrap").pinned_task_id, None);
    }
}
