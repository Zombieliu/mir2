#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
duration_seconds="${MIR2_SOAK_DURATION_SECONDS:-7200}"
action_interval_ms="${MIR2_SOAK_ACTION_INTERVAL_MS:-5000}"
clients="${MIR2_SOAK_CLIENTS:-100}"
account_prefix="${MIR2_WS_LOAD_ACCOUNT_PREFIX:-}"
timestamp="$(date -u +"%Y%m%dT%H%M%SZ")"
report_path="${1:-docs/generated/load/platinum-176-soak-${clients}p-${timestamp}.json}"

if [[ -z "${account_prefix}" ]]; then
  echo "MIR2_WS_LOAD_ACCOUNT_PREFIX must identify pre-seeded reusable accounts" >&2
  exit 2
fi
if ! [[ "${duration_seconds}" =~ ^[1-9][0-9]*$ ]]; then
  echo "MIR2_SOAK_DURATION_SECONDS must be a positive integer" >&2
  exit 2
fi
if ! [[ "${action_interval_ms}" =~ ^[1-9][0-9]*$ ]]; then
  echo "MIR2_SOAK_ACTION_INTERVAL_MS must be a positive integer" >&2
  exit 2
fi
if ! [[ "${clients}" =~ ^[1-9][0-9]*$ ]]; then
  echo "MIR2_SOAK_CLIENTS must be a positive integer" >&2
  exit 2
fi

actions="$(( (duration_seconds * 1000 + action_interval_ms - 1) / action_interval_ms ))"

cd "${repo_root}/apps/web"
MIR2_WS_LOAD_OUT="${report_path}" \
MIR2_WS_LOAD_CLIENTS="${clients}" \
MIR2_WS_LOAD_POOL="${MIR2_WS_LOAD_POOL:-${clients}}" \
MIR2_WS_LOAD_ACTIONS="${actions}" \
MIR2_WS_LOAD_THINK_MS="${action_interval_ms}" \
MIR2_WS_LOAD_HOLD_OPEN_MS=0 \
MIR2_WS_LOAD_READY_TIMEOUT_MS="${MIR2_WS_LOAD_READY_TIMEOUT_MS:-300000}" \
MIR2_WS_LOAD_READY_BARRIER=1 \
MIR2_WS_LOAD_READY_BARRIER_TIMEOUT_MS="${MIR2_WS_LOAD_READY_BARRIER_TIMEOUT_MS:-300000}" \
MIR2_WS_LOAD_EXPECT_READY="${clients}" \
MIR2_WS_LOAD_EXPECT_REJECTED=0 \
MIR2_WS_LOAD_EXPECT_KEEPALIVE_ACK_RATIO="${MIR2_WS_LOAD_EXPECT_KEEPALIVE_ACK_RATIO:-1}" \
MIR2_WS_LOAD_EXPECT_KEEPALIVE_P95_MAX_MS="${MIR2_WS_LOAD_EXPECT_KEEPALIVE_P95_MAX_MS:-3000}" \
MIR2_WS_LOAD_REUSE_EXISTING_ACCOUNTS=1 \
MIR2_WS_LOAD_CHECKPOINT_MS="${MIR2_WS_LOAD_CHECKPOINT_MS:-300000}" \
node scripts/load-gateway-ws.mjs
