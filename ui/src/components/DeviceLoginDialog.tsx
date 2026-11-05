import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { AppError } from "../bindings/AppError";
import type { DeviceCode } from "../bindings/DeviceCode";
import { getBackend, toAppError } from "../lib/backend";
import Button from "./Button";
import ErrorMessage from "./ErrorMessage";
import Modal from "./Modal";

export default function DeviceLoginDialog({ onClose }: { onClose(): void }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [code, setCode] = useState<DeviceCode | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [copied, setCopied] = useState(false);
  const [left, setLeft] = useState(0);

  useEffect(() => {
    const b = getBackend();
    let alive = true;
    b.accountLoginStart()
      .then((c) => {
        if (!alive) return;
        setCode(c);
        setLeft(c.expires_in);
      })
      .catch((e) => alive && setError(toAppError(e)));
    const offDone = b.onAuthDone(() => {
      void qc.invalidateQueries({ queryKey: ["accounts"] });
      onClose();
    });
    const offFailed = b.onAuthFailed((e) => setError(e));
    return () => {
      alive = false;
      offDone();
      offFailed();
    };
  }, [qc, onClose]);

  useEffect(() => {
    if (!code) return;
    const timer = setInterval(() => setLeft((s) => Math.max(0, s - 1)), 1000);
    return () => clearInterval(timer);
  }, [code]);

  async function copy() {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.user_code);
      setCopied(true);
    } catch {
      /* clipboard unavailable: the code is on screen */
    }
  }

  return (
    <Modal title={t("login.title")} onClose={onClose} width={480}>
      {error ? (
        error.key === "error.auth.not_configured" ? <p className="m-0 text-muted">{t("login.notConfigured")}</p> : <ErrorMessage error={error} />
      ) : !code ? (
        <div className="text-muted">{t("common.loading")}</div>
      ) : (
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <div className="text-sm text-muted">{t("login.step1")}</div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-mono text-sm break-all">{code.verification_uri}</span>
              <Button onClick={() => void getBackend().openUrl(code.verification_uri)}>{t("login.openPage")}</Button>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <div className="text-sm text-muted">{t("login.step2")}</div>
            <div className="flex flex-wrap items-center gap-3">
              <span className="rounded-xl bg-panel-2 px-4 py-2 font-mono text-3xl font-bold tracking-widest">{code.user_code}</span>
              <Button variant="ghost" onClick={() => void copy()}>
                {copied ? t("common.copied") : t("common.copy")}
              </Button>
            </div>
          </div>
          <div aria-live="polite" className="text-sm text-muted">
            {t("login.waiting")} {t("login.expiresIn", { minutes: Math.ceil(left / 60) })}
          </div>
        </div>
      )}
    </Modal>
  );
}
