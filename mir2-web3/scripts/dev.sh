#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repository_root="$(cd "${project_root}/.." && pwd)"
compose_file="${project_root}/infra/compose.developer.yml"
command="${1:-up}"
if [[ $# -gt 0 ]]; then
  shift
fi

web_port="${MIR2_WEB_PORT:-3002}"
gateway_web_port="${MIR2_GATEWAY_WEB_PORT:-7110}"
gateway_tcp_port="${MIR2_GATEWAY_TCP_PORT:-7000}"
bind_address="${MIR2_BIND_ADDRESS:-127.0.0.1}"
gateway_ws_url="${MIR2_GATEWAY_WS_URL:-}"
asset_base_url="${MIR2_ASSET_BASE_URL:-}"
open_browser=0
build=0
full_assets=0
remove_volumes=0

usage() {
  cat <<'EOF'
Usage: ./scripts/dev.sh [command] [options]

Commands:
  doctor  Validate Docker, version locks, submodule, and Compose
  auth    Authorize the private GitHub Release in a persistent Docker volume
  build   Build the pinned developer image
  up      Start Gateway and Player Web in the background (default)
  down    Stop the developer services
  logs    Follow Gateway and Player Web logs
  shell   Open a shell in the pinned developer image
  verify  Run Player Web typecheck and Gateway cargo check
  assets  Download and install the pinned full asset bundle
  status  Show container and HTTP readiness

Options:
  --web-port PORT
  --gateway-web-port PORT
  --gateway-tcp-port PORT
  --bind-address ADDRESS
  --gateway-ws-url URL
  --asset-base-url URL
  --build
  --full-assets
  --remove-volumes
  --open
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --web-port)
      web_port="$2"
      shift 2
      ;;
    --gateway-web-port)
      gateway_web_port="$2"
      shift 2
      ;;
    --gateway-tcp-port)
      gateway_tcp_port="$2"
      shift 2
      ;;
    --bind-address)
      bind_address="$2"
      shift 2
      ;;
    --gateway-ws-url)
      gateway_ws_url="$2"
      shift 2
      ;;
    --asset-base-url)
      asset_base_url="${2%/}"
      shift 2
      ;;
    --build)
      build=1
      shift
      ;;
    --full-assets)
      full_assets=1
      shift
      ;;
    --remove-volumes)
      remove_volumes=1
      shift
      ;;
    --open)
      open_browser=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "${command}" in
  doctor|auth|build|up|down|logs|shell|verify|assets|status) ;;
  *)
    echo "Unknown command: ${command}" >&2
    usage >&2
    exit 2
    ;;
esac

require_command() {
  local name="$1"
  local hint="$2"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command '${name}'. ${hint}" >&2
    exit 1
  fi
}

compose() {
  docker compose -f "${compose_file}" "$@"
}

http_ok() {
  curl --fail --silent --show-error --max-time 3 "$1" >/dev/null 2>&1
}

wait_for_http() {
  local name="$1"
  local url="$2"
  local attempts="${3:-300}"
  local attempt
  for ((attempt = 1; attempt <= attempts; attempt += 1)); do
    if http_ok "${url}"; then
      echo "[ready] ${name} ${url}"
      return 0
    fi
    sleep 2
  done

  echo "[error] ${name} did not become ready. Recent logs:" >&2
  compose logs --tail 120 gateway web >&2 || true
  return 1
}

release_lock_check() {
  require_command git "Install Git, then clone with --recurse-submodules."

  local expected_crystal
  local actual_crystal
  local submodule_state
  expected_crystal="$(git -C "${repository_root}" ls-tree HEAD Crystal | awk '{print $3}')"
  submodule_state="$(git -C "${repository_root}" submodule status --recursive || true)"
  if [[ -z "${submodule_state}" || "${submodule_state:0:1}" == "-" ]]; then
    echo "[dev] initialize Crystal submodule"
    git -C "${repository_root}" submodule update --init --recursive
  fi
  actual_crystal="$(git -C "${repository_root}/Crystal" rev-parse HEAD)"

  if [[ -z "${expected_crystal}" || "${expected_crystal}" != "${actual_crystal}" ]]; then
    echo "Crystal submodule mismatch. Run: git submodule update --init --recursive" >&2
    exit 1
  fi

  local lock_crystal
  local lock_tag
  local lock_hash
  local manifest_tag
  local manifest_hash
  lock_crystal="$(sed -n 's/^[[:space:]]*"commit":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
  lock_tag="$(sed -n 's/^[[:space:]]*"releaseTag":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
  lock_hash="$(sed -n 's/^[[:space:]]*"contentHash":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
  manifest_tag="$(sed -n 's/^[[:space:]]*"releaseTag":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-assets.json" | head -n 1)"
  manifest_hash="$(sed -n 's/^[[:space:]]*"contentHash":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-assets.json" | head -n 1)"

  if [[ "${lock_crystal}" != "${expected_crystal}" ]]; then
    echo "Crystal gitlink and developer release lock differ." >&2
    exit 1
  fi
  if [[ "${lock_tag}" != "${manifest_tag}" || "${lock_hash}" != "${manifest_hash}" ]]; then
    echo "Asset manifest and developer release lock differ." >&2
    exit 1
  fi

  echo "[ok] Crystal ${expected_crystal}"
  echo "[ok] Assets ${manifest_tag} / ${manifest_hash}"
}

open_url() {
  local url="$1"
  case "$(uname -s)" in
    Darwin) open "${url}" ;;
    Linux)
      if command -v xdg-open >/dev/null 2>&1; then
        xdg-open "${url}" >/dev/null 2>&1 &
      fi
      ;;
  esac
}

