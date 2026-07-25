#!/usr/bin/env bash
set -euo pipefail

manifest_path="${MIR2_ASSET_MANIFEST_PATH:-/asset-manifest/developer-assets.json}"
cache_root="${MIR2_ASSET_CACHE_ROOT:-/asset-cache}"

die() {
  printf 'Developer asset fetch failed: %s\n' "$*" >&2
  exit 1
}

command -v gh >/dev/null 2>&1 || die "GitHub CLI is unavailable."
command -v node >/dev/null 2>&1 || die "Node.js is unavailable."
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is unavailable."
[ -f "${manifest_path}" ] || die "Manifest is missing: ${manifest_path}"

mkdir -p "${cache_root}"
cache_root="$(cd "${cache_root}" && pwd -P)"
[ "${cache_root}" != "/" ] || die "Refusing to use the filesystem root as cache."

mapfile -t manifest_records < <(
  node - "${manifest_path}" <<'NODE'
const fs = require("node:fs");

const manifestPath = process.argv[2];
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const fail = (message) => {
  throw new Error(message);
};
const isHash = (value) => /^[a-f0-9]{64}$/.test(value);
const isPlainName = (value) =>
  typeof value === "string" &&
  /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value) &&
  value !== "." &&
  value !== "..";

if (manifest.schemaVersion !== 1 || manifest.kind !== "mir2-developer-asset-bundle") {
  fail("Unsupported developer asset manifest.");
}
if (manifest.repository !== "Zombieliu/mir2") {
  fail("The isolated fetcher only permits the locked Zombieliu/mir2 release.");
}
if (!/^developer-assets-[a-f0-9]{12}$/.test(manifest.releaseTag)) {
  fail("Invalid developer asset release tag.");
}
if (!isHash(manifest.contentHash)) {
  fail("Invalid developer asset content hash.");
}
if (!Array.isArray(manifest.parts) || manifest.parts.length === 0) {
  fail("The developer asset manifest has no parts.");
}

console.log(
  ["META", manifest.repository, manifest.releaseTag, manifest.contentHash].join("\t"),
);
const names = new Set();
for (const part of manifest.parts) {
  if (
    !isPlainName(part?.name) ||
    !Number.isSafeInteger(part?.size) ||
    part.size <= 0 ||
    !isHash(part?.sha256) ||
    names.has(part.name)
  ) {
    fail("Invalid or duplicate developer asset part metadata.");
  }
  names.add(part.name);
  console.log(["PART", part.name, String(part.size), part.sha256].join("\t"));
}
NODE
) || die "Manifest validation failed."

[ "${#manifest_records[@]}" -gt 1 ] || die "Manifest produced no download records."
IFS=$'\t' read -r meta repository release_tag content_hash <<<"${manifest_records[0]}"
[ "${meta}" = "META" ] || die "Manifest metadata record is invalid."

cache_directory="${cache_root}/${release_tag}"
mkdir -p "${cache_directory}"
cache_directory="$(cd "${cache_directory}" && pwd -P)"
case "${cache_directory}" in
  "${cache_root}"/*) ;;
  *) die "Resolved cache directory escaped the cache root." ;;
esac

file_size() {
  stat -c '%s' "$1"
}

part_is_valid() {
  local path="$1"
  local expected_size="$2"
  local expected_hash="$3"
  [ -f "${path}" ] &&
    [ "$(file_size "${path}")" = "${expected_size}" ] &&
    [ "$(sha256sum "${path}" | cut -d ' ' -f 1)" = "${expected_hash}" ]
}

for record in "${manifest_records[@]:1}"; do
  IFS=$'\t' read -r kind part_name part_size part_hash <<<"${record}"
  [ "${kind}" = "PART" ] || die "Unexpected manifest record."
  target="${cache_directory}/${part_name}"

  if part_is_valid "${target}" "${part_size}" "${part_hash}"; then
    printf '[assets] verified cached part: %s\n' "${part_name}"
    continue
  fi

  download_directory="$(mktemp -d "${cache_root}/.fetch.XXXXXXXX")"
  cleanup_download() {
    rm -f -- "${download_directory}/${part_name}"
    rmdir "${download_directory}" 2>/dev/null || true
  }
  trap cleanup_download EXIT

  printf '[assets] downloading isolated release part: %s\n' "${part_name}"
  gh release download "${release_tag}" \
    --repo "${repository}" \
    --pattern "${part_name}" \
    --dir "${download_directory}"

  downloaded="${download_directory}/${part_name}"
  part_is_valid "${downloaded}" "${part_size}" "${part_hash}" ||
    die "Downloaded part failed size or SHA-256 verification: ${part_name}"
  mv -f -- "${downloaded}" "${target}"
  rmdir "${download_directory}"
  trap - EXIT
done

printf 'Pinned developer asset parts are ready: %s\n' "${cache_directory}"
printf 'Content hash: %s\n' "${content_hash}"
