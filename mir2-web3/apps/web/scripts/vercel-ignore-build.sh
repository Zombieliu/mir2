#!/usr/bin/env bash
#
# Vercel "Ignored Build Step" — skip the (expensive Bevy-WASM + Next) production
# build when a push to main changed nothing the web bundle actually depends on.
#
# Wire it up ONCE: Vercel project → Settings → Git → Ignored Build Step →
#   bash scripts/vercel-ignore-build.sh
# (path is relative to the project Root Directory, mir2-web3/apps/web).
#
# Vercel convention (counter-intuitive but correct):
#   exit 0  -> SKIP the build; keep the current production deployment live.
#   exit 1  -> PROCEED with the build.
#
# Result: merging Rust/docs PRs (simulation, gateway, admin, docs, .github,
# Crystal, packages …) to main no longer burns a full Bevy/Next rebuild — the
# deploy is simply skipped. Only changes under the web bundle's real inputs
# rebuild. No need to manually "batch" non-web PRs.
#
# The web build consumes EXACTLY these trees (verified against
# apps/web/scripts/vercel-build.sh + build-bevy-runtime.mjs + package.json):
#   - mir2-web3/apps/web/                 the Next app, its public/ assets, lockfile
#   - mir2-web3/apps/game-client/runtime/ the Bevy runtime Rust source -> WASM
# (apps/web has no workspace `packages/` deps, so nothing else can affect it.)
# git always reports repo-root-relative paths, so these prefixes are correct
# regardless of the Root Directory the command runs from.
set -uo pipefail

WATCH=(
  "mir2-web3/apps/web/"
  "mir2-web3/apps/game-client/runtime/"
)

# Escape hatch: force a build regardless (e.g. asset pipeline / env change).
if [ "${MIR2_FORCE_VERCEL_BUILD:-}" = "1" ]; then
  echo "[ignore-build] MIR2_FORCE_VERCEL_BUILD=1 — building."
  exit 1
fi

# Safety first: if we can't determine the diff (first commit / shallow clone),
# build rather than risk shipping a stale production bundle.
if ! git rev-parse HEAD^ >/dev/null 2>&1; then
  echo "[ignore-build] no previous commit reachable — building to be safe."
  exit 1
fi

changed="$(git diff --name-only HEAD^ HEAD 2>/dev/null || true)"
if [ -z "$changed" ]; then
  echo "[ignore-build] could not compute changed files — building to be safe."
  exit 1
fi

for prefix in "${WATCH[@]}"; do
  if printf '%s\n' "$changed" | grep -q "^${prefix}"; then
    echo "[ignore-build] changes under ${prefix} — building."
    exit 1
  fi
done

echo "[ignore-build] no changes under web build inputs — skipping Vercel build."
printf '[ignore-build] (changed files this push:)\n%s\n' "$changed" | sed 's/^/    /'
exit 0
