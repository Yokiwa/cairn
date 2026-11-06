use std::sync::{Arc, Mutex};
use std::time::Duration;

use cairn_core::auth::store::{MemoryStore, SecretStore};
use cairn_core::auth::{AccountKind, AuthService, MsaEndpoints};
use cairn_core::http::HttpClient;
use cairn_core::Paths;
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

const UUID_SIMPLE: &str = "069a79f444e94726a5befca90e38aaf5";

struct Sequence(Arc<Mutex<Vec<ResponseTemplate>>>);
impl Respond for Sequence {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let mut v = self.0.lock().unwrap();
        if v.len() > 1 {
            v.remove(0)
        } else {
            v[0].clone()
        }
    }
}

async fn mount_chain(
    server: &MockServer,
    mc_expires: u64,
    profile_status: u16,
    xsts_err: Option<u64>,
) {
    Mock::given(method("POST")).and(path("/consumers/oauth2/v2.0/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"device_code": "dev-123", "user_code": "ABCD-EFGH", "verification_uri": "https://microsoft.com/link", "expires_in": 900, "interval": 5, "message": "go"})))
        .mount(server).await;
    let seq = vec![
        ResponseTemplate::new(400).set_body_json(json!({"error": "authorization_pending"})),
        ResponseTemplate::new(400).set_body_json(json!({"error": "slow_down"})),
        ResponseTemplate::new(200).set_body_json(json!({"access_token": "ms-access", "refresh_token": "ms-refresh-1", "expires_in": 3600})),
    ];
    Mock::given(method("POST"))
        .and(path("/consumers/oauth2/v2.0/token"))
        .and(body_string_contains("device_code"))
        .respond_with(Sequence(Arc::new(Mutex::new(seq))))
        .mount(server)
        .await;
    Mock::given(method("POST")).and(path("/consumers/oauth2/v2.0/token")).and(body_string_contains("grant_type=refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token": "ms-access-2", "refresh_token": "ms-refresh-2", "expires_in": 3600})))
        .mount(server).await;
    Mock::given(method("POST"))
        .and(path("/user/authenticate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"Token": "xbl-token", "DisplayClaims": {"xui": [{"uhs": "user-hash"}]}}),
        ))
        .mount(server)
        .await;
    let xsts = match xsts_err {
        Some(code) => ResponseTemplate::new(401).set_body_json(json!({"XErr": code, "Message": ""})),
        None => ResponseTemplate::new(200).set_body_json(json!({"Token": "xsts-token", "DisplayClaims": {"xui": [{"uhs": "user-hash", "xid": "2535"}]}})),
    };
    Mock::given(method("POST"))
        .and(path("/xsts/authorize"))
        .respond_with(xsts)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/authentication/login_with_xbox"))
        .and(body_string_contains("XBL3.0 x=user-hash;xsts-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": "mc-token", "expires_in": mc_expires})),
        )
        .mount(server)
        .await;
    let profile = if profile_status == 200 {
        ResponseTemplate::new(200).set_body_json(json!({"id": UUID_SIMPLE, "name": "Notch"}))
    } else {
        ResponseTemplate::new(profile_status).set_body_json(json!({"error": "NOT_FOUND"}))
    };
    Mock::given(method("GET"))
        .and(path("/minecraft/profile"))
        .respond_with(profile)
        .mount(server)
        .await;
}

fn service(server: &MockServer, dir: &std::path::Path, store: Arc<MemoryStore>) -> AuthService {
    let b = server.uri();
    let ep = MsaEndpoints {
        login: b.clone(),
        xbl: b.clone(),
        xsts: b.clone(),
        mc: b,
    };
    AuthService::new(
        Paths::new(dir.into()),
        HttpClient::new().unwrap(),
        ep,
        store,
    )
    .with_client_id(Some("test-client".into()))
    .with_poll_interval(Duration::from_millis(1))
}

