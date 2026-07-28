#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
OUTPUT_DIR="${1:-${REPO_DIR}/docs/generated/home-node/gate25-local}"
COLLECTOR_PORT="${MIR2_GATE25_COLLECTOR_PORT:-19325}"
ZONE_RPC_PORT="${MIR2_GATE25_ZONE_RPC_PORT:-19326}"
ZONE_OPERATOR_PORT="${MIR2_GATE25_ZONE_OPERATOR_PORT:-19327}"
OPERATOR_TOKEN="gate25-local-operator-token-0123456789abcdef"
COLLECTOR_PID=""
ZONE_PID=""
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mir2-gate25.XXXXXX")"

cleanup() {
  if [[ -n "${ZONE_PID}" ]]; then
    kill -TERM "${ZONE_PID}" >/dev/null 2>&1 || true
    wait "${ZONE_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${COLLECTOR_PID}" ]]; then
    kill -TERM "${COLLECTOR_PID}" >/dev/null 2>&1 || true
    wait "${COLLECTOR_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

cd "${REPO_DIR}"
cargo +1.89.0 test -p mir2-gateway home_beta --lib
cargo +1.89.0 build -q -p mir2-gateway \
  --bin home_agent \
  --bin home_telemetry_collector \
  --bin zone_host
cargo +1.89.0 run -q -p mir2-gateway --bin home_beta_local_acceptance -- "${OUTPUT_DIR}"
"${REPO_DIR}/infra/gate22/prepare-fixtures.sh" >/dev/null

MIR2_HOME_TELEMETRY_COLLECTOR_BIND="127.0.0.1:${COLLECTOR_PORT}" \
  MIR2_HOME_TELEMETRY_OPERATOR_TOKEN="${OPERATOR_TOKEN}" \
  "${REPO_DIR}/target/debug/home_telemetry_collector" \
  >"${TMP_DIR}/collector.log" 2>&1 &
COLLECTOR_PID=$!
deadline=$((SECONDS + 30))
until curl --fail --silent "http://127.0.0.1:${COLLECTOR_PORT}/healthz" \
  >"${TMP_DIR}/collector-health.json"; do
  if (( SECONDS >= deadline )); then
    cat "${TMP_DIR}/collector.log"
    echo "Gate 25 telemetry collector did not become ready" >&2
    exit 1
  fi
  sleep 0.1
done

MIR2_ZONE_HOST_ADDR="127.0.0.1:${ZONE_RPC_PORT}" \
  MIR2_ZONE_HOST_METRICS_ADDR="127.0.0.1:${ZONE_OPERATOR_PORT}" \
  MIR2_ZONE_HOST_TOKEN="gate25-zone-rpc-token" \
  MIR2_ZONE_HOST_MANAGEMENT_TOKEN="gate25-zone-management-token-0123456789" \
  MIR2_ACCOUNT_STORE_PATH="${TMP_DIR}/accounts.json" \
  "${REPO_DIR}/target/debug/zone_host" \
  >"${TMP_DIR}/zone.log" 2>&1 &
ZONE_PID=$!
deadline=$((SECONDS + 30))
until curl --fail --silent "http://127.0.0.1:${ZONE_OPERATOR_PORT}/healthz" \
  >"${TMP_DIR}/zone-health.json"; do
  if (( SECONDS >= deadline )); then
    cat "${TMP_DIR}/zone.log"
    echo "Gate 25 Zone Host did not become ready" >&2
    exit 1
  fi
  sleep 0.1
done

INGEST_STATUS="$(
  curl --silent --output "${TMP_DIR}/ingest.json" --write-out '%{http_code}' \
    --header 'Content-Type: application/json' \
    --data-binary "@${OUTPUT_DIR}/signed-telemetry.json" \
    "http://127.0.0.1:${COLLECTOR_PORT}/v1/telemetry"
)"
[[ "${INGEST_STATUS}" == "202" ]]
REPLAY_STATUS="$(
  curl --silent --output "${TMP_DIR}/replay.json" --write-out '%{http_code}' \
    --header 'Content-Type: application/json' \
    --data-binary "@${OUTPUT_DIR}/signed-telemetry.json" \
    "http://127.0.0.1:${COLLECTOR_PORT}/v1/telemetry"
)"
[[ "${REPLAY_STATUS}" == "422" ]]
curl --fail --silent \
  "http://127.0.0.1:${COLLECTOR_PORT}/v1/public?expectedReports=1" \
  >"${OUTPUT_DIR}/collector-public-telemetry.json"
NODE_ID="$(jq -er '.payload.nodeId' "${OUTPUT_DIR}/signed-telemetry.json")"
UNAUTHORIZED_STATUS="$(
  curl --silent --output /dev/null --write-out '%{http_code}' \
    "http://127.0.0.1:${COLLECTOR_PORT}/v1/operator/${NODE_ID}"
)"
[[ "${UNAUTHORIZED_STATUS}" == "401" ]]
curl --fail --silent \
  --header "Authorization: Bearer ${OPERATOR_TOKEN}" \
  "http://127.0.0.1:${COLLECTOR_PORT}/v1/operator/${NODE_ID}" \
  >"${OUTPUT_DIR}/collector-operator-telemetry.json"
curl --fail --silent \
  --request DELETE \
  --header "Authorization: Bearer ${OPERATOR_TOKEN}" \
  "http://127.0.0.1:${COLLECTOR_PORT}/v1/operator/${NODE_ID}" \
  >"${TMP_DIR}/deleted.json"
