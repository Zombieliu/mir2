#!/bin/bash
# Stage the runtime asset tree beside a native Mir2 executable.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
DESTINATION="${1:?usage: package-assets.sh DESTINATION}"

if [[ -z "$DESTINATION" || "$DESTINATION" == "/" ]]; then
  echo "refusing unsafe asset destination: $DESTINATION" >&2
  exit 2
fi

ENTITY_SOURCE="$ROOT/apps/web/public/bevy-entity-atlases"
MAP_ATLAS_SOURCE="$ROOT/apps/web/public/generated/map-atlas"
MAP_PACK_SOURCE="$ROOT/apps/web/lib/generated/crystal-map-pack"
FALLBACK_UI_SOURCE="$ROOT/apps/web/public/original-ui"

for required in \
  "$ENTITY_SOURCE/manifest.json" \
  "$MAP_ATLAS_SOURCE/manifest.json" \
  "$MAP_PACK_SOURCE/0.map.gz" \
  "$FALLBACK_UI_SOURCE/AArmour/00/0.png" \
  "$FALLBACK_UI_SOURCE/Monster/000/0.png" \
  "$FALLBACK_UI_SOURCE/NPC/00/0.png"; do
  if [[ ! -f "$required" ]]; then
    echo "missing required native asset: $required" >&2
    exit 1
  fi
done

mkdir -p \
  "$DESTINATION/bevy-entity-atlases" \
  "$DESTINATION/generated/map-atlas" \
  "$DESTINATION/crystal-map-pack" \
  "$DESTINATION/original-ui/AArmour/00" \
  "$DESTINATION/original-ui/Monster/000" \
  "$DESTINATION/original-ui/NPC/00"

cp -R "$ENTITY_SOURCE/." "$DESTINATION/bevy-entity-atlases/"
cp -R "$MAP_ATLAS_SOURCE/." "$DESTINATION/generated/map-atlas/"
cp -R "$MAP_PACK_SOURCE/." "$DESTINATION/crystal-map-pack/"
cp -R "$FALLBACK_UI_SOURCE/AArmour/00/." "$DESTINATION/original-ui/AArmour/00/"
cp -R "$FALLBACK_UI_SOURCE/Monster/000/." "$DESTINATION/original-ui/Monster/000/"
cp -R "$FALLBACK_UI_SOURCE/NPC/00/." "$DESTINATION/original-ui/NPC/00/"

test -f "$DESTINATION/bevy-entity-atlases/manifest.json"
test -f "$DESTINATION/generated/map-atlas/manifest.json"
test -f "$DESTINATION/crystal-map-pack/0.map.gz"
test -f "$DESTINATION/original-ui/AArmour/00/0.png"
test -f "$DESTINATION/original-ui/Monster/000/0.png"
test -f "$DESTINATION/original-ui/NPC/00/0.png"

echo "[platform-windows] packaged native assets: $DESTINATION"
