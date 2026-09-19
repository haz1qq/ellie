use std::collections::HashSet;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionDay {
    pub date: NaiveDate,
    pub contribution_count: u32,
    pub level: u8,
    pub weekday: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionWeek {
    pub first_day: NaiveDate,
    pub days: Vec<ContributionDay>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionCalendar {
    pub total_contributions: u32,
    pub started_on: NaiveDate,
    pub ended_on: NaiveDate,
    pub weeks: Vec<ContributionWeek>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContributionCalendarQuery {
    /// Calendar year boundary. `None` means GitHub's rolling last-year window.
    #[serde(default)]
    pub year: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreationInput {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "private_by_default")]
    pub private: bool,
    #[serde(default)]
    pub initialize_readme: bool,
}

fn private_by_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreationReview {
    pub review_id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub initialize_readme: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryCreationResolution {
    Exists,
    NotFound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryCreationAttemptState {
    OutcomeUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreationAttemptStatus {
    pub attempt_id: String,
    pub owner: String,
    pub name: String,
    pub state: RepositoryCreationAttemptState,
    pub repository_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

#[derive(Deserialize)]
struct RawGraphQlResponse {
    data: Option<RawContributionData>,
    #[serde(default)]
    errors: Vec<RawGraphQlError>,
}

#[derive(Deserialize)]
struct RawGraphQlError {
    #[serde(rename = "type")]
    error_type: Option<String>,
}

#[derive(Deserialize)]
struct RawContributionData {
    user: Option<RawContributionUser>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContributionUser {
    contributions_collection: RawContributionsCollection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContributionsCollection {
    contribution_calendar: RawContributionCalendar,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContributionCalendar {
    total_contributions: u32,
    weeks: Vec<RawContributionWeek>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContributionWeek {
    first_day: NaiveDate,
    contribution_days: Vec<RawContributionDay>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContributionDay {
    contribution_count: u32,
    contribution_level: String,
    date: NaiveDate,
    weekday: u8,
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

pub(crate) fn parse_repository(body: &[u8]) -> Result<RepositorySummary, GitHubError> {
    let raw: RawRepository =
        serde_json::from_slice(body).map_err(|_| GitHubError::malformed_response())?;
    sanitize_repository(raw)
}

pub(crate) fn parse_commits(body: &[u8]) -> Result<Vec<CommitSummary>, GitHubError> {
    let raw: Vec<RawCommit> =
        serde_json::from_slice(body).map_err(|_| GitHubError::malformed_response())?;
    raw.into_iter().map(sanitize_commit).collect()
}

pub(crate) fn parse_contribution_calendar(
    body: &[u8],
) -> Result<ContributionCalendar, GitHubError> {
    let raw: RawGraphQlResponse =
        serde_json::from_slice(body).map_err(|_| GitHubError::malformed_response())?;
    if !raw.errors.is_empty() {
        return Err(classify_graphql_errors(&raw.errors));
    }
    let calendar = raw
        .data
        .and_then(|data| data.user)
        .ok_or_else(GitHubError::not_found)?
        .contributions_collection
        .contribution_calendar;
    sanitize_contribution_calendar(calendar)
}

fn classify_graphql_errors(errors: &[RawGraphQlError]) -> GitHubError {
    let has_type = |expected: &str| {
        errors.iter().any(|error| {
            error
                .error_type
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(expected))
        })
    };
    if has_type("RATE_LIMITED") {
        GitHubError::rate_limited()
    } else if has_type("UNAUTHENTICATED") {
        GitHubError::authentication_expired()
    } else if has_type("FORBIDDEN") {
        GitHubError::permission_denied()
    } else if has_type("NOT_FOUND") {
        GitHubError::not_found()
    } else {
        GitHubError::provider_unavailable()
    }
}

fn sanitize_contribution_calendar(
    raw: RawContributionCalendar,
) -> Result<ContributionCalendar, GitHubError> {
    if raw.weeks.is_empty() || raw.weeks.len() > 54 {
        return Err(GitHubError::malformed_response());
    }

    let mut weeks = Vec::with_capacity(raw.weeks.len());
    let mut seen_dates = HashSet::new();
    let mut previous_first_day = None;
    let mut started_on = None;
    let mut ended_on = None;

    for raw_week in raw.weeks {
        if raw_week.contribution_days.len() > 7
            || previous_first_day.is_some_and(|previous| previous >= raw_week.first_day)
        {
            return Err(GitHubError::malformed_response());
        }
        previous_first_day = Some(raw_week.first_day);
        let mut days = Vec::with_capacity(raw_week.contribution_days.len());
        for raw_day in raw_week.contribution_days {
            if raw_day.weekday > 6
                || raw_day.date.weekday().num_days_from_sunday() as u8 != raw_day.weekday
                || raw_day.date < raw_week.first_day
                || raw_day.date > raw_week.first_day + chrono::Duration::days(6)
                || !seen_dates.insert(raw_day.date)
            {
                return Err(GitHubError::malformed_response());
            }
            let level = match raw_day.contribution_level.as_str() {
                "NONE" => 0,
                "FIRST_QUARTILE" => 1,
                "SECOND_QUARTILE" => 2,
                "THIRD_QUARTILE" => 3,
                "FOURTH_QUARTILE" => 4,
                _ => return Err(GitHubError::malformed_response()),
            };
            started_on =
                Some(started_on.map_or(raw_day.date, |date: NaiveDate| date.min(raw_day.date)));
            ended_on =
                Some(ended_on.map_or(raw_day.date, |date: NaiveDate| date.max(raw_day.date)));
            days.push(ContributionDay {
                date: raw_day.date,
                contribution_count: raw_day.contribution_count,
                level,
                weekday: raw_day.weekday,
            });
        }
        weeks.push(ContributionWeek {
            first_day: raw_week.first_day,
            days,
        });
    }

    Ok(ContributionCalendar {
        total_contributions: raw.total_contributions,
        started_on: started_on.ok_or_else(GitHubError::malformed_response)?,
        ended_on: ended_on.ok_or_else(GitHubError::malformed_response)?,
        weeks,
    })
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

pub(crate) fn validate_new_repository_name(value: &str) -> Result<String, GitHubError> {
    validate_repository_identifier(value)?;
    if value != value.trim()
        || matches!(value, "." | "..")
        || value.chars().any(char::is_whitespace)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(GitHubError::invalid_input());
    }
    Ok(value.to_string())
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

/// GitHub launched in 2008; older years have no contribution calendar.
pub(crate) const EARLIEST_SUPPORTED_YEAR: u32 = 2008;

/// Resolves the optional calendar-year filter to a UTC `[start, end]` window.
/// `None` keeps GitHub's default rolling last-year window.
pub(crate) fn contribution_calendar_window(
    year: Option<u32>,
    current_year: i32,
) -> Result<Option<(NaiveDate, NaiveDate)>, GitHubError> {
    let Some(year) = year else { return Ok(None) };
    let year_i32 = i32::try_from(year).map_err(|_| GitHubError::invalid_input())?;
    if year < EARLIEST_SUPPORTED_YEAR || year_i32 > current_year {
        return Err(GitHubError::invalid_input());
    }
    let start = NaiveDate::from_ymd_opt(year_i32, 1, 1).ok_or_else(GitHubError::invalid_input)?;
    let end = NaiveDate::from_ymd_opt(year_i32, 12, 31).ok_or_else(GitHubError::invalid_input)?;
    Ok(Some((start, end)))
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
    fn parses_profile_contribution_calendar_and_preserves_github_levels() {
        let calendar = parse_contribution_calendar(
            br#"{"data":{"user":{"contributionsCollection":{"contributionCalendar":{"totalContributions":3,"weeks":[{"firstDay":"2026-09-06","contributionDays":[{"contributionCount":0,"contributionLevel":"NONE","date":"2026-09-06","weekday":0},{"contributionCount":3,"contributionLevel":"FOURTH_QUARTILE","date":"2026-09-07","weekday":1}]}]}}}}}"#,
        )
        .expect("contribution calendar");

        assert_eq!(calendar.total_contributions, 3);
        assert_eq!(calendar.started_on.to_string(), "2026-09-06");
        assert_eq!(calendar.ended_on.to_string(), "2026-09-07");
        assert_eq!(calendar.weeks[0].days[0].level, 0);
        assert_eq!(calendar.weeks[0].days[1].level, 4);
    }

    #[test]
    fn rejects_malformed_or_failed_contribution_calendars() {
        let invalid_weekday = br#"{"data":{"user":{"contributionsCollection":{"contributionCalendar":{"totalContributions":1,"weeks":[{"firstDay":"2026-09-06","contributionDays":[{"contributionCount":1,"contributionLevel":"FIRST_QUARTILE","date":"2026-09-07","weekday":2}]}]}}}}}"#;
        assert_eq!(
            parse_contribution_calendar(invalid_weekday)
                .expect_err("weekday mismatch")
                .category(),
            crate::github::GitHubErrorCategory::MalformedResponse
        );

        let forbidden =
            br#"{"data":null,"errors":[{"type":"FORBIDDEN","message":"redacted by parser"}]}"#;
        assert_eq!(
            parse_contribution_calendar(forbidden)
                .expect_err("GraphQL failure")
                .category(),
            crate::github::GitHubErrorCategory::PermissionDenied
        );
    }

    #[test]
    fn contribution_calendar_years_resolve_windows_and_reject_future_or_ancient_years() {
        assert_eq!(
            contribution_calendar_window(None, 2026),
            Ok(None),
            "no filter keeps GitHub's rolling window"
        );
        assert_eq!(
            contribution_calendar_window(Some(2025), 2026).expect("2025 window"),
            Some((
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
            ))
        );
        assert!(contribution_calendar_window(Some(2027), 2026).is_err());
        assert!(contribution_calendar_window(Some(2007), 2026).is_err());
        assert!(contribution_calendar_window(Some(0), 2026).is_err());
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
    fn repository_creation_input_is_private_by_default_and_names_are_bounded() {
        let input: RepositoryCreationInput = serde_json::from_value(serde_json::json!({
            "name": "ellie-workspace"
        }))
        .expect("creation input");
        assert!(input.private);
        assert!(!input.initialize_readme);
        assert!(validate_new_repository_name("ellie.workspace-1").is_ok());
        for invalid in ["", ".", "..", " bad", "bad name", "bad/name", "💥"] {
            assert!(validate_new_repository_name(invalid).is_err());
        }
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
