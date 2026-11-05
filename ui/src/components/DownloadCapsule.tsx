import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { mbPerSec, percent } from "../lib/format";
import { useUi } from "../lib/store";

export default function DownloadCapsule() {
  const { t } = useTranslation();
  const all = useUi((s) => s.tasks);
  const tasks = Object.values(all).filter((x) => x.progress);
  const [open, setOpen] = useState(false);
  if (tasks.length === 0) return null;
  const running = tasks.filter((x) => x.status === "running");
  const main = running[0];
  const pct = main?.progress ? percent(main.progress) : 100;
  const stageLabel = (stage: string) => t(`downloads.stage.${stage}`, { defaultValue: stage });
  return (
    <div className="fixed right-5 bottom-5 z-30 flex max-w-[calc(100%-40px)] flex-col items-end gap-2.5">
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, scale: 0.97 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.97 }}
            className="flex w-[340px] max-w-full flex-col gap-3 rounded-2xl border border-line bg-panel p-4 shadow-2xl"
          >
            <div className="font-bold">{t("downloads.title")}</div>
            {tasks.map((task) => {
              const p = task.progress!;
              const v = task.status === "running" ? percent(p) : 100;
              return (
                <div key={task.id} className="flex flex-col gap-1">
                  <div className="flex justify-between text-xs">
                    <span>
                      {stageLabel(p.stage)}
                      {task.instanceId ? ` · ${task.instanceId}` : ""}
                    </span>
                    <span className="text-muted">
                      {task.status === "failed" ? t("downloads.failed") : `${v}%`}
                    </span>
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-line">
                    <div className="h-full rounded-full bg-accent transition-[width]" style={{ width: `${v}%` }} />
                  </div>
                </div>
              );
            })}
          </motion.div>
        )}
      </AnimatePresence>
      <button
        type="button"
        aria-label={t("downloads.title")}
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="flex min-h-11 items-center gap-2.5 rounded-full border border-line-strong bg-panel py-0 pr-4 pl-3 text-[13px] font-semibold shadow-xl"
      >
        {main ? (
          <svg className="animate-spin" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent)" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
            <path d="M12 3a9 9 0 1 0 9 9" />
          </svg>
        ) : (
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent)" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M5 12l5 5 9-10" />
          </svg>
        )}
        <span>
          {main
            ? `${pct}%${main.progress!.bytes_per_sec > 0 ? ` · ${t("downloads.speed", { speed: mbPerSec(main.progress!.bytes_per_sec) })}` : ""}`
            : t("downloads.allDone")}
        </span>
        <span className="inline-block h-1.5 w-[70px] overflow-hidden rounded-full bg-line">
          <span className="block h-full bg-accent" style={{ width: `${pct}%` }} />
        </span>
      </button>
    </div>
  );
}
