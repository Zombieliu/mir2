#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
evidence_dir="${repo_root}/docs/generated/regional"
load="${evidence_dir}/gate21-load.json"
stability="${evidence_dir}/gate21-stability.json"
faults="${evidence_dir}/gate21-faults.json"
output="${evidence_dir}/gate21.json"

for evidence in \
  "${evidence_dir}/gate18.json" \
  "${evidence_dir}/gate19.json" \
  "${evidence_dir}/gate20.json" \
  "${load}" "${stability}" "${faults}"; do
  if [[ ! -f "${evidence}" ]]; then
    echo "missing Regional prerequisite evidence: ${evidence}" >&2
    exit 1
  fi
done

for gate in 18 19 20; do
  jq -e --argjson gate "${gate}" '
    .schemaVersion == 1
    and .gate == $gate
    and .success == true
  ' "${evidence_dir}/gate${gate}.json" >/dev/null
done

jq -e '
  .schemaVersion == 1
  and .success == true
  and .profileId == "mir2-regional-v1-3000-15m"
  and .profileExact == true
  and (.gitCommit | length) >= 7
  and (.imageDigest | startswith("sha256:"))
  and .completedAtMs > .generatedAtMs
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
  and .requestedActiveDurationSeconds == 900
  and .measuredActiveDurationMs >= 900000
  and .roles == {
    "movement": 1800,
    "combat": 450,
    "social": 150,
    "economy": 150,
    "idle": 450
  }
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
  and .promotion.standbyGeneration > .promotion.activeGeneration
  and .promotion.sessionRefreshCount == .promotion.zoneSessionCount
  and .promotion.postPromotionProbeCount == 3000
  and .economyDuplicateCount == 0
  and .economyRuntimeLedgerMismatchCount == 0
  and .economyReconciliation.healthy == true
  and all(.assertions[]; . == true)
' "${load}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .gate == 21
  and .profileId == "mir2-regional-v1-3000-15m"
  and .sampledDurationMs >= 900000
  and .memory.observedGrowthPercent <= 5
  and .wal.maximumObservedBytes <= .wal.limitBytes
  and .success == true
  and all(.assertions[]; . == true)
' "${stability}" >/dev/null

load_commit="$(jq -r '.gitCommit' "${load}")"
jq -e --arg loadCommit "${load_commit}" '
  .schemaVersion == 1
  and .gate == 21
  and .profileId == "mir2-regional-v1-3000-15m"
  and .gitCommit == $loadCommit
  and (.faults | length) == 8
  and .success == true
  and all(.assertions[]; . == true)
' "${faults}" >/dev/null

generated_at_ms="$(( $(date +%s) * 1000 ))"
gate18_sha="$(shasum -a 256 "${evidence_dir}/gate18.json" | awk '{print $1}')"
gate19_sha="$(shasum -a 256 "${evidence_dir}/gate19.json" | awk '{print $1}')"
gate20_sha="$(shasum -a 256 "${evidence_dir}/gate20.json" | awk '{print $1}')"
load_sha="$(shasum -a 256 "${load}" | awk '{print $1}')"
stability_sha="$(shasum -a 256 "${stability}" | awk '{print $1}')"
faults_sha="$(shasum -a 256 "${faults}" | awk '{print $1}')"

jq -n \
  --argjson generatedAtMs "${generated_at_ms}" \
  --arg gate18Sha "${gate18_sha}" \
  --arg gate19Sha "${gate19_sha}" \
  --arg gate20Sha "${gate20_sha}" \
  --arg loadSha "${load_sha}" \
  --arg stabilitySha "${stability_sha}" \
  --arg faultsSha "${faults_sha}" \
  --slurpfile load "${load}" \
  --slurpfile stability "${stability}" \
  --slurpfile faults "${faults}" \
  '{
    schemaVersion: 1,
    gate: 21,
    profileId: "mir2-regional-v1-3000-15m",
    generatedAtMs: $generatedAtMs,
    gitCommit: $load[0].gitCommit,
    imageDigest: $load[0].imageDigest,
    summary: {
      requestedPlayers: $load[0].requestedPlayers,
      measuredActiveDurationMs: $load[0].measuredActiveDurationMs,
      activeMapCount: $load[0].activeMapCount,
      activeZoneCount: $load[0].activeZoneCount,
      hotMapPlayers: $load[0].hotMapPlayers,
      hotMapLinePlayers: $load[0].hotMapLinePlayers,
      errorRate: $load[0].errorRate,
      latencyMs: $load[0].latencyMs,
      memoryGrowthPercent: $stability[0].memory.observedGrowthPercent,
      maximumWalBytes: $stability[0].wal.maximumObservedBytes,
      acceptedFaults: $faults[0].faults
    },
    sourceSha256: {
      gate18: $gate18Sha,
      gate19: $gate19Sha,
      gate20: $gate20Sha,
      load: $loadSha,
      stability: $stabilitySha,
      faults: $faultsSha
    },
    assertions: {
      allEarlierRegionalGatesAccepted: true,
      exactThreeThousandPlayerFifteenMinuteProfileAccepted: true,
      oneHundredTwentyMapsAndOneHundredTwentyNineZonesWereActive: true,
      hotMapWasSplitIntoTenBalancedLines: true,
      p95AndP99StayedWithinRegionalSlo: true,
      shortWindowMemoryGrowthStayedWithinFivePercent: true,
      durableWalStayedWithinOneGiB: true,
      fullFaultAndRollingUpgradeMatrixAccepted: true,
      economyAndSessionStateReconciled: true
    },
    success: true
  }' >"${output}"

jq -e '
  .schemaVersion == 1
  and .gate == 21
  and .success == true
  and all(.assertions[]; . == true)
' "${output}" >/dev/null

echo "Gate 21 Regional aggregate evidence written to ${output}"
