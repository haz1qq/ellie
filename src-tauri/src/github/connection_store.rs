use std::{path::PathBuf, sync::Mutex};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, OptionalExtension};

use super::{auth, models, GitHubAccount, GitHubError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredConnection {
    pub client_id: String,
    pub account: Option<GitHubAccount>,
    pub updated_at: DateTime<Utc>,
}

pub trait GitHubConnectionStore: Send + Sync {
    fn load(&self) -> Result<Option<StoredConnection>, GitHubError>;
    fn save(&self, connection: &StoredConnection) -> Result<(), GitHubError>;
    fn clear_account(&self) -> Result<(), GitHubError>;
}

pub struct SqliteGitHubConnectionStore {
    database_path: PathBuf,
}

impl SqliteGitHubConnectionStore {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }
}

impl GitHubConnectionStore for SqliteGitHubConnectionStore {
    fn load(&self) -> Result<Option<StoredConnection>, GitHubError> {
        let connection = crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?;
        let row = connection
            .query_row(
                "SELECT client_id, account_id, account_login, updated_at
                 FROM github_connection WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| GitHubError::credential_store())?;
        row.map(parse_stored_row).transpose()
    }

    fn save(&self, connection: &StoredConnection) -> Result<(), GitHubError> {
        let connection = validate_stored_connection(connection.clone())?;
        let account_id = connection
            .account
            .as_ref()
            .map(|account| i64::try_from(account.id))
            .transpose()
            .map_err(|_| GitHubError::credential_store())?;
        let account_login = connection
            .account
            .as_ref()
            .map(|account| account.login.as_str());
        let updated_at = connection
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?
            .execute(
                "INSERT INTO github_connection
                    (id, client_id, account_id, account_login, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    client_id = excluded.client_id,
                    account_id = excluded.account_id,
                    account_login = excluded.account_login,
                    updated_at = excluded.updated_at",
                params![connection.client_id, account_id, account_login, updated_at],
            )
            .map_err(|_| GitHubError::credential_store())?;
        Ok(())
    }

    fn clear_account(&self) -> Result<(), GitHubError> {
        crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?
            .execute(
                "UPDATE github_connection
                 SET account_id = NULL,
                     account_login = NULL,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = 1",
                [],
            )
            .map_err(|_| GitHubError::credential_store())?;
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryGitHubConnectionStore {
    connection: Mutex<Option<StoredConnection>>,
}

impl GitHubConnectionStore for MemoryGitHubConnectionStore {
    fn load(&self) -> Result<Option<StoredConnection>, GitHubError> {
        self.connection
            .lock()
            .map(|connection| connection.clone())
            .map_err(|_| GitHubError::credential_store())
    }

    fn save(&self, connection: &StoredConnection) -> Result<(), GitHubError> {
        let connection = validate_stored_connection(connection.clone())?;
        *self
            .connection
            .lock()
            .map_err(|_| GitHubError::credential_store())? = Some(connection);
        Ok(())
    }

    fn clear_account(&self) -> Result<(), GitHubError> {
        if let Some(connection) = self
            .connection
            .lock()
            .map_err(|_| GitHubError::credential_store())?
            .as_mut()
        {
            connection.account = None;
            connection.updated_at = Utc::now();
        }
        Ok(())
    }
}

fn parse_stored_row(
    (client_id, account_id, account_login, updated_at): (
        String,
        Option<i64>,
        Option<String>,
        String,
    ),
) -> Result<StoredConnection, GitHubError> {
    auth::validate_client_id(&client_id).map_err(|_| GitHubError::credential_store())?;
    let account = match (account_id, account_login) {
        (None, None) => None,
        (Some(id), Some(login)) if id > 0 => {
            let account = GitHubAccount {
                id: u64::try_from(id).map_err(|_| GitHubError::credential_store())?,
                login,
            };
            let validated = models::validate_account(account.clone())
                .map_err(|_| GitHubError::credential_store())?;
            if validated != account {
                return Err(GitHubError::credential_store());
            }
            Some(account)
        }
        _ => return Err(GitHubError::credential_store()),
    };
    let parsed =
        DateTime::parse_from_rfc3339(&updated_at).map_err(|_| GitHubError::credential_store())?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err(GitHubError::credential_store());
    }
    Ok(StoredConnection {
        client_id,
        account,
        updated_at: parsed.with_timezone(&Utc),
    })
}

fn validate_stored_connection(
    connection: StoredConnection,
) -> Result<StoredConnection, GitHubError> {
    auth::validate_client_id(&connection.client_id)?;
    let account = connection
        .account
        .map(models::validate_account)
        .transpose()?;
    Ok(StoredConnection {
        account,
        ..connection
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_connection_round_trip_and_account_clear_keep_client_id() {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("ellie.sqlite3");
        crate::storage::initialize(&path).expect("initialize database");
        let store = SqliteGitHubConnectionStore::new(path);
        let saved = StoredConnection {
            client_id: "Iv1.sanitized-client".to_string(),
            account: Some(GitHubAccount {
                id: 42,
                login: "octo-cat".to_string(),
            }),
            updated_at: "2026-01-02T03:04:05Z".parse().expect("UTC timestamp"),
        };

        assert_eq!(store.load().expect("empty store"), None);
        store.save(&saved).expect("save connection");
        assert_eq!(store.load().expect("load connection"), Some(saved));
        store.clear_account().expect("clear account");
        let cleared = store
            .load()
            .expect("load cleared connection")
            .expect("connection remains");
        assert_eq!(cleared.client_id, "Iv1.sanitized-client");
        assert_eq!(cleared.account, None);
    }

    #[test]
    fn malformed_sqlite_rows_are_rejected() {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("ellie.sqlite3");
        crate::storage::initialize(&path).expect("initialize database");
        let database = crate::storage::connect(&path).expect("connect database");
        database
            .execute(
                "INSERT INTO github_connection
                 (id, client_id, account_id, account_login, updated_at)
                 VALUES (1, ?1, NULL, NULL, ?2)",
                params!["invalid/client", "2026-01-02T03:04:05Z"],
            )
            .expect("insert malformed row");
        let store = SqliteGitHubConnectionStore::new(path);
        assert_eq!(
            store.load().expect_err("reject malformed row").category(),
            super::super::GitHubErrorCategory::CredentialStore
        );
    }
}
