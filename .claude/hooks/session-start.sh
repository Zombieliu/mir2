#!/bin/bash
# Mir2 — Claude Code on the web SessionStart hook.
#
# Prepares the environment so the gate's `cargo +1.89.0 test` and the web
# `npm run smoke:* / test:* / assets:verify` scripts (plus `tsc`) run without
# per-session setup. No-op outside the remote web environment.
#
# Resilient by design: individual steps warn but don't abort the session start
# (deps are mostly cached/present); warnings are printed so real failures show.
set -uo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

ROOT="${CLAUDE_PROJECT_DIR:-$(pwd)}"
log() { echo "[mir2 session-start] $*"; }

# 1. Rust 1.89.0 toolchain (the gate pins it) + wasm32 target (web bevy runtime build).
if command -v rustup >/dev/null 2>&1; then
  log "ensure rust 1.89.0 + wasm32 target"
  rustup toolchain install 1.89.0 --profile minimal          || log "WARN: toolchain install failed"
  rustup target add wasm32-unknown-unknown --toolchain 1.89.0 || log "WARN: wasm32 target add failed"
fi

# 2. Prime the cargo dependency cache so the first `cargo +1.89.0 test` doesn't fetch.
if [ -f "$ROOT/mir2-web3/Cargo.toml" ]; then
  log "fetch cargo deps (mir2-web3 workspace)"
  ( cd "$ROOT/mir2-web3" && cargo +1.89.0 fetch --locked ) \
    || ( cd "$ROOT/mir2-web3" && cargo +1.89.0 fetch ) \
    || log "WARN: cargo fetch failed (continuing)"
fi

# 3. Web npm deps — player web + admin web (mirrors the candidate gate).
for app in apps/web apps/admin-web; do
  dir="$ROOT/mir2-web3/$app"
  [ -f "$dir/package-lock.json" ] || continue
  log "npm ci ($app)"
  ( cd "$dir" && npm ci ) || ( cd "$dir" && npm install ) \
    || log "WARN: npm install failed for $app (continuing)"
done

log "ready — rust 1.89 + wasm32, cargo deps fetched, web deps installed"
exit 0
