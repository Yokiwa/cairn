import { useState } from "react";
import { useNavigate } from "react-router";
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { LoaderKind } from "../bindings/LoaderKind";
import { getBackend, toAppError } from "../lib/backend";
import { javaHint } from "../lib/format";
import { LOADER_KINDS, LOADER_NAMES } from "../lib/instances";
import { useUi } from "../lib/store";
import type { AppError } from "../bindings/AppError";
import Button from "./Button";
import ErrorMessage from "./ErrorMessage";
import Modal from "./Modal";
import Switch from "./Switch";

type Choice = LoaderKind | "none";

export default function NewInstanceWizard({ onClose }: { onClose(): void }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [step, setStep] = useState(1);
  const [snapshots, setSnapshots] = useState(false);
  const [mc, setMc] = useState<string | null>(null);
  const [loader, setLoader] = useState<Choice>("none");
  const [name, setName] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);

  const versions = useQuery({ queryKey: ["versions", snapshots], queryFn: () => getBackend().versionsList(snapshots) });
  const loaderLists = useQueries({
    queries: LOADER_KINDS.map((kind) => ({
      queryKey: ["loaderVersions", kind, mc],
      queryFn: () => getBackend().loaderVersions(kind, mc!),
      enabled: step >= 2 && mc !== null,
    })),
  });
  const versionsOf = (kind: LoaderKind) => loaderLists[LOADER_KINDS.indexOf(kind)];
  const pickedVersion = (() => {
    if (loader === "none") return null;
    const list = versionsOf(loader).data ?? [];
    return (list.find((v) => v.stable) ?? list[0])?.version ?? null;
  })();

  const loaderLabel = loader === "none" ? t("common.vanilla") : LOADER_NAMES[loader];
  const defaultName = `${loader === "none" ? "Vanilla" : LOADER_NAMES[loader]} ${mc ?? ""}`;
  const finalName = (name ?? defaultName).trim();
  const java = mc ? (javaHint(mc) ?? t("wizard.javaAuto")) : "";

  const canNext = step === 1 ? mc !== null : step === 2 ? loader === "none" || pickedVersion !== null : finalName.length > 0;
  const steps = [t("wizard.stepVersion"), t("wizard.stepLoader"), t("wizard.stepName")];

  async function create() {
    setBusy(true);
    setError(null);
    try {
      const task = await getBackend().instanceCreate({
        name: finalName,
        minecraft: mc!,
        loader: loader === "none" ? null : { kind: loader, version: pickedVersion! },
        memory_mb: null,
      });
      useUi.getState().trackTask(task, "install");
      await qc.invalidateQueries({ queryKey: ["instances"] });
      onClose();
      navigate("/instances");
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal title={t("wizard.title")} onClose={onClose}>
      <ol className="m-0 flex list-none flex-wrap gap-2 p-0">
        {steps.map((label, i) => (
          <li key={label} aria-current={i + 1 === step ? "step" : undefined} className="flex min-w-[120px] flex-1 flex-col gap-1.5">
            <div className={`h-1 rounded-full ${i + 1 <= step ? "bg-accent" : "bg-line-strong"}`} />
            <div className={`text-xs font-semibold ${i + 1 <= step ? "text-fg" : "text-muted"}`}>
              {i + 1} {label}
            </div>
          </li>
        ))}
      </ol>

      {step === 1 && (
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="font-semibold">{t("wizard.pickVersion")}</div>
            <label className="flex items-center gap-2 text-[13px] text-muted">
              <Switch checked={snapshots} onChange={setSnapshots} label={t("wizard.showSnapshots")} />
              <span aria-hidden="true">{t("wizard.showSnapshots")}</span>
            </label>
          </div>
          {versions.isLoading && <div className="text-muted">{t("common.loading")}</div>}
          {versions.error && <ErrorMessage error={toAppError(versions.error)} />}
          <div className="grid max-h-[300px] grid-cols-[repeat(auto-fill,minmax(130px,1fr))] gap-2 overflow-y-auto">
            {versions.data?.map((v) => (
              <Button key={v.id} variant={mc === v.id ? "primary" : "default"} aria-pressed={mc === v.id} onClick={() => setMc(v.id)} className="font-mono">
                {v.id}
              </Button>
            ))}
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="flex flex-col gap-3">
          <div className="font-semibold">{t("wizard.pickLoader")}</div>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-2">
            {(["none", ...LOADER_KINDS] as Choice[]).map((kind) => {
              const q = kind === "none" ? null : versionsOf(kind);
              const unavailable = q !== null && q.isSuccess && q.data.length === 0;
              const selected = loader === kind;
              return (
                <button
                  key={kind}
                  type="button"
                  aria-pressed={selected}
                  disabled={unavailable}
                  onClick={() => setLoader(kind)}
                  className={`flex flex-col gap-1 rounded-2xl border bg-panel p-3.5 text-left text-fg transition disabled:cursor-not-allowed disabled:opacity-50 ${selected ? "border-accent" : "border-line hover:border-line-strong"}`}
                >
                  <span className="font-bold">{kind === "none" ? t("wizard.none") : LOADER_NAMES[kind]}</span>
                  <span className="text-xs text-muted">{unavailable ? t("wizard.notAvailable", { mc }) : t(`wizard.loaderDesc.${kind}`)}</span>
                </button>
              );
            })}
          </div>
          {pickedVersion && <div className="text-xs text-muted">{t("wizard.loaderAuto", { version: pickedVersion })}</div>}
        </div>
      )}

      {step === 3 && (
        <div className="flex flex-col gap-3.5">
          <label className="flex flex-col gap-1.5 font-semibold">
            {t("wizard.name")}
            <input
              value={name ?? defaultName}
              onChange={(e) => setName(e.target.value)}
              maxLength={64}
              className="min-h-11 rounded-[10px] border border-line-strong bg-bg px-3 font-normal text-fg"
            />
          </label>
          <div className="flex flex-col gap-2 rounded-2xl border border-line bg-bg p-4">
            <div className="font-semibold">{t("wizard.willDownload")}</div>
            <div className="text-[13px]">{t("wizard.summary", { mc, loader: loaderLabel, java })}</div>
            <div className="text-xs text-muted">{t("wizard.willDownloadNote")}</div>
          </div>
          {error && <ErrorMessage error={error} />}
        </div>
      )}

      <div className="flex flex-wrap justify-between gap-2.5">
        <Button variant="ghost" onClick={() => (step === 1 ? onClose() : setStep(step - 1))}>
          {step === 1 ? t("common.cancel") : t("common.back")}
        </Button>
        <Button variant="primary" disabled={!canNext || busy} onClick={() => (step < 3 ? setStep(step + 1) : void create())}>
          {step < 3 ? t("common.next") : t("common.create")}
        </Button>
      </div>
    </Modal>
  );
}
