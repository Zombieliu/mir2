#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${project_root}/.mir2-data"
test_root="$(mktemp -d "${project_root}/.mir2-data/fetcher-test.XXXXXXXX")"

cleanup() {
  case "${test_root}" in
    "${project_root}"/.mir2-data/fetcher-test.*) rm -rf -- "${test_root}" ;;
    *) printf 'Refusing to clean unexpected fetcher fixture path: %s\n' "${test_root}" >&2 ;;
  esac
}
trap cleanup EXIT

source_directory="${test_root}/release"
cache_directory="${test_root}/cache"
fake_bin="${test_root}/bin"
manifest_path="${test_root}/developer-assets.json"
download_log="${test_root}/downloads.log"
mkdir -p "${source_directory}" "${cache_directory}" "${fake_bin}"

printf 'first-pinned-part\n' > "${source_directory}/fixture.tar.part001"
printf 'second-pinned-part\n' > "${source_directory}/fixture.tar.part002"

part1_size="$(wc -c < "${source_directory}/fixture.tar.part001" | tr -d ' ')"
part2_size="$(wc -c < "${source_directory}/fixture.tar.part002" | tr -d ' ')"
part1_hash="$(sha256sum "${source_directory}/fixture.tar.part001" | cut -d ' ' -f 1)"
part2_hash="$(sha256sum "${source_directory}/fixture.tar.part002" | cut -d ' ' -f 1)"

cat > "${manifest_path}" <<EOF
{
  "schemaVersion": 1,
  "kind": "mir2-developer-asset-bundle",
  "repository": "Zombieliu/mir2",
  "releaseTag": "developer-assets-aaaaaaaaaaaa",
  "contentHash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "parts": [
    {
      "name": "fixture.tar.part001",
      "size": ${part1_size},
      "sha256": "${part1_hash}"
    },
    {
      "name": "fixture.tar.part002",
      "size": ${part2_size},
      "sha256": "${part2_hash}"
    }
  ]
}
EOF

cat > "${fake_bin}/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

[ "${1:-}" = "release" ] && [ "${2:-}" = "download" ] ||
  { echo "Unexpected gh command" >&2; exit 2; }
[ "${GH_TOKEN:-}" = "fixture-token-from-stdin" ] ||
  { echo "Fixture token was not forwarded through standard input" >&2; exit 2; }
tag="${3:-}"
shift 3
repository=""
pattern=""
directory=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo) repository="$2"; shift 2 ;;
    --pattern) pattern="$2"; shift 2 ;;
    --dir) directory="$2"; shift 2 ;;
    *) echo "Unexpected gh argument: $1" >&2; exit 2 ;;
  esac
done

[ "${repository}" = "Zombieliu/mir2" ] ||
  { echo "Unexpected repository: ${repository}" >&2; exit 2; }
[ "${tag}" = "developer-assets-aaaaaaaaaaaa" ] ||
  { echo "Unexpected release tag: ${tag}" >&2; exit 2; }
case "${pattern}" in
  fixture.tar.part001|fixture.tar.part002) ;;
  *) echo "Unexpected release pattern: ${pattern}" >&2; exit 2 ;;
esac

printf '%s\n' "${pattern}" >> "${GH_FIXTURE_DOWNLOAD_LOG}"
cp "${GH_FIXTURE_RELEASE_DIRECTORY}/${pattern}" "${directory}/${pattern}"
EOF
chmod +x "${fake_bin}/gh"

run_fetcher() {
  printf '%s\n' 'fixture-token-from-stdin' |
    PATH="${fake_bin}:${PATH}" \
    GH_FIXTURE_RELEASE_DIRECTORY="${source_directory}" \
    GH_FIXTURE_DOWNLOAD_LOG="${download_log}" \
    MIR2_ASSET_MANIFEST_PATH="${manifest_path}" \
    MIR2_ASSET_CACHE_ROOT="${cache_directory}" \
      "${project_root}/infra/developer-asset-fetch.sh"
}

run_fetcher
installed_cache="${cache_directory}/developer-assets-aaaaaaaaaaaa"
cmp "${source_directory}/fixture.tar.part001" "${installed_cache}/fixture.tar.part001"
cmp "${source_directory}/fixture.tar.part002" "${installed_cache}/fixture.tar.part002"
[ "$(wc -l < "${download_log}" | tr -d ' ')" = "2" ]

run_fetcher
[ "$(wc -l < "${download_log}" | tr -d ' ')" = "2" ]

printf 'corrupt\n' > "${installed_cache}/fixture.tar.part001"
run_fetcher
cmp "${source_directory}/fixture.tar.part001" "${installed_cache}/fixture.tar.part001"
[ "$(wc -l < "${download_log}" | tr -d ' ')" = "3" ]

malicious_manifest="${test_root}/malicious-repository.json"
sed 's#"Zombieliu/mir2"#"attacker/private-repository"#' \
  "${manifest_path}" > "${malicious_manifest}"
if printf '%s\n' 'fixture-token-from-stdin' |
   PATH="${fake_bin}:${PATH}" \
   GH_FIXTURE_RELEASE_DIRECTORY="${source_directory}" \
   GH_FIXTURE_DOWNLOAD_LOG="${download_log}" \
   MIR2_ASSET_MANIFEST_PATH="${malicious_manifest}" \
   MIR2_ASSET_CACHE_ROOT="${cache_directory}" \
     "${project_root}/infra/developer-asset-fetch.sh" >/dev/null 2>&1; then
  echo "Fetcher accepted an unauthorized repository." >&2
  exit 1
fi

traversal_manifest="${test_root}/traversal-part.json"
sed 's#"fixture.tar.part001"#"../fixture.tar.part001"#' \
  "${manifest_path}" > "${traversal_manifest}"
if printf '%s\n' 'fixture-token-from-stdin' |
   PATH="${fake_bin}:${PATH}" \
   GH_FIXTURE_RELEASE_DIRECTORY="${source_directory}" \
   GH_FIXTURE_DOWNLOAD_LOG="${download_log}" \
   MIR2_ASSET_MANIFEST_PATH="${traversal_manifest}" \
   MIR2_ASSET_CACHE_ROOT="${cache_directory}" \
     "${project_root}/infra/developer-asset-fetch.sh" >/dev/null 2>&1; then
  echo "Fetcher accepted a traversal part name." >&2
  exit 1
fi

printf 'Developer asset fetcher fixture passed.\n'
