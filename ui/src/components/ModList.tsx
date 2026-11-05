import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getBackend, toAppError } from "../lib/backend";
import { errorText } from "../lib/i18n";
import { useUi } from "../lib/store";
import ErrorMessage from "./ErrorMessage";
import Switch from "./Switch";

function sizeLabel(bytes: number): string {
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export default function ModList({ instanceId }: { instanceId: string }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [filter, setFilter] = useState("");
  const mods = useQuery({ queryKey: ["mods", instanceId], queryFn: () => getBackend().instanceMods(instanceId) });

  async function toggle(fileName: string, enabled: boolean) {
    try {
      await getBackend().instanceModToggle(instanceId, fileName, enabled);
    } catch (e) {
      useUi.getState().showToast(errorText(toAppError(e), t));
    }
    await qc.invalidateQueries({ queryKey: ["mods", instanceId] });
  }

  if (mods.error) return <ErrorMessage error={toAppError(mods.error)} />;
  const list = (mods.data ?? []).filter((m) => m.file_name.toLowerCase().includes(filter.toLowerCase()));
  return (
    <div className="flex flex-col gap-3">
      <input
        type="search"
        aria-label={t("detail.searchMods")}
        placeholder={t("detail.searchMods")}
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        className="min-h-11 rounded-[10px] border border-line-strong bg-panel px-3.5 text-fg"
      />
      {mods.data?.length === 0 && <div className="rounded-2xl border border-line bg-panel p-7 text-center text-muted">{t("detail.noMods")}</div>}
      {list.length > 0 && (
        <ul className="m-0 list-none overflow-hidden rounded-2xl border border-line bg-panel p-0">
          {list.map((m) => (
            <li key={m.file_name} className="flex items-center gap-3.5 border-b border-line px-4 py-2.5 last:border-b-0 hover:bg-panel-2">
              <Switch checked={m.enabled} onChange={(v) => void toggle(m.file_name, v)} label={m.file_name} />
              <div className={`min-w-0 grow truncate font-semibold ${m.enabled ? "" : "text-muted line-through"}`}>{m.file_name}</div>
              <span className="w-20 text-right font-mono text-xs text-muted">{sizeLabel(m.size)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
