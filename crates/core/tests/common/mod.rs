//! Shared helpers: a fake `java` script standing in for the game.
#![allow(dead_code)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cairn_core::download::Progress;
use cairn_core::instances::{JavaSetting, NewInstance};
use cairn_core::{Core, Endpoints, Paths};
use serde_json::json;

pub const FAKE_JAVA: &str = r#"#!/bin/sh
for a in "$@"; do
  case "$a" in
    -version) echo 'openjdk version "17.0.9" 2023-10-17' >&2; exit 0 ;;
    -Dfake.mode=crash) i=1; while [ $i -le 120 ]; do echo "line $i"; i=$((i+1)); done; exit 1 ;;
    -Dfake.mode=sleep) echo "sleeping"; sleep 3; exit 0 ;;
  esac
done
echo "hello"
echo "to stderr" >&2
exit 0
"#;

pub fn fake_java(dir: &Path) -> PathBuf {
    let bin = dir.join("fakejdk/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let p = bin.join("java");
    std::fs::File::create(&p)
        .unwrap()
        .write_all(FAKE_JAVA.as_bytes())
        .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// A core with vanilla 1.20.1 "installed" (JSON only, nothing to download),
/// an offline account and one instance using the fake java.
pub async fn setup(mode: Option<&str>) -> (tempfile::TempDir, Arc<Core>, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::with_endpoints(
        Paths::new(dir.path().join("data")),
        Endpoints::all_at("http://127.0.0.1:9"),
    )
    .unwrap();
    let v = json!({"id": "1.20.1", "mainClass": "net.minecraft.client.main.Main", "libraries": [], "type": "release",
        "javaVersion": {"majorVersion": 17}, "arguments": {"game": ["--username", "${auth_player_name}", "--accessToken", "${auth_access_token}"], "jvm": ["-cp", "${classpath}"]}});
    let vd = core.paths.versions_dir().join("1.20.1");
    std::fs::create_dir_all(&vd).unwrap();
    std::fs::write(vd.join("1.20.1.json"), serde_json::to_vec(&v).unwrap()).unwrap();
    let acc = core.auth.add_offline("Tester").await.unwrap();
    let mut inst = core
        .instances
        .create(
            NewInstance {
                name: "Play".into(),
                minecraft: "1.20.1".into(),
                loader: None,
                memory_mb: Some(1024),
            },
            3072,
        )
        .unwrap();
    inst.java = JavaSetting::Path {
        path: fake_java(dir.path()),
    };
    if let Some(m) = mode {
        inst.jvm_args = vec![format!("-Dfake.mode={m}")];
    }
    core.instances.save(&inst).unwrap();
    (dir, Arc::new(core), inst.id, acc.id)
}

pub fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}

pub fn collector() -> (Arc<Mutex<Vec<String>>>, cairn_core::launch::LogFn) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let l2 = lines.clone();
    (
        lines,
        Arc::new(move |s: &str| l2.lock().unwrap().push(s.to_string())),
    )
}