jq -e '.deleted == true' "${TMP_DIR}/deleted.json" >/dev/null

MIR2_HOME_AGENT_SIGNING_KEY_FILE="${REPO_DIR}/infra/gate22/generated/node-signing.key" \
  MIR2_HOME_AGENT_KEY_GENERATION="1" \
  MIR2_HOME_AGENT_INSTANCE_ID="gate25-real-agent-emitter" \
  MIR2_HOME_CAPACITY_CERTIFICATE_FILE="${REPO_DIR}/infra/gate22/generated/capacity-certificate.json" \
  MIR2_HOME_TELEMETRY_URL="http://127.0.0.1:${COLLECTOR_PORT}/v1/telemetry" \
  MIR2_HOME_TELEMETRY_ALLOW_INSECURE_LOOPBACK="true" \
  MIR2_HOME_ZONE_OPERATOR_URL="http://127.0.0.1:${ZONE_OPERATOR_PORT}" \
  MIR2_HOME_COARSE_REGION="local-lab" \
  MIR2_HOME_PROVIDER_CODE="local-lab-provider" \
  MIR2_HOME_RELAY_RTT_MS="1" \
  MIR2_HOME_PACKET_LOSS_BPS="0" \
  MIR2_HOME_UPSTREAM_KBPS="100000" \
  MIR2_HOME_CHECKPOINT_LAG_MS="0" \
  MIR2_HOME_PLACEMENT_GENERATION="1" \
  MIR2_HOME_GAME_ID="mir2" \
  MIR2_HOME_REWARD_EPOCH="1" \
  "${REPO_DIR}/target/debug/home_agent" telemetry-once \
  >"${TMP_DIR}/agent-telemetry.log" 2>&1
grep -q '^HOME_TELEMETRY_EMITTED_ONCE$' "${TMP_DIR}/agent-telemetry.log"
AGENT_NODE_ID="$(jq -er '.nodeId' "${REPO_DIR}/infra/gate22/generated/capacity-certificate.json")"
curl --fail --silent \
  --header "Authorization: Bearer ${OPERATOR_TOKEN}" \
  "http://127.0.0.1:${COLLECTOR_PORT}/v1/operator/${AGENT_NODE_ID}" \
  >"${OUTPUT_DIR}/home-agent-operator-telemetry.json"
jq -e '
  .agentInstanceId == "gate25-real-agent-emitter" and
  .sequence == 1 and
  .verifiedWorkUnits == 0 and
  .workMode == "serving"
' "${OUTPUT_DIR}/home-agent-operator-telemetry.json" >/dev/null
curl --fail --silent \
  "http://127.0.0.1:${COLLECTOR_PORT}/v1/public?expectedReports=1" \
  >"${OUTPUT_DIR}/home-agent-public-telemetry.json"
curl --fail --silent \
  --request DELETE \
  --header "Authorization: Bearer ${OPERATOR_TOKEN}" \
  "http://127.0.0.1:${COLLECTOR_PORT}/v1/operator/${AGENT_NODE_ID}" \
  >"${TMP_DIR}/agent-deleted.json"
jq -e '.deleted == true' "${TMP_DIR}/agent-deleted.json" >/dev/null

UPDATED_ACCEPTANCE="${TMP_DIR}/gate25-local-acceptance.json"
jq \
  --argjson ingestStatus "${INGEST_STATUS}" \
  --argjson replayStatus "${REPLAY_STATUS}" \
  --argjson unauthorizedStatus "${UNAUTHORIZED_STATUS}" \
  '. + {
    collectorIngestAccepted: ($ingestStatus == 202),
    collectorReplayRejected: ($replayStatus == 422),
    collectorOperatorAuthEnforced: ($unauthorizedStatus == 401),
    collectorDeletionVerified: true,
    homeAgentTelemetryEmissionVerified: true,
    homeAgentTelemetryUsesRealZoneHealth: true,
    homeAgentSelfReportedBillableWorkUnits: 0
  }' \
  "${OUTPUT_DIR}/gate25-local-acceptance.json" >"${UPDATED_ACCEPTANCE}"
mv "${UPDATED_ACCEPTANCE}" "${OUTPUT_DIR}/gate25-local-acceptance.json"

jq -e '
  .accepted == true and
  .productionHomeBetaAccepted == false and
  .signedTelemetryVerified == true and
  .rawIpPersisted == false and
  .rewardReconciliationPayable == true and
  .publicViewContainsNodeId == false and
  .simulatedRunProductionRejected == true and
  .insufficientCohortRejected == true and
  .collectorIngestAccepted == true and
  .collectorReplayRejected == true and
  .collectorOperatorAuthEnforced == true and
  .collectorDeletionVerified == true and
  .homeAgentTelemetryEmissionVerified == true and
  .homeAgentTelemetryUsesRealZoneHealth == true and
  .homeAgentSelfReportedBillableWorkUnits == 0 and
  .externalThreeIspEvidenceProvided == false
' "${OUTPUT_DIR}/gate25-local-acceptance.json" >/dev/null

echo "GATE25_LOCAL_POLICY_ACCEPTED output=${OUTPUT_DIR}"
echo "GATE25_PRODUCTION_NOT_ACCEPTED reason=three_physical_isp_evidence_required"
