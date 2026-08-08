#!/usr/bin/env bash
# Convenient local mir2-web iteration stack with a SAME-ORIGIN R2 asset proxy.
#
# The problem this solves: a localhost web build is a different origin from the
# R2 asset host (mir2.obelisk.build), which sends no Access-Control-Allow-Origin,
# so the browser CORS-blocks every R2-only asset (the ~156k uncovered map tiles,
# sprites, sounds) and the client retry-storms them -> a "stutter every ~2s".
# The `rewrites()` fallback in next.config.ts (gated on MIR2_R2_PROXY_BASE)
# proxies those assets through THIS origin, so the browser is same-origin with
# its assets: no CORS, no 404 storm, getImageData works. Production is untouched
# (it serves assets same-origin already and never sets MIR2_R2_PROXY_BASE).
#
# Usage (run from apps/web):
#   ./scripts/local-iterate.sh dev     # :3070 HMR — edit code, see changes live
#   ./scripts/local-iterate.sh prod    # :3080 optimized build — judge REAL perf
#
# Then open, pointing at the live gateway:
#   http://localhost:3070/?gatewayWs=ws://127.0.0.1:7141/ws    (dev)
#   http://localhost:3080/?gatewayWs=ws://127.0.0.1:7141/ws    (prod)
#
# IMPORTANT: judge render-perf / "卡" on the PROD build, never `next dev` — the
# dev React build's jsxDEV is ~25% of main-thread time (dev ~73 hitches/30s walk
# vs prod ~0). Use dev only for functional iteration. Measure the named per-frame
# cost with: node ./scripts/qa-cpu-profile.mjs --baseUrl <prod-url>
set -euo pipefail
cd "$(dirname "$0")/.."  # -> apps/web

# Same-origin R2 proxy base (the next.config rewrite reads this).
export MIR2_R2_PROXY_BASE="${MIR2_R2_PROXY_BASE:-https://mir2.obelisk.build}"
# Deliberately NOT setting NEXT_PUBLIC_MIR2_ASSET_BASE_URL: the client must
# request LOCAL asset paths (so the proxy fallback can serve them same-origin),
# not absolute cross-origin R2 URLs.

if [ ! -e node_modules ]; then
  echo "node_modules is missing. In a git worktree, symlink it from the main checkout:" >&2
  echo "  ln -s /ABS/PATH/TO/mir2-web3/apps/web/node_modules node_modules" >&2
  exit 1
fi
if [ ! -f public/original-asset-manifest.generated.json ]; then
  echo "WARNING: public/original-asset-manifest.generated.json missing -> /api/asset-manifest 500s" >&2
  echo "  (it is a generated file; copy it from a built main checkout or run the asset generator)" >&2
fi
# Turbopack cannot follow the worktree node_modules symlink ("points out of the
# filesystem root") -> always use --webpack.
case "${1:-dev}" in
  prod)
    npx next build --webpack
    exec npx next start -p "${PORT:-3080}"
    ;;
  dev)
    exec npx next dev --webpack -p "${PORT:-3070}"
    ;;
  *)
    echo "usage: $0 [dev|prod]" >&2
    exit 2
    ;;
esac
