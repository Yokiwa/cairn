//! Download a version: its JSON, client jar, libraries, natives and assets.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::manifest::fetch_manifest;
use super::maven::maven_path;
use super::rules::{rules_allow, Environment};
use super::version_json::{merge, Library, VersionJson};
use crate::download::{DownloadItem, ProgressFn};
use crate::{AppError, Core, Endpoints, Paths, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default)]
    pub map_to_resources: bool,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
}

pub fn version_json_path(paths: &Paths, id: &str) -> PathBuf {
    paths.versions_dir().join(id).join(format!("{id}.json"))
}

pub fn client_jar_path(paths: &Paths, v: &VersionJson) -> PathBuf {
    let id = v.jar_id();
    paths.versions_dir().join(id).join(format!("{id}.jar"))
}

pub fn asset_index_path(paths: &Paths, id: &str) -> PathBuf {
    paths
        .assets_dir()
        .join("indexes")
        .join(format!("{id}.json"))
}

/// Classifier of a library's native jar on this OS, if it has one.
pub fn natives_classifier(lib: &Library, env: &Environment) -> Option<String> {
    let arch_bits = if env.arch == "x86" { "32" } else { "64" };
    lib.natives
        .as_ref()?
        .get(env.os.key())
        .map(|c| c.replace("${arch}", arch_bits))
}

/// Libraries that apply on this OS.
pub fn applicable_libraries<'a>(
    v: &'a VersionJson,
    env: &'a Environment,
) -> impl Iterator<Item = &'a Library> {
    v.libraries
        .iter()
        .filter(move |l| rules_allow(&l.rules, env))
}

/// Where a library's main artifact lives on disk (None if it has none).
pub fn library_path(paths: &Paths, lib: &Library) -> Result<Option<PathBuf>> {
    if lib.natives.is_some()
        && lib
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
            .is_none()
    {
        return Ok(None);
    }
    let rel = match lib
        .downloads
        .as_ref()
        .and_then(|d| d.artifact.as_ref())
        .and_then(|a| a.path.clone())
    {
        Some(p) => p,
        None => maven_path(&lib.name)?,
    };
    Ok(Some(paths.libraries_dir().join(rel)))
}

/// Where a library's native jar for this OS lives on disk.
pub fn natives_path(paths: &Paths, lib: &Library, env: &Environment) -> Result<Option<PathBuf>> {
    let Some(classifier) = natives_classifier(lib, env) else {
        return Ok(None);
    };
    let rel = match lib
        .downloads
        .as_ref()
        .and_then(|d| d.classifiers.as_ref())
        .and_then(|c| c.get(&classifier))
    {
        Some(a) => a
            .path
            .clone()
            .map(Ok)
            .unwrap_or_else(|| maven_path(&format!("{}:{classifier}", lib.name)))?,
        None => maven_path(&format!("{}:{classifier}", lib.name))?,
    };
    Ok(Some(paths.libraries_dir().join(rel)))
}

fn library_items(
    lib: &Library,
    env: &Environment,
    paths: &Paths,
    ep: &Endpoints,
) -> Result<Vec<DownloadItem>> {
    let mut out = Vec::new();
    let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());
    if let Some(dest) = library_path(paths, lib)? {
        match artifact {
            // Files produced by Forge processors have an empty url: nothing to download.
            Some(a) if a.url.is_empty() => {}
            Some(a) => out.push(DownloadItem {
                url: a.url.clone(),
                dest,
                sha1: a.sha1.clone(),
                size: a.size,
            }),
            None => {
                let base = lib.url.clone().unwrap_or_else(|| ep.libraries.clone());
                let url = format!("{}/{}", base.trim_end_matches('/'), maven_path(&lib.name)?);
                out.push(DownloadItem {
                    url,
                    dest,
                    sha1: lib.sha1.clone(),
                    size: lib.size,
                });
            }
        }
    }
    if let (Some(classifier), Some(dest)) =
        (natives_classifier(lib, env), natives_path(paths, lib, env)?)
    {
        match lib
            .downloads
            .as_ref()
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier))
        {
            Some(a) => out.push(DownloadItem {
                url: a.url.clone(),
                dest,
                sha1: a.sha1.clone(),
                size: a.size,
            }),
            None => {
                let base = lib.url.clone().unwrap_or_else(|| ep.libraries.clone());
                let url = format!(
                    "{}/{}",
                    base.trim_end_matches('/'),
                    maven_path(&format!("{}:{classifier}", lib.name))?
                );
                out.push(DownloadItem {
                    url,
                    dest,
                    sha1: None,
                    size: None,
                });
            }
        }
    }
    Ok(out)
}

