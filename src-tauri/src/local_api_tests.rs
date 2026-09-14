use super::*;
use crate::{credentials::MemoryStore, error::AppError};
use axum::{middleware, routing::get};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct TestStore {
    memory: MemoryStore,
    fail_read: AtomicBool,
    fail_write: AtomicBool,
}
impl SecretStore for TestStore {
    fn get(&self, account: &str) -> Result<Option<String>, AppError> {
        if self.fail_read.load(Ordering::SeqCst) {
            return Err(AppError::Storage);
        }
        self.memory.get(account)
    }
    fn set(&self, account: &str, secret: &str) -> Result<(), AppError> {
        if self.fail_write.load(Ordering::SeqCst) {
            return Err(AppError::Storage);
        }
        self.memory.set(account, secret)
    }
    fn delete(&self, account: &str) -> Result<(), AppError> {
        self.memory.delete(account)
    }
}

fn router(api: &LocalApi) -> Router {
    Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            api.authorization(),
            crate::api::require_bearer_token,
        ))
}

fn setup(explicit: Option<OsString>) -> (tempfile::TempDir, Arc<TestStore>, Arc<LocalApi>) {
    let temp = tempfile::tempdir().expect("temporary database");
    let path = temp.path().join("ellie.sqlite3");
    storage::initialize(&path).expect("initialize");
    let store = Arc::new(TestStore::default());
    let api = LocalApi::new(path, store.clone(), explicit, "127.0.0.1:0");
    (temp, store, api)
}

async fn apply(api: &Arc<LocalApi>, action: Option<ApiAction>) -> ApiStatus {
    api.apply(action, router(api)).await
}

// An independent retained HTTP listener exercises the exact production middleware,
// including requests after disable (even connections that outlive listener shutdown).
async fn http_gate(api: &LocalApi) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let url = format!(
        "http://{}/api/v1/health",
        listener.local_addr().expect("local address")
    );
    let router = router(api);
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("test server");
    });
    (url, handle)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client")
}

#[tokio::test]
async fn disabled_override_does_not_read_store_or_authorize_and_disable_wins() {
    let (_temp, store, api) = setup(Some("test-only-override".into()));
    store.fail_read.store(true, Ordering::SeqCst);
    let status = apply(&api, None).await;
    assert!(!status.enabled && !status.listening && status.error.is_none());
    let (url, gate) = http_gate(&api).await;
    let client = client();
    assert_eq!(
        client
            .get(&url)
            .bearer_auth("test-only-override")
            .send()
            .await
            .expect("response")
            .status(),
        401
    );
    let enabled = apply(&api, Some(ApiAction::Enable)).await;
    assert!(enabled.enabled && enabled.listening);
    assert_eq!(enabled.token_source, TokenSource::Environment);
    assert_eq!(
        client
            .get(&url)
            .bearer_auth("test-only-override")
            .send()
            .await
            .expect("response")
            .status(),
        200
    );
    assert_eq!(
        apply(&api, Some(ApiAction::Rotate)).await.error,
        Some(ApiFailure::OverrideActive)
    );
    let disabled = apply(&api, Some(ApiAction::Disable)).await;
    assert!(!disabled.enabled && !disabled.listening);
    assert_eq!(
        client
            .get(&url)
            .bearer_auth("test-only-override")
            .send()
            .await
            .expect("response")
            .status(),
        401
    );
    assert!(!storage::read_local_api_enabled(&api.path).expect("preference"));
    gate.abort();
}

