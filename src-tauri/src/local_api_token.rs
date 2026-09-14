//! Narrow authentication shared by the app and HTTP-only CLI. No provider or database access.
use std::ffi::OsString;

pub const SERVICE: &str = "ellie";
pub const ACCOUNT: &str = "local_api_token";
pub const ENVIRONMENT: &str = "ELLIE_API_TOKEN";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("ELLIE_API_TOKEN is invalid; unset it to use Windows Credential Manager")]
    InvalidOverride,
    #[error("local API token is missing; enable Local API in Settings > Integrations")]
    Missing,
    #[error("local API credential is unavailable; check Windows Credential Manager")]
    Store,
}

// Visible ASCII without whitespace is safe in an Authorization header. Do not trim or
// silently replace explicit overrides. The size bound also bounds authentication work.
pub fn valid(token: &str) -> bool {
    !token.is_empty() && token.len() <= 512 && token.bytes().all(|b| (33..=126).contains(&b))
}

pub fn resolve(
    explicit: Option<OsString>,
    read: impl FnOnce() -> Result<Option<String>, TokenError>,
) -> Result<String, TokenError> {
    if let Some(explicit) = explicit {
        return explicit
            .into_string()
            .ok()
            .filter(|s| valid(s))
            .ok_or(TokenError::InvalidOverride);
    }
    let token = read()?.ok_or(TokenError::Missing)?;
    if !valid(&token) {
        return Err(TokenError::Store);
    }
    Ok(token)
}

/// Blocking OS operation: callers must use a blocking worker, never an async/UI thread.
pub fn read_stored_token() -> Result<Option<String>, TokenError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| TokenError::Store)?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(TokenError::Store),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_override_never_falls_back_or_reads_store() {
        for input in ["", " ", "bad\nheader", "a b", &"x".repeat(513)] {
            assert!(matches!(
                resolve(Some(input.into()), || panic!("must not read store")),
                Err(TokenError::InvalidOverride)
            ));
        }
        assert!(resolve(Some("test-only-override".into()), || panic!(
            "must not read store"
        ))
        .is_ok());
    }

    #[test]
    fn automatic_resolution_distinguishes_missing_and_store_error() {
        assert!(matches!(
            resolve(None, || Ok(None)),
            Err(TokenError::Missing)
        ));
        assert!(matches!(
            resolve(None, || Err(TokenError::Store)),
            Err(TokenError::Store)
        ));
        assert!(matches!(
            resolve(None, || Ok(Some(" ".into()))),
            Err(TokenError::Store)
        ));
        assert!(resolve(None, || Ok(Some("test-only-stored".into()))).is_ok());
    }
}
