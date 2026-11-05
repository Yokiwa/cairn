//! Mod loaders: list versions and install them as version profiles.

pub mod fabric;
pub mod forge;

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::download::ProgressFn;
use crate::instances::{LoaderKind, LoaderSpec};
use crate::versions::compare_versions;
use crate::{Core, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

/// Newest first: numeric segments, then stable before pre-release, then
/// pre-release text ("beta.9" after "beta.10" is handled by number parsing).
pub fn sort_newest_first(list: &mut [LoaderVersion]) {
    fn pre(v: &str) -> (String, u64) {
        let suffix = v.split_once('-').map(|(_, s)| s).unwrap_or("");
        let num = suffix
            .rsplit(['.', '-'])
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        (
            suffix
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string(),
            num,
        )
    }
    list.sort_by(|a, b| {
        compare_versions(&b.version, &a.version)
            .then_with(|| b.stable.cmp(&a.stable))
            .then_with(|| {
                let (pa, pb) = (pre(&a.version), pre(&b.version));
                pb.0.cmp(&pa.0).then(pb.1.cmp(&pa.1))
            })
            .then(Ordering::Equal)
    });
}

pub async fn list_loader_versions(
    core: &Core,
    kind: LoaderKind,
    mc: &str,
) -> Result<Vec<LoaderVersion>> {
    let mut list = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric::list_versions(core, kind, mc).await?,
        LoaderKind::Forge | LoaderKind::NeoForge => forge::list_versions(core, kind, mc).await?,
    };
    sort_newest_first(&mut list);
    Ok(list)
}

/// Install a loader for `mc`; returns the version id to launch.
pub async fn install_loader(
    core: &Core,
    mc: &str,
    loader: &LoaderSpec,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<String> {
    match loader.kind {
        LoaderKind::Fabric | LoaderKind::Quilt => {
            fabric::install(core, mc, loader, progress, cancel).await
        }
        LoaderKind::Forge | LoaderKind::NeoForge => {
            forge::install(core, mc, loader, progress, cancel).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_numeric_then_stable_then_prerelease() {
        let mut l: Vec<LoaderVersion> = [
            ("0.20.0-beta.9", false),
            ("0.24.0", true),
            ("0.20.0", true),
            ("0.20.0-beta.10", false),
            ("0.9.1", true),
        ]
        .into_iter()
        .map(|(v, s)| LoaderVersion {
            version: v.into(),
            stable: s,
        })
        .collect();
        sort_newest_first(&mut l);
        let got: Vec<_> = l.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(
            got,
            [
                "0.24.0",
                "0.20.0",
                "0.20.0-beta.10",
                "0.20.0-beta.9",
                "0.9.1"
            ]
        );
    }
}
