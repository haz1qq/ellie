use std::{collections::HashSet, path::Path, time::Duration};

use rusqlite::{params, Connection};

use crate::{error::AppError, settings::Settings};

const SCHEMA_VERSION: i64 = 17;

/// One migration per entry, in order. Index 0 is migration 0001.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_settings.sql"),
    include_str!("../migrations/0002_history.sql"),
    include_str!("../migrations/0003_subscription.sql"),
    include_str!("../migrations/0004_balance_currency.sql"),
    include_str!("../migrations/0005_spend_estimate.sql"),
    include_str!("../migrations/0006_model.sql"),
    include_str!("../migrations/0007_provider_visibility.sql"),
    include_str!("../migrations/0008_notifications.sql"),
    include_str!("../migrations/0009_notification_thresholds.sql"),
    include_str!("../migrations/0010_mini_floating_bar.sql"),
    include_str!("../migrations/0011_local_api.sql"),
    include_str!("../migrations/0012_github_connection.sql"),
    include_str!("../migrations/0013_workspace_tasks.sql"),
    include_str!("../migrations/0014_github_repository_creation.sql"),
    include_str!("../migrations/0015_workspace_task_list_repair.sql"),
    include_str!("../migrations/0016_workspace_task_kind.sql"),
    include_str!("../migrations/0017_workspace_task_sticky_note.sql"),
];

pub(crate) fn connect(path: &Path) -> Result<Connection, AppError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(3))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    Ok(connection)
}

/// Runs on a blocking worker, before the window is interactive.
pub fn initialize(path: &Path) -> Result<Settings, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AppError::Storage)?;
    }
    let mut connection = connect(path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(AppError::NewerDatabase);
    }
    if version < 0 {
        return Err(AppError::Storage);
    }
    if version < SCHEMA_VERSION {
        let mut current = version;
        for migration in &MIGRATIONS[current as usize..] {
            // A development build applied schema 13 before task_lists.name_key
            // was finalized. Repair those databases before schema 15 creates
            // the unique index; fresh databases already have the column.
            if current == 14 {
                repair_legacy_workspace_task_lists(&transaction)?;
            }
            transaction.execute_batch(migration)?;
            current += 1;
            tracing::info!(event = "schema_migrated", to = current);
        }
        transaction.pragma_update(None, "user_version", current)?;
    }
    transaction.commit()?;
    read_settings_from(&connection)
}

