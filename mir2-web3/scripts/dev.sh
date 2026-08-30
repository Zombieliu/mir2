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
runtime_image_prepared=0
bevy_runtime_prepared=0
asset_image_prepared=0
developer_revision=""
local_developer_image=""
published_image=""
published_digest=""
published_revision=""
published_reference=""
requested_developer_image="${MIR2_DEV_IMAGE:-}"

usage() {
  cat <<'EOF'
Usage: ./scripts/dev.sh [command] [options]

Commands:
  doctor  Validate Docker, version locks, submodule, and Compose
  auth    Authorize GitHub and pull the immutable private developer image
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
  local service="${4:-}"
  local attempt
  for ((attempt = 1; attempt <= attempts; attempt += 1)); do
    if http_ok "${url}"; then
      echo "[ready] ${name} ${url}"
      return 0
    fi
    if [[ -n "${service}" ]] &&
       compose ps --status exited --services | grep -Fxq "${service}"; then
      echo "[error] ${name} exited before becoming ready. Recent logs:" >&2
      compose logs --tail 120 "${service}" >&2 || true
      return 1
    fi
    if [[ -n "${service}" ]] &&
       compose ps --status unhealthy --services | grep -Fxq "${service}"; then
      echo "[error] ${name} became unhealthy before becoming ready. Recent logs:" >&2
      compose logs --tail 120 "${service}" >&2 || true
      return 1
    fi
    sleep 2
  done

  echo "[error] ${name} did not become ready. Recent logs:" >&2
  compose logs --tail 120 gateway web >&2 || true
  return 1
}

select_local_developer_image() {
  export MIR2_DEV_IMAGE="${local_developer_image}"
}

prepare_runtime_image() {
  if [[ "${runtime_image_prepared}" -eq 1 ]]; then
    return
  fi

  if [[ -n "${published_reference}" && "${MIR2_DEV_IMAGE}" == "${published_reference}" ]]; then
    if docker image inspect "${MIR2_DEV_IMAGE}" >/dev/null 2>&1 ||
       docker pull "${MIR2_DEV_IMAGE}" >/dev/null 2>&1; then
      runtime_image_prepared=1
      return
    fi
    echo "[dev] Published image is unavailable; falling back to the locked local build."
    select_local_developer_image
  fi

  if [[ "${MIR2_DEV_IMAGE}" == "${local_developer_image}" ]]; then
    local actual_revision=""
    if [[ "${build}" -eq 0 ]]; then
      actual_revision="$(
        docker image inspect \
          --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' \
          "${MIR2_DEV_IMAGE}" 2>/dev/null || true
      )"
    fi
    if [[ "${actual_revision}" != "${developer_revision}" ]]; then
      echo "[dev] Build the locked local developer image for ${developer_revision}."
      compose build workspace
    fi
  elif ! docker image inspect "${MIR2_DEV_IMAGE}" >/dev/null 2>&1 &&
       ! docker pull "${MIR2_DEV_IMAGE}" >/dev/null 2>&1; then
    if [[ "${MIR2_DEV_IMAGE}" == *@sha256:* ]]; then
      echo "Unable to pull the explicitly selected developer image: ${MIR2_DEV_IMAGE}" >&2
      exit 1
    fi
    compose build workspace
  fi

  runtime_image_prepared=1
}

prepare_bevy_runtime() {
  if [[ "${bevy_runtime_prepared}" -eq 1 ]]; then
    return
  fi

  prepare_runtime_image
  if ! compose run --rm --no-deps \
      --user "$(id -u):$(id -g)" \
      --entrypoint node \
      workspace \
      apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs; then
    echo "[dev] Pinned Bevy runtime is unavailable; rebuilding it from current source."
    compose run --rm --no-deps \
      --user "$(id -u):$(id -g)" \
      --entrypoint bash \
      workspace -lc \
      'CARGO_HOME=/tmp/mir2-runtime-cargo MIR2_BEVY_CARGO_TARGET_ROOT=/tmp/mir2-runtime-target RUSTUP_TOOLCHAIN="${MIR2_BEVY_RUNTIME_RUST_TOOLCHAIN:?missing runtime toolchain lock}" MIR2_USE_PREBUILT_BEVY_RUNTIME=0 node apps/web/scripts/build-bevy-runtime.mjs release'
  fi
  bevy_runtime_prepared=1
}

