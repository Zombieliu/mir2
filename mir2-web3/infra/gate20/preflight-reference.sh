#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
output="${1:-${repo_root}/docs/generated/regional/gate20-resource-attestation.json}"

mkdir -p "$(dirname "${output}")"

docker_info="$(docker info --format '{{json .}}')"
available_cpu="$(jq -r '.NCPU' <<<"${docker_info}")"
available_memory_bytes="$(jq -r '.MemTotal' <<<"${docker_info}")"
# Gate 20 is the bounded local-development acceptance tier. Container CPU and
# memory limits are ceilings rather than reservations; the evidence binds the
# actual Docker host and the latency/error SLO decides whether that host passes.
required_cpu=10
required_memory_bytes=8053063680

cpu_passed=false
memory_passed=false
(( available_cpu >= required_cpu )) && cpu_passed=true
(( available_memory_bytes >= required_memory_bytes )) && memory_passed=true

jq -n \
  --argjson generatedAtMs "$(( $(date +%s) * 1000 ))" \
  --arg profileId "mir2-regional-v1-3000-15m" \
  --arg architecture "$(jq -r '.Architecture' <<<"${docker_info}")" \
  --arg operatingSystem "$(jq -r '.OperatingSystem' <<<"${docker_info}")" \
  --arg dockerVersion "$(jq -r '.ServerVersion' <<<"${docker_info}")" \
  --argjson availableCpu "${available_cpu}" \
  --argjson availableMemoryBytes "${available_memory_bytes}" \
  --argjson requiredCpu "${required_cpu}" \
  --argjson requiredMemoryBytes "${required_memory_bytes}" \
  --argjson cpuPassed "${cpu_passed}" \
  --argjson memoryPassed "${memory_passed}" \
  '{
    schemaVersion: 1,
    generatedAtMs: $generatedAtMs,
    profileId: $profileId,
    tier: "local-1000ccu-15m",
    source: "docker-info",
    host: {
      architecture: $architecture,
      operatingSystem: $operatingSystem,
      dockerVersion: $dockerVersion,
      availableCpu: $availableCpu,
      availableMemoryBytes: $availableMemoryBytes
    },
    required: {
      cpu: $requiredCpu,
      memoryBytes: $requiredMemoryBytes
    },
    assertions: {
      cpuMeetsLocalAcceptanceFloor: $cpuPassed,
      memoryMeetsLocalAcceptanceFloor: $memoryPassed
    },
    success: ($cpuPassed and $memoryPassed)
  }' >"${output}"

if [[ "${cpu_passed}" != true || "${memory_passed}" != true ]]; then
  echo "Gate 20 local preflight failed: host has ${available_cpu} CPU / ${available_memory_bytes} bytes, requires ${required_cpu} CPU / ${required_memory_bytes} bytes" >&2
  exit 1
fi

echo "Gate 20 local resource attestation written to ${output}"
