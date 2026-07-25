#!/usr/bin/env bash

set -euo pipefail

fail() {
  printf 'Developer asset installer fixture failed: %s\n' "$*" >&2
  exit 1
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

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{ print tolower($1) }'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{ print tolower($1) }'
  else
    fail "sha256sum or shasum is required"
  fi
}

assert_no_install_artifacts() {
  local artifact

  artifact="$(find "$FIXTURE_PUBLIC" \
    \( -name '.mir2-asset-install-*' \
       -o -name '.full-backup-*' \
       -o -name '.full-install.lock' \
       -o -name '.full-install-lock-stale-*' \) \
    -print | sed -n '1p')"
  [ -z "$artifact" ] || fail "installer left a transaction artifact behind: $artifact"
}

create_pack() {
  local pack_root="$1"
  local content_hash="$2"
  local page_marker="$3"

  "$NODE_BIN" - "$pack_root" "$content_hash" "$page_marker" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const packRoot = process.argv[2];
const contentHash = process.argv[3];
const pageBytes = Buffer.from(process.argv[4], "utf8");
const pageHash = crypto.createHash("sha256").update(pageBytes).digest("hex");
const pageRelativePath = `pages/${pageHash.slice(0, 2)}/${pageHash}.png`;
const libraryRelativePath = "libraries/fixture/fixture.json";
const pagePath = path.join(packRoot, ...pageRelativePath.split("/"));
const libraryPath = path.join(packRoot, ...libraryRelativePath.split("/"));

fs.rmSync(packRoot, { recursive: true, force: true });
fs.mkdirSync(path.dirname(pagePath), { recursive: true });
fs.mkdirSync(path.dirname(libraryPath), { recursive: true });
fs.writeFileSync(pagePath, pageBytes);

const library = {
  libraryKey: "Fixture/00",
  pages: [{
    id: "p0",
    key: `sha256:${pageHash}`,
    sha256: pageHash,
    imageUrl: `/generated/crystal-packs/full/${pageRelativePath}`,
    networkBytes: pageBytes.length,
  }],
};
const libraryBytes = Buffer.from(`${JSON.stringify(library, null, 2)}\n`);
fs.writeFileSync(libraryPath, libraryBytes);
const libraryHash = crypto.createHash("sha256").update(libraryBytes).digest("hex");

const index = {
  schemaVersion: 1,
  kind: "mir2-crystal-full-pack-index",
  contentHash,
  sourceContentHash: "c".repeat(64),
  summary: { libraryCount: 1 },
  libraries: [{
    key: "Fixture/00",
    pageCount: 1,
    manifestUrl: `/generated/crystal-packs/full/${libraryRelativePath}`,
    shardUrl: `/generated/crystal-packs/full/${libraryRelativePath}`,
    manifestSha256: libraryHash,
  }],
};
fs.writeFileSync(path.join(packRoot, "index.json"), `${JSON.stringify(index, null, 2)}\n`);
NODE
}

write_manifest() {
  local manifest_path="$1"
  local archive_path="$2"
  local part_path="$3"
  local release_tag="$4"
  local content_hash="$5"

  "$NODE_BIN" - "$manifest_path" "$archive_path" "$part_path" "$release_tag" "$content_hash" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const manifestPath = process.argv[2];
const archivePath = process.argv[3];
const partPath = process.argv[4];
const releaseTag = process.argv[5];
const contentHash = process.argv[6];
const sha256 = (filePath) => crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
const manifest = {
  schemaVersion: 1,
  kind: "mir2-developer-asset-bundle",
  repository: "Zombieliu/mir2",
  releaseTag,
  contentHash,
  destination: "mir2-web3/apps/web/public/generated/crystal-packs/full",
  archive: {
    name: path.basename(archivePath),
    size: fs.statSync(archivePath).size,
    sha256: sha256(archivePath),
    format: "ustar",
  },
  parts: [{
    name: path.basename(partPath),
    size: fs.statSync(partPath).size,
    sha256: sha256(partPath),
  }],
};
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
NODE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
DATA_ROOT="$PROJECT_ROOT/.mir2-data"
NODE_BIN="$(find_node)" || fail "Node.js is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

mkdir -p "$DATA_ROOT"
DATA_ROOT="$(cd "$DATA_ROOT" && pwd -P)"
TEST_ROOT="$(mktemp -d "$DATA_ROOT/installer-test.XXXXXX")"
case "$TEST_ROOT" in
  "$DATA_ROOT"/installer-test.*) ;;
  *) fail "refusing to create fixture outside .mir2-data: $TEST_ROOT" ;;
