#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mir2-director-approval.XXXXXX")"
ZONE_PID=""
ADMIN_PID=""
COMMONWARE_GATEWAY_PID=""
COMMONWARE_VALIDATOR_PIDS=("")

cleanup() {
  if [[ -n "$ADMIN_PID" ]]; then kill "$ADMIN_PID" 2>/dev/null || true; fi
  if [[ -n "$ZONE_PID" ]]; then kill "$ZONE_PID" 2>/dev/null || true; fi
  if [[ -n "$COMMONWARE_GATEWAY_PID" ]]; then
    kill "$COMMONWARE_GATEWAY_PID" 2>/dev/null || true
  fi
  for pid in "${COMMONWARE_VALIDATOR_PIDS[@]}"; do
    [[ -n "$pid" ]] || continue
    kill "$pid" 2>/dev/null || true
  done
  wait "$ADMIN_PID" 2>/dev/null || true
  wait "$ZONE_PID" 2>/dev/null || true
  wait "$COMMONWARE_GATEWAY_PID" 2>/dev/null || true
  for pid in "${COMMONWARE_VALIDATOR_PIDS[@]}"; do
    [[ -n "$pid" ]] || continue
    wait "$pid" 2>/dev/null || true
  done
  case "$RUN_DIR" in
    "${TMPDIR:-/tmp}"/mir2-director-approval.*) rm -rf -- "$RUN_DIR" ;;
  esac
}
trap cleanup EXIT

cd "$ROOT_DIR"
cargo build -p mir2-gateway --bin node_identity --bin zone_host
cargo +1.95.0 build -p mir2-gateway --features commonware-2026-2 \
  --bin gate14_validator --bin gate14_gateway
cargo build -p mir2-admin-api --bin mir2-admin-api

identity() {
  local name="$1"
  target/debug/node_identity generate "$RUN_DIR/$name.key" >"$RUN_DIR/$name.json"
}

identity director
identity validator-1
identity validator-2
identity validator-3
identity validator-4
identity zone

json_field() {
  node -e 'const fs=require("fs"); const x=JSON.parse(fs.readFileSync(process.argv[1],"utf8")); process.stdout.write(String(x[process.argv[2]]));' "$1" "$2"
}

DIRECTOR_PUBLIC_KEY="$(json_field "$RUN_DIR/director.json" publicKey)"
COMMITTEE="$(
  for name in validator-1 validator-2 validator-3 validator-4; do
    json_field "$RUN_DIR/$name.json" publicKey
    printf ','
  done | sed 's/,$//'
)"
VALIDATOR_FILES="$RUN_DIR/validator-1.key,$RUN_DIR/validator-2.key,$RUN_DIR/validator-3.key,$RUN_DIR/validator-4.key"
MANAGEMENT_TOKEN="approval-acceptance-management-token"
OPERATOR_TOKEN="approval-acceptance-operator-token"
COMMONWARE_TOKEN="approval-commonware-control-token"

env \
  DUBHE_NODE_SIGNING_KEY_FILE="$RUN_DIR/zone.key" \
  MIR2_ZONE_HOST_ADDR=127.0.0.1:17020 \
  MIR2_ZONE_HOST_TOKEN=approval-zone-rpc-token \
  MIR2_ZONE_HOST_METRICS_ADDR=127.0.0.1:19100 \
  MIR2_ZONE_HOST_MANAGEMENT_TOKEN="$MANAGEMENT_TOKEN" \
  MIR2_ACCOUNT_STORE_PATH="$RUN_DIR/accounts.json" \
  MIR2_WORLD_DIRECTOR_TRUSTED_PUBLIC_KEY="$DIRECTOR_PUBLIC_KEY" \
  MIR2_WORLD_DIRECTOR_COMMITTEE="$COMMITTEE" \
  MIR2_WORLD_DIRECTOR_CHECKPOINT_FILE="$RUN_DIR/zone-director.json" \
  MIR2_WORLD_DIRECTOR_TICK_MS=100 \
  target/debug/zone_host >"$RUN_DIR/zone.log" 2>&1 &
