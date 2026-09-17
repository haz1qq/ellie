use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::GitHubError;

const MAX_LOGIN_CHARS: usize = 100;
const MAX_REPOSITORY_NAME_CHARS: usize = 100;
const MAX_FULL_NAME_CHARS: usize = 201;
const MAX_BRANCH_CHARS: usize = 255;
const MAX_COMMIT_SUBJECT_CHARS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubAccount {
    pub id: u64,
    pub login: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub default_branch: String,
    pub html_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub sha: String,
    pub subject: String,
    pub author_id: Option<u64>,
    pub author_login: Option<String>,
    pub authored_at: DateTime<Utc>,
    pub committed_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct RawGitHubUser {
    id: u64,
    login: String,
}

#[derive(Deserialize)]
struct RawRepository {
    id: u64,
    name: String,
    full_name: String,
    private: bool,
    default_branch: String,
    html_url: String,
}

#[derive(Deserialize)]
struct RawCommit {
    sha: String,
    commit: RawCommitDetails,
    author: Option<RawGitHubUser>,
}

#[derive(Deserialize)]
struct RawCommitDetails {
    message: String,
    author: RawGitSignature,
    committer: RawGitSignature,
}

#[derive(Deserialize)]
struct RawGitSignature {
    date: DateTime<Utc>,
}

pub(crate) fn parse_user(body: &[u8]) -> Result<GitHubAccount, GitHubError> {
    let raw: RawGitHubUser =
        serde_json::from_slice(body).map_err(|_| GitHubError::malformed_response())?;
    sanitize_user(raw)
}

pub(crate) fn parse_repositories(body: &[u8]) -> Result<Vec<RepositorySummary>, GitHubError> {
    let raw: Vec<RawRepository> =
        serde_json::from_slice(body).map_err(|_| GitHubError::malformed_response())?;
    raw.into_iter().map(sanitize_repository).collect()
}

pub(crate) fn parse_commits(body: &[u8]) -> Result<Vec<CommitSummary>, GitHubError> {
    let raw: Vec<RawCommit> =
        serde_json::from_slice(body).map_err(|_| GitHubError::malformed_response())?;
    raw.into_iter().map(sanitize_commit).collect()
}

fn sanitize_user(raw: RawGitHubUser) -> Result<GitHubAccount, GitHubError> {
    validate_account(GitHubAccount {
        id: raw.id,
        login: raw.login,
    })
}

pub(crate) fn validate_account(account: GitHubAccount) -> Result<GitHubAccount, GitHubError> {
    if account.id == 0 {
        return Err(GitHubError::malformed_response());
    }
    let login = sanitize_required_text(&account.login, MAX_LOGIN_CHARS, false)?;
    if login.contains(['/', '\\']) {
        return Err(GitHubError::malformed_response());
    }
    Ok(GitHubAccount {
        id: account.id,
        login,
    })
}

fn sanitize_repository(raw: RawRepository) -> Result<RepositorySummary, GitHubError> {
    if raw.id == 0 {
        return Err(GitHubError::malformed_response());
    }
    let name = sanitize_required_text(&raw.name, MAX_REPOSITORY_NAME_CHARS, false)?;
    if name.contains(['/', '\\']) {
        return Err(GitHubError::malformed_response());
    }
    let full_name = sanitize_required_text(&raw.full_name, MAX_FULL_NAME_CHARS, false)?;
    let Some((owner_name, repository_name)) = full_name.split_once('/') else {
        return Err(GitHubError::malformed_response());
    };
    if owner_name.is_empty()
        || repository_name.is_empty()
        || repository_name.contains('/')
        || full_name.contains('\\')
        || repository_name != name
    {
        return Err(GitHubError::malformed_response());
    }
    let default_branch = sanitize_required_text(&raw.default_branch, MAX_BRANCH_CHARS, false)?;
    let html_url = sanitize_html_url(&raw.html_url)?;
    Ok(RepositorySummary {
        id: raw.id,
        name,
        full_name,
        private: raw.private,
        default_branch,
        html_url,
    })
}

fn sanitize_commit(raw: RawCommit) -> Result<CommitSummary, GitHubError> {
    if !(40..=64).contains(&raw.sha.len()) || !raw.sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GitHubError::malformed_response());
    }
    let subject = sanitize_subject(&raw.commit.message);
    let author = raw.author.map(sanitize_user).transpose()?;
    Ok(CommitSummary {
        sha: raw.sha.to_ascii_lowercase(),
        subject,
        author_id: author.as_ref().map(|value| value.id),
        author_login: author.map(|value| value.login),
        authored_at: raw.commit.author.date,
        committed_at: raw.commit.committer.date,
    })
}