esac

cleanup() {
  local status="$?"
  trap - EXIT
  set +e
  case "$TEST_ROOT" in
    "$DATA_ROOT"/installer-test.*) rm -rf "$TEST_ROOT" ;;
  esac
  exit "$status"
}
trap 'exit 130' HUP INT TERM
trap cleanup EXIT

FIXTURE_PROJECT="$TEST_ROOT/project"
FIXTURE_SCRIPTS="$FIXTURE_PROJECT/scripts"
FIXTURE_PUBLIC="$FIXTURE_PROJECT/apps/web/public"
FIXTURE_ASSET_SCRIPTS="$FIXTURE_PROJECT/apps/web/scripts/asset-pipeline"
FIXTURE_PARTS="$TEST_ROOT/parts"
FIXTURE_CACHE="$TEST_ROOT/cache"
FIXTURE_PACK="$TEST_ROOT/full-pack"
EXISTING_PACK="$FIXTURE_PUBLIC/generated/crystal-packs/full"
CONTENT_HASH_V1="$(printf 'a%.0s' {1..64})"
CONTENT_HASH_V2="$(printf 'b%.0s' {1..64})"

mkdir -p \
  "$FIXTURE_SCRIPTS" \
  "$FIXTURE_ASSET_SCRIPTS" \
  "$FIXTURE_PUBLIC" \
  "$FIXTURE_PARTS" \
  "$FIXTURE_CACHE" \
  "$FIXTURE_PACK" \
  "$EXISTING_PACK"

cp "$SCRIPT_DIR/install-developer-assets.sh" "$FIXTURE_SCRIPTS/install-developer-assets.sh"
for script_name in \
  full-pack-closure.mjs \
  inspect-ustar-archive.mjs \
  verify-full-pack-closure.mjs; do
  cp "$PROJECT_ROOT/apps/web/scripts/asset-pipeline/$script_name" "$FIXTURE_ASSET_SCRIPTS/$script_name"
done

"$NODE_BIN" - "$EXISTING_PACK" "$CONTENT_HASH_V1" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

// A matching index with missing shards/pages must be repaired, not trusted.
fs.writeFileSync(
  path.join(process.argv[2], "index.json"),
  `${JSON.stringify({ contentHash: process.argv[3] }, null, 2)}\n`,
);
NODE
create_pack "$FIXTURE_PACK" "$CONTENT_HASH_V1" "fixture-page-v1"

ARCHIVE_PATH="$TEST_ROOT/fixture.tar"
REPEAT_ARCHIVE_PATH="$TEST_ROOT/fixture-repeat.tar"
"$NODE_BIN" "$PROJECT_ROOT/apps/web/scripts/asset-pipeline/package-full-pack-archive.mjs" \
  --root "$FIXTURE_PACK" \
  --output "$ARCHIVE_PATH" \
  --expectedContentHash "$CONTENT_HASH_V1"
"$NODE_BIN" "$PROJECT_ROOT/apps/web/scripts/asset-pipeline/package-full-pack-archive.mjs" \
  --root "$FIXTURE_PACK" \
  --output "$REPEAT_ARCHIVE_PATH" \
  --expectedContentHash "$CONTENT_HASH_V1" >/dev/null
[ "$(sha256_file "$ARCHIVE_PATH")" = "$(sha256_file "$REPEAT_ARCHIVE_PATH")" ] ||
  fail "deterministic fixture archive hashes differ"