#[tokio::test]
async fn enable_restart_rotate_and_http_secret_exclusion() {
    let (_temp, store, api) = setup(None);
    assert!(apply(&api, Some(ApiAction::Enable)).await.listening);
    let old = store
        .get(local_api_token::ACCOUNT)
        .expect("read mock")
        .expect("token");
    assert!(old.len() == 64 && old.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(storage::read_local_api_enabled(&api.path).expect("enabled persisted"));
    let path = api.path.clone();
    // Simulate process shutdown without changing the enabled preference.
    {
        let mut lifecycle = api.lifecycle.lock().await;
        api.fail(ApiFailure::Server, true).await;
        if let Some(server) = lifecycle.server.take() {
            server.abort();
            let _ = server.await;
        }
    }
    drop(api);
    let api = LocalApi::new(path, store.clone(), None, "127.0.0.1:0");
    assert!(apply(&api, None).await.listening);
    assert!(api.authorization().accepts(Some(&old)).await);
    let (url, gate) = http_gate(&api).await;
    let client = client();
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&old)
            .send()
            .await
            .expect("response")
            .status(),
        200
    );
    assert!(apply(&api, Some(ApiAction::Rotate)).await.listening);
    let new = local_api_token::resolve(None, || {
        store
            .get(local_api_token::ACCOUNT)
            .map_err(|_| TokenError::Store)
    })
    .expect("next CLI-style resolution");
    assert!(old != new);
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&old)
            .send()
            .await
            .expect("response")
            .status(),
        401
    );
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&new)
            .send()
            .await
            .expect("response")
            .status(),
        200
    );
    for headers in [
        [("origin", "null")],
        [("origin", "http://localhost")],
        [("sec-fetch-site", "same-origin")],
    ] {
        let (name, value) = headers[0];
        let response = client
            .get(&url)
            .bearer_auth(&new)
            .header(name, value)
            .send()
            .await
            .expect("response");
        assert_eq!(response.status(), 403);
        let body = response.text().await.expect("body");
        assert!(!body.contains(&old) && !body.contains(&new));
    }
    assert_eq!(
        client.get(&url).send().await.expect("response").status(),
        401
    );
    let status = serde_json::to_string(&api.status().await).expect("serialize status");
    assert!(!status.contains(&old) && !status.contains(&new));
    assert_eq!(
        serde_json::to_value(api.status().await)
            .expect("serialize")
            .as_object()
            .expect("object")
            .len(),
        4
    );
    let database = std::fs::read(&api.path).expect("read temporary database");
    for token in [&old, &new] {
        assert!(!database
            .windows(token.len())
            .any(|bytes| bytes == token.as_bytes()));
    }
    apply(&api, Some(ApiAction::Disable)).await;
    gate.abort();
}

#[tokio::test]
async fn rotation_write_failure_preserves_old_credential_and_http_authorization() {
    let (_temp, store, api) = setup(None);
    apply(&api, Some(ApiAction::Enable)).await;
    let old = store
        .get(local_api_token::ACCOUNT)
        .expect("mock read")
        .expect("token");
    store.fail_write.store(true, Ordering::SeqCst);
    let status = apply(&api, Some(ApiAction::Rotate)).await;
    assert_eq!(status.error, Some(ApiFailure::CredentialStore));
    assert!(status.enabled && status.listening);
    assert!(
        store
            .get(local_api_token::ACCOUNT)
            .expect("mock read")
            .as_deref()
            == Some(&old)
    );
    let (url, gate) = http_gate(&api).await;
    assert_eq!(
        client()
            .get(&url)
            .bearer_auth(&old)
            .send()
            .await
            .expect("response")
            .status(),
        200
    );
    apply(&api, Some(ApiAction::Disable)).await;
    gate.abort();
}

