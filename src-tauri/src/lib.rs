//! The Cairn desktop app: a Tauri window over `cairn-core`.

pub mod commands;

use std::sync::Arc;

use cairn_core::auth::{Account, DeviceCode};
use cairn_core::instances::{Instance, LoaderKind, ModFile, NewInstance};
use cairn_core::loaders::LoaderVersion;
use cairn_core::settings::Settings;
use cairn_core::tasks::{TaskId, TaskInfo};
use cairn_core::versions::manifest::ManifestEntry;
use cairn_core::{AppError, Core, Paths};
use commands::{AppState, EventSink, InstancesList, SystemInfo};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type R<T> = Result<T, AppError>;

struct TauriSink(AppHandle);
impl EventSink for TauriSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event, payload);
    }
}

#[tauri::command]
fn system_info(st: State<'_, AppState>) -> SystemInfo {
    commands::system_info(&st)
}
#[tauri::command]
async fn versions_list(st: State<'_, AppState>, snapshots: bool) -> R<Vec<ManifestEntry>> {
    commands::versions_list(&st, snapshots).await
}
#[tauri::command]
async fn loader_versions(
    st: State<'_, AppState>,
    kind: LoaderKind,
    mc: String,
) -> R<Vec<LoaderVersion>> {
    commands::loader_versions(&st, kind, &mc).await
}
#[tauri::command]
fn instances_list(st: State<'_, AppState>) -> R<InstancesList> {
    commands::instances_list(&st)
}
#[tauri::command]
fn instance_get(st: State<'_, AppState>, id: String) -> R<Instance> {
    commands::instance_get(&st, &id)
}
#[tauri::command]
async fn instance_create(st: State<'_, AppState>, req: NewInstance) -> R<TaskId> {
    commands::instance_create(&st, req).await
}
#[tauri::command]
fn instance_update(st: State<'_, AppState>, instance: Instance) -> R<()> {
    commands::instance_update(&st, instance)
}
#[tauri::command]
fn instance_delete(st: State<'_, AppState>, id: String) -> R<()> {
    commands::instance_delete(&st, &id)
}
#[tauri::command]
fn instance_mods(st: State<'_, AppState>, id: String) -> R<Vec<ModFile>> {
    commands::instance_mods(&st, &id)
}
#[tauri::command]
fn instance_mod_toggle(
    st: State<'_, AppState>,
    id: String,
    file_name: String,
    enabled: bool,
) -> R<()> {
    commands::instance_mod_toggle(&st, &id, &file_name, enabled)
}
#[tauri::command]
fn instance_open_folder(app: AppHandle, st: State<'_, AppState>, id: String) -> R<()> {
    let dir = commands::instance_folder(&st, &id)?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new("error.io").with_detail(e.to_string()))
}
#[tauri::command]
fn logs_open_folder(app: AppHandle, st: State<'_, AppState>, id: String) -> R<()> {
    let dir = commands::instance_folder(&st, &id)?.join("logs");
    let _ = std::fs::create_dir_all(&dir);
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new("error.io").with_detail(e.to_string()))
}
#[tauri::command]
async fn instance_launch(st: State<'_, AppState>, id: String) -> R<TaskId> {
    commands::instance_launch(&st, &id).await
}
#[tauri::command]
fn running_list(st: State<'_, AppState>) -> Vec<String> {
    commands::running_list(&st)
}
#[tauri::command]
fn settings_get(st: State<'_, AppState>) -> R<Settings> {
    commands::settings_get(&st)
}
#[tauri::command]
fn settings_set(st: State<'_, AppState>, settings: Settings) -> R<()> {
    commands::settings_set(&st, settings)
}
#[tauri::command]
fn accounts_list(st: State<'_, AppState>) -> R<Vec<Account>> {
    commands::accounts_list(&st)
}
#[tauri::command]
async fn account_login_start(st: State<'_, AppState>) -> R<DeviceCode> {
    commands::account_login_start(&st).await
}
#[tauri::command]
async fn account_add_offline(st: State<'_, AppState>, name: String) -> R<Account> {
    commands::account_add_offline(&st, &name).await
}
#[tauri::command]
fn account_remove(st: State<'_, AppState>, id: String) -> R<()> {
    commands::account_remove(&st, &id)
}
#[tauri::command]
fn account_set_active(st: State<'_, AppState>, id: String) -> R<()> {
    commands::account_set_active(&st, &id)
}
#[tauri::command]
fn tasks_list(st: State<'_, AppState>) -> Vec<TaskInfo> {
    commands::tasks_list(&st)
}
#[tauri::command]
fn task_cancel(st: State<'_, AppState>, id: String) {
    commands::task_cancel(&st, &id)
}

/// Check GitHub Releases for a newer version (None when up to date or when
/// this build has no update key).
#[tauri::command]
async fn update_check(app: AppHandle) -> R<Option<String>> {
    use tauri_plugin_updater::UpdaterExt;
    let Some(key) = commands::updater_pubkey() else {
        return Ok(None);
    };
    let err =
        |e: tauri_plugin_updater::Error| AppError::new("error.update").with_detail(e.to_string());
    let updater = app.updater_builder().pubkey(key).build().map_err(err)?;
    Ok(updater.check().await.map_err(err)?.map(|u| u.version))
}

/// Download and install the update, then restart.
#[tauri::command]
async fn update_install(app: AppHandle) -> R<()> {
    use tauri_plugin_updater::UpdaterExt;
    let Some(key) = commands::updater_pubkey() else {
        return Ok(());
    };
    let err =
        |e: tauri_plugin_updater::Error| AppError::new("error.update").with_detail(e.to_string());
    let updater = app.updater_builder().pubkey(key).build().map_err(err)?;
    if let Some(update) = updater.check().await.map_err(err)? {
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(err)?;
        app.restart();
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let paths = Paths::new(Paths::default_root()?);
            let guard = cairn_core::logging::init_logging(&paths).ok();
            // Keep the log writer alive for the whole app.
            app.manage(guard.map(Arc::new));
            let core = Core::new(paths)?;
            let state = AppState::new(core, Arc::new(TauriSink(app.handle().clone())));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system_info,
            versions_list,
            loader_versions,
            instances_list,
            instance_get,
            instance_create,
            instance_update,
            instance_delete,
            instance_mods,
            instance_mod_toggle,
            instance_open_folder,
            logs_open_folder,
            instance_launch,
            running_list,
            settings_get,
            settings_set,
            accounts_list,
            account_login_start,
            account_add_offline,
            account_remove,
            account_set_active,
            tasks_list,
            task_cancel,
            update_check,
            update_install
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cairn");
}
