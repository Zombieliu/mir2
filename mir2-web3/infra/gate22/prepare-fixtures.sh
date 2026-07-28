#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
GENERATED_DIR="${SCRIPT_DIR}/generated"

command -v openssl >/dev/null 2>&1 || {
  echo "openssl is required for Gate 22 fixture generation" >&2
  exit 1
}

mkdir -p "${GENERATED_DIR}"
chmod -R u+w "${GENERATED_DIR}"
find "${GENERATED_DIR}" -mindepth 1 -maxdepth 1 -type f -delete
umask 077

cargo +1.89.0 run \
  --locked \
  --release \
  -p mir2-gateway \
  --bin home_tunnel_fixture \
  -- "${GENERATED_DIR}"

openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
  -out "${GENERATED_DIR}/ca-key.pem"
openssl req -x509 -new -sha256 \
  -key "${GENERATED_DIR}/ca-key.pem" \
  -subj "/CN=Obelisk Gate22 Test CA" \
  -days 2 \
  -out "${GENERATED_DIR}/ca.pem"

openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
  -out "${GENERATED_DIR}/relay-key.pem"
openssl req -new \
  -key "${GENERATED_DIR}/relay-key.pem" \
  -subj "/CN=relay.test" \
  -out "${GENERATED_DIR}/relay.csr"
openssl x509 -req -sha256 \
  -in "${GENERATED_DIR}/relay.csr" \
  -CA "${GENERATED_DIR}/ca.pem" \
  -CAkey "${GENERATED_DIR}/ca-key.pem" \
  -CAcreateserial \
  -days 2 \
  -extfile "${SCRIPT_DIR}/relay-cert.ext" \
  -out "${GENERATED_DIR}/relay.pem"

openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
  -out "${GENERATED_DIR}/agent-key.pem"
openssl req -new \
  -key "${GENERATED_DIR}/agent-key.pem" \
  -subj "/CN=agent.test" \
  -out "${GENERATED_DIR}/agent.csr"
openssl x509 -req -sha256 \
  -in "${GENERATED_DIR}/agent.csr" \
  -CA "${GENERATED_DIR}/ca.pem" \
  -CAkey "${GENERATED_DIR}/ca-key.pem" \
  -CAcreateserial \
  -days 2 \
  -extfile "${SCRIPT_DIR}/agent-cert.ext" \
  -out "${GENERATED_DIR}/agent.pem"

openssl x509 -in "${GENERATED_DIR}/ca.pem" -outform DER \
  -out "${GENERATED_DIR}/ca.der"
openssl x509 -in "${GENERATED_DIR}/relay.pem" -outform DER \
  -out "${GENERATED_DIR}/relay.der"
openssl pkcs8 -topk8 -nocrypt \
  -in "${GENERATED_DIR}/relay-key.pem" -outform DER \
  -out "${GENERATED_DIR}/relay-key.der"
openssl x509 -in "${GENERATED_DIR}/agent.pem" -outform DER \
  -out "${GENERATED_DIR}/agent.der"
openssl pkcs8 -topk8 -nocrypt \
  -in "${GENERATED_DIR}/agent-key.pem" -outform DER \
  -out "${GENERATED_DIR}/agent-key.der"

chmod 0444 \
  "${GENERATED_DIR}/ca.der" \
  "${GENERATED_DIR}/relay.der" \
  "${GENERATED_DIR}/relay-key.der" \
  "${GENERATED_DIR}/agent.der" \
  "${GENERATED_DIR}/agent-key.der" \
  "${GENERATED_DIR}/node-signing.key" \
  "${GENERATED_DIR}/relay-signing.key" \
  "${GENERATED_DIR}/capacity-certificate.json" \
  "${GENERATED_DIR}/placements.json"

echo "Gate 22 fixtures ready at ${GENERATED_DIR}"
