#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
evidence_dir="${repo_root}/docs/generated/regional"
output="${evidence_dir}/gate19.json"

load="${evidence_dir}/gate19-load.json"
standby_zone="${evidence_dir}/gate19-standby-zone-kill.json"
active_zone="${evidence_dir}/gate19-zone-failover.json"
zone_session="${evidence_dir}/gate19-zone-session.json"
preflight="${evidence_dir}/gate19-infra-preflight.json"
gateway="${evidence_dir}/gate19-infra-gateway-kill.json"
redis="${evidence_dir}/gate19-infra-redis-failover.json"
postgres="${evidence_dir}/gate19-infra-postgres-failover.json"
commonware="${evidence_dir}/gate19-commonware-validator.json"

sources=(
  "${load}"
  "${standby_zone}"
  "${active_zone}"
  "${zone_session}"
  "${preflight}"
  "${gateway}"
  "${redis}"
  "${postgres}"
  "${commonware}"
)
for evidence in "${sources[@]}"; do
  if [[ ! -f "${evidence}" ]]; then
    echo "missing Gate 19 evidence: ${evidence}" >&2
    exit 1
  fi
done

jq -e '
  .schemaVersion == 1
  and .success == true
  and .profileId == "mir2-regional-v1"
  and .profileExact == true
  and (.gitCommit | length) >= 7
  and (.imageDigest | length) >= 12
  and .completedAtMs > .generatedAtMs
  and .requestedPlayers == 500
  and .connectedPlayers == 500
  and .distinctAccounts == 500
  and .distinctCharacters == 500
  and .profileCatalogMaps == 700
  and .runtimeManifestMaps >= 120
  and .activeZoneCount == 120
  and .requestedActiveDurationSeconds == 3600
  and .measuredActiveDurationMs >= 3600000
  and .roles == {
    "movement": 300,
    "combat": 75,
    "social": 25,
    "economy": 25,
    "idle": 75
  }
  and .workloadCommandCoverage >= 0.95
  and .errorRate <= 0.001
  and .promotion.activeOwner == "gate19-active"
  and .promotion.standbyOwner == "gate19-standby"
  and .promotion.standbyGeneration > .promotion.activeGeneration
  and .promotion.sessionRefreshCount == .promotion.zoneSessionCount
  and .promotion.postPromotionProbeCount == 500
  and .promotion.success == true
  and .economyDuplicateCount == 0
  and .economyRuntimeLedgerMismatchCount == 0
  and .economyReconciliation.healthy == true
  and all(.assertions[]; . == true)
' "${load}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and (.assertions | length) == 10
  and all(.assertions[]; . == true)
' "${standby_zone}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .activeOwner == "gate19-active"
  and .standbyOwner == "gate19-standby"
  and .newGeneration > .oldGeneration
  and .failoverRtoMs < 5000
  and .synchronizedCursor == .promotedCursor
  and .synchronizedDigest == .promotedDigest
  and all(.assertions[]; . == true)
' "${active_zone}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .identityPreserved == true
  and .initialMap == .promotedMap
  and .promotedGeneration > .initialGeneration
  and .resumeCommandMs < 5000
' "${zone_session}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .healthyGatewayCount == 3
  and .redisMasterRoundTrip == true
  and .postgresWritablePrimary == true
' "${preflight}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .phase == "gateway-kill"
  and .healthyGatewayCount >= 2
  and .recoveryRtoMs != null
  and .recoveryRtoMs < 10000
  and .redisRouteLeaseOwner == "gate19-gateway-2"
  and all(.assertions[]; . == true)
' "${gateway}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .phase == "redis-primary-failover"
  and .redisMasterRoundTrip == true
  and .redisMasterAddress != $preflightMaster
  and all(.assertions[]; . == true)
' --arg preflightMaster "$(jq -r '.redisMasterAddress' "${preflight}")" "${redis}" >/dev/null

jq -e '
  .schemaVersion == 1
  and .success == true
  and .phase == "postgres-primary-failover"
  and .postgresWritablePrimary == true
  and .postgresServerAddress != $preflightPostgres
  and all(.assertions[]; . == true)
' --arg preflightPostgres "$(jq -r '.postgresServerAddress' "${preflight}")" "${postgres}" >/dev/null

