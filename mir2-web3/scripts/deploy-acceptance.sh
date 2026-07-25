#!/usr/bin/env bash
set -Eeuo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_developer="${project_root}/infra/compose.developer.yml"
compose_acceptance="${project_root}/infra/compose.acceptance.yml"

domain="${MIR2_ACCEPTANCE_DOMAIN:-}"
asset_base_url="${MIR2_ASSET_BASE_URL:-}"
basic_auth_user="${MIR2_ACCEPTANCE_BASIC_AUTH_USER:-}"
basic_auth_hash="${MIR2_ACCEPTANCE_BASIC_AUTH_HASH:-}"
basic_auth_password="${MIR2_ACCEPTANCE_BASIC_AUTH_PASSWORD:-}"
unset MIR2_ACCEPTANCE_BASIC_AUTH_HASH
unset MIR2_ACCEPTANCE_BASIC_AUTH_PASSWORD
build=0

require_option_value() {
  local option="$1"
  local value="${2:-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "${option} requires a value." >&2
    exit 2
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --domain)
      require_option_value "$1" "${2:-}"
      domain="$2"
      shift 2
      ;;
    --asset-base-url)
      require_option_value "$1" "${2:-}"
      asset_base_url="${2%/}"
      shift 2
      ;;
    --basic-auth-user)
      require_option_value "$1" "${2:-}"
      basic_auth_user="$2"
      shift 2
      ;;
    --basic-auth-hash)
      require_option_value "$1" "${2:-}"
      basic_auth_hash="$2"
      shift 2
      ;;
    --build)
      build=1
      shift
      ;;
    -h|--help)
      cat <<'EOF'
Usage: ./scripts/deploy-acceptance.sh --domain play.example.com [options]

Options:
  --asset-base-url URL       Immutable authorized R2/object-storage release
  --basic-auth-user USER     Required acceptance Basic Auth username
  --basic-auth-hash HASH     Required pre-generated Caddy bcrypt/argon2id hash
  --build                    Rebuild the pinned developer image

The three Basic Auth values can instead be supplied through
MIR2_ACCEPTANCE_BASIC_AUTH_USER, MIR2_ACCEPTANCE_BASIC_AUTH_HASH, and
MIR2_ACCEPTANCE_BASIC_AUTH_PASSWORD. The plaintext password is accepted only
through the server secret environment, never as a command-line option.

Generate a hash with the pinned Caddy image before deployment:
  docker run --rm -it \
    caddy:2.10.0-alpine@sha256:ae4458638da8e1a91aafffb231c5f8778e964bca650c8a8cb23a7e8ac557aa3c \
    caddy hash-password --algorithm argon2id

The DNS record must already point to this server, and ports 80/443 must be open.
EOF
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${domain}" ]]; then
  echo "MIR2_ACCEPTANCE_DOMAIN or --domain is required." >&2
  exit 1
fi
if [[ "${domain}" == http://* || "${domain}" == https://* || "${domain}" == */* ||
      ! "${domain}" =~ ^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ||
      "${domain}" != *.* || "${domain}" == *..* ]]; then
  echo "Use a DNS host name without scheme or path: ${domain}" >&2
  exit 1
fi
if [[ -z "${basic_auth_user}" ]]; then
  echo "MIR2_ACCEPTANCE_BASIC_AUTH_USER or --basic-auth-user is required." >&2
  exit 1
fi
if [[ ! "${basic_auth_user}" =~ ^[A-Za-z0-9._@-]+$ ]]; then
  echo "Basic Auth username may contain only letters, numbers, '.', '_', '@', and '-'." >&2
  exit 1
fi
if [[ -z "${basic_auth_hash}" ]]; then
  echo "MIR2_ACCEPTANCE_BASIC_AUTH_HASH or --basic-auth-hash is required." >&2
  exit 1
fi
if [[ -z "${basic_auth_password}" ]]; then
  echo "MIR2_ACCEPTANCE_BASIC_AUTH_PASSWORD is required for health checks." >&2
  exit 1
fi
if [[ "${basic_auth_password}" == *$'\n'* || "${basic_auth_password}" == *$'\r'* ]]; then
  echo "Basic Auth password must not contain a newline." >&2
  exit 1
fi

