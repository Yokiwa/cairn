#!/usr/bin/env bash
# Regenerate the TypeScript types from Rust and fail if they differ from git.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test -q -p cairn-core --lib export_bindings >/dev/null
cargo test -q -p cairn-app --lib export_bindings >/dev/null
git diff --exit-code -- ui/src/bindings
test -z "$(git status --porcelain -- ui/src/bindings)" || { git status --porcelain -- ui/src/bindings; echo "new binding files not committed"; exit 1; }
echo "ok: ui/src/bindings is up to date"
