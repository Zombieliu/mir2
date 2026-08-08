#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"
INSTALL_ROOT="${HOME}/.local/lib/dubhe-home-agent"
BIN_DIR="${INSTALL_ROOT}/bin"
UNIT_DIR="${HOME}/.config/systemd/user"

install -d -m 700 "${BIN_DIR}" "${UNIT_DIR}"
install -m 500 "${SOURCE_DIR}/home_agent" "${BIN_DIR}/home_agent"
install -m 500 "${SOURCE_DIR}/home_agent_launcher" "${BIN_DIR}/home_agent_launcher"
install -m 500 "${SOURCE_DIR}/home_agent_supervisor" "${BIN_DIR}/home_agent_supervisor"
install -m 500 "${SOURCE_DIR}/zone_host" "${BIN_DIR}/zone_host"
"${BIN_DIR}/home_agent_supervisor" key-init

sed \
  -e "s|__INSTALL_ROOT__|${INSTALL_ROOT}|g" \
  "${SOURCE_DIR}/dubhe-home-agent.service.in" \
  >"${UNIT_DIR}/dubhe-home-agent.service"
chmod 600 "${UNIT_DIR}/dubhe-home-agent.service"
systemctl --user daemon-reload

echo "Binaries and the node identity are installed."
echo "Complete signed enrollment/configuration, then run:"
echo "  systemctl --user enable --now dubhe-home-agent.service"
