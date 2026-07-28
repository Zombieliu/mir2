#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
evidence_dir="${repo_root}/docs/generated/regional"
output="${evidence_dir}/gate18.json"

load="${evidence_dir}/gate18-load.json"
gameplay="${evidence_dir}/gate18-gameplay.json"
migrations="${evidence_dir}/gate18-migrations.json"
remote_economy="${evidence_dir}/gate18-remote-economy.json"
economy_producer="${evidence_dir}/gate18-economy-producer.json"
session_capacity="${evidence_dir}/gate18-500-session-120zones.json"

for evidence in \
  "${load}" \
  "${gameplay}" \
  "${migrations}" \
  "${remote_economy}" \
  "${economy_producer}" \
  "${session_capacity}"; do
  if [[ ! -f "${evidence}" ]]; then
    echo "missing Gate 18 evidence: ${evidence}" >&2
    exit 1
  fi
done

jq -e '
  .schemaVersion == 1
  and .success == true
  and .profileId == "mir2-regional-v1"
  and .profileExact == true
  and .requestedPlayers == 500
  and .connectedPlayers == 500
  and .distinctAccounts == 500
  and .distinctCharacters == 500
  and .profileCatalogMaps == 700
  and .runtimeManifestMaps >= 120
  and .activeZoneCount == 120
  and .requestedActiveDurationSeconds == 1800
  and .measuredActiveDurationMs >= 1800000
  and .promotionPauseExcludedFromActiveDuration == true
  and .roles == {
    "movement": 300,
    "combat": 75,
    "social": 25,
    "economy": 25,
    "idle": 75
  }
  and .workloadCommandCoverage >= 0.95
  and .errorRate <= 0.001
  and .promotion.activeOwner == "gate18-active"
  and .promotion.standbyOwner == "gate18-standby"
  and .promotion.zoneId == "map:0"
  and .promotion.zoneSessionCount == 30
  and .promotion.standbyGeneration > .promotion.activeGeneration
  and .promotion.sessionRefreshCount == .promotion.zoneSessionCount
  and .promotion.postPromotionProbeCount == 500
  and .promotion.success == true
  and .economyDuplicateCount == 0
  and .economyRuntimeLedgerMismatchCount == 0
  and .economyReconciliation.healthy == true
  and (.economyTransitions | length) == .expectedCommandsByRole.economy
  and ([.economyTransitions[] | select(.semanticSuccess == true)] | length)
      == .completedByRole.economy
  and ([.economyTransitions[] | select(.operation == "drop") | .objectId] | unique | length)
      == ([.economyTransitions[] | select(.operation == "drop") | .objectId] | length)
  and all(.assertions[]; . == true)
' "${load}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and (.assertions | length) == 10
  and all(.assertions[]; . == true)
' "${gameplay}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .concurrentWorkers == 16
  and .successfulWorkers == .concurrentWorkers
  and .appliedMigrationCount == .expectedMigrationCount
  and .requiredRelationsPresent == true
' "${migrations}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and (.assertions | length) == 13
  and all(.assertions[]; . == true)
' "${remote_economy}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and (.assertions | length) == 12
  and all(.assertions[]; . == true)
' "${economy_producer}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and all(.assertions[]; . == true)
' "${session_capacity}" >/dev/null

generated_at_ms="$(( $(date +%s) * 1000 ))"

jq -n \
  --argjson generatedAtMs "${generated_at_ms}" \
  --arg loadSha256 "$(shasum -a 256 "${load}" | awk '{print $1}')" \
  --arg gameplaySha256 "$(shasum -a 256 "${gameplay}" | awk '{print $1}')" \
  --arg migrationsSha256 "$(shasum -a 256 "${migrations}" | awk '{print $1}')" \
  --arg remoteEconomySha256 "$(shasum -a 256 "${remote_economy}" | awk '{print $1}')" \
  --arg economyProducerSha256 "$(shasum -a 256 "${economy_producer}" | awk '{print $1}')" \
  --arg sessionCapacitySha256 "$(shasum -a 256 "${session_capacity}" | awk '{print $1}')" \
  --slurpfile load "${load}" \
  --slurpfile gameplay "${gameplay}" \
  --slurpfile migrations "${migrations}" \
  --slurpfile remoteEconomy "${remote_economy}" \
  --slurpfile economyProducer "${economy_producer}" \
  --slurpfile sessionCapacity "${session_capacity}" \
  '{
    schemaVersion: 1,
    gate: 18,
    profileId: "mir2-regional-v1",
    generatedAtMs: $generatedAtMs,
    summary: {
      requestedPlayers: $load[0].requestedPlayers,
      connectedPlayers: $load[0].connectedPlayers,
      distinctAccounts: $load[0].distinctAccounts,
      distinctCharacters: $load[0].distinctCharacters,
      activeZoneCount: $load[0].activeZoneCount,
      runtimeManifestMaps: $load[0].runtimeManifestMaps,
      requestedActiveDurationSeconds: $load[0].requestedActiveDurationSeconds,
      measuredActiveDurationMs: $load[0].measuredActiveDurationMs,
      attemptedCommands: $load[0].attemptedCommands,
      completedCommands: $load[0].completedCommands,
      failedCommands: $load[0].failedCommands,
      failureReasons: $load[0].failureReasons,
      errorRate: $load[0].errorRate,
      workloadCommandCoverage: $load[0].workloadCommandCoverage,
      latencyMs: $load[0].latencyMs,
      promotion: $load[0].promotion,
      economyDuplicateCount: $load[0].economyDuplicateCount,
      economyRuntimeLedgerMismatchCount: $load[0].economyRuntimeLedgerMismatchCount,
      economyReconciliation: $load[0].economyReconciliation
    },
    sourceEvidence: {
      load: {
        path: "docs/generated/regional/gate18-load.json",
        sha256: $loadSha256,
        runId: $load[0].runId
      },
      gameplay: {
        path: "docs/generated/regional/gate18-gameplay.json",
        sha256: $gameplaySha256,
        runId: $gameplay[0].runId
      },
      migrations: {
        path: "docs/generated/regional/gate18-migrations.json",
        sha256: $migrationsSha256
      },
      remoteEconomy: {
        path: "docs/generated/regional/gate18-remote-economy.json",
        sha256: $remoteEconomySha256,
        runId: $remoteEconomy[0].runId
      },
      economyProducer: {
        path: "docs/generated/regional/gate18-economy-producer.json",
        sha256: $economyProducerSha256,
        runId: $economyProducer[0].runId
      },
      sessionCapacity: {
        path: "docs/generated/regional/gate18-500-session-120zones.json",
        sha256: $sessionCapacitySha256
      }
    },
    assertions: {
      exactRegionalProfileAccepted: true,
      fiveHundredDistinctPlayersRanForThirtyMinutes: true,
      oneHundredTwentyZonesWereActive: true,
      mixedGameplayCoverageMet: true,
      safeZonePromotionCompleted: true,
      postPromotionSessionsRemainedUsable: true,
      economySemanticFailuresStayedWithinSlo: true,
      economyHadNoDuplicateOrLedgerMismatch: true,
      gameplayAcceptancePassed: $gameplay[0].success,
      concurrentMigrationsPassed: $migrations[0].success,
      remoteEconomyAcceptancePassed: $remoteEconomy[0].success,
      producerCrashWindowAcceptancePassed: $economyProducer[0].success,
      supportingSessionCapacityPassed: $sessionCapacity[0].success
    },
    success: true
  }' >"${output}"

jq -e '
  .success == true
  and all(.assertions[]; . == true)
' "${output}" >/dev/null

echo "Gate 18 aggregate evidence written to ${output}"
