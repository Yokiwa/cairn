import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getBackend } from "./backend";
import { errorText } from "./i18n";
import { useUi } from "./store";

/** Turns backend events into UI state. Mount once, inside the providers. */
export function EventsBridge() {
  const qc = useQueryClient();
  const { t } = useTranslation();
  useEffect(() => {
    const b = getBackend();
    const ui = useUi.getState;
    const offs = [
      b.onTaskEvent((e) => {
        if (e.type === "progress") ui().setProgress(e.id, e.progress);
        else if (e.type === "done") {
          ui().finishTask(e.id, "done");
          void qc.invalidateQueries({ queryKey: ["instances"] });
        } else {
          ui().finishTask(e.id, "failed");
          const task = ui().tasks[e.id];
          if (task?.kind === "launch" && task.instanceId) ui().setRunning(task.instanceId, null);
          if (e.error.key !== "error.cancelled") ui().showToast(errorText(e.error, t));
        }
      }),
      b.onInstanceExit((e) => {
        ui().setRunning(e.instance_id, null);
        if (e.exit.code !== 0 && e.exit.code !== null) ui().showCrash({ instanceId: e.instance_id, exit: e.exit });
        void qc.invalidateQueries({ queryKey: ["instances"] });
      }),
      b.onAuthDone(() => void qc.invalidateQueries({ queryKey: ["accounts"] })),
    ];
    return () => offs.forEach((off) => off());
  }, [qc, t]);
  return null;
}
