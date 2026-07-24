#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/regional/gate18-remote-economy.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from remote-economy-acceptance \
  remote-economy-acceptance

jq -e \
   '.schemaVersion == 1
   and .success == true
   and all(.assertions[]; . == true)
   and .zoneHostEndpoint == "zone-host:7020"
   and .zoneHostId == "gate18-regional-owner"
   and .initialGold == .fixtureGold
   and .goldAfterDrop == (.initialGold - 25)
   and .goldAfterPickup == .initialGold
   and .goldAfterRetry == .goldAfterPickup
   and .bootstrapOpeningGold == .initialGold
   and .ledgerGoldAfterDrop == .goldAfterDrop
   and .ledgerGoldAfterPickup == .goldAfterPickup
   and .ledgerGoldAfterRetry == .ledgerGoldAfterPickup
   and .fixtureItemKey == "red-potion"
   and .initialItemQuantity == .fixtureItemQuantity
   and .itemQuantityAfterDrop == (.initialItemQuantity - 2)
   and .itemQuantityAfterPickup == .initialItemQuantity
   and .itemQuantityAfterRetry == .itemQuantityAfterPickup
   and .bootstrapOpeningItemQuantity == .initialItemQuantity
   and .ledgerItemQuantityAfterDrop == .itemQuantityAfterDrop
   and .ledgerItemQuantityAfterPickup == .itemQuantityAfterPickup
   and .ledgerItemQuantityAfterRetry == .ledgerItemQuantityAfterPickup' \
  "${output}" >/dev/null

echo "Gate 18 remote economy evidence written to ${output}"
