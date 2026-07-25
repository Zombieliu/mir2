#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/regional/gate18-gameplay.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from gameplay-acceptance \
  gameplay-acceptance

jq -e \
  '.schemaVersion == 1
   and .success == true
   and all(.assertions[]; . == true)
   and .zoneHostEndpoint == "zone-host:7020"
   and .zoneHostId == "gate18-regional-owner"
   and .startingZoneId == "map:0"
   and .handoffZoneId == "map:1"
   and .returnedZoneId == "map:0"
   and .groupMessageObserved == true
   and .guildMessageObserved == true
   and .deathObserved == true
   and .reviveObserved == true
   and .monsterDeathObserved == true
   and .experienceAfterKill > .experienceBeforeKill
   and .ledgerExperienceAfterKill == .experienceAfterKill
   and .itemQuantityAfterPickup
       == (.itemQuantityBeforePickup + .droppedItemQuantity)
   and .itemQuantityAfterRetry == .itemQuantityAfterPickup
   and .ledgerItemQuantityAfterPickup == .itemQuantityAfterPickup
   and .ledgerItemQuantityAfterRetry == .ledgerItemQuantityAfterPickup' \
  "${output}" >/dev/null

echo "Gate 18 gameplay evidence written to ${output}"
