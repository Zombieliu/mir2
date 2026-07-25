#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TARGET="${1:?usage: package-home-agent.sh <rust-target> <output-directory>}"
OUTPUT_DIR="${2:?usage: package-home-agent.sh <rust-target> <output-directory>}"
BIN_SUFFIX=""
if [[ "${TARGET}" == *windows* ]]; then
  BIN_SUFFIX=".exe"
fi

cargo +1.89.0 build \
  --locked \
  --release \
  --target "${TARGET}" \
  -p mir2-gateway \
  --bin home_agent \
  --bin home_agent_launcher \
  --bin home_agent_supervisor \
  --bin zone_host

PACKAGE_DIR="${OUTPUT_DIR}/dubhe-home-agent-${TARGET}"
install -d -m 755 "${PACKAGE_DIR}"
install -m 755 \
  "${REPO_ROOT}/target/${TARGET}/release/home_agent${BIN_SUFFIX}" \
  "${PACKAGE_DIR}/home_agent${BIN_SUFFIX}"
install -m 755 \
  "${REPO_ROOT}/target/${TARGET}/release/home_agent_launcher${BIN_SUFFIX}" \
  "${PACKAGE_DIR}/home_agent_launcher${BIN_SUFFIX}"
install -m 755 \
  "${REPO_ROOT}/target/${TARGET}/release/home_agent_supervisor${BIN_SUFFIX}" \
  "${PACKAGE_DIR}/home_agent_supervisor${BIN_SUFFIX}"
install -m 755 \
  "${REPO_ROOT}/target/${TARGET}/release/zone_host${BIN_SUFFIX}" \
  "${PACKAGE_DIR}/zone_host${BIN_SUFFIX}"
cp "${REPO_ROOT}/infra/gate23/install-macos.sh" "${PACKAGE_DIR}/"
cp "${REPO_ROOT}/infra/gate23/install-linux.sh" "${PACKAGE_DIR}/"
cp "${REPO_ROOT}/infra/gate23/install-windows.ps1" "${PACKAGE_DIR}/"
cp "${REPO_ROOT}/infra/gate23/com.obelisk-labs.dubhe-home-agent.plist.in" "${PACKAGE_DIR}/"
cp "${REPO_ROOT}/infra/gate23/dubhe-home-agent.service.in" "${PACKAGE_DIR}/"
cp "${REPO_ROOT}/infra/gate23/README.zh-CN.md" "${PACKAGE_DIR}/"

COPYFILE_DISABLE=1 tar -C "${OUTPUT_DIR}" \
  -czf "${PACKAGE_DIR}.tar.gz" "$(basename "${PACKAGE_DIR}")"
shasum -a 256 "${PACKAGE_DIR}.tar.gz" >"${PACKAGE_DIR}.tar.gz.sha256"
UPDATE_BUNDLE="${OUTPUT_DIR}/dubhe-home-agent-update-${TARGET}.tar.gz"
COPYFILE_DISABLE=1 tar -C "${PACKAGE_DIR}" -czf "${UPDATE_BUNDLE}" \
  "home_agent${BIN_SUFFIX}" \
  "home_agent_supervisor${BIN_SUFFIX}" \
  "zone_host${BIN_SUFFIX}"
shasum -a 256 "${UPDATE_BUNDLE}" >"${UPDATE_BUNDLE}.sha256"
echo "HOME_AGENT_PACKAGE_READY ${PACKAGE_DIR}.tar.gz"
echo "HOME_AGENT_UPDATE_BUNDLE_READY ${UPDATE_BUNDLE}"
