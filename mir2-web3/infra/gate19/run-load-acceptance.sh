#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
output="${repo_root}/docs/generated/regional/gate19-load.json"

mkdir -p "$(dirname "${output}")"
docker compose -f "${compose_file}" --profile acceptance down -v --remove-orphans \
  >/dev/null 2>&1 || true
trap 'docker compose -f "${compose_file}" --profile acceptance down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker compose -f "${compose_file}" build \
  zone-active \
  zone-standby \
  gateway-1 \
  infra-probe \
  regional-load
docker compose -f "${compose_file}" --profile acceptance up -d \
  postgres-primary \
  postgres-standby \
  zone-active \
  zone-standby

git_commit="$(git -C "${repo_root}" rev-parse HEAD)"
image_digest="$(
  docker image inspect mir2-gate19-regional-load:local --format '{{.Id}}'
)"
docker compose -f "${compose_file}" --profile acceptance run --rm --no-deps \
  -e MIR2_GIT_COMMIT="${git_commit}" \
  -e MIR2_IMAGE_DIGEST="${image_digest}" \
  regional-load

jq -e '
  .schemaVersion == 1
  and .success == true
  and .profileId == "mir2-regional-v1"
  and .profileExact == true
  and .gitCommit == $gitCommit
  and .imageDigest == $imageDigest
  and .requestedPlayers == 500
  and .connectedPlayers == 500
  and .distinctAccounts == 500
  and .distinctCharacters == 500
  and .activeZoneCount == 120
  and .requestedActiveDurationSeconds == 3600
  and .measuredActiveDurationMs >= 3600000
  and .workloadCommandCoverage >= 0.95
  and .errorRate <= 0.001
  and .promotion.activeOwner == "gate19-active"
  and .promotion.standbyOwner == "gate19-standby"
  and .promotion.success == true
  and .economyDuplicateCount == 0
  and .economyRuntimeLedgerMismatchCount == 0
  and .economyReconciliation.healthy == true
  and all(.assertions[]; . == true)
' --arg gitCommit "${git_commit}" --arg imageDigest "${image_digest}" "${output}" \
  >/dev/null

echo "Gate 19 exact one-hour load evidence written to ${output}"