#[tokio::test]
async fn device_login_full_chain() {
    let server = MockServer::start().await;
    mount_chain(&server, 86400, 200, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let auth = service(&server, dir.path(), store.clone());
    let (code, pending) = auth.start_device_login().await.unwrap();
    assert_eq!(
        (code.user_code.as_str(), code.verification_uri.as_str()),
        ("ABCD-EFGH", "https://microsoft.com/link")
    );
    let acc = pending.finish().await.unwrap();
    assert_eq!(acc.kind, AccountKind::Microsoft);
    assert_eq!(acc.username, "Notch");
    assert_eq!(acc.id, "069a79f4-44e9-4726-a5be-fca90e38aaf5");
    assert!(acc.active);
    assert_eq!(
        store
            .get(&format!("msa-refresh:{}", acc.id))
            .unwrap()
            .as_deref(),
        Some("ms-refresh-1")
    );
    let s = auth.session(&acc.id).await.unwrap();
    assert_eq!(
        (
            s.access_token.as_str(),
            s.user_type.as_str(),
            s.uuid.as_str()
        ),
        ("mc-token", "msa", UUID_SIMPLE)
    );
    assert_eq!(s.xuid.as_deref(), Some("2535"));
    assert_eq!(auth.accounts().unwrap().len(), 1);
    // Nothing secret on disk.
    let on_disk = std::fs::read_to_string(dir.path().join("accounts.json")).unwrap();
    assert!(!on_disk.contains("ms-refresh") && !on_disk.contains("mc-token"));
}

#[tokio::test]
async fn xsts_no_xbox_account_error() {
    let server = MockServer::start().await;
    mount_chain(&server, 86400, 200, Some(2148916233)).await;
    let dir = tempfile::tempdir().unwrap();
    let auth = service(&server, dir.path(), Arc::new(MemoryStore::default()));
    let (_, pending) = auth.start_device_login().await.unwrap();
    assert_eq!(
        pending.finish().await.unwrap_err().key,
        "error.auth.no_xbox"
    );
}

#[tokio::test]
async fn xsts_child_account_error() {
    let server = MockServer::start().await;
    mount_chain(&server, 86400, 200, Some(2148916238)).await;
    let dir = tempfile::tempdir().unwrap();
    let auth = service(&server, dir.path(), Arc::new(MemoryStore::default()));
    let (_, pending) = auth.start_device_login().await.unwrap();
    assert_eq!(
        pending.finish().await.unwrap_err().key,
        "error.auth.child_account"
    );
}

#[tokio::test]
async fn profile_404_means_no_game() {
    let server = MockServer::start().await;
    mount_chain(&server, 86400, 404, None).await;
    let dir = tempfile::tempdir().unwrap();
    let auth = service(&server, dir.path(), Arc::new(MemoryStore::default()));
    let (_, pending) = auth.start_device_login().await.unwrap();
    assert_eq!(
        pending.finish().await.unwrap_err().key,
        "error.auth.no_game"
    );
    assert!(auth.accounts().unwrap().is_empty());
}

#[tokio::test]
async fn session_refreshes_expired_token() {
    let server = MockServer::start().await;
    mount_chain(&server, 0, 200, None).await; // minecraft token expires immediately
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let auth = service(&server, dir.path(), store.clone());
    let (_, pending) = auth.start_device_login().await.unwrap();
    let acc = pending.finish().await.unwrap();
    auth.session(&acc.id).await.unwrap();
    assert_eq!(
        store
            .get(&format!("msa-refresh:{}", acc.id))
            .unwrap()
            .as_deref(),
        Some("ms-refresh-2"),
        "refresh token rotated"
    );
}

#[tokio::test]
async fn missing_refresh_token_asks_to_relogin() {
    let server = MockServer::start().await;
    mount_chain(&server, 0, 200, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let auth = service(&server, dir.path(), store.clone());
    let (_, pending) = auth.start_device_login().await.unwrap();
    let acc = pending.finish().await.unwrap();
    store.delete(&format!("msa-refresh:{}", acc.id)).unwrap();
    let fresh = service(&server, dir.path(), store); // new process: no cached token
    assert_eq!(
        fresh.session(&acc.id).await.unwrap_err().key,
        "error.auth.relogin"
    );
}

#[tokio::test]
async fn not_configured_without_client_id() {
    let dir = tempfile::tempdir().unwrap();
    let auth = AuthService::new(
        Paths::new(dir.path().into()),
        HttpClient::new().unwrap(),
        MsaEndpoints::default(),
        Arc::new(MemoryStore::default()),
    )
    .with_client_id(None);
    assert_eq!(
        auth.start_device_login().await.unwrap_err().key,
        "error.auth.not_configured"
    );
}

#[tokio::test]
async fn offline_accounts_and_active_switching() {
    let dir = tempfile::tempdir().unwrap();
    let auth = AuthService::new(
        Paths::new(dir.path().into()),
        HttpClient::new().unwrap(),
        MsaEndpoints::default(),
        Arc::new(MemoryStore::default()),
    );
    let a = auth.add_offline("Alex").await.unwrap();
    let b = auth.add_offline("Steve_2").await.unwrap();
    assert_eq!(a.kind, AccountKind::Offline);
    let list = auth.accounts().unwrap();
    assert_eq!(list.iter().filter(|x| x.active).count(), 1);
    assert_eq!(
        auth.active().unwrap().unwrap().id,
        b.id,
        "newest account becomes active"
    );
    auth.set_active(&a.id).unwrap();
    assert_eq!(auth.active().unwrap().unwrap().id, a.id);
    let s = auth.session(&a.id).await.unwrap();
    assert_eq!(
        (s.username.as_str(), s.user_type.as_str()),
        ("Alex", "legacy")
    );
    assert_eq!(
        s.uuid,
        cairn_core::auth::offline::offline_uuid("Alex")
            .simple()
            .to_string()
    );
    auth.remove(&a.id).unwrap();
    let list = auth.accounts().unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].active, "remaining account becomes active");
    assert_eq!(
        auth.add_offline("Steve_2").await.unwrap().id,
        b.id,
        "adding the same name again is idempotent"
    );
    assert_eq!(auth.accounts().unwrap().len(), 1);
}

