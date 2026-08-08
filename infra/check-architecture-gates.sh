#!/usr/bin/env bash
set -euo pipefail

export CARGO_CACHE_AUTO_CLEAN_FREQUENCY="${CARGO_CACHE_AUTO_CLEAN_FREQUENCY:-never}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run_optional_docker_compose_config() {
  if ! command -v docker >/dev/null 2>&1; then
    printf '\n==> skip docker compose config (docker is not installed)\n'
    return 0
  fi

  run docker compose -f infra/docker-compose.dev.yml config --quiet
}

run cargo +1.89.0 fmt --check -p mir2-gateway -p mir2-admin-api -p mir2-simulation
run cargo +1.89.0 check --locked -p mir2-gateway -p mir2-admin-api -p mir2-simulation

run cargo +1.89.0 test --locked -p mir2-gateway shared_in_process_registry -- --test-threads=1
run cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1
run cargo +1.89.0 test --locked -p mir2-gateway gameplay_event -- --test-threads=1
run cargo +1.89.0 test --locked -p mir2-gateway health_reports_cache_and_gameplay_event_boundaries -- --test-threads=1
run cargo +1.89.0 test --locked -p mir2-admin-api gameplay_event -- --test-threads=1
run cargo +1.89.0 test --locked -p mir2-simulation account_store_repository -- --test-threads=1

run npm run typecheck --prefix apps/admin-web
run_optional_docker_compose_config
run git diff --check
