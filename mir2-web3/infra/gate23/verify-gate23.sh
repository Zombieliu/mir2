#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
EVIDENCE_DIR="${REPO_ROOT}/docs/generated/home-node"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mir2-gate23.XXXXXX")"
KEYRING_ACCOUNT="gate23-acceptance-$$-$(date +%s)"
SUPERVISOR_PID=""

cleanup() {
  if [[ -n "${SUPERVISOR_PID}" ]]; then
    kill -INT "${SUPERVISOR_PID}" >/dev/null 2>&1 || true
    wait "${SUPERVISOR_PID}" >/dev/null 2>&1 || true
  fi
  MIR2_HOME_AGENT_KEYRING_ACCOUNT="${KEYRING_ACCOUNT}" \
    "${REPO_ROOT}/target/debug/home_agent_supervisor" key-delete >/dev/null 2>&1 || true
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

mkdir -p "${EVIDENCE_DIR}"

cargo +1.89.0 build \
  --locked \
  -p mir2-gateway \
  --bin node_identity \
  --bin zone_host \
  --bin home_agent \
  --bin home_agent_launcher \
  --bin home_agent_supervisor \
  --bin home_agent_release

cargo +1.89.0 test \
  --locked \
  -p mir2-gateway \
  home_agent_runtime \
  --lib \
  -- \
  --test-threads=1

MIR2_HOME_AGENT_KEYRING_ACCOUNT="${KEYRING_ACCOUNT}" \
  "${REPO_ROOT}/target/debug/home_agent_supervisor" key-init \
  >"${TMP_DIR}/key-init.json"
MIR2_HOME_AGENT_KEYRING_ACCOUNT="${KEYRING_ACCOUNT}" \
  "${REPO_ROOT}/target/debug/home_agent_supervisor" key-public \
  >"${TMP_DIR}/key-public.json"
jq -e \
  --slurpfile public "${TMP_DIR}/key-public.json" \
  '.created == true and .nodeId == $public[0].nodeId and .publicKey == $public[0].publicKey and .keyStore == "operating-system-keyring"' \
  "${TMP_DIR}/key-init.json" >/dev/null

"${REPO_ROOT}/target/debug/node_identity" generate "${TMP_DIR}/release-issuer.key" \
  >"${TMP_DIR}/release-issuer.json"
RELEASE_PUBLIC_KEY="$(jq -er '.publicKey' "${TMP_DIR}/release-issuer.json")"
COPYFILE_DISABLE=1 tar -C "${REPO_ROOT}/target/debug" \
  -czf "${TMP_DIR}/home-agent-bundle.tar.gz" \
  home_agent home_agent_supervisor zone_host
MIR2_HOME_RELEASE_SIGNING_KEY_FILE="${TMP_DIR}/release-issuer.key" \
  MIR2_HOME_RELEASE_MINIMUM_VERSION="0.1.0" \
  "${REPO_ROOT}/target/debug/home_agent_release" sign \
  "0.1.1" \
  "gate23-local-acceptance" \
  "${TMP_DIR}/home-agent-bundle.tar.gz" \
  "https://updates.obelisk.invalid/dubhe-home-agent" \
  "stable" \
  "${TMP_DIR}/release.json"

MIR2_HOME_UPDATE_ISSUER_PUBLIC_KEY="${RELEASE_PUBLIC_KEY}" \
  MIR2_HOME_UPDATE_TARGET="gate23-local-acceptance" \
  MIR2_HOME_UPDATE_CHANNEL="stable" \
  "${REPO_ROOT}/target/debug/home_agent_supervisor" verify-manifest \
  "${TMP_DIR}/release.json" \
  >"${TMP_DIR}/release-verified.json"
jq -e '.verified == true and .version == "0.1.1"' \
  "${TMP_DIR}/release-verified.json" >/dev/null

MIR2_HOME_UPDATE_ISSUER_PUBLIC_KEY="${RELEASE_PUBLIC_KEY}" \
  MIR2_HOME_UPDATE_TARGET="gate23-local-acceptance" \
  MIR2_HOME_UPDATE_CHANNEL="stable" \
  MIR2_HOME_UPDATE_ROOT="${TMP_DIR}/updates" \
  "${REPO_ROOT}/target/debug/home_agent_supervisor" stage-update \
  "${TMP_DIR}/release.json" \
  "${TMP_DIR}/home-agent-bundle.tar.gz" \
  >"${TMP_DIR}/release-staged.json"
jq -e '.staged == true and .version == "0.1.1"' \
  "${TMP_DIR}/release-staged.json" >/dev/null

ZONE_RPC_PORT="${MIR2_GATE23_ZONE_RPC_PORT:-19220}"
ZONE_OPERATOR_PORT="${MIR2_GATE23_ZONE_OPERATOR_PORT:-19221}"
SUPERVISOR_PORT="${MIR2_GATE23_SUPERVISOR_PORT:-19222}"
MANAGEMENT_TOKEN="gate23-management-token-0123456789abcdef"
HOME_AGENT_NODE_ID="$(jq -er '.nodeId' "${TMP_DIR}/key-init.json")"

printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  'trap "exit 0" INT TERM' \
  'while true; do' \
  '  updated_at_ms="$(($(date +%s) * 1000))"' \
  '  printf '\''{"version":"0.1.0","nodeId":"%s","relayId":"gate23-relay","relayConnected":true,"telemetryConfigured":true,"telemetryAccepted":true,"telemetrySequence":1,"lastTelemetryAtMs":%s,"lastError":null,"updatedAtMs":%s}\n'\'' "${MIR2_GATE23_NODE_ID}" "${updated_at_ms}" "${updated_at_ms}" >"${MIR2_HOME_AGENT_STATUS_FILE}"' \
  '  sleep 1' \
  'done' \
  >"${TMP_DIR}/home_agent"
chmod 700 "${TMP_DIR}/home_agent"

MIR2_ZONE_HOST_ADDR="127.0.0.1:${ZONE_RPC_PORT}" \
  MIR2_ZONE_HOST_METRICS_ADDR="127.0.0.1:${ZONE_OPERATOR_PORT}" \
  MIR2_ZONE_HOST_TOKEN="gate23-zone-rpc-token" \
  MIR2_ZONE_HOST_MANAGEMENT_TOKEN="${MANAGEMENT_TOKEN}" \
  MIR2_ACCOUNT_STORE_PATH="${TMP_DIR}/accounts.json" \
  MIR2_HOME_AGENT_KEYRING_ACCOUNT="${KEYRING_ACCOUNT}" \
  MIR2_HOME_SUPERVISOR_BIND="127.0.0.1:${SUPERVISOR_PORT}" \
  MIR2_HOME_ZONE_OPERATOR_URL="http://127.0.0.1:${ZONE_OPERATOR_PORT}" \
  MIR2_HOME_SAMPLE_INTERVAL_MS="500" \
  MIR2_HOME_OVERLOAD_SAMPLES="2" \
  MIR2_HOME_RECOVERY_SAMPLES="2" \
  MIR2_HOME_MAX_CPU_PERCENT="100" \
  MIR2_HOME_MIN_AVAILABLE_MEMORY_MIB="1" \
  MIR2_GATE23_NODE_ID="${HOME_AGENT_NODE_ID}" \
  MIR2_HOME_MANAGE_CHILDREN="true" \
  MIR2_HOME_ZONE_BINARY="${REPO_ROOT}/target/debug/zone_host" \
  MIR2_HOME_AGENT_BINARY="${TMP_DIR}/home_agent" \
  MIR2_HOME_AGENT_STATUS_FILE="${TMP_DIR}/home-agent-status.json" \
  MIR2_HOME_UPDATE_ROOT="${TMP_DIR}/updates" \
  MIR2_HOME_SUPERVISOR_HEALTH_URL="http://127.0.0.1:${SUPERVISOR_PORT}/v1/status" \
  MIR2_HOME_UPDATE_HEALTH_TIMEOUT_SECONDS="10" \
  "${REPO_ROOT}/target/debug/home_agent_launcher" \
  >"${TMP_DIR}/supervisor.log" 2>&1 &
SUPERVISOR_PID=$!

deadline=$((SECONDS + 30))
until curl --fail --silent "http://127.0.0.1:${SUPERVISOR_PORT}/v1/status" \
  >"${TMP_DIR}/status-initial.json" \
  && jq -e '.relayConnected == true and .telemetryAccepted == true' \
    "${TMP_DIR}/status-initial.json" >/dev/null; do
  if (( SECONDS >= deadline )); then
    cat "${TMP_DIR}/supervisor.log"
    echo "Gate 23 supervisor did not become ready" >&2
    exit 1
  fi
  sleep 0.25
done

UNAUTHORIZED_STATUS="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --request POST "http://127.0.0.1:${SUPERVISOR_PORT}/v1/drain")"
[[ "${UNAUTHORIZED_STATUS}" == "401" ]]

curl --fail --silent \
  --request POST \
  --header "Authorization: Bearer ${MANAGEMENT_TOKEN}" \
  "http://127.0.0.1:${SUPERVISOR_PORT}/v1/drain" \
  >"${TMP_DIR}/drain.json"
curl --fail --silent "http://127.0.0.1:${ZONE_OPERATOR_PORT}/healthz" \
  >"${TMP_DIR}/zone-draining.json"
jq -e '.health.draining == true' "${TMP_DIR}/zone-draining.json" >/dev/null

curl --fail --silent \
  --request POST \
  --header "Authorization: Bearer ${MANAGEMENT_TOKEN}" \
  "http://127.0.0.1:${SUPERVISOR_PORT}/v1/resume" \
  >"${TMP_DIR}/resume.json"
curl --fail --silent "http://127.0.0.1:${ZONE_OPERATOR_PORT}/healthz" \
  >"${TMP_DIR}/zone-serving.json"
jq -e '.health.draining == false' "${TMP_DIR}/zone-serving.json" >/dev/null

deadline=$((SECONDS + 10))
until grep -q '^HOME_AGENT_LAUNCHER_HEALTHY version=0.1.1 ' \
  "${TMP_DIR}/supervisor.log"; do
  if (( SECONDS >= deadline )); then
    cat "${TMP_DIR}/supervisor.log"
    echo "Gate 23 launcher did not finish the staged-version health window" >&2
    exit 1
  fi
  sleep 0.1
done

MANAGED_SUPERVISOR_PID="$(
  pgrep -P "${SUPERVISOR_PID}" -f "home_agent_supervisor" | head -n 1
)"
[[ -n "${MANAGED_SUPERVISOR_PID}" ]]
FAKE_AGENT_PID="$(
  pgrep -P "${MANAGED_SUPERVISOR_PID}" -f "${TMP_DIR}/home_agent" | head -n 1
)"
[[ -n "${FAKE_AGENT_PID}" ]]
kill -KILL "${FAKE_AGENT_PID}"
deadline=$((SECONDS + 10))
while kill -0 "${SUPERVISOR_PID}" >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    cat "${TMP_DIR}/supervisor.log"
    ps -o pid,ppid,stat,command -p \
      "${SUPERVISOR_PID},${MANAGED_SUPERVISOR_PID},${FAKE_AGENT_PID}" || true
    echo "Gate 23 supervisor did not fail closed after managed Home Agent exit" >&2
    exit 1
  fi
  sleep 0.1
