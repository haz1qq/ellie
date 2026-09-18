use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::{header, Client, StatusCode, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{GitHubError, GitHubErrorCategory};

pub const ACCESS_TOKEN_LIFETIME_SECONDS: u64 = 28_800;
pub const REFRESH_TOKEN_LIFETIME_SECONDS: u64 = 15_897_600;

const MAX_ACCESS_TOKEN_LIFETIME_SECONDS: u64 = 24 * 60 * 60;
const MAX_REFRESH_TOKEN_LIFETIME_SECONDS: u64 = 366 * 24 * 60 * 60;
const MAX_TOKEN_BYTES: usize = 2 * 1024;
const AUTHORIZE_PATH: &str = "/login/oauth/authorize";
const TOKEN_PATH: &str = "/login/oauth/access_token";
const MAX_AUTH_RESPONSE_BYTES: usize = 64 * 1024;
const AUTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

pub(crate) struct TokenSet {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub refresh_token_expires_in: u64,
}

#[derive(Deserialize)]
struct RawTokenSet {
    access_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<u64>,
    token_type: Option<String>,
}

#[derive(Deserialize)]
struct OAuthErrorBody {
    error: String,
}

pub(crate) fn generate_pkce() -> Result<PkcePair, GitHubError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|_| GitHubError::provider_unavailable())?;
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Ok(PkcePair {
        verifier,
        challenge,
    })
}

pub(crate) fn generate_state() -> Result<String, GitHubError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|_| GitHubError::provider_unavailable())?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}

pub(crate) fn build_authorize_url(
    auth_base_url: &str,
    client_id: &str,
    redirect_port: u16,
    challenge: &str,
    state: &str,
) -> Result<(String, String), GitHubError> {
    validate_client_id(client_id)?;
    if redirect_port == 0 || challenge.len() != 43 || state.len() != 43 {
        return Err(GitHubError::invalid_input());
    }

    let redirect_uri = format!("http://127.0.0.1:{redirect_port}/callback");
    let mut url = endpoint(auth_base_url, AUTHORIZE_PATH)?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    Ok((url.into(), redirect_uri))
}

pub(crate) async fn exchange_code(
    client: &Client,
    auth_base_url: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenSet, GitHubError> {
    validate_client_id(client_id)?;
    validate_client_secret(client_secret)?;
    validate_authorization_value(code, 1, 512)?;
    validate_authorization_value(verifier, 43, 128)?;
    post_token(
        client,
        auth_base_url,
        &[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
        ],
    )
    .await
}

pub(crate) async fn refresh_token(
    client: &Client,
    auth_base_url: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenSet, GitHubError> {
    validate_client_id(client_id)?;
    validate_client_secret(client_secret)?;
    validate_authorization_value(refresh_token, 1, 512)?;
    post_token(
        client,
        auth_base_url,
        &[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ],
    )
    .await
}

async fn post_token(
    client: &Client,
    auth_base_url: &str,
    fields: &[(&str, &str)],
) -> Result<TokenSet, GitHubError> {
    let endpoint = endpoint(auth_base_url, TOKEN_PATH)?;
    let form_body = encode_form(fields)?;
    let response = client
        .post(endpoint)
        .header(header::ACCEPT, "application/json")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .timeout(AUTH_REQUEST_TIMEOUT)
        .body(form_body)
        .send()
        .await
        .map_err(map_transport_error)?;
    let status = response.status();
    let body = read_bounded(response, MAX_AUTH_RESPONSE_BYTES).await?;
    parse_token_response(status, &body)
}

fn encode_form(fields: &[(&str, &str)]) -> Result<String, GitHubError> {
    let mut form = Url::parse("http://127.0.0.1/").map_err(|_| GitHubError::invalid_input())?;
    {
        let mut query = form.query_pairs_mut();
        for (key, value) in fields {
            query.append_pair(key, value);
        }
    }
    form.query()
        .map(str::to_owned)
        .ok_or_else(GitHubError::invalid_input)
}

async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, GitHubError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(GitHubError::malformed_response());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_transport_error)? {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(GitHubError::malformed_response)?;
        if next_len > limit {
            return Err(GitHubError::malformed_response());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(crate) fn parse_token_response(
    status: StatusCode,
    body: &[u8],
) -> Result<TokenSet, GitHubError> {
    if let Ok(error) = serde_json::from_slice::<OAuthErrorBody>(body) {
        return Err(map_oauth_error(&error.error));
    }
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => GitHubError::app_credentials_invalid(),
            403 => GitHubError::permission_denied(),
            429 => GitHubError::rate_limited(),
            500..=599 => GitHubError::provider_unavailable(),
            _ => GitHubError::authorization_denied(),
        });
    }

    let raw: RawTokenSet =
        serde_json::from_slice(body).map_err(|_| GitHubError::token_response_invalid())?;
    let has_access_token = raw.access_token.as_deref().is_some_and(valid_opaque_token);
    let has_bearer_type = raw
        .token_type
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("bearer"));
    if has_access_token
        && has_bearer_type
        && (raw.expires_in.is_none()
            || raw.refresh_token.is_none()
            || raw.refresh_token_expires_in.is_none())
    {
        return Err(GitHubError::token_expiration_required());
    }

    let (Some(access_token), Some(expires_in), Some(refresh_token), Some(refresh_expires_in)) = (
        raw.access_token,
        raw.expires_in,
        raw.refresh_token,
        raw.refresh_token_expires_in,
    ) else {
        return Err(GitHubError::token_response_invalid());
    };
    if !has_bearer_type
        || !valid_opaque_token(&access_token)
        || !valid_opaque_token(&refresh_token)
        || expires_in == 0
        || expires_in > MAX_ACCESS_TOKEN_LIFETIME_SECONDS
        || refresh_expires_in <= expires_in
        || refresh_expires_in > MAX_REFRESH_TOKEN_LIFETIME_SECONDS
    {
        return Err(GitHubError::token_response_invalid());
    }
    Ok(TokenSet {
        access_token,
        expires_in,
        refresh_token,
        refresh_token_expires_in: refresh_expires_in,
    })
}