verify_published_image_witness() {
  local witness_tag
  local reference_record
  local tag_type
  local tag_object
  local witness_record
  local witness_target
  local witness_message

  witness_tag="developer-image-${published_revision}"
  reference_record="$(
    gh api \
      "repos/Zombieliu/mir2/git/ref/tags/${witness_tag}" \
      --jq '.object.type, .object.sha'
  )"
  tag_type="$(printf '%s\n' "${reference_record}" | sed -n '1p')"
  tag_object="$(printf '%s\n' "${reference_record}" | sed -n '2p')"
  if [[ "${tag_type}" != "tag" || ! "${tag_object}" =~ ^[a-f0-9]{40}$ ]]; then
    echo "Published developer image witness is missing or is not annotated: ${witness_tag}" >&2
    exit 1
  fi

  witness_record="$(
    gh api \
      "repos/Zombieliu/mir2/git/tags/${tag_object}" \
      --jq '.object.sha, .message'
  )"
  witness_target="$(printf '%s\n' "${witness_record}" | sed -n '1p')"
  witness_message="$(printf '%s\n' "${witness_record}" | sed -n '2p')"
  if [[ "${witness_target}" != "${published_revision}" ||
        "${witness_message}" != "${published_reference}" ]]; then
    echo "Published developer image witness does not match the release lock." >&2
    exit 1
  fi
}

prepare_asset_image() {
  if [[ "${asset_image_prepared}" -eq 1 ]]; then
    return
  fi

  if [[ -z "${published_reference}" || -z "${published_revision}" ]]; then
    echo "Full assets require a published image digest and revision in config/developer-release.json." >&2
    exit 1
  fi
  if [[ "${published_image}" != "ghcr.io/zombieliu/mir2-developer" ||
        ! "${published_digest}" =~ ^sha256:[a-f0-9]{64}$ ||
        ! "${published_revision}" =~ ^[a-f0-9]{40}$ ]]; then
    echo "Full assets require the trusted published image, digest, and revision lock." >&2
    exit 1
  fi

  if [[ -n "${requested_developer_image}" &&
        "${requested_developer_image}" != "${published_reference}" ]]; then
    echo "Full asset authorization refuses a custom developer image." >&2
    echo "Expected exactly: ${published_reference}" >&2
    exit 1
  fi

  require_command gh "Install GitHub CLI, then run 'gh auth login'."
  if ! gh auth status --hostname github.com >/dev/null 2>&1; then
    echo "[assets] Authorize the private repository and package."
    gh auth login \
      --hostname github.com \
      --web \
      --git-protocol https \
      --scopes "repo,read:packages"
  fi
  verify_published_image_witness

  local github_login
  local docker_config
  github_login="${MIR2_GITHUB_LOGIN:-}"
  if [[ -z "${github_login}" ]]; then
    github_login="$(gh api user --jq .login)"
  fi
  if [[ -z "${github_login}" ]]; then
    echo "GitHub CLI did not return the authenticated login." >&2
    exit 1
  fi

  docker_config="$(mktemp -d "${TMPDIR:-/tmp}/mir2-docker-auth.XXXXXXXX")"
  if ! (
    export DOCKER_CONFIG="${docker_config}"
    gh auth token --hostname github.com |
      docker login ghcr.io --username "${github_login}" --password-stdin >/dev/null
    docker pull "${published_reference}" >/dev/null
  ); then
    rm -rf -- "${docker_config}"
    echo "Unable to authenticate and pull the immutable developer image." >&2
    exit 1
  fi
  rm -rf -- "${docker_config}"

  export MIR2_DEV_IMAGE="${published_reference}"
  local actual_revision
  actual_revision="$(
    docker image inspect \
      --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' \
      "${MIR2_DEV_IMAGE}"
  )"
  if [[ "${actual_revision}" != "${published_revision}" ]]; then
    echo "Published developer image revision mismatch: expected ${published_revision}, got ${actual_revision:-<empty>}." >&2
    exit 1
  fi

  asset_image_prepared=1
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
  local gh_token
  prepare_asset_image

  gh_token="$(gh auth token --hostname github.com)"
  printf '%s\n' "${gh_token}" |
    compose run --rm --no-deps -T asset-fetch
  unset gh_token

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
require_command git "Install Git, then clone with --recurse-submodules."
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

