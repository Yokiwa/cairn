//! Real network installs. Run with:
//!   cargo test -p cairn-core --test live -- --ignored --test-threads=1
//! Uses CAIRN_LIVE_HOME (default: <tmp>/cairn-live) so assets download once.

use std::sync::Arc;

use cairn_core::download::{sha1_file, Progress};
use cairn_core::instances::{LoaderKind, LoaderSpec};
use cairn_core::loaders::{install_loader, list_loader_versions};
use cairn_core::versions::install::{client_jar_path, install_version, load_installed};
use cairn_core::versions::manifest::fetch_manifest;
use cairn_core::{Core, Paths};
use tokio_util::sync::CancellationToken;

fn core() -> Core {
    let home = std::env::var_os("CAIRN_LIVE_HOME")
        .map(Into::into)
        .unwrap_or_else(|| std::env::temp_dir().join("cairn-live"));
    Core::with_endpoints(Paths::new(home), Default::default()).unwrap()
}

fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}

async fn assert_installed(core: &Core, id: &str) {
    let v = load_installed(&core.paths, id).unwrap().expect("installed");
    let jar = client_jar_path(&core.paths, &v);
    let want = v.downloads.as_ref().unwrap().client.sha1.clone().unwrap();
    assert_eq!(sha1_file(&jar).unwrap(), want, "client jar sha1");
}

#[tokio::test]
#[ignore]
async fn live_manifest_has_latest_release() {
    let c = core();
    let m = fetch_manifest(&c.http, &c.endpoints.mojang_meta, &c.paths.cache_dir())
        .await
        .unwrap();
    assert!(m.find(&m.latest.release).is_some());
}

#[tokio::test]
#[ignore]
async fn live_install_vanilla_1_20_1() {
    let c = core();
    install_version(&c, "1.20.1", noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_installed(&c, "1.20.1").await;
}

async fn install_latest(kind: LoaderKind, mc: &str) {
    let c = core();
    let list = list_loader_versions(&c, kind, mc).await.unwrap();
    let v = list
        .iter()
        .find(|v| v.stable)
        .or(list.first())
        .unwrap()
        .version
        .clone();
    let id = install_loader(
        &c,
        mc,
        &LoaderSpec { kind, version: v },
        noop(),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    install_version(&c, &id, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_installed(&c, &id).await;
}

#[tokio::test]
#[ignore]
async fn live_install_fabric_1_20_1() {
    install_latest(LoaderKind::Fabric, "1.20.1").await;
}

#[tokio::test]
#[ignore]
async fn live_install_quilt_1_20_1() {
    install_latest(LoaderKind::Quilt, "1.20.1").await;
}

#[tokio::test]
#[ignore]
async fn live_install_forge_1_20_1() {
    install_latest(LoaderKind::Forge, "1.20.1").await;
}

#[tokio::test]
#[ignore]
async fn live_install_neoforge_1_21_1() {
    install_latest(LoaderKind::NeoForge, "1.21.1").await;
}