fn valid_opaque_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TOKEN_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn map_oauth_error(error: &str) -> GitHubError {
    match error {
        "bad_refresh_token" | "expired_token" | "bad_verification_code" => {
            GitHubError::authentication_expired()
        }
        "access_denied" => GitHubError::authorization_denied(),
        "incorrect_client_credentials" => GitHubError::app_credentials_invalid(),
        "slow_down" => GitHubError::rate_limited(),
        _ => GitHubError::authorization_denied(),
    }
}

pub(crate) fn validate_client_id(client_id: &str) -> Result<(), GitHubError> {
    if client_id.is_empty()
        || client_id.len() > 128
        || !client_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b'\\')
    {
        return Err(GitHubError::invalid_input());
    }
    Ok(())
}

pub(crate) fn validate_client_secret(client_secret: &str) -> Result<(), GitHubError> {
    validate_authorization_value(client_secret, 1, 512)
}

fn validate_authorization_value(
    value: &str,
    min_len: usize,
    max_len: usize,
) -> Result<(), GitHubError> {
    if value.len() < min_len
        || value.len() > max_len
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(GitHubError::invalid_input());
    }
    Ok(())
}

fn endpoint(base_url: &str, path: &str) -> Result<Url, GitHubError> {
    let base = base_url.trim_end_matches('/');
    Url::parse(&format!("{base}{path}")).map_err(|_| GitHubError::invalid_input())
}

fn map_transport_error(error: reqwest::Error) -> GitHubError {
    if error.is_timeout() || error.is_connect() {
        GitHubError::network_unavailable()
    } else {
        GitHubError::provider_unavailable()
    }
}

