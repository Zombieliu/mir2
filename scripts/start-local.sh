#!/usr/bin/env bash
# Start the mir2-web3 stack locally (gateway + player web) against the R2 CDN asset release.
# Usage: ./scripts/start-local.sh            # start both
#        ./scripts/start-local.sh --web-only # only the web client
#        ./scripts/start-local.sh --gateway-only
#        ./scripts/start-local.sh stop       # stop both
#        ./scripts/start-local.sh status     # show health of both
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

ASSET_BASE_URL="${MIR2_ASSET_BASE_URL:-https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1}"
WEB_PORT="${MIR2_WEB_PORT:-3002}"
GATEWAY_WEB_ADDR="${MIR2_GATEWAY_WEB_ADDR:-127.0.0.1:7110}"

# next.config only reads config/production-web-assets.json (injecting the
# MIR2_PINNED_* envs that enable the R2 full Crystal pack + map atlas) when
# VERCEL=1 && VERCEL_ENV=production. Simulate that so local dev also uses the
# full pack instead of falling back to per-frame PNG loading. NODE_ENV stays
# development, so hot reload and dev asset headers are unchanged.
load_pinned_env() {
  export VERCEL="1"
  export VERCEL_ENV="production"
}

do_web() {
  echo "[web] starting on http://127.0.0.1:${WEB_PORT} (assets: ${ASSET_BASE_URL})"
  cd apps/web
  env \
    NEXT_PUBLIC_MIR2_ASSET_BASE_URL="${ASSET_BASE_URL}" \
    MIR2_R2_PROXY_BASE="${ASSET_BASE_URL}" \
    MIR2_ORIGINAL_ASSET_MANIFEST_MODE=remote-release \
    MIR2_PINNED_ASSET_VERSION="${MIR2_PINNED_ASSET_VERSION:-}" \
    MIR2_PINNED_ASSET_OBJECT_PREFIX="${MIR2_PINNED_ASSET_OBJECT_PREFIX:-}" \
    MIR2_PINNED_ASSET_BASE_URL="${MIR2_PINNED_ASSET_BASE_URL:-}" \
    MIR2_PINNED_ASSET_BROWSER_FALLBACK_BASE_URLS="${MIR2_PINNED_ASSET_BROWSER_FALLBACK_BASE_URLS:-}" \
    MIR2_PINNED_CRYSTAL_FULL_PACK_ENABLED="${MIR2_PINNED_CRYSTAL_FULL_PACK_ENABLED:-}" \
    MIR2_PINNED_CRYSTAL_FULL_PACK_VERIFIED="${MIR2_PINNED_CRYSTAL_FULL_PACK_VERIFIED:-}" \
    MIR2_PINNED_CRYSTAL_FULL_PACK_PATH="${MIR2_PINNED_CRYSTAL_FULL_PACK_PATH:-}" \
    MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH="${MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH:-}" \
    MIR2_PINNED_CRYSTAL_FULL_PACK_LIBRARY_COUNT="${MIR2_PINNED_CRYSTAL_FULL_PACK_LIBRARY_COUNT:-}" \
    MIR2_PINNED_CRYSTAL_FULL_PACK_PAGE_COUNT="${MIR2_PINNED_CRYSTAL_FULL_PACK_PAGE_COUNT:-}" \
    MIR2_PINNED_MAP_ATLAS_ENABLED="${MIR2_PINNED_MAP_ATLAS_ENABLED:-}" \
    MIR2_PINNED_MAP_ATLAS_VERIFIED="${MIR2_PINNED_MAP_ATLAS_VERIFIED:-}" \
    MIR2_PINNED_MAP_ATLAS_MANIFEST_PATH="${MIR2_PINNED_MAP_ATLAS_MANIFEST_PATH:-}" \
    MIR2_PINNED_MAP_ATLAS_CONTENT_HASH="${MIR2_PINNED_MAP_ATLAS_CONTENT_HASH:-}" \
    MIR2_PINNED_MAP_ATLAS_PAGE_COUNT="${MIR2_PINNED_MAP_ATLAS_PAGE_COUNT:-}" \
    MIR2_PINNED_MAP_ATLAS_MAX_PAGE_BYTES="${MIR2_PINNED_MAP_ATLAS_MAX_PAGE_BYTES:-}" \
    nohup npx next dev -p "${WEB_PORT}" >/tmp/mir2-web.log 2>&1 &
  echo $! > /tmp/mir2-web.pid
}

do_gateway() {
  echo "[gateway] starting on tcp:7000 + http:${GATEWAY_WEB_ADDR}"
  env \
    MIR2_ALLOW_DEV_IDENTITY_SECRETS=1 \
    MIR2_GATEWAY_WEB_ADDR="${GATEWAY_WEB_ADDR}" \
    nohup ./target/debug/mir2-gateway >/tmp/mir2-gateway.log 2>&1 &
  echo $! > /tmp/mir2-gateway.pid
}

stop_all() {
  [ -f /tmp/mir2-web.pid ] && kill "$(cat /tmp/mir2-web.pid)" 2>/dev/null || true
  [ -f /tmp/mir2-gateway.pid ] && kill "$(cat /tmp/mir2-gateway.pid)" 2>/dev/null || true
  pkill -f 'next dev' 2>/dev/null || true
  pkill -f 'mir2-gateway' 2>/dev/null || true
  echo "[done] stopped"
}

status() {
  echo -n "[web]   http://127.0.0.1:${WEB_PORT}/      -> "
  curl -s -o /dev/null -w "%{http_code}\n" --max-time 5 "http://127.0.0.1:${WEB_PORT}/" 2>&1 || echo "down"
  echo -n "[gw]    http://${GATEWAY_WEB_ADDR}/health  -> "
  curl -s -o /dev/null -w "%{http_code}\n" --max-time 5 "http://${GATEWAY_WEB_ADDR}/health" 2>&1 || echo "down"
  echo -n "[ws]    ws://${GATEWAY_WEB_ADDR}/ws        -> "
  curl -s -o /dev/null -w "%{http_code}\n" --max-time 5 \
    -H "Connection: Upgrade" -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
    "http://${GATEWAY_WEB_ADDR}/ws" 2>&1 || echo "down"
}

case "${1:-start}" in
  start)        load_pinned_env; do_gateway; do_web; status ;;
  --web-only)   load_pinned_env; do_web; status ;;
  --gateway-only) load_pinned_env; do_gateway; status ;;
  stop)         stop_all ;;
  status)       status ;;
  *)            echo "Usage: $0 [start|--web-only|--gateway-only|stop|status]" >&2; exit 2 ;;
esac