/// Client jar, libraries (with natives for this OS) and the logging config.
pub fn plan_game_files(
    v: &VersionJson,
    env: &Environment,
    paths: &Paths,
    ep: &Endpoints,
) -> Result<Vec<DownloadItem>> {
    let mut items = Vec::new();
    if let Some(d) = &v.downloads {
        items.push(DownloadItem {
            url: d.client.url.clone(),
            dest: client_jar_path(paths, v),
            sha1: d.client.sha1.clone(),
            size: d.client.size,
        });
    }
    for lib in applicable_libraries(v, env) {
        items.extend(library_items(lib, env, paths, ep)?);
    }
    if let Some(c) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        items.push(DownloadItem {
            url: c.file.url.clone(),
            dest: paths.assets_dir().join("log_configs").join(&c.file.id),
            sha1: Some(c.file.sha1.clone()),
            size: Some(c.file.size),
        });
    }
    // Two libraries can resolve to the same file; download it once.
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert(i.dest.clone()));
    Ok(items)
}

pub fn plan_assets(index: &AssetIndex, paths: &Paths, resources_base: &str) -> Vec<DownloadItem> {
    let mut seen = std::collections::HashSet::new();
    index
        .objects
        .values()
        .filter(|o| seen.insert(o.hash.clone()))
        .map(|o| {
            let prefix = &o.hash[..2.min(o.hash.len())];
            DownloadItem {
                url: format!(
                    "{}/{prefix}/{}",
                    resources_base.trim_end_matches('/'),
                    o.hash
                ),
                dest: paths
                    .assets_dir()
                    .join("objects")
                    .join(prefix)
                    .join(&o.hash),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
            }
        })
        .collect()
}

fn read_raw(paths: &Paths, id: &str) -> Result<Option<VersionJson>> {
    match std::fs::read(version_json_path(paths, id)) {
        Ok(b) => Ok(Some(serde_json::from_slice(&b)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Read an installed version from disk, merged with its parents. No network.
pub fn load_installed(paths: &Paths, id: &str) -> Result<Option<VersionJson>> {
    let Some(v) = read_raw(paths, id)? else {
        return Ok(None);
    };
    match v.inherits_from.clone() {
        None => Ok(Some(v)),
        Some(parent) => match load_installed(paths, &parent)? {
            Some(p) => Ok(Some(merge(p, v))),
            None => Ok(None),
        },
    }
}

pub fn load_asset_index(paths: &Paths, id: &str) -> Result<Option<AssetIndex>> {
    match std::fs::read(asset_index_path(paths, id)) {
        Ok(b) => Ok(Some(serde_json::from_slice(&b)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Make sure `versions/<id>/<id>.json` exists, fetching it from the manifest.
async fn ensure_version_json(
    core: &Core,
    id: &str,
    progress: &ProgressFn,
    cancel: &CancellationToken,
) -> Result<VersionJson> {
    if let Some(v) = read_raw(&core.paths, id)? {
        return Ok(v);
    }
    let manifest = fetch_manifest(
        &core.http,
        &core.endpoints.mojang_meta,
        &core.paths.cache_dir(),
    )
    .await?;
    let entry = manifest
        .find(id)
        .ok_or_else(|| AppError::new("error.version.not_found").with_param("version", id))?;
    let item = DownloadItem {
        url: entry.url.clone(),
        dest: version_json_path(&core.paths, id),
        sha1: Some(entry.sha1.clone()),
        size: None,
    };
    core.downloader
        .download_all("metadata", vec![item], progress.clone(), cancel.clone())
        .await?;
    read_raw(&core.paths, id)?
        .ok_or_else(|| AppError::new("error.version.not_found").with_param("version", id))
}

/// Install a version and everything it needs. Files already present and
/// verified are skipped, so this is also the "repair" path before launch.
pub async fn install_version(
    core: &Core,
    id: &str,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<VersionJson> {
    let raw = ensure_version_json(core, id, &progress, &cancel).await?;
    if let Some(parent) = raw.inherits_from.clone() {
        Box::pin(install_version(
            core,
            &parent,
            progress.clone(),
            cancel.clone(),
        ))
        .await?;
    }
    let v = load_installed(&core.paths, id)?
        .ok_or_else(|| AppError::new("error.version.not_found").with_param("version", id))?;

    let env = Environment::current();
    let items = plan_game_files(&v, &env, &core.paths, &core.endpoints)?;
    if let Some(ai) = &v.asset_index {
        let index_item = DownloadItem {
            url: ai.url.clone(),
            dest: asset_index_path(&core.paths, &ai.id),
            sha1: Some(ai.sha1.clone()),
            size: Some(ai.size),
        };
        core.downloader
            .download_all(
                "metadata",
                vec![index_item],
                progress.clone(),
                cancel.clone(),
            )
            .await?;
        if let Some(index) = load_asset_index(&core.paths, &ai.id)? {
            core.downloader
                .download_all(
                    "assets",
                    plan_assets(&index, &core.paths, &core.endpoints.resources),
                    progress.clone(),
                    cancel.clone(),
                )
                .await?;
        }
    }
    core.downloader
        .download_all("game", items, progress, cancel)
        .await?;
    Ok(v)
}
