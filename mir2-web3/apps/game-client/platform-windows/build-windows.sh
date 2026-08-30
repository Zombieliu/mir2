#!/bin/bash
# Cross-compile gate for the native Windows host.
#
# Requires (macOS host):
#   rustup target add --toolchain 1.95.0 x86_64-pc-windows-gnu
#   brew install mingw-w64
#
# On a Windows host simply run:
#   cargo build --manifest-path apps/game-client/platform-windows/Cargo.toml

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

TOOLCHAIN="${MIR2_CLIENT_TOOLCHAIN:-1.95.0}"
HOST_OS="$(uname -s)"
CARGO_HOME_DIR="${CARGO_HOME:-${HOME}/.cargo}"
RUSTUP_HOME_DIR="${RUSTUP_HOME:-${HOME}/.rustup}"
export RUSTFLAGS="${RUSTFLAGS:+${RUSTFLAGS} }--remap-path-prefix=${ROOT}=. --remap-path-prefix=${CARGO_HOME_DIR}=.cargo --remap-path-prefix=${RUSTUP_HOME_DIR}=.rustup"

if [[ "${HOST_OS}" == "Darwin" ]]; then
  export PATH="$(brew --prefix mingw-w64)/bin:${PATH}"
  export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc"
fi

echo "[platform-windows] cross-compiling to x86_64-pc-windows-gnu (${TOOLCHAIN})"
cargo "+${TOOLCHAIN}" build \
  --manifest-path apps/game-client/platform-windows/Cargo.toml \
  --target x86_64-pc-windows-gnu \
  --release

BINARY="apps/game-client/platform-windows/target/x86_64-pc-windows-gnu/release/mir2-platform-windows.exe"
DIST="apps/game-client/platform-windows/dist/windows-x86_64"
mkdir -p "$DIST"
cp "$BINARY" "$DIST/mir2-platform-windows.exe"
echo "[platform-windows] installing deterministic Web asset-generator dependencies"
npm --prefix apps/web ci
apps/game-client/platform-windows/package-assets.sh "$DIST/mir2-assets"

echo "[platform-windows] gate passed: $DIST"
