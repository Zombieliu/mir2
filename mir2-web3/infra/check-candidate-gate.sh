#!/usr/bin/env bash
set -euo pipefail

export CARGO_CACHE_AUTO_CLEAN_FREQUENCY="${CARGO_CACHE_AUTO_CLEAN_FREQUENCY:-never}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

SCOPE="${MIR2_CANDIDATE_SCOPE:-local}"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run_web_typecheck() {
  if [ -x apps/web/node_modules/.bin/tsc ]; then
    run apps/web/node_modules/.bin/tsc --noEmit --project apps/web/tsconfig.json
  else
    printf '\n==> skip player web typecheck (apps/web/node_modules/.bin/tsc is missing)\n'
  fi
}

run_web_movement_controller_tests() {
  run npm run test:movement-controller --prefix apps/web
}

run_web_minimap_transform_tests() {
  run npm run test:minimap-transform --prefix apps/web
}

run_web_resource_loading_tests() {
  run npm run test:resource-loading --prefix apps/web
}

run_player_web_build_if_available() {
  if command -v pwsh >/dev/null 2>&1 || command -v powershell >/dev/null 2>&1; then
    run npm run build --prefix apps/web
  else
    printf '\n==> skip player web build (PowerShell runtime build helper is unavailable)\n'
  fi
}

run_static_player_smokes() {
  run npm run smoke:crystal-minimap-assets --prefix apps/web
  if [ -n "${MIR2_WEB_BASE_URL:-}" ]; then
    run npm run smoke:crystal-map-api --prefix apps/web
  else
    printf '\n==> skip map API smoke (set MIR2_WEB_BASE_URL to a running player web URL)\n'
  fi
}

run_live_player_smokes() {
  if [ -z "${MIR2_WEB_BASE_URL:-}" ]; then
    echo "MIR2_CANDIDATE_SCOPE=live requires MIR2_WEB_BASE_URL" >&2
    exit 2
  fi
  if [ -z "${MIR2_GATEWAY_WS_URL:-}" ]; then
    echo "MIR2_CANDIDATE_SCOPE=live requires MIR2_GATEWAY_WS_URL" >&2
    exit 2
  fi
  run npm run smoke:crystal-map-api --prefix apps/web
  run npm run smoke:stage5-ui --prefix apps/web
  run npm run load:gateway-ws --prefix apps/web
}

case "$SCOPE" in
  local|full|live)
    ;;
  *)
    echo "Unsupported MIR2_CANDIDATE_SCOPE '$SCOPE'. Use local, full, or live." >&2
    exit 2
    ;;
esac

run bash infra/check-architecture-gates.sh
run cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1
run cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1
run_web_typecheck
run_web_movement_controller_tests
run_web_minimap_transform_tests
run_web_resource_loading_tests

if [ "$SCOPE" = "full" ] || [ "$SCOPE" = "live" ]; then
  run cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1
  run cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1
  run cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
  run npm run build --prefix apps/admin-web
  run_player_web_build_if_available
  run_static_player_smokes
fi

if [ "$SCOPE" = "live" ]; then
  run_live_player_smokes
fi

run git diff --check
