use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use cairn_core::download::Progress;
use cairn_core::versions::install::{
    install_version, load_installed, plan_assets, plan_game_files, AssetIndex,
};
use cairn_core::versions::rules::{Environment, OsName};
use cairn_core::versions::version_json::VersionJson;
use cairn_core::{Core, Endpoints, Paths};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/tests/fixtures/versions/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
fn linux() -> Environment {
    Environment {
        os: OsName::Linux,
        arch: "x86_64".into(),
        os_version: String::new(),
        features: HashMap::new(),
    }
}
fn osx() -> Environment {
    Environment {
        os: OsName::Osx,
        ..linux()
    }
}
fn sha1_hex(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}
fn endpoints() -> Endpoints {
    Endpoints::default()
}

#[test]
fn plan_includes_client_libs_and_natives_for_current_os() {
    let v: VersionJson = serde_json::from_value(fixture("1.8.9.json")).unwrap();
    let paths = Paths::new("/r".into());
    let items = plan_game_files(&v, &linux(), &paths, &endpoints()).unwrap();
    let dests: Vec<String> = items
        .iter()
        .map(|i| i.dest.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(
        dests.contains(&"/r/shared/versions/1.8.9/1.8.9.jar".to_string()),
        "{dests:?}"
    );
    assert!(dests
        .iter()
        .any(|d| d.ends_with("lwjgl-platform-2.9.4-nightly-20150209-natives-linux.jar")));
    assert!(dests
        .iter()
        .any(|d| d.ends_with("jinput-platform-2.0.5-natives-linux.jar")));
    assert!(!dests.iter().any(|d| d.contains("natives-windows")));
    assert!(dests.iter().any(|d| d.ends_with("authlib-1.5.21.jar")));
    assert!(dests.contains(&"/r/shared/assets/log_configs/client-1.7.xml".to_string()));
    assert!(
        items.iter().all(|i| i.sha1.is_some()),
        "mojang files all carry sha1"
    );
}

#[test]
fn plan_skips_disallowed_library() {
    let v: VersionJson = serde_json::from_value(fixture("1.8.9.json")).unwrap();
    let paths = Paths::new("/r".into());
    let items = plan_game_files(&v, &osx(), &paths, &endpoints()).unwrap();
    assert!(!items
        .iter()
        .any(|i| i.dest.to_string_lossy().contains("lwjgl-2.9.4")));
    assert!(items.iter().any(|i| i
        .dest
        .to_string_lossy()
        .ends_with("jinput-platform-2.0.5-natives-osx.jar")));
}

#[test]
fn plan_library_with_only_maven_url() {
    let v: VersionJson = serde_json::from_value(fixture("fabric-1.20.1.json")).unwrap();
    let paths = Paths::new("/r".into());
    let items = plan_game_files(&v, &linux(), &paths, &endpoints()).unwrap();
    let lib = &v.libraries[0];
    let want_path = cairn_core::versions::maven::maven_path(&lib.name).unwrap();
    let it = items
        .iter()
        .find(|i| {
            i.dest
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with(&want_path)
        })
        .unwrap();
    assert_eq!(it.url, format!("https://maven.fabricmc.net/{want_path}"));
    assert_eq!(it.sha1, lib.sha1);
}

#[test]
fn plan_assets_uses_hash_layout() {
    let index: AssetIndex =
        serde_json::from_value(json!({"objects": {"a/b.png": {"hash": "abcdef0123", "size": 3}}}))
            .unwrap();
    let items = plan_assets(&index, &Paths::new("/r".into()), "https://res.example");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].url, "https://res.example/ab/abcdef0123");
    assert_eq!(
        items[0].dest.to_string_lossy().replace('\\', "/"),
        "/r/shared/assets/objects/ab/abcdef0123"
    );
    assert_eq!(items[0].size, Some(3));
}

/// Serve a fake but self-consistent 1.20.1 from a mock server.
struct FakeMojang {
    server: MockServer,
    lib_paths: Vec<String>,
}

