#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"

profile_label="${GATE16_PROFILE_LABEL:-2c2g}"
cpu_cores="${GATE16_PROFILE_CPU_CORES:-2}"
memory_bytes="${GATE16_PROFILE_MEMORY_BYTES:-2147483648}"
player_profiles="${MIR2_GATE16_PLAYER_PROFILES:-50,125}"
history_steps="${MIR2_GATE16_HISTORY_STEPS:-700,10000,100000}"
output_dir="${repo_root}/docs/generated/gate16"
output_file="${output_dir}/v5-certification.json"
image="mir2-gate16-v5-certification:${profile_label}"
container_name="mir2-gate16-v5-certification-${profile_label}"

mkdir -p "${output_dir}"

docker build \
  --target gate16-v5-certification \
  --tag "${image}" \
  "${repo_root}"

docker run --rm \
  --name "${container_name}" \
  --cpus "${cpu_cores}" \
  --memory "${memory_bytes}" \
  --memory-swap "${memory_bytes}" \
  --pids-limit 1024 \
  --network none \
  --read-only \
  --tmpfs /tmp:size=256m,mode=1777 \
  --user "$(id -u):$(id -g)" \
  --volume "${output_dir}:/evidence" \
  --env GATE16_PROFILE_LABEL="${profile_label}" \
  --env GATE16_PROFILE_CPU_CORES="${cpu_cores}" \
  --env GATE16_PROFILE_MEMORY_BYTES="${memory_bytes}" \
  --env MIR2_GATE16_PLAYER_PROFILES="${player_profiles}" \
  --env MIR2_GATE16_HISTORY_STEPS="${history_steps}" \
  --env MIR2_GATE16_CERTIFICATION_OUT=/evidence/v5-certification.json \
  "${image}"

jq -e \
  --arg profile_label "${profile_label}" \
  --arg cpu_cores "${cpu_cores}" \
  --arg memory_bytes "${memory_bytes}" \
  --arg player_profiles "${player_profiles}" \
  --arg history_steps "${history_steps}" \
  '.schemaVersion == 1
    and .build == "release"
    and .environment.profileLabel == $profile_label
    and .environment.requestedCpuCores == $cpu_cores
    and .environment.requestedMemoryBytes == $memory_bytes
    and .success == true
    and .assertions.playerProfilesPass == true
    and .assertions.historyProfilesPass == true
    and .assertions.networkReductionAtLeast80Percent == true
    and .assertions.cpuReductionAtLeast80Percent == true
    and .assertions.wallReductionAtLeast80Percent == true
    and (.playerResults | map(.requestedPlayers)) == ($player_profiles | split(",") | map(tonumber))
    and (.historyResults | map(.historyEntries)) == ($history_steps | split(",") | map(tonumber))
    and all(.historyResults[];
      .deltaEntries == 64
      and .headsMatch == true
      and .networkReductionPercent >= 80
      and .cpuReductionPercent >= 80
      and .wallReductionPercent >= 80)' \
  "${output_file}" >/dev/null

echo "Gate16 v5 certification written to ${output_file}"