UNSAFE_ARCHIVE_PATH="$TEST_ROOT/fixture-symlink.tar"
"$NODE_BIN" - "$ARCHIVE_PATH" "$UNSAFE_ARCHIVE_PATH" <<'NODE'
const fs = require("node:fs");

const bytes = fs.readFileSync(process.argv[2]);
bytes[156] = "2".charCodeAt(0);
bytes.fill(0x20, 148, 156);
const checksum = bytes.subarray(0, 512).reduce((sum, value) => sum + value, 0);
const encoded = Buffer.from(checksum.toString(8).padStart(6, "0"), "ascii");
encoded.copy(bytes, 148);
bytes[154] = 0;
bytes[155] = 0x20;
fs.writeFileSync(process.argv[3], bytes);
NODE
if "$NODE_BIN" "$FIXTURE_ASSET_SCRIPTS/inspect-ustar-archive.mjs" \
  --archive "$UNSAFE_ARCHIVE_PATH" \
  --prefix "generated/crystal-packs/full" >/dev/null 2>&1; then
  fail "archive inspector accepted a synthetic symlink entry"
fi

PART_NAME="fixture.tar.part001"
PART_PATH="$FIXTURE_PARTS/$PART_NAME"
cp "$ARCHIVE_PATH" "$PART_PATH"
MANIFEST_PATH="$TEST_ROOT/developer-assets.json"
write_manifest "$MANIFEST_PATH" "$ARCHIVE_PATH" "$PART_PATH" "fixture-v1" "$CONTENT_HASH_V1"
mkdir -p "$FIXTURE_PROJECT/config"
cp "$MANIFEST_PATH" "$FIXTURE_PROJECT/config/developer-assets.json"

bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
  --manifest-path "$MANIFEST_PATH" \
  --parts-directory "$FIXTURE_PARTS" \
  --cache-directory "$FIXTURE_CACHE"

[ -f "$EXISTING_PACK/index.json" ] || fail "fixture install index is missing"
INSTALLED_HASH="$("$NODE_BIN" -e '
  const fs = require("node:fs");
  process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).contentHash);
' "$EXISTING_PACK/index.json")"
[ "$INSTALLED_HASH" = "$CONTENT_HASH_V1" ] || fail "fixture install content hash mismatch"
PAGE_PATH="$(find "$EXISTING_PACK/pages" -type f -name '*.png' -print | sed -n '1p')"
[ -n "$PAGE_PATH" ] || fail "fixture install page is missing"
[ ! -e "$FIXTURE_CACHE/fixture.tar" ] || fail "installer did not remove the reassembled archive"
assert_no_install_artifacts

"$NODE_BIN" "$FIXTURE_ASSET_SCRIPTS/verify-full-pack-closure.mjs" \
  --root "$EXISTING_PACK" \
  --expectedContentHash "$CONTENT_HASH_V1" \
  --verifyPages true >/dev/null

# Replace the default manifest with a new release and prove that the wrapper's
# normal path upgrades a fully verified old hash without --force.
FIXTURE_PACK_V2="$TEST_ROOT/full-pack-v2"
ARCHIVE_PATH_V2="$TEST_ROOT/fixture-v2.tar"
FIXTURE_PARTS_V2="$TEST_ROOT/parts-v2"
FIXTURE_CACHE_V2="$TEST_ROOT/cache-v2"
PART_NAME_V2="fixture-v2.tar.part001"
PART_PATH_V2="$FIXTURE_PARTS_V2/$PART_NAME_V2"
MANIFEST_PATH_V2="$TEST_ROOT/developer-assets-v2.json"
mkdir -p "$FIXTURE_PARTS_V2" "$FIXTURE_CACHE_V2"
create_pack "$FIXTURE_PACK_V2" "$CONTENT_HASH_V2" "fixture-page-v2"
"$NODE_BIN" "$PROJECT_ROOT/apps/web/scripts/asset-pipeline/package-full-pack-archive.mjs" \
  --root "$FIXTURE_PACK_V2" \
  --output "$ARCHIVE_PATH_V2" \
  --expectedContentHash "$CONTENT_HASH_V2" >/dev/null
