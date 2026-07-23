#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
compose_file="${script_dir}/docker-compose.yml"
node_key="${GATE13_NODE_SIGNING_KEY_FILE:-}"
issuer_key="${GATE13_CAPACITY_ISSUER_KEY_FILE:-}"
evidence_dir="${GATE13_EVIDENCE_DIR:-}"

if [[ -z "${node_key}" || ! -f "${node_key}" ]]; then
  echo "GATE13_NODE_SIGNING_KEY_FILE must point to the active testnet node key" >&2
  exit 1
fi
if [[ -z "${issuer_key}" || ! -f "${issuer_key}" ]]; then
  echo "GATE13_CAPACITY_ISSUER_KEY_FILE must point to a capacity issuer key" >&2
  exit 1
fi
if [[ -z "${evidence_dir}" ]]; then
  evidence_dir="$(mktemp -d "${TMPDIR:-/tmp}/obelisk-gate13.XXXXXX")"
fi
mkdir -p "${evidence_dir}"

export GATE13_NODE_SIGNING_KEY_FILE="$(cd -- "$(dirname -- "${node_key}")" && pwd)/$(basename -- "${node_key}")"
export GATE13_CAPACITY_ISSUER_KEY_FILE="$(cd -- "$(dirname -- "${issuer_key}")" && pwd)/$(basename -- "${issuer_key}")"
export GATE13_EVIDENCE_DIR="$(cd -- "${evidence_dir}" && pwd)"

cleanup() {
  docker compose -f "${compose_file}" down --volumes --remove-orphans
}
trap cleanup EXIT

docker compose -f "${compose_file}" up \
  --build \
  --abort-on-container-exit \
  --exit-code-from acceptance

test -s "${GATE13_EVIDENCE_DIR}/gate13-acceptance.json"
grep -q '"accepted": true' "${GATE13_EVIDENCE_DIR}/gate13-acceptance.json"
cat "${GATE13_EVIDENCE_DIR}/gate13-acceptance.json"
printf 'Gate 13 acceptance evidence: %s\n' "${GATE13_EVIDENCE_DIR}/gate13-acceptance.json"
