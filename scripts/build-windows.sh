#!/usr/bin/env bash
# Cross-compile the Windows installer from Linux.
#
# Requires: lld, llvm, clang, nsis, the x86_64-pc-windows-msvc rust target and
# cargo-xwin. Produces an NSIS setup executable; .msi would need a Windows host.
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-msvc

command -v cargo-xwin >/dev/null || { echo "missing cargo-xwin: cargo install --locked cargo-xwin"; exit 1; }
rustup target list --installed | grep -q "$TARGET" || rustup target add "$TARGET"

echo "→ renderer"
npm --prefix ui install
npm --prefix ui run build

echo "→ avatar manifest"
node scripts/fetch-avatar.mjs || true

echo "→ native ($TARGET)"
npm run tauri -- build --runner cargo-xwin --target "$TARGET"

echo
echo "installer:"
find target/x86_64-pc-windows-msvc/release/bundle -name '*.exe' -o -name '*.msi' 2>/dev/null | sed 's/^/  /'
