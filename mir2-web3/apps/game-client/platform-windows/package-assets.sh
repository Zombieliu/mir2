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
NATIVE_KEYED_MAP_SOURCE="$ROOT/apps/web/public/generated/native-map-keyed"
MAP_PACK_SOURCE="$ROOT/apps/web/lib/generated/crystal-map-pack"
FALLBACK_UI_SOURCE="$ROOT/apps/web/public/original-ui"

# The keyed pages are deterministic derivatives of tracked map sources.  Build
# them at the packaging boundary so a clean checkout never depends on a stale
# developer-machine output tree.
npm --prefix "$ROOT/apps/web" run assets:native-map-keyed:build

for required in \
  "$ENTITY_SOURCE/manifest.json" \
  "$MAP_ATLAS_SOURCE/manifest.json" \
  "$NATIVE_KEYED_MAP_SOURCE/manifest.json" \
  "$MAP_PACK_SOURCE/0.map.gz" \
  "$FALLBACK_UI_SOURCE/ChrSel/0.png" \
  "$FALLBACK_UI_SOURCE/ChrSel/20.png" \
  "$FALLBACK_UI_SOURCE/ChrSel/375.png" \
  "$FALLBACK_UI_SOURCE/ChrSel/600.png" \
  "$FALLBACK_UI_SOURCE/ChrSel/895.png" \
  "$FALLBACK_UI_SOURCE/MMap/101.png" \
  "$FALLBACK_UI_SOURCE/Prguse/1080.png" \
  "$FALLBACK_UI_SOURCE/Prguse/1081.png" \
  "$FALLBACK_UI_SOURCE/Prguse/1082.png" \
  "$FALLBACK_UI_SOURCE/Prguse/1083.png" \
  "$FALLBACK_UI_SOURCE/Prguse/1084.png" \
  "$FALLBACK_UI_SOURCE/UI_32bit/472.png" \
  "$FALLBACK_UI_SOURCE/UI_32bit/473.png" \
  "$FALLBACK_UI_SOURCE/Title/30.png" \
  "$FALLBACK_UI_SOURCE/Title/300.png" \
  "$FALLBACK_UI_SOURCE/Title/301.png" \
  "$FALLBACK_UI_SOURCE/Title/302.png" \
  "$FALLBACK_UI_SOURCE/Title/303.png" \
  "$FALLBACK_UI_SOURCE/Title/304.png" \
  "$FALLBACK_UI_SOURCE/Title/305.png" \
  "$FALLBACK_UI_SOURCE/Title/306.png" \
  "$FALLBACK_UI_SOURCE/Title/307.png" \
  "$FALLBACK_UI_SOURCE/Title/308.png" \
  "$FALLBACK_UI_SOURCE/Title/309.png" \
  "$FALLBACK_UI_SOURCE/Title/310.png" \
  "$FALLBACK_UI_SOURCE/Title/311.png" \
  "$FALLBACK_UI_SOURCE/Title/320.png" \
  "$FALLBACK_UI_SOURCE/Title/334.png" \
  "$FALLBACK_UI_SOURCE/AArmour/00/0.png" \
  "$FALLBACK_UI_SOURCE/Monster/000/0.png" \
  "$FALLBACK_UI_SOURCE/NPC/00/0.png"; do
  if [[ ! -f "$required" || ! -s "$required" ]]; then
    echo "missing or empty required native asset: $required" >&2
    exit 1
  fi
done

for required in \
  "$FALLBACK_UI_SOURCE/Sound/Login2.wav" \
  "$FALLBACK_UI_SOURCE/Sound/Select2.wav"; do
  if [[ ! -f "$required" || ! -s "$required" ]]; then
    echo "missing or empty required native audio asset: $required" >&2
    exit 1
  fi
done

mkdir -p \
  "$DESTINATION/bevy-entity-atlases" \
  "$DESTINATION/generated/map-atlas" \
  "$DESTINATION/generated/native-map-keyed" \
  "$DESTINATION/crystal-map-pack" \
  "$DESTINATION/original-ui/ChrSel" \
  "$DESTINATION/original-ui/MMap" \
  "$DESTINATION/original-ui/Prguse" \
  "$DESTINATION/original-ui/UI_32bit" \
  "$DESTINATION/original-ui/Title" \
  "$DESTINATION/original-ui/AArmour/00" \
  "$DESTINATION/original-ui/Monster/000" \
  "$DESTINATION/original-ui/NPC/00" \
  "$DESTINATION/original-ui/Sound"

