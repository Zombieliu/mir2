#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: install-ucloud-devnet.sh <build-root> <source-commit>" >&2
  exit 2
fi

build_root="$1"
source_commit="$2"
installer_dir="$(cd "$(dirname "$0")" && pwd)"
reconciler_source="$installer_dir/reconcile-finality.py"
placement_renewer_source="$installer_dir/renew-placement.py"
release_id="$(date -u +%Y%m%dT%H%M%SZ)-${source_commit:0:8}"
release_dir="/opt/mir2/commonware/releases/$release_id"
current_link="/opt/mir2/commonware/current"
config_dir="/etc/mir2/commonware"
state_dir="/var/lib/mir2/commonware"
gateway_env="/etc/mir2/gateway.env"

for binary in \
  gate14_validator \
  gate14_gateway \
  gate14_projector \
  mir2-gateway \
  home_player_probe
do
  test -x "$build_root/target/release/$binary"
done
sudo -n test -r "$gateway_env"
test -f "$reconciler_source"
test -f "$placement_renewer_source"

read_env() {
  local name="$1"
  sudo -n awk -F= -v name="$name" '
    $1 == name {
      sub(/^[^=]*=/, "")
      print
      exit
    }
  ' "$gateway_env"
}

database_url="$(read_env MIR2_ACCOUNT_STORE_DATABASE_URL)"
redis_url="$(read_env MIR2_GATEWAY_REDIS_CACHE_URL)"
test -n "$database_url"
test -n "$redis_url"

sudo -n install -d -o root -g root -m 0755 \
  /opt/mir2/commonware/releases \
  /opt/mir2/commonware/bin \
  "$release_dir"
sudo -n install -d -o root -g mir2 -m 0750 "$config_dir"
sudo -n install -d -o mir2 -g mir2 -m 0750 "$state_dir"

for index in 0 1 2 3; do
  sudo -n install -d -o mir2 -g mir2 -m 0750 "$state_dir/validator-$index"
done

for binary in \
  gate14_validator \
  gate14_gateway \
  gate14_projector \
  mir2-gateway \
  home_player_probe
do
  sudo -n install -o root -g root -m 0755 \
    "$build_root/target/release/$binary" \
    "$release_dir/$binary"
done
(
  cd "$release_dir"
  sha256sum \
    gate14_validator \
    gate14_gateway \
    gate14_projector \
    mir2-gateway \
    home_player_probe
) | sudo -n tee "$release_dir/SHA256SUMS" >/dev/null
printf '%s\n' "$source_commit" |
  sudo -n tee "$release_dir/SOURCE_COMMIT" >/dev/null
sudo -n chmod 0644 "$release_dir/SHA256SUMS" "$release_dir/SOURCE_COMMIT"
sudo -n install -o root -g root -m 0755 \
  "$reconciler_source" \
  /opt/mir2/commonware/bin/reconcile-finality.py
sudo -n install -o root -g root -m 0755 \
  "$placement_renewer_source" \
  /opt/mir2/commonware/bin/renew-placement.py

for index in 0 1 2 3; do
  p2p_port="$((19300 + index))"
  api_port="$((19400 + index))"
  bootstrap=""
  if [ "$index" -ne 0 ]; then
    bootstrap="GATE14_BOOTSTRAPPERS=0@127.0.0.1:19300"
  fi
  sudo -n tee "$config_dir/validator-$index.env" >/dev/null <<EOF
GATE14_VALIDATOR_SEED=$index
GATE14_VALIDATOR_ID=ucloud-hk-validator-$index
GATE14_PARTICIPANTS=0,1,2,3
GATE14_P2P_BIND=127.0.0.1:$p2p_port
GATE14_P2P_ADVERTISE=127.0.0.1:$p2p_port
$bootstrap
GATE14_API_BIND=127.0.0.1:$api_port
GATE14_DATA_DIR=$state_dir/validator-$index
RUST_LOG=info
EOF
done

validator_urls="http://127.0.0.1:19400,http://127.0.0.1:19401,http://127.0.0.1:19402,http://127.0.0.1:19403"
sudo -n tee "$config_dir/gateway.env" >/dev/null <<EOF
GATE14_GATEWAY_ID=ucloud-hk-control-1
GATE14_VALIDATOR_URLS=$validator_urls
GATE14_GATEWAY_BIND=127.0.0.1:19500
GATE14_REDIS_URL=$redis_url
EOF
sudo -n tee "$config_dir/projector.env" >/dev/null <<EOF
GATE14_PROJECTOR_ID=ucloud-hk-projector-1
GATE14_VALIDATOR_URLS=$validator_urls
GATE14_PROJECTOR_BIND=127.0.0.1:19600
GATE14_DATABASE_URL=$database_url
GATE14_REDIS_URL=$redis_url
EOF
sudo -n chown root:mir2 \
  "$config_dir/validator-0.env" \
  "$config_dir/validator-1.env" \
  "$config_dir/validator-2.env" \
  "$config_dir/validator-3.env" \
  "$config_dir/gateway.env" \
  "$config_dir/projector.env"
