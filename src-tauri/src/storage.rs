use std::{path::Path, time::Duration};

use rusqlite::{params, Connection};

use crate::{error::AppError, settings::Settings};

const SCHEMA_VERSION: i64 = 2;

/// One migration per entry, in order. Index 0 is migration 0001, index 1 is 0002.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_settings.sql"),
    include_str!("../migrations/0002_history.sql"),
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
            transaction.execute_batch(migration)?;
            current += 1;
            tracing::info!(event = "schema_migrated", to = current);
        }
        transaction.pragma_update(None, "user_version", current)?;
    }
    transaction.commit()?;
    read_settings_from(&connection)
}

pub fn read_settings(path: &Path) -> Result<Settings, AppError> {
    read_settings_from(&connect(path)?)
}

fn read_settings_from(connection: &Connection) -> Result<Settings, AppError> {
    Ok(connection.query_row(
        "SELECT close_to_tray, show_mascot, friendly_messages FROM application_settings WHERE id = 1",
        [],
        |row| Ok(Settings { close_to_tray: row.get(0)?, show_mascot: row.get(1)?, friendly_messages: row.get(2)? }),
    )?)
}

pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), AppError> {
    let connection = connect(path)?;
    let changed = connection.execute(
        "UPDATE application_settings SET close_to_tray = ?1, show_mascot = ?2, friendly_messages = ?3,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = 1",
        params![settings.close_to_tray, settings.show_mascot, settings.friendly_messages],
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
    fn migrates_once_and_preserves_preferences_after_restart(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("ellie.sqlite3");
        let initial = initialize(&path)?;
        assert!(initial.close_to_tray && initial.show_mascot && initial.friendly_messages);
        let changed = Settings {
            close_to_tray: false,
            show_mascot: false,
            friendly_messages: true,
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
            2
        );
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM usage_snapshots", [], |row| row
                .get::<_, i64>(0))?,
            0
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
