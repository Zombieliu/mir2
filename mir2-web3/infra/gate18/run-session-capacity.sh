#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"

profile_label="${GATE18_PROFILE_LABEL:-gate18-2c2g-120zones}"
cpu_cores="${GATE18_PROFILE_CPU_CORES:-2}"
memory_bytes="${GATE18_PROFILE_MEMORY_BYTES:-2147483648}"
players="${MIR2_GATE18_PLAYERS:-500}"
zone_count="${MIR2_GATE18_ZONE_COUNT:-120}"
history_steps="${MIR2_GATE18_HISTORY_STEPS:-700}"
output_dir="${repo_root}/docs/generated/regional"
output_file="${output_dir}/gate18-500-session-120zones.json"
image="mir2-gate18-session-capacity:${profile_label}"
container_name="mir2-gate18-session-capacity-${profile_label}"

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
  --pids-limit 2048 \
  --network none \
  --read-only \
  --tmpfs /tmp:size=256m,mode=1777 \
  --user "$(id -u):$(id -g)" \
  --volume "${output_dir}:/evidence" \
  --env GATE16_PROFILE_LABEL="${profile_label}" \
  --env GATE16_PROFILE_CPU_CORES="${cpu_cores}" \
  --env GATE16_PROFILE_MEMORY_BYTES="${memory_bytes}" \
  --env MIR2_GATE16_PLAYER_PROFILES="${players}" \
  --env MIR2_GATE16_PLAYER_ZONE_COUNT="${zone_count}" \
  --env MIR2_GATE16_HISTORY_STEPS="${history_steps}" \
  --env MIR2_GATE16_CERTIFICATION_OUT=/evidence/gate18-500-session-120zones.json \
  "${image}"

jq -e \
  --argjson players "${players}" \
  --argjson zone_count "${zone_count}" \
  '.success == true
   and .assertions.playerProfilesPass == true
   and (.playerResults | length) == 1
   and .playerResults[0].requestedPlayers == $players
   and .playerResults[0].connectedPlayers == $players
   and .playerResults[0].zoneCount == $zone_count
   and .playerResults[0].failedCommands == 0
   and .playerResults[0].completedCommands
       == ($players * .playerResults[0].commandsPerPlayer)' \
  "${output_file}" >/dev/null

echo "Gate 18 session capacity evidence written to ${output_file}"
