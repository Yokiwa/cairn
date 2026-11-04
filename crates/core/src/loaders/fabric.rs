//! Fabric and Quilt: both publish a ready-made profile JSON.

use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::LoaderVersion;
use crate::download::ProgressFn;
use crate::fsutil::write_atomic;
use crate::instances::{LoaderKind, LoaderSpec};
use crate::versions::install::{applicable_libraries, version_json_path};
use crate::versions::rules::Environment;
use crate::versions::version_json::VersionJson;
use crate::{AppError, Core, Result};

#[derive(Debug, Deserialize)]
struct Entry {
    loader: LoaderInfo,
}
#[derive(Debug, Deserialize)]
struct LoaderInfo {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

fn meta_base(core: &Core, kind: LoaderKind) -> String {
    match kind {
        LoaderKind::Quilt => format!("{}/v3", core.endpoints.quilt_meta),
        _ => format!("{}/v2", core.endpoints.fabric_meta),
    }
}

pub async fn list_versions(core: &Core, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    let url = format!("{}/versions/loader/{mc}", meta_base(core, kind));
    let entries: Vec<Entry> = core.http.get_json(&url).await?;
    Ok(entries
        .into_iter()
        .map(|e| {
            // Quilt has no "stable" flag; pre-releases carry a "-beta"/"-pre" suffix.
            let stable = e
                .loader
                .stable
                .unwrap_or_else(|| !e.loader.version.contains('-'));
            LoaderVersion {
                version: e.loader.version,
                stable,
            }
        })
        .collect())
}

pub async fn install(
    core: &Core,
    mc: &str,
    loader: &LoaderSpec,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<String> {
    let url = format!(
        "{}/versions/loader/{mc}/{}/profile/json",
        meta_base(core, loader.kind),
        loader.version
    );
    let profile: VersionJson = core.http.get_json(&url).await.map_err(|e| {
        if e.key == "error.http" {
            AppError::new("error.loader.not_found")
                .with_param("loader", format!("{:?} {}", loader.kind, loader.version))
                .with_param("mc", mc)
                .with_detail(e.detail)
        } else {
            e
        }
    })?;
    write_atomic(
        &version_json_path(&core.paths, &profile.id),
        &serde_json::to_vec_pretty(&profile)?,
    )?;

    // Only the loader's own libraries here; vanilla comes with install_version.
    let env = Environment::current();
    let only_loader = VersionJson {
        downloads: None,
        logging: None,
        ..profile.clone()
    };
    let mut items = Vec::new();
    for lib in applicable_libraries(&only_loader, &env) {
        let single = VersionJson {
            libraries: vec![lib.clone()],
            ..only_loader.clone()
        };
        items.extend(crate::versions::install::plan_game_files(
            &single,
            &env,
            &core.paths,
            &core.endpoints,
        )?);
    }
    core.downloader
        .download_all("loader", items, progress, cancel)
        .await?;
    Ok(profile.id)
}