ZONE_PID=$!

wait_http() {
  local url="$1"
  for _ in $(seq 1 300); do
    if curl --silent --fail "$url" >/dev/null 2>&1; then return 0; fi
    sleep 0.1
  done
  return 1
}

wait_http http://127.0.0.1:19100/healthz

for validator_index in 0 1 2 3; do
  validator_p2p_port=$((18300 + validator_index))
  validator_api_port=$((18400 + validator_index))
  validator_bootstrap=""
  if [[ "$validator_index" != "0" ]]; then
    validator_bootstrap="0@127.0.0.1:18300"
  fi
  env \
    GATE14_VALIDATOR_SEED="$validator_index" \
    GATE14_VALIDATOR_ID="approval-validator-$validator_index" \
    GATE14_PARTICIPANTS=0,1,2,3 \
    GATE14_P2P_BIND="127.0.0.1:$validator_p2p_port" \
    GATE14_P2P_ADVERTISE="127.0.0.1:$validator_p2p_port" \
    GATE14_BOOTSTRAPPERS="$validator_bootstrap" \
    GATE14_API_BIND="127.0.0.1:$validator_api_port" \
    GATE14_DATA_DIR="$RUN_DIR/commonware-validator-$validator_index" \
    RUST_LOG=warn \
    target/debug/gate14_validator \
    >"$RUN_DIR/commonware-validator-$validator_index.log" 2>&1 &
  COMMONWARE_VALIDATOR_PIDS+=("$!")
done

for validator_index in 0 1 2 3; do
  wait_http "http://127.0.0.1:$((18400 + validator_index))/healthz"
done

env \
  GATE14_GATEWAY_ID=approval-gateway \
  GATE14_VALIDATOR_URLS=http://127.0.0.1:18400,http://127.0.0.1:18401,http://127.0.0.1:18402,http://127.0.0.1:18403 \
  GATE14_GATEWAY_BIND=127.0.0.1:18500 \
  GATE14_CONTROL_TOKEN="$COMMONWARE_TOKEN" \
  target/debug/gate14_gateway >"$RUN_DIR/commonware-gateway.log" 2>&1 &
COMMONWARE_GATEWAY_PID=$!
wait_http http://127.0.0.1:18500/healthz

start_admin() {
  env \
    ADMIN_API_ADDR=127.0.0.1:17420 \
    ADMIN_OPERATOR_TOKEN="$OPERATOR_TOKEN" \
    MIR2_WORLD_DIRECTOR_SIGNING_KEY_FILE="$RUN_DIR/director.key" \
    MIR2_WORLD_DIRECTOR_VALIDATOR_KEY_FILES="$VALIDATOR_FILES" \
    MIR2_WORLD_DIRECTOR_ZONE_HOST_URLS=http://127.0.0.1:19100 \
    MIR2_WORLD_DIRECTOR_MANAGEMENT_TOKEN="$MANAGEMENT_TOKEN" \
    MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_URL=http://127.0.0.1:18500 \
    MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_TOKEN="$COMMONWARE_TOKEN" \
    MIR2_WORLD_DIRECTOR_REQUIRE_REMOTE_COMMONWARE=true \
    MIR2_WORLD_DIRECTOR_APPROVAL_FILE="$RUN_DIR/approval-state.json" \
    MIR2_WORLD_DIRECTOR_AUTOMATIC_GENERATION=false \
    target/debug/mir2-admin-api >"$RUN_DIR/admin.log" 2>&1 &
  ADMIN_PID=$!
  wait_http http://127.0.0.1:17420/health
}

start_admin

AUTH_HEADERS=(
  -H "Authorization: Bearer $OPERATOR_TOKEN"
  -H "x-operator-id: approval-operator"
  -H "x-operator-email: approval@obelisk.local"
  -H "x-operator-role: world-director-operator"
  -H "x-operator-permissions: approval_manage,server_control,content_read,audit_read"
  -H "content-type: application/json"
)

