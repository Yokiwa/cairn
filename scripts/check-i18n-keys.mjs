#!/usr/bin/env node
// Every error key the Rust code can produce must have an English and a
// Chinese message in ui/src/locales/*.json (under "errors").
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// fileURLToPath, not URL.pathname: on Windows the latter gives "/D:/..." which is not a path.
const root = fileURLToPath(new URL("..", import.meta.url));
const keys = new Set(["error.unknown"]); // produced by the UI itself
function walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (name === "target" || name === "node_modules" || name === "tests") continue;
    if (statSync(p).isDirectory()) walk(p);
    else if (name.endsWith(".rs")) {
      // Unit tests live after #[cfg(test)]; their placeholder keys don't ship.
      const code = readFileSync(p, "utf8").split("#[cfg(test)]")[0];
      for (const m of code.matchAll(/AppError::new\("([^"]+)"\)/g)) keys.add(m[1]);
    }
  }
}
walk(join(root, "crates"));
walk(join(root, "src-tauri", "src"));

let missing = 0;
for (const lang of ["en", "zh-CN"]) {
  const errors = JSON.parse(readFileSync(join(root, "ui", "src", "locales", `${lang}.json`), "utf8")).errors ?? {};
  for (const k of [...keys].sort()) {
    if (!errors[k]) {
      console.error(`missing ${lang} message for ${k}`);
      missing++;
    }
  }
}
if (missing) process.exit(1);
console.log(`ok: ${keys.size} error keys translated in en and zh-CN`);
