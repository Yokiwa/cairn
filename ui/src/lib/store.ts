import { create } from "zustand";
import type { GameExit } from "../bindings/GameExit";
import type { Progress } from "../bindings/Progress";

export type TaskStatus = "running" | "done" | "failed";

export interface UiTask {
  id: string;
  status: TaskStatus;
  progress: Progress | null;
  /** Instance this task installs or launches, when known. */
  instanceId?: string;
  kind?: "install" | "launch";
}

interface UiState {
  tasks: Record<string, UiTask>;
  running: Record<string, string>; // instance id → launch task id
  crash: { instanceId: string; exit: GameExit } | null;
  toast: string | null;
  trackTask(id: string, kind: UiTask["kind"], instanceId?: string): void;
  setProgress(id: string, p: Progress): void;
  finishTask(id: string, status: TaskStatus): void;
  setRunning(instanceId: string, taskId: string | null): void;
  showCrash(c: UiState["crash"]): void;
  showToast(msg: string | null): void;
  reset(): void;
}

export const useUi = create<UiState>((set) => ({
  tasks: {},
  running: {},
  crash: null,
  toast: null,
  trackTask: (id, kind, instanceId) =>
    set((s) => ({ tasks: { ...s.tasks, [id]: { ...(s.tasks[id] ?? { id, status: "running", progress: null }), kind, instanceId } } })),
  setProgress: (id, p) =>
    set((s) => ({ tasks: { ...s.tasks, [id]: { ...(s.tasks[id] ?? { id, status: "running" }), progress: p, status: "running" } } })),
  finishTask: (id, status) =>
    set((s) => (s.tasks[id] ? { tasks: { ...s.tasks, [id]: { ...s.tasks[id], status } } } : { tasks: { ...s.tasks, [id]: { id, status, progress: null } } })),
  setRunning: (instanceId, taskId) =>
    set((s) => {
      const running = { ...s.running };
      if (taskId) running[instanceId] = taskId;
      else delete running[instanceId];
      return { running };
    }),
  showCrash: (crash) => set({ crash }),
  showToast: (toast) => set({ toast }),
  reset: () => set({ tasks: {}, running: {}, crash: null, toast: null }),
}));