fn repair_legacy_workspace_task_lists(connection: &Connection) -> Result<(), AppError> {
    let has_name_key = {
        let mut statement = connection.prepare("PRAGMA table_info(task_lists)")?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for column in columns {
            if column? == "name_key" {
                found = true;
                break;
            }
        }
        found
    };
    if has_name_key {
        return Ok(());
    }

    connection.execute("ALTER TABLE task_lists ADD COLUMN name_key TEXT", [])?;
    let lists = {
        let mut statement = connection.prepare("SELECT id, name FROM task_lists ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut used = HashSet::new();
    for (id, name) in &lists {
        let base = {
            let normalized = name.trim().to_lowercase();
            if normalized.is_empty() {
                format!("list-{id}")
            } else {
                normalized
            }
        };
        let mut attempt = 0_u64;
        let name_key = loop {
            let candidate = match attempt {
                0 => base.clone(),
                1 => format!("{base}#{id}"),
                _ => format!("{base}#{id}-{attempt}"),
            };
            if used.insert(candidate.clone()) {
                break candidate;
            }
            attempt = attempt.saturating_add(1);
        };
        connection.execute(
            "UPDATE task_lists SET name_key = ?1 WHERE id = ?2",
            params![name_key, id],
        )?;
    }

    tracing::info!(
        event = "workspace_task_list_schema_repaired",
        list_count = lists.len()
    );
    Ok(())
}

pub fn read_settings(path: &Path) -> Result<Settings, AppError> {
    read_settings_from(&connect(path)?)
}

fn read_settings_from(connection: &Connection) -> Result<Settings, AppError> {
    let (mut settings, thresholds, hidden): (Settings, String, String) = connection.query_row(
        "SELECT close_to_tray, show_mascot, friendly_messages, notifications_enabled,
                notification_thresholds, hidden_provider_ids, mini_bar_enabled,
                mini_bar_opacity, mini_bar_x, mini_bar_y
         FROM application_settings WHERE id = 1",
        [],
        |row| {
            Ok((
                Settings {
                    close_to_tray: row.get(0)?,
                    show_mascot: row.get(1)?,
                    friendly_messages: row.get(2)?,
                    notifications_enabled: row.get(3)?,
                    notification_thresholds: crate::settings::DEFAULT_NOTIFICATION_THRESHOLDS,
                    hidden_provider_ids: vec![],
                    mini_bar_enabled: row.get(6)?,
                    mini_bar_opacity: row.get(7)?,
                    mini_bar_x: row.get(8)?,
                    mini_bar_y: row.get(9)?,
                },
                row.get(4)?,
                row.get(5)?,
            ))
        },
    )?;
    settings.notification_thresholds =
        serde_json::from_str(&thresholds).map_err(|_| AppError::Storage)?;
    settings.hidden_provider_ids = serde_json::from_str(&hidden).map_err(|_| AppError::Storage)?;
    settings.validate()?;
    Ok(settings)
}

#[cfg(test)]
pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), AppError> {
    save_settings_to(&connect(path)?, settings)
}

fn save_settings_to(connection: &Connection, settings: &Settings) -> Result<(), AppError> {
    settings.validate()?;
    let thresholds =
        serde_json::to_string(&settings.notification_thresholds).map_err(|_| AppError::Storage)?;
    let hidden =
        serde_json::to_string(&settings.hidden_provider_ids).map_err(|_| AppError::Storage)?;
    let changed = connection.execute(
        "UPDATE application_settings SET close_to_tray = ?1, show_mascot = ?2, friendly_messages = ?3,
         notifications_enabled = ?4, notification_thresholds = ?5, hidden_provider_ids = ?6,
         mini_bar_enabled = ?7, mini_bar_opacity = ?8, mini_bar_x = ?9, mini_bar_y = ?10,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = 1",
        params![
            settings.close_to_tray,
            settings.show_mascot,
            settings.friendly_messages,
            settings.notifications_enabled,
            thresholds,
            hidden,
            settings.mini_bar_enabled,
            settings.mini_bar_opacity,
            settings.mini_bar_x,
            settings.mini_bar_y,
        ],
    )?;
    if changed != 1 {
        return Err(AppError::Storage);
    }
    Ok(())
}

/// Saves user-editable preferences while retaining the latest Rust-owned
/// window coordinates, which may have changed after the form was opened.
pub fn save_settings_preserving_position(
    path: &Path,
    requested: &Settings,
) -> Result<Settings, AppError> {
    requested.validate()?;
    let mut connection = connect(path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = read_settings_from(&transaction)?;
    let mut merged = requested.clone();
    merged.mini_bar_x = current.mini_bar_x;
    merged.mini_bar_y = current.mini_bar_y;
    save_settings_to(&transaction, &merged)?;
    transaction.commit()?;
    Ok(merged)
}

pub fn save_mini_bar_position(path: &Path, x: i32, y: i32) -> Result<(), AppError> {
    let connection = connect(path)?;
    let changed = connection.execute(
        "UPDATE application_settings SET mini_bar_x = ?1, mini_bar_y = ?2,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = 1",
        params![x, y],
    )?;
    if changed != 1 {
        return Err(AppError::Storage);
    }
    Ok(())
}

// Kept outside the generic Settings DTO: only the dedicated auth lifecycle may write it.
pub fn read_local_api_enabled(path: &Path) -> Result<bool, AppError> {
    Ok(connect(path)?.query_row(
        "SELECT local_api_enabled FROM application_settings WHERE id = 1",
        [],
        |row| row.get(0),
    )?)
}

pub fn save_local_api_enabled(path: &Path, enabled: bool) -> Result<(), AppError> {
    let changed = connect(path)?.execute(
        "UPDATE application_settings SET local_api_enabled = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = 1", [enabled],
    )?;
    if changed != 1 {
        return Err(AppError::Storage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_api_upgrade_is_opt_in_and_generic_settings_cannot_change_it() {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("ellie.sqlite3");
        let connection = connect(&path).expect("connect");
        for migration in &MIGRATIONS[..10] {
            connection.execute_batch(migration).expect("old migration");
        }
        connection
            .pragma_update(None, "user_version", 10)
            .expect("old version");
        let settings = initialize(&path).expect("upgrade");
        assert!(!read_local_api_enabled(&path).expect("default off"));
        save_local_api_enabled(&path, true).expect("enable");
        save_settings_preserving_position(&path, &settings).expect("stale generic save");
        initialize(&path).expect("restart");
        assert!(read_local_api_enabled(&path).expect("preserved enablement"));
        let mut json = serde_json::to_value(settings).expect("settings");
        json["localApiEnabled"] = serde_json::json!(false);
        assert!(serde_json::from_value::<Settings>(json).is_err());
    }

    #[test]
    fn migrates_once_and_preserves_preferences_after_restart(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let initial = initialize(&path)?;
        assert!(
            initial.close_to_tray
                && initial.show_mascot
                && initial.friendly_messages
                && initial.notifications_enabled
                && initial.notification_thresholds
                    == crate::settings::DEFAULT_NOTIFICATION_THRESHOLDS
        );
        let changed = Settings {
            close_to_tray: false,
            show_mascot: false,
            friendly_messages: true,
            notifications_enabled: false,
            notification_thresholds: [60.0, 80.0, 95.0],
            hidden_provider_ids: vec!["ellie-demo".into()],
            mini_bar_enabled: true,
            mini_bar_opacity: 0.75,
            mini_bar_x: Some(120),
            mini_bar_y: Some(-40),
        };
        save_settings(&path, &changed)?;
        assert_eq!(initialize(&path)?, changed);
        let connection = connect(&path)?;
        let timestamp: String =
            connection.query_row("SELECT updated_at FROM application_settings", [], |row| {
                row.get(0)
            })?;
        assert!(timestamp.ends_with('Z') && timestamp.contains('T'));
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM application_settings", [], |row| row
                .get::<_, i64>(
                0
            ))?,
            1
        );
        for table in [
            "providers",
            "accounts",
            "usage_snapshots",
            "usage_windows",
            "token_usage",
            "notification_rules",
            "notification_state",
        ] {
            assert!(connection
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |_| Ok(()),
                )
                .is_ok());
        }
        Ok(())
    }

    #[test]
    fn migrating_from_schema_1_preserves_settings_and_history_tables(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        // Simulate a milestone 1 database: settings at schema version 1.
        let connection = connect(&path)?;
        connection.execute_batch(include_str!("../migrations/0001_settings.sql"))?;
        connection.pragma_update(None, "user_version", 1)?;
        connection.execute(
            "UPDATE application_settings SET show_mascot = 0 WHERE id = 1",
            [],
        )?;
        drop(connection);

        let settings = initialize(&path)?;
        assert!(!settings.show_mascot);
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            SCHEMA_VERSION
        );
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM usage_snapshots", [], |row| row
                .get::<_, i64>(0))?,
            0
        );
        Ok(())
    }

    #[tokio::test]
    async fn schema_6_migration_defaults_to_visible_and_visibility_survives_restart(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let connection = connect(&path)?;
        for migration in &MIGRATIONS[..6] {
            connection.execute_batch(migration)?;
        }
        connection.pragma_update(None, "user_version", 6)?;
        connection.execute(
            "UPDATE application_settings SET show_mascot = 0 WHERE id = 1",
            [],
        )?;
        drop(connection);
        let mut settings = initialize(&path)?;
        assert!(settings.hidden_provider_ids.is_empty());
        assert!(!settings.show_mascot);
        assert!(settings.notifications_enabled);
        assert_eq!(
            settings.notification_thresholds,
            crate::settings::DEFAULT_NOTIFICATION_THRESHOLDS
        );
        use crate::providers::UsageProvider;
        let snapshot = crate::providers::MockProvider.fetch_usage().await?;
        crate::history::insert_snapshot(&path, &snapshot)?;
        settings.hidden_provider_ids = vec!["ellie-demo".into(), "deepseek".into()];
        save_settings(&path, &settings)?;
        assert_eq!(initialize(&path)?, settings);
        settings.hidden_provider_ids.clear();
        save_settings(&path, &settings)?;
        assert_eq!(initialize(&path)?, settings);
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            SCHEMA_VERSION
        );
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM usage_snapshots", [], |row| row
                .get::<_, i64>(0))?,
            1
        );
        Ok(())
    }

    #[test]
    fn schema_9_migration_adds_mini_bar_defaults_and_persists_position(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let connection = connect(&path)?;
        for migration in &MIGRATIONS[..9] {
            connection.execute_batch(migration)?;
        }
        connection.pragma_update(None, "user_version", 9)?;
        drop(connection);

        let initial = initialize(&path)?;
        assert!(!initial.mini_bar_enabled);
        assert_eq!(
            initial.mini_bar_opacity,
            crate::settings::DEFAULT_MINI_BAR_OPACITY
        );
        assert_eq!((initial.mini_bar_x, initial.mini_bar_y), (None, None));

        let changed = Settings {
            mini_bar_enabled: true,
            mini_bar_opacity: 0.55,
            ..initial.clone()
        };
        save_settings(&path, &changed)?;
        save_mini_bar_position(&path, -300, 220)?;
        let persisted = initialize(&path)?;
        assert!(persisted.mini_bar_enabled);
        assert_eq!(persisted.mini_bar_opacity, 0.55);
        assert_eq!(
            (persisted.mini_bar_x, persisted.mini_bar_y),
            (Some(-300), Some(220))
        );

        let stale_form = Settings {
            mini_bar_opacity: 0.8,
            mini_bar_x: None,
            mini_bar_y: None,
            ..persisted
        };
        let merged = save_settings_preserving_position(&path, &stale_form)?;
        assert_eq!(
            (merged.mini_bar_x, merged.mini_bar_y),
            (Some(-300), Some(220))
        );
        assert_eq!(read_settings(&path)?, merged);
        Ok(())
    }

    #[test]
    fn invalid_mini_bar_write_keeps_previous_settings() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let initial = initialize(&path)?;
        let invalid = Settings {
            mini_bar_opacity: 0.1,
            mini_bar_enabled: true,
            ..initial.clone()
        };
        assert!(save_settings(&path, &invalid).is_err());
        assert_eq!(read_settings(&path)?, initial);
        Ok(())
    }

    #[test]
    fn rejects_invalid_visibility_without_changing_saved_settings(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let initial = initialize(&path)?;
        for ids in [
            vec!["".into()],
            vec!["x".repeat(65)],
            vec!["a".into(); 65],
            vec!["deepseek".into(); 2],
            vec!["invalid provider".into()],
        ] {
            let invalid = Settings {
                hidden_provider_ids: ids,
                close_to_tray: !initial.close_to_tray,
                ..initial.clone()
            };
            assert!(save_settings(&path, &invalid).is_err());
            assert_eq!(read_settings(&path)?, initial);
        }
        Ok(())
    }

    #[test]
    fn schema_11_upgrade_preserves_existing_data_and_adds_github_connection(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let connection = connect(&path)?;
        for migration in &MIGRATIONS[..11] {
            connection.execute_batch(migration)?;
        }
        connection.pragma_update(None, "user_version", 11)?;
        connection.execute(
            "UPDATE application_settings
             SET close_to_tray = 0, show_mascot = 0
             WHERE id = 1",
            [],
        )?;
        connection.execute(
            "INSERT INTO providers (provider_key, display_name) VALUES (?1, ?2)",
            params!["preserved-provider", "Preserved Provider"],
        )?;
        drop(connection);

        let settings = initialize(&path)?;
        assert!(!settings.close_to_tray);
        assert!(!settings.show_mascot);
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            SCHEMA_VERSION
        );
        assert_eq!(
            connection.query_row(
                "SELECT display_name FROM providers WHERE provider_key = ?1",
                ["preserved-provider"],
                |row| row.get::<_, String>(0),
            )?,
            "Preserved Provider"
        );
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM github_connection", [], |row| {
                row.get::<_, i64>(0)
            })?,
            0
        );
        for table in [
            "application_settings",
            "providers",
            "accounts",
            "usage_snapshots",
            "usage_windows",
            "token_usage",
            "notification_rules",
            "notification_state",
            "github_connection",
        ] {
            assert!(connection
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |_| Ok(()),
                )
                .is_ok());
        }
        Ok(())
    }

    #[test]
    fn schema_12_upgrade_preserves_existing_data_and_adds_workspace_tables(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let connection = connect(&path)?;
        for migration in &MIGRATIONS[..12] {
            connection.execute_batch(migration)?;
        }
        connection.pragma_update(None, "user_version", 12)?;
        connection.execute(
            "UPDATE application_settings SET friendly_messages = 0 WHERE id = 1",
            [],
        )?;
        connection.execute(
            "INSERT INTO providers (provider_key, display_name) VALUES (?1, ?2)",
            params!["preserved-provider", "Preserved Provider"],
        )?;
        connection.execute(
            "INSERT INTO github_connection
                (id, client_id, account_id, account_login, updated_at)
             VALUES (1, ?1, 42, ?2, ?3)",
            params![
                "Iv1.sanitized-client",
                "octo-cat",
                "2026-01-02T03:04:05.000Z"
            ],
        )?;
        drop(connection);

        let settings = initialize(&path)?;
        assert!(!settings.friendly_messages);
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            SCHEMA_VERSION
        );
        assert_eq!(
            connection.query_row(
                "SELECT display_name FROM providers WHERE provider_key = ?1",
                ["preserved-provider"],
                |row| row.get::<_, String>(0),
            )?,
            "Preserved Provider"
        );
        assert_eq!(
            connection.query_row(
                "SELECT account_login FROM github_connection WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )?,
            "octo-cat"
        );
        for table in [
            "task_lists",
            "tasks",
            "workspace_task_state",
            "github_repository_creation_attempts",
        ] {
            assert!(connection
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |_| Ok(()),
                )
                .is_ok());
        }
        assert_eq!(
            connection.query_row(
                "SELECT COUNT(*) FROM workspace_task_state WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            1
        );
        Ok(())
    }

    #[test]
    fn schema_14_upgrade_repairs_legacy_task_lists_without_losing_data(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let connection = connect(&path)?;
        for migration in &MIGRATIONS[..12] {
            connection.execute_batch(migration)?;
        }
        let legacy_tasks = include_str!("../migrations/0013_workspace_tasks.sql")
            .lines()
            .filter(|line| !line.contains("name_key TEXT"))
            .collect::<Vec<_>>()
            .join("\n");
        connection.execute_batch(&legacy_tasks)?;
        connection.execute_batch(MIGRATIONS[13])?;
        connection.pragma_update(None, "user_version", 14)?;
        let now = "2026-01-02T03:04:05.000Z";
        connection.execute(
            "INSERT INTO task_lists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params!["Work", now],
        )?;
        let first_list = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO task_lists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params!["work", now],
        )?;
        let second_list = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO tasks
                (list_id, title, notes, priority, due_date, repository_id,
                 repository_full_name, completed_at, created_at, updated_at)
             VALUES (?1, 'Preserve me', NULL, 'none', NULL, NULL, NULL, NULL, ?2, ?2)",
            params![first_list, now],
        )?;
        connection.execute(
            "INSERT INTO tasks
                (list_id, title, notes, priority, due_date, repository_id,
                 repository_full_name, completed_at, created_at, updated_at)
             VALUES (?1, 'Linked work', NULL, 'high', NULL, 42,
                     'octo-cat/ellie', NULL, ?2, ?2)",
            params![second_list, now],
        )?;
        drop(connection);

        initialize(&path)?;
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            SCHEMA_VERSION
        );
        let columns = {
            let mut statement = connection.prepare("PRAGMA table_info(task_lists)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        assert!(columns.iter().any(|column| column == "name_key"));
        let keys = {
            let mut statement =
                connection.prepare("SELECT name_key FROM task_lists ORDER BY id")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| !key.is_empty()));
        assert_ne!(keys[0], keys[1]);
        assert!(connection
            .execute(
                "INSERT INTO task_lists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
                params!["Missing key", now],
            )
            .is_err());
        drop(connection);

        let bootstrap = crate::tasks::TaskService::new(path).bootstrap()?;
        assert_eq!(bootstrap.lists.len(), 2);
        assert_eq!(bootstrap.tasks.len(), 2);
        let personal = bootstrap
            .tasks
            .iter()
            .find(|task| task.title == "Preserve me")
            .expect("personal task");
        assert_eq!(personal.kind, crate::tasks::TaskKind::Personal);
        let work = bootstrap
            .tasks
            .iter()
            .find(|task| task.title == "Linked work")
            .expect("work task");
        assert_eq!(work.kind, crate::tasks::TaskKind::Work);
        Ok(())
    }

    #[test]
    fn failed_schema_12_migration_rolls_back_and_preserves_schema_11_data(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("invalid-upgrade.sqlite3");
        let connection = connect(&path)?;
        for migration in &MIGRATIONS[..11] {
            connection.execute_batch(migration)?;
        }
        connection.pragma_update(None, "user_version", 11)?;
        connection.execute(
            "UPDATE application_settings SET show_mascot = 0 WHERE id = 1",
            [],
        )?;
        connection.execute_batch(
            "CREATE TABLE github_connection (sentinel TEXT);
             INSERT INTO github_connection VALUES ('keep');",
        )?;
        drop(connection);

        assert!(initialize(&path).is_err());
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            11
        );
        assert_eq!(
            connection.query_row("SELECT show_mascot FROM application_settings", [], |row| {
                row.get::<_, i64>(0)
            })?,
            0
        );
        assert_eq!(
            connection.query_row("SELECT sentinel FROM github_connection", [], |row| {
                row.get::<_, String>(0)
            })?,
            "keep"
        );
        Ok(())
    }

    #[test]
    fn newer_schema_is_not_modified() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("future.sqlite3");
        let connection = connect(&path)?;
        connection.pragma_update(None, "user_version", 99)?;
        assert!(matches!(initialize(&path), Err(AppError::NewerDatabase)));
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            99
        );
        Ok(())
    }

    #[test]
    fn migrating_from_schema_2_adds_subscription_column() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        // Simulate a milestone 3 database: settings + history at schema 2.
        let connection = connect(&path)?;
        connection.execute_batch(include_str!("../migrations/0001_settings.sql"))?;
        connection.execute_batch(include_str!("../migrations/0002_history.sql"))?;
        connection.pragma_update(None, "user_version", 2)?;
        drop(connection);

        initialize(&path)?;
        let connection = connect(&path)?;
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            SCHEMA_VERSION
        );
        assert!(
            connection.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('usage_snapshots') \
                 WHERE name = 'has_subscription'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )?,
            "has_subscription column exists after migration"
        );
        Ok(())
    }

    #[test]
    fn failed_migration_rolls_back_version_and_data() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("invalid.sqlite3");
        let connection = connect(&path)?;
        connection.execute_batch("CREATE TABLE application_settings (sentinel TEXT); INSERT INTO application_settings VALUES ('keep');")?;
        assert!(initialize(&path).is_err());
        assert_eq!(
            connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
            0
        );
        assert_eq!(
            connection.query_row("SELECT sentinel FROM application_settings", [], |row| {
                row.get::<_, String>(0)
            })?,
            "keep"
        );
        Ok(())
    }
}
