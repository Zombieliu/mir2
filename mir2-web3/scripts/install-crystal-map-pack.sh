#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'TXT'
Install the Crystal map pack (gzipped .map collision data) so the gateway can
host every non-starter map, then wire MIR2_CRYSTAL_MAP_PACK into the gateway env.

The gateway loads map collision from CRYSTAL_CLIENT_ROOT/Map/*.map first and
falls back to this pack for any map the client install does not carry. Without
the pack only the client-root maps are reachable.

Source precedence (first match wins):
  MIR2_CRYSTAL_MAP_PACK_RELEASE_URL  tar.gz URL (+ optional _SHA256_URL / _SHA256)
  MIR2_CRYSTAL_MAP_PACK_ARCHIVE      local tar.gz path
  MIR2_CRYSTAL_MAP_PACK_SRC          directory of *.map.gz
                                     (default: in-repo apps/web/lib/generated/crystal-map-pack)

Optional:
  MIR2_CRYSTAL_MAP_PACK_DIR              install target. Default: /var/lib/mir2/crystal-map-pack
  MIR2_GATEWAY_ENV_PATH                  Default: /etc/mir2/gateway.env
  MIR2_GATEWAY_USER                      Service user. Default: mir2
  MIR2_CRYSTAL_MAP_PACK_RESTART_GATEWAY  1 to restart mir2-gateway after install. Default: 1
  MIR2_INSTALL_NO_SUDO                   1 to run without sudo for local smoke tests
TXT
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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
install_dir="${MIR2_CRYSTAL_MAP_PACK_DIR:-/var/lib/mir2/crystal-map-pack}"
env_path="${MIR2_GATEWAY_ENV_PATH:-/etc/mir2/gateway.env}"
restart_gateway="${MIR2_CRYSTAL_MAP_PACK_RESTART_GATEWAY:-1}"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/mir2-crystal-map-pack-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

staged="$tmp/pack"
mkdir -p "$staged"

release_url="${MIR2_CRYSTAL_MAP_PACK_RELEASE_URL:-}"
archive_path="${MIR2_CRYSTAL_MAP_PACK_ARCHIVE:-}"
src_dir="${MIR2_CRYSTAL_MAP_PACK_SRC:-$ROOT/apps/web/lib/generated/crystal-map-pack}"

if [ -n "$release_url" ]; then
  archive="$tmp/crystal-map-pack.tar.gz"
  curl -fsSL "$release_url" -o "$archive"
  actual_sha256="$(sha256_file "$archive")"
  if [ -n "${MIR2_CRYSTAL_MAP_PACK_RELEASE_SHA256_URL:-}" ]; then
    curl -fsSL "$MIR2_CRYSTAL_MAP_PACK_RELEASE_SHA256_URL" -o "$tmp/sum"
    expected_sha256="$(awk '{print $1}' "$tmp/sum")"
  else
    expected_sha256="${MIR2_CRYSTAL_MAP_PACK_RELEASE_SHA256:-}"
  fi
  if [ -n "$expected_sha256" ] && [ "$actual_sha256" != "$expected_sha256" ]; then
    echo "sha256 mismatch: expected $expected_sha256, got $actual_sha256" >&2
    exit 1
  fi
  tar -xzf "$archive" -C "$staged"
  source_desc="url:$release_url"
elif [ -n "$archive_path" ]; then
  tar -xzf "$archive_path" -C "$staged"
  source_desc="archive:$archive_path"
elif [ -d "$src_dir" ]; then
  cp -R "$src_dir/." "$staged/"
  source_desc="src:$src_dir"
else
  echo "no map-pack source found (set MIR2_CRYSTAL_MAP_PACK_RELEASE_URL / _ARCHIVE / _SRC)" >&2
  exit 2
fi

# The source may nest *.map.gz under one or more directories (e.g. a git-archive
# tarball). Pin staged to whichever directory actually holds the *.map.gz files.
if [ -z "$(find "$staged" -maxdepth 1 -name '*.map.gz' -print -quit)" ]; then
  first_pack="$(find "$staged" -name '*.map.gz' -print -quit)"
  if [ -n "$first_pack" ]; then
    staged="$(dirname "$first_pack")"
  fi
fi

pack_count="$(find "$staged" -maxdepth 1 -name '*.map.gz' | wc -l | tr -d '[:space:]')"
if [ "$pack_count" = "0" ]; then
  echo "map-pack source contains no *.map.gz files" >&2
  exit 1
fi

if [ "${MIR2_INSTALL_NO_SUDO:-0}" != "1" ] && ! id "$service_user" >/dev/null 2>&1; then
  as_root useradd --system --home /var/lib/mir2 --shell /usr/sbin/nologin "$service_user"
fi

as_root mkdir -p "$install_dir" "$(dirname "$env_path")"
as_root cp "$staged"/*.map.gz "$install_dir"/
as_root chmod -R a+rX "$install_dir"
if [ "${MIR2_INSTALL_NO_SUDO:-0}" != "1" ]; then
  as_root chown -R "$service_user:$service_user" "$install_dir"
fi

env_tmp="$tmp/gateway.env"
if [ -f "$env_path" ]; then
  as_root cat "$env_path" > "$env_tmp"
else
  : > "$env_tmp"
fi

awk -v value="$install_dir" '
  BEGIN { done = 0 }
  /^#?MIR2_CRYSTAL_MAP_PACK=/ {
    if (done == 0) {
      print "MIR2_CRYSTAL_MAP_PACK=" value
      done = 1
    }
    next
  }
  { print }
  END {
    if (done == 0) {
      print "MIR2_CRYSTAL_MAP_PACK=" value
    }
  }
' "$env_tmp" > "$env_tmp.next"

as_root cp "$env_tmp.next" "$env_path"
as_root chmod 0600 "$env_path"
if [ "${MIR2_INSTALL_NO_SUDO:-0}" != "1" ]; then
  as_root chown root:root "$env_path"
fi

if command -v systemctl >/dev/null 2>&1 && [ "$restart_gateway" = "1" ]; then
  if systemctl list-unit-files mir2-gateway.service >/dev/null 2>&1; then
    as_root systemctl restart mir2-gateway
  fi
fi

echo "installed Crystal map pack ($source_desc)"
echo "map-pack files: $pack_count"
echo "install dir: $install_dir"
echo "env: $env_path (MIR2_CRYSTAL_MAP_PACK=$install_dir)"
