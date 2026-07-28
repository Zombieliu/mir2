#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repository_root="$(cd "${project_root}/.." && pwd)"
repository="${MIR2_CLEAN_REPOSITORY:-${repository_root}}"
revision="${MIR2_CLEAN_REVISION:-HEAD}"
destination=""
destination_generated=0
temp_root="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
full_assets=0
keep=0
web_port="${MIR2_CLEAN_WEB_PORT:-13002}"
gateway_web_port="${MIR2_CLEAN_GATEWAY_WEB_PORT:-17110}"
gateway_tcp_port="${MIR2_CLEAN_GATEWAY_TCP_PORT:-17000}"
compose_project_name="${MIR2_CLEAN_COMPOSE_PROJECT_NAME:-mir2-clean-room-${web_port}-$$}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repository)
      repository="$2"
      shift 2
      ;;
    --revision)
      revision="$2"
      shift 2
      ;;
    --destination)
      destination="$2"
      shift 2
      ;;
    --full-assets)
      full_assets=1
      shift
      ;;
    --keep)
      keep=1
      shift
      ;;
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
    -h|--help)
      echo "Usage: ./scripts/accept-clean-room.sh [--repository URL] [--revision REF] [--full-assets] [--keep]"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${destination}" ]]; then
  destination="$(mktemp -d "${temp_root}/mir2-clean-room.XXXXXXXX")"
  rmdir "${destination}"
  destination_generated=1
fi
if [[ -e "${destination}" ]]; then
  echo "Clean-room destination already exists: ${destination}" >&2
  exit 1
fi

clone_root="${destination}/mir2"
clone_project="${clone_root}/mir2-web3"
export MIR2_COMPOSE_PROJECT_NAME="${compose_project_name}"

cleanup() {
  if [[ -d "${clone_project}" ]]; then
    "${clone_project}/scripts/dev.sh" down \
      --remove-volumes \
      --web-port "${web_port}" \
      --gateway-web-port "${gateway_web_port}" \
      --gateway-tcp-port "${gateway_tcp_port}" || true
  fi
  if [[ "${keep}" -eq 0 && "${destination_generated}" -eq 1 && -d "${destination}" ]]; then
    case "${destination}" in
      "${temp_root}"/mir2-clean-room.*) rm -rf -- "${destination}" ;;
      *) echo "Refusing to remove non-temporary clean-room path: ${destination}" >&2 ;;
    esac
  fi
}
trap cleanup EXIT

mkdir -p "${destination}"
echo "[clean-room] clone repository into an empty directory"
git clone --no-local --recurse-submodules "${repository}" "${clone_root}"
if [[ "${revision}" != "HEAD" ]]; then
  git -C "${clone_root}" checkout --detach "${revision}"
  git -C "${clone_root}" submodule update --init --recursive
fi

up_args=(
  up
  --build
  --web-port "${web_port}"
  --gateway-web-port "${gateway_web_port}"
  --gateway-tcp-port "${gateway_tcp_port}"
)
if [[ "${full_assets}" -eq 1 ]]; then
  up_args+=(--full-assets)
fi
"${clone_project}/scripts/dev.sh" "${up_args[@]}"

curl --fail --silent --show-error "http://127.0.0.1:${gateway_web_port}/health" >/dev/null
curl --fail --silent --show-error "http://127.0.0.1:${web_port}/" >/dev/null

echo
echo "Clean-room acceptance passed."
echo "Repository: ${repository}"
echo "Revision:   $(git -C "${clone_root}" rev-parse HEAD)"
echo "Player Web: http://127.0.0.1:${web_port}/"