export MIR2_WEB_PORT="${web_port}"
export MIR2_GATEWAY_WEB_PORT="${gateway_web_port}"
export MIR2_GATEWAY_TCP_PORT="${gateway_tcp_port}"
export MIR2_BIND_ADDRESS="${bind_address}"
export MIR2_GATEWAY_WS_URL="${gateway_ws_url:-ws://127.0.0.1:${gateway_web_port}/ws}"
export MIR2_ASSET_BASE_URL="${asset_base_url}"
developer_revision="$(git -C "${repository_root}" rev-parse HEAD)"
if [[ ! "${developer_revision}" =~ ^[a-f0-9]{40}$ ]]; then
  echo "Unable to resolve a full Git revision for the developer image." >&2
  exit 1
fi
export MIR2_DEVELOPER_IMAGE_REVISION="${developer_revision}"
local_developer_image="mir2-web3-developer:local-${developer_revision:0:12}"
published_image="$(sed -n 's/^[[:space:]]*"publishedImage":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
published_digest="$(sed -n 's/^[[:space:]]*"publishedDigest":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
published_revision="$(sed -n 's/^[[:space:]]*"publishedRevision":[[:space:]]*"\([^"]*\)".*/\1/p' "${project_root}/config/developer-release.json" | head -n 1)"
if [[ -n "${published_digest}" ]]; then
  published_reference="${published_image}@${published_digest}"
fi
if [[ -z "${MIR2_DEV_IMAGE:-}" ]]; then
  if [[ -n "${published_reference}" ]]; then
    export MIR2_DEV_IMAGE="${published_reference}"
  else
    select_local_developer_image
  fi
fi
if [[ "${build}" -eq 1 ]]; then
  select_local_developer_image
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
    prepare_asset_image
    echo "[ok] GitHub and GHCR authorization are ready for ${published_reference}."
    ;;
  build)
    select_local_developer_image
    release_lock_check
    compose build workspace
    ;;
  up)
    bash "${project_root}/scripts/Initialize-LocalSaveRecovery.sh" --project-root "${project_root}" --quiet
    release_lock_check
    if [[ "${full_assets}" -eq 1 ]]; then
      install_full_assets
      if [[ "${build}" -eq 1 ]]; then
        select_local_developer_image
        runtime_image_prepared=0
      fi
    fi
    prepare_bevy_runtime
    up_args=(up -d)
    up_args+=(gateway web)
    compose "${up_args[@]}"
    wait_for_http "Gateway" "${gateway_health_url}" 300 gateway
    wait_for_http "Player Web" "${web_url}" 300 web
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
    prepare_runtime_image
    compose run --rm --no-deps workspace bash
    ;;
  verify)
    release_lock_check
    prepare_bevy_runtime
    compose run --rm --no-deps workspace bash -lc \
      'node apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs && node scripts/check-developer-release.mjs && cargo +1.89.0 fmt --all -- --check && npm ci --prefix apps/web && npm ci --prefix apps/admin-web && npm --prefix apps/web run typecheck && npm --prefix apps/admin-web run typecheck && cargo +1.89.0 check --locked -p mir2-gateway -p mir2-admin-api'
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
