#!/usr/bin/env bash
# Regenerate the TypeScript types from Rust and fail if they differ from git.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test -q -p cairn-core --lib export_bindings >/dev/null
cargo test -q -p cairn-app --lib export_bindings >/dev/null
git diff --exit-code -- ui/src/bindings
test -z "$(git status --porcelain -- ui/src/bindings)" || { git status --porcelain -- ui/src/bindings; echo "new binding files not committed"; exit 1; }
# Every binding must import its siblings only; a "../" path means an export dir mismatch.
if grep -rn 'from "\.\./' ui/src/bindings; then echo "bindings import outside ui/src/bindings"; exit 1; fi
echo "ok: ui/src/bindings is up to date"
