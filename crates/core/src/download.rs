//! Parallel, resumable, verified downloads (spec section 4, "下载引擎").

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{StreamExt, TryStreamExt};
use reqwest::StatusCode;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::http::{status_error, HttpClient};
use crate::tasks::cancelled;
pub use crate::tasks::{Progress, ProgressFn};
use crate::{AppError, Result};

/// Retries after the first attempt.
const RETRIES: u32 = 3;
const REPORT_EVERY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone)]
pub struct Downloader {
    http: HttpClient,
    concurrency: usize,
    retry_base: Duration,
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// Is an existing file at `dest` already the one we want?
fn already_good(item: &DownloadItem) -> Result<bool> {
    let meta = match std::fs::metadata(&item.dest) {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };
    if let Some(want) = &item.sha1 {
        return Ok(sha1_file(&item.dest)?.eq_ignore_ascii_case(want));
    }
    if let Some(size) = item.size {
        return Ok(meta.len() == size);
    }
    Ok(true)
}

struct Counters {
    done_files: AtomicU64,
    done_bytes: AtomicU64,
}

enum Attempt {
    Done,
    Retry(AppError),
}

impl Downloader {
    pub fn new(http: HttpClient, concurrency: usize) -> Self {
        Self {
            http,
            concurrency: concurrency.clamp(1, 64),
            retry_base: Duration::from_secs(1),
        }
    }

    pub fn with_retry_base(mut self, d: Duration) -> Self {
        self.retry_base = d;
        self
    }

    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.clamp(1, 64);
        self
    }

    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    pub async fn download_all(
        &self,
        stage: &str,
        items: Vec<DownloadItem>,
        progress: ProgressFn,
        cancel: CancellationToken,
    ) -> Result<()> {
        let total_files = items.len() as u64;
        let total_bytes: u64 = items.iter().filter_map(|i| i.size).sum();
        let counters = Arc::new(Counters {
            done_files: AtomicU64::new(0),
            done_bytes: AtomicU64::new(0),
        });

        let snapshot = {
            let counters = counters.clone();
            let stage = stage.to_string();
            move |bytes_per_sec: u64| Progress {
                stage: stage.clone(),
                done_files: counters.done_files.load(Ordering::Relaxed),
                total_files,
                done_bytes: counters.done_bytes.load(Ordering::Relaxed),
                total_bytes,
                bytes_per_sec,
            }
        };

        // Reporter: emits progress every REPORT_EVERY with a speed estimate.
        let stop = CancellationToken::new();
        let reporter = {
            let stop = stop.clone();
            let progress = progress.clone();
            let snapshot = snapshot.clone();
            let counters = counters.clone();
            tokio::spawn(async move {
                let mut last = (Instant::now(), 0u64);
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = tokio::time::sleep(REPORT_EVERY) => {}
                    }
                    let bytes = counters.done_bytes.load(Ordering::Relaxed);
                    let dt = last.0.elapsed().as_secs_f64().max(0.001);
                    let speed = ((bytes.saturating_sub(last.1)) as f64 / dt) as u64;
                    last = (Instant::now(), bytes);
                    progress(&snapshot(speed));
                }
            })
        };

        let result = futures::stream::iter(items.into_iter().map(|item| {
            let counters = counters.clone();
            let cancel = cancel.clone();
            async move { self.download_one(item, &counters, &cancel).await }
        }))
        .buffer_unordered(self.concurrency)
        .try_collect::<Vec<()>>()
        .await;

        stop.cancel();
        let _ = reporter.await;
        progress(&snapshot(0));
        result.map(|_| ())
    }

    async fn download_one(
        &self,
        item: DownloadItem,
        counters: &Counters,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        if already_good(&item)? {
            counters.done_files.fetch_add(1, Ordering::Relaxed);
            counters
                .done_bytes
                .fetch_add(item.size.unwrap_or(0), Ordering::Relaxed);
            return Ok(());
        }
        if let Some(parent) = item.dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut last_err = AppError::new("error.download.failed");
        for attempt in 0..=RETRIES {
            if attempt > 0 {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(cancelled()),
                    _ = tokio::time::sleep(self.retry_base * 2u32.pow(attempt - 1)) => {}
                }
            }
            let before = counters.done_bytes.load(Ordering::Relaxed);
            match self.attempt(&item, counters, cancel).await {
                Ok(Attempt::Done) => {
                    counters.done_files.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
                Ok(Attempt::Retry(e)) => last_err = e,
                Err(e) if e.key == "error.cancelled" => return Err(e),
                Err(e) => last_err = e,
            }
            // Bytes from a failed attempt don't count.
            let after = counters.done_bytes.load(Ordering::Relaxed);
            counters
                .done_bytes
                .fetch_sub(after.saturating_sub(before), Ordering::Relaxed);
        }
        Err(last_err.with_param(
            "file",
            item.dest
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        ))
    }

    async fn attempt(
        &self,
        item: &DownloadItem,
        counters: &Counters,
        cancel: &CancellationToken,
    ) -> Result<Attempt> {
        let part = part_path(&item.dest);
        let offset = tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let mut req = self.http.inner().get(&item.url);
        if offset > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Err(cancelled()),
            r = req.send() => r,
        };
        let resp = match resp {
            Ok(r) => r,
            Err(e) => return Ok(Attempt::Retry(e.into())),
        };
        let status = resp.status();
        let mut file = match status {
            StatusCode::PARTIAL_CONTENT => {
                counters.done_bytes.fetch_add(offset, Ordering::Relaxed);
                tokio::fs::OpenOptions::new()
                    .append(true)
                    .open(&part)
                    .await?
            }
            s if s.is_success() => tokio::fs::File::create(&part).await?,
            StatusCode::RANGE_NOT_SATISFIABLE => {
                let _ = tokio::fs::remove_file(&part).await;
                return Ok(Attempt::Retry(status_error(status, &item.url)));
            }
            s if s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS => {
                return Ok(Attempt::Retry(status_error(status, &item.url)))
            }
            s => return Err(status_error(s, &item.url)),
        };

        let mut stream = resp.bytes_stream();
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => return Err(cancelled()),
                c = stream.next() => c,
            };
            match chunk {
                None => break,
                Some(Ok(bytes)) => {
                    file.write_all(&bytes).await?;
                    counters
                        .done_bytes
                        .fetch_add(bytes.len() as u64, Ordering::Relaxed);
                }
                Some(Err(e)) => {
                    file.flush().await?;
                    return Ok(Attempt::Retry(e.into()));
                }
            }
        }
        file.flush().await?;
        drop(file);

        if let Some(want) = &item.sha1 {
            let got = {
                let part = part.clone();
                tokio::task::spawn_blocking(move || sha1_file(&part))
                    .await
                    .map_err(|e| AppError::new("error.io").with_detail(e.to_string()))??
            };
            if !got.eq_ignore_ascii_case(want) {
                let _ = tokio::fs::remove_file(&part).await;
                return Ok(Attempt::Retry(
                    AppError::new("error.download.checksum")
                        .with_detail(format!("{}: expected sha1 {want}, got {got}", item.url)),
                ));
            }
        }
        tokio::fs::rename(&part, &item.dest).await?;
        Ok(Attempt::Done)
    }
}
