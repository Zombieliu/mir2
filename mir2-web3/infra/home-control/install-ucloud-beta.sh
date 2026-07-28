#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: install-ucloud-beta.sh <source-root> <finalized-registration-json>" >&2
  exit 2
fi

source_root="$1"
registration_source="$2"
commit_id="923981964fc7b998db5ff2982cff1995ff1f59b0"
release_id="$(date -u +%Y%m%dT%H%M%SZ)-${commit_id:0:8}"
release_dir="/opt/mir2/home-control/releases/$release_id"
current_link="/opt/mir2/home-control/current"
config_dir="/etc/mir2/home-control"
state_dir="/var/lib/mir2/home-control"
secret_dir="$state_dir/secrets"
relay_hostname="relay-hk.obelisk.build"
relay_legacy_hostname="165.154.65.136.sslip.io"
node_id="ed25519:cc0993651c112ce0b10bafd2a1ee633589d0bc0ae781c8ecacb4fadd810d5c41"

for binary in \
  home_relay \
  home_enrollment_service \
  home_telemetry_collector \
  node_identity
do
  test -x "$source_root/target/release/$binary"
done
test -f "$registration_source"

sudo -n install -d -o root -g root -m 0755 \
  /opt/mir2/home-control/releases \
  "$release_dir"
sudo -n chown root:mir2 /etc/mir2
sudo -n chmod 0750 /etc/mir2
sudo -n install -d -o root -g mir2 -m 0750 "$config_dir"
sudo -n install -d -o mir2 -g mir2 -m 0750 "$state_dir"
sudo -n install -d -o mir2 -g mir2 -m 0700 "$secret_dir"

for binary in \
  home_relay \
  home_enrollment_service \
  home_telemetry_collector \
  node_identity
do
  sudo -n install -o root -g root -m 0755 \
    "$source_root/target/release/$binary" \
    "$release_dir/$binary"
done
(
  cd "$release_dir"
  sha256sum \
    home_relay \
    home_enrollment_service \
    home_telemetry_collector \
    node_identity
) | sudo -n tee "$release_dir/SHA256SUMS" >/dev/null
sudo -n chmod 0644 "$release_dir/SHA256SUMS"

generate_identity() {
  local name="$1"
  local key_path="$secret_dir/$name-signing.key"
  if ! sudo -n test -f "$key_path"; then
    sudo -n -u mir2 "$release_dir/node_identity" generate "$key_path" >/dev/null
  fi
  sudo -n -u mir2 "$release_dir/node_identity" inspect "$key_path" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["publicKey"])'
}

enrollment_public_key="$(generate_identity enrollment)"
control_public_key="$(generate_identity control)"
relay_public_key="$(generate_identity relay)"

if ! sudo -n test -f "$secret_dir/relay-ca.der"; then
  sudo -n -u mir2 openssl genpkey \
    -algorithm ED25519 \
    -out "$secret_dir/relay-ca-key.pem"
  sudo -n -u mir2 openssl req \
    -x509 \
    -new \
    -key "$secret_dir/relay-ca-key.pem" \
    -out "$secret_dir/relay-ca.pem" \
    -days 3650 \
    -subj "/CN=Obelisk Dubhe Home Relay Beta CA" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign,digitalSignature"
  sudo -n -u mir2 openssl genpkey \
    -algorithm ED25519 \
    -out "$secret_dir/relay-server-key.pem"
  sudo -n -u mir2 openssl req \
    -new \
    -key "$secret_dir/relay-server-key.pem" \
    -out "$secret_dir/relay-server.csr" \
    -subj "/CN=$relay_hostname" \
    -addext "subjectAltName=DNS:$relay_hostname,DNS:$relay_legacy_hostname" \
    -addext "extendedKeyUsage=serverAuth" \
    -addext "keyUsage=critical,digitalSignature"
  sudo -n -u mir2 openssl x509 \
    -req \
    -in "$secret_dir/relay-server.csr" \
    -CA "$secret_dir/relay-ca.pem" \
    -CAkey "$secret_dir/relay-ca-key.pem" \
    -CAcreateserial \
    -out "$secret_dir/relay-server.pem" \
    -days 825 \
    -copy_extensions copy
  sudo -n -u mir2 openssl x509 \
    -in "$secret_dir/relay-ca.pem" \
    -outform DER \
    -out "$secret_dir/relay-ca.der"
  sudo -n -u mir2 openssl pkcs8 \
    -topk8 \
    -nocrypt \
    -in "$secret_dir/relay-ca-key.pem" \
    -outform DER \
    -out "$secret_dir/relay-ca-key.der"
  sudo -n -u mir2 openssl x509 \
    -in "$secret_dir/relay-server.pem" \
    -outform DER \
    -out "$secret_dir/relay-server.der"
  sudo -n -u mir2 openssl pkcs8 \
    -topk8 \
    -nocrypt \
    -in "$secret_dir/relay-server-key.pem" \
    -outform DER \
    -out "$secret_dir/relay-server-key.der"
