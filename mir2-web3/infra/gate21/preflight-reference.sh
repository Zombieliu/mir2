#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
profile="${repo_root}/infra/regional/profile.json"
output="${1:-${repo_root}/docs/generated/regional/gate21-resource-attestation.json}"

mkdir -p "$(dirname "${output}")"

docker_info="$(docker info --format '{{json .}}')"
available_cpu="$(jq -r '.NCPU' <<<"${docker_info}")"
available_memory_bytes="$(jq -r '.MemTotal' <<<"${docker_info}")"
reference_cpu="$(
  jq '
    .referenceDeployment
    | (.gatewayReplicas * .gatewayCpuPerReplica)
      + (.zoneHostReplicas * .zoneHostCpuPerReplica)
      + (.postgresReplicas * .postgresCpuPerReplica)
      + (.redisReplicas * .redisCpuPerReplica)
  ' "${profile}"
)"
reference_memory_gib="$(
  jq '
    .referenceDeployment
    | (.gatewayReplicas * .gatewayMemoryGiBPerReplica)
      + (.zoneHostReplicas * .zoneHostMemoryGiBPerReplica)
      + (.postgresReplicas * .postgresMemoryGiBPerReplica)
      + (.redisReplicas * .redisMemoryGiBPerReplica)
  ' "${profile}"
)"

# The reference deployment is 98 CPU / 240 GiB. A single-host certification
# runner additionally carries the load generator, durable Zone replicator,
# three Redis Sentinels, and four 2-CPU/2-GiB Commonware validators.
harness_cpu="14.75"
harness_memory_gib="20.375"
required_cpu="$(
  jq -n --argjson reference "${reference_cpu}" --argjson harness "${harness_cpu}" \
    '($reference + $harness) | ceil'
)"
required_memory_bytes="$(
  jq -n \
    --argjson reference "${reference_memory_gib}" \
    --argjson harness "${harness_memory_gib}" \
    '(($reference + $harness) * 1073741824) | ceil'
)"

cpu_passed=false
memory_passed=false
(( available_cpu >= required_cpu )) && cpu_passed=true
(( available_memory_bytes >= required_memory_bytes )) && memory_passed=true

jq -n \
  --argjson generatedAtMs "$(( $(date +%s) * 1000 ))" \
  --arg profileId "$(jq -r '.profileId' "${profile}")" \
  --arg architecture "$(jq -r '.Architecture' <<<"${docker_info}")" \
  --arg operatingSystem "$(jq -r '.OperatingSystem' <<<"${docker_info}")" \
  --arg dockerVersion "$(jq -r '.ServerVersion' <<<"${docker_info}")" \
  --argjson availableCpu "${available_cpu}" \
  --argjson availableMemoryBytes "${available_memory_bytes}" \
  --argjson referenceCpu "${reference_cpu}" \
  --argjson referenceMemoryGiB "${reference_memory_gib}" \
  --argjson harnessCpu "${harness_cpu}" \
  --argjson harnessMemoryGiB "${harness_memory_gib}" \
  --argjson requiredCpu "${required_cpu}" \
  --argjson requiredMemoryBytes "${required_memory_bytes}" \
  --argjson cpuPassed "${cpu_passed}" \
  --argjson memoryPassed "${memory_passed}" \
  '{
    schemaVersion: 1,
    generatedAtMs: $generatedAtMs,
    profileId: $profileId,
    source: "docker-info",
    host: {
      architecture: $architecture,
      operatingSystem: $operatingSystem,
      dockerVersion: $dockerVersion,
      availableCpu: $availableCpu,
      availableMemoryBytes: $availableMemoryBytes
    },
    referenceDeployment: {
      cpu: $referenceCpu,
      memoryGiB: $referenceMemoryGiB
    },
    certificationHarness: {
      cpu: $harnessCpu,
      memoryGiB: $harnessMemoryGiB,
      components: [
        "load-generator=4CPU/8GiB",
        "zone-replicator=2CPU/4GiB",
        "redis-sentinels=0.75CPU/0.375GiB",
        "commonware-validators=8CPU/8GiB"
      ]
    },
    requiredHost: {
      cpu: $requiredCpu,
      memoryBytes: $requiredMemoryBytes
    },
    assertions: {
      cpuMeetsReferenceAndHarness: $cpuPassed,
      memoryMeetsReferenceAndHarness: $memoryPassed
    },
    success: ($cpuPassed and $memoryPassed)
  }' >"${output}"

if [[ "${cpu_passed}" != true || "${memory_passed}" != true ]]; then
  echo "Gate 21 reference preflight failed: host has ${available_cpu} CPU / ${available_memory_bytes} bytes, requires ${required_cpu} CPU / ${required_memory_bytes} bytes" >&2
  exit 1
fi

echo "Gate 21 resource attestation written to ${output}"
