//! Microsoft → Xbox Live → XSTS → Minecraft token chain.

use serde::Deserialize;
use serde_json::{json, Value};

use super::MsaEndpoints;
use crate::http::HttpClient;
use crate::{AppError, Result};

pub const SCOPE: &str = "XboxLive.signin offline_access";
pub const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}
fn default_interval() -> u64 {
    5
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// What a finished chain yields.
#[derive(Debug, Clone)]
pub struct McLogin {
    pub access_token: String,
    pub expires_in: u64,
    pub uuid_simple: String,
    pub username: String,
    pub xuid: Option<String>,
}

/// POST and return (status, JSON body) without failing on 4xx — these APIs
/// put the interesting error inside the body.
pub async fn post_raw(
    http: &HttpClient,
    url: &str,
    form: Option<&[(&str, &str)]>,
    body: Option<&Value>,
) -> Result<(u16, Value)> {
    let mut req = http.inner().post(url).header("Accept", "application/json");
    if let Some(f) = form {
        req = req.form(f);
    }
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await?;
    let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Ok((status, v))
}

pub enum Poll {
    Pending,
    SlowDown,
    Done(MsTokens),
}

pub async fn poll_token(
    http: &HttpClient,
    ep: &MsaEndpoints,
    client_id: &str,
    device_code: &str,
) -> Result<Poll> {
    let url = format!("{}/consumers/oauth2/v2.0/token", ep.login);
    let form = [
        ("grant_type", DEVICE_GRANT),
        ("client_id", client_id),
        ("device_code", device_code),
    ];
    let (status, v) = post_raw(http, &url, Some(&form), None).await?;
    if status == 200 {
        return Ok(Poll::Done(serde_json::from_value(v)?));
    }
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => Ok(Poll::Pending),
        Some("slow_down") => Ok(Poll::SlowDown),
        Some("expired_token") => Err(AppError::new("error.auth.expired")),
        Some("authorization_declined") => Err(AppError::new("error.auth.declined")),
        other => Err(AppError::new("error.auth.failed")
            .with_detail(format!("token endpoint: HTTP {status} {other:?}"))),
    }
}

pub async fn refresh(
    http: &HttpClient,
    ep: &MsaEndpoints,
    client_id: &str,
    refresh_token: &str,
) -> Result<MsTokens> {
    let url = format!("{}/consumers/oauth2/v2.0/token", ep.login);
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("refresh_token", refresh_token),
        ("scope", SCOPE),
    ];
    let (status, v) = post_raw(http, &url, Some(&form), None).await?;
    if status != 200 {
        return Err(
            AppError::new("error.auth.relogin").with_detail(format!("refresh: HTTP {status}"))
        );
    }
    Ok(serde_json::from_value(v)?)
}

fn str_at<'a>(v: &'a Value, ptr: &str) -> Option<&'a str> {
    v.pointer(ptr).and_then(Value::as_str)
}

/// Exchange a Microsoft access token for a Minecraft session.
pub async fn minecraft_login(
    http: &HttpClient,
    ep: &MsaEndpoints,
    ms_access: &str,
) -> Result<McLogin> {
    let failed = |stage: &str, status: u16| {
        AppError::new("error.auth.failed").with_detail(format!("{stage}: HTTP {status}"))
    };

    let body = json!({
        "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")},
        "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"
    });
    let (status, xbl) = post_raw(
        http,
        &format!("{}/user/authenticate", ep.xbl),
        None,
        Some(&body),
    )
    .await?;
    let xbl_token = str_at(&xbl, "/Token")
        .filter(|_| status == 200)
        .ok_or_else(|| failed("xbox live", status))?;

    let body = json!({
        "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
        "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"
    });
    let (status, xsts) = post_raw(
        http,
        &format!("{}/xsts/authorize", ep.xsts),
        None,
        Some(&body),
    )
    .await?;
    if status != 200 {
        return Err(match xsts.get("XErr").and_then(Value::as_u64) {
            Some(2148916233) => AppError::new("error.auth.no_xbox"),
            Some(2148916235) => AppError::new("error.auth.region"),
            Some(2148916236) | Some(2148916237) => AppError::new("error.auth.adult_verification"),
            Some(2148916238) => AppError::new("error.auth.child_account"),
            _ => failed("xsts", status),
        });
    }
    let xsts_token = str_at(&xsts, "/Token").ok_or_else(|| failed("xsts", status))?;
    let uhs =
        str_at(&xsts, "/DisplayClaims/xui/0/uhs").ok_or_else(|| failed("xsts uhs", status))?;
    let xuid = str_at(&xsts, "/DisplayClaims/xui/0/xid").map(str::to_string);

    let body = json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")});
    let (status, mc) = post_raw(
        http,
        &format!("{}/authentication/login_with_xbox", ep.mc),
        None,
        Some(&body),
    )
    .await?;
    let access = str_at(&mc, "/access_token")
        .filter(|_| status == 200)
        .ok_or_else(|| failed("minecraft login", status))?
        .to_string();
    let expires_in = mc.get("expires_in").and_then(Value::as_u64).unwrap_or(0);

    let resp = http
        .inner()
        .get(format!("{}/minecraft/profile", ep.mc))
        .bearer_auth(&access)
        .send()
        .await?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Err(AppError::new("error.auth.no_game"));
    }
    let profile: Value = serde_json::from_slice(&resp.bytes().await?).unwrap_or(Value::Null);
    let (Some(id), Some(name)) = (str_at(&profile, "/id"), str_at(&profile, "/name")) else {
        return Err(failed("profile", status));
    };
    Ok(McLogin {
        access_token: access,
        expires_in,
        uuid_simple: id.to_string(),
        username: name.to_string(),
        xuid,
    })
}
