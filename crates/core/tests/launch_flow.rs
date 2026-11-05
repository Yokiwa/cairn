#![cfg(unix)]
//! Launch flow with a fake `java` script standing in for the game.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cairn_core::download::Progress;
use cairn_core::instances::{JavaSetting, NewInstance};
use cairn_core::launch::launch_instance;
use cairn_core::{Core, Endpoints, Paths};
use serde_json::json;
use tokio_util::sync::CancellationToken;

const FAKE_JAVA: &str = r#"#!/bin/sh
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

fn fake_java(dir: &Path) -> PathBuf {
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
async fn setup(mode: Option<&str>) -> (tempfile::TempDir, Arc<Core>, String, String) {
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

fn noop() -> Arc<dyn Fn(&Progress) + Send + Sync> {
    Arc::new(|_: &Progress| {})
}

fn collector() -> (Arc<Mutex<Vec<String>>>, Arc<dyn Fn(&str) + Send + Sync>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let l2 = lines.clone();
    (
        lines,
        Arc::new(move |s: &str| l2.lock().unwrap().push(s.to_string())),
    )
}

#[tokio::test]
async fn launch_streams_logs_and_returns_exit() {
    let (_d, core, id, acc) = setup(None).await;
    let (lines, log) = collector();
    let exit = launch_instance(&core, &id, &acc, log, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(exit.code, Some(0));
    let lines = lines.lock().unwrap();
    assert!(
        lines.contains(&"hello".to_string()) && lines.contains(&"to stderr".to_string()),
        "{lines:?}"
    );
    assert!(core.instances.get(&id).unwrap().last_played_at.is_some());
    assert!(!core.running.is_running(&id));
}

#[tokio::test]
async fn crash_returns_last_50_lines() {
    let (_d, core, id, acc) = setup(Some("crash")).await;
    let (_lines, log) = collector();
    let exit = launch_instance(&core, &id, &acc, log, noop(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(exit.code, Some(1));
    assert_eq!(exit.tail.len(), 50);
    assert_eq!(exit.tail.last().unwrap(), "line 120");
    assert_eq!(exit.tail[0], "line 71");
}

#[tokio::test]
async fn second_launch_of_running_instance_is_rejected() {
    let (_d, core, id, acc) = setup(Some("sleep")).await;
    let (c2, id2, acc2) = (core.clone(), id.clone(), acc.clone());
    let first = tokio::spawn(async move {
        let (_l, log) = collector();
        launch_instance(&c2, &id2, &acc2, log, noop(), CancellationToken::new()).await
    });
    for _ in 0..100 {
        if core.running.is_running(&id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(core.running.is_running(&id));
    let (_l, log) = collector();
    let err = launch_instance(&core, &id, &acc, log, noop(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.launch.already_running");
    assert_eq!(first.await.unwrap().unwrap().code, Some(0));
}

#[tokio::test]
async fn cancel_stops_the_game() {
    let (_d, core, id, acc) = setup(Some("sleep")).await;
    let token = CancellationToken::new();
    let t2 = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        t2.cancel();
    });
    let (_l, log) = collector();
    let started = std::time::Instant::now();
    let exit = launch_instance(&core, &id, &acc, log, noop(), token)
        .await
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "game was not stopped"
    );
    assert_ne!(exit.code, Some(0));
}

#[tokio::test]
async fn explicit_java_path_must_be_valid() {
    let (_d, core, id, acc) = setup(None).await;
    let mut inst = core.instances.get(&id).unwrap();
    inst.java = JavaSetting::Path {
        path: PathBuf::from("/nope/bin/java"),
    };
    core.instances.save(&inst).unwrap();
    let (_l, log) = collector();
    let err = launch_instance(&core, &id, &acc, log, noop(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.key, "error.java.invalid_path");
}

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);
impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn launcher_log_never_contains_token() {
    let (_d, core, id, acc) = setup(None).await;
    let buf = Buf::default();
    let b2 = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || b2.clone())
        .with_ansi(false)
        .finish();
    let _g = tracing::subscriber::set_default(sub);
    // Offline sessions use "0" as the token; give the session a recognisable one.
    let (_l, log) = collector();
    launch_instance(&core, &id, &acc, log, noop(), CancellationToken::new())
        .await
        .unwrap();
    let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    assert!(text.contains("-Xmx1024M"), "launch command logged: {text}");
    assert!(
        text.contains("--accessToken ***") || text.contains("\"--accessToken\", \"***\""),
        "token redacted: {text}"
    );
}
