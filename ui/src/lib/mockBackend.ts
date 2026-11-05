// In-memory stand-in for the Rust backend: used in a plain browser (`pnpm dev`),
// in unit tests and in Playwright. Data is sample data only.
import type { Account } from "../bindings/Account";
import type { AppError } from "../bindings/AppError";
import type { ExitEvent } from "../bindings/ExitEvent";
import type { Instance } from "../bindings/Instance";
import type { LoaderKind } from "../bindings/LoaderKind";
import type { LoaderVersion } from "../bindings/LoaderVersion";
import type { LogEvent } from "../bindings/LogEvent";
import type { ManifestEntry } from "../bindings/ManifestEntry";
import type { ModFile } from "../bindings/ModFile";
import type { Settings } from "../bindings/Settings";
import type { TaskEvent } from "../bindings/TaskEvent";
import type { TaskInfo } from "../bindings/TaskInfo";
import type { Backend } from "./backend";

const err = (key: string, params: Record<string, string> = {}): AppError => ({ key, params, detail: "" });

const RELEASES = ["26.3", "26.2", "26.1.2", "1.21.4", "1.21.1", "1.20.4", "1.20.1", "1.18.2", "1.16.5", "1.12.2", "1.8.9"];
const SNAPSHOTS = ["26.4-snapshot-7", "26.4-snapshot-6"];

function inst(id: string, name: string, mc: string, loader: Instance["loader"], lastPlayedHoursAgo: number | null): Instance {
  return {
    schema: 1,
    id,
    name,
    minecraft: mc,
    loader,
    java: { mode: "auto" },
    memory_mb: 6144,
    jvm_args: [],
    created_at: new Date(Date.now() - 30 * 86400000).toISOString(),
    last_played_at: lastPlayedHoursAgo === null ? null : new Date(Date.now() - lastPlayedHoursAgo * 3600000).toISOString(),
    mods: {},
    launch_version: null,
  };
}

const SAMPLE_MODS: ModFile[] = [
  "fabric-api-0.92.2.jar", "sodium-0.5.11.jar", "lithium-0.11.2.jar", "iris-1.7.2.jar", "modmenu-7.2.2.jar", "appleskin-2.5.1.jar",
].map((f) => ({ file_name: f, enabled: true, size: 400000 }));

interface MockState {
  instances: Instance[];
  mods: Record<string, ModFile[]>;
  accounts: Account[];
  settings: Settings;
  running: Set<string>;
  tasks: Map<string, TaskInfo>;
  launches: Map<string, string>; // task id → instance id
  msaConfigured: boolean;
}

function fresh(empty = false): MockState {
  const instances = empty
    ? []
    : [
        inst("survival-1-20-1", "Survival 1.20.1", "1.20.1", { kind: "fabric", version: "0.16.9" }, 2),
        inst("tech-valley", "Tech Valley", "1.21.1", { kind: "neoforge", version: "21.1.77" }, 26),
        inst("vanilla-1-21-4", "Vanilla 1.21.4", "1.21.4", null, 72),
        inst("skyblock", "Skyblock", "1.20.1", { kind: "forge", version: "47.4.28" }, 170),
      ];
  return {
    instances,
    mods: empty ? {} : { "survival-1-20-1": SAMPLE_MODS.map((m) => ({ ...m })), "tech-valley": SAMPLE_MODS.slice(0, 2).map((m) => ({ ...m })) },
    accounts: [],
    settings: { schema: 1, language: null, theme: "system", accent: "#34D399", download_threads: 32, default_memory_mb: null, show_snapshots: false },
    running: new Set(),
    tasks: new Map(),
    launches: new Map(),
    msaConfigured: false,
  };
}

let state = fresh();
type Listener<T> = (v: T) => void;
const listeners = {
  task: new Set<Listener<TaskEvent>>(),
  log: new Set<Listener<LogEvent>>(),
  exit: new Set<Listener<ExitEvent>>(),
  authDone: new Set<Listener<Account>>(),
  authFailed: new Set<Listener<AppError>>(),
};
const timers = new Set<ReturnType<typeof setTimeout>>();

function later(ms: number, f: () => void) {
  const t = setTimeout(() => {
    timers.delete(t);
    f();
  }, ms);
  timers.add(t);
}

/** Reset all mock data (tests). `empty` starts with no instances. */
export function resetMock(opts: { empty?: boolean; accounts?: Account[]; msaConfigured?: boolean } = {}) {
  timers.forEach(clearTimeout);
  timers.clear();
  state = fresh(opts.empty);
  if (opts.accounts) state.accounts = opts.accounts.map((a) => ({ ...a }));
  state.msaConfigured = opts.msaConfigured ?? false;
}

