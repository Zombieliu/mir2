#!/usr/bin/env bash
set -euo pipefail

TOOLCHAIN="${RUST_TOOLCHAIN_VERSION:-1.89.0}"
export PATH="$HOME/.cargo/bin:$PATH"

RUNTIME_LOCK="$(pwd)/../game-client/runtime/Cargo.lock"
PREBUILT_RUNTIME_WEBGPU_JS="$(pwd)/public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js"
PREBUILT_RUNTIME_WEBGPU_WASM="$(pwd)/public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm"
PREBUILT_RUNTIME_WEBGL2_JS="$(pwd)/public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js"
PREBUILT_RUNTIME_WEBGL2_WASM="$(pwd)/public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime_bg.wasm"

if [ ! -f "$RUNTIME_LOCK" ]; then
  if [ -s "$PREBUILT_RUNTIME_WEBGPU_JS" ] && [ -s "$PREBUILT_RUNTIME_WEBGPU_WASM" ] && [ -s "$PREBUILT_RUNTIME_WEBGL2_JS" ] && [ -s "$PREBUILT_RUNTIME_WEBGL2_WASM" ]; then
    echo "[vercel-build] Rust runtime source is not present in this deployment archive; using prebuilt WebGPU/WebGL2 packages."
    export MIR2_USE_PREBUILT_BEVY_RUNTIME=1
    npm run build
    exit 0
  fi

  echo "[vercel-build] Missing $RUNTIME_LOCK and no prebuilt Bevy runtime packages were found." >&2
  exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$TOOLCHAIN"
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

rustup toolchain install "$TOOLCHAIN" --profile minimal
rustup target add wasm32-unknown-unknown --toolchain "$TOOLCHAIN"

WASM_BINDGEN_VERSION="$(node -e 'const fs=require("node:fs");const path=require("node:path");const lockPath=path.resolve(process.cwd(),"..","game-client","runtime","Cargo.lock");const lockText=fs.readFileSync(lockPath,"utf8");for(const block of lockText.split(/\n(?=\[\[package\]\]\n)/)){if(!block.includes("name = \"wasm-bindgen\""))continue;const version=block.match(/version = "([^"]+)"/)?.[1];if(version){process.stdout.write(version);process.exit(0);}}throw new Error(`wasm-bindgen package not found in ${lockPath}`);')"

CURRENT_WASM_BINDGEN_VERSION="$(wasm-bindgen --version 2>/dev/null | sed -E 's/.* ([0-9]+\.[0-9]+\.[0-9]+).*/\1/' || true)"
if [ "$CURRENT_WASM_BINDGEN_VERSION" != "$WASM_BINDGEN_VERSION" ]; then
  cargo +"$TOOLCHAIN" install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked
fi

npm run build
