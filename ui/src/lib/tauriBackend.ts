import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Backend, Unsubscribe } from "./backend";

function on<T>(event: string, cb: (payload: T) => void): Unsubscribe {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  void listen<T>(event, (e) => cb(e.payload)).then((u) => {
    if (cancelled) u();
    else unlisten = u;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

export const tauriBackend: Backend = {
  systemInfo: () => invoke("system_info"),
  versionsList: (snapshots) => invoke("versions_list", { snapshots }),
  loaderVersions: (kind, mc) => invoke("loader_versions", { kind, mc }),
  instancesList: () => invoke("instances_list"),
  instanceGet: (id) => invoke("instance_get", { id }),
  instanceCreate: (req) => invoke("instance_create", { req }),
  instanceUpdate: (instance) => invoke("instance_update", { instance }),
  instanceDelete: (id) => invoke("instance_delete", { id }),
  instanceMods: (id) => invoke("instance_mods", { id }),
  instanceModToggle: (id, fileName, enabled) => invoke("instance_mod_toggle", { id, fileName, enabled }),
  instanceOpenFolder: (id) => invoke("instance_open_folder", { id }),
  logsOpenFolder: (id) => invoke("logs_open_folder", { id }),
  instanceLaunch: (id) => invoke("instance_launch", { id }),
  runningList: () => invoke("running_list"),
  settingsGet: () => invoke("settings_get"),
  settingsSet: (settings) => invoke("settings_set", { settings }),
  accountsList: () => invoke("accounts_list"),
  accountLoginStart: () => invoke("account_login_start"),
  accountAddOffline: (name) => invoke("account_add_offline", { name }),
  accountRemove: (id) => invoke("account_remove", { id }),
  accountSetActive: (id) => invoke("account_set_active", { id }),
  tasksList: () => invoke("tasks_list"),
  taskCancel: (id) => invoke("task_cancel", { id }),
  updateCheck: () => invoke("update_check"),
  updateInstall: () => invoke("update_install"),
  openUrl: (url) => openUrl(url),
  onTaskEvent: (cb) => on("task://event", cb),
  onInstanceLog: (cb) => on("instance://log", cb),
  onInstanceExit: (cb) => on("instance://exit", cb),
  onAuthDone: (cb) => on("auth://done", cb),
  onAuthFailed: (cb) => on("auth://failed", cb),
};
