#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"
INSTALL_ROOT="${HOME}/Library/Application Support/Obelisk/DubheHomeAgent"
BIN_DIR="${INSTALL_ROOT}/bin"
PLIST="${HOME}/Library/LaunchAgents/com.obelisk-labs.dubhe-home-agent.plist"

install -d -m 700 "${BIN_DIR}" "$(dirname "${PLIST}")"
install -m 500 "${SOURCE_DIR}/home_agent" "${BIN_DIR}/home_agent"
install -m 500 "${SOURCE_DIR}/home_agent_launcher" "${BIN_DIR}/home_agent_launcher"
install -m 500 "${SOURCE_DIR}/home_agent_supervisor" "${BIN_DIR}/home_agent_supervisor"
install -m 500 "${SOURCE_DIR}/zone_host" "${BIN_DIR}/zone_host"
"${BIN_DIR}/home_agent_supervisor" key-init

sed \
  -e "s|__INSTALL_ROOT__|${INSTALL_ROOT}|g" \
  "${SOURCE_DIR}/com.obelisk-labs.dubhe-home-agent.plist.in" >"${PLIST}"
chmod 600 "${PLIST}"

echo "Binaries and the node identity are installed."
echo "Complete signed enrollment/configuration before running:"
echo "  launchctl bootstrap gui/$(id -u) '${PLIST}'"