NOW_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
cat >"$RUN_DIR/generate.json" <<JSON
{
  "snapshot": {
    "schema": "obelisk.world-director.v1",
    "snapshotId": "acceptance-${NOW_MS}",
    "gameId": "mir2",
    "regionId": "asia-hk",
    "observedAtMs": ${NOW_MS},
    "windowMs": 900000,
    "maps": [
      {"zoneId":"map:0","activePlayers":80,"medianLevel":18,"newPlayerCount":20,"returningPlayerCount":5,"monsterKills":8000,"bossKills":8,"playerDeaths":20,"completedQuests":50},
      {"zoneId":"map:D022","activePlayers":20,"medianLevel":26,"newPlayerCount":0,"returningPlayerCount":0,"monsterKills":4000,"bossKills":4,"playerDeaths":15,"completedQuests":15},
      {"zoneId":"map:D023","activePlayers":12,"medianLevel":29,"newPlayerCount":0,"returningPlayerCount":0,"monsterKills":3000,"bossKills":6,"playerDeaths":12,"completedQuests":8},
      {"zoneId":"map:D024","activePlayers":8,"medianLevel":31,"newPlayerCount":0,"returningPlayerCount":0,"monsterKills":1000,"bossKills":12,"playerDeaths":30,"completedQuests":2}
    ],
    "economy": {"goldCreated":2000000,"goldDestroyed":1200000,"medianTradePriceIndexBps":11200},
    "guilds": {"activeGuilds":9,"largestGuildPopulationBps":2500,"largestGuildBossKillShareBps":5800}
  }
}
JSON

curl --silent --fail "${AUTH_HEADERS[@]}" \
  --data-binary "@$RUN_DIR/generate.json" \
  http://127.0.0.1:17420/admin/world-director/proposals/generate \
  >"$RUN_DIR/proposal.json"
PROPOSAL_ID="$(json_field "$RUN_DIR/proposal.json" proposalId)"

curl --silent --fail "${AUTH_HEADERS[@]}" \
  --data '{"reason":"根据当前压力证据调整受限事件参数","durationMs":2100000,"rewardBudget":120000,"targetZones":["map:D022","map:D023","map:D024"]}' \
  "http://127.0.0.1:17420/admin/world-director/proposals/$PROPOSAL_ID/edit" \
  >"$RUN_DIR/edited.json"

curl --silent --fail "${AUTH_HEADERS[@]}" \
  --data '{"reason":"人工审批验收期间暂停新的导演命令"}' \
  http://127.0.0.1:17420/admin/world-director/control/pause \
  >"$RUN_DIR/paused.json"

HTTP_STATUS="$(
  curl --silent --output "$RUN_DIR/blocked.json" --write-out '%{http_code}' \
    "${AUTH_HEADERS[@]}" \
    --data '{"reason":"这次批准必须被全局暂停安全门阻止"}' \
    "http://127.0.0.1:17420/admin/world-director/proposals/$PROPOSAL_ID/approve"
)"
test "$HTTP_STATUS" = "409"

curl --silent --fail "${AUTH_HEADERS[@]}" \
  --data '{"reason":"安全门验收完成恢复人工审批处理"}' \
  http://127.0.0.1:17420/admin/world-director/control/resume \
  >"$RUN_DIR/resumed.json"

curl --silent --fail "${AUTH_HEADERS[@]}" \
  --data '{"reason":"已核对压力证据奖励预算目标地图并批准执行"}' \
  "http://127.0.0.1:17420/admin/world-director/proposals/$PROPOSAL_ID/approve" \
  >"$RUN_DIR/approved.json"

curl --silent --fail "${AUTH_HEADERS[@]}" \
  http://127.0.0.1:17420/admin/world-director \
  >"$RUN_DIR/dashboard-before-restart.json"
curl --silent --fail http://127.0.0.1:17420/metrics \
  >"$RUN_DIR/metrics.txt"
curl --silent --fail -H "Authorization: Bearer $MANAGEMENT_TOKEN" \
  http://127.0.0.1:19100/v1/world-director \
  >"$RUN_DIR/runtime.json"
curl --silent --fail http://127.0.0.1:18500/v1/status \
  >"$RUN_DIR/commonware-status.json"
