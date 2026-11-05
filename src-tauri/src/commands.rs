//! Everything the UI can ask for. Plain functions over [`AppState`] so they can
//! be tested without a window; `lib.rs` wraps each one as a Tauri command.

use std::sync::Arc;

use cairn_core::auth::{Account, DeviceCode};
pub use cairn_core::instances::NewInstance;
use cairn_core::instances::{Instance, LoaderKind, ModFile};
use cairn_core::launch::{launch_instance, prepare_instance, GameExit, LogFn};
use cairn_core::loaders::{list_loader_versions, LoaderVersion};
use cairn_core::settings::{recommended_memory_mb, Settings};
use cairn_core::tasks::{TaskId, TaskInfo};
use cairn_core::versions::manifest::{fetch_manifest, ManifestEntry, VersionKind};
use cairn_core::{AppError, Core, Result};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const EV_TASK: &str = "task://event";
pub const EV_LOG: &str = "instance://log";
pub const EV_EXIT: &str = "instance://exit";
pub const EV_AUTH_DONE: &str = "auth://done";
pub const EV_AUTH_FAILED: &str = "auth://failed";

/// Where events for the UI go (the Tauri app handle, or a test collector).
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<Core>,
    pub sink: Arc<dyn EventSink>,
}

impl AppState {
    /// Wire the core up and forward its task events to the UI.
    pub fn new(core: Core, sink: Arc<dyn EventSink>) -> Self {
        let core = Arc::new(core);
        let mut rx = core.tasks.subscribe();
        let s2 = sink.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => s2.emit(EV_TASK, serde_json::to_value(&ev).unwrap_or_default()),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        });
        Self { core, sink }
    }

    fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        self.sink
            .emit(event, serde_json::to_value(payload).unwrap_or_default());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/bindings/")]
