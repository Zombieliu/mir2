#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/regional/gate18-economy-producer.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from economy-producer-acceptance \
  economy-producer-acceptance

jq -e \
  '.schemaVersion == 1
   and .success == true
   and all(.assertions[]; . == true)
   and .ledgerGoldAfter == 25
   and .activeGoldAfter == (.activeGoldBefore + 25)
   and .standbyGoldAfter == (.standbyGoldBefore + 25)' \
  "${output}" >/dev/null

echo "Gate 18 economy producer evidence written to ${output}"