jq -e '
  .accepted == true
  and .commonwareRelease == "v2026.2.0"
  and any(.faults[];
    .fault == "validator-3-stop-and-catch-up"
    and .accepted == true
    and .degradedGateway.respondingValidators == 3
    and .recoveredValidator.finalizedHeight
      >= .degradedGateway.finalizedHeight)
  and .milestones.goal4FaultRecovery.accepted == true
' "${commonware}" >/dev/null

generated_at_ms="$(( $(date +%s) * 1000 ))"

jq -n \
  --argjson generatedAtMs "${generated_at_ms}" \
  --arg loadSha "$(shasum -a 256 "${load}" | awk '{print $1}')" \
  --arg standbyZoneSha "$(shasum -a 256 "${standby_zone}" | awk '{print $1}')" \
  --arg activeZoneSha "$(shasum -a 256 "${active_zone}" | awk '{print $1}')" \
  --arg zoneSessionSha "$(shasum -a 256 "${zone_session}" | awk '{print $1}')" \
  --arg preflightSha "$(shasum -a 256 "${preflight}" | awk '{print $1}')" \
  --arg gatewaySha "$(shasum -a 256 "${gateway}" | awk '{print $1}')" \
  --arg redisSha "$(shasum -a 256 "${redis}" | awk '{print $1}')" \
  --arg postgresSha "$(shasum -a 256 "${postgres}" | awk '{print $1}')" \
  --arg commonwareSha "$(shasum -a 256 "${commonware}" | awk '{print $1}')" \
  --slurpfile load "${load}" \
  --slurpfile activeZone "${active_zone}" \
  --slurpfile zoneSession "${zone_session}" \
  --slurpfile gateway "${gateway}" \
  --slurpfile redis "${redis}" \
  --slurpfile postgres "${postgres}" \
  --slurpfile commonware "${commonware}" \
  '{
    schemaVersion: 1,
    gate: 19,
    profileId: "mir2-regional-v1",
    generatedAtMs: $generatedAtMs,
    summary: {
      requestedPlayers: $load[0].requestedPlayers,
      measuredActiveDurationMs: $load[0].measuredActiveDurationMs,
      activeZoneCount: $load[0].activeZoneCount,
      attemptedCommands: $load[0].attemptedCommands,
      completedCommands: $load[0].completedCommands,
      failedCommands: $load[0].failedCommands,
      errorRate: $load[0].errorRate,
      latencyMs: $load[0].latencyMs,
      zoneFailoverRtoMs: $activeZone[0].failoverRtoMs,
      zoneResumeCommandMs: $zoneSession[0].resumeCommandMs,
      gatewayRecoveryRtoMs: $gateway[0].recoveryRtoMs,
      redisPromotedMaster: $redis[0].redisMasterAddress,
      postgresPromotedServer: $postgres[0].postgresServerAddress,
      commonwareFinalHeight: $commonware[0].milestones.goal4FaultRecovery.finalHeight
    },
    faults: [
      "active-zone-host-kill",
      "standby-zone-host-kill",
      "gateway-kill",
      "redis-primary-failover",
      "postgres-primary-failover",
      "commonware-validator-kill"
    ],
    sourceSha256: {
      load: $loadSha,
      standbyZone: $standbyZoneSha,
      activeZone: $activeZoneSha,
      zoneSession: $zoneSessionSha,
      preflight: $preflightSha,
      gateway: $gatewaySha,
      redis: $redisSha,
      postgres: $postgresSha,
      commonware: $commonwareSha
    },
    assertions: {
      exactFiveHundredPlayerOneHourProfileAccepted: true,
      oneHundredTwentyZonesWereActive: true,
      activeZoneFailoverMetFiveSecondRto: true,
      realPlayerIdentityAndMapSurvivedPromotion: true,
      standbyZoneLossDidNotInterruptAuthoritativeGameplay: true,
      gatewayRouteMovedWithinTenSeconds: true,
      redisSentinelPromotedANewWritableMaster: true,
      postgresStandbyPromotedToWritablePrimary: true,
      commonwareThreeOfFourFinalityAndCatchUpPassed: true,
      sixRequiredSingleFaultScenariosPassed: true,
      economyHadNoDuplicateOrLedgerMismatch: true
    },
    success: true
  }' >"${output}"

jq -e '
  .success == true
  and (.faults | length) >= 6
  and all(.assertions[]; . == true)
' "${output}" >/dev/null

echo "Gate 19 aggregate evidence written to ${output}"