pub struct SystemInfo {
    pub os: String,
    #[ts(type = "number")]
    pub total_memory_mb: u64,
    pub recommended_memory_mb: u32,
    pub app_version: String,
    pub microsoft_login_configured: bool,
    pub updates_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/bindings/")]
pub struct InstancesList {
    pub instances: Vec<Instance>,
    pub errors: Vec<AppError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/bindings/")]
pub struct LogEvent {
    pub instance_id: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/bindings/")]
pub struct ExitEvent {
    pub instance_id: String,
    pub exit: GameExit,
}

pub fn updater_pubkey() -> Option<&'static str> {
    option_env!("CAIRN_UPDATER_PUBKEY").filter(|s| !s.is_empty())
}

pub fn system_info(st: &AppState) -> SystemInfo {
    let total = cairn_core::system::total_memory_mb();
    SystemInfo {
        os: std::env::consts::OS.into(),
        total_memory_mb: total,
        recommended_memory_mb: recommended_memory_mb(total),
        app_version: env!("CARGO_PKG_VERSION").into(),
        microsoft_login_configured: st.core.auth.microsoft_configured(),
        updates_enabled: updater_pubkey().is_some(),
    }
}

pub async fn versions_list(st: &AppState, snapshots: bool) -> Result<Vec<ManifestEntry>> {
    let m = fetch_manifest(
        &st.core.http,
        &st.core.endpoints.mojang_meta,
        &st.core.paths.cache_dir(),
    )
    .await?;
    Ok(m.versions
        .into_iter()
        .filter(|v| {
            v.kind == VersionKind::Release || (snapshots && v.kind == VersionKind::Snapshot)
        })
        .collect())
}

pub async fn loader_versions(
    st: &AppState,
    kind: LoaderKind,
    mc: &str,
) -> Result<Vec<LoaderVersion>> {
    list_loader_versions(&st.core, kind, mc).await
}

pub fn instances_list(st: &AppState) -> Result<InstancesList> {
    let (instances, errors) = st.core.instances.list()?;
    Ok(InstancesList { instances, errors })
}

pub fn instance_get(st: &AppState, id: &str) -> Result<Instance> {
    st.core.instances.get(id)
}

fn default_memory(st: &AppState) -> Result<u32> {
    let s = Settings::load(&st.core.paths)?;
    Ok(s.default_memory_mb
        .unwrap_or_else(|| recommended_memory_mb(cairn_core::system::total_memory_mb())))
}

/// Create the instance now and install it in the background.
pub async fn instance_create(st: &AppState, req: NewInstance) -> Result<TaskId> {
    let inst = st.core.instances.create(req, default_memory(st)?)?;
    let core = st.core.clone();
    let title = format!("install:{}", inst.id);
    Ok(st
        .core
        .tasks
        .spawn(title, move |progress, cancel| async move {
            prepare_instance(&core, &inst, progress, cancel).await?;
            Ok(serde_json::json!({ "instance_id": inst.id }))
        }))
}

pub fn instance_update(st: &AppState, instance: Instance) -> Result<()> {
    st.core.instances.get(&instance.id)?;
    st.core.instances.save(&instance)
}

pub fn instance_delete(st: &AppState, id: &str) -> Result<()> {
    if st.core.running.is_running(id) {
        return Err(AppError::new("error.launch.already_running").with_param("id", id));
    }
    st.core.instances.delete(id)
}

pub fn instance_mods(st: &AppState, id: &str) -> Result<Vec<ModFile>> {
    st.core.instances.list_mods(id)
}

pub fn instance_mod_toggle(st: &AppState, id: &str, file_name: &str, enabled: bool) -> Result<()> {
    st.core.instances.set_mod_enabled(id, file_name, enabled)
}

pub fn instance_folder(st: &AppState, id: &str) -> Result<std::path::PathBuf> {
    st.core.instances.get(id)?;
    Ok(st.core.instances.game_dir(id))
}

/// Launch with the active account. Logs and the exit arrive as events; the
/// returned task can be cancelled to stop the game.
pub async fn instance_launch(st: &AppState, id: &str) -> Result<TaskId> {
    st.core.instances.get(id)?;
    let account = st
        .core
        .auth
        .active()?
        .ok_or_else(|| AppError::new("error.auth.no_account"))?;
    if st.core.running.is_running(id) {
        return Err(AppError::new("error.launch.already_running").with_param("id", id));
    }
    let (core, st2, id) = (st.core.clone(), st.clone(), id.to_string());
    Ok(st
        .core
        .tasks
        .spawn(format!("launch:{id}"), move |progress, cancel| async move {
            let log: LogFn = {
                let (st, id) = (st2.clone(), id.clone());
                Arc::new(move |line: &str| {
                    st.emit(
                        EV_LOG,
                        &LogEvent {
                            instance_id: id.clone(),
                            line: line.to_string(),
                        },
                    )
                })
            };
            let exit = launch_instance(&core, &id, &account.id, log, progress, cancel).await?;
            st2.emit(
                EV_EXIT,
                &ExitEvent {
                    instance_id: id.clone(),
                    exit: exit.clone(),
                },
            );
            Ok(serde_json::to_value(&exit)?)
        }))
}

pub fn running_list(st: &AppState) -> Vec<String> {
    st.core.running.list()
}

pub fn settings_get(st: &AppState) -> Result<Settings> {
    Settings::load(&st.core.paths)
}

/// Saved immediately; a new download thread count applies after a restart.
pub fn settings_set(st: &AppState, settings: Settings) -> Result<()> {
    settings.save(&st.core.paths)
}

pub fn accounts_list(st: &AppState) -> Result<Vec<Account>> {
    st.core.auth.accounts()
}

/// Start a Microsoft sign-in; the result arrives as `auth://done` / `auth://failed`.
pub async fn account_login_start(st: &AppState) -> Result<DeviceCode> {
    let (code, pending) = st.core.auth.start_device_login().await?;
    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        match pending.finish().await {
            Ok(acc) => st2.emit(EV_AUTH_DONE, &acc),
            Err(e) => st2.emit(EV_AUTH_FAILED, &e),
        }
    });
    Ok(code)
}

pub async fn account_add_offline(st: &AppState, name: &str) -> Result<Account> {
    st.core.auth.add_offline(name).await
}

