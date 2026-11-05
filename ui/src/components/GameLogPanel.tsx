import { useTranslation } from "react-i18next";
import { getBackend } from "../lib/backend";
import { useUi } from "../lib/store";
import Button from "./Button";
import Modal from "./Modal";

/** Opens by itself when a game exits with an error. */
export default function GameLogPanel() {
  const { t } = useTranslation();
  const crash = useUi((s) => s.crash);
  const close = () => useUi.getState().showCrash(null);
  if (!crash) return null;
  return (
    <Modal title={t("launch.crashTitle")} onClose={close} width={760}>
      <p className="m-0 text-muted">{t("launch.crashBody", { code: crash.exit.code ?? "?" })}</p>
      <pre className="m-0 max-h-[360px] overflow-auto rounded-[10px] border border-line bg-bg p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap">
        {crash.exit.tail.join("\n")}
      </pre>
      <div className="flex flex-wrap justify-end gap-2">
        <Button onClick={() => void getBackend().logsOpenFolder(crash.instanceId)}>{t("launch.openLogs")}</Button>
        <Button variant="primary" onClick={close}>
          {t("common.close")}
        </Button>
      </div>
    </Modal>
  );
}