fi

if ! sudo -n test -f "$secret_dir/telemetry-operator.token"; then
  sudo -n -u mir2 sh -c \
    'umask 077; openssl rand -hex 32 > "$1"' \
    sh "$secret_dir/telemetry-operator.token"
fi
if ! sudo -n test -f "$secret_dir/gateway-relay.token"; then
  sudo -n -u mir2 sh -c \
    'umask 077; openssl rand -hex 32 > "$1"' \
    sh "$secret_dir/gateway-relay.token"
fi

for state_file in placements.json admissions.json; do
  if ! sudo -n test -f "$state_dir/$state_file"; then
    printf '[]\n' | sudo -n -u mir2 tee "$state_dir/$state_file" >/dev/null
    sudo -n chmod 0600 "$state_dir/$state_file"
  fi
done

sudo -n install -o root -g mir2 -m 0640 \
  "$registration_source" \
  "$config_dir/finalized-registrations.json"

sudo -n tee "$config_dir/enrollment.env" >/dev/null <<EOF
MIR2_HOME_ENROLLMENT_BIND=127.0.0.1:18080
MIR2_HOME_ENROLLMENT_SIGNING_KEY_FILE=$secret_dir/enrollment-signing.key
MIR2_HOME_ENROLLMENT_CONTROL_SIGNING_KEY_FILE=$secret_dir/control-signing.key
MIR2_HOME_ENROLLMENT_RELAY_PUBLIC_KEY=$relay_public_key
MIR2_HOME_ENROLLMENT_CONTROL_ISSUER_PUBLIC_KEY=$control_public_key
MIR2_HOME_ENROLLMENT_TLS_CA_CERTIFICATE_DER=$secret_dir/relay-ca.der
MIR2_HOME_ENROLLMENT_TLS_CA_KEY_DER=$secret_dir/relay-ca-key.der
MIR2_HOME_ENROLLMENT_PLACEMENTS_FILE=$state_dir/placements.json
MIR2_HOME_ENROLLMENT_ADMISSIONS_FILE=$state_dir/admissions.json
MIR2_HOME_ENROLLMENT_ALLOWED_NODE_IDS=$node_id
MIR2_HOME_ENROLLMENT_REGISTRATIONS_FILE=$config_dir/finalized-registrations.json
MIR2_HOME_ENROLLMENT_RELAY_ID=relay-hk-beta-1
MIR2_HOME_ENROLLMENT_RELAY_ADDR=$relay_hostname:9443
MIR2_HOME_ENROLLMENT_RELAY_SERVER_NAME=$relay_hostname
MIR2_HOME_ENROLLMENT_TELEMETRY_URL=https://$relay_hostname/home/telemetry/v1/telemetry
MIR2_HOME_ENROLLMENT_ALLOWED_GAMES=mir2
MIR2_HOME_ENROLLMENT_ALLOWED_ZONES=primary
MIR2_HOME_ENROLLMENT_MAX_SESSIONS=128
MIR2_HOME_ENROLLMENT_MAX_SESSIONS_PER_ZONE=32
MIR2_HOME_ENROLLMENT_MAX_ZONES=8
MIR2_HOME_ENROLLMENT_CPU_LIMIT_PERCENT=75
MIR2_HOME_ENROLLMENT_RESERVED_MEMORY_BYTES=2147483648
MIR2_HOME_ENROLLMENT_CAPACITY_CONCURRENT_SESSIONS=128
MIR2_HOME_ENROLLMENT_CAPACITY_SESSIONS_PER_ZONE=32
MIR2_HOME_ENROLLMENT_CAPACITY_ZONE_COUNT=8
MIR2_HOME_ENROLLMENT_CAPACITY_COMMANDS=2000
MIR2_HOME_ENROLLMENT_CAPACITY_MAXIMUM_P95_MS=100
MIR2_HOME_ENROLLMENT_CAPACITY_MINIMUM_SUCCESS_BPS=9990
MIR2_HOME_ENROLLMENT_CAPACITY_CERTIFICATE_TTL_MS=86400000
MIR2_HOME_ENROLLMENT_RELAY_CREDENTIAL_TTL_MS=86400000
MIR2_HOME_ENROLLMENT_BUNDLE_TTL_MS=86400000
EOF

