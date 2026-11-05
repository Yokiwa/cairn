import { useState } from "react";
import { Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import Button from "../components/Button";
import ErrorMessage from "../components/ErrorMessage";
import InstanceTile from "../components/InstanceTile";
import LaunchButton from "../components/LaunchButton";
import NewInstanceWizard from "../components/NewInstanceWizard";
import Logo from "../components/Logo";
import { getBackend, toAppError } from "../lib/backend";
import { LOADER_NAMES, byRecent } from "../lib/instances";

export default function HomePage() {
  const { t } = useTranslation();
  const [wizard, setWizard] = useState(false);
  const instances = useQuery({ queryKey: ["instances"], queryFn: () => getBackend().instancesList() });
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: () => getBackend().accountsList() });
  const active = accounts.data?.find((a) => a.active);
  const list = byRecent(instances.data?.instances ?? []);
  const current = list[0];

  if (instances.error) return <ErrorMessage error={toAppError(instances.error)} />;
  if (instances.isLoading) return <div className="text-muted">{t("common.loading")}</div>;

  if (!current) {
    return (
      <div className="mx-auto mt-16 flex max-w-md flex-col items-center gap-4 text-center">
        <Logo size={56} />
        <h1 className="m-0 text-2xl">{t("home.emptyTitle")}</h1>
        <p className="m-0 text-muted">{t("home.emptyBody")}</p>
        <Button variant="primary" onClick={() => setWizard(true)}>
          {t("instances.new")}
        </Button>
        {wizard && <NewInstanceWizard onClose={() => setWizard(false)} />}
      </div>
    );
  }

  const loader = current.loader ? LOADER_NAMES[current.loader.kind] : t("common.vanilla");
  return (
    <div className="flex flex-col gap-5.5">
      <section className="flex min-h-[260px] flex-col gap-4.5 rounded-2xl border border-hero-line bg-hero p-7">
        <div className="text-[13px] font-semibold text-accent-strong">{active ? t("home.welcome", { name: active.username }) : t("home.welcomeAnon")}</div>
        <div>
          <div className="text-xs text-muted">{t("home.lastPlayed")}</div>
          <h1 className="m-0 text-[30px] font-bold tracking-tight">{current.name}</h1>
          <div className="mt-2 flex flex-wrap gap-2">
            <span className="rounded-md bg-panel px-2.5 py-0.5 font-mono text-xs">{current.minecraft}</span>
            <span className="rounded-md bg-panel px-2.5 py-0.5 text-xs">{loader}</span>
          </div>
        </div>
        <div className="grow" />
        <div className="flex flex-wrap items-center gap-2.5">
          <LaunchButton instance={current} big />
          <Link to="/instances" className="inline-flex min-h-11 items-center rounded-[10px] border border-line-strong bg-panel-2 px-4 text-sm font-semibold text-fg no-underline hover:bg-line">
            {t("home.switchInstance")}
          </Link>
        </div>
      </section>
      <div className="flex items-baseline justify-between">
        <h2 className="m-0 text-lg">{t("home.recent")}</h2>
        <Link to="/instances" className="text-sm text-muted">
          {t("home.viewAll")}
        </Link>
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3.5">
        {list.slice(0, 4).map((i) => (
          <Link key={i.id} to={`/instances/${i.id}`} className="flex items-center gap-3 rounded-2xl border border-line bg-panel p-4 text-fg no-underline transition hover:-translate-y-0.5 hover:border-line-strong">
            <InstanceTile id={i.id} name={i.name} />
            <div className="min-w-0">
              <div className="truncate font-semibold">{i.name}</div>
              <div className="text-xs text-muted">
                {i.minecraft} · {i.loader ? LOADER_NAMES[i.loader.kind] : t("common.vanilla")}
              </div>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}
