use std::sync::Arc;

use cairn_core::download::Progress;
use cairn_core::instances::{LoaderKind, LoaderSpec};
use cairn_core::loaders::{install_loader, list_loader_versions};
use cairn_core::versions::install::load_installed;
use cairn_core::{Core, Endpoints, Paths};
use serde_json::Value;
use sha1::{Digest, Sha1};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(dir: &str, name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/tests/fixtures/{dir}/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}

#[tokio::test]
async fn lists_fabric_versions_with_stable_flag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/versions/loader/1.20.1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("loaders", "fabric-versions.json")),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().into()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    let list = list_loader_versions(&core, LoaderKind::Fabric, "1.20.1")
        .await
        .unwrap();
    assert_eq!(list[0].version, "0.20.1");
    assert!(list[0].stable);
    assert!(list.iter().any(|v| !v.stable));
}

#[tokio::test]
async fn lists_quilt_versions_sorted_and_marks_betas_unstable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/versions/loader/1.20.1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("loaders", "quilt-versions.json")),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().into()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    let list = list_loader_versions(&core, LoaderKind::Quilt, "1.20.1")
        .await
        .unwrap();
    let names: Vec<_> = list
        .iter()
        .map(|v| (v.version.as_str(), v.stable))
        .collect();
    assert_eq!(
        names,
        [
            ("0.24.0", true),
            ("0.20.2-beta.1", false),
            ("0.20.0-beta.9", false),
            ("0.16.1-beta.3", false)
        ]
    );
}

#[tokio::test]
async fn install_fabric_writes_profile_and_libraries() {
    let server = MockServer::start().await;
    let profile = fixture("loaders", "fabric-profile.json");
    let loader_version = profile["id"]
        .as_str()
        .unwrap()
        .strip_prefix("fabric-loader-")
        .unwrap()
        .strip_suffix("-1.20.1")
        .unwrap()
        .to_string();
    // Point every library at the mock and give it a matching sha1.
    let mut profile = profile;
    let base = server.uri();
    for lib in profile["libraries"].as_array_mut().unwrap() {
        let body = format!("lib {}", lib["name"].as_str().unwrap()).into_bytes();
        lib["url"] = Value::from(format!("{base}/maven/"));
        lib["sha1"] = Value::from(hex::encode(Sha1::digest(&body)));
        lib["size"] = Value::from(body.len());
        let p = cairn_core::versions::maven::maven_path(lib["name"].as_str().unwrap()).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/maven/{p}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path(format!(
            "/v2/versions/loader/1.20.1/{loader_version}/profile/json"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(&profile))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex("^/mc/.*"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().into()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    // Pretend vanilla 1.20.1 is already installed (only its JSON matters here).
    let vanilla = fixture("versions", "1.20.1.json");
    let vdir = core.paths.versions_dir().join("1.20.1");
    std::fs::create_dir_all(&vdir).unwrap();
    std::fs::write(
        vdir.join("1.20.1.json"),
        serde_json::to_vec(&vanilla).unwrap(),
    )
    .unwrap();

    let spec = LoaderSpec {
        kind: LoaderKind::Fabric,
        version: loader_version.clone(),
    };
    let id = install_loader(&core, "1.20.1", &spec, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(id, format!("fabric-loader-{loader_version}-1.20.1"));
    let merged = load_installed(&core.paths, &id).unwrap().unwrap();
    assert_eq!(
        merged.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    assert_eq!(merged.jar_id(), id);
    for lib in profile["libraries"].as_array().unwrap() {
        let p = cairn_core::versions::maven::maven_path(lib["name"].as_str().unwrap()).unwrap();
        assert!(core.paths.libraries_dir().join(p).exists());
    }
}

#[tokio::test]
async fn forge_kinds_report_unsupported_until_implemented_or_work() {
    // Forge/NeoForge are dispatched to the forge module; an unknown mc must fail cleanly.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().into()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap();
    assert!(list_loader_versions(&core, LoaderKind::Forge, "1.20.1")
        .await
        .is_err());
}
