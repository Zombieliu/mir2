#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${MIR2_CRYSTAL_MAP_OUT_DIR:-$ROOT/dist/crystal-map-assets}"
TAG="${MIR2_CRYSTAL_MAP_RELEASE_TAG:-}"

usage() {
  cat <<'TXT'
Usage:
  bash scripts/package-crystal-map-assets.sh

Optional:
  MIR2_CRYSTAL_CLIENT_ROOT       Crystal client root containing Map/*.map.
  CRYSTAL_CLIENT_ROOT            Fallback Crystal client root.
  MIR2_CRYSTAL_MAP_OUT_DIR       Output directory. Default: dist/crystal-map-assets.
  MIR2_CRYSTAL_MAP_RELEASE_TAG   Release tag. Default: UTC timestamp plus git short SHA.
TXT
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

if [ -z "$TAG" ]; then
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  if git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    revision="$(git -C "$ROOT" rev-parse --short HEAD)"
  else
    revision="nogit"
  fi
  TAG="${timestamp}-${revision}"
fi

candidate_roots=()
if [ -n "${MIR2_CRYSTAL_CLIENT_ROOT:-}" ]; then
  candidate_roots+=("$MIR2_CRYSTAL_CLIENT_ROOT")
fi
if [ -n "${CRYSTAL_CLIENT_ROOT:-}" ]; then
  candidate_roots+=("$CRYSTAL_CLIENT_ROOT")
fi
candidate_roots+=(
  "$ROOT/../downloads/crystal-client-full"
  "$ROOT/downloads/crystal-client-full"
)

client_root=""
for candidate in "${candidate_roots[@]}"; do
  if [ -d "$candidate/Map" ] && [ -n "$(find "$candidate/Map" -maxdepth 1 -type f \( -name '*.map' -o -name '*.MAP' \) -print -quit)" ]; then
    client_root="$(cd "$candidate" && pwd)"
    break
  fi
done

if [ -z "$client_root" ]; then
  echo "Could not find Crystal client Map/*.map. Set MIR2_CRYSTAL_CLIENT_ROOT or CRYSTAL_CLIENT_ROOT." >&2
  exit 1
fi

map_dir="$client_root/Map"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

map_file_count="$(find "$map_dir" -maxdepth 1 -type f \( -name '*.map' -o -name '*.MAP' \) | wc -l | tr -d '[:space:]')"
total_bytes="0"
while IFS= read -r file; do
  size="$(wc -c < "$file" | tr -d '[:space:]')"
  total_bytes="$((total_bytes + size))"
done < <(find "$map_dir" -maxdepth 1 -type f \( -name '*.map' -o -name '*.MAP' \) | sort)

built_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
git_revision="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
git_dirty="null"
if git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if git -C "$ROOT" diff --quiet --ignore-submodules -- && git -C "$ROOT" diff --cached --quiet --ignore-submodules --; then
    git_dirty="false"
  else
    git_dirty="true"
  fi
fi

stage="$(mktemp -d "${TMPDIR:-/tmp}/mir2-crystal-map-assets.XXXXXX")"
trap 'rm -rf "$stage"' EXIT

mkdir -p "$stage/scripts"
install -m 0755 "$ROOT/scripts/install-crystal-map-assets.sh" \
  "$stage/scripts/install-crystal-map-assets.sh"

cat > "$stage/RELEASE.json" <<JSON
{
  "name": "mir2-crystal-map-assets",
  "tag": "$TAG",
  "builtAt": "$built_at",
  "gitRevision": "$git_revision",
  "gitDirty": $git_dirty,
  "sourceRoot": "$client_root",
  "mapDirectory": "$map_dir",
  "mapFileCount": $map_file_count,
  "mapTotalBytes": $total_bytes,
  "layout": "Map/*.map"
}
JSON

cat > "$stage/README.txt" <<'TXT'
Mir2 Crystal map asset package.

This package contains Crystal client Map/*.map files for server-side movement
and collision lookup. It intentionally does not include Data/Map artwork files;
Player Web visual assets are published separately to R2/CDN.

Expected install layout:
  /var/lib/mir2/crystal-client/current/Map/*.map

Set the gateway environment to:
  CRYSTAL_CLIENT_ROOT=/var/lib/mir2/crystal-client/current
TXT

mkdir -p "$OUT_DIR"
archive_name="mir2-crystal-map-assets-${TAG}.tar.gz"
archive_path="$OUT_DIR/$archive_name"
tar -C "$client_root" -czf "$archive_path" Map \
  -C "$stage" RELEASE.json README.txt scripts

archive_sha256="$(sha256_file "$archive_path")"
printf '%s  %s\n' "$archive_sha256" "$archive_name" > "$archive_path.sha256"

cat > "$OUT_DIR/latest-crystal-map-assets-release.json" <<JSON
{
  "name": "mir2-crystal-map-assets",
  "tag": "$TAG",
  "builtAt": "$built_at",
  "archive": "$archive_name",
  "archiveSha256": "$archive_sha256",
  "mapFileCount": $map_file_count,
  "mapTotalBytes": $total_bytes,
  "gitRevision": "$git_revision",
  "gitDirty": $git_dirty
}
JSON

archive_size_bytes="$(wc -c < "$archive_path" | tr -d '[:space:]')"

echo "Crystal map asset package written:"
echo "  source: $client_root"
echo "  archive: $archive_path"
echo "  archive bytes: $archive_size_bytes"
echo "  archive sha256: $archive_sha256"
echo "  map files: $map_file_count"
echo "  map bytes: $total_bytes"