#[tokio::test]
async fn missing_startup_store_errors_and_invalid_overrides_fail_closed() {
    let (_temp, store, api) = setup(None);
    storage::save_local_api_enabled(&api.path, true).expect("preference");
    assert_eq!(
        apply(&api, None).await.error,
        Some(ApiFailure::MissingToken)
    );
    assert!(store
        .get(local_api_token::ACCOUNT)
        .expect("mock read")
        .is_none());
    store.fail_read.store(true, Ordering::SeqCst);
    assert_eq!(
        apply(&api, Some(ApiAction::Enable)).await.error,
        Some(ApiFailure::CredentialStore)
    );
    store.fail_read.store(false, Ordering::SeqCst);
    store.fail_write.store(true, Ordering::SeqCst);
    assert_eq!(
        apply(&api, Some(ApiAction::Enable)).await.error,
        Some(ApiFailure::CredentialStore)
    );
    assert!(!api.status().await.listening);
    store.fail_write.store(false, Ordering::SeqCst);
    store
        .set(local_api_token::ACCOUNT, "test-only-stored")
        .expect("mock write");
    let path = api.path.clone();
    drop(api);
    for explicit in ["", " ", "invalid\nheader"] {
        let invalid = LocalApi::new(
            path.clone(),
            store.clone(),
            Some(explicit.into()),
            "127.0.0.1:0",
        );
        assert_eq!(
            apply(&invalid, None).await.error,
            Some(ApiFailure::InvalidOverride)
        );
        assert!(
            !invalid
                .authorization()
                .accepts(Some("test-only-stored"))
                .await
        );
    }
}

#[tokio::test]
async fn persistence_failure_never_enables_and_failed_disable_revokes() {
    let (_temp, store, api) = setup(None);
    apply(&api, None).await;
    // A constraint failure leaves the previously valid persisted value unchanged.
    let connection = storage::connect(&api.path).expect("database");
    connection.execute_batch("CREATE TRIGGER fail_api_update BEFORE UPDATE OF local_api_enabled ON application_settings BEGIN SELECT RAISE(ABORT, 'test failure'); END;").expect("failure injection");
    assert_eq!(
        apply(&api, Some(ApiAction::Enable)).await.error,
        Some(ApiFailure::Persistence)
    );
    assert!(!api.status().await.listening);
    assert!(!storage::read_local_api_enabled(&api.path).expect("preference"));
    connection
        .execute_batch("DROP TRIGGER fail_api_update;")
        .expect("remove injection");
    assert!(apply(&api, Some(ApiAction::Enable)).await.listening);
    let token = store
        .get(local_api_token::ACCOUNT)
        .expect("mock read")
        .expect("token");
    connection.execute_batch("CREATE TRIGGER fail_api_update BEFORE UPDATE OF local_api_enabled ON application_settings BEGIN SELECT RAISE(ABORT, 'test failure'); END;").expect("failure injection");
    let failed = apply(&api, Some(ApiAction::Disable)).await;
    assert_eq!(failed.error, Some(ApiFailure::Persistence));
    assert!(failed.enabled && !failed.listening);
    assert!(!api.authorization().accepts(Some(&token)).await);
}

#[tokio::test]
async fn reports_bind_failure_instead_of_claiming_listening() {
    let (_temp, store, api) = setup(None);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("occupy port");
    let blocked = LocalApi::new(
        api.path.clone(),
        store,
        None,
        &listener.local_addr().expect("address").to_string(),
    );
    let status = apply(&blocked, Some(ApiAction::Enable)).await;
    assert_eq!(status.error, Some(ApiFailure::Bind));
    assert!(!status.enabled && !status.listening);
    let disabled = apply(&blocked, Some(ApiAction::Disable)).await;
    assert!(!disabled.enabled && !disabled.listening && disabled.error.is_none());
    assert!(!storage::read_local_api_enabled(&blocked.path).expect("owner disabled preference"));
    drop(listener);
    assert!(apply(&blocked, Some(ApiAction::Enable)).await.listening);
    let token = blocked
        .store
        .get(local_api_token::ACCOUNT)
        .expect("mock read")
        .expect("token");
    let url = format!("http://{}/api/v1/health", blocked.address);
    assert_eq!(
        client()
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .expect("real bound listener")
            .status(),
        200
    );
    apply(&blocked, Some(ApiAction::Disable)).await;
    assert!(client().get(&url).bearer_auth(&token).send().await.is_err());
}