install_full_assets() {
  local release_tag
  local run_args=(run --rm --no-deps)
  if [[ -z "${MIR2_DEV_IMAGE:-}" || "${MIR2_DEV_IMAGE}" != *@sha256:* ]]; then
    echo "Full assets require the published digest-pinned developer image." >&2
    echo "Wait for the Developer Image workflow and update config/developer-release.json." >&2
    exit 1
  fi

  docker volume create mir2-developer-gh-config >/dev/null
  if [[ -n "${GH_TOKEN:-}" ]]; then
    run_args+=(-e GH_TOKEN)
  fi

  if ! compose "${run_args[@]}" asset-auth gh auth status >/dev/null 2>&1; then
    echo "[assets] Authorize access to the pinned private release."
    compose run --rm --no-deps asset-auth gh auth login --web --git-protocol https
  fi
  compose "${run_args[@]}" asset-fetch

  release_tag="$(
    sed -n 's/^[[:space:]]*"releaseTag":[[:space:]]*"\([^"]*\)".*/\1/p' \
      "${project_root}/config/developer-assets.json" | head -n 1
  )"
  [ -n "${release_tag}" ] || {
    echo "Developer asset release tag is missing." >&2
    exit 1
  }
  compose run --rm --no-deps workspace \
    bash scripts/install-developer-assets.sh \
    --parts-directory ".mir2-data/developer-assets/${release_tag}" \
    --cache-directory ".mir2-data/developer-assets/${release_tag}"
}

require_command docker "Install Docker Desktop (macOS/Windows) or Docker Engine with Compose."
if ! docker_server_version="$(docker info --format '{{.ServerVersion}}' 2>&1)"; then
  echo "Docker engine is not ready. Start Docker Desktop and wait for the Linux engine." >&2
  printf '%s\n' "${docker_server_version}" >&2
  exit 1
fi
if [[ -z "${docker_server_version}" ]]; then
  echo "Docker engine returned an empty server version. Start Docker Desktop and wait for the Linux engine." >&2
  exit 1
fi
echo "[ok] Docker engine ${docker_server_version}"
docker compose version >/dev/null
docker volume create mir2-developer-gh-config >/dev/null

export MIR2_WEB_PORT="${web_port}"
export MIR2_GATEWAY_WEB_PORT="${gateway_web_port}"
export MIR2_GATEWAY_TCP_PORT="${gateway_tcp_port}"
export MIR2_BIND_ADDRESS="${bind_address}"
export MIR2_GATEWAY_WS_URL="${gateway_ws_url:-ws://127.0.0.1:${gateway_web_port}/ws}"
export MIR2_ASSET_BASE_URL="${asset_base_url}"
if [[ -z "${MIR2_DEV_IMAGE:-}" ]]; then
  published_image="$(sed -n 's/^[[:space:]]*"publishedImage":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
  published_digest="$(sed -n 's/^[[:space:]]*"publishedDigest":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
  if [[ -n "${published_digest}" ]]; then
    export MIR2_DEV_IMAGE="${published_image}@${published_digest}"
  fi
fi

web_url="http://127.0.0.1:${web_port}/"
gateway_health_url="http://127.0.0.1:${gateway_web_port}/health"

case "${command}" in
  doctor)
    release_lock_check
    compose config --quiet
    echo "[ok] Developer environment definition is valid."
    ;;
  auth)
    release_lock_check
    if [[ -z "${MIR2_DEV_IMAGE:-}" || "${MIR2_DEV_IMAGE}" != *@sha256:* ]]; then
      echo "Asset authorization requires the published digest-pinned developer image." >&2
      exit 1
    fi
    docker volume create mir2-developer-gh-config >/dev/null
    compose run --rm --no-deps asset-auth \
      gh auth login --web --git-protocol https
    ;;
  build)
    export MIR2_DEV_IMAGE="mir2-web3-developer:local"
    release_lock_check
    compose build workspace
    ;;
  up)
    release_lock_check
    if [[ "${full_assets}" -eq 1 ]]; then
      install_full_assets
    fi
    if [[ "${build}" -eq 1 ]]; then
      export MIR2_DEV_IMAGE="mir2-web3-developer:local"
    fi
    up_args=(up -d)
    if [[ "${build}" -eq 1 ]]; then
      up_args+=(--build)
    fi
    up_args+=(gateway web)
    compose "${up_args[@]}"
    wait_for_http "Gateway" "${gateway_health_url}"
    wait_for_http "Player Web" "${web_url}"
    printf '\nMir2 is ready: %s\n' "${web_url}"
    echo "Stop it with: ./scripts/dev.sh down"
    if [[ "${open_browser}" -eq 1 ]]; then
      open_url "${web_url}"
    fi
    ;;
  down)
    down_args=(down --remove-orphans)
    if [[ "${remove_volumes}" -eq 1 ]]; then
      down_args+=(--volumes)
    fi
    compose "${down_args[@]}"
    ;;
  logs)
    compose logs -f --tail 200 gateway web
    ;;
  shell)
    compose run --rm --no-deps workspace bash
    ;;
  verify)
    release_lock_check
    compose run --rm --no-deps workspace bash -lc \
      'npm ci --prefix apps/web && npm --prefix apps/web run typecheck && cargo +1.89.0 check --locked -p mir2-gateway'
    ;;
  assets)
    release_lock_check
    install_full_assets
    ;;
  status)
    compose ps
    if http_ok "${gateway_health_url}"; then
      echo "Gateway health: ready"
    else
      echo "Gateway health: not ready"
    fi
    if http_ok "${web_url}"; then
      echo "Player Web:     ready"
    else
      echo "Player Web:     not ready"
    fi
    ;;
esac