#[test]
fn session_debug_hides_token() {
    let s = cairn_core::auth::Session {
        username: "a".into(),
        uuid: "u".into(),
        access_token: "secret-token".into(),
        user_type: "msa".into(),
        xuid: None,
    };
    assert!(!format!("{s:?}").contains("secret-token"));
}

#[tokio::test]
async fn microsoft_account_plays_offline_when_network_is_down() {
    let server = MockServer::start().await;
    mount_chain(&server, 0, 200, None).await; // token already expired → would need a refresh
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let auth = service(&server, dir.path(), store.clone());
    let (_, pending) = auth.start_device_login().await.unwrap();
    let acc = pending.finish().await.unwrap();
    // Restart the launcher with no network: nothing listens on port 9.
    let dead = "http://127.0.0.1:9".to_string();
    let ep = MsaEndpoints { login: dead.clone(), xbl: dead.clone(), xsts: dead.clone(), mc: dead };
    let offline = AuthService::new(Paths::new(dir.path().into()), HttpClient::new().unwrap(), ep, store.clone()).with_client_id(Some("test-client".into()));
    let s = offline.session(&acc.id).await.unwrap();
    assert_eq!(
        (s.username.as_str(), s.uuid.as_str()),
        ("Notch", UUID_SIMPLE)
    );
    assert_eq!(
        s.access_token, "0",
        "no valid token offline; single-player still works"
    );
    assert_eq!(
        store
            .get(&format!("msa-refresh:{}", acc.id))
            .unwrap()
            .as_deref(),
        Some("ms-refresh-1"),
        "refresh token kept for later"
    );
}