cp "$ARCHIVE_PATH_V2" "$PART_PATH_V2"
write_manifest \
  "$MANIFEST_PATH_V2" \
  "$ARCHIVE_PATH_V2" \
  "$PART_PATH_V2" \
  "fixture-v2" \
  "$CONTENT_HASH_V2"
cp "$MANIFEST_PATH_V2" "$FIXTURE_PROJECT/config/developer-assets.json"

bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
  --parts-directory "$FIXTURE_PARTS_V2" \
  --cache-directory "$FIXTURE_CACHE_V2" >/dev/null

INSTALLED_HASH="$("$NODE_BIN" -e '
  const fs = require("node:fs");
  process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).contentHash);
' "$EXISTING_PACK/index.json")"
[ "$INSTALLED_HASH" = "$CONTENT_HASH_V2" ] ||
  fail "default installer path did not upgrade the old content hash"
"$NODE_BIN" "$FIXTURE_ASSET_SCRIPTS/verify-full-pack-closure.mjs" \
  --root "$EXISTING_PACK" \
  --expectedContentHash "$CONTENT_HASH_V2" \
  --verifyPages true >/dev/null
assert_no_install_artifacts

# The remaining failure/recovery checks operate against the upgraded release.
FIXTURE_PACK="$FIXTURE_PACK_V2"
ARCHIVE_PATH="$ARCHIVE_PATH_V2"
FIXTURE_PARTS="$FIXTURE_PARTS_V2"
FIXTURE_CACHE="$FIXTURE_CACHE_V2"
PART_NAME="$PART_NAME_V2"
PART_PATH="$PART_PATH_V2"
MANIFEST_PATH="$MANIFEST_PATH_V2"

# Corrupt an archived page without touching the ustar headers. Safety inspection
# still passes, but closure verification must reject it before destination swap.
GOOD_INSTALL="$TEST_ROOT/good-install"
mv "$EXISTING_PACK" "$GOOD_INSTALL"
mkdir -p "$EXISTING_PACK"
printf 'keep-me\n' > "$EXISTING_PACK/sentinel.txt"
"$NODE_BIN" -e '
  const fs = require("node:fs");
  fs.writeFileSync(process.argv[1], JSON.stringify({ contentHash: "d".repeat(64) }));
' "$EXISTING_PACK/index.json"

BAD_ARCHIVE_PATH="$TEST_ROOT/fixture-bad.tar"
"$NODE_BIN" - "$ARCHIVE_PATH" "$BAD_ARCHIVE_PATH" <<'NODE'
const fs = require("node:fs");

const bytes = fs.readFileSync(process.argv[2]);
const marker = Buffer.from("fixture-page", "utf8");
const offset = bytes.indexOf(marker);
if (offset < 0) throw new Error("fixture page payload was not found in archive");
bytes[offset] ^= 0xff;
fs.writeFileSync(process.argv[3], bytes);
NODE
BAD_PARTS="$TEST_ROOT/bad-parts"
BAD_CACHE="$TEST_ROOT/bad-cache"
mkdir -p "$BAD_PARTS" "$BAD_CACHE"
BAD_PART_PATH="$BAD_PARTS/fixture-bad.tar.part001"
cp "$BAD_ARCHIVE_PATH" "$BAD_PART_PATH"
BAD_MANIFEST_PATH="$TEST_ROOT/developer-assets-bad.json"
write_manifest \
  "$BAD_MANIFEST_PATH" \
  "$BAD_ARCHIVE_PATH" \
  "$BAD_PART_PATH" \
  "fixture-bad" \
  "$CONTENT_HASH_V2"

if bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
  --manifest-path "$BAD_MANIFEST_PATH" \
  --parts-directory "$BAD_PARTS" \
  --cache-directory "$BAD_CACHE" \
  --force >"$TEST_ROOT/bad-install.out.log" 2>"$TEST_ROOT/bad-install.err.log"; then
  fail "installer accepted a bundle with invalid closure"
fi
[ -f "$EXISTING_PACK/sentinel.txt" ] ||
  fail "failed staging verification replaced the existing destination"
