import { useEffect, useId, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { Instance } from "../bindings/Instance";
import Button from "../components/Button";
import ErrorMessage from "../components/ErrorMessage";
import InstanceTile from "../components/InstanceTile";
import LaunchButton from "../components/LaunchButton";
import MemorySlider from "../components/MemorySlider";
import Modal from "../components/Modal";
import ModList from "../components/ModList";
import { getBackend, toAppError } from "../lib/backend";
import { errorText } from "../lib/i18n";
import { LOADER_NAMES } from "../lib/instances";
import { useUi } from "../lib/store";

type Tab = "overview" | "mods" | "settings";

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="rounded-2xl border border-line bg-panel p-4.5">
      <div className="text-xs text-muted">{label}</div>
      <div className="mt-1 text-[22px] font-bold">{value}</div>
      {sub && <div className="text-xs text-muted">{sub}</div>}
    </div>
  );
}

function InstanceSettings({ instance }: { instance: Instance }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const info = useQuery({ queryKey: ["system"], queryFn: () => getBackend().systemInfo() });
  const [memory, setMemory] = useState(instance.memory_mb);
  const [javaPath, setJavaPath] = useState(instance.java.mode === "path" ? instance.java.path : "");
  const [custom, setCustom] = useState(instance.java.mode === "path");
  const [args, setArgs] = useState(instance.jvm_args.join(" "));
  const [loaderVersion, setLoaderVersion] = useState(instance.loader?.version ?? "");
  const argsId = useId();
  const loaderId = useId();
  const loaderVersions = useQuery({
    queryKey: ["loaderVersions", instance.loader?.kind, instance.minecraft],
    queryFn: () => getBackend().loaderVersions(instance.loader!.kind, instance.minecraft),
    enabled: instance.loader !== null,
  });
  const versionOptions = (() => {
    const list = loaderVersions.data ?? [];
    const current = instance.loader?.version;
    return current && !list.some((v) => v.version === current) ? [{ version: current, stable: false }, ...list] : list;
  })();

  async function save() {
    const next: Instance = {
      ...instance,
      memory_mb: memory,
      java: custom && javaPath.trim() ? { mode: "path", path: javaPath.trim() } : { mode: "auto" },
      jvm_args: args.split(/\s+/).filter(Boolean),
      loader: instance.loader && loaderVersion ? { kind: instance.loader.kind, version: loaderVersion } : instance.loader,
    };
    try {
      await getBackend().instanceUpdate(next);
      useUi.getState().showToast(t("common.saved"));
      await qc.invalidateQueries({ queryKey: ["instance", instance.id] });
    } catch (e) {
      useUi.getState().showToast(errorText(toAppError(e), t));
    }
  }

  return (
    <div className="flex max-w-[640px] flex-col gap-4.5 rounded-2xl border border-line bg-panel p-5.5">
      {info.data && (
        <MemorySlider label={t("detail.memory")} value={memory} onChange={setMemory} totalMb={info.data.total_memory_mb} recommendedMb={info.data.recommended_memory_mb} />
      )}
      {instance.loader && (
        <div className="flex flex-col gap-1.5">
          <label htmlFor={loaderId} className="font-semibold">
            {t("detail.loaderVersion")}
          </label>
          <select
            id={loaderId}
            value={loaderVersion}
            onChange={(e) => setLoaderVersion(e.target.value)}
            className="min-h-11 rounded-[10px] border border-line-strong bg-bg px-3 font-mono text-[13px] text-fg"
          >
            {versionOptions.map((v) => (
              <option key={v.version} value={v.version}>
                {v.stable ? `${v.version} (${t("detail.stable")})` : v.version}
              </option>
            ))}
          </select>
          <div className="text-xs text-muted">{t("detail.loaderVersionHint", { loader: LOADER_NAMES[instance.loader.kind] })}</div>
        </div>
      )}
      <fieldset className="m-0 flex flex-col gap-2 border-0 p-0">
        <legend className="mb-1 font-semibold">{t("detail.javaMode")}</legend>
        <label className="flex items-center gap-2">
          <input type="radio" name="java" checked={!custom} onChange={() => setCustom(false)} /> {t("detail.javaAuto")}
        </label>
        <label className="flex items-center gap-2">
          <input type="radio" name="java" checked={custom} onChange={() => setCustom(true)} /> {t("detail.javaCustom")}
        </label>
        {custom && (
          <input
            aria-label={t("detail.javaPath")}
            placeholder={t("detail.javaPath")}
            value={javaPath}
            onChange={(e) => setJavaPath(e.target.value)}
            className="min-h-11 rounded-[10px] border border-line-strong bg-bg px-3 font-mono text-[13px] text-fg"
          />
        )}
      </fieldset>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={argsId} className="font-semibold">
          {t("detail.jvmArgs")}
        </label>
        <input id={argsId} value={args} onChange={(e) => setArgs(e.target.value)} className="min-h-11 rounded-[10px] border border-line-strong bg-bg px-3 font-mono text-[13px] text-fg" />
      </div>
      <div>
        <Button variant="primary" onClick={() => void save()}>
          {t("common.save")}
        </Button>
      </div>
    </div>
  );
}

