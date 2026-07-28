#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
base_compose="${repo_root}/infra/gate19/docker-compose.yml"
gate21_compose="${script_dir}/docker-compose.yml"
evidence_dir="${repo_root}/docs/generated/regional"
previous_zone_image="${MIR2_GATE21_PREVIOUS_ZONE_IMAGE:-}"
rolling_zone_image="mir2-gate21-zone-host:rolling"
current_zone_image="mir2-gate21-zone-host:current"
export MIR2_GATE21_ZONE_IMAGE="${rolling_zone_image}"

if [[ -z "${previous_zone_image}" ]]; then
  echo "MIR2_GATE21_PREVIOUS_ZONE_IMAGE is required for a real rolling-upgrade test" >&2
  exit 1
fi

compose() {
  docker compose -f "${base_compose}" -f "${gate21_compose}" --profile acceptance "$@"
}

wait_healthy() {
  local service="$1"
  local attempts="${2:-180}"
  local container state
  for _ in $(seq 1 "${attempts}"); do
    container="$(compose ps -q "${service}")"
    state="$(
      docker inspect -f \
        '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' \
        "${container}" 2>/dev/null || true
    )"
    [[ "${state}" == "healthy" ]] && return 0
    sleep 1
  done
  compose logs --no-color "${service}" >&2 || true
  return 1
}

wait_log_count_increase() {
  local pattern="$1"
  local previous_count="$2"
  local attempts="${3:-300}"
  local count
  for _ in $(seq 1 "${attempts}"); do
    count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "${pattern}" || true)"
    (( count > previous_count )) && return 0
    sleep 0.1
  done
  compose logs --no-color zone-replicator >&2 || true
  return 1
}

wait_seed_ready() {
  local container="$1"
  local attempts="${2:-300}"
  for _ in $(seq 1 "${attempts}"); do
    docker logs "${container}" 2>&1 | rg -q 'GATE19_ZONE_SEED_READY' && return 0
    state="$(docker inspect -f '{{.State.Status}}' "${container}" 2>/dev/null || true)"
    [[ "${state}" == "exited" || "${state}" == "dead" ]] && break
    sleep 0.1
  done
  docker logs "${container}" >&2 || true
  return 1
}

wait_exit_zero() {
  local container="$1"
  local attempts="${2:-600}"
  local state
  for _ in $(seq 1 "${attempts}"); do
    state="$(docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' "${container}" 2>/dev/null || true)"
    [[ "${state}" == "exited 0" ]] && return 0
    if [[ "${state}" == exited* || "${state}" == dead* ]]; then
      docker logs "${container}" >&2 || true
      return 1
    fi
    sleep 0.1
  done
  docker logs "${container}" >&2 || true
  return 1
}

start_seed() {
  local promoted_owner="$1"
  local output_name="$2"
  compose run -d --no-deps \
    -e MIR2_ZONE_STANDBY_OWNER_ID="${promoted_owner}" \
    -e MIR2_GATE19_ZONE_SESSION_OUT="/evidence/${output_name}" \
    zone-seed
}

probe() {
  local phase="$1"
  local output_name="$2"
  shift 2
  compose run --rm --no-deps \
    -e MIR2_GATE19_PROBE_PHASE="${phase}" \
    -e MIR2_GATE19_INFRA_PROBE_OUT="/evidence/${output_name}" \
    "$@" infra-probe
}

mkdir -p "${evidence_dir}"
"${script_dir}/preflight-reference.sh" \
  "${evidence_dir}/gate21-fault-resource-attestation.json"
compose down -v --remove-orphans >/dev/null 2>&1 || true
trap 'compose down -v --remove-orphans >/dev/null 2>&1 || true' EXIT

docker build --target zone-host -t "${current_zone_image}" "${repo_root}"
previous_image_id="$(docker image inspect "${previous_zone_image}" --format '{{.Id}}')"
current_image_id="$(docker image inspect "${current_zone_image}" --format '{{.Id}}')"
if [[ "${previous_image_id}" == "${current_image_id}" ]]; then
  echo "rolling upgrade requires different previous/current Zone image digests" >&2
  exit 1
fi
docker tag "${previous_zone_image}" "${rolling_zone_image}"

compose build gateway-1 zone-replicator zone-seed gameplay-probe infra-probe
compose up -d \
  postgres-primary postgres-standby \
  redis-primary redis-replica-1 redis-replica-2 \
  redis-sentinel-1 redis-sentinel-2 redis-sentinel-3 \
  zone-active zone-active-2 zone-active-3 zone-active-4 \
  zone-active-5 zone-active-6 zone-active-7 zone-standby \
  gateway-1 gateway-2 gateway-3 zone-replicator

