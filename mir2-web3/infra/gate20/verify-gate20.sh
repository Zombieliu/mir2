#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
evidence_dir="${repo_root}/docs/generated/regional"
load="${evidence_dir}/gate20-load.json"
output="${evidence_dir}/gate20.json"

if [[ ! -f "${load}" ]]; then
  echo "missing Gate 20 load evidence: ${load}" >&2
  exit 1
fi

jq -e '
  .schemaVersion == 1
  and .success == true
  and .profileId == "mir2-regional-v1"
  and .profileExact == true
  and (.gitCommit | length) >= 7
  and (.imageDigest | startswith("sha256:"))
  and .completedAtMs > .generatedAtMs
  and .requestedPlayers == 1000
  and .connectedPlayers == 1000
  and .distinctAccounts == 1000
  and .distinctCharacters == 1000
  and .profileCatalogMaps == 700
  and .runtimeManifestMaps >= 120
  and .activeMapCount == 120
  and .activeZoneCount >= 125
  and .hotMapFileName == "0"
  and .hotMapPlayers == 300
  and (.hotMapLinePlayers | length) == 6
  and all(.hotMapLinePlayers[]; . == 50)
  and .zoneHostSessionCount == 1000
  and .zoneHostActiveConnections <= 130
  and (.zoneHostActiveConnections * 4) < .zoneHostSessionCount
  and .requestedActiveDurationSeconds == 3600
  and .measuredActiveDurationMs >= 3600000
  and .roles == {
    "movement": 600,
    "combat": 150,
    "social": 50,
    "economy": 50,
    "idle": 150
  }
  and .latencyMs.p95 <= 200
  and .resources.zoneRpcCodec == "msgpack"
  and .resources.zoneRpcSharedPoolSize == 128
  and .resources.zoneRpcQueueTimeoutMs == 500
  and .workloadCommandCoverage >= 0.95
  and .errorRate <= 0.001
  and .promotion.success == true
  and .promotion.standbyGeneration > .promotion.activeGeneration
  and .promotion.sessionRefreshCount == .promotion.zoneSessionCount
  and .promotion.postPromotionProbeCount == 1000
  and .economyDuplicateCount == 0
  and .economyRuntimeLedgerMismatchCount == 0
  and .economyReconciliation.healthy == true
  and all(.assertions[]; . == true)
' "${load}" >/dev/null

generated_at_ms="$(( $(date +%s) * 1000 ))"
load_sha="$(shasum -a 256 "${load}" | awk '{print $1}')"

jq -n \
  --argjson generatedAtMs "${generated_at_ms}" \
  --arg loadSha "${load_sha}" \
  --slurpfile load "${load}" \
  '{
    schemaVersion: 1,
    gate: 20,
    profileId: "mir2-regional-v1",
    generatedAtMs: $generatedAtMs,
    gitCommit: $load[0].gitCommit,
    imageDigest: $load[0].imageDigest,
    summary: {
      requestedPlayers: $load[0].requestedPlayers,
      measuredActiveDurationMs: $load[0].measuredActiveDurationMs,
      activeMapCount: $load[0].activeMapCount,
      activeZoneCount: $load[0].activeZoneCount,
      hotMapFileName: $load[0].hotMapFileName,
      hotMapPlayers: $load[0].hotMapPlayers,
      hotMapLinePlayers: $load[0].hotMapLinePlayers,
      zoneHostSessionCount: $load[0].zoneHostSessionCount,
      zoneHostActiveConnections: $load[0].zoneHostActiveConnections,
      attemptedCommands: $load[0].attemptedCommands,
      completedCommands: $load[0].completedCommands,
      failedCommands: $load[0].failedCommands,
      errorRate: $load[0].errorRate,
      latencyMs: $load[0].latencyMs
    },
    resources: $load[0].resources,
    sourceSha256: {
      load: $loadSha
    },
    assertions: {
      exactOneThousandPlayerOneHourProfileAccepted: true,
      oneHundredTwentyMapsWereActive: true,
      crystalMapHadExactlyThreeHundredPlayers: true,
      hotMapWasSplitIntoSixBalancedLines: true,
      lineHardCapacityWasNotExceeded: true,
      zoneRpcUsedBinaryCodec: true,
      oneThousandSessionsUsedABoundedSharedConnectionPool: true,
      rpcControlTrafficHadReservedCapacity: true,
      p95StayedWithinTwoHundredMilliseconds: true,
      safePromotionPreservedAllSessions: true,
      economyHadNoDuplicateOrLedgerMismatch: true
    },
    success: true
  }' >"${output}"

jq -e '
  .schemaVersion == 1
  and .gate == 20
  and .success == true
  and all(.assertions[]; . == true)
' "${output}" >/dev/null

echo "Gate 20 aggregate evidence written to ${output}"