fn sanitize_html_url(value: &str) -> Result<String, GitHubError> {
    let value = sanitize_required_text(value, 2_048, false)?;
    let url = Url::parse(&value).map_err(|_| GitHubError::malformed_response())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(GitHubError::malformed_response());
    }
    Ok(url.into())
}

fn sanitize_required_text(
    value: &str,
    max_chars: usize,
    allow_empty: bool,
) -> Result<String, GitHubError> {
    if value.chars().count() > max_chars || value.chars().any(char::is_control) {
        return Err(GitHubError::malformed_response());
    }
    let trimmed = value.trim();
    if !allow_empty && trimmed.is_empty() {
        return Err(GitHubError::malformed_response());
    }
    Ok(trimmed.to_string())
}

fn sanitize_subject(message: &str) -> String {
    let first_line = message.split(['\r', '\n']).next().unwrap_or_default();
    let mut subject = String::new();
    for character in first_line.chars().take(MAX_COMMIT_SUBJECT_CHARS) {
        if character == '\t' {
            subject.push(' ');
        } else if !character.is_control() {
            subject.push(character);
        }
    }
    subject.trim().to_string()
}

pub(crate) fn validate_repository_identifier(value: &str) -> Result<(), GitHubError> {
    if value.is_empty()
        || value.chars().count() > MAX_REPOSITORY_NAME_CHARS
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
    {
        return Err(GitHubError::invalid_input());
    }
    Ok(())
}

pub(crate) fn validate_branch(value: &str) -> Result<(), GitHubError> {
    if value.is_empty()
        || value.chars().count() > MAX_BRANCH_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(GitHubError::invalid_input());
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) struct BoundedPagination {
    current_page: usize,
    loaded_rows: usize,
    max_pages: usize,
    max_rows: usize,
}

pub(crate) struct PageDecision {
    pub take_rows: usize,
    pub next_page: Option<usize>,
}

impl BoundedPagination {
    pub fn new(max_pages: usize, max_rows: usize) -> Self {
        Self {
            current_page: 1,
            loaded_rows: 0,
            max_pages: max_pages.max(1),
            max_rows: max_rows.max(1),
        }
    }

    pub fn current_page(self) -> usize {
        self.current_page
    }

