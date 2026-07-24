#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/gate17/gate17-acceptance.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from acceptance

jq -e \
  '.schemaVersion == 1
   and .success == true
   and all(.assertions[]; . == true)
   and .balances.aliceGold == 67
   and .balances.bobGold == 35
   and .balances.aliceSword == 1
   and .balances.bobSword == 0
   and .reconciliation.healthy == true' \
  "${output}" >/dev/null

echo "Gate17 acceptance written to ${output}"
