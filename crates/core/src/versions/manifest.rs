use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::http::HttpClient;
use crate::Result;

pub const MANIFEST_PATH: &str = "/mc/game/version_manifest_v2.json";
const MANIFEST_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub sha1: String,
    pub release_time: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

impl VersionManifest {
    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|v| v.id == id)
    }
}

/// Fetch the version list (cached for 10 minutes; stale cache when offline).
pub async fn fetch_manifest(
    http: &HttpClient,
    mojang_meta: &str,
    cache_dir: &Path,
) -> Result<VersionManifest> {
    http.get_json_cached(
        &format!("{mojang_meta}{MANIFEST_PATH}"),
        cache_dir,
        MANIFEST_TTL,
    )
    .await
}