/** Finish a pending Microsoft sign-in (tests). */
export function mockFinishLogin(username = "Steve_MS") {
  const acc: Account = { id: "11111111-2222-4333-8444-555555555555", username, kind: "microsoft", active: true };
  state.accounts = state.accounts.map((a) => ({ ...a, active: false })).concat(acc);
  listeners.authDone.forEach((l) => l({ ...acc }));
}

/** Push a task event as if the backend sent it (tests). */
export function emitMockEvent(e: TaskEvent) {
  if (e.type === "progress") {
    const t = state.tasks.get(e.id) ?? { id: e.id, title: "test", state: "running", progress: null };
    state.tasks.set(e.id, { ...t, progress: e.progress });
  }
  listeners.task.forEach((l) => l(e));
}

/** Make a running instance crash (tests, Playwright). */
export function mockCrash(instanceId: string, code = 1) {
  if (!state.running.has(instanceId)) return;
  state.running.delete(instanceId);
  const tail = Array.from({ length: 50 }, (_, i) => `[main/ERROR] sample log line ${i + 71}`);
  tail[49] = "java.lang.IllegalStateException: sample crash";
  listeners.exit.forEach((l) => l({ instance_id: instanceId, exit: { code, tail } }));
  for (const [task, id] of state.launches) {
    if (id === instanceId) {
      state.launches.delete(task);
      listeners.task.forEach((l) => l({ type: "done", id: task, result: { code, tail } }));
    }
  }
}

if (typeof window !== "undefined") {
  (window as unknown as { __cairnMock: unknown }).__cairnMock = { crash: mockCrash, reset: resetMock, emit: emitMockEvent, finishLogin: (n?: string) => mockFinishLogin(n) };
}

let nextTask = 1;
function simulateTask(title: string, onDone: () => void): string {
  const id = `mock-task-${nextTask++}`;
  state.tasks.set(id, { id, title, state: "running", progress: null });
  const steps = [
    { stage: "metadata", f: 1, t: 1 },
    { stage: "game", f: 120, t: 240 },
    { stage: "assets", f: 1900, t: 3862 },
    { stage: "assets", f: 3862, t: 3862 },
  ];
  steps.forEach((s, i) =>
    later(60 * (i + 1), () =>
      emitMockEvent({
        type: "progress",
        id,
        progress: { stage: s.stage, done_files: s.f, total_files: s.t, done_bytes: s.f * 1000, total_bytes: s.t * 1000, bytes_per_sec: 18_400_000 },
      }),
    ),
  );
  later(60 * (steps.length + 1), () => {
    const t = state.tasks.get(id);
    if (t) state.tasks.set(id, { ...t, state: "done" });
    onDone();
    listeners.task.forEach((l) => l({ type: "done", id, result: null }));
  });
  return id;
}

function slugify(name: string): string {
  const s = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return s || "instance";
}

function find(id: string): Instance {
  const i = state.instances.find((x) => x.id === id);
  if (!i) throw err("error.instance.not_found", { id });
  return i;
}

const loaderSupport: Record<LoaderKind, (mc: string) => LoaderVersion[]> = {
  fabric: (mc) => (mc === "1.12.2" || mc === "1.8.9" ? [] : [{ version: "0.20.1", stable: true }, { version: "0.20.0", stable: false }]),
  quilt: (mc) => (mc.startsWith("1.2") || mc.startsWith("1.18") ? [{ version: "0.30.2", stable: true }] : []),
  forge: (mc) => (mc === "1.8.9" ? [] : [{ version: mc === "1.20.1" ? "47.4.28" : "52.1.20", stable: true }]),
  neoforge: (mc) => (mc.startsWith("1.21") || mc.startsWith("26") || mc === "1.20.1" ? [{ version: mc.startsWith("26") ? "26.3.0.101" : "21.1.265", stable: true }] : []),
};

const delay = <T>(v: T): Promise<T> => new Promise((r) => setTimeout(() => r(v), 0));

