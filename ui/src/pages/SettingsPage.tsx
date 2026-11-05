import { useEffect, useId, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { Settings } from "../bindings/Settings";
import AccountList from "../components/AccountList";
import Button from "../components/Button";
import MemorySlider from "../components/MemorySlider";
import Pill from "../components/Pill";
import { getBackend, toAppError } from "../lib/backend";
import { detectLanguage, errorText } from "../lib/i18n";
import { useUi } from "../lib/store";
import { ACCENTS } from "../lib/theme";

function Section({ title, id, children }: { title: string; id?: string; children: ReactNode }) {
  return (
    <section id={id} aria-label={title} className="flex flex-col gap-4 rounded-2xl border border-line bg-panel p-5">
      <h2 className="m-0 text-base">{title}</h2>
      {children}
    </section>
  );
}

function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2.5">
      <div>
        <div className="font-semibold">{title}</div>
        {hint && <div className="text-xs text-muted">{hint}</div>}
      </div>
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

export default function SettingsPage() {
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const threadsId = useId();
  const settings = useQuery({ queryKey: ["settings"], queryFn: () => getBackend().settingsGet() });
  const info = useQuery({ queryKey: ["system"], queryFn: () => getBackend().systemInfo() });
  const [update, setUpdate] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    if (window.location.hash === "#accounts") document.getElementById("accounts")?.scrollIntoView();
  }, []);

  async function save(patch: Partial<Settings>) {
    if (!settings.data) return;
    const next = { ...settings.data, ...patch };
    qc.setQueryData(["settings"], next);
    if (patch.language !== undefined) void i18n.changeLanguage(detectLanguage(navigator.language, patch.language));
    try {
      await getBackend().settingsSet(next);
    } catch (e) {
      useUi.getState().showToast(errorText(toAppError(e), t));
    }
    await qc.invalidateQueries({ queryKey: ["settings"] });
  }

  async function checkUpdate() {
    try {
      setUpdate(await getBackend().updateCheck());
    } catch (e) {
      useUi.getState().showToast(errorText(toAppError(e), t));
    }
  }

  const s = settings.data;
  const lang = detectLanguage(navigator.language, s?.language);
  return (
    <div className="flex max-w-[760px] flex-col gap-4">
      <h1 className="m-0 text-[26px]">{t("settings.title")}</h1>
      {s && (
        <>
          <Section title={t("settings.theme")}>
            <Row title={t("settings.language")} hint={t("settings.languageHint")}>
              <Pill on={lang === "zh-CN"} onClick={() => void save({ language: "zh-CN" })} lang="zh-CN">
                中文
              </Pill>
              <Pill on={lang === "en"} onClick={() => void save({ language: "en" })} lang="en">
                English
              </Pill>
            </Row>
            <Row title={t("settings.theme")}>
              <Pill on={s.theme === "system"} onClick={() => void save({ theme: "system" })}>
                {t("settings.themeSystem")}
              </Pill>
              <Pill on={s.theme === "light"} onClick={() => void save({ theme: "light" })}>
                {t("settings.themeLight")}
              </Pill>
              <Pill on={s.theme === "dark"} onClick={() => void save({ theme: "dark" })}>
                {t("settings.themeDark")}
              </Pill>
            </Row>
            <Row title={t("settings.accent")}>
              {ACCENTS.map((a) => (
                <button
                  key={a.hex}
                  type="button"
                  aria-label={t(`settings.accentNames.${a.name}`)}
                  aria-pressed={s.accent === a.hex}
                  onClick={() => void save({ accent: a.hex })}
                  className="grid min-h-11 min-w-11 place-items-center rounded-full"
                >
                  <span className="block h-7 w-7 rounded-full border-[3px]" style={{ background: a.hex, borderColor: s.accent === a.hex ? "var(--color-fg)" : "transparent" }} />
                </button>
              ))}
            </Row>
          </Section>
          <Section title={t("downloads.title")}>
            <div className="flex flex-col gap-1">
              <label htmlFor={threadsId} className="font-semibold">
                {t("settings.threads")}: {s.download_threads}
              </label>
              <input
                id={threadsId}
                type="range"
                min={1}
                max={64}
                value={s.download_threads}
                onChange={(e) => qc.setQueryData(["settings"], { ...s, download_threads: Number(e.target.value) })}
                onPointerUp={(e) => void save({ download_threads: Number((e.target as HTMLInputElement).value) })}
                onKeyUp={(e) => void save({ download_threads: Number((e.target as HTMLInputElement).value) })}
                className="w-full accent-[var(--color-accent)]"
              />
              <div className="text-xs text-muted">{t("settings.threadsHint")}</div>
            </div>
            {info.data && (
              <MemorySlider
                label={t("settings.defaultMemory")}
                value={s.default_memory_mb ?? info.data.recommended_memory_mb}
                totalMb={info.data.total_memory_mb}
                recommendedMb={info.data.recommended_memory_mb}
                onChange={(mb) => void save({ default_memory_mb: mb })}
              />
            )}
          </Section>
        </>
      )}
      <Section title={t("settings.accounts")} id="accounts">
        <AccountList />
      </Section>
      <Section title={t("settings.about")}>
        <div>{t("settings.version", { version: info.data?.app_version ?? "" })}</div>
        {info.data?.updates_enabled && (
          <div className="flex flex-wrap items-center gap-2.5">
            <Button onClick={() => void checkUpdate()}>{t("settings.checkUpdates")}</Button>
            {update === null && <span className="text-sm text-muted">{t("settings.upToDate")}</span>}
            {update && (
              <>
                <span className="text-sm">{t("settings.updateAvailable", { version: update })}</span>
                <Button variant="primary" onClick={() => void getBackend().updateInstall()}>
                  {t("settings.installUpdate")}
                </Button>
              </>
            )}
          </div>
        )}
        <a href="https://github.com/Yokiwa/cairn" onClick={(e) => (e.preventDefault(), void getBackend().openUrl("https://github.com/Yokiwa/cairn"))}>
          {t("settings.github")}
        </a>
        <div className="text-xs text-muted">{t("settings.license")}</div>
      </Section>
    </div>
  );
}