#[tokio::test]
async fn concurrent_controls_serialize_and_end_with_matching_persisted_authorization() {
    let (_temp, store, api) = setup(None);
    let (a, b, c) = tokio::join!(
        apply(&api, Some(ApiAction::Enable)),
        apply(&api, Some(ApiAction::Rotate)),
        apply(&api, Some(ApiAction::Disable))
    );
    assert!(a.error.is_none());
    assert!(b.error.is_none() || b.error == Some(ApiFailure::Disabled));
    assert!(c.error.is_none());
    let status = api.status().await;
    assert_eq!(
        status.enabled,
        storage::read_local_api_enabled(&api.path).expect("preference")
    );
    let token = store.get(local_api_token::ACCOUNT).expect("mock read");
    assert_eq!(
        api.authorization().accepts(token.as_deref()).await,
        status.enabled && status.listening
    );
    apply(&api, Some(ApiAction::Disable)).await;
}

#[test]
fn token_comparison_rejects_prefix_and_suffix() {
    assert!(constant_time_equal(b"test-token", b"test-token"));
    assert!(!constant_time_equal(b"test-token", b"test-token-2"));
    assert!(!constant_time_equal(b"test-token", b"test"));
}

#[tokio::test]
async fn non_owner_cannot_disable_shared_database_listener_and_lock_can_be_reacquired() {
    let (_temp, store, initial) = setup(None);
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("select test port");
    let address = port.local_addr().expect("test address").to_string();
    drop(port);
    let owner = LocalApi::new(initial.path.clone(), store.clone(), None, &address);
    assert!(apply(&owner, Some(ApiAction::Enable)).await.listening);
    let token = store
        .get(local_api_token::ACCOUNT)
        .expect("mock read")
        .expect("token");
    let url = format!("http://{address}/api/v1/health");
    let client = client();
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .expect("owner response")
            .status(),
        200
    );

    let secondary = LocalApi::new(initial.path.clone(), store.clone(), None, &address);
    let status = apply(&secondary, None).await;
    assert!(status.enabled && !status.listening);
    assert_eq!(status.error, Some(ApiFailure::ControlsUnavailable));
    for action in [ApiAction::Disable, ApiAction::Enable, ApiAction::Rotate] {
        let rejected = apply(&secondary, Some(action)).await;
        assert!(rejected.enabled && !rejected.listening);
        assert_eq!(rejected.error, Some(ApiFailure::ControlsUnavailable));
        assert!(storage::read_local_api_enabled(&initial.path).expect("unchanged preference"));
        let status = owner.status().await;
        assert!(status.enabled && status.listening && status.error.is_none());
        assert_eq!(
            client
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .expect("owner still authorizes")
                .status(),
            200
        );
    }

    let disabled = apply(&owner, Some(ApiAction::Disable)).await;
    assert!(!disabled.enabled && !disabled.listening && disabled.error.is_none());
    assert!(!owner.authorization().accepts(Some(&token)).await);
    assert!(!storage::read_local_api_enabled(&initial.path).expect("owner saved disable"));
    let status = secondary.status().await;
    assert!(!status.enabled && !status.listening);
    assert_eq!(status.error, Some(ApiFailure::ControlsUnavailable));
    // Stopping the listener does not transfer control while its controller remains alive.
    assert_eq!(
        apply(&secondary, Some(ApiAction::Enable)).await.error,
        Some(ApiFailure::ControlsUnavailable)
    );
    assert_eq!(
        std::fs::metadata(
            initial
                .path
                .parent()
                .expect("directory")
                .join("local-api.lock")
        )
        .expect("lock file")
        .len(),
        0
    );

    drop(owner);
    let enabled = apply(&secondary, Some(ApiAction::Enable)).await;
    assert!(enabled.enabled && enabled.listening && enabled.error.is_none());
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .expect("new owner response")
            .status(),
        200
    );
    apply(&secondary, Some(ApiAction::Disable)).await;
}