basic_auth_algorithm=""
case "${basic_auth_hash}" in
  '$argon2id$'*)
    if [[ ! "${basic_auth_hash}" =~ ^\$argon2id\$v=19\$m=([0-9]+),t=([0-9]+),p=([0-9]+)\$[A-Za-z0-9+/]+={0,2}\$[A-Za-z0-9+/]+={0,2}$ ]]; then
      echo "The argon2id hash is malformed. Generate it with caddy hash-password." >&2
      exit 1
    fi
    argon_memory_kib="${BASH_REMATCH[1]}"
    argon_iterations="${BASH_REMATCH[2]}"
    argon_parallelism="${BASH_REMATCH[3]}"
    if (( argon_memory_kib < 47104 || argon_iterations < 1 || argon_parallelism < 1 )); then
      echo "The argon2id hash is weaker than the pinned Caddy defaults." >&2
      exit 1
    fi
    basic_auth_algorithm="argon2id"
    ;;
  '$2a$'*|'$2b$'*|'$2y$'*)
    if [[ ! "${basic_auth_hash}" =~ ^\$2[aby]\$[0-9]{2}\$[./A-Za-z0-9]{53}$ ]]; then
      echo "The bcrypt hash is malformed. Generate it with caddy hash-password." >&2
      exit 1
    fi
    bcrypt_cost="${basic_auth_hash:4:2}"
    if (( 10#${bcrypt_cost} < 12 || 10#${bcrypt_cost} > 31 )); then
      echo "The bcrypt cost must be between 12 and 31." >&2
      exit 1
    fi
    basic_auth_algorithm="bcrypt"
    ;;
  *)
    echo "Unsupported Basic Auth hash. Use Caddy argon2id or bcrypt." >&2
    exit 1
    ;;
esac

for required_command in docker git curl base64 jq; do
  if ! command -v "${required_command}" >/dev/null 2>&1; then
    echo "Required command is not installed: ${required_command}" >&2
    exit 1
  fi
done

compose_version="$(docker compose version --short)"
compose_version="${compose_version#v}"
IFS=. read -r compose_major compose_minor compose_patch <<<"${compose_version%%-*}"
compose_patch="${compose_patch:-0}"
if [[ ! "${compose_major}" =~ ^[0-9]+$ ||
      ! "${compose_minor}" =~ ^[0-9]+$ ||
      ! "${compose_patch}" =~ ^[0-9]+$ ]] ||
   (( compose_major < 2 ||
      (compose_major == 2 && compose_minor < 24) ||
      (compose_major == 2 && compose_minor == 24 && compose_patch < 4) )); then
  echo "Docker Compose 2.24.4 or newer is required; found ${compose_version}." >&2
  exit 1
fi

dirty_status="$(git -C "${project_root}" status --porcelain=v1 --untracked-files=all)"
if [[ -n "${dirty_status}" ]]; then
  echo "The repository has tracked or untracked changes. Commit or clean it before deploying:" >&2
  printf '%s\n' "${dirty_status}" >&2
  exit 1
fi

revision="$(git -C "${project_root}" rev-parse --verify HEAD)"
basic_auth_hash_b64="$(printf '%s' "${basic_auth_hash}" | base64 | tr -d '\r\n')"

export MIR2_ACCEPTANCE_DOMAIN="${domain}"
export MIR2_ACCEPTANCE_BASIC_AUTH_USER="${basic_auth_user}"
export MIR2_ACCEPTANCE_BASIC_AUTH_HASH_B64="${basic_auth_hash_b64}"
export MIR2_ACCEPTANCE_BASIC_AUTH_ALGORITHM="${basic_auth_algorithm}"
export MIR2_GATEWAY_WS_URL="wss://${domain}/ws"
export MIR2_ASSET_BASE_URL="${asset_base_url}"
export MIR2_BIND_ADDRESS="127.0.0.1"
export MIR2_DEPLOY_REVISION="${revision}"
export MIR2_COMPOSE_PROJECT_NAME="${MIR2_COMPOSE_PROJECT_NAME:-mir2-web3-acceptance}"

compose_args=(
  compose
  -f "${compose_developer}"
  -f "${compose_acceptance}"
)
curl_local_args=(
  --resolve "${domain}:443:127.0.0.1"
  --connect-timeout 5
)
curl_secret_config="$(mktemp)"
chmod 0600 "${curl_secret_config}"
escaped_basic_auth_password="${basic_auth_password//\\/\\\\}"
escaped_basic_auth_password="${escaped_basic_auth_password//\"/\\\"}"
printf 'user = "%s:%s"\n' \
  "${basic_auth_user}" "${escaped_basic_auth_password}" > "${curl_secret_config}"
unset escaped_basic_auth_password
unset basic_auth_password

cleanup_secrets() {
  rm -f -- "${curl_secret_config}"
}
trap cleanup_secrets EXIT

dump_logs() {
  echo "----- acceptance service logs -----" >&2
  docker "${compose_args[@]}" logs --tail 200 gateway web caddy >&2 || true
  echo "----- end acceptance service logs -----" >&2
}

deployment_started=0
on_error() {
  local exit_code=$?
  trap - ERR
  if [[ "${deployment_started}" -eq 1 ]]; then
    dump_logs
  fi
  echo "Acceptance deployment failed (exit ${exit_code})." >&2
  exit "${exit_code}"
}
trap on_error ERR

up_args=(up --detach --force-recreate --remove-orphans)
if [[ "${build}" -eq 1 ]]; then
  up_args+=(--build)
fi
up_args+=(gateway web caddy)

docker volume create mir2-developer-gh-config >/dev/null
docker "${compose_args[@]}" config --quiet
deployment_started=1
docker "${compose_args[@]}" "${up_args[@]}"

ready=0
for _ in $(seq 1 180); do
  if curl --fail --silent --show-error --max-time 5 \
       "${curl_local_args[@]}" \
       --config "${curl_secret_config}" \
       "https://${domain}/health" >/dev/null 2>&1 &&
     curl --fail --silent --show-error --max-time 10 \
       "${curl_local_args[@]}" \
       --config "${curl_secret_config}" \
       "https://${domain}/" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 2
done

if [[ "${ready}" -ne 1 ]]; then
  echo "Shared acceptance environment did not become ready within 6 minutes." >&2
  false
fi

gateway_health_json="$(
  curl --fail --silent --show-error --max-time 10 \
    "${curl_local_args[@]}" \
    --config "${curl_secret_config}" \
    "https://${domain}/health"
)"
web_version_json="$(
  curl --fail --silent --show-error --max-time 10 \
    "${curl_local_args[@]}" \
    --config "${curl_secret_config}" \
    "https://${domain}/version"
)"
gateway_runtime_revision="$(printf '%s' "${gateway_health_json}" | jq -er '.revision')"
web_runtime_revision="$(printf '%s' "${web_version_json}" | jq -er '.revision')"
if [[ "${gateway_runtime_revision}" != "${revision}" ]]; then
  echo "Gateway runtime revision mismatch: expected=${revision} actual=${gateway_runtime_revision}" >&2
  false
