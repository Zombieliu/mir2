#!/bin/bash
# Build gate for the Mir2 Tauri desktop launcher (macOS / Windows / Linux).
#
# Usage:
#   ./build-desktop.sh            # host platform bundle
#   ./build-desktop.sh windows    # cross-compile Windows (needs mingw-w64)
#   ./build-desktop.sh linux      # cross-compile Linux (needs linux GNU toolchain)
#
# Requirements:
#   - Rust toolchain 1.95.0 (matching the game-client crates)
#   - Node + @tauri-apps/cli (npm install in this directory)
#   - Windows: brew install mingw-w64 + rustup target x86_64-pc-windows-gnu
#   - Linux: a Linux host or full cross sysroot (WebKitGTK system libs)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LAUNCHER="$ROOT/apps/mir2-launcher-tauri"
TOOLCHAIN="${MIR2_CLIENT_TOOLCHAIN:-1.95.0}"
TARGET="${1:-host}"

echo "[launcher] npm deps"
(cd "$LAUNCHER" && npm ci >/dev/null 2>&1)

case "${TARGET}" in
  host)
    echo "[launcher] building host bundle (${TOOLCHAIN})"
    (cd "$LAUNCHER" && npx tauri build --bundles app)
    ;;
  windows)
    export PATH="$(brew --prefix mingw-w64)/bin:${PATH}"
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc"
    echo "[launcher] cross-compiling Windows (${TOOLCHAIN})"
    (cd "$LAUNCHER" && npx tauri build --target x86_64-pc-windows-gnu --bundles nsis)
    ;;
  linux)
    echo "[launcher] cross-compiling Linux (${TOOLCHAIN})"
    echo "NOTE: requires a Linux sysroot with WebKitGTK; run on Linux CI for a real gate."
    (cd "$LAUNCHER" && cargo "+${TOOLCHAIN}" check --manifest-path src-tauri/Cargo.toml --target x86_64-unknown-linux-gnu)
    ;;
  *)
    echo "unknown target: ${TARGET}" >&2
    exit 1
    ;;
esac

echo "[launcher] ${TARGET} gate passed"