curl --silent --fail http://127.0.0.1:18400/v1/state \
  >"$RUN_DIR/commonware-state.json"

kill "$ADMIN_PID"
wait "$ADMIN_PID" 2>/dev/null || true
ADMIN_PID=""
start_admin
curl --silent --fail "${AUTH_HEADERS[@]}" \
  http://127.0.0.1:17420/admin/world-director \
  >"$RUN_DIR/dashboard-after-restart.json"

node - "$RUN_DIR" "$PROPOSAL_ID" <<'NODE'
const fs = require("fs");
const [dir, proposalId] = process.argv.slice(2);
const read = (name) => JSON.parse(fs.readFileSync(`${dir}/${name}.json`, "utf8"));
const approved = read("approved");
const runtime = read("runtime");
const before = read("dashboard-before-restart");
const after = read("dashboard-after-restart");
const commonwareStatus = read("commonware-status");
const commonwareState = read("commonware-state");
const metrics = fs.readFileSync(`${dir}/metrics.txt`, "utf8");
if (approved.status !== "executing") throw new Error(`unexpected approval status ${approved.status}`);
if (approved.finalizedHeight !== 1) throw new Error("expected Commonware height 1");
if (approved.commonwareNetworkHeight !== 1 || !approved.commonwareNetworkStateRoot) {
  throw new Error("remote Commonware finality receipt is missing");
}
if (!approved.zoneReceipts?.length) throw new Error("missing Zone Host receipt");
if (runtime.finalizedHeight !== 1 || runtime.installedCommandCount !== 1) {
  throw new Error("Zone Host did not install finalized command");
}
if (commonwareStatus.finalizedHeight !== 1 || commonwareStatus.agreeingValidators?.length < 3) {
  throw new Error("remote Commonware quorum did not finalize the director anchor");
}
const commonwareAnchor = commonwareState.worldDirectorAnchors?.[approved.commandId];
if (!commonwareAnchor) {
  throw new Error("director command is absent from Commonware authoritative state");
}
if (
  commonwareAnchor.finalizedHeight !== approved.commonwareNetworkHeight ||
  commonwareAnchor.commandDigest !== approved.commonwareNetworkCommandDigest
) {
  throw new Error("Commonware anchor metadata does not match the approval receipt");
}
for (const expectedMetric of [
  'mir2_world_director_proposals{status="executing"} 1',
  "mir2_world_director_remote_anchors 1",
  "mir2_world_director_zone_receipts 1",
  'mir2_world_director_zone_targets{status="live"} 1',
]) {
  if (!metrics.includes(expectedMetric)) throw new Error(`missing metric: ${expectedMetric}`);
}
const restored = after.proposals.find((record) => record.proposalId === proposalId);
if (!restored || restored.status !== "executing") throw new Error("approval state did not recover");
if (after.audit.length < 6) throw new Error("decision audit chain is incomplete");
console.log(JSON.stringify({
  schema: "obelisk.world-director.approval.acceptance.v1",
  accepted: true,
  proposalId,
  status: approved.status,
  finalizedHeight: approved.finalizedHeight,
  finalizedDigest: approved.finalizedDigest,
  commonwareNetworkHeight: approved.commonwareNetworkHeight,
  commonwareNetworkStateRoot: approved.commonwareNetworkStateRoot,
  commonwareAgreeingValidators: commonwareStatus.agreeingValidators.length,
  commonwareAnchorVerified: true,
  commandId: approved.commandId,
  zoneReceiptCount: approved.zoneReceipts.length,
  zoneInstalledCommands: runtime.installedCommandCount,
  zoneAppliedActions: runtime.appliedActionCount,
  pauseSafetyGateVerified: true,
  boundedEditVerified: approved.proposal.durationMs === 2100000 && approved.proposal.rewardBudget === 120000,
  restartRecoveryVerified: restored.status === "executing",
  prometheusMetricsVerified: true,
  auditRecordCount: after.audit.length,
  auditHead: after.audit[0]?.recordHash,
  beforeRestartProposalCount: before.proposals.length
}, null, 2));
NODE
