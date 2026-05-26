#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'TXT'
Usage:
  MIR2_GATEWAY_RELEASE_URL=https://.../mir2-gateway-linux-x64.tar.gz \
  MIR2_GATEWAY_RELEASE_SHA256_URL=https://.../mir2-gateway-linux-x64.tar.gz.sha256 \
  bash scripts/install-gateway-release.sh

Required:
  MIR2_GATEWAY_RELEASE_URL

Optional:
  MIR2_GATEWAY_RELEASE_SHA256_URL  URL for a sha256sum-style checksum file.
  MIR2_GATEWAY_RELEASE_SHA256      Raw expected archive sha256.
  MIR2_GATEWAY_RELEASE_TAG         Override install release directory name.
  MIR2_GATEWAY_START               1 to restart/enable systemd after install. Default: 1.
  MIR2_GATEWAY_USER                Service user. Default: mir2.
TXT
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

release_url="${MIR2_GATEWAY_RELEASE_URL:-}"
if [ -z "$release_url" ]; then
  usage >&2
  exit 2
fi

if [ "$(id -u)" -eq 0 ]; then
  sudo_cmd=()
else
  sudo_cmd=(sudo)
fi

service_user="${MIR2_GATEWAY_USER:-mir2}"
install_root="${MIR2_GATEWAY_INSTALL_ROOT:-/opt/mir2/gateway}"
data_dir="${MIR2_GATEWAY_DATA_DIR:-/var/lib/mir2}"
log_dir="${MIR2_GATEWAY_LOG_DIR:-/var/log/mir2}"
env_path="${MIR2_GATEWAY_ENV_PATH:-/etc/mir2/gateway.env}"
service_path="${MIR2_GATEWAY_SERVICE_PATH:-/etc/systemd/system/mir2-gateway.service}"
start_service="${MIR2_GATEWAY_START:-1}"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/mir2-gateway-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

archive="$tmp/mir2-gateway.tar.gz"
curl -fsSL "$release_url" -o "$archive"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

actual_sha256="$(sha256_file "$archive")"
if [ -n "${MIR2_GATEWAY_RELEASE_SHA256_URL:-}" ]; then
  checksum_file="$tmp/mir2-gateway.tar.gz.sha256"
  curl -fsSL "$MIR2_GATEWAY_RELEASE_SHA256_URL" -o "$checksum_file"
  expected_sha256="$(awk '{print $1}' "$checksum_file")"
elif [ -n "${MIR2_GATEWAY_RELEASE_SHA256:-}" ]; then
  expected_sha256="$MIR2_GATEWAY_RELEASE_SHA256"
else
  expected_sha256=""
fi

if [ -n "$expected_sha256" ] && [ "$actual_sha256" != "$expected_sha256" ]; then
  echo "sha256 mismatch: expected $expected_sha256, got $actual_sha256" >&2
  exit 1
fi

unpack="$tmp/unpack"
mkdir -p "$unpack"
tar -xzf "$archive" -C "$unpack"

if [ ! -x "$unpack/mir2-gateway" ]; then
  echo "release package does not contain executable mir2-gateway" >&2
  exit 1
fi

release_tag="${MIR2_GATEWAY_RELEASE_TAG:-}"
if [ -z "$release_tag" ] && [ -f "$unpack/RELEASE.json" ]; then
  release_tag="$(sed -n 's/.*"tag": *"\([^"]*\)".*/\1/p' "$unpack/RELEASE.json" | head -n 1)"
fi
if [ -z "$release_tag" ]; then
  release_tag="$(date -u +%Y%m%dT%H%M%SZ)"
fi

release_dir="$install_root/releases/$release_tag"

if ! id "$service_user" >/dev/null 2>&1; then
  "${sudo_cmd[@]}" useradd --system --home "$data_dir" --shell /usr/sbin/nologin "$service_user"
fi

"${sudo_cmd[@]}" mkdir -p "$release_dir" "$install_root/releases" "$data_dir" "$log_dir" "$(dirname "$env_path")"
"${sudo_cmd[@]}" tar -xzf "$archive" -C "$release_dir"
"${sudo_cmd[@]}" chmod 0755 "$release_dir/mir2-gateway"
"${sudo_cmd[@]}" chown -R "$service_user:$service_user" "$install_root" "$data_dir" "$log_dir"
"${sudo_cmd[@]}" ln -sfn "$release_dir" "$install_root/current"

if [ -f "$release_dir/systemd/mir2-gateway.service" ]; then
  "${sudo_cmd[@]}" cp "$release_dir/systemd/mir2-gateway.service" "$service_path"
fi

if [ ! -f "$env_path" ]; then
  if [ ! -f "$release_dir/systemd/mir2-gateway.env.example" ]; then
    echo "missing systemd env template; create $env_path manually" >&2
    exit 1
  fi
  "${sudo_cmd[@]}" cp "$release_dir/systemd/mir2-gateway.env.example" "$env_path"
  if command -v openssl >/dev/null 2>&1; then
    secret="$(openssl rand -hex 32)"
  else
    secret="$(date +%s%N | sha256sum | awk '{print $1}')"
  fi
  "${sudo_cmd[@]}" sed -i "s/replace-with-random-32-byte-secret/$secret/g" "$env_path"
  "${sudo_cmd[@]}" chmod 0600 "$env_path"
  "${sudo_cmd[@]}" chown root:root "$env_path"
fi

if command -v systemctl >/dev/null 2>&1; then
  "${sudo_cmd[@]}" systemctl daemon-reload
  if [ "$start_service" = "1" ]; then
    "${sudo_cmd[@]}" systemctl enable --now mir2-gateway
    "${sudo_cmd[@]}" systemctl restart mir2-gateway
  fi
fi

echo "installed mir2-gateway release $release_tag"
echo "archive sha256: $actual_sha256"
echo "current: $install_root/current"
echo "env: $env_path"
echo "health: curl -fsS http://127.0.0.1:7110/health"
