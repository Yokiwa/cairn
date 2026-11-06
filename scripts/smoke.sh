#!/usr/bin/env bash
# Real end-to-end check: install Minecraft with a mod loader and start it until
# the game has built its block texture atlas (resources loaded, renderer up),
# using a virtual display and software GL.
# Needs network access to Mojang and the loader's servers, Xvfb (xvfb-run) and Mesa.
#   MC=1.20.1 LOADER=fabric bash scripts/smoke.sh       (defaults)
#   MC=1.20.1 LOADER=forge  bash scripts/smoke.sh
#   MC=1.21.1 LOADER=neoforge bash scripts/smoke.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -q -p cairn-cli
export CAIRN_HOME="${CAIRN_HOME:-$(mktemp -d)}"
export LIBGL_ALWAYS_SOFTWARE=1
MC="${MC:-1.20.1}"
LOADER="${LOADER:-fabric}"
NAME="smoke-${LOADER}-${MC//./-}"
CAIRN=./target/debug/cairn
echo "data dir: $CAIRN_HOME"
"$CAIRN" instance list | grep -q "^$NAME " || "$CAIRN" instance create "$NAME" --mc "$MC" --loader "$LOADER" --memory 3072
xvfb-run -a -s "-screen 0 1280x720x24" \
  "$CAIRN" instance launch "$NAME" --offline Smoke --smoke-test "${SMOKE_PATTERN:-Created: .*minecraft:textures/atlas/blocks}" --timeout "${SMOKE_TIMEOUT:-420}"
