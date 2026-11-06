//! Accounts and sign-in: offline accounts and Microsoft accounts.

pub mod msa;
pub mod offline;
pub mod store;

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::fsutil::write_atomic;
use crate::http::HttpClient;
use crate::{AppError, Paths, Result};
use store::SecretStore;

/// Base URLs of the Microsoft / Xbox / Minecraft sign-in services.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsaEndpoints {
    pub login: String,
    pub xbl: String,
    pub xsts: String,
    pub mc: String,
}

impl Default for MsaEndpoints {
    fn default() -> Self {
        Self {
            login: "https://login.microsoftonline.com".into(),
            xbl: "https://user.auth.xboxlive.com".into(),
            xsts: "https://xsts.auth.xboxlive.com".into(),
            mc: "https://api.minecraftservices.com".into(),
        }
    }
}

/// The Azure application id baked in at build time (`CAIRN_MSA_CLIENT_ID`).
pub fn msa_client_id() -> Option<&'static str> {
    option_env!("CAIRN_MSA_CLIENT_ID").filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AccountKind {
    Microsoft,
    Offline,
}

/// One entry of `accounts.json` — no secrets in here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Account {
    /// Player UUID with dashes.
    pub id: String,
    pub username: String,
    pub kind: AccountKind,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    #[ts(type = "number")]
    pub expires_in: u64,
    #[ts(type = "number")]
    pub interval: u64,
}

/// What the game needs to start as a player.
#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    pub username: String,
    /// UUID without dashes.
    pub uuid: String,
    pub access_token: String,
    /// "msa" or "legacy".
    pub user_type: String,
    pub xuid: Option<String>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &"***")
            .field("user_type", &self.user_type)
            .field("xuid", &self.xuid)
            .finish()
    }
}

#[derive(Clone)]
struct Cached {
    token: String,
    xuid: Option<String>,
    expires: Instant,
}

fn refresh_key(id: &str) -> String {
    format!("msa-refresh:{id}")
}

fn dashed(simple: &str) -> Result<String> {
    Uuid::parse_str(simple)
        .map(|u| u.hyphenated().to_string())
        .map_err(|e| AppError::new("error.parse").with_detail(e.to_string()))
}

#[derive(Clone)]
pub struct AuthService {
    paths: Paths,
    http: HttpClient,
    endpoints: MsaEndpoints,
    store: Arc<dyn SecretStore>,
    client_id: Option<String>,
    poll_interval: Option<Duration>,
    cache: Arc<Mutex<HashMap<String, Cached>>>,
    file_lock: Arc<Mutex<()>>,
}

/// A device-code sign-in waiting for the person to enter the code.
pub struct PendingLogin {
    service: AuthService,
    client_id: String,
    device_code: String,
    interval: Duration,
    deadline: Instant,
}

impl fmt::Debug for PendingLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingLogin")
            .field("device_code", &"***")
            .field("interval", &self.interval)
            .finish()
    }
}

impl PendingLogin {
    /// Poll until the person finishes signing in (or the code expires).
    pub async fn finish(self) -> Result<Account> {
        let s = &self.service;
        let mut interval = self.interval;
        let slow_step = if s.poll_interval.is_some() {
            self.interval
        } else {
            Duration::from_secs(5)
        };
        let tokens = loop {
            if Instant::now() > self.deadline {
                return Err(AppError::new("error.auth.expired"));
            }
            tokio::time::sleep(interval).await;
            match msa::poll_token(&s.http, &s.endpoints, &self.client_id, &self.device_code).await?
            {
                msa::Poll::Pending => {}
                msa::Poll::SlowDown => interval += slow_step,
                msa::Poll::Done(t) => break t,
            }
        };
        s.complete(tokens).await
    }
}

