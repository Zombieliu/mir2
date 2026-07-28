#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
EVIDENCE_DIR="${REPO_ROOT}/docs/generated/home-node"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mir2-gate24.XXXXXX")"
COMPOSE=(docker compose -f "${SCRIPT_DIR}/docker-compose.yml")
export MIR2_GATE24_SECCOMP_SHA256
MIR2_GATE24_SECCOMP_SHA256="$(shasum -a 256 "${SCRIPT_DIR}/seccomp.json" | awk '{print $1}')"

cleanup() {
  "${COMPOSE[@]}" down --remove-orphans >/dev/null 2>&1 || true
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

mkdir -p "${EVIDENCE_DIR}"
"${REPO_ROOT}/infra/gate22/prepare-fixtures.sh"

cargo +1.89.0 build \
  --locked \
  -p mir2-gateway \
  --bin node_identity \
  --bin home_sandbox_policy
cargo +1.89.0 test \
  --locked \
  -p mir2-gateway \
  home_sandbox \
  --lib \
  -- \
  --test-threads=1
cargo +1.89.0 test \
  --locked \
  -p mir2-gateway \
  --test home_tunnel \
  -- \
  --test-threads=1

"${COMPOSE[@]}" build zone-host home-relay home-agent acceptance
"${COMPOSE[@]}" up -d zone-host home-relay home-agent

deadline=$((SECONDS + 60))
until "${COMPOSE[@]}" logs --no-color home-agent 2>/dev/null | grep -q "HOME_AGENT_READY"; do
  if (( SECONDS >= deadline )); then
    "${COMPOSE[@]}" ps
    "${COMPOSE[@]}" logs --no-color
    echo "Gate 24 hardened Home Agent did not become ready" >&2
    exit 1
  fi
  sleep 1
done

"${COMPOSE[@]}" --profile acceptance run --rm acceptance \
  >"${TMP_DIR}/session.json"
jq -e '.accepted == true' "${TMP_DIR}/session.json" >/dev/null

ZONE_CONTAINER="$("${COMPOSE[@]}" ps -q zone-host)"
docker inspect "${ZONE_CONTAINER}" >"${TMP_DIR}/zone-inspect.json"
IMAGE_DIGEST="$(jq -er '.[0].Image' "${TMP_DIR}/zone-inspect.json")"
NODE_ID="$(jq -er '.nodeId' "${REPO_ROOT}/infra/gate22/generated/capacity-certificate.json")"

"${REPO_ROOT}/target/debug/node_identity" generate "${TMP_DIR}/sandbox-issuer.key" \
  >"${TMP_DIR}/sandbox-issuer.json"
SANDBOX_ISSUER="$(jq -er '.publicKey' "${TMP_DIR}/sandbox-issuer.json")"
MIR2_HOME_SANDBOX_SIGNING_KEY_FILE="${TMP_DIR}/sandbox-issuer.key" \
  MIR2_HOME_SANDBOX_SECCOMP_SHA256="${MIR2_GATE24_SECCOMP_SHA256}" \
  MIR2_HOME_SANDBOX_MEMORY_BYTES="1073741824" \
  MIR2_HOME_SANDBOX_NANO_CPUS="2000000000" \
  MIR2_HOME_SANDBOX_PIDS_LIMIT="128" \
  "${REPO_ROOT}/target/debug/home_sandbox_policy" sign \
  "${IMAGE_DIGEST}" \
  "${NODE_ID}" \
  "mir2-gate24_home-private" \
  "${TMP_DIR}/sandbox-manifest.json"

MIR2_HOME_SANDBOX_ISSUER_PUBLIC_KEY="${SANDBOX_ISSUER}" \
  MIR2_HOME_SANDBOX_EXPECTED_NODE_ID="${NODE_ID}" \
  MIR2_HOME_SANDBOX_GENERATION="1" \
  "${REPO_ROOT}/target/debug/home_sandbox_policy" attest \
  "${TMP_DIR}/sandbox-manifest.json" \
  "${TMP_DIR}/zone-inspect.json" \
  >"${TMP_DIR}/sandbox-attestation.json"
jq -e '.accepted == true and .runAsUser == "65534:65534" and .readOnlyRootFilesystem == true and .privileged == false' \
  "${TMP_DIR}/sandbox-attestation.json" >/dev/null

ROOTFS_WRITE_BLOCKED=false
if ! "${COMPOSE[@]}" exec -T zone-host sh -c 'touch /usr/local/bin/gate24-write-probe' \
  >"${TMP_DIR}/rootfs-write.out" 2>&1; then
  ROOTFS_WRITE_BLOCKED=true
fi
[[ "${ROOTFS_WRITE_BLOCKED}" == "true" ]]

RUNNING_UID="$("${COMPOSE[@]}" exec -T zone-host id -u | tr -d '\r')"
[[ "${RUNNING_UID}" == "65534" ]]

RELAY_TO_ZONE_BLOCKED=false
if ! "${COMPOSE[@]}" exec -T home-relay getent hosts zone-host \
  >"${TMP_DIR}/relay-zone-dns.out" 2>&1; then
  RELAY_TO_ZONE_BLOCKED=true
fi
[[ "${RELAY_TO_ZONE_BLOCKED}" == "true" ]]

ZONE_EGRESS_BLOCKED=false
if ! "${COMPOSE[@]}" exec -T zone-host timeout 2 bash -ec \
  'exec 3<>/dev/tcp/1.1.1.1/80' >"${TMP_DIR}/zone-egress.out" 2>&1; then
  ZONE_EGRESS_BLOCKED=true
fi
[[ "${ZONE_EGRESS_BLOCKED}" == "true" ]]

AGENT_PUBLISHED_PORTS="$(docker inspect "$("${COMPOSE[@]}" ps -q home-agent)" \
  --format '{{json .NetworkSettings.Ports}}')"
[[ "${AGENT_PUBLISHED_PORTS}" == "null" || "${AGENT_PUBLISHED_PORTS}" == "{}" ]]

jq -n \
  --slurpfile session "${TMP_DIR}/session.json" \
  --slurpfile sandbox "${TMP_DIR}/sandbox-attestation.json" \
  --argjson rootfsWriteBlocked "${ROOTFS_WRITE_BLOCKED}" \
  --argjson relayToZoneBlocked "${RELAY_TO_ZONE_BLOCKED}" \
  --argjson zoneEgressBlocked "${ZONE_EGRESS_BLOCKED}" \
  --arg runningUid "${RUNNING_UID}" \
  --arg seccompSha256 "${MIR2_GATE24_SECCOMP_SHA256}" \
  '{
    schema: "obelisk.gate24.acceptance.v1",
    accepted: (
      $session[0].accepted == true and
      $sandbox[0].accepted == true and
      $rootfsWriteBlocked and
      $relayToZoneBlocked and
      $zoneEgressBlocked and
      $runningUid == "65534"
    ),
    realMir2Session: $session[0],
    sandbox: $sandbox[0],
    probes: {
      rootfsWriteBlocked: $rootfsWriteBlocked,
      relayCannotResolvePrivateZone: $relayToZoneBlocked,
      zoneInternetEgressBlocked: $zoneEgressBlocked,
      runningUid: $runningUid,
      homeAgentPublishedPorts: false,
      seccompSha256: $seccompSha256
    },
    externalBoundaries: {
      cloudDdosScrubbingCertified: false,
      thirdPartyPenetrationTestCompleted: false,
      note: "Requires external provider and independent audit evidence"
    }
  }' >"${EVIDENCE_DIR}/gate24-sandbox-acceptance.json"

jq -e '.accepted == true' "${EVIDENCE_DIR}/gate24-sandbox-acceptance.json" >/dev/null
echo "GATE24_SANDBOX_ACCEPTED"