pub(crate) fn state_matches(expected: &str, actual: &str) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    expected
        .bytes()
        .zip(actual.bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

impl GitHubError {
    fn authorization_denied() -> Self {
        Self::new(GitHubErrorCategory::AuthorizationDenied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUCCESS: &str = r#"{
        "access_token":"ghu_sanitized_access_value",
        "expires_in":28800,
        "refresh_token":"ghr_sanitized_refresh_value",
        "refresh_token_expires_in":15897600,
        "token_type":"bearer",
        "scope":""
    }"#;

    #[test]
    fn pkce_verifier_and_challenge_are_base64url_sha256_without_padding() {
        let pair = generate_pkce().expect("PKCE generation");
        assert_eq!(pair.verifier.len(), 43);
        assert_eq!(pair.challenge.len(), 43);
        assert!(!pair.verifier.contains('='));
        assert!(!pair.challenge.contains('='));
        let decoded = URL_SAFE_NO_PAD
            .decode(pair.verifier.as_bytes())
            .expect("verifier round trip");
        assert_eq!(decoded.len(), 32);
        assert_eq!(URL_SAFE_NO_PAD.encode(decoded), pair.verifier);
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(pair.verifier.as_bytes())),
            pair.challenge
        );
    }

    #[test]
    fn authorize_url_has_only_public_pkce_parameters() {
        let pair = generate_pkce().expect("PKCE generation");
        let state = generate_state().expect("state generation");
        let (url, redirect_uri) = build_authorize_url(
            "https://github.com",
            "Iv1.sanitized-client",
            58210,
            &pair.challenge,
            &state,
        )
        .expect("authorize URL");
        let parsed = Url::parse(&url).expect("URL");
        let parameters = parsed
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(parsed.path(), AUTHORIZE_PATH);
        assert_eq!(parameters.len(), 6);
        assert_eq!(
            parameters.get("client_id").map(String::as_str),
            Some("Iv1.sanitized-client")
        );
        assert_eq!(
            parameters.get("response_type").map(String::as_str),
            Some("code")
        );
        assert_eq!(parameters.get("redirect_uri"), Some(&redirect_uri));
        assert_eq!(parameters.get("code_challenge"), Some(&pair.challenge));
        assert_eq!(
            parameters.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(parameters.get("state"), Some(&state));
        assert!(!url.contains(&pair.verifier));
        assert!(!url.contains("access_token"));
        assert!(!url.contains("refresh_token"));
        assert!(!url.contains("client_secret"));
    }

    #[test]
    fn parses_exchange_and_refresh_success_with_documented_expiry() {
        for status in [StatusCode::OK, StatusCode::CREATED] {
            let token = parse_token_response(status, SUCCESS.as_bytes()).expect("token response");
            assert_eq!(token.expires_in, ACCESS_TOKEN_LIFETIME_SECONDS);
            assert_eq!(
                token.refresh_token_expires_in,
                REFRESH_TOKEN_LIFETIME_SECONDS
            );
        }
    }

    #[test]
    fn parses_redacted_oauth_errors_and_classifies_token_shape_failures() {
        let error = parse_token_response(
            StatusCode::BAD_REQUEST,
            br#"{"error":"bad_refresh_token","error_description":"sensitive provider copy"}"#,
        )
        .err()
        .expect("error response");
        assert_eq!(error.category(), GitHubErrorCategory::AuthenticationExpired);
        assert!(!error.to_string().contains("sensitive"));

        let invalid_credentials = parse_token_response(
            StatusCode::UNAUTHORIZED,
            br#"{"error":"incorrect_client_credentials","error_description":"sensitive provider copy"}"#,
        )
        .err()
        .expect("invalid client credentials");
        assert_eq!(
            invalid_credentials.category(),
            GitHubErrorCategory::AppCredentialsInvalid
        );
        assert!(!invalid_credentials.to_string().contains("sensitive"));

        let missing_refresh = br#"{
            "access_token":"opaque_sanitized_access_value",
            "expires_in":28800,
            "token_type":"bearer",
            "scope":""
        }"#;
        assert_eq!(
            parse_token_response(StatusCode::OK, missing_refresh)
                .err()
                .expect("non-expiring token response")
                .category(),
            GitHubErrorCategory::TokenExpirationRequired
        );

        let excessive_expiry = SUCCESS.replace("28800", "172800");
        assert_eq!(
            parse_token_response(StatusCode::OK, excessive_expiry.as_bytes())
                .err()
                .expect("unsupported expiry")
                .category(),
            GitHubErrorCategory::TokenResponseInvalid
        );
        assert_eq!(
            parse_token_response(StatusCode::OK, br#"{"unexpected":true}"#)
                .err()
                .expect("unsupported response")
                .category(),
            GitHubErrorCategory::TokenResponseInvalid
        );
    }

    #[test]
    fn treats_provider_tokens_as_opaque_bounded_values() {
        let changed_prefixes = SUCCESS
            .replace("ghu_", "future_access_")
            .replace("ghr_", "future_refresh_");
        let token = parse_token_response(StatusCode::OK, changed_prefixes.as_bytes())
            .expect("opaque provider tokens");
        assert!(token.access_token.starts_with("future_access_"));
        assert!(token.refresh_token.starts_with("future_refresh_"));
    }

    #[test]
    fn state_comparison_rejects_mismatches_strictly() {
        assert!(state_matches("same-state", "same-state"));
        assert!(!state_matches("same-state", "same-state-extra"));
        assert!(!state_matches("same-state", "same-statz"));
    }
}
