#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PACKAGE="mir2-gateway"
BIN="mir2-gateway"
TOOLCHAIN="${MIR2_RUST_TOOLCHAIN:-1.89.0}"
TARGET="${MIR2_RELEASE_TARGET:-}"
OUT_DIR="${MIR2_RELEASE_OUT_DIR:-$ROOT/dist/gateway-releases}"
TAG="${MIR2_RELEASE_TAG:-}"

if [ -z "$TAG" ]; then
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    revision="$(git rev-parse --short HEAD)"
  else
    revision="nogit"
  fi
  TAG="${timestamp}-${revision}"
fi

if [ -n "$TARGET" ] && command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET"
fi

build_args=(+"$TOOLCHAIN" build --locked --release -p "$PACKAGE" --bin "$BIN")
if [ -n "$TARGET" ]; then
  build_args+=(--target "$TARGET")
  target_slug="$TARGET"
  binary_dir="$ROOT/target/$TARGET/release"
else
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os" in
    darwin) os="darwin" ;;
    linux) os="linux" ;;
  esac
  case "$arch" in
    x86_64|amd64) arch="x64" ;;
    aarch64|arm64) arch="arm64" ;;
  esac
  target_slug="${os}-${arch}"
  binary_dir="$ROOT/target/release"
fi

cargo "${build_args[@]}"

binary_path="$binary_dir/$BIN"
if [ ! -x "$binary_path" ]; then
  echo "missing release binary: $binary_path" >&2
  exit 1
fi

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

size_bytes="$(wc -c < "$binary_path" | tr -d '[:space:]')"
binary_sha256="$(sha256_file "$binary_path")"
built_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
git_revision="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
git_dirty="null"
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if git diff --quiet --ignore-submodules -- && git diff --cached --quiet --ignore-submodules --; then
    git_dirty="false"
  else
    git_dirty="true"
  fi
fi
if [ "${MIR2_RELEASE_REQUIRE_CLEAN:-0}" = "1" ] && [ "$git_dirty" != "false" ]; then
  echo "refusing to package a production Gateway from a dirty source tree" >&2
  exit 1
fi

stage="$(mktemp -d "${TMPDIR:-/tmp}/mir2-gateway-release.XXXXXX")"
trap 'rm -rf "$stage"' EXIT

install -m 0755 "$binary_path" "$stage/$BIN"
mkdir -p "$stage/systemd" "$stage/scripts"
install -m 0644 "$ROOT/infra/systemd/mir2-gateway.service" \
  "$stage/systemd/mir2-gateway.service"
install -m 0644 "$ROOT/infra/systemd/mir2-gateway.env.example" \
  "$stage/systemd/mir2-gateway.env.example"
install -m 0755 "$ROOT/scripts/install-gateway-release.sh" \
  "$stage/scripts/install-gateway-release.sh"

cat > "$stage/RELEASE.json" <<JSON
{
  "name": "mir2-gateway",
  "tag": "$TAG",
  "target": "$target_slug",
  "builtAt": "$built_at",
  "gitRevision": "$git_revision",
  "gitDirty": $git_dirty,
  "binarySizeBytes": $size_bytes,
  "binarySha256": "$binary_sha256"
}
JSON

cat > "$stage/README.txt" <<'TXT'
Mir2 Gateway release package.

Install layout:
  /opt/mir2/gateway/releases/<tag>/mir2-gateway
  /opt/mir2/gateway/current -> /opt/mir2/gateway/releases/<tag>
  /etc/mir2/gateway.env
  /var/lib/mir2

Run with systemd using infra/systemd/mir2-gateway.service from the repository.
TXT

mkdir -p "$OUT_DIR"
archive_name="mir2-gateway-${target_slug}-${TAG}.tar.gz"
archive_path="$OUT_DIR/$archive_name"
tar -C "$stage" -czf "$archive_path" \
  "$BIN" RELEASE.json README.txt systemd scripts

archive_sha256="$(sha256_file "$archive_path")"
printf '%s  %s\n' "$archive_sha256" "$archive_name" > "$archive_path.sha256"

cat > "$OUT_DIR/latest-mir2-gateway-release.json" <<JSON
{
  "name": "mir2-gateway",
  "tag": "$TAG",
  "target": "$target_slug",
  "builtAt": "$built_at",
  "archive": "$archive_name",
  "archiveSha256": "$archive_sha256",
  "binarySizeBytes": $size_bytes,
  "binarySha256": "$binary_sha256",
  "gitRevision": "$git_revision",
  "gitDirty": $git_dirty
}
JSON

archive_size_bytes="$(wc -c < "$archive_path" | tr -d '[:space:]')"

echo "Gateway release package written:"
echo "  archive: $archive_path"
echo "  archive bytes: $archive_size_bytes"
echo "  archive sha256: $archive_sha256"
echo "  binary bytes: $size_bytes"
echo "  binary sha256: $binary_sha256"
