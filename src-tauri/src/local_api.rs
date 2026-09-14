//! Serialized, fail-closed local API lifecycle. Only status (never the token) crosses IPC.
use std::{ffi::OsString, fs::File, path::PathBuf, sync::Arc};

use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    credentials::SecretStore,
    local_api_token::{self, TokenError},
    storage,
};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TokenSource {
    None,
    Environment,
    CredentialManager,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApiFailure {
    CredentialStore,
    InvalidOverride,
    MissingToken,
    Persistence,
    Bind,
    Server,
    OverrideActive,
    Disabled,
    ControlsUnavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStatus {
    pub enabled: bool,
    pub listening: bool,
    pub token_source: TokenSource,
    pub error: Option<ApiFailure>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiAction {
    Enable,
    Disable,
    Rotate,
}

struct Active {
    status: ApiStatus,
    token: Option<String>,
    generation: u64,
}

/// Middleware sees the current token on every request, including existing keep-alive connections.
#[derive(Clone)]
pub struct Authorization(Arc<RwLock<Active>>);

impl Authorization {
    pub async fn accepts(&self, provided: Option<&str>) -> bool {
        let active = self.0.read().await;
        if !active.status.enabled || !active.status.listening {
            return false;
        }
        match (active.token.as_deref(), provided) {
            (Some(expected), Some(provided)) if provided.len() <= 512 => {
                constant_time_equal(expected.as_bytes(), provided.as_bytes())
            }
            _ => false,
        }
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

#[derive(Default)]
struct Lifecycle {
    // OS lock shared by all controllers using this application data directory.
    owner: Option<Arc<File>>,
    initialized: bool,
    server: Option<JoinHandle<()>>,
}

pub struct LocalApi {
    path: PathBuf,
    store: Arc<dyn SecretStore>,
    explicit: Option<OsString>,
    auth: Authorization,
    lifecycle: Mutex<Lifecycle>,
    address: String,
}

impl LocalApi {
    pub fn new(
        path: PathBuf,
        store: Arc<dyn SecretStore>,
        explicit: Option<OsString>,
        address: &str,
    ) -> Arc<Self> {
        let source = if explicit.is_some() {
            TokenSource::Environment
        } else {
            TokenSource::None
        };
        Arc::new(Self {
            path,
            store,
            explicit,
            auth: Authorization(Arc::new(RwLock::new(Active {
                status: ApiStatus {
                    enabled: false,
                    listening: false,
                    token_source: source,
                    error: None,
                },
                token: None,
                generation: 0,
            }))),
            lifecycle: Mutex::new(Lifecycle::default()),
            address: address.into(),
        })
    }

    pub fn authorization(&self) -> Authorization {
        self.auth.clone()
    }
    pub async fn status(&self) -> ApiStatus {
        let lifecycle = self.lifecycle.lock().await;
        if lifecycle.initialized && lifecycle.owner.is_none() {
            // A secondary window reports the shared preference, not its old startup snapshot.
            let error = self
                .read_preference()
                .await
                .err()
                .unwrap_or(ApiFailure::ControlsUnavailable);
            self.fail(error, true).await;
        }
        self.snapshot().await
    }

    async fn snapshot(&self) -> ApiStatus {
        self.auth.0.read().await.status.clone()
    }

    async fn read_preference(&self) -> Result<(), ApiFailure> {
        let path = self.path.clone();
        let enabled = tokio::task::spawn_blocking(move || storage::read_local_api_enabled(&path))
            .await
            .map_err(|_| ApiFailure::Persistence)?
            .map_err(|_| ApiFailure::Persistence)?;
        self.auth.0.write().await.status.enabled = enabled;
        Ok(())
    }

    async fn acquire_owner(&self) -> Result<Arc<File>, ApiFailure> {
        let path = self
            .path
            .parent()
            .ok_or(ApiFailure::ControlsUnavailable)?
            .join("local-api.lock");
        tokio::task::spawn_blocking(move || {
            // Empty, non-secret file. Do not unlink it: all instances must lock the same inode.
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .map_err(|_| ApiFailure::ControlsUnavailable)?;
            fs2::FileExt::try_lock_exclusive(&file).map_err(|_| ApiFailure::ControlsUnavailable)?;
            Ok(Arc::new(file))
        })
        .await
        .map_err(|_| ApiFailure::ControlsUnavailable)?
    }

    /// Detached operation owns the serialization lock even if its IPC caller goes away.
    pub async fn apply(self: &Arc<Self>, action: Option<ApiAction>, router: Router) -> ApiStatus {
        let controller = self.clone();
        let fallback = self.clone();
        match tokio::spawn(async move { controller.execute(action, router).await }).await {
            Ok(status) => status,
            Err(_) => {
                fallback.fail(ApiFailure::Server, true).await;
                fallback.status().await
            }
        }
    }

    async fn fail(&self, error: ApiFailure, revoke: bool) {
        let mut active = self.auth.0.write().await;
        active.status.error = Some(error);
        if revoke {
            active.token = None;
            active.status.listening = false;
        }
    }

    async fn persist(&self, enabled: bool) -> Result<(), ApiFailure> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || storage::save_local_api_enabled(&path, enabled))
            .await
            .map_err(|_| ApiFailure::Persistence)?
            .map_err(|_| ApiFailure::Persistence)
    }

    async fn token(&self, generate_missing: bool, rotate: bool) -> Result<String, ApiFailure> {
        let store = self.store.clone();
        let explicit = self.explicit.clone();
        tokio::task::spawn_blocking(move || {
            if rotate && explicit.is_some() {
                return Err(ApiFailure::OverrideActive);
            }
            if !rotate {
                match local_api_token::resolve(explicit, || {
                    store
                        .get(local_api_token::ACCOUNT)
                        .map_err(|_| TokenError::Store)
                }) {
                    Ok(token) => return Ok(token),
                    Err(TokenError::Missing) if generate_missing => {}
                    Err(TokenError::Missing) => return Err(ApiFailure::MissingToken),
                    Err(TokenError::InvalidOverride) => return Err(ApiFailure::InvalidOverride),
                    Err(TokenError::Store) => return Err(ApiFailure::CredentialStore),
                }
            }
            let mut random = [0u8; 32];
            getrandom::fill(&mut random).map_err(|_| ApiFailure::CredentialStore)?;
            let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
            // Windows CredWrite replaces a credential atomically. Publish only after success;
            // a failed write must leave the previous credential and active token intact.
            store
                .set(local_api_token::ACCOUNT, &token)
                .map_err(|_| ApiFailure::CredentialStore)?;
            Ok(token)
        })
        .await
        .map_err(|_| ApiFailure::CredentialStore)?
    }

    async fn execute(&self, action: Option<ApiAction>, router: Router) -> ApiStatus {
        let mut lifecycle = self.lifecycle.lock().await;
        let acquiring = lifecycle.owner.is_none();
        if acquiring {
            match self.acquire_owner().await {
                Ok(owner) => lifecycle.owner = Some(owner),
                Err(error) => {
                    lifecycle.initialized = true;
                    let error = self.read_preference().await.err().unwrap_or(error);
                    self.fail(error, true).await;
                    return self.snapshot().await;
                }
            }
        }
        if !lifecycle.initialized || acquiring {
            if let Err(error) = self.read_preference().await {
                self.fail(error, true).await;
                return self.snapshot().await;
            }
            lifecycle.initialized = true;
        } else if action.is_none() {
            return self.snapshot().await;
        }

        let result = match action {
            Some(ApiAction::Disable) => {
                // Revoke before any fallible I/O. Even a failed preference write denies requests.
                {
                    let mut active = self.auth.0.write().await;
                    active.token = None;
                    active.status.listening = false;
                    active.generation += 1;
                }
                if let Some(server) = lifecycle.server.take() {
                    server.abort();
                    let _ = server.await;
                }
                match self.persist(false).await {
                    Ok(()) => {
                        let mut active = self.auth.0.write().await;
                        active.status.enabled = false;
                        active.status.error = None;
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
            Some(ApiAction::Rotate) => {
                let status = self.snapshot().await;
                if self.explicit.is_some() {
                    Err(ApiFailure::OverrideActive)
                } else if !status.enabled || !status.listening {
                    Err(ApiFailure::Disabled)
                } else {
                    match self.token(false, true).await {
                        Ok(token) => {
                            let mut active = self.auth.0.write().await;
                            active.token = Some(token);
                            active.status.error = None;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    }
                }
            }
            _ => {
                if action.is_none() && !self.snapshot().await.enabled {
                    return self.snapshot().await;
                }
                if self.snapshot().await.listening {
                    return self.snapshot().await;
                }
                self.enable(action.is_some(), router, &mut lifecycle).await
            }
        };
        if let Err(error) = result {
            self.fail(error, false).await;
        }
        self.snapshot().await
    }

    async fn enable(
        &self,
        explicit_enable: bool,
        router: Router,
        lifecycle: &mut Lifecycle,
    ) -> Result<(), ApiFailure> {
        let owner = lifecycle
            .owner
            .clone()
            .ok_or(ApiFailure::ControlsUnavailable)?;
        // Reserve the endpoint before generating credentials: a conflicting app
        // instance must not mutate the token used by the existing listener.
        let listener = TcpListener::bind(&self.address)
            .await
            .map_err(|_| ApiFailure::Bind)?;
        let token = self.token(explicit_enable, false).await?;
        if explicit_enable {
            self.persist(true).await?;
        }
        let generation = {
            let mut active = self.auth.0.write().await;
            active.generation += 1;
            active.token = Some(token);
            active.status = ApiStatus {
                enabled: true,
                listening: true,
                token_source: if self.explicit.is_some() {
                    TokenSource::Environment
                } else {
                    TokenSource::CredentialManager
                },
                error: None,
            };
            active.generation
        };
        let auth = self.auth.clone();
        lifecycle.server = Some(tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
            let mut active = auth.0.write().await;
            if active.generation == generation {
                active.token = None;
                active.status.listening = false;
                active.status.error = Some(ApiFailure::Server);
            }
            // Keep ownership until the detached listener has stopped and authorization is revoked.
            drop(owner);
        }));
        Ok(())
    }
}

#[cfg(test)]
#[path = "local_api_tests.rs"]
mod tests;
