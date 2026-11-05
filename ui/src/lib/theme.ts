import { useEffect } from "react";
import type { Settings } from "../bindings/Settings";

export const ACCENTS = [
  { hex: "#34D399", name: "green" },
  { hex: "#7DD3FC", name: "sky" },
  { hex: "#FDBA74", name: "orange" },
  { hex: "#C4B5FD", name: "violet" },
  { hex: "#F9A8D4", name: "pink" },
] as const;

function systemDark(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function resolvedTheme(theme: Settings["theme"] | undefined): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  return systemDark() ? "dark" : "light";
}

/** Apply the theme and accent from settings to <html>. */
export function useApplyTheme(settings: Settings | undefined) {
  useEffect(() => {
    const root = document.documentElement;
    const apply = () => {
      root.dataset.theme = resolvedTheme(settings?.theme);
      root.style.setProperty("--accent", settings?.accent ?? "#34D399");
    };
    apply();
    if (settings?.theme !== "system" || typeof window.matchMedia !== "function") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [settings?.theme, settings?.accent]);
}
