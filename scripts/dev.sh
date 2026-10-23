#!/usr/bin/env bash
# Run Yura with hot reload.
#
# Starts Vite (renderer) and Tauri (native window) together. On a Linux dev box
# this needs the webkit2gtk development packages; cross-compiling to Windows
# does not.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ ! -d node_modules ]; then
  echo "→ installing root dependencies"
  npm install
fi

if [ ! -d ui/node_modules ]; then
  echo "→ installing renderer dependencies"
  npm --prefix ui install
fi

exec npm run tauri:dev
