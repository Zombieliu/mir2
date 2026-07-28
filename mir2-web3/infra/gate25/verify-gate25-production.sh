#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 5 ]]; then
  echo "usage: $0 <trusted-operator-public-key> <output.json> <physical-run-1.json> <physical-run-2.json> <physical-run-3.json> [physical-run-N.json]" >&2
  exit 64
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
TRUSTED_OPERATOR_PUBLIC_KEY="$1"
OUTPUT_PATH="$2"
shift 2

for evidence in "$@"; do
  if [[ ! -f "${evidence}" ]]; then
    echo "missing signed physical evidence: ${evidence}" >&2
    exit 66
  fi
done

cd "${REPO_DIR}"
cargo +1.89.0 run -q -p mir2-gateway --bin home_beta_policy -- \
  verify-cohort \
  "${TRUSTED_OPERATOR_PUBLIC_KEY}" \
  "${OUTPUT_PATH}" \
  "$@"

jq -e '
  .schema == "obelisk.home-network-beta-cohort.v1" and
  .accepted == true and
  .physicalRunCount >= 3 and
  .distinctNodeCount >= 3 and
  .distinctProviderCount >= 3 and
  .distinctAsnCount >= 3 and
  .distinctFailureDomainCount >= 3 and
  .maximumObservedRtoMs < 5000 and
  .economyDuplicateCount == 0
' "${OUTPUT_PATH}" >/dev/null

echo "GATE25_PRODUCTION_ACCEPTED output=${OUTPUT_PATH}"