export const mockBackend: Backend = {
  systemInfo: () =>
    delay({ os: "linux", total_memory_mb: 16384, recommended_memory_mb: 6144, app_version: "0.1.0", microsoft_login_configured: state.msaConfigured, updates_enabled: false }),
  versionsList: (snapshots) =>
    delay([
      ...(snapshots ? SNAPSHOTS.map((id) => ({ id, type: "snapshot" as const })) : []),
      ...RELEASES.map((id) => ({ id, type: "release" as const })),
    ].map((v): ManifestEntry => ({ ...v, url: "", sha1: "", releaseTime: "2026-01-01T00:00:00Z" }))),
  loaderVersions: (kind, mc) => delay(loaderSupport[kind](mc)),
  instancesList: () => delay({ instances: state.instances.map((i) => ({ ...i })), errors: [] }),
  instanceGet: async (id) => ({ ...find(id) }),
  instanceCreate: async (req) => {
    let id = slugify(req.name);
    for (let n = 2; state.instances.some((i) => i.id === id); n++) id = `${slugify(req.name)}-${n}`;
    const i = inst(id, req.name, req.minecraft, req.loader, null);
    i.memory_mb = req.memory_mb ?? state.settings.default_memory_mb ?? 6144;
    state.instances.push(i);
    state.mods[id] = [];
    return simulateTask(`install:${id}`, () => {});
  },
  instanceUpdate: async (instance) => {
    find(instance.id);
    state.instances = state.instances.map((i) => (i.id === instance.id ? { ...instance } : i));
  },
  instanceDelete: async (id) => {
    find(id);
    if (state.running.has(id)) throw err("error.launch.already_running", { id });
    state.instances = state.instances.filter((i) => i.id !== id);
  },
  instanceMods: async (id) => (state.mods[id] ?? []).map((m) => ({ ...m })),
  instanceModToggle: async (id, fileName, enabled) => {
    const base = fileName.replace(/\.disabled$/, "");
    state.mods[id] = (state.mods[id] ?? []).map((m) =>
      m.file_name.replace(/\.disabled$/, "") === base ? { ...m, enabled, file_name: enabled ? base : `${base}.disabled` } : m,
    );
  },
  instanceOpenFolder: async () => {},
  logsOpenFolder: async () => {},
  instanceLaunch: async (id) => {
    const i = find(id);
    const acc = state.accounts.find((a) => a.active);
    if (!acc) throw err("error.auth.no_account");
    if (state.running.has(id)) throw err("error.launch.already_running", { id });
    state.running.add(id);
    i.last_played_at = new Date().toISOString();
    const task = `mock-launch-${nextTask++}`;
    state.launches.set(task, id);
    state.tasks.set(task, { id: task, title: `launch:${id}`, state: "running", progress: null });
    later(30, () => listeners.log.forEach((l) => l({ instance_id: id, line: "[main/INFO] Setting user: " + acc.username })));
    return task;
  },
  runningList: () => delay([...state.running]),
  settingsGet: () => delay({ ...state.settings }),
  settingsSet: async (s) => {
    state.settings = { ...s, download_threads: Math.min(64, Math.max(1, s.download_threads)) };
  },
  accountsList: () => delay(state.accounts.map((a) => ({ ...a }))),
  accountLoginStart: async () => {
    if (!state.msaConfigured) throw err("error.auth.not_configured");
    return { user_code: "ABCD-EFGH", verification_uri: "https://www.microsoft.com/link", expires_in: 900, interval: 5 };
  },
  accountAddOffline: async (name) => {
    if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) throw err("error.auth.invalid_name", { name });
    const id = `00000000-0000-3000-8000-${name.toLowerCase().padEnd(12, "0").slice(0, 12).replace(/[^0-9a-f]/g, "0")}`;
    state.accounts = state.accounts.filter((a) => a.id !== id).map((a) => ({ ...a, active: false }));
    const acc: Account = { id, username: name, kind: "offline", active: true };
    state.accounts.push(acc);
    return { ...acc };
  },
  accountRemove: async (id) => {
    state.accounts = state.accounts.filter((a) => a.id !== id);
    if (state.accounts.length && !state.accounts.some((a) => a.active)) state.accounts[0].active = true;
  },
  accountSetActive: async (id) => {
    state.accounts = state.accounts.map((a) => ({ ...a, active: a.id === id }));
  },
  tasksList: () => delay([...state.tasks.values()]),
  taskCancel: async (id) => {
    const inst = state.launches.get(id);
    if (inst) {
      state.running.delete(inst);
      state.launches.delete(id);
      listeners.exit.forEach((l) => l({ instance_id: inst, exit: { code: 0, tail: [] } }));
      listeners.task.forEach((l) => l({ type: "done", id, result: null }));
    }
  },
  updateCheck: () => delay(null),
  updateInstall: async () => {},
  openUrl: async () => {},
  onTaskEvent: (cb) => (listeners.task.add(cb), () => listeners.task.delete(cb)),
  onInstanceLog: (cb) => (listeners.log.add(cb), () => listeners.log.delete(cb)),
  onInstanceExit: (cb) => (listeners.exit.add(cb), () => listeners.exit.delete(cb)),
  onAuthDone: (cb) => (listeners.authDone.add(cb), () => listeners.authDone.delete(cb)),
  onAuthFailed: (cb) => (listeners.authFailed.add(cb), () => listeners.authFailed.delete(cb)),
};