assert_no_install_artifacts

rm -rf "$EXISTING_PACK"
mv "$GOOD_INSTALL" "$EXISTING_PACK"

# Exercise interrupted-download recovery with a deterministic local gh shim.
RECOVERED_INSTALL="$TEST_ROOT/recovered-install"
mv "$EXISTING_PACK" "$RECOVERED_INSTALL"
printf 'partial-download\n' > "$FIXTURE_CACHE/$PART_NAME"

FAKE_BIN="$TEST_ROOT/fake-bin"
mkdir -p "$FAKE_BIN"
cat > "$FAKE_BIN/gh" <<'GH'
#!/usr/bin/env bash
set -euo pipefail

if [ "${MIR2_TEST_GH_FAIL:-0}" = "1" ]; then
  exit 97
fi

pattern=""
destination=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --pattern)
      pattern="$2"
      shift 2
      ;;
    --dir)
      destination="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
[ -n "$pattern" ] && [ -n "$destination" ]
cp "$MIR2_TEST_GH_SOURCE/$pattern" "$destination/$pattern"
GH
chmod +x "$FAKE_BIN/gh"

PATH="$FAKE_BIN:$PATH" MIR2_TEST_GH_SOURCE="$FIXTURE_PARTS" \
  bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
    --manifest-path "$MANIFEST_PATH" \
    --cache-directory "$FIXTURE_CACHE" \
    --download

[ -n "$(find "$EXISTING_PACK/pages" -type f -name '*.png' -print | sed -n '1p')" ] ||
  fail "recovered fixture install page is missing"
[ "$(sha256_file "$FIXTURE_CACHE/$PART_NAME")" = "$(sha256_file "$PART_PATH")" ] ||
  fail "interrupted-download recovery did not replace the corrupt cached part"
assert_no_install_artifacts

# A verified existing install must be closure-checked and return before gh runs.
PATH="$FAKE_BIN:$PATH" MIR2_TEST_GH_SOURCE="$FIXTURE_PARTS" MIR2_TEST_GH_FAIL=1 \
  bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
    --cache-directory "$FIXTURE_CACHE" \
    --download >/dev/null

# Simulate SIGKILL after the old destination was renamed but before the staged
# pack was committed. Startup must restore backup first and discard staging.
TRANSACTION_PARENT="$(dirname "$EXISTING_PACK")"
RECOVERY_BACKUP="$TRANSACTION_PARENT/.full-backup-fixture-crash"
RECOVERY_STAGING="$FIXTURE_PUBLIC/.mir2-asset-install-fixture-crash"
mv "$EXISTING_PACK" "$RECOVERY_BACKUP"
mkdir -p "$RECOVERY_STAGING/generated/crystal-packs/full"
printf 'partial-staging\n' > "$RECOVERY_STAGING/generated/crystal-packs/full/partial.txt"
bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
  --parts-directory "$FIXTURE_PARTS" \
  --cache-directory "$FIXTURE_CACHE" >/dev/null
[ -f "$EXISTING_PACK/index.json" ] ||
  fail "startup recovery did not restore the missing current directory"
assert_no_install_artifacts

LOCK_DIRECTORY="$TRANSACTION_PARENT/.full-install.lock"
LOCK_HOST="$(hostname 2>/dev/null || uname -n)"

# A live cross-process lock must be rejectable immediately when the caller
# chooses a zero-second wait.
mkdir "$LOCK_DIRECTORY"
{
  printf 'pid=%s\n' "$$"
  printf 'host=%s\n' "$LOCK_HOST"
  printf 'token=fixture-live-lock\n'
} > "$LOCK_DIRECTORY/owner"
if MIR2_DEVELOPER_ASSET_LOCK_WAIT_SECONDS=0 \
  bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
    --parts-directory "$FIXTURE_PARTS" \
    --cache-directory "$FIXTURE_CACHE" \
    >"$TEST_ROOT/live-lock.out.log" 2>"$TEST_ROOT/live-lock.err.log"; then
  fail "installer ignored a live cross-process lock"
