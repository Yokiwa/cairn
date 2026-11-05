# Contributing to Cairn

Thanks for helping! This page covers setup, layout, tests and translations.

## Setup

- Rust stable (`rust-toolchain.toml` picks it), Node.js 22, pnpm 10
- Linux: `sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
- Java 17+ with `javac` (only for the Forge installer tests; they skip without it)

```bash
pnpm -C ui install
```

| Task | Command |
|---|---|
| Desktop app with hot reload | `node ui/node_modules/@tauri-apps/cli/tauri.js dev` |
| UI only, in a browser (sample data) | `pnpm -C ui dev` |
| Command line | `cargo run -p cairn-cli -- --help` |
| Installers | `node ui/node_modules/@tauri-apps/cli/tauri.js build` |

Set `CAIRN_HOME=/some/dir` to keep test data away from your real launcher folder.

## Layout

```
crates/core/   all launcher logic (no UI): downloads, versions, Java, loaders, instances, accounts, launch
crates/cli/    the `cairn` command line
src-tauri/     desktop shell: Tauri commands and events over cairn-core
ui/            React + TypeScript interface; ui/src/bindings is generated from Rust
docs/          design spec, implementation plans, setup guides, screenshots
scripts/       CI helpers and the live smoke test
```

In the browser the UI talks to `ui/src/lib/mockBackend.ts`; inside the app it uses `tauriBackend.ts`. Both implement `Backend` in `ui/src/lib/backend.ts`.

## Tests

| What | Command |
|---|---|
| Rust | `cargo test --workspace` |
| UI unit tests | `pnpm -C ui test` |
| End-to-end (browser, mock backend) | `pnpm -C ui e2e` |
| Real downloads (network, ~1.5 GB) | `cargo test -p cairn-core --test live -- --ignored --test-threads=1` |
| Real launch to the title screen (Linux, Xvfb) | `bash scripts/smoke.sh` |
| Lints | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm -C ui lint`, `pnpm -C ui typecheck` |

Write the failing test first, then the code. CI runs all but the network tests on Windows, macOS and Linux.

## Types shared with the UI

Rust types the UI uses derive `ts_rs::TS`. After changing one, regenerate and commit:

```bash
bash scripts/check-bindings.sh
```

## Translations

UI text lives in `ui/src/locales/en.json` and `zh-CN.json`; both files must have the same keys (a test checks this). Every `AppError::new("error.…")` in Rust needs a message under `errors` in both files — `node scripts/check-i18n-keys.mjs` checks it. To add a language, copy `en.json`, translate it, and register it in `ui/src/lib/i18n.ts`.

## Commits

Small commits with a type prefix: `feat(core): …`, `fix(ui): …`, `test: …`, `docs: …`, `ci: …`.
By contributing you agree your work is licensed under GPL-3.0.