for service in \
  zone-active zone-active-2 zone-active-3 zone-active-4 \
  zone-active-5 zone-active-6 zone-active-7 zone-standby \
  gateway-1 gateway-2 gateway-3; do
  wait_healthy "${service}"
done
wait_log_count_increase 'ZONE_REPLICATOR_SYNCHRONIZED zone=map:0:line:1 ' 0 900

pair_index="$(
  python3 - <<'PY'
import hashlib
print(int.from_bytes(hashlib.sha256(b"map:0").digest()[:8], "big") % 4)
PY
)"
active_services=(zone-active zone-active-2 zone-active-3 zone-active-4)
standby_services=(zone-active-5 zone-active-6 zone-active-7 zone-standby)
active_service="${active_services[${pair_index}]}"
standby_service="${standby_services[${pair_index}]}"
target_zone="map:0:line:1"

# 1. A paired standby loss must not interrupt authoritative gameplay.
compose kill -s KILL "${standby_service}"
compose run --rm --no-deps \
  -e MIR2_GATE18_GAMEPLAY_OUT="/evidence/gate21-standby-zone-kill.json" \
  gameplay-probe
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
compose start "${standby_service}"
wait_healthy "${standby_service}"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900

# 2. Kill the active process with one real player attached. The durable
# replicator must fence, promote, refresh the lease, and resume under 5 seconds.
seed_active_kill="$(start_seed gate21-standby gate21-active-zone-kill-session.json)"
wait_seed_ready "${seed_active_kill}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900
active_fault_started_at_ms="$(( $(date +%s) * 1000 ))"
compose kill -s KILL "${active_service}"
wait_exit_zero "${seed_active_kill}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
compose start "${active_service}"
wait_healthy "${active_service}"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900

# 3. A Gateway process loss leaves two healthy replicas and moves its route.
probe preflight gate21-infra-preflight.json \
  -e MIR2_GATE19_PROBE_KEY=gate21-shared-route \
  -e MIR2_GATE19_ROUTE_OWNER=gate21-gateway-1
gateway_fault_started_at_ms="$(( $(date +%s) * 1000 ))"
compose kill -s KILL gateway-1
sleep 2
probe gateway-kill gate21-gateway-kill.json \
  -e MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS=2 \
  -e MIR2_GATE19_PROBE_KEY=gate21-shared-route \
  -e MIR2_GATE19_ROUTE_OWNER=gate21-gateway-2 \
  -e MIR2_GATE19_FAULT_STARTED_AT_MS="${gateway_fault_started_at_ms}"
compose start gateway-1
wait_healthy gateway-1

# 4. Sentinel promotes a different writable Redis master.
preflight_master="$(jq -r '.redisMasterAddress' "${evidence_dir}/gate21-infra-preflight.json")"
compose kill -s KILL redis-primary
promoted_master=""
for _ in $(seq 1 180); do
  promoted_master="$(
    compose exec -T redis-sentinel-1 \
      redis-cli -p 26379 --raw SENTINEL get-master-addr-by-name mir2-primary \
      2>/dev/null | paste -sd: - || true
  )"
  [[ -n "${promoted_master}" && "${promoted_master}" != "${preflight_master}" ]] && break
  sleep 0.1
done
if [[ -z "${promoted_master}" || "${promoted_master}" == "${preflight_master}" ]]; then
  echo "Redis Sentinel did not promote a new master" >&2
  exit 1
fi
probe redis-primary-failover gate21-redis-primary-failover.json \
  -e MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS=3 \
  -e MIR2_GATE19_ROUTE_OWNER=gate21-gateway-3

# 5. Run the production Commonware v2026.2.0 3-of-4 finality/catch-up fault.
python3 "${repo_root}/scripts/gate14_acceptance.py" \
  --reset \
  --down-after \
  --evidence "${evidence_dir}/gate21-commonware-validator-kill.json"

# 6. Replace both members of the live pair with a genuinely different image.
# Each owner replacement carries a real Session, and the opposite member must
# fully catch up before the second owner is touched. Then roll the six hosts
# outside the measured pair one at a time.
seed_rolling="$(start_seed gate21-active gate21-rolling-upgrade-session.json)"
wait_seed_ready "${seed_rolling}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900
rolling_started_at_ms="$(( $(date +%s) * 1000 ))"
docker tag "${current_zone_image}" "${rolling_zone_image}"
compose up --no-deps --force-recreate -d "${standby_service}"
wait_exit_zero "${seed_rolling}"
wait_healthy "${standby_service}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900

seed_rolling_return="$(start_seed gate21-standby gate21-rolling-upgrade-return-session.json)"
wait_seed_ready "${seed_rolling_return}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900
compose up --no-deps --force-recreate -d "${active_service}"
wait_exit_zero "${seed_rolling_return}"
wait_healthy "${active_service}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900

