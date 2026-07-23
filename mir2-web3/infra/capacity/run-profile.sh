#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
profile_name="${1:-2c2g-5mbps-100gb}"
profile_file="${script_dir}/profiles/${profile_name}.env"

if [[ ! -f "${profile_file}" ]]; then
  echo "unknown capacity profile: ${profile_name}" >&2
  exit 1
fi

set -a
# shellcheck disable=SC1090
source "${profile_file}"
set +a

required=(
  DUBHE_PROFILE_LABEL
  DUBHE_PROFILE_CPU_CORES
  DUBHE_PROFILE_MEMORY_BYTES
  DUBHE_PROFILE_NETWORK_EGRESS_MBPS
  DUBHE_PROFILE_DISK_BYTES
  DUBHE_PROFILE_SAFETY_BPS
  MIR2_LOAD_STEPS
  MIR2_LOAD_TICKS
  MIR2_LOAD_BUDGET_MS
  MIR2_LOAD_COMMAND_INTERVAL_MS
  MIR2_LOAD_ZONES
  MIR2_LOAD_ZONE_PLAYER_STEPS
  MIR2_LOAD_ZONE_TICKS
)
for name in "${required[@]}"; do
  if [[ -z "${!name:-}" ]]; then
    echo "${name} is required in ${profile_file}" >&2
    exit 1
  fi
done

output_dir="${repo_root}/docs/generated/capacity/${profile_name}"
output_file="${output_dir}/latest.json"
image="mir2-capacity-benchmark:${profile_name}"
container_name="dubhe-capacity-${profile_name}"
mkdir -p "${output_dir}"

docker build \
  --target capacity-benchmark \
  --tag "${image}" \
  "${repo_root}"

docker run --rm \
  --name "${container_name}" \
  --cpus "${DUBHE_PROFILE_CPU_CORES}" \
  --memory "${DUBHE_PROFILE_MEMORY_BYTES}" \
  --memory-swap "${DUBHE_PROFILE_MEMORY_BYTES}" \
  --pids-limit 512 \
  --network none \
  --read-only \
  --tmpfs /tmp:size=64m,mode=1777 \
  --user "$(id -u):$(id -g)" \
  --volume "${output_dir}:/evidence" \
  --env DUBHE_PROFILE_LABEL \
  --env DUBHE_PROFILE_CPU_CORES \
  --env DUBHE_PROFILE_MEMORY_BYTES \
  --env DUBHE_PROFILE_NETWORK_EGRESS_MBPS \
  --env DUBHE_PROFILE_DISK_BYTES \
  --env DUBHE_PROFILE_SAFETY_BPS \
  --env MIR2_LOAD_STEPS \
  --env MIR2_LOAD_TICKS \
  --env MIR2_LOAD_BUDGET_MS \
  --env MIR2_LOAD_COMMAND_INTERVAL_MS \
  --env MIR2_LOAD_ZONES \
  --env MIR2_LOAD_ZONE_PLAYER_STEPS \
  --env MIR2_LOAD_ZONE_TICKS \
  --env MIR2_LOAD_OUT=/evidence/latest.json \
  "${image}"

jq -e \
  --arg label "${DUBHE_PROFILE_LABEL}" \
  --arg cpu_cores "${DUBHE_PROFILE_CPU_CORES}" \
  --arg memory_bytes "${DUBHE_PROFILE_MEMORY_BYTES}" \
  '.schemaVersion == 1
    and .build == "release"
    and .hardware.label == $label
    and .hardware.requestedCpuCores == $cpu_cores
    and .hardware.requestedMemoryBytes == ($memory_bytes | tonumber)
    and .hardware.cgroupCpuMax == (($cpu_cores | tonumber) * 100000 | tostring) + " 100000"
    and .hardware.cgroupMemoryMax == $memory_bytes
    and .hardware.availableParallelism == ($cpu_cores | tonumber)
    and (.singleZone | length) > 0
    and (.multiZone | length) > 0
    and all(.singleZone[]; .packetEncodeErrors == 0 and .rssAfterBytes != null)
    and all(.multiZone[]; .packetEncodeErrors == 0)
    and .recommendation.maxTestedCombinedPlayers != null
    and .recommendation.maxTestedCombinedTotalPlayers != null' \
  "${output_file}" >/dev/null

echo "Capacity profile written to ${output_file}"
