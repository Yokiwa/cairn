import type { Account } from "../bindings/Account";
import type { AppError } from "../bindings/AppError";
import type { DeviceCode } from "../bindings/DeviceCode";
import type { ExitEvent } from "../bindings/ExitEvent";
import type { Instance } from "../bindings/Instance";
import type { InstancesList } from "../bindings/InstancesList";
import type { LoaderKind } from "../bindings/LoaderKind";
import type { LoaderVersion } from "../bindings/LoaderVersion";
import type { LogEvent } from "../bindings/LogEvent";
import type { ManifestEntry } from "../bindings/ManifestEntry";
import type { ModFile } from "../bindings/ModFile";
import type { NewInstance } from "../bindings/NewInstance";
import type { Settings } from "../bindings/Settings";
import type { SystemInfo } from "../bindings/SystemInfo";
import type { TaskEvent } from "../bindings/TaskEvent";
import type { TaskInfo } from "../bindings/TaskInfo";
import { mockBackend } from "./mockBackend";
import { tauriBackend } from "./tauriBackend";

export type Unsubscribe = () => void;

/** Everything the UI can ask the launcher for. Mirrors src-tauri's commands. */
export interface Backend {
  systemInfo(): Promise<SystemInfo>;
  versionsList(snapshots: boolean): Promise<ManifestEntry[]>;
  loaderVersions(kind: LoaderKind, mc: string): Promise<LoaderVersion[]>;
  instancesList(): Promise<InstancesList>;
  instanceGet(id: string): Promise<Instance>;
  instanceCreate(req: NewInstance): Promise<string>;
  instanceUpdate(instance: Instance): Promise<void>;
  instanceDelete(id: string): Promise<void>;
  instanceMods(id: string): Promise<ModFile[]>;
  instanceModToggle(id: string, fileName: string, enabled: boolean): Promise<void>;
  instanceOpenFolder(id: string): Promise<void>;
  logsOpenFolder(id: string): Promise<void>;
  instanceLaunch(id: string): Promise<string>;
  runningList(): Promise<string[]>;
  settingsGet(): Promise<Settings>;
  settingsSet(settings: Settings): Promise<void>;
  accountsList(): Promise<Account[]>;
  accountLoginStart(): Promise<DeviceCode>;
  accountAddOffline(name: string): Promise<Account>;
  accountRemove(id: string): Promise<void>;
  accountSetActive(id: string): Promise<void>;
  tasksList(): Promise<TaskInfo[]>;
  taskCancel(id: string): Promise<void>;
  updateCheck(): Promise<string | null>;
  updateInstall(): Promise<void>;
  openUrl(url: string): Promise<void>;
  onTaskEvent(cb: (e: TaskEvent) => void): Unsubscribe;
  onInstanceLog(cb: (e: LogEvent) => void): Unsubscribe;
  onInstanceExit(cb: (e: ExitEvent) => void): Unsubscribe;
  onAuthDone(cb: (a: Account) => void): Unsubscribe;
  onAuthFailed(cb: (e: AppError) => void): Unsubscribe;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

let current: Backend | null = null;

/** The real backend inside the app; an in-memory mock in a plain browser and in tests. */
export function getBackend(): Backend {
  current ??= isTauri() ? tauriBackend : mockBackend;
  return current;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "key" in e && typeof (e as AppError).key === "string";
}

/** Normalise anything thrown into an AppError. */
export function toAppError(e: unknown): AppError {
  if (isAppError(e)) return e;
  return { key: "error.unknown", params: {}, detail: e instanceof Error ? e.message : String(e) };
}
