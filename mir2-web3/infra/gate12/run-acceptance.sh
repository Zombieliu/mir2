#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
compose_file="${script_dir}/docker-compose.yml"
evidence_dir="${GATE12_EVIDENCE_DIR:-}"
keep_stack="${GATE12_KEEP_STACK:-0}"

if [[ -z "${evidence_dir}" ]]; then
  evidence_dir="$(mktemp -d "${TMPDIR:-/tmp}/obelisk-gate12.XXXXXX")"
fi
mkdir -p "${evidence_dir}"
evidence_dir="$(cd -- "${evidence_dir}" && pwd)"
export GATE12_EVIDENCE_DIR="${evidence_dir}"

cleanup() {
  if [[ "${keep_stack}" != "1" ]]; then
    docker compose -f "${compose_file}" --profile acceptance down --volumes --remove-orphans
  fi
}
trap cleanup EXIT

rm -f \
  "${evidence_dir}/primary-ready" \
  "${evidence_dir}/continue-after-primary-stop" \
  "${evidence_dir}/gate12-acceptance.json" \
  "${evidence_dir}/acceptance.log"

docker compose -f "${compose_file}" --profile acceptance up \
  --build --detach \
  postgres zone-host-a zone-host-b gateway zone-replicator prometheus grafana

wait_for_service() {
  local service="$1"
  local deadline=$((SECONDS + 120))
  local container_id=""
  local state=""
  local health=""
  while (( SECONDS < deadline )); do
    container_id="$(docker compose -f "${compose_file}" ps --quiet "${service}")"
    if [[ -n "${container_id}" ]]; then
      state="$(docker inspect --format '{{.State.Status}}' "${container_id}")"
      health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{end}}' "${container_id}")"
      if [[ "${state}" == "running" && ( -z "${health}" || "${health}" == "healthy" ) ]]; then
        return 0
      fi
      if [[ "${state}" == "exited" || "${health}" == "unhealthy" ]]; then
        docker compose -f "${compose_file}" logs --tail=100 "${service}"
        echo "Gate 12 service ${service} failed readiness (state=${state}, health=${health})" >&2
        return 1
      fi
    fi
    sleep 1
  done
  docker compose -f "${compose_file}" ps -a
  docker compose -f "${compose_file}" logs --tail=100 "${service}"
  echo "Timed out waiting for Gate 12 service ${service}" >&2
  return 1
}

for service in postgres zone-host-a zone-host-b gateway zone-replicator prometheus grafana; do
  wait_for_service "${service}"
done

prometheus_port="${GATE12_PROMETHEUS_PORT:-19090}"
grafana_port="${GATE12_GRAFANA_PORT:-13000}"
prometheus_query="$(
  curl --fail --silent --get \
    --data-urlencode 'query=sum(obelisk_zone_host_up)' \
    "http://127.0.0.1:${prometheus_port}/api/v1/query"
)"
grep -q '"status":"success"' <<<"${prometheus_query}"
grep -q '"value":' <<<"${prometheus_query}"
grafana_dashboard="$(
  curl --fail --silent \
    "http://127.0.0.1:${grafana_port}/api/dashboards/uid/obelisk-zone-hosts"
)"
grep -q '"title":"Obelisk Zone Hosts"' <<<"${grafana_dashboard}"

# `up` only builds the long-running services named above. Build the profiled
# acceptance target explicitly so its heartbeat schema cannot lag the nodes.
docker compose -f "${compose_file}" --profile acceptance build acceptance
docker compose -f "${compose_file}" --profile acceptance run --rm acceptance \
  >"${evidence_dir}/acceptance.log" 2>&1 &
acceptance_pid=$!

deadline=$((SECONDS + 120))
while [[ ! -f "${evidence_dir}/primary-ready" ]]; do
  if ! kill -0 "${acceptance_pid}" 2>/dev/null; then
    wait "${acceptance_pid}" || true
    cat "${evidence_dir}/acceptance.log"
    echo "Gate 12 acceptance exited before the primary-ready marker" >&2
    exit 1
  fi
  if (( SECONDS >= deadline )); then
    cat "${evidence_dir}/acceptance.log"
    echo "Timed out waiting for the Gate 12 primary-ready marker" >&2
    exit 1
  fi
  sleep 0.2
done

failure_started_ms="$(date +%s%3N 2>/dev/null || date +%s000)"
docker compose -f "${compose_file}" stop --timeout 1 zone-host-a
printf '%s\n' "${failure_started_ms}" >"${evidence_dir}/continue-after-primary-stop"

wait "${acceptance_pid}"

test -s "${evidence_dir}/gate12-acceptance.json"
grep -q '"accepted": true' "${evidence_dir}/gate12-acceptance.json"

docker compose -f "${compose_file}" ps
cat "${evidence_dir}/gate12-acceptance.json"
rm -f \
  "${evidence_dir}/primary-ready" \
  "${evidence_dir}/continue-after-primary-stop" \
  "${evidence_dir}/acceptance.log"
printf 'Gate 12 acceptance evidence: %s\n' "${evidence_dir}/gate12-acceptance.json"