async fn fake_mojang() -> FakeMojang {
    let server = MockServer::start().await;
    let base = server.uri();
    let mut v = fixture("1.20.1.json");
    let mut n = 0;
    let mut bodies: Vec<(String, Vec<u8>)> = Vec::new();
    let mut lib_paths = Vec::new();

    // asset index with one object
    let asset_body = b"fake asset".to_vec();
    let ah = sha1_hex(&asset_body);
    bodies.push((format!("/{}/{}", &ah[..2], ah), asset_body.clone()));
    let index = serde_json::to_vec(
        &json!({"objects": {"icons/x.png": {"hash": ah, "size": asset_body.len()}}}),
    )
    .unwrap();
    v["assetIndex"]["url"] = json!(format!("{base}/index.json"));
    v["assetIndex"]["sha1"] = json!(sha1_hex(&index));
    v["assetIndex"]["size"] = json!(index.len());
    bodies.push(("/index.json".into(), index));

    fn rewrite(
        o: &mut Value,
        base: &str,
        n: &mut usize,
        bodies: &mut Vec<(String, Vec<u8>)>,
        lib_paths: &mut Vec<String>,
    ) {
        match o {
            Value::Object(map) => {
                if map.contains_key("url")
                    && map.contains_key("sha1")
                    && !map.contains_key("objects")
                {
                    *n += 1;
                    let p = format!("/f{n}");
                    let body = format!("file {n}").into_bytes();
                    map.insert("url".into(), json!(format!("{base}{p}")));
                    map.insert("sha1".into(), json!(sha1_hex(&body)));
                    map.insert("size".into(), json!(body.len()));
                    if let Some(path) = map.get("path").and_then(|p| p.as_str()) {
                        lib_paths.push(path.to_string());
                    }
                    bodies.push((p, body));
                }
                for (k, v) in map.iter_mut() {
                    if k != "assetIndex" {
                        rewrite(v, base, n, bodies, lib_paths);
                    }
                }
            }
            Value::Array(a) => a
                .iter_mut()
                .for_each(|x| rewrite(x, base, n, bodies, lib_paths)),
            _ => {}
        }
    }
    rewrite(&mut v, &base, &mut n, &mut bodies, &mut lib_paths);

    let vbody = serde_json::to_vec(&v).unwrap();
    bodies.push(("/v/1.20.1.json".into(), vbody.clone()));
    let manifest = json!({
        "latest": {"release": "1.20.1", "snapshot": "1.20.1"},
        "versions": [{"id": "1.20.1", "type": "release", "url": format!("{base}/v/1.20.1.json"), "sha1": sha1_hex(&vbody), "releaseTime": "2023-06-12T13:25:51+00:00", "time": "2023-06-12T13:25:51+00:00"}]
    });
    bodies.push((
        "/mc/game/version_manifest_v2.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    ));
    for (p, b) in bodies {
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b))
            .mount(&server)
            .await;
    }
    FakeMojang { server, lib_paths }
}

fn core_for(root: &Path, server: &MockServer) -> Core {
    Core::with_endpoints(
        Paths::new(root.to_path_buf()),
        Endpoints::all_at(&server.uri()),
    )
    .unwrap()
}

fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}

#[tokio::test]
async fn install_downloads_everything_then_loads_offline() {
    let fake = fake_mojang().await;
    let dir = tempfile::tempdir().unwrap();
    let core = core_for(dir.path(), &fake.server);
    let v = install_version(&core, "1.20.1", noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(v.id, "1.20.1");
    assert!(core.paths.versions_dir().join("1.20.1/1.20.1.jar").exists());
    assert!(core
        .paths
        .versions_dir()
        .join("1.20.1/1.20.1.json")
        .exists());
    assert!(
        core.paths.assets_dir().join("indexes/5.json").exists()
            || std::fs::read_dir(core.paths.assets_dir().join("indexes"))
                .unwrap()
                .count()
                == 1
    );
    assert_eq!(
        std::fs::read_dir(core.paths.assets_dir().join("objects"))
            .unwrap()
            .count(),
        1
    );
    drop(fake);
    let again = load_installed(&core.paths, "1.20.1").unwrap().unwrap();
    assert_eq!(again, v);
}

#[tokio::test]
async fn second_install_only_fetches_missing() {
    let fake = fake_mojang().await;
    let dir = tempfile::tempdir().unwrap();
    let core = core_for(dir.path(), &fake.server);
    install_version(&core, "1.20.1", noop(), CancellationToken::new())
        .await
        .unwrap();
    let first = fake.server.received_requests().await.unwrap().len();

    // Pick a library that applies on this OS and delete it.
    let present: Vec<&String> = fake
        .lib_paths
        .iter()
        .filter(|p| core.paths.libraries_dir().join(p).exists())
        .collect();
    let victim = present[0].clone();
    std::fs::remove_file(core.paths.libraries_dir().join(&victim)).unwrap();

    install_version(&core, "1.20.1", noop(), CancellationToken::new())
        .await
        .unwrap();
    let reqs = fake.server.received_requests().await.unwrap();
    let new: HashSet<String> = reqs[first..]
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    assert_eq!(
        new.len(),
        1,
        "only the missing library is fetched again: {new:?}"
    );
    assert!(core.paths.libraries_dir().join(&victim).exists());
}

#[tokio::test]
async fn unknown_version_is_a_clear_error() {
    let fake = fake_mojang().await;
    let dir = tempfile::tempdir().unwrap();
    let core = core_for(dir.path(), &fake.server);
    let err = install_version(&core, "9.9.9", noop(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.version.not_found");
    assert_eq!(err.params.get("version").map(String::as_str), Some("9.9.9"));
}
