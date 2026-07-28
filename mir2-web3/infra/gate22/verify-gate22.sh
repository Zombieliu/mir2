#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
EVIDENCE_DIR="${REPO_ROOT}/docs/generated/home-node"
COMPOSE=(docker compose -f "${SCRIPT_DIR}/docker-compose.yml")

mkdir -p "${EVIDENCE_DIR}"
"${SCRIPT_DIR}/prepare-fixtures.sh"

cleanup() {
  "${COMPOSE[@]}" down --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

"${COMPOSE[@]}" build zone-host home-relay home-agent acceptance
"${COMPOSE[@]}" up -d zone-host home-relay home-agent

deadline=$((SECONDS + 60))
until "${COMPOSE[@]}" logs --no-color home-agent 2>/dev/null | grep -q "HOME_AGENT_READY"; do
  if (( SECONDS >= deadline )); then
    "${COMPOSE[@]}" ps
    "${COMPOSE[@]}" logs --no-color
    echo "Home Agent did not become ready within 60 seconds" >&2
    exit 1
  fi
  sleep 1
done

"${COMPOSE[@]}" --profile acceptance run --rm acceptance \
  | tee "${EVIDENCE_DIR}/gate22-docker-initial.json"
grep -q '"accepted": true' "${EVIDENCE_DIR}/gate22-docker-initial.json"

"${COMPOSE[@]}" restart home-agent
deadline=$((SECONDS + 60))
until [[ "$("${COMPOSE[@]}" logs --no-color --since 10s home-agent 2>/dev/null | grep -c "HOME_AGENT_READY" || true)" -ge 1 ]]; do
  if (( SECONDS >= deadline )); then
    "${COMPOSE[@]}" logs --no-color home-agent
    echo "Home Agent did not re-register after restart" >&2
    exit 1
  fi
  sleep 1
done

MIR2_HOME_ACCEPTANCE_SESSION_ID=gate22-docker-session-after-restart \
  "${COMPOSE[@]}" --profile acceptance run --rm acceptance \
  | tee "${EVIDENCE_DIR}/gate22-docker-reconnect.json"
grep -q '"accepted": true' "${EVIDENCE_DIR}/gate22-docker-reconnect.json"

echo "GATE22_DOCKER_ACCEPTED"