fi
if [[ "${web_runtime_revision}" != "${revision}" ]]; then
  echo "Player Web runtime revision mismatch: expected=${revision} actual=${web_runtime_revision}" >&2
  false
fi

for protected_path in "/" "/health" "/ws"; do
  unauthenticated_status="$(
    curl --silent --show-error --max-time 10 \
      "${curl_local_args[@]}" \
      --output /dev/null --write-out '%{http_code}' \
      "https://${domain}${protected_path}" || true
  )"
  if [[ "${unauthenticated_status}" != "401" ]]; then
    echo "Expected HTTP 401 without credentials for ${protected_path}, got ${unauthenticated_status:-no response}." >&2
    false
  fi
done

verify_container_revision() {
  local service="$1"
  local container_id
  local label_revision
  local env_revision=""
  local state
  local health
  local image_ref
  local image_id
  local env_entry

  container_id="$(docker "${compose_args[@]}" ps -q "${service}")"
  if [[ -z "${container_id}" ]]; then
    echo "No running container found for ${service}." >&2
    return 1
  fi

  label_revision="$(
    docker inspect --format '{{ index .Config.Labels "com.mir2.acceptance.revision" }}' \
      "${container_id}"
  )"
  while IFS= read -r env_entry; do
    case "${env_entry}" in
      MIR2_DEPLOY_REVISION=*)
        env_revision="${env_entry#MIR2_DEPLOY_REVISION=}"
        ;;
    esac
  done < <(docker inspect --format '{{range .Config.Env}}{{println .}}{{end}}' "${container_id}")

  if [[ "${label_revision}" != "${revision}" || "${env_revision}" != "${revision}" ]]; then
    echo "${service} revision mismatch: expected=${revision} label=${label_revision:-missing} env=${env_revision:-missing}" >&2
    return 1
  fi

  state="$(docker inspect --format '{{.State.Status}}' "${container_id}")"
  health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "${container_id}")"
  if [[ "${state}" != "running" ]]; then
    echo "${service} is not running (state=${state})." >&2
    return 1
  fi
  if [[ "${service}" != "caddy" && "${health}" != "healthy" ]]; then
    echo "${service} is not healthy (health=${health})." >&2
    return 1
  fi

  image_ref="$(docker inspect --format '{{.Config.Image}}' "${container_id}")"
  image_id="$(docker inspect --format '{{.Image}}' "${container_id}")"
  printf '%-7s revision=%s state=%s health=%s image=%s image_id=%s\n' \
    "${service}" "${revision}" "${state}" "${health}" "${image_ref}" "${image_id}"
}

verify_container_revision gateway
verify_container_revision web
verify_container_revision caddy

trap - ERR
echo "Shared acceptance environment is ready."
echo "Revision: ${revision}"
echo "Gateway:  ${gateway_runtime_revision}"
echo "Web:      ${web_runtime_revision}"
echo "URL:      https://${domain}/"
