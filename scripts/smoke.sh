#!/usr/bin/env bash
# Real end-to-end check: install Minecraft 1.20.1 + Fabric and start it until
# the game has built its block texture atlas (resources loaded, renderer up),
# using a virtual display and software GL.
# Needs network access to Mojang/Fabric, Xvfb (xvfb-run) and Mesa.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -q -p cairn-cli
export CAIRN_HOME="${CAIRN_HOME:-$(mktemp -d)}"
export LIBGL_ALWAYS_SOFTWARE=1
CAIRN=./target/debug/cairn
echo "data dir: $CAIRN_HOME"
"$CAIRN" instance list | grep -q '^smoke ' || "$CAIRN" instance create smoke --mc 1.20.1 --loader fabric --memory 2048
xvfb-run -a -s "-screen 0 1280x720x24" \
  "$CAIRN" instance launch smoke --offline Smoke --smoke-test "${SMOKE_PATTERN:-Created: .*minecraft:textures/atlas/blocks}" --timeout "${SMOKE_TIMEOUT:-300}"
