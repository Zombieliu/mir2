#!/usr/bin/env bash
set -euo pipefail
umask 077

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
project_root="$(cd -- "${script_dir}/.." && pwd -P)"
self_test=0
quiet=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --project-root) project_root="$(cd -- "$2" && pwd -P)"; shift 2 ;;
    --self-test) self_test=1; shift ;;
    --quiet) quiet=1; shift ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done

assert_not_link() {
  [[ ! -L "$1" ]] || { echo "Refusing symlink in local save-recovery path: $1" >&2; return 1; }
}

permission_mode() {
  if stat -c '%a' "$1" >/dev/null 2>&1; then stat -c '%a' "$1"; else stat -f '%Lp' "$1"; fi
}

initialize() {
  local root="$1" data="${1}/.mir2-data"
  local secrets="${1}/.mir2-data/local-secrets"
  local key="${secrets}/save-recovery-mac-key.hex"
  local recovery="${1}/.mir2-data/save-recovery/v1/developer-gateway"
  local path
  for path in "${root}" "${data}" "${secrets}" "${key}" "${data}/save-recovery" "${data}/save-recovery/v1" "${recovery}"; do assert_not_link "${path}"; done
  mkdir -p "${secrets}" "${recovery}"
  for path in "${data}" "${secrets}" "${data}/save-recovery" "${data}/save-recovery/v1" "${recovery}"; do
    assert_not_link "${path}"; chmod 700 "${path}"
    [[ "$(permission_mode "${path}")" == 700 ]] || { echo "Unsafe directory permissions: ${path}" >&2; return 1; }
  done
  if [[ ! -e "${key}" ]]; then
    local tmp="${secrets}/.save-recovery-mac-key.$$.$RANDOM.tmp"
    if command -v openssl >/dev/null 2>&1; then openssl rand -hex 32 >"${tmp}"; else od -An -N32 -tx1 /dev/urandom | tr -d ' \n' >"${tmp}"; printf '\n' >>"${tmp}"; fi
    chmod 600 "${tmp}"
    if ! ln "${tmp}" "${key}" 2>/dev/null; then [[ -f "${key}" ]] || { rm -f "${tmp}"; return 1; }; fi
    rm -f "${tmp}"
  fi
  assert_not_link "${key}"; chmod 600 "${key}"
  [[ "$(permission_mode "${key}")" == 600 ]] || { echo "Unsafe key permissions." >&2; return 1; }
  local value; value="$(tr -d '\r\n' <"${key}")"
  [[ "${value}" =~ ^[0-9a-f]{64}$ ]] || { echo "Local save-recovery key is invalid." >&2; return 1; }
  unset value
}

run_self_test() {
  local root; root="$(mktemp -d "${TMPDIR:-/tmp}/mir2-local-save-recovery-selftest.XXXXXX")"
  self_test_root="${root}"
  trap 'rm -rf "$self_test_root"' EXIT
  initialize "${root}"
  local first; first="$(cat "${root}/.mir2-data/local-secrets/save-recovery-mac-key.hex")"
  initialize "${root}" & local p1=$!
  initialize "${root}" & local p2=$!
  wait "${p1}"; wait "${p2}"
  [[ "$(cat "${root}/.mir2-data/local-secrets/save-recovery-mac-key.hex")" == "${first}" ]]
  printf invalid >"${root}/.mir2-data/local-secrets/save-recovery-mac-key.hex"
  if initialize "${root}" >/dev/null 2>&1; then echo "Invalid key was accepted." >&2; return 1; fi
  if ln -s "${root}" "${root}/linked" 2>/dev/null; then
    if assert_not_link "${root}/linked" >/dev/null 2>&1; then echo "Symlink was accepted." >&2; return 1; fi
  fi
  echo "SAVE-RECOVERY-LAUNCH-LOCAL Unix selftest: PASS"
}

if [[ "${self_test}" -eq 1 ]]; then run_self_test; else initialize "${project_root}"; [[ "${quiet}" -eq 1 ]] || echo "Local save-recovery secret is ready."; fi