    pub fn accept_page(&mut self, page_rows: usize, has_next: bool) -> PageDecision {
        let remaining = self.max_rows.saturating_sub(self.loaded_rows);
        let take_rows = page_rows.min(remaining);
        self.loaded_rows = self.loaded_rows.saturating_add(take_rows);
        let can_continue = has_next
            && take_rows == page_rows
            && self.loaded_rows < self.max_rows
            && self.current_page < self.max_pages;
        if can_continue {
            self.current_page += 1;
        }
        PageDecision {
            take_rows,
            next_page: can_continue.then_some(self.current_page),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sanitized_user_repository_and_commit_fixtures() {
        let user =
            parse_user(br#"{"id":42,"login":"octo-cat","ignored":"value"}"#).expect("user fixture");
        assert_eq!(user.id, 42);
        assert_eq!(user.login, "octo-cat");

        let repositories = parse_repositories(
            br#"[{"id":7,"name":"ellie","full_name":"octo-cat/ellie","private":true,"default_branch":"main","html_url":"https://github.com/octo-cat/ellie"}]"#,
        )
        .expect("repository fixture");
        assert_eq!(repositories[0].default_branch, "main");
        assert!(repositories[0].private);

        let commits = parse_commits(
            br#"[{"sha":"0123456789abcdef0123456789abcdef01234567","commit":{"message":"Fix <b>display</b>\n\nNever returned","author":{"date":"2026-01-02T03:04:05Z"},"committer":{"date":"2026-01-02T04:05:06+00:00"}},"author":{"id":42,"login":"octo-cat"}}]"#,
        )
        .expect("commit fixture");
        assert_eq!(commits[0].subject, "Fix <b>display</b>");
        assert_eq!(commits[0].author_id, Some(42));
        assert_eq!(commits[0].author_login.as_deref(), Some("octo-cat"));
        assert_eq!(
            commits[0].authored_at.to_rfc3339(),
            "2026-01-02T03:04:05+00:00"
        );
    }

    #[test]
    fn missing_required_fields_and_hostile_values_are_rejected() {
        for body in [
            br#"{"login":"octo-cat"}"#.as_slice(),
            br#"{"id":42}"#.as_slice(),
            br#"{"id":42,"login":"bad/login"}"#.as_slice(),
        ] {
            assert!(parse_user(body).is_err());
        }
        assert!(parse_repositories(
            br#"[{"id":7,"name":"ellie","full_name":"octo-cat/ellie","private":true,"default_branch":"main"}]"#
        )
        .is_err());
        assert!(parse_repositories(
            br#"[{"id":7,"name":"ellie","full_name":"octo-cat/ellie","private":true,"default_branch":"main","html_url":"javascript:alert(1)"}]"#
        )
        .is_err());
        assert!(parse_commits(
            br#"[{"sha":"not-a-sha","commit":{"message":"subject","author":{"date":"2026-01-02T03:04:05Z"},"committer":{"date":"2026-01-02T04:05:06Z"}},"author":null}]"#
        )
        .is_err());
    }

    #[test]
    fn commit_subject_is_plain_bounded_text_and_author_is_optional() {
        let message = format!("hello\tworld\u{0007}{}\nbody", "x".repeat(600));
        let fixture = serde_json::json!([{
            "sha": "abcdef0123456789abcdef0123456789abcdef01",
            "commit": {
                "message": message,
                "author": {"date": "2026-01-02T03:04:05Z"},
                "committer": {"date": "2026-01-02T04:05:06Z"}
            },
            "author": null
        }]);
        let commits =
            parse_commits(&serde_json::to_vec(&fixture).expect("fixture")).expect("commit fixture");
        assert!(commits[0].subject.starts_with("hello world"));
        assert!(commits[0].subject.chars().count() <= MAX_COMMIT_SUBJECT_CHARS);
        assert_eq!(commits[0].author_id, None);
        assert_eq!(commits[0].author_login, None);
    }

    #[test]
    fn pagination_never_exceeds_page_or_row_bounds() {
        let mut pagination = BoundedPagination::new(2, 3);
        assert_eq!(pagination.current_page(), 1);
        let first = pagination.accept_page(2, true);
        assert_eq!(first.take_rows, 2);
        assert_eq!(first.next_page, Some(2));
        let second = pagination.accept_page(2, true);
        assert_eq!(second.take_rows, 1);
        assert_eq!(second.next_page, None);
    }

    #[test]
    fn validates_repository_identifiers_and_branches() {
        for invalid in ["", "owner/repo", "owner\\repo", "bad\nname"] {
            assert!(validate_repository_identifier(invalid).is_err());
        }
        assert!(validate_repository_identifier(&"x".repeat(101)).is_err());
        assert!(validate_repository_identifier("octo-cat").is_ok());
        assert!(validate_branch("feature/workspace").is_ok());
        assert!(validate_branch("bad\rbranch").is_err());
    }
}