export default function InstanceDetailPage() {
  const { id = "" } = useParams();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>("overview");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const q = useQuery({ queryKey: ["instance", id], queryFn: () => getBackend().instanceGet(id) });
  useEffect(() => setTab("overview"), [id]);

  if (q.error) {
    const err = toAppError(q.error);
    return (
      <div className="flex flex-col gap-3">
        <Link to="/instances" className="text-muted">
          ← {t("common.back")}
        </Link>
        {err.key === "error.instance.not_found" ? <p>{t("detail.notFound")}</p> : <ErrorMessage error={err} />}
      </div>
    );
  }
  const inst = q.data;
  if (!inst) return <div className="text-muted">{t("common.loading")}</div>;
  const loader = inst.loader ? `${LOADER_NAMES[inst.loader.kind]} ${inst.loader.version}` : t("common.vanilla");

  async function remove() {
    try {
      await getBackend().instanceDelete(inst!.id);
      await qc.invalidateQueries({ queryKey: ["instances"] });
      navigate("/instances");
    } catch (e) {
      setConfirmDelete(false);
      useUi.getState().showToast(errorText(toAppError(e), t));
    }
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: "overview", label: t("detail.tabOverview") },
    { id: "mods", label: t("detail.tabMods") },
    { id: "settings", label: t("detail.tabSettings") },
  ];

  return (
    <div className="flex flex-col gap-4">
      <Link to="/instances" className="self-start text-sm text-muted">
        ← {t("common.back")}
      </Link>
      <div className="flex flex-wrap items-center gap-4">
        <InstanceTile id={inst.id} name={inst.name} size={64} />
        <div className="min-w-0 flex-[1_1_240px]">
          <h1 className="m-0 text-2xl">{inst.name}</h1>
          <div className="text-muted">
            {inst.minecraft} · {loader}
          </div>
        </div>
        <div className="flex flex-wrap gap-2.5">
          <Button onClick={() => void getBackend().instanceOpenFolder(inst.id)}>{t("common.openFolder")}</Button>
          <Button variant="danger" onClick={() => setConfirmDelete(true)}>
            {t("common.delete")}
          </Button>
          <LaunchButton instance={inst} />
        </div>
      </div>
      <div role="tablist" className="flex flex-wrap border-b border-line">
        {tabs.map((x) => (
          <button
            key={x.id}
            type="button"
            role="tab"
            id={`tab-${x.id}`}
            aria-selected={tab === x.id}
            aria-controls={`panel-${x.id}`}
            onClick={() => setTab(x.id)}
            className={`mr-5.5 min-h-11 border-b-2 px-1 text-sm font-semibold ${tab === x.id ? "border-accent text-fg" : "border-transparent text-muted hover:text-fg"}`}
          >
            {x.label}
          </button>
        ))}
      </div>
      <div role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {tab === "overview" && (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3.5">
            <Stat label={t("detail.gameVersion")} value={inst.minecraft} />
            <Stat label={t("detail.loader")} value={loader} />
            <Stat label={t("detail.java")} value={inst.java.mode === "auto" ? t("detail.javaAuto") : t("detail.javaCustom")} sub={inst.java.mode === "auto" ? t("detail.javaAutoDesc") : inst.java.path} />
            <Stat label={t("detail.memory")} value={`${inst.memory_mb} MB`} />
          </div>
        )}
        {tab === "mods" &&
          (inst.loader ? <ModList instanceId={inst.id} /> : <div className="rounded-2xl border border-line bg-panel p-7 text-center text-muted">{t("detail.vanillaNoMods")}</div>)}
        {tab === "settings" && <InstanceSettings key={inst.id} instance={inst} />}
      </div>
      {confirmDelete && (
        <Modal title={t("detail.deleteTitle", { name: inst.name })} onClose={() => setConfirmDelete(false)} width={480}>
          <p className="m-0 text-muted">{t("detail.deleteBody")}</p>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setConfirmDelete(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="danger" onClick={() => void remove()}>
              {t("common.delete")}
            </Button>
          </div>
        </Modal>
      )}
    </div>
  );
}
