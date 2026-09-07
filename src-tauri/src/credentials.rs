use std::collections::BTreeMap;

use crate::error::AppError;

/// Windows Credential Manager wrapper via the keyring crate. Secrets are
/// never logged or returned through IPC; only presence/status is exposed.
pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, AppError>;
    fn set(&self, account: &str, secret: &str) -> Result<(), AppError>;
    fn delete(&self, account: &str) -> Result<(), AppError>;
}

const SERVICE: &str = "ellie";

/// Key-backed providers and the credential account name each uses.
pub const KEY_PROVIDERS: &[(&str, &str)] = &[
    ("openai-api", "openai_admin_api_key"),
    ("anthropic-claude", "anthropic_api_key"),
    ("deepseek", "deepseek_api_key"),
];

pub fn provider_account(provider_id: &str) -> Option<&str> {
    KEY_PROVIDERS
        .iter()
        .find(|(id, _)| *id == provider_id)
        .map(|(_, account)| *account)
}

/// Environment variable each key-backed provider also honors.
pub fn provider_env_var(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "openai-api" => Some("OPENAI_ADMIN_KEY"),
        "anthropic-claude" => Some("ANTHROPIC_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        _ => None,
    }
}

/// Environment first, then the OS credential store, with no logging of the
/// value at any point.
pub fn provider_key(
    store: &dyn SecretStore,
    provider_id: &str,
) -> Result<Option<String>, AppError> {
    if let Some(env_var) = provider_env_var(provider_id) {
        if let Ok(key) = std::env::var(env_var) {
            if !key.trim().is_empty() {
                return Ok(Some(key));
            }
        }
    }
    let Some(account) = provider_account(provider_id) else {
        return Ok(None);
    };
    store.get(account)
}

/// Reports whether a key is available and where it came from — never the key
/// itself.
pub fn provider_key_status(
    store: &dyn SecretStore,
    provider_id: &str,
) -> Result<KeySource, AppError> {
    if let Some(env_var) = provider_env_var(provider_id) {
        if std::env::var(env_var).is_ok_and(|value| !value.trim().is_empty()) {
            return Ok(KeySource::Environment);
        }
    }
    let Some(account) = provider_account(provider_id) else {
        return Ok(KeySource::None);
    };
    Ok(if store.get(account)?.is_some() {
        KeySource::CredentialManager
    } else {
        KeySource::None
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeySource {
    CredentialManager,
    Environment,
    None,
}

/// Bounds for keys accepted over IPC; secrets are never echoed back.
pub fn validate_key(key: &str) -> Result<String, AppError> {
    let trimmed = key.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(AppError::Storage);
    }
    Ok(trimmed.to_string())
}

/// Real store backed by Windows Credential Manager (keyring).
pub struct WindowsCredentialStore;

impl SecretStore for WindowsCredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|_| AppError::Storage)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::Storage),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|_| AppError::Storage)?;
        entry.set_password(secret).map_err(|_| AppError::Storage)
    }

    fn delete(&self, account: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(SERVICE, account).map_err(|_| AppError::Storage)?;
        entry.delete_credential().map_err(|_| AppError::Storage)
    }
}

/// In-memory store used by tests (and safe for environments without an OS
/// credential service).
#[derive(Default)]
pub struct MemoryStore {
    secrets: std::sync::Mutex<BTreeMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .secrets
            .lock()
            .ok()
            .and_then(|map| map.get(account).cloned()))
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), AppError> {
        self.secrets
            .lock()
            .map_err(|_| AppError::Storage)?
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), AppError> {
        self.secrets
            .lock()
            .map_err(|_| AppError::Storage)?
            .remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_wins_over_store_and_status_reports_source() {
        let store = MemoryStore::default();
        store
            .set("deepseek_api_key", "sk-store")
            .expect("store write");
        assert_eq!(
            provider_key_status(&store, "deepseek").expect("status"),
            KeySource::CredentialManager
        );
        // env set → env wins, status says environment
        std::env::set_var("DEEPSEEK_API_KEY", "sk-env");
        assert_eq!(
            provider_key(&store, "deepseek").expect("key").as_deref(),
            Some("sk-env")
        );
        assert_eq!(
            provider_key_status(&store, "deepseek").expect("status"),
            KeySource::Environment
        );
        std::env::remove_var("DEEPSEEK_API_KEY");
        assert_eq!(
            provider_key_status(&store, "deepseek").expect("status"),
            KeySource::CredentialManager
        );
    }

    #[test]
    fn openai_api_uses_a_distinct_admin_credential() {
        assert_eq!(provider_account("openai-api"), Some("openai_admin_api_key"));
        assert_eq!(provider_env_var("openai-api"), Some("OPENAI_ADMIN_KEY"));
        assert_ne!(
            provider_account("openai-api"),
            provider_account("openai-codex")
        );
    }

    #[test]
    fn unknown_provider_has_no_key_or_status() {
        let store = MemoryStore::default();
        assert_eq!(provider_key(&store, "openai-codex").expect("key"), None);
        assert_eq!(
            provider_key_status(&store, "openai-codex").expect("status"),
            KeySource::None
        );
        assert!(provider_account("openai-codex").is_none());
    }

    #[test]
    fn validate_key_rejects_empty_and_overlong() {
        assert!(validate_key("   ").is_err());
        assert!(validate_key(&"k".repeat(513)).is_err());
        assert_eq!(validate_key("  sk-abc  ").expect("key"), "sk-abc");
    }
}
