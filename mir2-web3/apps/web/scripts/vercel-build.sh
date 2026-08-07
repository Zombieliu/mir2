#!/usr/bin/env bash
set -euo pipefail

# Cost guard: only the Production branch (main) runs the full build. Preview /
# feature-branch deployments exit here, BEFORE the expensive Rust toolchain +
# wasm-bindgen + Bevy/Next compile, so parallel branches don't burn Build CPU
# minutes. Set MIR2_FORCE_PREVIEW_BUILD=1 to force a preview build when needed.
# The clean, complete fix is the Vercel project "Ignored Build Step"
# (Settings -> Git), which skips the build entirely; this guard cuts cost even
# without it. No-op locally (VERCEL is unset), and production (main) is unaffected.
if [ "${VERCEL:-}" = "1" ] && [ "${VERCEL_ENV:-}" != "production" ] \
  && [ "${MIR2_FORCE_PREVIEW_BUILD:-}" != "1" ]; then
  echo "[vercel-build] VERCEL_ENV=${VERCEL_ENV:-unset} is not 'production' — skipping preview build to save Build CPU minutes (set MIR2_FORCE_PREVIEW_BUILD=1 to override)."
  exit 0
fi

# Production asset identity is versioned with the code. This deliberately
# overrides stale dashboard variables so a main-branch deployment cannot point
# at an older R2 prefix than the commit that triggered it.
if [ "${VERCEL:-}" = "1" ] && [ "${VERCEL_ENV:-}" = "production" ]; then
  PRODUCTION_ASSET_CONFIG="$(pwd)/../../config/production-web-assets.json"
  if [ ! -f "$PRODUCTION_ASSET_CONFIG" ]; then
    echo "[vercel-build] Missing $PRODUCTION_ASSET_CONFIG." >&2
    exit 1
  fi

  export MIR2_ASSET_VERSION="$(
    node -p "require(process.argv[1]).version" "$PRODUCTION_ASSET_CONFIG"
  )"
  export MIR2_ASSET_OBJECT_PREFIX="$(
    node -p "require(process.argv[1]).objectPrefix" "$PRODUCTION_ASSET_CONFIG"
  )"
  export NEXT_PUBLIC_MIR2_ASSET_BASE_URL="$(
    node -p "require(process.argv[1]).assetBaseUrl" "$PRODUCTION_ASSET_CONFIG"
  )"
  export MIR2_ORIGINAL_ASSET_MANIFEST_MODE="remote-release"
  export MIR2_ORIGINAL_ASSET_REMOTE_RELEASE="$(
    node -p "require(process.argv[1]).remoteReleaseUrl" "$PRODUCTION_ASSET_CONFIG"
  )"
  export MIR2_USE_PREBUILT_BEVY_RUNTIME="${MIR2_USE_PREBUILT_BEVY_RUNTIME:-1}"
  export MIR2_REUSE_ORIGINAL_ASSET_MANIFEST="${MIR2_REUSE_ORIGINAL_ASSET_MANIFEST:-1}"

  npm run generate:original-asset-manifest
  echo "[vercel-build] Using production R2 assets $MIR2_ASSET_VERSION."
fi

TOOLCHAIN="${RUST_TOOLCHAIN_VERSION:-1.89.0}"
export PATH="$HOME/.cargo/bin:$PATH"

RUNTIME_LOCK="$(pwd)/../game-client/runtime/Cargo.lock"
PREBUILT_RUNTIME_WEBGPU_JS="$(pwd)/public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js"
PREBUILT_RUNTIME_WEBGPU_WASM="$(pwd)/public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm"
PREBUILT_RUNTIME_WEBGL2_JS="$(pwd)/public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js"
PREBUILT_RUNTIME_WEBGL2_WASM="$(pwd)/public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime_bg.wasm"

prebuilt_runtime_ready() {
  [ -s "$PREBUILT_RUNTIME_WEBGPU_JS" ] \
    && [ -s "$PREBUILT_RUNTIME_WEBGPU_WASM" ] \
    && [ -s "$PREBUILT_RUNTIME_WEBGL2_JS" ] \
    && [ -s "$PREBUILT_RUNTIME_WEBGL2_WASM" ]
}

# Hosted production releases pin immutable WebGPU/WebGL2 packages that were
# already built and uploaded by the asset release gate. Do not install a Rust
# toolchain merely because the source tree is present in the deployment
# archive; `npm run runtime:build:release` validates and reuses these packages.
if [ "${MIR2_USE_PREBUILT_BEVY_RUNTIME:-0}" = "1" ]; then
  if ! prebuilt_runtime_ready; then
    echo "[vercel-build] Prebuilt runtime is absent from Git; the pinned package will be fetched and verified during npm build."
  fi
  echo "[vercel-build] Using pinned prebuilt WebGPU/WebGL2 packages; skipping Rust toolchain installation."
  npm run build
  exit 0
fi

if [ ! -f "$RUNTIME_LOCK" ]; then
  if prebuilt_runtime_ready; then
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
