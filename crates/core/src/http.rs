//! Shared HTTP client: User-Agent, timeouts, retries, rate limits, response cache.

use std::path::Path;
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::fsutil::write_atomic;
use crate::{AppError, Result};

pub const USER_AGENT: &str = concat!(
    "Cairn/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/Yokiwa/cairn)"
);

/// Retries after the first attempt for 5xx, 429 and connection errors.
const RETRIES: u32 = 3;
/// Longest wait honoured from a `Retry-After` header.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    retry_base: Duration,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    fetched_at: u64,
    body: serde_json::Value,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn status_error(status: StatusCode, url: &str) -> AppError {
    AppError::new("error.http")
        .with_param("status", status.as_u16().to_string())
        .with_param("url", url.to_string())
        .with_detail(format!("HTTP {status} from {url}"))
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let inner = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            inner,
            retry_base: Duration::from_secs(1),
        })
    }

    /// Base wait between retries (1s, 2s, 4s by default). Tests shorten it.
    pub fn with_retry_base(mut self, d: Duration) -> Self {
        self.retry_base = d;
        self
    }

    pub fn inner(&self) -> &reqwest::Client {
        &self.inner
    }

    pub fn retry_base(&self) -> Duration {
        self.retry_base
    }

    async fn send(
        &self,
        url: &str,
        build: impl Fn(&reqwest::Client) -> RequestBuilder,
    ) -> Result<Response> {
        let mut attempt = 0;
        loop {
            let res = build(&self.inner).send().await;
            let wait = match res {
                Ok(r) if r.status().is_success() => return Ok(r),
                Ok(r) if r.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RETRIES => r
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|s| Duration::from_secs(s).min(MAX_RETRY_AFTER))
                    .unwrap_or(self.retry_base * 2u32.pow(attempt)),
                Ok(r) if r.status().is_server_error() && attempt < RETRIES => {
                    self.retry_base * 2u32.pow(attempt)
                }
                Ok(r) => return Err(status_error(r.status(), url)),
                Err(e)
                    if (e.is_connect() || e.is_timeout() || e.is_request())
                        && attempt < RETRIES =>
                {
                    self.retry_base * 2u32.pow(attempt)
                }
                Err(e) => return Err(e.into()),
            };
            tokio::time::sleep(wait).await;
            attempt += 1;
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let r = self.send(url, |c| c.get(url)).await?;
        let bytes = r.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Bytes> {
        Ok(self.send(url, |c| c.get(url)).await?.bytes().await?)
    }

    pub async fn post_form<T: DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<T> {
        let r = self.send(url, |c| c.post(url).form(form)).await?;
        Ok(serde_json::from_slice(&r.bytes().await?)?)
    }

    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let r = self.send(url, |c| c.post(url).json(body)).await?;
        Ok(serde_json::from_slice(&r.bytes().await?)?)
    }

    /// GET JSON through an on-disk cache. A fresh entry (younger than `ttl`)
    /// is served without a request; if the request fails, a stale entry is
    /// served instead so the launcher keeps working offline.
    pub async fn get_json_cached<T: DeserializeOwned>(
        &self,
        url: &str,
        cache_dir: &Path,
        ttl: Duration,
    ) -> Result<T> {
        let file = cache_dir.join("http").join(format!(
            "{}.json",
            hex::encode(Sha1::digest(url.as_bytes()))
        ));
        let cached: Option<CacheEntry> = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        if let Some(c) = &cached {
            if now_secs().saturating_sub(c.fetched_at) < ttl.as_secs() {
                return Ok(serde_json::from_value(c.body.clone())?);
            }
        }
        match self.get_json::<serde_json::Value>(url).await {
            Ok(body) => {
                let entry = CacheEntry {
                    fetched_at: now_secs(),
                    body,
                };
                write_atomic(&file, &serde_json::to_vec(&entry)?)?;
                Ok(serde_json::from_value(entry.body)?)
            }
            Err(e) => match cached {
                Some(c) => Ok(serde_json::from_value(c.body)?),
                None => Err(e),
            },
        }
    }
}
