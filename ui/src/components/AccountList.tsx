import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { AppError } from "../bindings/AppError";
import { getBackend, toAppError } from "../lib/backend";
import Button from "./Button";
import DeviceLoginDialog from "./DeviceLoginDialog";
import ErrorMessage from "./ErrorMessage";

export default function AccountList() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: () => getBackend().accountsList() });
  const [name, setName] = useState("");
  const [error, setError] = useState<AppError | null>(null);
  const [login, setLogin] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: ["accounts"] });

  async function run(f: () => Promise<unknown>) {
    setError(null);
    try {
      await f();
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {accounts.data?.length === 0 && <div className="text-muted">{t("settings.noAccounts")}</div>}
      <ul className="m-0 flex list-none flex-col gap-2 p-0">
        {accounts.data?.map((a) => (
          <li key={a.id} className="flex flex-wrap items-center gap-3">
            <div aria-hidden="true" className="grid h-9 w-9 place-items-center rounded-lg bg-accent font-bold text-accent-fg">
              {a.username[0]?.toUpperCase()}
            </div>
            <div className="min-w-[140px] flex-1">
              <div className="font-semibold">{a.username}</div>
              <div className="text-xs text-muted">
                {t(a.kind === "microsoft" ? "settings.microsoft" : "settings.offline")}
                {a.active ? ` · ${t("settings.active")}` : ""}
              </div>
            </div>
            {!a.active && (
              <Button aria-label={`${t("settings.useAccount")} ${a.username}`} onClick={() => void run(() => getBackend().accountSetActive(a.id))}>
                {t("settings.useAccount")}
              </Button>
            )}
            <Button variant="ghost" aria-label={`${t("settings.remove")} ${a.username}`} onClick={() => void run(() => getBackend().accountRemove(a.id))}>
              {t("settings.remove")}
            </Button>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap gap-2.5">
        <Button variant="primary" onClick={() => setLogin(true)}>
          {t("settings.addMicrosoft")}
        </Button>
      </div>
      <form
        className="flex flex-wrap items-end gap-2.5"
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await getBackend().accountAddOffline(name.trim());
            setName("");
          });
        }}
      >
        <label className="flex min-w-[180px] flex-1 flex-col gap-1 text-sm font-semibold">
          {t("settings.offlineName")}
          <input value={name} onChange={(e) => setName(e.target.value)} className="min-h-11 rounded-[10px] border border-line-strong bg-bg px-3 font-normal text-fg" />
        </label>
        <Button type="submit">{t("settings.addOffline")}</Button>
      </form>
      <div className="text-xs text-muted">{t("settings.offlineHint")}</div>
      {error && <ErrorMessage error={error} />}
      {login && <DeviceLoginDialog onClose={() => setLogin(false)} />}
    </div>
  );
}
