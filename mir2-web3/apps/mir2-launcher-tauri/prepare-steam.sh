#!/bin/bash
# Steam packaging for the Mir2 Tauri desktop launcher.
#
# Generates steam_appid.txt next to the built binary and bundles the native
# steam_api library into the Tauri resources so the launcher can initialize
# Steamworks at runtime.
#
# Usage:
#   STEAM_APP_ID=480 ./prepare-steam.sh macos   # or: windows / linux
#   STEAM_API_LIB=/path/to/steam_api ./prepare-steam.sh windows
#
# Environment:
#   STEAM_APP_ID       the Steam App ID (default 480 = placeholder; set yours)
#   STEAM_API_LIB      path to the native steam_api library (.dll/.so/.dylib)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LAUNCHER="$ROOT/apps/mir2-launcher-tauri"
TAURI_SRC="$LAUNCHER/src-tauri"
STEAM_APP_ID="${STEAM_APP_ID:-480}"
TARGET="${1:-host}"

# steam_appid.txt lets Steamworks initialize when launched outside Steam too.
mkdir -p "$TAURI_SRC/resources"
cat > "$TAURI_SRC/resources/steam_appid.txt" << EOF
${STEAM_APP_ID}
EOF
echo "[steam] wrote steam_appid.txt with AppID ${STEAM_APP_ID}"

# Copy the native steam_api library into resources for the given target.
STEAM_API_LIB="${STEAM_API_LIB:-}"
if [[ -n "${STEAM_API_LIB}" ]]; then
  case "${TARGET}" in
    windows) DEST_SUFFIX="x86_64-pc-windows-gnu/steam_api64.dll" ;;
    macos)   DEST_SUFFIX="darwin/steam_api.dylib" ;;
    linux)   DEST_SUFFIX="linux/steam_api.so" ;;
    *) echo "unknown target: ${TARGET}" >&2; exit 1 ;;
  esac
  DEST="$TAURI_SRC/resources/$DEST_SUFFIX"
  mkdir -p "$(dirname "$DEST")"
  cp "$STEAM_API_LIB" "$DEST"
  echo "[steam] copied steam_api lib -> ${DEST}"
else
  echo "[steam] STEAM_API_LIB not set; skipping native library copy"
fi

echo "[steam] preparation done"
