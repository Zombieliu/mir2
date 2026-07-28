#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Install the pinned Mir2 developer asset bundle.

Usage:
  scripts/install-developer-assets.sh [options]

Options:
  --manifest-path PATH     Bundle manifest (default: config/developer-assets.json)
  --parts-directory PATH   Directory containing pre-downloaded archive parts
  --cache-directory PATH   Download and reassembly cache directory
  --download               Download missing or invalid parts with GitHub CLI
  --force                  Accepted for compatibility; verified upgrades are automatic
  --keep-archive           Keep the reassembled archive after a successful install
  -h, --help               Show this help
EOF
}

die() {
  printf 'Developer asset install failed: %s\n' "$*" >&2
  exit 1
}

warn() {
  printf 'WARNING: %s\n' "$*" >&2
}

find_node() {
  if command -v node >/dev/null 2>&1; then
    command -v node
  elif command -v nodejs >/dev/null 2>&1; then
    command -v nodejs
  else
    return 1
  fi
}

canonical_file() {
  local input_path="$1"
  local input_dir
  local input_name

  [ -f "$input_path" ] || return 1
  input_dir="$(dirname "$input_path")"
  input_name="$(basename "$input_path")"
  input_dir="$(cd "$input_dir" && pwd -P)"
  printf '%s/%s\n' "$input_dir" "$input_name"
}

canonical_directory() {
  local input_path="$1"

  [ -d "$input_path" ] || return 1
  (cd "$input_path" && pwd -P)
}

canonical_future_path() {
  local resolved_path

  resolved_path="$("$NODE_BIN" - "$1" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

let current = path.resolve(process.argv[2]);
const missing = [];
while (!fs.existsSync(current)) {
  const parent = path.dirname(current);
  if (parent === current) {
    throw new Error(`Unable to resolve path: ${process.argv[2]}`);
  }
  missing.unshift(path.basename(current));
  current = parent;
}
process.stdout.write(path.join(fs.realpathSync(current), ...missing));
NODE
  )" || return 1
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -u "$resolved_path"
  else
    printf '%s\n' "$resolved_path"
  fi
}

