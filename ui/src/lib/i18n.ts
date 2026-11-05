import i18n, { type TFunction } from "i18next";
import { initReactI18next } from "react-i18next";
import en from "../locales/en.json";
import zh from "../locales/zh-CN.json";
import type { AppError } from "../bindings/AppError";

export type Lang = "en" | "zh-CN";

/** The person's choice wins; otherwise any Chinese system language → zh-CN, else English. */
export function detectLanguage(navigatorLanguage: string, setting?: string | null): Lang {
  const pick = setting || navigatorLanguage || "en";
  return pick.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, "zh-CN": { translation: zh } },
  lng: detectLanguage(typeof navigator !== "undefined" ? navigator.language : "en"),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
  returnNull: false,
});

const tables: Record<Lang, Record<string, string>> = { en: en.errors, "zh-CN": zh.errors };

/** Human text for an AppError in the current language (error keys contain dots, so look them up directly). */
export function errorText(e: AppError, t: TFunction): string {
  const lang: Lang = i18n.language === "zh-CN" ? "zh-CN" : "en";
  const template = tables[lang][e.key] ?? tables.en[e.key];
  if (!template) return e.detail ? `${t("common.unknownError")} (${e.detail})` : t("common.unknownError");
  return template.replace(/\{\{(\w+)\}\}/g, (_, k: string) => e.params[k] ?? "");
}

export default i18n;
