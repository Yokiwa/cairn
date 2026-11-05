import { useState } from "react";
import { Link } from "react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { Instance } from "../bindings/Instance";
import { getBackend, toAppError } from "../lib/backend";
import { errorText } from "../lib/i18n";
import { useUi } from "../lib/store";
import Button from "./Button";
import Modal from "./Modal";

const PlayIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <path d="M7 4v16l13-8z" />
  </svg>
);

export default function LaunchButton({ instance, big = false }: { instance: Instance; big?: boolean }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const runningTask = useUi((s) => s.running[instance.id]);
  const serverRunning = useQuery({ queryKey: ["running"], queryFn: () => getBackend().runningList() });
  const [starting, setStarting] = useState(false);
  const [needAccount, setNeedAccount] = useState(false);
  const running = Boolean(runningTask) || (serverRunning.data ?? []).includes(instance.id);

  async function launch() {
    setStarting(true);
    try {
      const task = await getBackend().instanceLaunch(instance.id);
      useUi.getState().trackTask(task, "launch", instance.id);
      useUi.getState().setRunning(instance.id, task);
      void qc.invalidateQueries({ queryKey: ["instances"] });
    } catch (e) {
      const err = toAppError(e);
      if (err.key === "error.auth.no_account") setNeedAccount(true);
      else useUi.getState().showToast(errorText(err, t));
    } finally {
      setStarting(false);
      void qc.invalidateQueries({ queryKey: ["running"] });
    }
  }

  async function stop() {
    if (runningTask) await getBackend().taskCancel(runningTask);
  }

  const size = big ? "min-h-[52px] px-8 text-base" : "";
  return (
    <>
      {running ? (
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="primary" disabled className={size}>
            {t("home.running")}
          </Button>
          {runningTask && (
            <Button variant="ghost" onClick={stop}>
              {t("launch.stop")}
            </Button>
          )}
        </div>
      ) : (
        <Button variant="primary" className={size} onClick={launch} disabled={starting}>
          <PlayIcon />
          {starting ? t("home.launching") : t("home.play")}
        </Button>
      )}
      {needAccount && (
        <Modal title={t("launch.noAccountTitle")} onClose={() => setNeedAccount(false)} width={460}>
          <p className="m-0 text-muted">{t("launch.noAccountBody")}</p>
          <div className="flex justify-end">
            <Link
              to="/settings#accounts"
              onClick={() => setNeedAccount(false)}
              className="inline-flex min-h-11 items-center rounded-[10px] bg-accent px-4 font-semibold text-accent-fg no-underline"
            >
              {t("launch.goToSettings")}
            </Link>
          </div>
        </Modal>
      )}
    </>
  );
}
