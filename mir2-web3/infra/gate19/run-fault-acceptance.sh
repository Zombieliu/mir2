#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
gate14_compose="${repo_root}/infra/gate14/docker-compose.yml"
evidence_dir="${repo_root}/docs/generated/regional"

compose() {
  docker compose -f "${compose_file}" --profile acceptance "$@"
}

wait_healthy() {
  local container="$1"
  local attempts="${2:-120}"
  local state
  for _ in $(seq 1 "${attempts}"); do
    state="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "${container}" 2>/dev/null || true)"
    if [[ "${state}" == "healthy" ]]; then
      return 0
    fi
    sleep 1
  done
  docker inspect "${container}" >&2 || true
  return 1
}

wait_log() {
  local service="$1"
  local pattern="$2"
  local attempts="${3:-120}"
  for _ in $(seq 1 "${attempts}"); do
    if compose logs --no-color "${service}" 2>&1 | rg -q "${pattern}"; then
      return 0
    fi
    sleep 1
  done
  compose logs --no-color "${service}" >&2 || true
  return 1
}

wait_exit_zero() {
  local container="$1"
  local attempts="${2:-300}"
  local state
  for _ in $(seq 1 "${attempts}"); do
    state="$(docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' "${container}" 2>/dev/null || true)"
    if [[ "${state}" == "exited 0" ]]; then
      return 0
    fi
    if [[ "${state}" == exited* ]]; then
      docker logs "${container}" >&2 || true
      return 1
    fi
    sleep 0.1
  done
  docker logs "${container}" >&2 || true
  return 1
}

probe() {
  local phase="$1"
  local output="$2"
  shift 2
  compose run --rm --no-deps \
    -e MIR2_GATE19_PROBE_PHASE="${phase}" \
    -e MIR2_GATE19_INFRA_PROBE_OUT="/evidence/${output}" \
    "$@" \
    infra-probe
}

mkdir -p "${evidence_dir}"
compose down -v --remove-orphans >/dev/null 2>&1 || true
docker compose -f "${gate14_compose}" down -v --remove-orphans >/dev/null 2>&1 || true

compose build \
  zone-active \
  zone-standby \
  gateway-1 \
  infra-probe \
  zone-seed \
  zone-failover-controller \
  gameplay-probe

git_commit="$(git -C "${repo_root}" rev-parse HEAD)"
zone_image="$(docker image inspect mir2-gate19-zone-host:local --format '{{.Id}}')"
gateway_image="$(docker image inspect mir2-gate19-gateway:local --format '{{.Id}}')"
infra_probe_image="$(docker image inspect mir2-gate19-infra-probe:local --format '{{.Id}}')"
zone_seed_image="$(docker image inspect mir2-gate19-zone-seed:local --format '{{.Id}}')"
controller_image="$(
  docker image inspect mir2-gate19-zone-failover-controller:local --format '{{.Id}}'
)"
gameplay_image="$(docker image inspect mir2-gate19-gameplay-probe:local --format '{{.Id}}')"
jq -n \
  --argjson generatedAtMs "$(( $(date +%s) * 1000 ))" \
  --arg gitCommit "${git_commit}" \
  --arg zone "${zone_image}" \
  --arg gateway "${gateway_image}" \
  --arg infraProbe "${infra_probe_image}" \
  --arg zoneSeed "${zone_seed_image}" \
  --arg controller "${controller_image}" \
  --arg gameplayProbe "${gameplay_image}" \
  '{
    schemaVersion: 1,
    gate: 19,
    generatedAtMs: $generatedAtMs,
    gitCommit: $gitCommit,
    images: {
      zone: $zone,
      gateway: $gateway,
      infraProbe: $infraProbe,
      zoneSeed: $zoneSeed,
      failoverController: $controller,
      gameplayProbe: $gameplayProbe
    },
    success:
      (($gitCommit | length) >= 7)
      and all([$zone, $gateway, $infraProbe, $zoneSeed, $controller, $gameplayProbe][];
        startswith("sha256:"))
  }' >"${evidence_dir}/gate19-runtime-manifest.json"
jq -e '.success == true' "${evidence_dir}/gate19-runtime-manifest.json" >/dev/null

compose up -d \
  postgres-primary \
  postgres-standby \
  redis-primary \
  redis-replica-1 \
  redis-replica-2 \
  redis-sentinel-1 \
  redis-sentinel-2 \
  redis-sentinel-3 \
  zone-active \
  zone-standby \
  gateway-1 \
  gateway-2 \
  gateway-3