cp -R "$ENTITY_SOURCE/." "$DESTINATION/bevy-entity-atlases/"
cp -R "$MAP_ATLAS_SOURCE/." "$DESTINATION/generated/map-atlas/"
cp -R "$NATIVE_KEYED_MAP_SOURCE/." "$DESTINATION/generated/native-map-keyed/"
cp -R "$MAP_PACK_SOURCE/." "$DESTINATION/crystal-map-pack/"
cp -R "$FALLBACK_UI_SOURCE/ChrSel/." "$DESTINATION/original-ui/ChrSel/"
cp -R "$FALLBACK_UI_SOURCE/MMap/." "$DESTINATION/original-ui/MMap/"
cp -R "$FALLBACK_UI_SOURCE/Prguse/." "$DESTINATION/original-ui/Prguse/"
cp -R "$FALLBACK_UI_SOURCE/UI_32bit/." "$DESTINATION/original-ui/UI_32bit/"
cp -R "$FALLBACK_UI_SOURCE/Title/." "$DESTINATION/original-ui/Title/"
cp -R "$FALLBACK_UI_SOURCE/AArmour/00/." "$DESTINATION/original-ui/AArmour/00/"
cp -R "$FALLBACK_UI_SOURCE/Monster/000/." "$DESTINATION/original-ui/Monster/000/"
cp -R "$FALLBACK_UI_SOURCE/NPC/00/." "$DESTINATION/original-ui/NPC/00/"
cp "$FALLBACK_UI_SOURCE/Sound/Login2.wav" "$DESTINATION/original-ui/Sound/Login2.wav"
cp "$FALLBACK_UI_SOURCE/Sound/Select2.wav" "$DESTINATION/original-ui/Sound/Select2.wav"

test -f "$DESTINATION/bevy-entity-atlases/manifest.json"
test -f "$DESTINATION/generated/map-atlas/manifest.json"
test -f "$DESTINATION/generated/native-map-keyed/manifest.json"
test -f "$DESTINATION/crystal-map-pack/0.map.gz"
test -f "$DESTINATION/original-ui/ChrSel/0.png"
test -f "$DESTINATION/original-ui/ChrSel/20.png"
test -f "$DESTINATION/original-ui/ChrSel/375.png"
test -f "$DESTINATION/original-ui/ChrSel/600.png"
test -f "$DESTINATION/original-ui/ChrSel/895.png"
test -f "$DESTINATION/original-ui/MMap/101.png"
test -s "$DESTINATION/original-ui/Prguse/1080.png"
test -s "$DESTINATION/original-ui/Prguse/1081.png"
test -s "$DESTINATION/original-ui/Prguse/1082.png"
test -s "$DESTINATION/original-ui/Prguse/1083.png"
test -f "$DESTINATION/original-ui/Prguse/1084.png"
test -f "$DESTINATION/original-ui/UI_32bit/472.png"
test -f "$DESTINATION/original-ui/UI_32bit/473.png"
test -f "$DESTINATION/original-ui/Title/30.png"
test -s "$DESTINATION/original-ui/Title/300.png"
test -s "$DESTINATION/original-ui/Title/301.png"
test -s "$DESTINATION/original-ui/Title/302.png"
test -s "$DESTINATION/original-ui/Title/303.png"
test -s "$DESTINATION/original-ui/Title/304.png"
test -s "$DESTINATION/original-ui/Title/305.png"
test -s "$DESTINATION/original-ui/Title/306.png"
test -s "$DESTINATION/original-ui/Title/307.png"
test -s "$DESTINATION/original-ui/Title/308.png"
test -s "$DESTINATION/original-ui/Title/309.png"
test -s "$DESTINATION/original-ui/Title/310.png"
test -s "$DESTINATION/original-ui/Title/311.png"
test -f "$DESTINATION/original-ui/Title/320.png"
test -f "$DESTINATION/original-ui/Title/334.png"
test -f "$DESTINATION/original-ui/AArmour/00/0.png"
test -f "$DESTINATION/original-ui/Monster/000/0.png"
test -f "$DESTINATION/original-ui/NPC/00/0.png"
test -s "$DESTINATION/original-ui/Sound/Login2.wav"
test -s "$DESTINATION/original-ui/Sound/Select2.wav"

echo "[platform-windows] packaged native assets: $DESTINATION"
