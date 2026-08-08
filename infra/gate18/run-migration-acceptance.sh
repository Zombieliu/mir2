#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/regional/gate18-migrations.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from migration-acceptance \
  migration-acceptance

jq -e \
  '.schemaVersion == 1
   and .success == true
   and .concurrentWorkers == 16
   and .successfulWorkers == .concurrentWorkers
   and .appliedMigrationCount == .expectedMigrationCount
   and .requiredRelationsPresent == true' \
  "${output}" >/dev/null

echo "Gate 18 concurrent migration evidence written to ${output}"
