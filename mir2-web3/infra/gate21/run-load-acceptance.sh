#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
base_compose="${repo_root}/infra/gate19/docker-compose.yml"
gate21_compose="${script_dir}/docker-compose.yml"
evidence_dir="${repo_root}/docs/generated/regional"
output="${evidence_dir}/gate21-load.json"
resource_attestation="${evidence_dir}/gate21-resource-attestation.json"
sampler_pid=""

compose() {
  docker compose -f "${base_compose}" -f "${gate21_compose}" --profile acceptance "$@"
}

cleanup() {
  if [[ -n "${sampler_pid}" ]]; then
    kill "${sampler_pid}" >/dev/null 2>&1 || true
    wait "${sampler_pid}" >/dev/null 2>&1 || true
  fi
  compose down -v --remove-orphans >/dev/null 2>&1 || true
}

mkdir -p "${evidence_dir}"
"${script_dir}/preflight-reference.sh" "${resource_attestation}"
compose down -v --remove-orphans >/dev/null 2>&1 || true
trap cleanup EXIT

compose build gateway-1 zone-active zone-replicator regional-load
compose up -d \
  postgres-primary postgres-standby \
  redis-primary redis-replica-1 redis-replica-2 \
  redis-sentinel-1 redis-sentinel-2 redis-sentinel-3 \
  zone-active zone-active-2 zone-active-3 zone-active-4 \
  zone-active-5 zone-active-6 zone-active-7 zone-standby \
  gateway-1 gateway-2 gateway-3 zone-replicator

git_commit="$(git -C "${repo_root}" rev-parse HEAD)"
image_digest="$(
  docker image inspect mir2-gate21-regional-load:local --format '{{.Id}}'
)"
export MIR2_GIT_COMMIT="${git_commit}"
export MIR2_IMAGE_DIGEST="${image_digest}"
compose up -d regional-load

"${script_dir}/sample-stability.sh" &
sampler_pid="$!"
load_container="$(compose ps -q regional-load)"
if [[ -z "${load_container}" ]]; then
  echo "Gate 21 regional-load container was not created" >&2
  exit 1
fi
load_exit_code="$(docker wait "${load_container}")"
if [[ "${load_exit_code}" != "0" ]]; then
  compose logs --no-color regional-load >&2 || true
  exit 1
fi
wait "${sampler_pid}"
sampler_pid=""

resource_tmp="$(mktemp "${output}.resources.XXXXXX")"
jq --slurpfile resourceAttestation "${resource_attestation}" \
  '. + {referenceResourceAttestation: $resourceAttestation[0]}' \
  "${output}" >"${resource_tmp}"
mv "${resource_tmp}" "${output}"

jq -e '
  .schemaVersion == 1
  and .success == true
  and .profileId == "mir2-regional-v1"
  and .profileExact == true
  and .gitCommit == $gitCommit
  and .imageDigest == $imageDigest
  and .requestedPlayers == 3000
  and .connectedPlayers == 3000
  and .distinctAccounts == 3000
  and .distinctCharacters == 3000
  and .profileCatalogMaps == 700
  and .runtimeManifestMaps >= 120
  and .activeMapCount == 120
  and .activeZoneCount == 129
  and .hotMapFileName == "0"
  and .hotMapPlayers == 500
  and (.hotMapLinePlayers | length) == 10
  and all(.hotMapLinePlayers[]; . == 50)
  and .zoneHostSessionCount == 3000
  and .zoneHostActiveConnections <= 260
  and (.zoneHostActiveConnections * 4) < .zoneHostSessionCount
  and .requestedActiveDurationSeconds == 259200
  and .measuredActiveDurationMs >= 259200000
  and .latencyMs.p95 <= 200
  and .latencyMs.p99 <= 500
  and .resources.zoneRpcCodec == "msgpack"
  and .resources.zoneRpcSharedPoolSize == 256
  and .resources.zoneRpcQueueTimeoutMs == 500
  and .resources.cgroupCpuMax == "400000 100000"
  and .resources.cgroupMemoryMax == "8589934592"
  and .referenceResourceAttestation.success == true
  and all(.referenceResourceAttestation.assertions[]; . == true)
  and .workloadCommandCoverage >= 0.95
  and .errorRate <= 0.001
  and .promotion.success == true
  and .promotion.sessionRefreshCount == .promotion.zoneSessionCount
  and .promotion.postPromotionProbeCount == 3000
  and .economyDuplicateCount == 0
  and .economyRuntimeLedgerMismatchCount == 0
  and .economyReconciliation.healthy == true
  and all(.assertions[]; . == true)
' --arg gitCommit "${git_commit}" --arg imageDigest "${image_digest}" "${output}" >/dev/null

echo "Gate 21 exact 3,000-player/72-hour load evidence written to ${output}"
