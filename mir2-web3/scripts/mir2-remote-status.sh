#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${MIR2_REMOTE_HOST:-ubuntu@165.154.65.136}"
REMOTE_PORT="${MIR2_REMOTE_PORT:-22}"
REMOTE_BIN="${MIR2_REMOTE_STATUS_BIN:-/usr/local/bin/mir2-status}"
USE_SUDO=0

usage() {
  cat <<'USAGE'
Usage: mir2-remote-status [--sudo] [mir2-status options]

Runs the remote UCloud Mir2 status script from this Mac over SSH.

Examples:
  mir2-remote-status
  mir2-remote-status --logs
  mir2-remote-status --sudo --logs

Environment:
  MIR2_REMOTE_HOST        default: ubuntu@165.154.65.136
  MIR2_REMOTE_PORT        default: 22
  MIR2_REMOTE_STATUS_BIN  default: /usr/local/bin/mir2-status
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --sudo)
      USE_SUDO=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      break
      ;;
  esac
done

quote_remote_arg() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

remote_cmd=""
if [ "$USE_SUDO" -eq 1 ]; then
  remote_cmd="sudo "
fi
remote_cmd="${remote_cmd}$(quote_remote_arg "$REMOTE_BIN")"
for arg in "$@"; do
  remote_cmd="${remote_cmd} $(quote_remote_arg "$arg")"
done

if [ "$USE_SUDO" -eq 1 ]; then
  exec ssh -t -p "$REMOTE_PORT" -o StrictHostKeyChecking=no "$REMOTE_HOST" "$remote_cmd"
else
  exec ssh -p "$REMOTE_PORT" -o StrictHostKeyChecking=no "$REMOTE_HOST" "$remote_cmd"
fi