pub fn account_remove(st: &AppState, id: &str) -> Result<()> {
    st.core.auth.remove(id)
}

pub fn account_set_active(st: &AppState, id: &str) -> Result<()> {
    st.core.auth.set_active(id)
}

pub fn tasks_list(st: &AppState) -> Vec<TaskInfo> {
    st.core.tasks.list()
}

pub fn task_cancel(st: &AppState, id: &str) {
    st.core.tasks.cancel(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Default)]
    struct Collect(Mutex<Vec<(String, serde_json::Value)>>);
    impl EventSink for Collect {
        fn emit(&self, event: &str, payload: serde_json::Value) {
            self.0.lock().unwrap().push((event.to_string(), payload));
        }
    }

    fn state() -> (tempfile::TempDir, AppState, Arc<Collect>) {
        let d = tempfile::tempdir().unwrap();
        let core = cairn_core::Core::with_endpoints(
            cairn_core::Paths::new(d.path().into()),
            cairn_core::Endpoints::all_at("http://127.0.0.1:9"),
        )
        .unwrap();
        let sink = Arc::new(Collect::default());
        let st = AppState::new(core, sink.clone());
        (d, st, sink)
    }

    #[tokio::test]
    async fn instance_create_returns_task_and_instance_appears() {
        let (_d, st, sink) = state();
        let req = NewInstance {
            name: "Test".into(),
            minecraft: "1.20.1".into(),
            loader: None,
            memory_mb: Some(2048),
        };
        let task = instance_create(&st, req).await.unwrap();
        assert!(!task.is_empty());
        let list = instances_list(&st).unwrap();
        assert_eq!(list.instances.len(), 1);
        assert_eq!(list.instances[0].memory_mb, 2048);
        // The install task fails (nothing listens on the endpoint; ~7s of retries)
        // and that failure reaches the UI as an event.
        for _ in 0..1000 {
            if sink.0.lock().unwrap().iter().any(|(e, p)| {
                e == "task://event" && p["type"] == "failed" && p["id"] == task.as_str()
            }) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("no failed task event: {:?}", sink.0.lock().unwrap());
    }

    #[tokio::test]
    async fn launch_without_account_errors() {
        let (_d, st, _s) = state();
        let req = NewInstance {
            name: "T".into(),
            minecraft: "1.20.1".into(),
            loader: None,
            memory_mb: None,
        };
        instance_create(&st, req).await.unwrap();
        let id = instances_list(&st).unwrap().instances[0].id.clone();
        assert_eq!(
            instance_launch(&st, &id).await.unwrap_err().key,
            "error.auth.no_account"
        );
    }

    #[tokio::test]
    async fn settings_set_then_get_roundtrip() {
        let (_d, st, _s) = state();
        let mut s = settings_get(&st).unwrap();
        s.language = Some("en".into());
        s.download_threads = 8;
        settings_set(&st, s.clone()).unwrap();
        assert_eq!(settings_get(&st).unwrap(), s);
    }

    #[tokio::test]
    async fn offline_account_and_mod_toggle() {
        let (_d, st, _s) = state();
        let acc = account_add_offline(&st, "Tester").await.unwrap();
        assert_eq!(accounts_list(&st).unwrap()[0].id, acc.id);
        instance_create(
            &st,
            NewInstance {
                name: "M".into(),
                minecraft: "1.20.1".into(),
                loader: None,
                memory_mb: None,
            },
        )
        .await
        .unwrap();
        let id = instances_list(&st).unwrap().instances[0].id.clone();
        let mods = st.core.instances.game_dir(&id).join("mods");
        std::fs::write(mods.join("a.jar"), b"x").unwrap();
        instance_mod_toggle(&st, &id, "a.jar", false).unwrap();
        assert!(!instance_mods(&st, &id).unwrap()[0].enabled);
    }

    #[test]
    fn system_info_has_recommendation() {
        let (_d, st, _s) = state();
        let info = system_info(&st);
        assert!(info.total_memory_mb > 0);
        assert!([3072, 6144, 8192].contains(&info.recommended_memory_mb));
    }
}
