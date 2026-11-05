import type { Progress } from "../bindings/Progress";

/** 0–100, from bytes when known, else from files. */
export function percent(p: Progress): number {
  if (p.total_bytes > 0) return Math.min(100, Math.floor((p.done_bytes / p.total_bytes) * 100));
  if (p.total_files > 0) return Math.min(100, Math.floor((p.done_files / p.total_files) * 100));
  return 0;
}

export function mbPerSec(bytesPerSec: number): string {
  return (bytesPerSec / 1048576).toFixed(1);
}

function cmp(a: string, b: string): number {
  const n = (s: string) => s.split(/[-+ ]/)[0].split(".").map((x) => parseInt(x, 10) || 0);
  const [x, y] = [n(a), n(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d) return d;
  }
  return 0;
}

/** Java the game will most likely need, for display only (the backend reads the real requirement). */
export function javaHint(mc: string): number | null {
  if (cmp(mc, "26") >= 0) return null; // year-based versions: shown as "auto"
  if (cmp(mc, "1.20.5") >= 0) return 21;
  if (cmp(mc, "1.18") >= 0) return 17;
  if (cmp(mc, "1.17") >= 0) return 16;
  return 8;
}

export function relativeTime(iso: string | null, lang: string): string | null {
  if (!iso) return null;
  const diff = (new Date(iso).getTime() - Date.now()) / 1000;
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  const abs = Math.abs(diff);
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
  if (abs < 86400 * 30) return rtf.format(Math.round(diff / 86400), "day");
  return rtf.format(Math.round(diff / (86400 * 30)), "month");
}