sudo -n tee "$config_dir/relay.env" >/dev/null <<EOF
MIR2_HOME_RELAY_ID=relay-hk-beta-1
MIR2_HOME_RELAY_QUIC_BIND=0.0.0.0:9443
MIR2_HOME_RELAY_GATEWAY_BIND=127.0.0.1:9444
MIR2_HOME_RELAY_TLS_CA_DER=$secret_dir/relay-ca.der
MIR2_HOME_RELAY_TLS_CERT_CHAIN_DER=$secret_dir/relay-server.der
MIR2_HOME_RELAY_TLS_KEY_DER=$secret_dir/relay-server-key.der
MIR2_HOME_RELAY_SIGNING_KEY_FILE=$secret_dir/relay-signing.key
MIR2_HOME_CAPACITY_ISSUER_PUBLIC_KEY=$enrollment_public_key
MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY=$control_public_key
MIR2_HOME_PLACEMENTS_FILE=$state_dir/placements.json
MIR2_HOME_RELAY_GATEWAY_TOKEN_FILE=$secret_dir/gateway-relay.token
MIR2_HOME_RELAY_MAX_AGENT_CONNECTIONS=64
MIR2_HOME_RELAY_MAX_GATEWAY_CONNECTIONS=256
MIR2_HOME_RELAY_MAX_STREAMS_PER_NODE=128
EOF

gateway_relay_token="$(sudo -n cat "$secret_dir/gateway-relay.token")"
sudo -n sed -i \
  "s#^MIR2_HOME_RELAY_GATEWAY_TOKEN_FILE=.*#MIR2_HOME_RELAY_GATEWAY_TOKEN=$gateway_relay_token#" \
  "$config_dir/relay.env"

sudo -n tee "$config_dir/telemetry.env" >/dev/null <<EOF
MIR2_HOME_TELEMETRY_COLLECTOR_BIND=127.0.0.1:18081
MIR2_HOME_TELEMETRY_OPERATOR_TOKEN_FILE=$secret_dir/telemetry-operator.token
MIR2_HOME_TELEMETRY_ADMISSIONS_FILE=$state_dir/admissions.json
MIR2_HOME_TELEMETRY_ENROLLMENT_ISSUER_PUBLIC_KEY=$enrollment_public_key
MIR2_HOME_TELEMETRY_MAXIMUM_AGE_MS=120000
MIR2_HOME_TELEMETRY_RETENTION_MS=2592000000
EOF
sudo -n chmod 0640 \
  "$config_dir/enrollment.env" \
  "$config_dir/relay.env" \
  "$config_dir/telemetry.env"
sudo -n chown root:mir2 \
  "$config_dir/enrollment.env" \
  "$config_dir/relay.env" \
  "$config_dir/telemetry.env"

install_unit() {
  local service="$1"
  local environment="$2"
  local executable="$3"
  local memory_max="$4"
  sudo -n tee "/etc/systemd/system/$service.service" >/dev/null <<EOF
[Unit]
Description=Dubhe Home $service
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=mir2
Group=mir2
EnvironmentFile=$config_dir/$environment
ExecStart=$current_link/$executable
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
ReadWritePaths=$state_dir
MemoryMax=$memory_max
CPUQuota=200%
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
}

install_unit \
  dubhe-home-enrollment \
  enrollment.env \
  home_enrollment_service \
  512M
install_unit \
  dubhe-home-relay \
  relay.env \
  home_relay \
  1G
install_unit \
  dubhe-home-telemetry \
  telemetry.env \
  home_telemetry_collector \
  512M

sudo -n ln -sfn "$release_dir" "$current_link"
sudo -n systemctl daemon-reload
sudo -n systemctl enable --now \
  dubhe-home-enrollment.service \
  dubhe-home-relay.service \
  dubhe-home-telemetry.service

echo "DUBHE_HOME_CONTROL_INSTALLED release=$release_id"
