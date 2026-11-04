//! Forge and NeoForge (implemented in the next task).

use tokio_util::sync::CancellationToken;

use super::LoaderVersion;
use crate::download::ProgressFn;
use crate::instances::{LoaderKind, LoaderSpec};
use crate::{AppError, Core, Result};

pub async fn list_versions(
    _core: &Core,
    kind: LoaderKind,
    _mc: &str,
) -> Result<Vec<LoaderVersion>> {
    Err(AppError::new("error.loader.unsupported").with_param("loader", format!("{kind:?}")))
}

pub async fn install(
    _core: &Core,
    _mc: &str,
    loader: &LoaderSpec,
    _p: ProgressFn,
    _c: CancellationToken,
) -> Result<String> {
    Err(AppError::new("error.loader.unsupported")
        .with_param("loader", format!("{:?}", loader.kind)))
}
