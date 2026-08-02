#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mir2-director-postgres.XXXXXX")"
POSTGRES_PORT="${MIR2_DIRECTOR_POSTGRES_ACCEPTANCE_PORT:-55439}"
ADMIN_A_PORT="${MIR2_DIRECTOR_ADMIN_A_ACCEPTANCE_PORT:-17620}"
ADMIN_B_PORT="${MIR2_DIRECTOR_ADMIN_B_ACCEPTANCE_PORT:-17621}"
ADMIN_A_PID=""
ADMIN_B_PID=""
REQUEST_PIDS=("")

cleanup() {
  if [[ -n "$ADMIN_A_PID" ]]; then kill "$ADMIN_A_PID" 2>/dev/null || true; fi
  if [[ -n "$ADMIN_B_PID" ]]; then kill "$ADMIN_B_PID" 2>/dev/null || true; fi
  wait "$ADMIN_A_PID" 2>/dev/null || true
  wait "$ADMIN_B_PID" 2>/dev/null || true
  pg_ctl -D "$RUN_DIR/postgres" -m fast stop >/dev/null 2>&1 || true
  case "$RUN_DIR" in
    "${TMPDIR:-/tmp}"/mir2-director-postgres.*) rm -rf -- "$RUN_DIR" ;;
  esac
}
trap cleanup EXIT

for command in initdb pg_ctl createdb psql curl jq; do
  command -v "$command" >/dev/null || {
    echo "missing required command: $command" >&2
    exit 2
  }
done

cd "$ROOT_DIR"
cargo build -p mir2-admin-api --bin mir2-admin-api

initdb -D "$RUN_DIR/postgres" -A trust -U postgres >"$RUN_DIR/initdb.log"
pg_ctl -D "$RUN_DIR/postgres" \
  -o "-p $POSTGRES_PORT -h 127.0.0.1" \
  -l "$RUN_DIR/postgres.log" start >/dev/null
createdb -h 127.0.0.1 -p "$POSTGRES_PORT" -U postgres mir2
DATABASE_URL="postgres://postgres@127.0.0.1:$POSTGRES_PORT/mir2"

start_admin() {
  local port="$1"
  env \
    ADMIN_API_ADDR="127.0.0.1:$port" \
    ADMIN_DATABASE_URL="$DATABASE_URL" \
    ADMIN_OPERATOR_TOKEN=pg-concurrency-token \
    MIR2_WORLD_DIRECTOR_AUTOMATIC_GENERATION=false \
    target/debug/mir2-admin-api >"$RUN_DIR/admin-$port.log" 2>&1 &
  printf '%s' "$!"
}

wait_http() {
  local url="$1"
  for _ in $(seq 1 200); do
    if curl --silent --fail "$url" >/dev/null 2>&1; then return 0; fi
    sleep 0.05
  done
  return 1
}

ADMIN_A_PID="$(start_admin "$ADMIN_A_PORT")"
wait_http "http://127.0.0.1:$ADMIN_A_PORT/health"
ADMIN_B_PID="$(start_admin "$ADMIN_B_PORT")"
wait_http "http://127.0.0.1:$ADMIN_B_PORT/health"

AUTH_HEADERS=(
  -H "Authorization: Bearer pg-concurrency-token"
  -H "x-operator-id: pg-operator"
  -H "x-operator-email: pg@obelisk.local"
  -H "x-operator-role: director-operator"
  -H "x-operator-permissions: approval_manage,server_control"
  -H "content-type: application/json"
)

cat >"$RUN_DIR/generate.json" <<'JSON'
{
  "snapshot": {
    "schema": "obelisk.world-director.v1",
    "snapshotId": "postgres-concurrency",
    "gameId": "mir2",
    "regionId": "asia-hk",
    "observedAtMs": 1785318000000,
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

PROPOSAL_ID="$(
  curl --silent --fail "${AUTH_HEADERS[@]}" \
    --data-binary "@$RUN_DIR/generate.json" \
    "http://127.0.0.1:$ADMIN_A_PORT/admin/world-director/proposals/generate" |
    jq -r .proposalId
)"

for request_index in $(seq 1 40); do
  port="$ADMIN_A_PORT"
  if (( request_index % 2 )); then port="$ADMIN_B_PORT"; fi
  (
    curl --silent --output "$RUN_DIR/response-$request_index.json" \
      --write-out '%{http_code}' \
      -H "Authorization: Bearer pg-concurrency-token" \
      -H "x-operator-id: pg-operator-$request_index" \
      -H "x-operator-email: pg-$request_index@obelisk.local" \
      -H "x-operator-role: director-operator" \
      -H "x-operator-permissions: approval_manage,server_control" \
      -H "content-type: application/json" \
      --data "{\"reason\":\"concurrent bounded edit number $request_index\",\"durationMs\":2100000,\"rewardBudget\":120000,\"targetZones\":[\"map:D022\",\"map:D023\",\"map:D024\"]}" \
      "http://127.0.0.1:$port/admin/world-director/proposals/$PROPOSAL_ID/edit" \
      >"$RUN_DIR/status-$request_index"
  ) &
  REQUEST_PIDS+=("$!")
done
for pid in "${REQUEST_PIDS[@]}"; do
  [[ -n "$pid" ]] || continue
  wait "$pid"
done

SUCCESS_COUNT="$(
  { grep -l '^200$' "$RUN_DIR"/status-* || true; } | wc -l | tr -d ' '
)"
CONFLICT_COUNT="$(
  { grep -l '^409$' "$RUN_DIR"/status-* || true; } | wc -l | tr -d ' '
)"
UNEXPECTED_COUNT="$(
  { grep -L -E '^(200|409)$' "$RUN_DIR"/status-* || true; } | wc -l | tr -d ' '
)"
REVISION="$(psql "$DATABASE_URL" -Atc \
  'SELECT revision FROM world_director_control_state WHERE singleton=TRUE')"
AUDIT_ROWS="$(psql "$DATABASE_URL" -Atc 'SELECT count(*) FROM world_director_audit')"
CHECKPOINT_AUDIT="$(psql "$DATABASE_URL" -Atc \
  "SELECT jsonb_array_length(checkpoint_json->'audit') FROM world_director_control_state WHERE singleton=TRUE")"
EXPECTED="$((SUCCESS_COUNT + 1))"

test "$UNEXPECTED_COUNT" -eq 0
test "$CONFLICT_COUNT" -gt 0
test "$REVISION" -eq "$EXPECTED"
test "$AUDIT_ROWS" -eq "$EXPECTED"
test "$CHECKPOINT_AUDIT" -eq "$EXPECTED"

jq -n \
  --arg proposalId "$PROPOSAL_ID" \
  --argjson concurrentRequests 40 \
  --argjson successfulTransitions "$SUCCESS_COUNT" \
  --argjson rejectedConflicts "$CONFLICT_COUNT" \
  --argjson finalRevision "$REVISION" \
  --argjson auditRows "$AUDIT_ROWS" \
  '{
    schema: "obelisk.world-director.postgres-concurrency.acceptance.v1",
    accepted: true,
    proposalId: $proposalId,
    concurrentRequests: $concurrentRequests,
    successfulTransitions: $successfulTransitions,
    rejectedConflicts: $rejectedConflicts,
    finalRevision: $finalRevision,
    auditRows: $auditRows,
    lostUpdates: 0
  }'
