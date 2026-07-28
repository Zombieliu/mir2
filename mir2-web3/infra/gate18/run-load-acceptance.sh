#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/regional/gate18-load.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from load-acceptance \
  load-acceptance

jq -e \
  '.schemaVersion == 1
   and .success == true
   and .profileId == "mir2-regional-v1"
   and .profileExact == true
   and .requestedPlayers == 500
   and .connectedPlayers == 500
   and .distinctAccounts == 500
   and .distinctCharacters == 500
   and .profileCatalogMaps == 700
   and .runtimeManifestMaps >= 120
   and .activeZoneCount == 120
   and .requestedActiveDurationSeconds == 1800
   and .promotionPauseExcludedFromActiveDuration == true
   and .roles.movement == 300
   and .roles.combat == 75
   and .roles.social == 25
   and .roles.economy == 25
   and .roles.idle == 75
   and .workloadCommandCoverage >= 0.95
   and .errorRate <= 0.001
   and .promotion.activeOwner == "gate18-active"
   and .promotion.standbyOwner == "gate18-standby"
   and .promotion.zoneId == "map:0"
   and .promotion.zoneSessionCount == 30
   and .promotion.standbyGeneration > .promotion.activeGeneration
   and .promotion.sessionRefreshCount == .promotion.zoneSessionCount
   and .promotion.postPromotionProbeCount == 500
   and .promotion.success == true
   and .economyDuplicateCount == 0
   and .economyRuntimeLedgerMismatchCount == 0
   and .economyReconciliation.healthy == true
   and all(.assertions[]; . == true)' \
  "${output}" >/dev/null

echo "Gate 18 mixed-load evidence written to ${output}"
