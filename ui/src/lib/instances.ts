import type { Instance } from "../bindings/Instance";
import type { LoaderKind } from "../bindings/LoaderKind";

export const LOADER_NAMES: Record<LoaderKind, string> = { fabric: "Fabric", quilt: "Quilt", forge: "Forge", neoforge: "NeoForge" };
export const LOADER_KINDS: LoaderKind[] = ["fabric", "quilt", "forge", "neoforge"];

const TILES = ["#6EE7B7", "#FDBA74", "#93C5FD", "#C4B5FD", "#FCD34D", "#F9A8D4"];

export function tileColor(id: string): string {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return TILES[h % TILES.length];
}

export function initial(name: string): string {
  return [...name.trim()][0]?.toUpperCase() ?? "?";
}

/** Most recently played first, then most recently created. */
export function byRecent(list: Instance[]): Instance[] {
  const key = (i: Instance) => Math.max(i.last_played_at ? Date.parse(i.last_played_at) : 0, Date.parse(i.created_at) / 1e6);
  return [...list].sort((a, b) => key(b) - key(a));
}
