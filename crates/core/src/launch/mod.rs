//! Prepare an instance and start the game.

pub mod args;
pub mod natives;
pub mod process;

use std::process::Stdio;

use chrono::Utc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

pub use process::{GameExit, LogFn, LogTail, RunningGuard, RunningRegistry};

use crate::download::ProgressFn;
use crate::instances::{Instance, JavaSetting, LaunchVersion};
use crate::java::detect::inspect_home;
use crate::java::{ensure_java, required_major, JavaInstall};
use crate::loaders::install_loader;
use crate::versions::install::{install_version, load_asset_index, load_installed};
use crate::versions::rules::Environment;
use crate::versions::version_json::VersionJson;
use crate::{AppError, Core, Result};

fn java_from_path(core: &Core, path: &std::path::Path) -> Result<JavaInstall> {
    let invalid =
        || AppError::new("error.java.invalid_path").with_detail(path.display().to_string());
    let home = path.parent().and_then(|b| b.parent()).ok_or_else(invalid)?;
    inspect_home(home, &core.paths.java_dir()).ok_or_else(invalid)
}

/// Install (or verify) everything an instance needs; returns the merged
/// version to launch and the Java to launch it with.
pub async fn prepare_instance(
    core: &Core,
    instance: &Instance,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<(VersionJson, JavaInstall)> {
    let id = match &instance.loader {
        None => instance.minecraft.clone(),
        Some(loader) => {
            let reuse = instance
                .launch_version
                .as_ref()
                .filter(|lv| &lv.loader == loader)
                .filter(|lv| load_installed(&core.paths, &lv.id).ok().flatten().is_some())
                .map(|lv| lv.id.clone());
            match reuse {
                Some(id) => id,
                None => {
                    let id = install_loader(
                        core,
                        &instance.minecraft,
                        loader,
                        progress.clone(),
                        cancel.clone(),
                    )
                    .await?;
                    let lv = LaunchVersion {
                        loader: loader.clone(),
                        id: id.clone(),
                    };
                    core.instances
                        .update(&instance.id, |i| i.launch_version = Some(lv))?;
                    id
                }
            }
        }
    };
    // Installs vanilla through inheritsFrom and re-checks every file.
    let version = install_version(core, &id, progress.clone(), cancel.clone()).await?;
    let java = match &instance.java {
        JavaSetting::Path { path } => java_from_path(core, path)?,
        JavaSetting::Auto => ensure_java(core, required_major(&version), progress, cancel).await?,
    };
    Ok((version, java))
}

async fn pump<R: tokio::io::AsyncRead + Unpin>(r: R, log: LogFn, tail: LogTail) {
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        tail.push(&line);
        log(&line);
    }
}

/// Start an instance with an account and wait for the game to exit.
/// Cancelling the token stops the game.
pub async fn launch_instance(
    core: &Core,
    instance_id: &str,
    account_id: &str,
    log: LogFn,
    progress: ProgressFn,
    cancel: CancellationToken,
) -> Result<GameExit> {
    let _guard = core.running.try_register(instance_id)?;
    let instance = core.instances.get(instance_id)?;
    let (version, java) = prepare_instance(core, &instance, progress, cancel.clone()).await?;
    let session = core.auth.session(account_id).await?;

    let env = Environment::current();
    let natives_dir = core.paths.instance_dir(instance_id).join("natives");
    natives::extract_natives(&version, &env, &core.paths, &natives_dir)?;
    let gdir = args::game_dir(&core.paths, &instance);
    if let Some(ai) = &version.asset_index {
        if let Some(index) = load_asset_index(&core.paths, &ai.id)? {
            natives::prepare_legacy_assets(
                &index,
                &core.paths,
                &args::game_assets_dir(&version, &core.paths, &gdir),
            )?;
        }
    }

    let ctx = args::LaunchContext {
        version: &version,
        instance: &instance,
        session: &session,
        java: &java,
        paths: &core.paths,
        env: &env,
        natives_dir,
        window: None,
    };
    let cmd = args::build_command(&ctx)?;
    tracing::info!(instance = instance_id, program = %cmd.program.display(), "launching: {}", cmd.redacted().join(" "));

    core.instances
        .update(instance_id, |i| i.last_played_at = Some(Utc::now()))?;

    std::fs::create_dir_all(&cmd.cwd)?;
    let mut child = tokio::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(&cmd.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::new("error.launch.spawn").with_detail(e.to_string()))?;

    let tail = LogTail::default();
    let out = tokio::spawn(pump(
        child.stdout.take().expect("piped"),
        log.clone(),
        tail.clone(),
    ));
    let err = tokio::spawn(pump(child.stderr.take().expect("piped"), log, tail.clone()));

    let (status, cancelled) = tokio::select! {
        s = child.wait() => (s?, false),
        _ = cancel.cancelled() => {
            let _ = child.kill().await;
            (child.wait().await?, true)
        }
    };
    // Leftover child processes can keep the pipes open; don't wait on them forever.
    let grace = std::time::Duration::from_millis(if cancelled { 300 } else { 3000 });
    for pump in [out, err] {
        let abort = pump.abort_handle();
        if tokio::time::timeout(grace, pump).await.is_err() {
            abort.abort();
        }
    }
    tracing::info!(instance = instance_id, code = ?status.code(), "game exited");
    Ok(GameExit {
        code: status.code(),
        tail: tail.lines(),
    })
}
