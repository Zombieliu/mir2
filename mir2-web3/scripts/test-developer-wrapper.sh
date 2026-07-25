#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temp_root="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
test_root="$(mktemp -d "${temp_root}/mir2-wrapper-test.XXXXXXXX")"
fake_bin="${test_root}/bin"
mkdir -p "${fake_bin}"

cleanup() {
  rm -f -- "${fake_bin}/docker"
  rmdir "${fake_bin}" 2>/dev/null || true
  rmdir "${test_root}" 2>/dev/null || true
}
trap cleanup EXIT

cat > "${fake_bin}/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  info)
    printf '26.1.0\n'
    ;;
  volume)
    [ "${2:-}" = "create" ] || exit 2
    printf '%s\n' "${3:-mir2-developer-gh-config}"
    ;;
  compose)
    if [ "${2:-}" = "version" ]; then
      printf 'Docker Compose version v2.24.4\n'
      exit 0
    fi
    case " $* " in
      *" config --quiet "*) exit 0 ;;
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

output="$(
  PATH="${fake_bin}:${PATH}" \
  MIR2_DEV_IMAGE="mir2-web3-developer:wrapper-test" \
    "${project_root}/scripts/dev.sh" doctor
)"
printf '%s\n' "${output}"
grep -F "[ok] Developer environment definition is valid." <<<"${output}" >/dev/null
printf 'Developer Bash wrapper fixture passed.\n'
