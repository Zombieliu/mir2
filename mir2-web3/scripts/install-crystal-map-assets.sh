#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'TXT'
Usage:
  MIR2_CRYSTAL_MAP_RELEASE_URL=https://.../mir2-crystal-map-assets.tar.gz \
  MIR2_CRYSTAL_MAP_RELEASE_SHA256_URL=https://.../mir2-crystal-map-assets.tar.gz.sha256 \
  bash scripts/install-crystal-map-assets.sh

Required:
  MIR2_CRYSTAL_MAP_RELEASE_URL

Optional:
  MIR2_CRYSTAL_MAP_RELEASE_SHA256_URL  URL for a sha256sum-style checksum file.
  MIR2_CRYSTAL_MAP_RELEASE_SHA256      Raw expected archive sha256.
  MIR2_CRYSTAL_MAP_RELEASE_TAG         Override install release directory name.
  MIR2_CRYSTAL_MAP_INSTALL_ROOT        Default: /var/lib/mir2/crystal-client.
  MIR2_GATEWAY_ENV_PATH                Default: /etc/mir2/gateway.env.
  MIR2_GATEWAY_USER                    Service user. Default: mir2.
  MIR2_CRYSTAL_MAP_RESTART_GATEWAY     1 to restart mir2-gateway after install. Default: 1.
  MIR2_INSTALL_NO_SUDO                 1 to run without sudo for local smoke tests.
TXT
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

release_url="${MIR2_CRYSTAL_MAP_RELEASE_URL:-}"
if [ -z "$release_url" ]; then
  usage >&2
  exit 2
fi

if [ "${MIR2_INSTALL_NO_SUDO:-0}" = "1" ] || [ "$(id -u)" -eq 0 ]; then
  use_sudo=0
else
  use_sudo=1
fi

as_root() {
  if [ "$use_sudo" = "1" ]; then
    sudo "$@"
  else
    "$@"
  fi
}

service_user="${MIR2_GATEWAY_USER:-mir2}"
install_root="${MIR2_CRYSTAL_MAP_INSTALL_ROOT:-/var/lib/mir2/crystal-client}"
env_path="${MIR2_GATEWAY_ENV_PATH:-/etc/mir2/gateway.env}"
restart_gateway="${MIR2_CRYSTAL_MAP_RESTART_GATEWAY:-1}"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/mir2-crystal-map-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

archive="$tmp/mir2-crystal-map-assets.tar.gz"
curl -fsSL "$release_url" -o "$archive"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

actual_sha256="$(sha256_file "$archive")"
if [ -n "${MIR2_CRYSTAL_MAP_RELEASE_SHA256_URL:-}" ]; then
  checksum_file="$tmp/mir2-crystal-map-assets.tar.gz.sha256"
  curl -fsSL "$MIR2_CRYSTAL_MAP_RELEASE_SHA256_URL" -o "$checksum_file"
  expected_sha256="$(awk '{print $1}' "$checksum_file")"
elif [ -n "${MIR2_CRYSTAL_MAP_RELEASE_SHA256:-}" ]; then
  expected_sha256="$MIR2_CRYSTAL_MAP_RELEASE_SHA256"
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

if [ ! -d "$unpack/Map" ] || [ -z "$(find "$unpack/Map" -maxdepth 1 -type f \( -name '*.map' -o -name '*.MAP' \) -print -quit)" ]; then
  echo "release package does not contain Map/*.map files" >&2
  exit 1
fi

release_tag="${MIR2_CRYSTAL_MAP_RELEASE_TAG:-}"
if [ -z "$release_tag" ] && [ -f "$unpack/RELEASE.json" ]; then
  release_tag="$(sed -n 's/.*"tag": *"\([^"]*\)".*/\1/p' "$unpack/RELEASE.json" | head -n 1)"
fi
if [ -z "$release_tag" ]; then
  release_tag="$(date -u +%Y%m%dT%H%M%SZ)"
fi

release_dir="$install_root/releases/$release_tag"
current_link="$install_root/current"

if [ "${MIR2_INSTALL_NO_SUDO:-0}" != "1" ] && ! id "$service_user" >/dev/null 2>&1; then
  as_root useradd --system --home /var/lib/mir2 --shell /usr/sbin/nologin "$service_user"
fi

as_root mkdir -p "$release_dir" "$install_root/releases" "$(dirname "$env_path")"
as_root tar -xzf "$archive" -C "$release_dir"
if [ "${MIR2_INSTALL_NO_SUDO:-0}" != "1" ]; then
  as_root chown -R "$service_user:$service_user" "$install_root"
fi
as_root ln -sfn "$release_dir" "$current_link"

env_tmp="$tmp/gateway.env"
if [ -f "$env_path" ]; then
  as_root cat "$env_path" > "$env_tmp"
else
  : > "$env_tmp"
fi

awk -v value="$current_link" '
  BEGIN { done = 0 }
  /^#?CRYSTAL_CLIENT_ROOT=/ {
    if (done == 0) {
      print "CRYSTAL_CLIENT_ROOT=" value
      done = 1
    }
    next
  }
  { print }
  END {
    if (done == 0) {
      print "CRYSTAL_CLIENT_ROOT=" value
    }
  }
' "$env_tmp" > "$env_tmp.next"

as_root cp "$env_tmp.next" "$env_path"
as_root chmod 0600 "$env_path"
if [ "${MIR2_INSTALL_NO_SUDO:-0}" != "1" ]; then
  as_root chown root:root "$env_path"
fi

if command -v systemctl >/dev/null 2>&1 && [ "$restart_gateway" = "1" ]; then
  as_root systemctl daemon-reload
  if systemctl list-unit-files mir2-gateway.service >/dev/null 2>&1; then
    as_root systemctl restart mir2-gateway
  fi
fi

map_file_count="$(find "$unpack/Map" -maxdepth 1 -type f \( -name '*.map' -o -name '*.MAP' \) | wc -l | tr -d '[:space:]')"

echo "installed Crystal map asset release $release_tag"
echo "archive sha256: $actual_sha256"
echo "map files: $map_file_count"
echo "current: $current_link"
echo "env: $env_path"
