#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temp_root="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
test_root="$(mktemp -d "${temp_root}/mir2-wrapper-test.XXXXXXXX")"
fake_bin="${test_root}/bin"
mkdir -p "${fake_bin}"
release_lock="${project_root}/config/developer-release.json"
release_backup="${test_root}/developer-release.json"
docker_log="${test_root}/docker.log"

cleanup() {
  if [[ -f "${release_backup}" ]]; then
    cp -p -- "${release_backup}" "${release_lock}"
    rm -f -- "${release_backup}"
  fi
  rm -f -- "${fake_bin}/gh"
  rm -f -- "${fake_bin}/docker"
  rm -f -- "${docker_log}"
  rmdir "${fake_bin}" 2>/dev/null || true
  rmdir "${test_root}" 2>/dev/null || true
}
trap cleanup EXIT

cat > "${fake_bin}/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${MIR2_FAKE_DOCKER_LOG:-}" ]]; then
  printf '%s\n' "$*" >>"${MIR2_FAKE_DOCKER_LOG}"
fi

case "${1:-}" in
  info)
    printf '26.1.0\n'
    ;;
  login)
    cat >/dev/null
    [ "${2:-}" = "ghcr.io" ] || exit 2
    ;;
  pull)
    if [[ "${MIR2_FAKE_PULL_FAIL:-0}" = "1" ]]; then
      exit 1
    fi
    [ "${2:-}" = "ghcr.io/zombieliu/mir2-developer@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" ] || exit 2
    ;;
  image)
    [ "${2:-}" = "inspect" ] || exit 2
    if [[ "${MIR2_FAKE_PULL_FAIL:-0}" = "1" ]]; then
      exit 1
    fi
    printf '%s\n' "${MIR2_DEVELOPER_IMAGE_REVISION:?missing image revision}"
    ;;
  compose)
    if [ "${2:-}" = "version" ]; then
      printf 'Docker Compose version v2.24.4\n'
      exit 0
    fi
    case " $* " in
      *" config --quiet "*) exit 0 ;;
      *" build workspace "*) exit 0 ;;
      *" run --rm --no-deps -T asset-fetch "*) cat >/dev/null; exit 0 ;;
      *" run --rm --no-deps --user "*" --entrypoint node workspace apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs "*)
        if [[ "${MIR2_FAKE_RUNTIME_FETCH_FAIL:-0}" = "1" ]]; then exit 1; fi
        exit 0
        ;;
      *" run --rm --no-deps --user "*" --entrypoint bash workspace -lc "*"MIR2_USE_PREBUILT_BEVY_RUNTIME=0"*) exit 0 ;;
      *" run --rm --no-deps workspace bash "*) exit 0 ;;
      *) printf 'Unexpected fake Docker Compose command: %s\n' "$*" >&2; exit 2 ;;
    esac
    ;;
  *)
    printf 'Unexpected fake Docker command: %s\n' "$*" >&2
    exit 2
    ;;
esac
EOF
chmod +x "${fake_bin}/docker"

cat > "${fake_bin}/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${MIR2_FAKE_GH_FORBIDDEN:-0}" = "1" ]]; then
  printf 'GitHub CLI must not be called during Starter fallback.\n' >&2
  exit 99
fi

case " $* " in
  *" api repos/Zombieliu/mir2/git/ref/tags/developer-image-"*)
    printf 'tag\ncccccccccccccccccccccccccccccccccccccccc\n'
    ;;
  *" api repos/Zombieliu/mir2/git/tags/cccccccccccccccccccccccccccccccccccccccc "*)
    printf '%s\nghcr.io/zombieliu/mir2-developer@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n' \
      "${MIR2_DEVELOPER_IMAGE_REVISION:?missing image revision}"
    ;;
  *" auth status --hostname github.com "*) exit 0 ;;
  *" api user --jq .login "*) printf 'fixture-user\n' ;;
  *" auth token --hostname github.com "*) printf 'fixture-token\n' ;;
  *) printf 'Unexpected fake GitHub CLI command: %s\n' "$*" >&2; exit 2 ;;
esac
EOF
chmod +x "${fake_bin}/gh"

output="$(
  PATH="${fake_bin}:${PATH}" \
  MIR2_DEV_IMAGE="mir2-web3-developer:wrapper-test" \
    "${project_root}/scripts/dev.sh" doctor
)"
printf '%s\n' "${output}"
grep -F "[ok] Developer environment definition is valid." <<<"${output}" >/dev/null

cp -p -- "${release_lock}" "${release_backup}"
node - "${release_lock}" "$(git -C "${project_root}/.." rev-parse HEAD)" <<'NODE'
const fs = require("node:fs");
const [path, revision] = process.argv.slice(2);
const release = JSON.parse(fs.readFileSync(path, "utf8"));
release.container.publishedDigest = `sha256:${"a".repeat(64)}`;
release.container.publishedRevision = revision;
fs.writeFileSync(path, `${JSON.stringify(release, null, 2)}\n`);
NODE

asset_output="$(
  PATH="${fake_bin}:${PATH}" \
    env -u MIR2_DEV_IMAGE "${project_root}/scripts/dev.sh" assets
)"
printf '%s\n' "${asset_output}"
grep -F "[ok] Assets" <<<"${asset_output}" >/dev/null

fallback_output="$(
  PATH="${fake_bin}:${PATH}" \
  MIR2_FAKE_PULL_FAIL=1 \
  MIR2_FAKE_GH_FORBIDDEN=1 \
    env -u MIR2_DEV_IMAGE "${project_root}/scripts/dev.sh" shell
)"
printf '%s\n' "${fallback_output}"
grep -F "falling back to the locked local build" <<<"${fallback_output}" >/dev/null
grep -F "Build the locked local developer image" <<<"${fallback_output}" >/dev/null

runtime_fallback_output="$(
  PATH="${fake_bin}:${PATH}" \
  MIR2_DEV_IMAGE="mir2-web3-developer:wrapper-test" \
  MIR2_FAKE_RUNTIME_FETCH_FAIL=1 \
  MIR2_FAKE_DOCKER_LOG="${docker_log}" \
    "${project_root}/scripts/dev.sh" verify
)"
printf '%s\n' "${runtime_fallback_output}"
grep -F "Pinned Bevy runtime is unavailable; rebuilding it from current source." \
  <<<"${runtime_fallback_output}" >/dev/null
grep -F "MIR2_USE_PREBUILT_BEVY_RUNTIME=0 node apps/web/scripts/build-bevy-runtime.mjs release" \
  "${docker_log}" >/dev/null

set +e
custom_output="$(
  PATH="${fake_bin}:${PATH}" \
    MIR2_DEV_IMAGE="ghcr.io/example/evil@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" \
    "${project_root}/scripts/dev.sh" assets 2>&1
)"
custom_status=$?
set -e
if [[ "${custom_status}" -eq 0 ]]; then
  echo "Custom digest image was not rejected for full assets." >&2
  exit 1
fi
grep -F "Full asset authorization refuses a custom developer image." <<<"${custom_output}" >/dev/null
cp -p -- "${release_backup}" "${release_lock}"
printf 'Developer Bash wrapper fixture passed.\n'
