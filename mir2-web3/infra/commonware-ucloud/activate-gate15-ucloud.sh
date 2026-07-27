#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: activate-gate15-ucloud.sh <commonware-release-root> <source-commit>" >&2
  exit 2
fi

source_root="$1"
source_commit="$2"
gateway_env="/etc/mir2/gateway.env"
gateway_current="/opt/mir2/gateway/current"
release_id="$(date -u +%Y%m%dT%H%M%SZ)-${source_commit:0:8}-gate15"
release_dir="/opt/mir2/gateway/releases/$release_id"
backup_dir="/var/lib/mir2/commonware/cutover-backups/$release_id"
validator_urls="http://127.0.0.1:19400,http://127.0.0.1:19401,http://127.0.0.1:19402,http://127.0.0.1:19403"

test -x "$source_root/mir2-gateway"
test -x "$source_root/home_player_probe"
sudo -n test -f "$gateway_env"
previous_release="$(readlink -f "$gateway_current")"
test -n "$previous_release"

sudo -n install -d -o root -g root -m 0755 "$release_dir"
sudo -n install -d -o root -g mir2 -m 0750 "$backup_dir"
sudo -n cp -a "$gateway_env" "$backup_dir/gateway.env"
printf '%s\n' "$previous_release" |
  sudo -n tee "$backup_dir/previous-release" >/dev/null

sudo -n install -o root -g root -m 0755 \
  "$source_root/mir2-gateway" \
  "$release_dir/mir2-gateway"
sudo -n install -o root -g root -m 0755 \
  "$source_root/home_player_probe" \
  "$release_dir/home_player_probe"
(
  cd "$release_dir"
  sha256sum mir2-gateway home_player_probe
) | sudo -n tee "$release_dir/SHA256SUMS" >/dev/null
printf '%s\n' "$source_commit" |
  sudo -n tee "$release_dir/SOURCE_COMMIT" >/dev/null
sudo -n chmod 0644 "$release_dir/SHA256SUMS" "$release_dir/SOURCE_COMMIT"

temporary_env="$(mktemp)"
trap 'rm -f "$temporary_env"' EXIT
sudo -n awk '
  !/^MIR2_GATEWAY_INSTANCE_ID=/ &&
  !/^MIR2_GATE15_GATEWAY_ID=/ &&
  !/^MIR2_GATE15_VALIDATOR_URLS=/ &&
  !/^MIR2_GATE15_SESSION_LEASE_TTL_MS=/ &&
  !/^MIR2_GATE15_OBSERVER_INTERVAL_MS=/
' "$gateway_env" >"$temporary_env"
cat >>"$temporary_env" <<EOF
MIR2_GATEWAY_INSTANCE_ID=ucloud-hk-player-gateway-1
MIR2_GATE15_GATEWAY_ID=ucloud-hk-player-gateway-1
MIR2_GATE15_VALIDATOR_URLS=$validator_urls
MIR2_GATE15_SESSION_LEASE_TTL_MS=60000
MIR2_GATE15_OBSERVER_INTERVAL_MS=200
EOF
sudo -n install -o root -g mir2 -m 0640 "$temporary_env" "$gateway_env"
sudo -n ln -sfn "$release_dir" "$gateway_current"

rollback() {
  sudo -n cp -a "$backup_dir/gateway.env" "$gateway_env"
  sudo -n ln -sfn "$previous_release" "$gateway_current"
  sudo -n systemctl restart mir2-gateway.service
}

sudo -n systemctl restart mir2-gateway.service
healthy=0
for _ in $(seq 1 45); do
  if health="$(curl -fsS --max-time 2 http://127.0.0.1:7110/health 2>/dev/null)" &&
    python3 -c '
import json,sys
h=json.load(sys.stdin)
g=h.get("gate15") or {}
raise SystemExit(0 if h.get("ok") and g.get("enabled") and g.get("healthy") else 1)
' <<<"$health"
  then
    healthy=1
    break
  fi
  sleep 1
done
if [ "$healthy" -ne 1 ]; then
  rollback
  echo "Gate15 cutover failed; previous Gateway release restored" >&2
  exit 1
fi

sudo -n systemctl stop mir2-gateway-gate15-canary.service 2>/dev/null || true
echo "GATE15_UCLOUD_CUTOVER_PASS release=$release_id backup=$backup_dir"
