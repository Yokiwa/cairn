#![cfg(unix)]
//! Launch flow with a fake `java` script standing in for the game.

mod common;

use cairn_core::instances::JavaSetting;
use cairn_core::launch::launch_instance;
use cairn_core::{Core, Endpoints, Paths};
use common::{collector, noop, setup};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

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

#[tokio::test]
async fn launch_keeps_the_installed_loader_profile() {
    use cairn_core::instances::{LoaderKind, LoaderSpec};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    let profile = json!({"id": "fabric-loader-0.1-1.20.1", "inheritsFrom": "1.20.1", "mainClass": "net.fabricmc.Knot", "libraries": [], "arguments": {"game": [], "jvm": []}});
    Mock::given(method("GET"))
        .and(path("/v2/versions/loader/1.20.1/0.1/profile/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(profile))
        .expect(1)
        .mount(&server)
        .await;

    let (dir, _old, id, _acc) = setup(None).await;
    let core = Arc::new(
        Core::with_endpoints(
            Paths::new(dir.path().join("data")),
            Endpoints::all_at(&server.uri()),
        )
        .unwrap(),
    );
    let acc = core.auth.add_offline("Tester").await.unwrap();
    let mut inst = core.instances.get(&id).unwrap();
    inst.loader = Some(LoaderSpec {
        kind: LoaderKind::Fabric,
        version: "0.1".into(),
    });
    core.instances.save(&inst).unwrap();

    let (_l, log) = collector();
    launch_instance(
        &core,
        &id,
        &acc.id,
        log.clone(),
        noop(),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let after = core.instances.get(&id).unwrap();
    assert_eq!(
        after.launch_version.as_ref().map(|l| l.id.as_str()),
        Some("fabric-loader-0.1-1.20.1"),
        "launch must not wipe launch_version"
    );
    assert!(after.last_played_at.is_some());
    // Second launch reuses the installed profile: no second profile request (expect(1) above).
    launch_instance(&core, &id, &acc.id, log, noop(), CancellationToken::new())
        .await
        .unwrap();
}