impl AuthService {
    pub fn new(
        paths: Paths,
        http: HttpClient,
        endpoints: MsaEndpoints,
        store: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            paths,
            http,
            endpoints,
            store,
            client_id: msa_client_id().map(str::to_string),
            poll_interval: None,
            cache: Arc::new(Mutex::new(HashMap::new())),
            file_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn with_client_id(mut self, id: Option<String>) -> Self {
        self.client_id = id;
        self
    }

    /// Override the device-code polling interval (tests).
    pub fn with_poll_interval(mut self, d: Duration) -> Self {
        self.poll_interval = Some(d);
        self
    }

    pub fn microsoft_configured(&self) -> bool {
        self.client_id.is_some()
    }

    pub fn accounts(&self) -> Result<Vec<Account>> {
        match std::fs::read(self.paths.accounts_file()) {
            Ok(b) => Ok(serde_json::from_slice(&b)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_accounts(&self, list: &[Account]) -> Result<()> {
        write_atomic(
            &self.paths.accounts_file(),
            &serde_json::to_vec_pretty(list)?,
        )
    }

    /// Insert or replace an account and make it the active one.
    fn upsert_active(&self, acc: Account) -> Result<Account> {
        let _g = self.file_lock.lock().unwrap();
        let mut list = self.accounts()?;
        list.retain(|a| a.id != acc.id);
        for a in &mut list {
            a.active = false;
        }
        let acc = Account {
            active: true,
            ..acc
        };
        list.push(acc.clone());
        self.save_accounts(&list)?;
        Ok(acc)
    }

    pub fn active(&self) -> Result<Option<Account>> {
        Ok(self.accounts()?.into_iter().find(|a| a.active))
    }

    pub async fn add_offline(&self, name: &str) -> Result<Account> {
        offline::validate_name(name)?;
        let id = offline::offline_uuid(name).hyphenated().to_string();
        self.upsert_active(Account {
            id,
            username: name.to_string(),
            kind: AccountKind::Offline,
            active: true,
        })
    }

    pub fn set_active(&self, id: &str) -> Result<()> {
        let _g = self.file_lock.lock().unwrap();
        let mut list = self.accounts()?;
        if !list.iter().any(|a| a.id == id) {
            return Err(AppError::new("error.auth.no_account"));
        }
        for a in &mut list {
            a.active = a.id == id;
        }
        self.save_accounts(&list)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let _g = self.file_lock.lock().unwrap();
        let mut list = self.accounts()?;
        list.retain(|a| a.id != id);
        if !list.is_empty() && !list.iter().any(|a| a.active) {
            list[0].active = true;
        }
        self.save_accounts(&list)?;
        self.cache.lock().unwrap().remove(id);
        self.store.delete(&refresh_key(id))
    }

    pub async fn start_device_login(&self) -> Result<(DeviceCode, PendingLogin)> {
        let client_id = self
            .client_id
            .clone()
            .ok_or_else(|| AppError::new("error.auth.not_configured"))?;
        let url = format!("{}/consumers/oauth2/v2.0/devicecode", self.endpoints.login);
        let (status, v) = msa::post_raw(
            &self.http,
            &url,
            Some(&[("client_id", client_id.as_str()), ("scope", msa::SCOPE)]),
            None,
        )
        .await?;
        if status != 200 {
            return Err(AppError::new("error.auth.failed")
                .with_detail(format!("devicecode: HTTP {status}")));
        }
        let r: msa::DeviceCodeResponse = serde_json::from_value(v)?;
        let code = DeviceCode {
            user_code: r.user_code,
            verification_uri: r.verification_uri,
            expires_in: r.expires_in,
            interval: r.interval,
        };
        let pending = PendingLogin {
            service: self.clone(),
            client_id,
            device_code: r.device_code,
            interval: self
                .poll_interval
                .unwrap_or(Duration::from_secs(r.interval.max(1))),
            deadline: Instant::now() + Duration::from_secs(r.expires_in),
        };
        Ok((code, pending))
    }

    fn remember(&self, id: &str, login: &msa::McLogin) {
        let ttl = Duration::from_secs(login.expires_in.saturating_sub(60));
        self.cache.lock().unwrap().insert(
            id.to_string(),
            Cached {
                token: login.access_token.clone(),
                xuid: login.xuid.clone(),
                expires: Instant::now() + ttl,
            },
        );
    }

    async fn complete(&self, tokens: msa::MsTokens) -> Result<Account> {
        let login = msa::minecraft_login(&self.http, &self.endpoints, &tokens.access_token).await?;
        let id = dashed(&login.uuid_simple)?;
        self.store.set(&refresh_key(&id), &tokens.refresh_token)?;
        self.remember(&id, &login);
        self.upsert_active(Account {
            id,
            username: login.username,
            kind: AccountKind::Microsoft,
            active: true,
        })
    }

    /// Exchange the stored refresh token for a fresh Minecraft token.
    async fn refresh_session(&self, id: &str) -> Result<Cached> {
        let client_id = self
            .client_id
            .clone()
            .ok_or_else(|| AppError::new("error.auth.not_configured"))?;
        let refresh = self
            .store
            .get(&refresh_key(id))?
            .ok_or_else(|| AppError::new("error.auth.relogin"))?;
        let tokens = msa::refresh(&self.http, &self.endpoints, &client_id, &refresh).await?;
        // Keep the rotated refresh token even if a later step loses the connection.
        self.store.set(&refresh_key(id), &tokens.refresh_token)?;
        let login = msa::minecraft_login(&self.http, &self.endpoints, &tokens.access_token)
            .await
            .map_err(|e| {
                if e.key == "error.network" {
                    e
                } else {
                    AppError::new("error.auth.relogin")
                        .with_detail(format!("{}: {}", e.key, e.detail))
                }
            })?;
        self.remember(id, &login);
        Ok(self
            .cache
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .expect("just cached"))
    }

    /// A session to launch with; refreshes the Microsoft sign-in when needed.
    pub async fn session(&self, id: &str) -> Result<Session> {
        let acc = self
            .accounts()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| AppError::new("error.auth.no_account"))?;
        let uuid = Uuid::parse_str(&acc.id)
            .map_err(|e| AppError::new("error.parse").with_detail(e.to_string()))?
            .simple()
            .to_string();
        if acc.kind == AccountKind::Offline {
            return Ok(Session {
                username: acc.username,
                uuid,
                access_token: "0".into(),
                user_type: "legacy".into(),
                xuid: None,
            });
        }
        let cached = self.cache.lock().unwrap().get(id).cloned();
        let cached = match cached {
            Some(c) if c.expires > Instant::now() => c,
            _ => match self.refresh_session(id).await {
                Ok(c) => c,
                // No network: play anyway with the cached profile, like the vanilla
                // launcher. Single-player works; online servers will refuse.
                Err(e) if e.key == "error.network" => {
                    tracing::warn!(
                        account = id,
                        "offline: starting without a Minecraft token ({})",
                        e.detail
                    );
                    return Ok(Session {
                        username: acc.username,
                        uuid,
                        access_token: "0".into(),
                        user_type: "msa".into(),
                        xuid: None,
                    });
                }
                Err(e) => return Err(e),
            },
        };
        Ok(Session {
            username: acc.username,
            uuid,
            access_token: cached.token,
            user_type: "msa".into(),
            xuid: cached.xuid,
        })
    }
}