sudo -n chmod 0640 \
  "$config_dir/validator-0.env" \
  "$config_dir/validator-1.env" \
  "$config_dir/validator-2.env" \
  "$config_dir/validator-3.env" \
  "$config_dir/gateway.env" \
  "$config_dir/projector.env"

sudo -n tee /etc/systemd/system/mir2-commonware-validator@.service >/dev/null <<EOF
[Unit]
Description=Mir2 Commonware validator %i
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=mir2
Group=mir2
EnvironmentFile=$config_dir/validator-%i.env
ExecStart=$current_link/gate14_validator
Restart=on-failure
RestartSec=2s
TimeoutStopSec=20s
KillSignal=SIGINT
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
ReadWritePaths=$state_dir
MemoryMax=768M
CPUQuota=100%
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

sudo -n tee /etc/systemd/system/mir2-commonware-gateway.service >/dev/null <<EOF
[Unit]
Description=Mir2 Gate 14 Commonware control gateway
After=network-online.target mir2-commonware-validator@0.service mir2-commonware-validator@1.service mir2-commonware-validator@2.service mir2-commonware-validator@3.service
Wants=network-online.target

[Service]
Type=simple
User=mir2
Group=mir2
EnvironmentFile=$config_dir/gateway.env
ExecStart=$current_link/gate14_gateway
Restart=on-failure
RestartSec=2s
TimeoutStopSec=15s
KillSignal=SIGINT
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
MemoryMax=512M
CPUQuota=100%
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

sudo -n tee /etc/systemd/system/mir2-commonware-projector.service >/dev/null <<EOF
[Unit]
Description=Mir2 Gate 14 finalized-state projector
After=network-online.target mir2-commonware-gateway.service postgresql.service redis-server.service
Wants=network-online.target

[Service]
Type=simple
User=mir2
Group=mir2
EnvironmentFile=$config_dir/projector.env
ExecStart=$current_link/gate14_projector
Restart=on-failure
RestartSec=2s
TimeoutStopSec=15s
KillSignal=SIGINT
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
MemoryMax=512M
CPUQuota=100%
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

sudo -n tee /etc/systemd/system/mir2-commonware-reconcile.service >/dev/null <<EOF
[Unit]
Description=Mir2 Commonware quorum-verified validator catch-up
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=mir2
Group=mir2
ExecStart=/usr/bin/python3 /opt/mir2/commonware/bin/reconcile-finality.py
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
MemoryMax=256M
CPUQuota=50%

[Install]
WantedBy=multi-user.target
EOF

sudo -n tee /etc/systemd/system/mir2-commonware-reconcile.timer >/dev/null <<EOF
[Unit]
Description=Run Mir2 Commonware validator catch-up every 10 seconds

[Timer]
OnBootSec=15s
OnUnitActiveSec=10s
AccuracySec=1s
Persistent=true
Unit=mir2-commonware-reconcile.service

[Install]
WantedBy=timers.target
EOF

sudo -n tee /etc/systemd/system/mir2-commonware-placement-renew.service >/dev/null <<EOF
[Unit]
Description=Renew Mir2 Commonware Home Node placement
After=mir2-commonware-gateway.service

[Service]
Type=oneshot
User=mir2
Group=mir2
ExecStart=/usr/bin/python3 /opt/mir2/commonware/bin/renew-placement.py
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
MemoryMax=128M
CPUQuota=25%
EOF

sudo -n tee /etc/systemd/system/mir2-commonware-placement-renew.timer >/dev/null <<EOF
[Unit]
Description=Check Mir2 Commonware placement expiry every 15 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=15min
AccuracySec=30s
Persistent=true
Unit=mir2-commonware-placement-renew.service

[Install]
WantedBy=timers.target
EOF

sudo -n ln -sfn "$release_dir" "$current_link"
sudo -n systemctl daemon-reload
sudo -n systemctl enable --now \
  mir2-commonware-validator@0.service \
  mir2-commonware-validator@1.service \
  mir2-commonware-validator@2.service \
  mir2-commonware-validator@3.service

for _ in $(seq 1 60); do
  healthy=0
  for port in 19400 19401 19402 19403; do
    if curl -fsS --max-time 1 "http://127.0.0.1:$port/healthz" >/dev/null; then
      healthy="$((healthy + 1))"
    fi
  done
  if [ "$healthy" -eq 4 ]; then
    break
  fi
  sleep 1
done
test "$healthy" -eq 4

sudo -n systemctl enable --now \
  mir2-commonware-gateway.service \
  mir2-commonware-projector.service
sudo -n systemctl enable --now mir2-commonware-reconcile.timer
sudo -n systemctl enable --now mir2-commonware-placement-renew.timer

echo "COMMONWARE_UCLOUD_DEVNET_INSTALLED release=$release_id"