for service in \
  zone-active zone-active-2 zone-active-3 zone-active-4 \
  zone-active-5 zone-active-6 zone-active-7 zone-standby; do
  [[ "${service}" == "${standby_service}" || "${service}" == "${active_service}" ]] && continue
  compose up --no-deps --force-recreate -d "${service}"
  wait_healthy "${service}"
done
for service in \
  zone-active zone-active-2 zone-active-3 zone-active-4 \
  zone-active-5 zone-active-6 zone-active-7 zone-standby; do
  container="$(compose ps -q "${service}")"
  observed_image="$(docker inspect -f '{{.Image}}' "${container}")"
  if [[ "${observed_image}" != "${current_image_id}" ]]; then
    echo "${service} did not roll to ${current_image_id}" >&2
    exit 1
  fi
done

# 7. Partition the current paired owner from the data/control network. The
# player must move to the opposite fenced owner and preserve identity/map.
seed_partition="$(start_seed gate21-active gate21-network-partition-session.json)"
wait_seed_ready "${seed_partition}"
sync_count="$(compose logs --no-color zone-replicator 2>&1 | rg -c "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " || true)"
wait_log_count_increase "ZONE_REPLICATOR_SYNCHRONIZED zone=${target_zone} " "${sync_count}" 900
partition_started_at_ms="$(( $(date +%s) * 1000 ))"
partition_container="$(compose ps -q "${standby_service}")"
partition_network="$(
  docker inspect "${partition_container}" |
    jq -r '.[0].NetworkSettings.Networks | keys[0]'
)"
docker network disconnect "${partition_network}" "${partition_container}"
wait_exit_zero "${seed_partition}"
docker network connect --alias "${standby_service}" \
  "${partition_network}" "${partition_container}"
wait_healthy "${standby_service}"

# 8. The physical PostgreSQL standby becomes the selected writable endpoint.
# This is intentionally last: rebuilding the old primary as a new replica is
# an operator follow-up, not part of the measured failover RTO.
preflight_postgres="$(
  jq -r '.postgresServerAddress' "${evidence_dir}/gate21-infra-preflight.json"
)"
postgres_fault_started_at_ms="$(( $(date +%s) * 1000 ))"
compose stop postgres-primary
compose exec -T -u postgres postgres-standby \
  pg_ctl -D /var/lib/postgresql/data promote
for _ in $(seq 1 180); do
  recovery="$(
    compose exec -T postgres-standby \
      psql -U mir2 -d mir2 -tAc 'SELECT pg_is_in_recovery()' \
      2>/dev/null || true
  )"
  [[ "${recovery}" == "f" ]] && break
  sleep 0.1
done
[[ "${recovery:-}" == "f" ]] || {
  echo "PostgreSQL standby did not promote" >&2
  exit 1
}
probe postgres-primary-failover gate21-postgres-primary-failover.json \
  -e MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS=3 \
  -e MIR2_GATE19_ROUTE_OWNER=gate21-gateway-2

jq -n \
  --argjson generatedAtMs "$(( $(date +%s) * 1000 ))" \
  --arg gitCommit "$(git -C "${repo_root}" rev-parse HEAD)" \
  --arg targetZone "${target_zone}" \
  --arg activeService "${active_service}" \
  --arg standbyService "${standby_service}" \
  --arg previousImage "${previous_image_id}" \
  --arg currentImage "${current_image_id}" \
  --arg preflightPostgres "${preflight_postgres}" \
  --argjson activeFaultStartedAtMs "${active_fault_started_at_ms}" \
  --argjson gatewayFaultStartedAtMs "${gateway_fault_started_at_ms}" \
  --argjson rollingStartedAtMs "${rolling_started_at_ms}" \
  --argjson partitionStartedAtMs "${partition_started_at_ms}" \
  --argjson postgresFaultStartedAtMs "${postgres_fault_started_at_ms}" \
  '{
    schemaVersion: 1,
    gate: 21,
    generatedAtMs: $generatedAtMs,
    gitCommit: $gitCommit,
    targetZone: $targetZone,
    pair: {activeService: $activeService, standbyService: $standbyService},
    rollingUpgrade: {
      previousImage: $previousImage,
      currentImage: $currentImage,
      startedAtMs: $rollingStartedAtMs
    },
    faultStartedAtMs: {
      activeZoneHost: $activeFaultStartedAtMs,
      gateway: $gatewayFaultStartedAtMs,
      networkPartition: $partitionStartedAtMs,
      postgres: $postgresFaultStartedAtMs
    },
    preflightPostgresServer: $preflightPostgres
  }' >"${evidence_dir}/gate21-fault-runtime-manifest.json"

python3 "${script_dir}/verify-faults.py" \
  --evidence-dir "${evidence_dir}" \
  --output "${evidence_dir}/gate21-faults.json"