fi
grep -q "holds the lock" "$TEST_ROOT/live-lock.err.log" ||
  fail "live-lock rejection did not report the lock owner conflict"
rm -rf "$LOCK_DIRECTORY"

# The default lock path waits instead of interleaving. Release a lock from a
# separate process and require the installer to continue successfully.
mkdir "$LOCK_DIRECTORY"
{
  printf 'pid=%s\n' "$$"
  printf 'host=%s\n' "$LOCK_HOST"
  printf 'token=fixture-wait-lock\n'
} > "$LOCK_DIRECTORY/owner"
(
  sleep 2
  rm -rf "$LOCK_DIRECTORY"
) &
LOCK_RELEASER_PID="$!"
MIR2_DEVELOPER_ASSET_LOCK_WAIT_SECONDS=10 \
  bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
    --parts-directory "$FIXTURE_PARTS" \
    --cache-directory "$FIXTURE_CACHE" >/dev/null
wait "$LOCK_RELEASER_PID"
assert_no_install_artifacts

# A dead owner left by SIGKILL or power loss must be reclaimed automatically.
mkdir "$LOCK_DIRECTORY"
{
  printf 'pid=2147483647\n'
  printf 'host=%s\n' "$LOCK_HOST"
  printf 'token=fixture-dead-lock\n'
} > "$LOCK_DIRECTORY/owner"
MIR2_DEVELOPER_ASSET_LOCK_WAIT_SECONDS=2 \
  bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
    --parts-directory "$FIXTURE_PARTS" \
    --cache-directory "$FIXTURE_CACHE" >/dev/null
assert_no_install_artifacts

# Reject both descendants and ancestors of the live destination before either
# path can be created or mutated.
if bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
  --parts-directory "$FIXTURE_PARTS" \
  --cache-directory "$EXISTING_PACK/cache" \
  >"$TEST_ROOT/overlap-child.out.log" 2>"$TEST_ROOT/overlap-child.err.log"; then
  fail "installer accepted a cache nested inside the destination"
fi
[ ! -e "$EXISTING_PACK/cache" ] ||
  fail "overlap rejection created a cache inside the destination"
if bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
  --parts-directory "$FIXTURE_PARTS" \
  --cache-directory "$TRANSACTION_PARENT" \
  >"$TEST_ROOT/overlap-parent.out.log" 2>"$TEST_ROOT/overlap-parent.err.log"; then
  fail "installer accepted a cache containing the destination"
fi
grep -q "must not overlap" "$TEST_ROOT/overlap-child.err.log" ||
  fail "nested overlap rejection did not explain the path conflict"
grep -q "must not overlap" "$TEST_ROOT/overlap-parent.err.log" ||
  fail "parent overlap rejection did not explain the path conflict"
"$NODE_BIN" "$FIXTURE_ASSET_SCRIPTS/verify-full-pack-closure.mjs" \
  --root "$EXISTING_PACK" \
  --expectedContentHash "$CONTENT_HASH_V2" \
  --verifyPages true >/dev/null
assert_no_install_artifacts

# KeepArchive must retain the verified reassembled ustar when explicitly requested.
mv "$EXISTING_PACK" "$TEST_ROOT/idempotent-install"
KEEP_CACHE="$TEST_ROOT/keep-cache"
if command -v shasum >/dev/null 2>&1; then
  MIR2_DEVELOPER_ASSET_SHA256_TOOL=shasum \
    bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
      --parts-directory "$FIXTURE_PARTS" \
      --cache-directory "$KEEP_CACHE" \
      --keep-archive >/dev/null
else
  bash "$FIXTURE_SCRIPTS/install-developer-assets.sh" \
    --parts-directory "$FIXTURE_PARTS" \
    --cache-directory "$KEEP_CACHE" \
    --keep-archive >/dev/null
fi
[ -f "$KEEP_CACHE/fixture-v2.tar" ] || fail "--keep-archive did not retain the archive"
assert_no_install_artifacts

printf 'Developer asset installer fixture passed.\n'
