import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import type { Instance } from "../bindings/Instance";
import { relativeTime } from "../lib/format";
import { LOADER_NAMES, initial, tileColor } from "../lib/instances";

export default function InstanceCard({ instance }: { instance: Instance }) {
  const { t, i18n } = useTranslation();
  const loader = instance.loader ? LOADER_NAMES[instance.loader.kind] : t("common.vanilla");
  const when = relativeTime(instance.last_played_at, i18n.language) ?? t("common.never");
  return (
    <Link
      to={`/instances/${instance.id}`}
      className="flex flex-col overflow-hidden rounded-2xl border border-line bg-panel text-fg no-underline transition hover:-translate-y-0.5 hover:border-line-strong"
    >
      <div aria-hidden="true" className="flex h-[92px] items-end p-3" style={{ background: tileColor(instance.id) }}>
        <span className="font-mono text-[34px] leading-none text-[#101217] opacity-80">{initial(instance.name)}</span>
      </div>
      <div className="flex flex-col gap-1.5 px-4 pt-3.5 pb-4">
        <div className="text-[15px] font-bold">{instance.name}</div>
        <div className="flex flex-wrap gap-1.5">
          <span className="rounded-md bg-panel-2 px-2 py-0.5 font-mono text-[11px] text-muted">{instance.minecraft}</span>
          <span className="rounded-md bg-panel-2 px-2 py-0.5 text-[11px] text-muted">{loader}</span>
        </div>
        <div className="text-xs text-muted">{t("instances.lastPlayed", { when })}</div>
      </div>
    </Link>
  );
}