probe preflight gate19-infra-preflight.json \
  -e MIR2_GATE19_PROBE_KEY=gate19-shared-route \
  -e MIR2_GATE19_ROUTE_OWNER=gate19-gateway-1

# A dead standby must not disturb real gameplay on the authoritative active.
compose kill -s KILL zone-standby
compose run --rm --no-deps gameplay-probe
compose start zone-standby
wait_healthy mir2-gate19-zone-standby-1

# The same short Redis route lease must be recoverable by another Gateway.
fault_started_at_ms="$(( $(date +%s) * 1000 ))"
compose kill -s KILL gateway-1
sleep 2
probe gateway-kill gate19-infra-gateway-kill.json \
  -e MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS=2 \
  -e MIR2_GATE19_PROBE_KEY=gate19-shared-route \
  -e MIR2_GATE19_ROUTE_OWNER=gate19-gateway-2 \
  -e MIR2_GATE19_FAULT_STARTED_AT_MS="${fault_started_at_ms}"

# Sentinel must nominate a different writable Redis master.
preflight_master="$(jq -r '.redisMasterAddress' "${evidence_dir}/gate19-infra-preflight.json")"
compose kill -s KILL redis-primary
for _ in $(seq 1 120); do
  promoted_master="$(
    compose exec -T redis-sentinel-1 \
      redis-cli -p 26379 --raw SENTINEL get-master-addr-by-name mir2-primary \
      2>/dev/null | paste -sd: - || true
  )"
  if [[ -n "${promoted_master}" && "${promoted_master}" != "${preflight_master}" ]]; then
    break
  fi
  sleep 0.1
done
if [[ -z "${promoted_master:-}" || "${promoted_master}" == "${preflight_master}" ]]; then
  echo "Redis Sentinel did not promote a new master" >&2
  exit 1
fi
probe redis-primary-failover gate19-infra-redis-failover.json \
  -e MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS=2 \
  -e MIR2_GATE19_ROUTE_OWNER=gate19-gateway-3

# Seed one real player, copy its live session, kill the active process, and
# require both the controller and player continuity probes to exit successfully.
compose up --no-deps --force-recreate -d zone-seed
wait_log zone-seed 'GATE19_ZONE_SEED_READY'
compose up --no-deps --force-recreate -d zone-failover-controller
wait_log zone-failover-controller 'GATE19_ZONE_SYNCHRONIZED cursor=[1-9]'
compose kill -s KILL zone-active
wait_exit_zero mir2-gate19-zone-failover-controller-1
wait_exit_zero mir2-gate19-zone-seed-1

# The physical PostgreSQL replica becomes the selected writable endpoint.
compose stop postgres-primary
compose exec -T -u postgres postgres-standby \
  pg_ctl -D /var/lib/postgresql/data promote
for _ in $(seq 1 120); do
  recovery="$(
    compose exec -T postgres-standby \
      psql -U mir2 -d mir2 -tAc 'SELECT pg_is_in_recovery()' \
      2>/dev/null || true
  )"
  if [[ "${recovery}" == "f" ]]; then
    break
  fi
  sleep 0.1
done
if [[ "${recovery:-}" != "f" ]]; then
  echo "PostgreSQL standby did not promote" >&2
  exit 1
fi
probe postgres-primary-failover gate19-infra-postgres-failover.json \
  -e MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS=2 \
  -e MIR2_GATE19_ROUTE_OWNER=gate19-gateway-2

# Reuse the production Commonware v2026.2.0 four-validator harness. It stops
# validator 3, finalizes through 3-of-4, restarts it, and imports certificates.
commonware_passed=0
for _ in 1 2; do
  if python3 "${repo_root}/scripts/gate14_acceptance.py" \
    --reset \
    --down-after \
    --evidence "${evidence_dir}/gate19-commonware-validator.json"; then
    commonware_passed=1
    break
  fi
done
if [[ "${commonware_passed}" != "1" ]]; then
  echo "Commonware validator fault acceptance did not pass" >&2
  exit 1
fi

jq -e '.success == true' \
  "${evidence_dir}/gate19-standby-zone-kill.json" \
  "${evidence_dir}/gate19-zone-failover.json" \
  "${evidence_dir}/gate19-zone-session.json" \
  "${evidence_dir}/gate19-infra-preflight.json" \
  "${evidence_dir}/gate19-infra-gateway-kill.json" \
  "${evidence_dir}/gate19-infra-redis-failover.json" \
  "${evidence_dir}/gate19-infra-postgres-failover.json" >/dev/null
jq -e '.accepted == true' \
  "${evidence_dir}/gate19-commonware-validator.json" >/dev/null

echo "Gate 19 six-fault acceptance evidence is complete"
