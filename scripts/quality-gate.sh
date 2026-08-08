#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo +1.89.0 fmt --check \
  -p mir2-simulation \
  -p mir2-gateway \
  -p mir2-admin-api

cargo +1.89.0 check --locked \
  -p mir2-simulation \
  -p mir2-gateway \
  -p mir2-admin-api

(
  cd apps/web
  npm run typecheck
)

if [ -d apps/admin-web/node_modules ]; then
  (
    cd apps/admin-web
    npm run typecheck
  )
fi

git diff --check

if [ "${MIR2_QUALITY_FULL:-0}" = "1" ]; then
  cargo +1.89.0 test --locked -p mir2-gateway -p mir2-simulation -- --test-threads=1
fi
