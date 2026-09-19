use std::{path::PathBuf, sync::Mutex};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::params;

use super::{models, GitHubAccount, GitHubError};

const MAX_ATTEMPTS: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredCreationState {
    Dispatching,
    OutcomeUnknown,
}

impl StoredCreationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dispatching => "dispatching",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }

    fn parse(value: &str) -> Result<Self, GitHubError> {
        match value {
            "dispatching" => Ok(Self::Dispatching),
            "outcome_unknown" => Ok(Self::OutcomeUnknown),
            _ => Err(GitHubError::credential_store()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCreationAttempt {
    pub id: String,
    pub account: GitHubAccount,
    pub repository_name: String,
    pub state: StoredCreationState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub trait RepositoryCreationStore: Send + Sync {
    fn begin(&self, attempt: &StoredCreationAttempt) -> Result<(), GitHubError>;
    fn mark_outcome_unknown(&self, id: &str, updated_at: DateTime<Utc>) -> Result<(), GitHubError>;
    fn remove(&self, id: &str) -> Result<(), GitHubError>;
    fn list_unresolved(&self) -> Result<Vec<StoredCreationAttempt>, GitHubError>;
}

pub struct SqliteRepositoryCreationStore {
    database_path: PathBuf,
}

impl SqliteRepositoryCreationStore {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }
}

impl RepositoryCreationStore for SqliteRepositoryCreationStore {
    fn begin(&self, attempt: &StoredCreationAttempt) -> Result<(), GitHubError> {
        let attempt = validate_attempt(attempt.clone())?;
        let account_id =
            i64::try_from(attempt.account.id).map_err(|_| GitHubError::credential_store())?;
        crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?
            .execute(
                "INSERT INTO github_repository_creation_attempts
                    (id, account_id, account_login, repository_name, state,
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    attempt.id,
                    account_id,
                    attempt.account.login,
                    attempt.repository_name,
                    attempt.state.as_str(),
                    format_time(attempt.created_at),
                    format_time(attempt.updated_at),
                ],
            )
            .map_err(|_| GitHubError::credential_store())?;
        Ok(())
    }

    fn mark_outcome_unknown(&self, id: &str, updated_at: DateTime<Utc>) -> Result<(), GitHubError> {
        validate_attempt_id(id)?;
        let changed = crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?
            .execute(
                "UPDATE github_repository_creation_attempts
                 SET state = 'outcome_unknown', updated_at = ?1
                 WHERE id = ?2",
                params![format_time(updated_at), id],
            )
            .map_err(|_| GitHubError::credential_store())?;
        if changed != 1 {
            return Err(GitHubError::credential_store());
        }
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), GitHubError> {
        validate_attempt_id(id)?;
        crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?
            .execute(
                "DELETE FROM github_repository_creation_attempts WHERE id = ?1",
                [id],
            )
            .map_err(|_| GitHubError::credential_store())?;
        Ok(())
    }

    fn list_unresolved(&self) -> Result<Vec<StoredCreationAttempt>, GitHubError> {
        let connection = crate::storage::connect(&self.database_path)
            .map_err(|_| GitHubError::credential_store())?;
        let mut statement = connection
            .prepare(
                "SELECT id, account_id, account_login, repository_name, state,
                        created_at, updated_at
                 FROM github_repository_creation_attempts
                 ORDER BY updated_at DESC, id
                 LIMIT ?1",
            )
            .map_err(|_| GitHubError::credential_store())?;
        let rows = statement
            .query_map([MAX_ATTEMPTS as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|_| GitHubError::credential_store())?;
        rows.map(|row| {
            let row = row.map_err(|_| GitHubError::credential_store())?;
            parse_attempt(row)
        })
        .collect()
    }
}

#[derive(Default)]
pub struct MemoryRepositoryCreationStore {
    attempts: Mutex<Vec<StoredCreationAttempt>>,
}

impl RepositoryCreationStore for MemoryRepositoryCreationStore {
    fn begin(&self, attempt: &StoredCreationAttempt) -> Result<(), GitHubError> {
        let attempt = validate_attempt(attempt.clone())?;
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| GitHubError::credential_store())?;
        if attempts.iter().any(|value| value.id == attempt.id) {
            return Err(GitHubError::credential_store());
        }
        attempts.push(attempt);
        Ok(())
    }

    fn mark_outcome_unknown(&self, id: &str, updated_at: DateTime<Utc>) -> Result<(), GitHubError> {
        validate_attempt_id(id)?;
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| GitHubError::credential_store())?;
        let attempt = attempts
            .iter_mut()
            .find(|attempt| attempt.id == id)
            .ok_or_else(GitHubError::credential_store)?;
        attempt.state = StoredCreationState::OutcomeUnknown;
        attempt.updated_at = updated_at;
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), GitHubError> {
        validate_attempt_id(id)?;
        self.attempts
            .lock()
            .map_err(|_| GitHubError::credential_store())?
            .retain(|attempt| attempt.id != id);
        Ok(())
    }

    fn list_unresolved(&self) -> Result<Vec<StoredCreationAttempt>, GitHubError> {
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| GitHubError::credential_store())?
            .clone();
        attempts.sort_by_key(|attempt| std::cmp::Reverse(attempt.updated_at));
        attempts.truncate(MAX_ATTEMPTS);
        Ok(attempts)
    }
}

fn validate_attempt(attempt: StoredCreationAttempt) -> Result<StoredCreationAttempt, GitHubError> {
    validate_attempt_id(&attempt.id)?;
    let account = models::validate_account(attempt.account)?;
    models::validate_repository_identifier(&attempt.repository_name)?;
    if attempt.updated_at < attempt.created_at {
        return Err(GitHubError::credential_store());
    }
    Ok(StoredCreationAttempt { account, ..attempt })
}

pub(crate) fn validate_attempt_id(value: &str) -> Result<(), GitHubError> {
    if !(16..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(GitHubError::invalid_input());
    }
    Ok(())
}

fn format_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

type AttemptRow = (String, i64, String, String, String, String, String);

fn parse_attempt(row: AttemptRow) -> Result<StoredCreationAttempt, GitHubError> {
    if row.1 <= 0 {
        return Err(GitHubError::credential_store());
    }
    let created_at = parse_time(&row.5)?;
    let updated_at = parse_time(&row.6)?;
    validate_attempt(StoredCreationAttempt {
        id: row.0,
        account: GitHubAccount {
            id: u64::try_from(row.1).map_err(|_| GitHubError::credential_store())?,
            login: row.2,
        },
        repository_name: row.3,
        state: StoredCreationState::parse(&row.4)?,
        created_at,
        updated_at,
    })
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, GitHubError> {
    let parsed =
        DateTime::parse_from_rfc3339(value).map_err(|_| GitHubError::credential_store())?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err(GitHubError::credential_store());
    }
    Ok(parsed.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt() -> StoredCreationAttempt {
        StoredCreationAttempt {
            id: "safe-review-identifier-123".to_string(),
            account: GitHubAccount {
                id: 42,
                login: "octo-cat".to_string(),
            },
            repository_name: "ellie-workspace".to_string(),
            state: StoredCreationState::Dispatching,
            created_at: "2026-01-02T03:04:05Z".parse().expect("created"),
            updated_at: "2026-01-02T03:04:05Z".parse().expect("updated"),
        }
    }

    #[test]
    fn sqlite_attempt_survives_restart_and_can_be_resolved() {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("ellie.sqlite3");
        crate::storage::initialize(&path).expect("initialize");
        let store = SqliteRepositoryCreationStore::new(path.clone());
        store.begin(&attempt()).expect("begin");
        let reopened = SqliteRepositoryCreationStore::new(path);
        assert_eq!(reopened.list_unresolved().expect("list"), vec![attempt()]);
        let changed_at = "2026-01-02T03:05:00Z".parse().expect("changed");
        reopened
            .mark_outcome_unknown(&attempt().id, changed_at)
            .expect("mark unknown");
        let row = reopened.list_unresolved().expect("list").remove(0);
        assert_eq!(row.state, StoredCreationState::OutcomeUnknown);
        assert_eq!(row.updated_at, changed_at);
        reopened.remove(&row.id).expect("remove");
        assert!(reopened.list_unresolved().expect("empty").is_empty());
    }

    #[test]
    fn malformed_rows_are_rejected_without_exposing_values() {
        let temp = tempfile::tempdir().expect("temporary database");
        let path = temp.path().join("ellie.sqlite3");
        crate::storage::initialize(&path).expect("initialize");
        let connection = crate::storage::connect(&path).expect("connect");
        connection
            .execute(
                "INSERT INTO github_repository_creation_attempts
                    (id, account_id, account_login, repository_name, state,
                     created_at, updated_at)
                 VALUES (?1, 42, 'octo-cat', 'ellie', 'dispatching', ?2, ?2)",
                params!["safe-review-identifier-123", "not-a-time"],
            )
            .expect("insert malformed");
        let store = SqliteRepositoryCreationStore::new(path);
        assert_eq!(
            store.list_unresolved().expect_err("reject").category(),
            super::super::GitHubErrorCategory::CredentialStore
        );
    }

    #[test]
    fn missing_unknown_update_is_fail_closed() {
        let store = MemoryRepositoryCreationStore::default();
        assert_eq!(
            store
                .mark_outcome_unknown("safe-review-identifier-123", Utc::now())
                .expect_err("missing")
                .category(),
            super::super::GitHubErrorCategory::CredentialStore
        );
        assert_eq!(
            store
                .begin(&attempt())
                .and_then(|_| store.begin(&attempt()))
                .expect_err("duplicate")
                .category(),
            super::super::GitHubErrorCategory::CredentialStore
        );
    }
}