done
set +e
wait "${SUPERVISOR_PID}"
SUPERVISOR_EXIT_STATUS=$?
set -e
SUPERVISOR_PID=""
[[ "${SUPERVISOR_EXIT_STATUS}" -ne 0 ]]
if curl --fail --silent "http://127.0.0.1:${ZONE_OPERATOR_PORT}/readyz" >/dev/null; then
  echo "Gate 23 managed Zone Host survived supervisor fail-closed shutdown" >&2
  exit 1
fi

jq -n \
  --slurpfile key "${TMP_DIR}/key-init.json" \
  --slurpfile release "${TMP_DIR}/release-verified.json" \
  --slurpfile staged "${TMP_DIR}/release-staged.json" \
  --slurpfile status "${TMP_DIR}/status-initial.json" \
  --slurpfile draining "${TMP_DIR}/zone-draining.json" \
  --slurpfile serving "${TMP_DIR}/zone-serving.json" \
  --slurpfile updateState "${TMP_DIR}/updates/state.json" \
  --argjson unauthorizedStatus "${UNAUTHORIZED_STATUS}" \
  --argjson supervisorExitStatus "${SUPERVISOR_EXIT_STATUS}" \
  '{
    schema: "obelisk.gate23.acceptance.v1",
    accepted: (
      $key[0].created == true and
      $release[0].verified == true and
      $staged[0].staged == true and
      $status[0].keyStore == "operating-system-keyring" and
      $status[0].managedProcesses == true and
      $updateState[0].currentVersion == "0.1.1" and
      $draining[0].health.draining == true and
      $serving[0].health.draining == false and
      $unauthorizedStatus == 401 and
      $supervisorExitStatus != 0
    ),
    keyring: $key[0],
    signedRelease: $release[0],
    stagedRelease: {
      staged: $staged[0].staged,
      version: $staged[0].version,
      activation: $staged[0].activation,
      releaseDirectory: ("versions/" + $staged[0].version)
    },
    launcher: {
      activatedVersion: $updateState[0].currentVersion,
      previousVersion: $updateState[0].previousVersion
    },
    supervisor: $status[0],
    drain: {
      unauthorizedStatus: $unauthorizedStatus,
      authenticatedDrain: $draining[0].health.draining,
      authenticatedResume: ($serving[0].health.draining == false)
    },
    managedProcessFailure: {
      childExitFailedClosed: true,
      supervisorExitStatus: $supervisorExitStatus,
      zoneHostStoppedWithSupervisor: true
    }
  }' >"${EVIDENCE_DIR}/gate23-local-acceptance.json"

jq -e '.accepted == true' "${EVIDENCE_DIR}/gate23-local-acceptance.json" >/dev/null
echo "GATE23_LOCAL_ACCEPTED"
