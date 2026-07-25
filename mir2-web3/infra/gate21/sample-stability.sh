#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
base_compose="${repo_root}/infra/gate19/docker-compose.yml"
gate21_compose="${script_dir}/docker-compose.yml"
evidence_dir="${repo_root}/docs/generated/regional"
raw_output="${evidence_dir}/gate21-stability-samples.jsonl"
summary_output="${evidence_dir}/gate21-stability.json"
interval_seconds="${MIR2_GATE21_STABILITY_SAMPLE_SECONDS:-300}"

compose() {
  docker compose -f "${base_compose}" -f "${gate21_compose}" --profile acceptance "$@"
}

mkdir -p "${evidence_dir}"
temporary="$(mktemp "${raw_output}.XXXXXX")"
trap 'rm -f "${temporary}"' EXIT

load_container="$(compose ps -q regional-load)"
if [[ -z "${load_container}" ]]; then
  echo "Gate 21 regional-load container is not running" >&2
  exit 1
fi

active_start_ms=""
for _ in $(seq 1 18000); do
  marker="$(compose logs --no-color regional-load 2>&1 | rg 'REGIONAL_LOAD_ACTIVE_STARTED gate=21' | tail -1 || true)"
  active_start_ms="$(sed -n 's/.* at_ms=\([0-9][0-9]*\) .*/\1/p' <<<"${marker}")"
  [[ -n "${active_start_ms}" ]] && break
  state="$(docker inspect -f '{{.State.Status}}' "${load_container}" 2>/dev/null || true)"
  if [[ "${state}" == "exited" || "${state}" == "dead" ]]; then
    compose logs --no-color regional-load >&2 || true
    exit 1
  fi
  sleep 0.1
done
if [[ -z "${active_start_ms}" ]]; then
  echo "Gate 21 load did not publish its active-start marker" >&2
  exit 1
fi

reference_services=(
  gateway-1 gateway-2 gateway-3
  zone-active zone-active-2 zone-active-3 zone-active-4
  zone-active-5 zone-active-6 zone-active-7 zone-standby
  postgres-primary postgres-standby
  redis-primary redis-replica-1 redis-replica-2
)

sample_once() {
  local sampled_at_ms reference_memory_bytes replicator_memory_bytes wal_bytes
  local service container memory_bytes
  sampled_at_ms="$(( $(date +%s) * 1000 ))"
  reference_memory_bytes=0
  containers='{}'
  for service in "${reference_services[@]}"; do
    container="$(compose ps -q "${service}")"
    if [[ -z "${container}" ]]; then
      echo "Gate 21 reference service ${service} is missing" >&2
      return 1
    fi
    memory_bytes="$(docker exec "${container}" /bin/sh -ec 'cat /sys/fs/cgroup/memory.current')"
    reference_memory_bytes="$(( reference_memory_bytes + memory_bytes ))"
    containers="$(
      jq -c --arg service "${service}" --argjson memoryBytes "${memory_bytes}" \
        '. + {($service): $memoryBytes}' <<<"${containers}"
    )"
  done
  replicator_container="$(compose ps -q zone-replicator)"
  if [[ -z "${replicator_container}" ]]; then
    echo "Gate 21 zone-replicator is missing" >&2
    return 1
  fi
  replicator_memory_bytes="$(
    docker exec "${replicator_container}" /bin/sh -ec \
      'cat /sys/fs/cgroup/memory.current'
  )"
  wal_bytes="$(
    docker exec "${replicator_container}" /bin/sh -ec \
      "find /var/lib/obelisk/zone-replication -type f -exec stat -c '%s' {} + | awk '{total += \$1} END {print total + 0}'"
  )"
  jq -cn \
    --argjson sampledAtMs "${sampled_at_ms}" \
    --argjson activeStartMs "${active_start_ms}" \
    --argjson referenceMemoryBytes "${reference_memory_bytes}" \
    --argjson replicatorMemoryBytes "${replicator_memory_bytes}" \
    --argjson walBytes "${wal_bytes}" \
    --argjson containers "${containers}" \
    '{
      sampledAtMs: $sampledAtMs,
      activeStartMs: $activeStartMs,
      elapsedMs: ($sampledAtMs - $activeStartMs),
      referenceMemoryBytes: $referenceMemoryBytes,
      replicatorMemoryBytes: $replicatorMemoryBytes,
      walBytes: $walBytes,
      containers: $containers
    }' >>"${temporary}"
}

while true; do
  sample_once
  state="$(docker inspect -f '{{.State.Status}}' "${load_container}" 2>/dev/null || true)"
  if [[ "${state}" == "exited" || "${state}" == "dead" || -z "${state}" ]]; then
    break
  fi
  sleep "${interval_seconds}"
done

mv "${temporary}" "${raw_output}"
trap - EXIT
python3 "${script_dir}/summarize-stability.py" \
  --samples "${raw_output}" \
  --load "${evidence_dir}/gate21-load.json" \
  --output "${summary_output}" \
  --sample-interval-seconds "${interval_seconds}"