paths_overlap() {
  local first_path="$1"
  local second_path="$2"

  case "$first_path" in
    "$second_path"|"$second_path"/*) return 0 ;;
  esac
  case "$second_path" in
    "$first_path"/*) return 0 ;;
  esac
  return 1
}

assert_safe_child_path() {
  local child_path="$1"
  local parent_path="$2"
  local label="$3"

  case "$child_path" in
    "$parent_path"/*) ;;
    *) die "$label is outside the expected parent directory: $child_path" ;;
  esac
}

file_size() {
  LC_ALL=C wc -c < "$1" | tr -d '[:space:]'
}

sha256_file() {
  if [ "$SHA256_TOOL" = "sha256sum" ]; then
    sha256sum "$1" | awk '{ print tolower($1) }'
  else
    shasum -a 256 "$1" | awk '{ print tolower($1) }'
  fi
}

test_asset_part() {
  local part_path="$1"
  local expected_size="$2"
  local expected_sha256="$3"
  local actual_size
  local actual_sha256

  [ -f "$part_path" ] || return 1
  actual_size="$(file_size "$part_path")"
  [ "$actual_size" = "$expected_size" ] || return 1
  actual_sha256="$(sha256_file "$part_path")"
  [ "$actual_sha256" = "$expected_sha256" ]
}

read_index_content_hash() {
  "$NODE_BIN" -e '
    const fs = require("node:fs");
    const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(String(value.contentHash ?? ""));
  ' "$1"
}

verify_pack_directory() {
  local pack_root="$1"
  local pack_index="$pack_root/index.json"
  local pack_hash

  [ -f "$pack_index" ] || return 1
  pack_hash="$(read_index_content_hash "$pack_index" 2>/dev/null)" || return 1
  case "$pack_hash" in
    *[!a-f0-9]*|"") return 1 ;;
  esac
  [ "${#pack_hash}" -eq 64 ] || return 1
  "$NODE_BIN" "$CLOSURE_VERIFIER" \
    --root "$pack_root" \
    --expectedContentHash "$pack_hash" \
    --verifyPages true >/dev/null 2>&1
}

read_lock_field() {
  local field_name="$1"
  local owner_file="$LOCK_DIRECTORY/owner"

  [ -f "$owner_file" ] || return 1
  sed -n "s/^${field_name}=//p" "$owner_file" | sed -n '1p'
}

lock_age_seconds() {
  local age_path="$LOCK_DIRECTORY/heartbeat"

  if [ ! -e "$age_path" ]; then
    age_path="$LOCK_DIRECTORY"
  fi
  "$NODE_BIN" -e '
    const fs = require("node:fs");
    const age = Math.max(0, Math.floor((Date.now() - fs.statSync(process.argv[1]).mtimeMs) / 1000));
    process.stdout.write(String(age));
  ' "$age_path" 2>/dev/null
}

lock_is_live() {
  local owner_host
  local owner_pid
  local age

  owner_host="$(read_lock_field host 2>/dev/null || true)"
  owner_pid="$(read_lock_field pid 2>/dev/null || true)"
  if [ "$owner_host" = "$LOCK_HOST" ]; then
    case "$owner_pid" in
      *[!0-9]*|"") ;;
      *)
        if kill -0 "$owner_pid" 2>/dev/null; then
          return 0
        fi
        return 1
        ;;
    esac
  fi

  age="$(lock_age_seconds 2>/dev/null || true)"
  case "$age" in
    *[!0-9]*|"") return 0 ;;
  esac
  [ "$age" -lt "$LOCK_STALE_SECONDS" ]
}

release_install_lock() {
  local owner_token

  [ "$LOCK_HELD" -eq 1 ] || return 0
  owner_token="$(read_lock_field token 2>/dev/null || true)"
  if [ "$owner_token" = "$LOCK_TOKEN" ]; then
    case "$LOCK_DIRECTORY" in
      "$DESTINATION_PARENT"/.full-install.lock) rm -rf "$LOCK_DIRECTORY" ;;
    esac
  fi
  LOCK_HELD=0
}

lock_heartbeat() {
  local owner_token

  while [ -d "$LOCK_DIRECTORY" ]; do
    owner_token="$(read_lock_field token 2>/dev/null || true)"
    [ "$owner_token" = "$LOCK_TOKEN" ] || return 0
    touch "$LOCK_DIRECTORY/heartbeat" 2>/dev/null || return 0
    sleep 2
  done
}

acquire_install_lock() {
  local started_at
  local now
  local stale_directory
  local announced_wait=0

  started_at="$(date +%s)"
  while ! mkdir "$LOCK_DIRECTORY" 2>/dev/null; do
    if [ -d "$LOCK_DIRECTORY" ] && ! lock_is_live; then
      stale_directory="$DESTINATION_PARENT/.full-install-lock-stale-$INSTALL_ID"
      assert_safe_child_path "$stale_directory" "$DESTINATION_PARENT" "Stale install lock"
      if mv "$LOCK_DIRECTORY" "$stale_directory" 2>/dev/null; then
        rm -rf "$stale_directory"
        warn "Recovered a stale developer asset install lock."
        continue
      fi
    fi

    now="$(date +%s)"
    if [ "$LOCK_WAIT_SECONDS" -eq 0 ] ||
       [ $((now - started_at)) -ge "$LOCK_WAIT_SECONDS" ]; then
      die "Another developer asset installation holds the lock: $LOCK_DIRECTORY"
    fi
    if [ "$announced_wait" -eq 0 ]; then
      printf '[assets] waiting for another installer to finish...\n'
      announced_wait=1
    fi
    sleep 1
  done

  LOCK_HELD=1
  {
    printf 'pid=%s\n' "$$"
    printf 'host=%s\n' "$LOCK_HOST"
    printf 'token=%s\n' "$LOCK_TOKEN"
  } > "$LOCK_DIRECTORY/owner"
  touch "$LOCK_DIRECTORY/heartbeat"
  lock_heartbeat &
  LOCK_HEARTBEAT_PID="$!"
}

remove_stale_staging_directories() {
  local stale_path

  for stale_path in "$PUBLIC_ROOT"/.mir2-asset-install-*; do
    if [ -e "$stale_path" ] || [ -L "$stale_path" ]; then
      assert_safe_child_path "$stale_path" "$PUBLIC_ROOT" "Stale asset staging directory"
      rm -rf "$stale_path"
      warn "Removed interrupted developer asset staging directory: $stale_path"
    fi
  done
}

remove_all_backup_directories() {
  local backup_path

  for backup_path in "$DESTINATION_PARENT"/.full-backup-*; do
    if [ -e "$backup_path" ] || [ -L "$backup_path" ]; then
      assert_safe_child_path "$backup_path" "$DESTINATION_PARENT" "Asset backup directory"
      rm -rf "$backup_path"
    fi
  done
}

recover_interrupted_install() {
  local backup_path
  local selected_backup=""
  local first_backup=""
  local has_backup=0

  for backup_path in "$DESTINATION_PARENT"/.full-backup-*; do
    if [ -e "$backup_path" ] || [ -L "$backup_path" ]; then
      assert_safe_child_path "$backup_path" "$DESTINATION_PARENT" "Asset backup directory"
      has_backup=1
      if [ -z "$first_backup" ]; then
        first_backup="$backup_path"
      fi
      if verify_pack_directory "$backup_path"; then
        selected_backup="$backup_path"
        break
      fi
    fi
  done

  if [ "$has_backup" -eq 1 ] &&
     [ ! -e "$EXPECTED_DESTINATION" ] &&
     [ ! -L "$EXPECTED_DESTINATION" ]; then
    if [ -z "$selected_backup" ]; then
      selected_backup="$first_backup"
    fi
    warn "Restoring the full Crystal pack from an interrupted install backup."
    mv "$selected_backup" "$EXPECTED_DESTINATION" ||
      die "Unable to restore interrupted developer asset install backup."
  fi

  if [ "$has_backup" -eq 1 ] &&
     { [ -e "$EXPECTED_DESTINATION" ] || [ -L "$EXPECTED_DESTINATION" ]; } &&
     verify_pack_directory "$EXPECTED_DESTINATION"; then
    remove_all_backup_directories
  fi

  remove_stale_staging_directories
}

MANIFEST_PATH=""
PARTS_DIRECTORY=""
CACHE_DIRECTORY=""
DOWNLOAD=0
FORCE=0
KEEP_ARCHIVE=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --manifest-path)
      [ "$#" -ge 2 ] || die "--manifest-path requires a value"
      MANIFEST_PATH="$2"
      shift 2
      ;;
    --manifest-path=*)
      MANIFEST_PATH="${1#*=}"
      shift
      ;;
    --parts-directory)
      [ "$#" -ge 2 ] || die "--parts-directory requires a value"
      PARTS_DIRECTORY="$2"
      shift 2
      ;;
    --parts-directory=*)
      PARTS_DIRECTORY="${1#*=}"
      shift
      ;;
    --cache-directory)
      [ "$#" -ge 2 ] || die "--cache-directory requires a value"
      CACHE_DIRECTORY="$2"
      shift 2
      ;;
    --cache-directory=*)
      CACHE_DIRECTORY="${1#*=}"
      shift
      ;;
    --download)
      DOWNLOAD=1
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --keep-archive)
      KEEP_ARCHIVE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
WEB_ROOT="$PROJECT_ROOT/apps/web"
PUBLIC_ROOT="$WEB_ROOT/public"
ARCHIVE_INSPECTOR="$WEB_ROOT/scripts/asset-pipeline/inspect-ustar-archive.mjs"
CLOSURE_VERIFIER="$WEB_ROOT/scripts/asset-pipeline/verify-full-pack-closure.mjs"
DESTINATION_PARENT=""
EXPECTED_DESTINATION=""
DESTINATION=""

NODE_BIN="$(find_node)" || die "Node.js is required to validate and install the developer asset bundle."
[ -f "$ARCHIVE_INSPECTOR" ] || die "Archive inspector is missing: $ARCHIVE_INSPECTOR"
[ -f "$CLOSURE_VERIFIER" ] || die "Closure verifier is missing: $CLOSURE_VERIFIER"
command -v tar >/dev/null 2>&1 || die "tar is required to extract the developer asset bundle."
mkdir -p "$PUBLIC_ROOT"
PUBLIC_ROOT="$(canonical_directory "$PUBLIC_ROOT")" ||
  die "Unable to resolve the web public directory: $PUBLIC_ROOT"

case "${MIR2_DEVELOPER_ASSET_SHA256_TOOL:-auto}" in
  auto)
    if command -v sha256sum >/dev/null 2>&1; then
      SHA256_TOOL="sha256sum"
    elif command -v shasum >/dev/null 2>&1; then
      SHA256_TOOL="shasum"
    else
      die "SHA-256 verification requires sha256sum (Linux) or shasum (macOS)."
    fi
    ;;
  sha256sum)
    command -v sha256sum >/dev/null 2>&1 ||
      die "MIR2_DEVELOPER_ASSET_SHA256_TOOL requests sha256sum, but it is unavailable."
    SHA256_TOOL="sha256sum"
    ;;
  shasum)
    command -v shasum >/dev/null 2>&1 ||
      die "MIR2_DEVELOPER_ASSET_SHA256_TOOL requests shasum, but it is unavailable."
    SHA256_TOOL="shasum"
    ;;
  *)
    die "MIR2_DEVELOPER_ASSET_SHA256_TOOL must be auto, sha256sum, or shasum."
    ;;
esac

if [ -z "$MANIFEST_PATH" ]; then
  MANIFEST_PATH="$PROJECT_ROOT/config/developer-assets.json"
fi
MANIFEST_PATH="$(canonical_file "$MANIFEST_PATH")" ||
  die "Developer asset manifest is missing: $MANIFEST_PATH"

METADATA_FILE="$(mktemp "${TMPDIR:-/tmp}/mir2-developer-assets.XXXXXX")"
STAGING_ROOT=""
BACKUP_DESTINATION=""
NEW_DESTINATION_INSTALLED=0
INSTALL_COMMITTED=0
LOCK_DIRECTORY=""
LOCK_HELD=0
LOCK_HEARTBEAT_PID=""
LOCK_TOKEN=""
LOCK_HOST=""
LOCK_WAIT_SECONDS="${MIR2_DEVELOPER_ASSET_LOCK_WAIT_SECONDS:-1800}"
LOCK_STALE_SECONDS="${MIR2_DEVELOPER_ASSET_LOCK_STALE_SECONDS:-30}"
INSTALL_ID=""

cleanup() {
  local status="$?"
  trap - EXIT
  set +e

  if [ "$INSTALL_COMMITTED" -ne 1 ]; then
    if [ -n "$BACKUP_DESTINATION" ] &&
       { [ -e "$BACKUP_DESTINATION" ] || [ -L "$BACKUP_DESTINATION" ]; }; then
      if [ -e "$EXPECTED_DESTINATION" ] || [ -L "$EXPECTED_DESTINATION" ]; then
        rm -rf "$EXPECTED_DESTINATION"
      fi
      mv "$BACKUP_DESTINATION" "$EXPECTED_DESTINATION"
    elif [ "$NEW_DESTINATION_INSTALLED" -eq 1 ] &&
         { [ -e "$EXPECTED_DESTINATION" ] || [ -L "$EXPECTED_DESTINATION" ]; }; then
      rm -rf "$EXPECTED_DESTINATION"
    fi
  fi

  if [ -n "$STAGING_ROOT" ]; then
    case "$STAGING_ROOT" in
      "$PUBLIC_ROOT"/.mir2-asset-install-*) rm -rf "$STAGING_ROOT" ;;
    esac
  fi
  if [ -n "$LOCK_HEARTBEAT_PID" ]; then
    kill "$LOCK_HEARTBEAT_PID" 2>/dev/null
    wait "$LOCK_HEARTBEAT_PID" 2>/dev/null
  fi
  release_install_lock
  rm -f "$METADATA_FILE"
  exit "$status"
}

trap 'exit 130' HUP INT TERM
trap cleanup EXIT

"$NODE_BIN" - "$MANIFEST_PATH" > "$METADATA_FILE" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

const manifestPath = process.argv[2];

function fail(message) {
  throw new Error(message);
}

function isLowerSha256(value) {
  return typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
}

function isPositiveSafeInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function assertPlainFileName(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value === "." ||
    value === ".." ||
    value !== path.basename(value) ||
    /[\\/]/.test(value) ||
    /[\x00-\x1f\x7f]/.test(value)
  ) {
    fail(`${label} must be a plain file name: ${String(value)}`);
  }
}

let manifest;
try {
  manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
} catch (error) {
  fail(`Unable to parse developer asset manifest ${manifestPath}: ${error.message}`);
}

if (manifest.kind !== "mir2-developer-asset-bundle" || manifest.schemaVersion !== 1) {
  fail(`Unsupported developer asset manifest: ${manifestPath}`);
}
if (manifest.destination !== "mir2-web3/apps/web/public/generated/crystal-packs/full") {
  fail(`Unexpected developer asset destination in manifest: ${String(manifest.destination)}`);
}
if (!isLowerSha256(manifest.contentHash)) {
  fail("Developer asset manifest has an invalid contentHash.");
}
if (
  typeof manifest.repository !== "string" ||
  !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(manifest.repository)
) {
  fail("Developer asset manifest has an invalid GitHub repository.");
}
if (
  typeof manifest.releaseTag !== "string" ||
  !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(manifest.releaseTag)
) {
  fail("Developer asset manifest has an invalid release tag.");
}
if (
  !manifest.archive ||
  manifest.archive.format !== "ustar" ||
  !isPositiveSafeInteger(manifest.archive.size) ||
  !isLowerSha256(manifest.archive.sha256)
) {
  fail("Developer asset manifest has invalid archive metadata.");
}
assertPlainFileName(manifest.archive.name, "Archive name");
if (!Array.isArray(manifest.parts) || manifest.parts.length === 0) {
  fail("Developer asset manifest contains no archive parts.");
}

const seenPartNames = new Set();
for (const part of manifest.parts) {
  assertPlainFileName(part?.name, "Archive part name");
  if (seenPartNames.has(part.name)) {
    fail(`Developer asset manifest contains a duplicate part: ${part.name}`);
  }
  if (!isPositiveSafeInteger(part.size) || !isLowerSha256(part.sha256)) {
    fail(`Developer asset manifest contains invalid metadata for part: ${part.name}`);
  }
  seenPartNames.add(part.name);
}
if (seenPartNames.has(manifest.archive.name)) {
  fail("Archive name must not conflict with an archive part name.");
}

process.stdout.write([
  "META",
  manifest.repository,
  manifest.releaseTag,
  manifest.contentHash,
  manifest.archive.name,
  String(manifest.archive.size),
  manifest.archive.sha256,
].join("\t") + "\n");
for (const part of manifest.parts) {
  process.stdout.write([
    "PART",
    part.name,
    String(part.size),
    part.sha256,
  ].join("\t") + "\n");
}
NODE

REPOSITORY=""
RELEASE_TAG=""
CONTENT_HASH=""
ARCHIVE_NAME=""
ARCHIVE_SIZE=""
ARCHIVE_SHA256=""
PART_NAMES=()
PART_SIZES=()
PART_SHA256S=()

while IFS=$'\t' read -r record field1 field2 field3 field4 field5 field6; do
  case "$record" in
    META)
      REPOSITORY="$field1"
      RELEASE_TAG="$field2"
      CONTENT_HASH="$field3"
      ARCHIVE_NAME="$field4"
      ARCHIVE_SIZE="$field5"
      ARCHIVE_SHA256="$field6"
      ;;
    PART)
      PART_NAMES[${#PART_NAMES[@]}]="$field1"
      PART_SIZES[${#PART_SIZES[@]}]="$field2"
      PART_SHA256S[${#PART_SHA256S[@]}]="$field3"
      ;;
    *)
      die "Manifest parser produced an unexpected record: $record"
      ;;
  esac
done < "$METADATA_FILE"

[ -n "$REPOSITORY" ] && [ -n "$CONTENT_HASH" ] && [ "${#PART_NAMES[@]}" -gt 0 ] ||
  die "Developer asset manifest metadata is incomplete."

case "$LOCK_WAIT_SECONDS" in
  *[!0-9]*|"") die "MIR2_DEVELOPER_ASSET_LOCK_WAIT_SECONDS must be a non-negative integer." ;;
esac
case "$LOCK_STALE_SECONDS" in
  *[!0-9]*|"") die "MIR2_DEVELOPER_ASSET_LOCK_STALE_SECONDS must be a non-negative integer." ;;
esac

DESTINATION_PARENT="$PUBLIC_ROOT/generated/crystal-packs"
EXPECTED_DESTINATION="$DESTINATION_PARENT/full"
DESTINATION="$EXPECTED_DESTINATION"
if [ -z "$CACHE_DIRECTORY" ]; then
  CACHE_DIRECTORY="$PROJECT_ROOT/.mir2-data/developer-assets/$RELEASE_TAG"
fi

mkdir -p "$DESTINATION_PARENT"
DESTINATION_PARENT="$(canonical_directory "$DESTINATION_PARENT")" ||
  die "Unable to create developer asset destination parent: $DESTINATION_PARENT"
EXPECTED_DESTINATION="$DESTINATION_PARENT/full"
DESTINATION="$EXPECTED_DESTINATION"

INSTALL_ID="$("$NODE_BIN" -e 'process.stdout.write(require("node:crypto").randomBytes(16).toString("hex"))')"
LOCK_TOKEN="$INSTALL_ID"
LOCK_HOST="$(hostname 2>/dev/null || uname -n)"
LOCK_DIRECTORY="$DESTINATION_PARENT/.full-install.lock"
assert_safe_child_path "$LOCK_DIRECTORY" "$DESTINATION_PARENT" "Asset install lock"
acquire_install_lock
recover_interrupted_install

future_destination="$(canonical_future_path "$EXPECTED_DESTINATION")" ||
  die "Unable to resolve the developer asset destination: $EXPECTED_DESTINATION"
future_cache="$(canonical_future_path "$CACHE_DIRECTORY")" ||
  die "Unable to resolve the developer asset cache: $CACHE_DIRECTORY"
if paths_overlap "$future_cache" "$future_destination"; then
  die "Asset cache and destination directories must not overlap: $future_cache"
fi

mkdir -p "$CACHE_DIRECTORY"
CACHE_DIRECTORY="$(canonical_directory "$CACHE_DIRECTORY")" ||
  die "Unable to create developer asset cache directory: $CACHE_DIRECTORY"
[ "$CACHE_DIRECTORY" != "/" ] || die "Refusing to use the filesystem root as the asset cache."
if paths_overlap "$CACHE_DIRECTORY" "$EXPECTED_DESTINATION"; then
  die "Asset cache and destination directories must not overlap: $CACHE_DIRECTORY"
fi

EXISTING_INDEX="$DESTINATION/index.json"
if [ -f "$EXISTING_INDEX" ]; then
  if ! EXISTING_HASH="$(read_index_content_hash "$EXISTING_INDEX")"; then
    warn "Installed full Crystal pack index is invalid; it will only be replaced after bundle verification."
  elif [ "$EXISTING_HASH" = "$CONTENT_HASH" ]; then
    if "$NODE_BIN" "$CLOSURE_VERIFIER" \
      --root "$DESTINATION" \
      --expectedContentHash "$CONTENT_HASH" \
      --verifyPages true >/dev/null 2>&1; then
      printf 'Full Crystal pack is already installed and verified: %s\n' "$CONTENT_HASH"
      exit 0
    fi
    warn "The installed full Crystal pack is incomplete or corrupt; reinstalling the pinned bundle."
  else
    warn "Upgrading the full Crystal pack from $EXISTING_HASH to $CONTENT_HASH after verification."
  fi
elif [ -e "$DESTINATION" ] || [ -L "$DESTINATION" ]; then
  warn "The installed full Crystal pack has no index; it will only be replaced after bundle verification."
fi

if [ "$DOWNLOAD" -eq 1 ]; then
  command -v gh >/dev/null 2>&1 ||
    die "GitHub CLI is required for --download. Install it, run 'gh auth login', and retry."

  for ((index = 0; index < ${#PART_NAMES[@]}; index += 1)); do
    part_name="${PART_NAMES[$index]}"
    target_path="$CACHE_DIRECTORY/$part_name"
    assert_safe_child_path "$target_path" "$CACHE_DIRECTORY" "Cached archive part"

    if test_asset_part "$target_path" "${PART_SIZES[$index]}" "${PART_SHA256S[$index]}"; then
      printf '[assets] verified cached part: %s\n' "$part_name"
      continue
    fi

    if [ -e "$target_path" ] || [ -L "$target_path" ]; then
      warn "Removing incomplete or corrupt cached part: $part_name"
      rm -f "$target_path"
    fi

    printf '[assets] downloading: %s\n' "$part_name"
    if ! gh release download "$RELEASE_TAG" \
      --repo "$REPOSITORY" \
      --pattern "$part_name" \
      --dir "$CACHE_DIRECTORY"; then
      die "Failed to download $part_name from GitHub Release."
    fi
    test_asset_part "$target_path" "${PART_SIZES[$index]}" "${PART_SHA256S[$index]}" ||
      die "Downloaded asset bundle part failed size or SHA-256 verification: $target_path"
  done
  PARTS_DIRECTORY="$CACHE_DIRECTORY"
elif [ -z "$PARTS_DIRECTORY" ]; then
  PARTS_DIRECTORY="$(dirname "$MANIFEST_PATH")"
fi

PARTS_DIRECTORY="$(canonical_directory "$PARTS_DIRECTORY")" ||
  die "Archive parts directory is missing: $PARTS_DIRECTORY"

for ((index = 0; index < ${#PART_NAMES[@]}; index += 1)); do
  part_path="$PARTS_DIRECTORY/${PART_NAMES[$index]}"
  [ -f "$part_path" ] || die "Asset bundle part is missing: $part_path"

  actual_size="$(file_size "$part_path")"
  [ "$actual_size" = "${PART_SIZES[$index]}" ] ||
    die "Asset bundle part size mismatch: $part_path"

  actual_sha256="$(sha256_file "$part_path")"
  [ "$actual_sha256" = "${PART_SHA256S[$index]}" ] ||
    die "Asset bundle part hash mismatch: $part_path"
done

ARCHIVE_PATH="$CACHE_DIRECTORY/$ARCHIVE_NAME"
assert_safe_child_path "$ARCHIVE_PATH" "$CACHE_DIRECTORY" "Reassembled archive"
if [ -e "$ARCHIVE_PATH" ] || [ -L "$ARCHIVE_PATH" ]; then
  rm -f "$ARCHIVE_PATH"
fi
: > "$ARCHIVE_PATH"
for ((index = 0; index < ${#PART_NAMES[@]}; index += 1)); do
  cat "$PARTS_DIRECTORY/${PART_NAMES[$index]}" >> "$ARCHIVE_PATH"
done

actual_archive_size="$(file_size "$ARCHIVE_PATH")"
[ "$actual_archive_size" = "$ARCHIVE_SIZE" ] ||
  die "Reassembled archive size mismatch: $ARCHIVE_PATH"
actual_archive_sha256="$(sha256_file "$ARCHIVE_PATH")"
[ "$actual_archive_sha256" = "$ARCHIVE_SHA256" ] ||
  die "Reassembled archive hash mismatch: $ARCHIVE_PATH"

if ! "$NODE_BIN" "$ARCHIVE_INSPECTOR" \
  --archive "$ARCHIVE_PATH" \
  --prefix "generated/crystal-packs/full"; then
  die "Asset archive safety inspection failed."
fi

STAGING_ROOT="$PUBLIC_ROOT/.mir2-asset-install-$INSTALL_ID"
STAGED_DESTINATION="$STAGING_ROOT/generated/crystal-packs/full"
BACKUP_DESTINATION="$DESTINATION_PARENT/.full-backup-$INSTALL_ID"
assert_safe_child_path "$STAGING_ROOT" "$PUBLIC_ROOT" "Asset staging directory"
assert_safe_child_path "$BACKUP_DESTINATION" "$DESTINATION_PARENT" "Asset backup directory"
[ ! -e "$STAGING_ROOT" ] && [ ! -L "$STAGING_ROOT" ] ||
  die "Asset staging directory already exists: $STAGING_ROOT"
[ ! -e "$BACKUP_DESTINATION" ] && [ ! -L "$BACKUP_DESTINATION" ] ||
  die "Asset backup directory already exists: $BACKUP_DESTINATION"

mkdir -p "$STAGING_ROOT"
if ! tar -xf "$ARCHIVE_PATH" -C "$STAGING_ROOT"; then
  die "Asset extraction failed."
fi

STAGED_INDEX="$STAGED_DESTINATION/index.json"
[ -f "$STAGED_INDEX" ] ||
  die "Extracted asset bundle does not contain its full-pack index."
if ! STAGED_CONTENT_HASH="$(read_index_content_hash "$STAGED_INDEX")"; then
  die "Extracted full Crystal pack index is not valid JSON."
fi
[ "$STAGED_CONTENT_HASH" = "$CONTENT_HASH" ] ||
  die "Extracted full Crystal pack content hash does not match the bundle manifest."

if ! "$NODE_BIN" "$CLOSURE_VERIFIER" \
  --root "$STAGED_DESTINATION" \
  --expectedContentHash "$CONTENT_HASH" \
  --verifyPages true; then
  die "Extracted full Crystal pack closure verification failed."
fi

mkdir -p "$(dirname "$EXPECTED_DESTINATION")"
if [ -e "$EXPECTED_DESTINATION" ] || [ -L "$EXPECTED_DESTINATION" ]; then
  mv "$EXPECTED_DESTINATION" "$BACKUP_DESTINATION"
fi
if ! mv "$STAGED_DESTINATION" "$EXPECTED_DESTINATION"; then
  die "Unable to atomically install the verified full Crystal pack."
fi
NEW_DESTINATION_INSTALLED=1
INSTALL_COMMITTED=1

remove_all_backup_directories

if [ "$KEEP_ARCHIVE" -ne 1 ]; then
  assert_safe_child_path "$ARCHIVE_PATH" "$CACHE_DIRECTORY" "Reassembled archive"
  rm -f "$ARCHIVE_PATH"
fi

printf 'Full Crystal pack installed: %s\n' "$DESTINATION"
printf 'Content hash: %s\n' "$CONTENT_HASH"
