#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALLER="$ROOT/scripts/install-gateway-release.sh"
PACKAGER="$ROOT/scripts/package-gateway-release.sh"
ENV_TEMPLATE="$ROOT/infra/systemd/mir2-gateway.env.example"
SERVICE_UNIT="$ROOT/infra/systemd/mir2-gateway.service"

test_tmp="$(mktemp -d "${TMPDIR:-/tmp}/mir2-gateway-security-selftest.XXXXXX")"
package_test_stage_name=""
cleanup_selftest() {
  local status="$?"
  trap - EXIT
  if [ -n "$package_test_stage_name" ]; then
    bash "$PACKAGER" --selftest-package-cleanup-stage \
      "$package_test_stage_name" >/dev/null 2>&1 || true
  fi
  rm -rf "$test_tmp"
  exit "$status"
}
trap cleanup_selftest EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

run_success() {
  local label="$1"
  local log_file="$2"
  shift 2
  if ! "$@" >"$log_file" 2>&1; then
    fail "$label"
  fi
}

run_rejected() {
  local label="$1"
  local log_file="$2"
  shift 2
  if "$@" >"$log_file" 2>&1; then
    fail "$label"
  fi
}

require_literal() {
  local file="$1"
  local needle="$2"
  local label="$3"
  grep -Fq -- "$needle" "$file" || fail "$label"
}

ban_literal() {
  local file="$1"
  local needle="$2"
  local label="$3"
  if grep -Fq -- "$needle" "$file"; then
    fail "$label"
  fi
}

ban_ere() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if grep -Eq -- "$pattern" "$file"; then
    fail "$label"
  fi
}

assert_log_omits() {
  local log_file="$1"
  local secret="$2"
  if grep -Fq -- "$secret" "$log_file"; then
    fail "command output exposed key material"
  fi
}

assert_credential_log_omits() {
  local log_file="$1"
  shift
  local secret
  for secret in "$@"; do
    assert_log_omits "$log_file" "$secret"
  done
}

wait_for_file() {
  local path="$1"
  local label="$2"
  local attempt
  for attempt in $(seq 1 200); do
    [ -f "$path" ] && return 0
    sleep 0.05
  done
  fail "$label"
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
generate_csprng_secret() {
  local label="$1"
  local bytes="${2:-32}"
  local needed=$((bytes * 2))
  local secret=""

  if command -v openssl >/dev/null 2>&1; then
    secret="$(openssl rand -hex "$bytes" 2>/dev/null || true)"
    if [ "${#secret}" -eq "$needed" ]; then
      printf '%s\n' "$secret"
      return 0
    fi
  fi

  if [ -r /dev/urandom ]; then
    secret="$(tr -dc '0123456789abcdef' </dev/urandom | head -c "$needed" || true)"
    if [ "${#secret}" -eq "$needed" ]; then
      printf '%s\n' "$secret"
      return 0
    fi
  fi

  fail "no available CSPRNG source for $label"
}


read_value() {
  local env_file="$1"
  local key="$2"
  local line
  local found=0

  parsed_value=""
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    case "$line" in
      "$key="*)
        found=$((found + 1))
        parsed_value="${line#*=}"
        ;;
    esac
  done < "$env_file"
  [ "$found" -eq 1 ] || fail "$key was not rendered exactly once"
}

platform="$(uname -s)"
if [ "${MIR2_REQUIRE_LINUX_SECURITY_GATE:-0}" = "1" ]; then
  case "$platform" in
    Linux*) ;;
    *) fail "Linux security gate was required on non-Linux platform $platform" ;;
  esac
fi
if [ "$EUID" -eq 0 ]; then
  fail "security selftest must run as a non-root account"
fi

test_python=""
for python_candidate in python3 python; do
  if command -v "$python_candidate" >/dev/null 2>&1; then
    python_candidate_path="$(command -v "$python_candidate")"
    if "$python_candidate_path" -I -c \
      'import sys; raise SystemExit(0 if sys.version_info.major == 3 else 1)' \
      >/dev/null 2>&1; then
      test_python="$python_candidate_path"
      break
    fi
  fi
done
[ -n "$test_python" ] || fail "a working Python 3 is required for security fixtures"

case "$platform" in
  Linux*)
    run_success "packager Linux platform gate rejected Linux" \
      "$test_tmp/package-platform.log" \
      bash "$PACKAGER" --selftest-package-platform-check
    ;;
  *)
    run_rejected "packager platform gate accepted non-Linux" \
      "$test_tmp/package-platform.log" \
      bash "$PACKAGER" --selftest-package-platform-check
    ;;
esac

# Static privileged architecture bans. Behavioral fixtures follow.
ban_ere "$INSTALLER" '(^|[^A-Za-z])sudo([^A-Za-z]|$)|sudo_cmd' \
  "installer still has a caller-to-root sudo bridge"
ban_literal "$INSTALLER" 'declare -f' \
  "installer still serializes caller code"
ban_literal "$INSTALLER" 'installed_helper' \
  "installer still executes an archive-installed helper"
ban_literal "$INSTALLER" 'tarfile' \
  "installer uses the unbounded Python tarfile parser"
ban_literal "$INSTALLER" '--max-filesize' \
  "installer still depends on curl --max-filesize semantics"
ban_literal "$INSTALLER" '--location' \
  "installer still permits HTTP redirects"
ban_literal "$INSTALLER" '--proto-redir' \
  "installer still contains redirect handling"
ban_ere "$INSTALLER" '(^|[[:space:]])tar[[:space:]]+(-[^[:space:]]*x|--extract)' \
  "installer still extracts with tar"
ban_ere "$INSTALLER" 'chown[[:space:]]+(-R|--recursive)' \
  "installer still contains broad recursive chown"
ban_ere "$PACKAGER" 'tar[[:space:]].*(-czf|-zcf).*(OUT_DIR|archive_path)' \
  "packager still creates a trusted archive by reopening an OUT_DIR path"
ban_literal "$PACKAGER" 'README.txt systemd scripts' \
  "packager still includes systemd/scripts in the archive"
ban_literal "$PACKAGER" 'cat > "$stage/RELEASE.json" <<JSON' \
  "packager still raw-interpolates JSON"

require_literal "$INSTALLER" '#!/bin/bash -p' \
  "production installer shebang does not enable Bash privileged mode"
require_literal "$INSTALLER" \
  'trusted_pin_path=/etc/mir2/gateway-release.pin' \
  "installer lacks the fixed root-owned pin path"
require_literal "$INSTALLER" \
  'trusted_installer_path=/usr/local/libexec/mir2/install-gateway-release.sh' \
  "installer lacks the fixed trusted executable path"
require_literal "$INSTALLER" 'file_stat.st_nlink != 1' \
  "root regular-file checks omit nlink=1"
require_literal "$INSTALLER" 'archive_stat.st_nlink != 1' \
  "archive source check omits nlink=1"
require_literal "$INSTALLER" 'FILE_CREATE_FLAGS' \
  "privileged destinations are not O_EXCL/O_NOFOLLOW"
require_literal "$INSTALLER" 'parse_archive_from_fd(' \
  "archive is not parsed and copied through an open FD"
require_literal "$INSTALLER" 'resource.RLIMIT_FSIZE' \
  "production downloader lacks a kernel file-size limit"
require_literal "$INSTALLER" 'stdout=output_fd' \
  "production downloader does not write through its pre-opened output FD"
require_literal "$INSTALLER" 'renameat2(RENAME_NOREPLACE)' \
  "release publication does not require atomic no-replace rename"
require_literal "$INSTALLER" 'install_root_fd = ensure_root_chain(INSTALL_ROOT)' \
  "install root is not held to the root-owned directory contract"
require_literal "$INSTALLER" 'ensure_atomic_root_file(' \
  "unit/env first creation does not use the atomic file transaction"
require_literal "$INSTALLER" 'atomic_root_file_test(' \
  "Linux fixtures cannot invoke the production atomic file transaction"
require_literal "$INSTALLER" 'sweep_download_residues(' \
  "download startup path omits bounded stale-residue sweeping"
require_literal "$INSTALLER" 'sweep_unpublished_releases(' \
  "release startup path omits bounded incoming-residue sweeping"
require_literal "$INSTALLER" 'validate_production_identity()' \
  "production identity path does not use the local/NSS validator"
require_literal "$INSTALLER" 'validate_nsswitch_files_only' \
  "production identity path does not enforce files-only NSS"
require_literal "$INSTALLER" '("/etc/gshadow", 4_194_304, False)' \
  "production identity preflight omits the local gshadow database"
require_literal "$INSTALLER" 'SYS_UID_MIN' \
  "production identity preflight omits trusted login.defs ranges"
require_literal "$INSTALLER" 'id_all_gids != str(gid)' \
  "production identity preflight permits supplementary groups"
require_literal "$INSTALLER" 'mir2 appears in another local gshadow record' \
  "production identity preflight does not scan every gshadow record"
require_literal "$INSTALLER" 'validate_activation_credentials(gateway_env)' \
  "activation path omits database/cache credential preflight"
require_literal "$INSTALLER" 'sanitize_exported_environment' \
  "root entry lacks environment allowlist sanitization"
require_literal "$INSTALLER" 'SERVICE_UNIT = b"""[Unit]' \
  "systemd unit is not generated from the trusted installer contract"
require_literal "$PACKAGER" '"archiveContainsInstaller": False' \
  "package metadata does not exclude installer code"
require_literal "$PACKAGER" 'json.dump(metadata' \
  "package metadata is not JSON-encoded"
require_literal "$PACKAGER" 'MIR2_RELEASE_TARGET is not an allowlisted Linux target' \
  "packager target allowlist is absent"
require_literal "$PACKAGER" '"kernelDownloadFileSizeLimitRequired": True' \
  "package contract omits the kernel download limit"
require_literal "$PACKAGER" '"atomicNoReplacePublicationRequired": True' \
  "package contract omits atomic no-replace publication"
require_literal "$PACKAGER" 'archive_sha = hash_fd(archive_fd)' \
  "packager does not hash the archive through its held creation FD"
require_literal "$PACKAGER" 'mode != 0o700' \
  "packager does not require a private output directory"
require_literal "$PACKAGER" 'marker_owner_is_live(marker)' \
  "packager does not sweep dead-owner publication residues"
require_literal "$PACKAGER" 'MIR2_RELEASE_PUBLISHER_UID' \
  "packager lacks an explicit publisher UID trust boundary"

run_success "allowlisted target/tag token was rejected" \
  "$test_tmp/valid-package-token.log" \
  bash "$PACKAGER" --selftest-validate-token \
  x86_64-unknown-linux-gnu gateway-2026.08.24
run_rejected "target ../../ escape reached build handling" \
  "$test_tmp/unsafe-package-target.log" \
  bash "$PACKAGER" --selftest-validate-token '../../escape' gateway-1
run_rejected "tag ../../ escape was accepted" \
  "$test_tmp/unsafe-package-tag.log" \
  bash "$PACKAGER" --selftest-validate-token \
  x86_64-unknown-linux-gnu '../../escape'
run_rejected "reserved incoming release tag was accepted" \
  "$test_tmp/reserved-package-tag.log" \
  bash "$PACKAGER" --selftest-validate-token \
  x86_64-unknown-linux-gnu 'incoming.attacker'

[ "$(bash "$INSTALLER" --selftest-reserved-prefix-owner 60000 0)" = "ignore" ] ||
  fail "foreign-UID /var/tmp reserved-prefix entry is not ignored"
[ "$(bash "$INSTALLER" --selftest-reserved-prefix-owner 0 0)" = "owned" ] ||
  fail "root-owned /var/tmp reserved-prefix entry escapes strict validation"

rendered_service="$test_tmp/mir2-gateway.service"
run_success "trusted service contract could not be rendered" \
  "$test_tmp/render-service.log" \
  bash "$INSTALLER" --selftest-render-service "$rendered_service"
normalized_service="$test_tmp/mir2-gateway.service.lf"
sed 's/\r$//' "$SERVICE_UNIT" > "$normalized_service"
cmp -s "$rendered_service" "$normalized_service" ||
  fail "repository systemd unit differs from fixed trusted installer bytes"
for hardening_line in \
  'PrivateDevices=true' \
  'ProtectProc=invisible' \
  'RestrictNamespaces=true' \
  'SystemCallFilter=@system-service' \
  'InaccessiblePaths=/etc/shadow /etc/gshadow' \
  'RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6' \
  'LimitCORE=0' \
  'TasksMax=4096'; do
  grep -Fxq -- "$hardening_line" "$rendered_service" ||
    fail "trusted systemd unit lacks $hardening_line"
done

run_success "empty caller release authority was rejected" \
  "$test_tmp/no-caller-authority.log" \
  bash "$INSTALLER" --selftest-reject-caller-authority
attacker_sha='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
run_rejected "matching caller URL+SHA substitution was accepted as authority" \
  "$test_tmp/caller-authority.log" \
  env \
  MIR2_GATEWAY_RELEASE_URL=https://attacker.invalid/release.tar.gz \
  MIR2_GATEWAY_RELEASE_SHA256="$attacker_sha" \
  MIR2_GATEWAY_RELEASE_TAG=attacker-release \
  bash "$INSTALLER" --selftest-reject-caller-authority

run_success "unsafe root-entry environment survived allowlist sanitization" \
  "$test_tmp/environment-sanitize.log" \
  env \
  SYSTEMD_BUS_ADDRESS=unix:path=/tmp/attacker-systemd \
  DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/attacker-dbus \
  LD_LIBRARY_PATH="$test_tmp/attacker-libs" \
  PYTHONPATH="$test_tmp/attacker-python" \
  CURL_HOME="$test_tmp/attacker-curl" \
  HTTPS_PROXY=http://attacker.invalid:8080 \
  bash "$INSTALLER" --selftest-sanitize-environment

make_identity_fixture() {
  local fixture_dir="$1"
  mkdir -p "$fixture_dir"
  printf '%s\n' \
    'root:x:0:0:root:/root:/bin/bash' \
    'mir2:x:991:991:Mir2 service:/var/lib/mir2/gateway-data:/usr/sbin/nologin' \
    > "$fixture_dir/passwd"
  printf '%s\n' 'root:x:0:' 'mir2:x:991:' > "$fixture_dir/group"
  printf '%s\n' \
    'root:!:20000:0:99999:7:::' \
    'mir2:!:20000:0:99999:7:::' > "$fixture_dir/shadow"
  printf '%s\n' 'root:!::' 'mir2:!::' > "$fixture_dir/gshadow"
  printf '%s\n' \
    'SYS_UID_MIN 100' 'SYS_UID_MAX 999' \
    'SYS_GID_MIN 100' 'SYS_GID_MAX 999' > "$fixture_dir/login.defs"
  printf '%s\n' \
    'passwd: files' 'group: files' 'shadow: files' 'gshadow: files' \
    > "$fixture_dir/nsswitch.conf"
  cp "$fixture_dir/passwd" "$fixture_dir/nss-passwd"
  cp "$fixture_dir/group" "$fixture_dir/nss-group"
}

validate_identity_fixture() {
  local fixture_dir="$1"
  local getent_shadow="${2:-mir2:!:20000:0:99999:7:::}"
  local getent_gshadow="${3:-mir2:!::}"
  local id_uid="${4:-991}"
  local id_gid="${5:-991}"
  local id_primary="${6:-mir2}"
  local id_groups="${7:-991}"
  local shadow_file_gid="${8:-0}"
  local gshadow_file_gid="${9:-0}"
  bash "$INSTALLER" --selftest-validate-identity \
    "$fixture_dir/passwd" "$fixture_dir/group" \
    "$fixture_dir/shadow" "$fixture_dir/gshadow" \
    "$fixture_dir/login.defs" "$fixture_dir/nsswitch.conf" \
    "$fixture_dir/nss-passwd" "$fixture_dir/nss-group" \
    "$getent_shadow" "$getent_gshadow" \
    "$id_uid" "$id_gid" "$id_primary" "$id_groups" \
    "$shadow_file_gid" "$gshadow_file_gid"
}

identity_valid="$test_tmp/identity-valid"
make_identity_fixture "$identity_valid"
run_success "strict local/NSS mir2 identity was rejected" \
  "$test_tmp/identity-valid.log" \
  validate_identity_fixture "$identity_valid"

identity_shell="$test_tmp/identity-shell"
make_identity_fixture "$identity_shell"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/bash' \
  'mir2:x:991:991:Mir2:/var/lib/mir2/gateway-data:/bin/bash' \
  > "$identity_shell/passwd"
cp "$identity_shell/passwd" "$identity_shell/nss-passwd"
run_rejected "interactive mir2 shell was accepted" \
  "$test_tmp/identity-shell.log" validate_identity_fixture "$identity_shell"

identity_home="$test_tmp/identity-home"
make_identity_fixture "$identity_home"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/bash' \
  'mir2:x:991:991:Mir2:/home/mir2:/usr/sbin/nologin' \
  > "$identity_home/passwd"
cp "$identity_home/passwd" "$identity_home/nss-passwd"
run_rejected "reused mir2 account home was accepted" \
  "$test_tmp/identity-home.log" validate_identity_fixture "$identity_home"

identity_passwd_hash="$test_tmp/identity-passwd-hash"
make_identity_fixture "$identity_passwd_hash"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/bash' \
  'mir2:$6$attacker-passwd-hash:991:991:Mir2:/var/lib/mir2/gateway-data:/usr/sbin/nologin' \
  > "$identity_passwd_hash/passwd"
cp "$identity_passwd_hash/passwd" "$identity_passwd_hash/nss-passwd"
run_rejected "passwd hash was accepted as a lock marker" \
  "$test_tmp/identity-passwd-hash.log" \
  validate_identity_fixture "$identity_passwd_hash"
assert_log_omits "$test_tmp/identity-passwd-hash.log" '$6$attacker-passwd-hash'

identity_passwd_empty="$test_tmp/identity-passwd-empty"
make_identity_fixture "$identity_passwd_empty"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/bash' \
  'mir2::991:991:Mir2:/var/lib/mir2/gateway-data:/usr/sbin/nologin' \
  > "$identity_passwd_empty/passwd"
cp "$identity_passwd_empty/passwd" "$identity_passwd_empty/nss-passwd"
run_rejected "empty passwd marker was accepted" \
  "$test_tmp/identity-passwd-empty.log" \
  validate_identity_fixture "$identity_passwd_empty"

identity_shadow="$test_tmp/identity-shadow"
make_identity_fixture "$identity_shadow"
printf '%s\n' \
  'root:!:20000:0:99999:7:::' \
  'mir2:$6$attacker-shadow-hash:20000:0:99999:7:::' \
  > "$identity_shadow/shadow"
run_rejected "unlocked shadow password was accepted" \
  "$test_tmp/identity-shadow.log" \
  validate_identity_fixture "$identity_shadow" \
  'mir2:$6$attacker-shadow-hash:20000:0:99999:7:::'
assert_log_omits "$test_tmp/identity-shadow.log" '$6$attacker-shadow-hash'

identity_group_password="$test_tmp/identity-group-password"
make_identity_fixture "$identity_group_password"
printf '%s\n' 'root:x:0:' 'mir2:$6$group-hash:991:' \
  > "$identity_group_password/group"
cp "$identity_group_password/group" "$identity_group_password/nss-group"
run_rejected "group password hash was accepted as a lock marker" \
  "$test_tmp/identity-group-password.log" \
  validate_identity_fixture "$identity_group_password"
assert_log_omits "$test_tmp/identity-group-password.log" '$6$group-hash'

identity_group_empty="$test_tmp/identity-group-empty"
make_identity_fixture "$identity_group_empty"
printf '%s\n' 'root:x:0:' 'mir2::991:' > "$identity_group_empty/group"
cp "$identity_group_empty/group" "$identity_group_empty/nss-group"
run_rejected "empty group password marker was accepted" \
  "$test_tmp/identity-group-empty.log" \
  validate_identity_fixture "$identity_group_empty"

identity_group_member="$test_tmp/identity-group-member"
make_identity_fixture "$identity_group_member"
printf '%s\n' 'root:x:0:' 'mir2:x:991:attacker' \
  > "$identity_group_member/group"
cp "$identity_group_member/group" "$identity_group_member/nss-group"
run_rejected "mir2 primary group member was accepted" \
  "$test_tmp/identity-group-member.log" \
  validate_identity_fixture "$identity_group_member"

identity_gshadow="$test_tmp/identity-gshadow"
make_identity_fixture "$identity_gshadow"
printf '%s\n' 'root:!::' 'mir2:$6$gshadow-hash::' \
  > "$identity_gshadow/gshadow"
run_rejected "unlocked gshadow password was accepted" \
  "$test_tmp/identity-gshadow.log" \
  validate_identity_fixture "$identity_gshadow" \
  'mir2:!:20000:0:99999:7:::' 'mir2:$6$gshadow-hash::'
assert_log_omits "$test_tmp/identity-gshadow.log" '$6$gshadow-hash'

identity_gshadow_member="$test_tmp/identity-gshadow-member"
make_identity_fixture "$identity_gshadow_member"
printf '%s\n' 'root:!::' 'mir2:!:attacker:attacker' \
  > "$identity_gshadow_member/gshadow"
run_rejected "gshadow administrators/members were accepted" \
  "$test_tmp/identity-gshadow-member.log" \
  validate_identity_fixture "$identity_gshadow_member" \
  'mir2:!:20000:0:99999:7:::' 'mir2:!:attacker:attacker'

identity_other_gshadow="$test_tmp/identity-other-gshadow"
make_identity_fixture "$identity_other_gshadow"
printf '%s\n' 'root:!::' 'operators:!:mir2:' 'mir2:!::' \
  > "$identity_other_gshadow/gshadow"
run_rejected "mir2 as administrator of another gshadow group was accepted" \
  "$test_tmp/identity-other-gshadow.log" \
  validate_identity_fixture "$identity_other_gshadow"

identity_private_gid="$test_tmp/identity-private-gid"
make_identity_fixture "$identity_private_gid"
run_rejected "shadow root:mir2 private-file access was accepted" \
  "$test_tmp/identity-private-gid.log" \
  validate_identity_fixture "$identity_private_gid" \
  'mir2:!:20000:0:99999:7:::' 'mir2:!::' 991 991 mir2 991 991 0

identity_remote_nss="$test_tmp/identity-remote-nss"
make_identity_fixture "$identity_remote_nss"
printf '%s\n' \
  'passwd: files ldap' 'group: files' 'shadow: files' 'gshadow: files' \
  > "$identity_remote_nss/nsswitch.conf"
run_rejected "non-enumerating remote NSS backend was accepted" \
  "$test_tmp/identity-remote-nss.log" \
  validate_identity_fixture "$identity_remote_nss"

identity_uid_collision="$test_tmp/identity-uid-collision"
make_identity_fixture "$identity_uid_collision"
printf '%s\n' \
  'collision:x:991:992:collision:/nonexistent:/usr/sbin/nologin' \
  >> "$identity_uid_collision/passwd"
cp "$identity_uid_collision/passwd" "$identity_uid_collision/nss-passwd"
run_rejected "shared mir2 UID was accepted" \
  "$test_tmp/identity-uid-collision.log" \
  validate_identity_fixture "$identity_uid_collision"

identity_gid_collision="$test_tmp/identity-gid-collision"
make_identity_fixture "$identity_gid_collision"
printf '%s\n' 'shared:x:991:' >> "$identity_gid_collision/group"
cp "$identity_gid_collision/group" "$identity_gid_collision/nss-group"
run_rejected "shared mir2 GID was accepted" \
  "$test_tmp/identity-gid-collision.log" \
  validate_identity_fixture "$identity_gid_collision"

identity_primary_gid_collision="$test_tmp/identity-primary-gid-collision"
make_identity_fixture "$identity_primary_gid_collision"
printf '%s\n' \
  'collision:x:992:991:collision:/nonexistent:/usr/sbin/nologin' \
  >> "$identity_primary_gid_collision/passwd"
cp "$identity_primary_gid_collision/passwd" \
  "$identity_primary_gid_collision/nss-passwd"
run_rejected "another account using the mir2 primary GID was accepted" \
  "$test_tmp/identity-primary-gid-collision.log" \
  validate_identity_fixture "$identity_primary_gid_collision"

identity_nss_duplicate="$test_tmp/identity-nss-duplicate"
make_identity_fixture "$identity_nss_duplicate"
printf '%s\n' \
  'mir2:x:992:992:remote:/remote:/usr/sbin/nologin' \
  >> "$identity_nss_duplicate/nss-passwd"
run_rejected "duplicate/remote NSS mir2 record was accepted" \
  "$test_tmp/identity-nss-duplicate.log" \
  validate_identity_fixture "$identity_nss_duplicate"

identity_nss_mismatch="$test_tmp/identity-nss-mismatch"
make_identity_fixture "$identity_nss_mismatch"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/bash' \
  'mir2:x:991:991:remote:/remote:/usr/sbin/nologin' \
  > "$identity_nss_mismatch/nss-passwd"
run_rejected "NSS mir2 record differing from local source was accepted" \
  "$test_tmp/identity-nss-mismatch.log" \
  validate_identity_fixture "$identity_nss_mismatch"

identity_nss_member="$test_tmp/identity-nss-member"
make_identity_fixture "$identity_nss_member"
printf '%s\n' 'admin:x:27:mir2' >> "$identity_nss_member/nss-group"
run_rejected "NSS supplementary member record was accepted" \
  "$test_tmp/identity-nss-member.log" \
  validate_identity_fixture "$identity_nss_member"

identity_range="$test_tmp/identity-range"
make_identity_fixture "$identity_range"
printf '%s\n' \
  'SYS_UID_MIN 100' 'SYS_UID_MAX 500' \
  'SYS_GID_MIN 100' 'SYS_GID_MAX 500' > "$identity_range/login.defs"
run_rejected "UID/GID outside trusted login.defs ranges were accepted" \
  "$test_tmp/identity-range.log" validate_identity_fixture "$identity_range"

run_rejected "mir2 supplementary admin group was accepted" \
  "$test_tmp/identity-supplementary.log" \
  validate_identity_fixture "$identity_valid" \
  'mir2:!:20000:0:99999:7:::' 'mir2:!::' 991 991 mir2 '991 27'
run_rejected "wrong mir2 primary group name was accepted" \
  "$test_tmp/identity-primary.log" \
  validate_identity_fixture "$identity_valid" \
  'mir2:!:20000:0:99999:7:::' 'mir2:!::' 991 991 users 991

trusted_sha='0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
pin_file="$test_tmp/gateway-release.pin"
printf '%s\n' \
  'MIR2_GATEWAY_RELEASE_URL=https://releases.example.invalid/gateway.tar.gz' \
  "MIR2_GATEWAY_RELEASE_SHA256=$trusted_sha" \
  'MIR2_GATEWAY_RELEASE_TAG=gateway-2026.08.24' > "$pin_file"
pin_output="$(
  bash "$INSTALLER" --selftest-read-pin "$pin_file"
)" || fail "valid fixed pin syntax was rejected"
expected_pin_output="$(printf '%s\n%s\n%s' \
  'https://releases.example.invalid/gateway.tar.gz' \
  "$trusted_sha" \
  'gateway-2026.08.24')"
[ "$pin_output" = "$expected_pin_output" ] ||
  fail "pin parser did not return only the fixed manifest authority"

run_success "valid HTTPS URL was rejected" "$test_tmp/valid-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'https://releases.example.invalid/gateway.tar.gz'
run_rejected "HTTP release URL was accepted" "$test_tmp/http-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'http://releases.example.invalid/gateway.tar.gz'
run_rejected "option-like curl URL was accepted" "$test_tmp/option-url.log" \
  bash "$INSTALLER" --selftest-validate-url '-K/tmp/attacker.curlrc'
run_rejected "credential-bearing release URL was accepted" \
  "$test_tmp/credential-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'https://user:password@releases.example.invalid/gateway.tar.gz'
assert_log_omits "$test_tmp/credential-url.log" 'password'
run_rejected "fragment-bearing release URL was accepted" \
  "$test_tmp/fragment-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'https://releases.example.invalid/gateway.tar.gz#ignored'
run_rejected "query-bearing release URL was accepted" \
  "$test_tmp/query-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'https://releases.example.invalid/gateway.tar.gz?mirror=attacker'
run_rejected "empty query delimiter was accepted" \
  "$test_tmp/empty-query-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'https://releases.example.invalid/gateway.tar.gz?'
run_rejected "empty fragment delimiter was accepted" \
  "$test_tmp/empty-fragment-url.log" \
  bash "$INSTALLER" --selftest-validate-url \
  'https://releases.example.invalid/gateway.tar.gz#'

curl_home="$test_tmp/curl-home"
mkdir -p "$curl_home"
printf '%s\n' '--output /tmp/curlrc-was-loaded' > "$curl_home/.curlrc"
curl_argv_log="$test_tmp/curl-argv.log"
run_success "safe curl argv construction failed" "$curl_argv_log" \
  env CURL_HOME="$curl_home" \
  bash "$INSTALLER" --selftest-curl-argv \
  'https://releases.example.invalid/gateway.tar.gz'
mapfile -t curl_args < "$curl_argv_log"
[ "${curl_args[0]:-}" = "/usr/bin/curl" ] ||
  fail "curl argv does not use the fixed binary"
[ "${curl_args[1]:-}" = "-q" ] ||
  fail "curl -q is not the first curl option"
last_index=$(("${#curl_args[@]}" - 1))
[ "${curl_args[last_index-1]:-}" = "--" ] &&
  [ "${curl_args[last_index]:-}" = \
    'https://releases.example.invalid/gateway.tar.gz' ] ||
  fail "curl URL is not isolated after --"
grep -Fxq -- '--proto' "$curl_argv_log" ||
  fail "curl argv lacks protocol restriction"
grep -Fxq -- '--connect-timeout' "$curl_argv_log" ||
  fail "curl argv lacks connect timeout"
grep -Fxq -- '--max-time' "$curl_argv_log" ||
  fail "curl argv lacks total timeout"
if grep -Eq -- '^--(location|proto-redir|max-filesize|output)$' "$curl_argv_log"; then
  fail "curl argv contains redirect, version-dependent size, or path output handling"
fi
if grep -Fq -- 'curlrc-was-loaded' "$curl_argv_log"; then
  fail "curlrc content influenced curl argv construction"
fi

env_file="$test_tmp/gateway.env"
run_success "first-install env render failed" "$test_tmp/render-env.log" \
  bash "$INSTALLER" --selftest-render-env "$ENV_TEMPLATE" "$env_file"
read_value "$env_file" MIR2_SAVE_RECOVERY_MAC_KEY
recovery_key="$parsed_value"
[[ "$recovery_key" =~ ^[0-9a-f]{64}$ ]] ||
  fail "first install did not generate a 32-byte recovery key"
assert_log_omits "$test_tmp/render-env.log" "$recovery_key"
read_value "$env_file" MIR2_PASSKEY_AUTH_SECRET
passkey_secret="$parsed_value"
read_value "$env_file" MIR2_IDENTITY_SESSION_SECRET
identity_secret="$parsed_value"
read_value "$env_file" MIR2_IDENTITY_RECOVERY_PEPPER
identity_pepper="$parsed_value"
if [ "$recovery_key" = "$passkey_secret" ] ||
  [ "$recovery_key" = "$identity_secret" ] ||
  [ "$recovery_key" = "$identity_pepper" ]; then
  fail "recovery key was reused for another installer secret"
fi

cp "$env_file" "$test_tmp/gateway.env.snapshot"
run_success "existing env was rejected or overwritten during repeat render" \
  "$test_tmp/repeat-render.log" \
  bash "$INSTALLER" --selftest-render-env "$ENV_TEMPLATE" "$env_file"
cmp -s "$env_file" "$test_tmp/gateway.env.snapshot" ||
  fail "repeat render rotated the existing recovery key"
assert_log_omits "$test_tmp/repeat-render.log" "$recovery_key"
run_success "valid existing recovery env was rejected" \
  "$test_tmp/validate-env.log" \
  bash "$INSTALLER" --selftest-validate-env "$env_file"
cmp -s "$env_file" "$test_tmp/gateway.env.snapshot" ||
  fail "recovery validation changed the existing env"

run_rejected "activation accepted template database/cache placeholders" \
  "$test_tmp/activation-placeholder.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$env_file"
assert_credential_log_omits "$test_tmp/activation-placeholder.log" \
  "replace-with-private-db-password" "replace-with-private-redis-password"

activation_env="$test_tmp/activation.env"
cp "$env_file" "$activation_env"
database_password="$(generate_csprng_secret "postgres authentication secret")"
redis_password="$(generate_csprng_secret "redis authentication secret")"
operator_token="$(generate_csprng_secret "admin operator token")"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:$database_password@127.0.0.1:5432/mir2#" \
  "$activation_env"
sed -i \
  "s#^MIR2_GATEWAY_REDIS_CACHE_URL=.*#MIR2_GATEWAY_REDIS_CACHE_URL=redis://:$redis_password@127.0.0.1:6379#" \
  "$activation_env"
printf '%s\n' "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=$operator_token" >> "$activation_env"
run_success "strong authenticated database/cache activation was accepted" \
  "$test_tmp/activation-valid.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_env"
assert_log_omits "$test_tmp/activation-valid.log" "$database_password"
assert_log_omits "$test_tmp/activation-valid.log" "$redis_password"
assert_log_omits "$test_tmp/activation-valid.log" "$operator_token"

reject_activation_value() {
  local key="$1"
  local value="$2"
  local label="$3"
  local slug="$4"
  local candidate="$test_tmp/activation-$slug.env"
  local replacement

  cp "$activation_env" "$candidate"
  case "$key" in
    MIR2_ACCOUNT_STORE_DATABASE_URL)
      replacement="MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:$value@127.0.0.1:5432/mir2"
      ;;
    MIR2_GATEWAY_REDIS_CACHE_URL)
      replacement="MIR2_GATEWAY_REDIS_CACHE_URL=redis://:$value@127.0.0.1:6379"
      ;;
    MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN)
      replacement="MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=$value"
      ;;
    *)
      fail "unsupported activation credential key: $key"
      ;;
  esac
  sed -i "s#^$key=.*#$replacement#" "$candidate"
  local log_file="$test_tmp/activation-$slug.log"
  run_rejected "$label" "$log_file" \
    bash "$INSTALLER" --selftest-validate-activation-env "$candidate"
  assert_log_omits "$log_file" "$value"
}
activation_db_seq_1="$test_tmp/activation-db-seq-123.env"
cp "$activation_env" "$activation_db_seq_1"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef@127.0.0.1:5432/mir2#" \
  "$activation_db_seq_1"
run_rejected "activation accepted sequential 1234567890abcdef DB password" \
  "$test_tmp/activation-db-seq-123.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_seq_1"
assert_log_omits "$test_tmp/activation-db-seq-123.log" \
  "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

activation_db_seq_2="$test_tmp/activation-db-seq-fedcba.env"
cp "$activation_env" "$activation_db_seq_2"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321@127.0.0.1:5432/mir2#" \
  "$activation_db_seq_2"
run_rejected "activation accepted sequential fedcba0987654321 DB password" \
  "$test_tmp/activation-db-seq-fedcba.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_seq_2"
assert_log_omits "$test_tmp/activation-db-seq-fedcba.log" \
  "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321"

activation_db_seq_3="$test_tmp/activation-db-seq-0123456789abcdef.env"
cp "$activation_env" "$activation_db_seq_3"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef@127.0.0.1:5432/mir2#" \
  "$activation_db_seq_3"
run_rejected "activation accepted sequential 0123456789abcdef DB password" \
  "$test_tmp/activation-db-seq-0123456789abcdef.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_seq_3"
assert_log_omits "$test_tmp/activation-db-seq-0123456789abcdef.log" \
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

activation_db_keyboard="$test_tmp/activation-db-keyboard.env"
cp "$activation_env" "$activation_db_keyboard"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:qwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwerty@127.0.0.1:5432/mir2#" \
  "$activation_db_keyboard"
run_rejected "activation accepted keyboard-pattern DB password" \
  "$test_tmp/activation-db-keyboard.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_keyboard"
assert_log_omits "$test_tmp/activation-db-keyboard.log" \
  "qwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwerty"

activation_db_repeat="$test_tmp/activation-db-repeat.env"
cp "$activation_env" "$activation_db_repeat"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa@127.0.0.1:5432/mir2#" \
  "$activation_db_repeat"
run_rejected "activation accepted repeated DB password" \
  "$test_tmp/activation-db-repeat.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_repeat"
assert_log_omits "$test_tmp/activation-db-repeat.log" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

activation_db_placeholder="$test_tmp/activation-db-placeholder.env"
cp "$activation_env" "$activation_db_placeholder"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:replace-with-private-db-password@127.0.0.1:5432/mir2#" \
  "$activation_db_placeholder"
run_rejected "activation accepted placeholder DB password" \
  "$test_tmp/activation-db-placeholder.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_placeholder"
assert_log_omits "$test_tmp/activation-db-placeholder.log" \
  "replace-with-private-db-password"

# URL userinfo is decoded before the weak-pattern heuristic; malformed escapes
# fail closed, and short multi-character cycles are rejected as repetitions.
activation_db_url_encoded_weak="$test_tmp/activation-db-url-encoded-weak.env"
cp "$activation_env" "$activation_db_url_encoded_weak"
activation_db_url_encoded_weak_value="$(printf 'a%%61%.0s' {1..32})"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:$activation_db_url_encoded_weak_value@127.0.0.1:5432/mir2#" \
  "$activation_db_url_encoded_weak"
run_rejected "activation accepted URL-encoded weak DB password" \
  "$test_tmp/activation-db-url-encoded-weak.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_url_encoded_weak"
activation_db_url_encoded_weak_decoded_value="$(printf 'aa%.0s' {1..32})"
assert_credential_log_omits \
  "$test_tmp/activation-db-url-encoded-weak.log" \
  "$activation_db_url_encoded_weak_value" \
  "$activation_db_url_encoded_weak_decoded_value"

activation_db_malformed_percent="$test_tmp/activation-db-malformed-percent.env"
cp "$activation_env" "$activation_db_malformed_percent"
activation_db_malformed_percent_value='malformed%G1'
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:$activation_db_malformed_percent_value@127.0.0.1:5432/mir2#" \
  "$activation_db_malformed_percent"
run_rejected "activation accepted malformed DB userinfo percent escape" \
  "$test_tmp/activation-db-malformed-percent.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_malformed_percent"
assert_log_omits "$test_tmp/activation-db-malformed-percent.log" \
  "$activation_db_malformed_percent_value"

activation_db_short_cycle="$test_tmp/activation-db-short-cycle.env"
cp "$activation_env" "$activation_db_short_cycle"
activation_db_short_cycle_value="$(printf 'ab%.0s' {1..32})"
sed -i \
  "s#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:$activation_db_short_cycle_value@127.0.0.1:5432/mir2#" \
  "$activation_db_short_cycle"
run_rejected "activation accepted multi-character short-cycle DB password" \
  "$test_tmp/activation-db-short-cycle.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_db_short_cycle"
assert_log_omits "$test_tmp/activation-db-short-cycle.log" \
  "$activation_db_short_cycle_value"

activation_redis_seq="$test_tmp/activation-redis-seq-123.env"
cp "$activation_env" "$activation_redis_seq"
sed -i \
  "s#^MIR2_GATEWAY_REDIS_CACHE_URL=.*#MIR2_GATEWAY_REDIS_CACHE_URL=redis://:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef@127.0.0.1:6379#" \
  "$activation_redis_seq"
run_rejected "activation accepted sequential 1234567890abcdef Redis password" \
  "$test_tmp/activation-redis-seq-123.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_redis_seq"
assert_log_omits "$test_tmp/activation-redis-seq-123.log" \
  "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

activation_redis_repeat="$test_tmp/activation-redis-repeat.env"
cp "$activation_env" "$activation_redis_repeat"
sed -i \
  "s#^MIR2_GATEWAY_REDIS_CACHE_URL=.*#MIR2_GATEWAY_REDIS_CACHE_URL=redis://:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa@127.0.0.1:6379#" \
  "$activation_redis_repeat"
run_rejected "activation accepted repeated Redis password" \
  "$test_tmp/activation-redis-repeat.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_redis_repeat"
assert_log_omits "$test_tmp/activation-redis-repeat.log" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

activation_operator_fixed="$test_tmp/activation-operator-template.env"
cp "$activation_env" "$activation_operator_fixed"
sed -i \
  "s#^MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=.*#MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=replace-with-random-32-byte-gateway-operator-token#" \
  "$activation_operator_fixed"
run_rejected "activation accepted fixed template admin operator token" \
  "$test_tmp/activation-operator-template.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_operator_fixed"
assert_log_omits "$test_tmp/activation-operator-template.log" \
  "replace-with-random-32-byte-gateway-operator-token"
reject_activation_value "MIR2_GATEWAY_REDIS_CACHE_URL" \
  "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321" \
  "activation rejected sequential fedcba0987654321 Redis password" "redis-seq-fedcba"
reject_activation_value "MIR2_GATEWAY_REDIS_CACHE_URL" \
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" \
  "activation rejected sequential 0123456789abcdef Redis password" "redis-seq-hex"
reject_activation_value "MIR2_GATEWAY_REDIS_CACHE_URL" \
  "qwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwerty" \
  "activation rejected keyboard-pattern Redis password" "redis-keyboard"
reject_activation_value "MIR2_GATEWAY_REDIS_CACHE_URL" \
  "replace-with-private-redis-password" \
  "activation rejected placeholder Redis password" "redis-placeholder"
reject_activation_value "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" \
  "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" \
  "activation rejected sequential 1234567890abcdef admin token" "operator-seq-123"
reject_activation_value "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" \
  "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321" \
  "activation rejected sequential fedcba0987654321 admin token" "operator-seq-fedcba"
reject_activation_value "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" \
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" \
  "activation rejected sequential 0123456789abcdef admin token" "operator-seq-hex"
reject_activation_value "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" \
  "qwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwertyqwerty" \
  "activation rejected keyboard-pattern admin token" "operator-keyboard"
reject_activation_value "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" \
  "activation rejected repeated admin token" "operator-repeat"
reject_activation_value "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" \
  "replace-with-random-32-byte-gateway-operator-token" \
  "activation rejected placeholder admin token" "operator-placeholder"
activation_no_redis_auth="$test_tmp/activation-no-redis-auth.env"
cp "$activation_env" "$activation_no_redis_auth"
sed -i \
  's#^MIR2_GATEWAY_REDIS_CACHE_URL=.*#MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379#' \
  "$activation_no_redis_auth"
run_rejected "activation accepted unauthenticated Redis" \
  "$test_tmp/activation-no-redis-auth.log" \
  bash "$INSTALLER" --selftest-validate-activation-env \
  "$activation_no_redis_auth"
assert_credential_log_omits "$test_tmp/activation-no-redis-auth.log" \
  "$database_password" "$operator_token"

activation_weak_db="$test_tmp/activation-weak-db.env"
cp "$activation_env" "$activation_weak_db"
sed -i \
  's#^MIR2_ACCOUNT_STORE_DATABASE_URL=.*#MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:postgres@127.0.0.1:5432/mir2#' \
  "$activation_weak_db"
run_rejected "activation accepted a weak-pattern Postgres password" \
  "$test_tmp/activation-weak-db.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_weak_db"
assert_credential_log_omits "$test_tmp/activation-weak-db.log" \
  "postgres" "$redis_password" "$operator_token"

activation_operator_raw_percent="$test_tmp/activation-operator-raw-percent.env"
cp "$activation_env" "$activation_operator_raw_percent"
operator_raw_percent_value='A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6%20R7s8T9u0V1w2X3y4Z5'
sed -i \
  "s#^MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=.*#MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=$operator_raw_percent_value#" \
  "$activation_operator_raw_percent"
run_success "raw percent-bearing admin operator token was accepted" \
  "$test_tmp/activation-operator-raw-percent.log" \
  bash "$INSTALLER" --selftest-validate-activation-env "$activation_operator_raw_percent"
assert_log_omits "$test_tmp/activation-operator-raw-percent.log" \
  "$operator_raw_percent_value"

placeholder_env="$test_tmp/placeholder.env"
cp "$env_file" "$placeholder_env"
sed -i \
  's/^MIR2_SAVE_RECOVERY_MAC_KEY=.*/MIR2_SAVE_RECOVERY_MAC_KEY=replace-with-stable-independent-64-hex-secret/' \
  "$placeholder_env"
run_rejected "placeholder recovery key was accepted" \
  "$test_tmp/placeholder.log" \
  bash "$INSTALLER" --selftest-validate-env "$placeholder_env"
assert_log_omits "$test_tmp/placeholder.log" \
  'replace-with-stable-independent-64-hex-secret'
invalid_key='bad-recovery-key-material'
invalid_env="$test_tmp/invalid.env"
cp "$env_file" "$invalid_env"
sed -i \
  "s/^MIR2_SAVE_RECOVERY_MAC_KEY=.*/MIR2_SAVE_RECOVERY_MAC_KEY=$invalid_key/" \
  "$invalid_env"
run_rejected "malformed recovery key was accepted" \
  "$test_tmp/invalid.log" \
  bash "$INSTALLER" --selftest-validate-env "$invalid_env"
assert_log_omits "$test_tmp/invalid.log" "$invalid_key"
weak_key='deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef'
weak_env="$test_tmp/weak.env"
cp "$env_file" "$weak_env"
sed -i \
  "s/^MIR2_SAVE_RECOVERY_MAC_KEY=.*/MIR2_SAVE_RECOVERY_MAC_KEY=$weak_key/" \
  "$weak_env"
run_rejected "weak repeated recovery key was accepted" \
  "$test_tmp/weak.log" \
  bash "$INSTALLER" --selftest-validate-env "$weak_env"
assert_log_omits "$test_tmp/weak.log" "$weak_key"

truncated_env="$test_tmp/truncated.env"
sed '/^MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS=/d' "$env_file" > "$truncated_env"
run_rejected "truncated gateway env was accepted on a later preflight" \
  "$test_tmp/truncated-env.log" \
  bash "$INSTALLER" --selftest-validate-env "$truncated_env"

duplicate_env="$test_tmp/duplicate.env"
cp "$env_file" "$duplicate_env"
printf '%s\n' 'MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS=5000' >> "$duplicate_env"
run_rejected "duplicate gateway env field was accepted" \
  "$test_tmp/duplicate-env.log" \
  bash "$INSTALLER" --selftest-validate-env "$duplicate_env"

fixture_stage="$test_tmp/archive-stage"
mkdir -p "$fixture_stage"
printf '%s\n' 'gateway-fixture-binary' > "$fixture_stage/mir2-gateway"
printf '%s\n' 'zone-host-fixture-binary' > "$fixture_stage/zone_host"
printf '%s\n' 'fixture release readme' > "$fixture_stage/README.txt"
gateway_sha="$(sha256_file "$fixture_stage/mir2-gateway")"
zone_sha="$(sha256_file "$fixture_stage/zone_host")"
cat > "$fixture_stage/RELEASE.json" <<JSON
{
  "name": "mir2-gateway",
  "tag": "fixture-release",
  "target": "linux-x64",
  "binarySha256": "$gateway_sha",
  "zoneHostBinarySha256": "$zone_sha",
  "installation": {
    "requiresRootOwnedPinManifest": true,
    "checksumSidecarIsAuthority": false,
    "rootPinRehashFromArchiveFdRequired": true,
    "publisherUidTrustBoundaryRequired": true,
    "archiveContainsInstaller": false,
    "archiveContainsSystemdUnit": false,
    "archiveContainsEnvironmentTemplate": false,
    "kernelDownloadFileSizeLimitRequired": true,
    "redirectsAllowed": false,
    "atomicNoReplacePublicationRequired": true,
    "strictServiceIdentityRequired": true,
    "localNssIdentityConsistencyRequired": true,
    "filesOnlyNssIdentityRequired": true,
    "activationCredentialPreflightRequired": true,
    "sameFdArchiveDigestRequired": true,
    "privatePublisherTransactionsRequired": true,
    "crashConsistentArchiveSidecarRequired": true,
    "boundedResidueSweepRequired": true,
    "atomicConfigPublicationRequired": true
  }
}
JSON

valid_archive="$test_tmp/valid-release.tar.gz"
tar --format=ustar -C "$fixture_stage" -czf "$valid_archive" \
  mir2-gateway zone_host RELEASE.json README.txt
valid_archive_sha="$(sha256_file "$valid_archive")"
run_success "valid four-member archive was rejected" \
  "$test_tmp/valid-archive.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$valid_archive" "$valid_archive_sha" 1048576 1048576 4
printf '%s  %s\n' "$attacker_sha" "$(basename "$valid_archive")" \
  > "$valid_archive.sha256"
run_success "adjacent attacker sidecar overrode the independent pin digest" \
  "$test_tmp/sidecar-not-authority.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$valid_archive" "$valid_archive_sha" 1048576 1048576 4
run_rejected "archive hash mismatch was accepted" \
  "$test_tmp/archive-hash-mismatch.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$valid_archive" "$attacker_sha" 1048576 1048576 4

helper_stage="$test_tmp/helper-stage"
mkdir -p "$helper_stage"
cp "$fixture_stage/"* "$helper_stage/"
printf '%s\n' '#!/bin/bash' 'echo attacker' \
  > "$helper_stage/install-gateway-release.sh"
helper_archive="$test_tmp/malicious-helper.tar.gz"
tar --format=ustar -C "$helper_stage" -czf "$helper_archive" \
  mir2-gateway zone_host RELEASE.json README.txt install-gateway-release.sh
helper_sha="$(sha256_file "$helper_archive")"
run_rejected "archive-provided installer/helper was accepted" \
  "$test_tmp/malicious-helper.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$helper_archive" "$helper_sha" 1048576 1048576 8

unit_stage="$test_tmp/unit-stage"
mkdir -p "$unit_stage"
cp "$fixture_stage/"* "$unit_stage/"
printf '%s\n' '[Service]' 'ExecStart=/tmp/attacker' \
  > "$unit_stage/mir2-gateway.service"
unit_archive="$test_tmp/malicious-unit.tar.gz"
tar --format=ustar -C "$unit_stage" -czf "$unit_archive" \
  mir2-gateway zone_host RELEASE.json README.txt mir2-gateway.service
unit_sha="$(sha256_file "$unit_archive")"
run_rejected "archive-provided systemd unit was accepted" \
  "$test_tmp/malicious-unit.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$unit_archive" "$unit_sha" 1048576 1048576 8

link_stage="$test_tmp/link-stage"
mkdir -p "$link_stage"
cp "$fixture_stage/mir2-gateway" \
  "$fixture_stage/RELEASE.json" "$fixture_stage/README.txt" "$link_stage/"
MSYS=winsymlinks:nativestrict ln -s mir2-gateway "$link_stage/zone_host"
[ -L "$link_stage/zone_host" ] ||
  fail "archive symlink fixture could not be created"
link_archive="$test_tmp/symlink-member.tar.gz"
tar --format=ustar -C "$link_stage" -czf "$link_archive" \
  mir2-gateway zone_host RELEASE.json README.txt
link_sha="$(sha256_file "$link_archive")"
run_rejected "archive symlink member was accepted" \
  "$test_tmp/symlink-member.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$link_archive" "$link_sha" 1048576 1048576 4

pax_archive="$test_tmp/pax-header.tar.gz"
"$test_python" -I - "$fixture_stage" "$pax_archive" <<'PY'
import os
import sys
import tarfile

stage, output = sys.argv[1:]
names = ("mir2-gateway", "zone_host", "RELEASE.json", "README.txt")
with tarfile.open(output, "w:gz", format=tarfile.PAX_FORMAT) as bundle:
    for name in names:
        path = os.path.join(stage, name)
        info = bundle.gettarinfo(path, arcname=name)
        if name == "README.txt":
            info.pax_headers = {"comment": "x" * 4096}
        with open(path, "rb") as source:
            bundle.addfile(info, source)
PY
pax_sha="$(sha256_file "$pax_archive")"
run_rejected "PAX/extended header archive was accepted" \
  "$test_tmp/pax-header.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$pax_archive" "$pax_sha" 1048576 1048576 8

"$test_python" -I - "$fixture_stage" "$test_tmp" <<'PY'
import io
import os
import sys
import tarfile

stage, output_root = sys.argv[1:]
members = ("mir2-gateway", "zone_host", "RELEASE.json", "README.txt")
payloads = {
    name: open(os.path.join(stage, name), "rb").read()
    for name in members
}


def add_regular(bundle, archive_name, source_name):
    data = payloads[source_name]
    info = tarfile.TarInfo(archive_name)
    info.size = len(data)
    info.mode = 0o755 if source_name in {"mir2-gateway", "zone_host"} else 0o644
    bundle.addfile(info, io.BytesIO(data))


def build(name, format_value, writer):
    path = os.path.join(output_root, name)
    with tarfile.open(path, "w:gz", format=format_value) as bundle:
        writer(bundle)


build(
    "traversal-member.tar.gz",
    tarfile.USTAR_FORMAT,
    lambda bundle: (
        add_regular(bundle, "mir2-gateway", "mir2-gateway"),
        add_regular(bundle, "zone_host", "zone_host"),
        add_regular(bundle, "RELEASE.json", "RELEASE.json"),
        add_regular(bundle, "../README.txt", "README.txt"),
    ),
)


def duplicate_writer(bundle):
    for member in members:
        add_regular(bundle, member, member)
    add_regular(bundle, "README.txt", "README.txt")


build("duplicate-member.tar.gz", tarfile.USTAR_FORMAT, duplicate_writer)


def special_writer(type_value):
    def writer(bundle):
        for member in members[:3]:
            add_regular(bundle, member, member)
        info = tarfile.TarInfo("README.txt")
        info.type = type_value
        info.mode = 0o644
        if type_value == tarfile.CHRTYPE:
            info.devmajor = 1
            info.devminor = 3
        bundle.addfile(info)
    return writer


build("fifo-member.tar.gz", tarfile.USTAR_FORMAT, special_writer(tarfile.FIFOTYPE))
build("device-member.tar.gz", tarfile.USTAR_FORMAT, special_writer(tarfile.CHRTYPE))
build(
    "sparse-member.tar.gz",
    tarfile.GNU_FORMAT,
    special_writer(tarfile.GNUTYPE_SPARSE),
)


def longname_writer(bundle):
    for member in members[:3]:
        add_regular(bundle, member, member)
    add_regular(bundle, "x" * 140, "README.txt")


build("gnu-longname.tar.gz", tarfile.GNU_FORMAT, longname_writer)
PY

for malicious_case in \
  traversal-member duplicate-member fifo-member device-member \
  sparse-member gnu-longname; do
  malicious_archive="$test_tmp/$malicious_case.tar.gz"
  malicious_sha="$(sha256_file "$malicious_archive")"
  run_rejected "$malicious_case archive was accepted" \
    "$test_tmp/$malicious_case.log" \
    bash "$INSTALLER" --selftest-validate-archive \
    "$malicious_archive" "$malicious_sha" 1048576 1048576 8
done

run_rejected "compressed archive byte cap was not enforced" \
  "$test_tmp/archive-byte-limit.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$valid_archive" "$valid_archive_sha" 64 1048576 4
run_rejected "expanded archive byte cap was not enforced" \
  "$test_tmp/expanded-byte-limit.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$valid_archive" "$valid_archive_sha" 1048576 1024 4
run_rejected "archive member-count cap was not enforced" \
  "$test_tmp/member-count-limit.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$valid_archive" "$valid_archive_sha" 1048576 1048576 3

archive_copy="$test_tmp/archive-copy.tar.gz"
archive_hardlink="$test_tmp/archive-copy-hardlink.tar.gz"
cp "$valid_archive" "$archive_copy"
ln "$archive_copy" "$archive_hardlink"
run_rejected "hard-linked archive source was accepted" \
  "$test_tmp/archive-hardlink.log" \
  bash "$INSTALLER" --selftest-validate-archive \
  "$archive_copy" "$(sha256_file "$archive_copy")" 1048576 1048576 4

nlink_source="$test_tmp/nlink-source"
nlink_peer="$test_tmp/nlink-peer"
printf '%s\n' 'nlink fixture' > "$nlink_source"
ln "$nlink_source" "$nlink_peer"
run_rejected "regular-file nlink>1 was accepted" \
  "$test_tmp/nlink.log" \
  bash "$INSTALLER" --selftest-regular-nlink "$nlink_source"

case "$platform" in
  Linux*)
    [ -x /usr/bin/python3 ] ||
      fail "Linux security gate requires fixed /usr/bin/python3"
    [ -x /usr/bin/setsid ] ||
      fail "Linux security gate requires /usr/bin/setsid for SIGKILL fixtures"

    publisher_fixture_uid="$(id -u)"
    foreign_fixture_uid=$((publisher_fixture_uid + 1))
    run_success "explicit publisher UID boundary rejected the effective publisher" \
      "$test_tmp/publisher-uid-valid.log" \
      env MIR2_RELEASE_PUBLISHER_UID="$publisher_fixture_uid" \
      bash "$PACKAGER" --selftest-package-publisher-identity
    run_rejected "missing explicit publisher UID boundary was accepted" \
      "$test_tmp/publisher-uid-missing.log" \
      env -u MIR2_RELEASE_PUBLISHER_UID \
      bash "$PACKAGER" --selftest-package-publisher-identity
    run_rejected "mismatched explicit publisher UID boundary was accepted" \
      "$test_tmp/publisher-uid-mismatch.log" \
      env MIR2_RELEASE_PUBLISHER_UID="$foreign_fixture_uid" \
      bash "$PACKAGER" --selftest-package-publisher-identity
    [ "$(bash "$PACKAGER" --selftest-package-reserved-prefix-owner \
      "$foreign_fixture_uid" "$publisher_fixture_uid")" = "ignore" ] ||
      fail "foreign-UID /tmp package-stage prefix entry is not ignored"
    [ "$(bash "$PACKAGER" --selftest-package-reserved-prefix-owner \
      "$publisher_fixture_uid" "$publisher_fixture_uid")" = "owned" ] ||
      fail "publisher-owned /tmp stage prefix entry escapes strict validation"

    atomic_env_root="$test_tmp/atomic-env-root"
    mkdir -p "$atomic_env_root"
    chmod 0700 "$atomic_env_root"
    run_success "production atomic env first publication failed" \
      "$test_tmp/atomic-env-first.log" \
      bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_env_root" env "$ENV_TEMPLATE" - none
    atomic_env="$atomic_env_root/gateway.env"
    [ -f "$atomic_env" ] && [ ! -L "$atomic_env" ] ||
      fail "production atomic env transaction did not publish a regular file"
    [ "$(stat -c '%a' -- "$atomic_env")" = "600" ] ||
      fail "production atomic env transaction did not enforce mode 0600"
    [ "$(stat -c '%u:%g' -- "$atomic_env")" = "$(id -u):$(id -g)" ] ||
      fail "production atomic env transaction did not enforce fixture ownership"
    read_value "$atomic_env" MIR2_SAVE_RECOVERY_MAC_KEY
    atomic_recovery_key="$parsed_value"
    [[ "$atomic_recovery_key" =~ ^[0-9a-f]{64}$ ]] ||
      fail "production atomic env transaction did not generate a recovery key"
    assert_log_omits "$test_tmp/atomic-env-first.log" "$atomic_recovery_key"
    cp "$atomic_env" "$test_tmp/atomic-env.snapshot"
    atomic_env_inode="$(stat -c '%d:%i' -- "$atomic_env")"
    run_success "production atomic env repeat preflight failed" \
      "$test_tmp/atomic-env-repeat.log" \
      bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_env_root" env "$ENV_TEMPLATE" - none
    cmp -s "$atomic_env" "$test_tmp/atomic-env.snapshot" ||
      fail "production atomic env repeat rotated or rewrote the env"
    [ "$(stat -c '%d:%i' -- "$atomic_env")" = "$atomic_env_inode" ] ||
      fail "production atomic env repeat replaced the existing inode"
    assert_log_omits "$test_tmp/atomic-env-repeat.log" "$atomic_recovery_key"

    atomic_truncated_root="$test_tmp/atomic-truncated-root"
    mkdir -p "$atomic_truncated_root"
    chmod 0700 "$atomic_truncated_root"
    printf '%s\n' '[Unit]' > "$atomic_truncated_root/mir2-gateway.service"
    chmod 0644 "$atomic_truncated_root/mir2-gateway.service"
    run_rejected "production atomic file path accepted a truncated existing unit" \
      "$test_tmp/atomic-truncated.log" \
      bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_truncated_root" service - - none
    grep -Fxq -- '[Unit]' "$atomic_truncated_root/mir2-gateway.service" ||
      fail "truncated existing unit was overwritten"

    atomic_race_root="$test_tmp/atomic-race-root"
    atomic_race_hook="$test_tmp/atomic-race-hook"
    mkdir -p "$atomic_race_root" "$atomic_race_hook"
    chmod 0700 "$atomic_race_root" "$atomic_race_hook"
    bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_race_root" service - "$atomic_race_hook" payload \
      > "$test_tmp/atomic-race.log" 2>&1 &
    atomic_race_pid=$!
    wait_for_file "$atomic_race_hook/opened" \
      "production atomic no-replace race hook was not reached"
    cp "$normalized_service" "$atomic_race_root/mir2-gateway.service"
    chmod 0644 "$atomic_race_root/mir2-gateway.service"
    : > "$atomic_race_hook/continue"
    atomic_race_status=0
    wait "$atomic_race_pid" || atomic_race_status=$?
    [ "$atomic_race_status" -eq 0 ] ||
      fail "production atomic no-replace rejected an identical race winner"
    cmp -s "$atomic_race_root/mir2-gateway.service" "$normalized_service" ||
      fail "production atomic no-replace changed the race winner"
    if compgen -G "$atomic_race_root/.mir2-gateway.service.incoming.*" >/dev/null; then
      fail "production atomic no-replace left transaction residue"
    fi

    atomic_bad_race_root="$test_tmp/atomic-bad-race-root"
    atomic_bad_race_hook="$test_tmp/atomic-bad-race-hook"
    mkdir -p "$atomic_bad_race_root" "$atomic_bad_race_hook"
    chmod 0700 "$atomic_bad_race_root" "$atomic_bad_race_hook"
    bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_bad_race_root" service - "$atomic_bad_race_hook" payload \
      > "$test_tmp/atomic-bad-race.log" 2>&1 &
    atomic_bad_race_pid=$!
    wait_for_file "$atomic_bad_race_hook/opened" \
      "production atomic mismatched race hook was not reached"
    printf '%s\n' 'attacker-truncated-unit' \
      > "$atomic_bad_race_root/mir2-gateway.service"
    chmod 0644 "$atomic_bad_race_root/mir2-gateway.service"
    : > "$atomic_bad_race_hook/continue"
    atomic_bad_race_status=0
    wait "$atomic_bad_race_pid" || atomic_bad_race_status=$?
    [ "$atomic_bad_race_status" -ne 0 ] ||
      fail "production atomic no-replace accepted a mismatched race winner"
    grep -Fxq -- 'attacker-truncated-unit' \
      "$atomic_bad_race_root/mir2-gateway.service" ||
      fail "production atomic no-replace overwrote a mismatched race winner"
    if compgen -G "$atomic_bad_race_root/.mir2-gateway.service.incoming.*" >/dev/null; then
      fail "mismatched atomic race left transaction residue"
    fi

    atomic_parent_root="$test_tmp/atomic-parent-root"
    atomic_parent_held="$test_tmp/atomic-parent-held"
    atomic_parent_attacker="$test_tmp/atomic-parent-attacker"
    atomic_parent_hook="$test_tmp/atomic-parent-hook"
    mkdir -p "$atomic_parent_root" "$atomic_parent_attacker" "$atomic_parent_hook"
    chmod 0700 "$atomic_parent_root" "$atomic_parent_attacker" "$atomic_parent_hook"
    bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_parent_root" service - "$atomic_parent_hook" payload \
      > "$test_tmp/atomic-parent-swap.log" 2>&1 &
    atomic_parent_pid=$!
    wait_for_file "$atomic_parent_hook/opened" \
      "production atomic parent-swap hook was not reached"
    mv "$atomic_parent_root" "$atomic_parent_held"
    ln -s "$atomic_parent_attacker" "$atomic_parent_root"
    : > "$atomic_parent_hook/continue"
    atomic_parent_status=0
    wait "$atomic_parent_pid" || atomic_parent_status=$?
    rm -f "$atomic_parent_root"
    mv "$atomic_parent_held" "$atomic_parent_root"
    [ "$atomic_parent_status" -ne 0 ] ||
      fail "production atomic transaction accepted a parent symlink swap"
    if find "$atomic_parent_attacker" -mindepth 1 -maxdepth 1 -print -quit |
      grep -q .; then
      fail "production atomic parent swap wrote into the attacker directory"
    fi

    atomic_kill_root="$test_tmp/atomic-kill-root"
    atomic_kill_hook="$test_tmp/atomic-kill-hook"
    mkdir -p "$atomic_kill_root" "$atomic_kill_hook"
    chmod 0700 "$atomic_kill_root" "$atomic_kill_hook"
    /usr/bin/setsid bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_kill_root" env "$ENV_TEMPLATE" "$atomic_kill_hook" payload \
      > "$test_tmp/atomic-kill.log" 2>&1 &
    atomic_kill_pid=$!
    wait_for_file "$atomic_kill_hook/opened" \
      "production atomic SIGKILL hook was not reached"
    kill -KILL -- "-$atomic_kill_pid"
    wait "$atomic_kill_pid" 2>/dev/null || true
    mapfile -t atomic_kill_residues < <(
      find "$atomic_kill_root" -mindepth 1 -maxdepth 1 -type d \
        -name '.gateway.env.incoming.*' -print
    )
    [ "${#atomic_kill_residues[@]}" -eq 1 ] ||
      fail "production atomic SIGKILL fixture left an invalid residue count"
    [ -f "${atomic_kill_residues[0]}/.mir2-root-file-transaction.json" ] &&
      [ ! -L "${atomic_kill_residues[0]}/.mir2-root-file-transaction.json" ] &&
      [ -f "${atomic_kill_residues[0]}/payload" ] &&
      [ ! -L "${atomic_kill_residues[0]}/payload" ] ||
      fail "production atomic SIGKILL residue lacks marker/payload"
    run_success "production atomic next-start stale sweep/retry failed" \
      "$test_tmp/atomic-kill-retry.log" \
      bash "$INSTALLER" --selftest-atomic-root-file \
      "$atomic_kill_root" env "$ENV_TEMPLATE" - none
    [ -f "$atomic_kill_root/gateway.env" ] ||
      fail "production atomic SIGKILL retry did not publish gateway.env"
    if compgen -G "$atomic_kill_root/.gateway.env.incoming.*" >/dev/null; then
      fail "production atomic SIGKILL retry left transaction residue"
    fi

    package_stage_output="$(
      bash "$PACKAGER" --selftest-package-create-stage
    )" || fail "private package stage creation failed"
    mapfile -t package_stage_values <<< "$package_stage_output"
    [ "${#package_stage_values[@]}" -eq 2 ] ||
      fail "package stage selftest returned an invalid contract"
    package_test_stage="${package_stage_values[0]}"
    package_test_stage_name="${package_stage_values[1]}"
    printf '%s\n' 'package-original-gateway-bytes' \
      > "$package_test_stage/mir2-gateway"
    printf '%s\n' 'package-original-zone-bytes' \
      > "$package_test_stage/zone_host"
    printf '%s\n' \
      'Recovery key and sidecars are never packaged; back up and restore both together.' \
      > "$package_test_stage/README.txt"
    chmod 0755 "$package_test_stage/mir2-gateway" \
      "$package_test_stage/zone_host"
    chmod 0644 "$package_test_stage/README.txt"
    package_gateway_sha="$(sha256_file "$package_test_stage/mir2-gateway")"
    package_zone_sha="$(sha256_file "$package_test_stage/zone_host")"
    package_gateway_size="$(stat -c '%s' -- "$package_test_stage/mir2-gateway")"
    package_zone_size="$(stat -c '%s' -- "$package_test_stage/zone_host")"
    /usr/bin/python3 -I - \
      "$package_test_stage/RELEASE.json" \
      "$package_gateway_sha" "$package_zone_sha" \
      "$package_gateway_size" "$package_zone_size" <<'PY'
import json
import os
import sys

path, gateway_sha, zone_sha, gateway_size, zone_size = sys.argv[1:]
manifest = {
    "name": "mir2-gateway",
    "tag": "package-fixture",
    "target": "linux-x64",
    "binarySizeBytes": int(gateway_size),
    "binarySha256": gateway_sha,
    "zoneHostBinarySizeBytes": int(zone_size),
    "zoneHostBinarySha256": zone_sha,
    "installation": {
        "requiresRootOwnedPinManifest": True,
        "checksumSidecarIsAuthority": False,
        "rootPinRehashFromArchiveFdRequired": True,
        "publisherUidTrustBoundaryRequired": True,
        "archiveContainsInstaller": False,
        "archiveContainsSystemdUnit": False,
        "archiveContainsEnvironmentTemplate": False,
        "kernelDownloadFileSizeLimitRequired": True,
        "redirectsAllowed": False,
        "atomicNoReplacePublicationRequired": True,
        "strictServiceIdentityRequired": True,
        "localNssIdentityConsistencyRequired": True,
        "filesOnlyNssIdentityRequired": True,
        "activationCredentialPreflightRequired": True,
        "sameFdArchiveDigestRequired": True,
        "privatePublisherTransactionsRequired": True,
        "crashConsistentArchiveSidecarRequired": True,
        "boundedResidueSweepRequired": True,
        "atomicConfigPublicationRequired": True,
    },
}
file_fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
try:
    data = (json.dumps(manifest, sort_keys=True) + "\n").encode("ascii")
    os.write(file_fd, data)
    os.fsync(file_fd)
finally:
    os.close(file_fd)
PY
    chmod 0644 "$package_test_stage/RELEASE.json"

    writable_output="$test_tmp/package-writable-output"
    mkdir -p "$writable_output"
    chmod 0777 "$writable_output"
    run_rejected "group/other-writable package OUT_DIR was trusted" \
      "$test_tmp/package-writable-output.log" \
      bash "$PACKAGER" --selftest-package-publish \
      "$package_test_stage" "$writable_output" \
      'mir2-gateway-linux-x64-writable.tar.gz' - none

    package_output="$test_tmp/package-output"
    mkdir -p "$package_output"
    chmod 0700 "$package_output"
    package_source_hook="$test_tmp/package-source-hook"
    mkdir -p "$package_source_hook"
    chmod 0700 "$package_source_hook"
    bash "$PACKAGER" --selftest-package-publish \
      "$package_test_stage" "$package_output" \
      'mir2-gateway-linux-x64-package-fixture.tar.gz' \
      "$package_source_hook" sources \
      > "$test_tmp/package-source-swap.log" 2>&1 &
    package_source_pid=$!
    wait_for_file "$package_source_hook/opened" \
      "package source-swap hook did not open held FDs"
    mv "$package_test_stage/mir2-gateway" \
      "$test_tmp/package-original-gateway"
    printf '%s\n' 'attacker replacement package bytes' \
      > "$package_test_stage/mir2-gateway"
    chmod 0755 "$package_test_stage/mir2-gateway"
    : > "$package_source_hook/continue"
    package_source_status=0
    wait "$package_source_pid" || package_source_status=$?
    rm -f "$package_test_stage/mir2-gateway"
    mv "$test_tmp/package-original-gateway" \
      "$package_test_stage/mir2-gateway"
    [ "$package_source_status" -eq 0 ] ||
      fail "same-FD package source-swap defense failed"
    packaged_archive="$package_output/mir2-gateway-linux-x64-package-fixture.tar.gz"
    packaged_sidecar="$packaged_archive.sha256"
    [ -f "$packaged_archive" ] && [ -f "$packaged_sidecar" ] ||
      fail "package archive/sidecar pair was not committed"
    read -r packaged_sidecar_sha packaged_sidecar_name < "$packaged_sidecar"
    [ "$packaged_sidecar_sha" = "$(sha256_file "$packaged_archive")" ] &&
      [ "$packaged_sidecar_name" = \
        'mir2-gateway-linux-x64-package-fixture.tar.gz' ] ||
      fail "package sidecar does not match the committed archive"
    run_success "same-FD packaged archive failed installer validation" \
      "$test_tmp/package-archive-validation.log" \
      bash "$INSTALLER" --selftest-validate-archive \
      "$packaged_archive" "$packaged_sidecar_sha" \
      536870912 536870912 4

    package_swap_output="$test_tmp/package-swap-output"
    package_swap_held="$test_tmp/package-swap-held"
    package_swap_attacker="$test_tmp/package-swap-attacker"
    package_swap_hook="$test_tmp/package-swap-hook"
    mkdir -p "$package_swap_output" "$package_swap_attacker" \
      "$package_swap_hook"
    chmod 0700 "$package_swap_output" "$package_swap_attacker" \
      "$package_swap_hook"
    bash "$PACKAGER" --selftest-package-publish \
      "$package_test_stage" "$package_swap_output" \
      'mir2-gateway-linux-x64-output-swap.tar.gz' \
      "$package_swap_hook" sources \
      > "$test_tmp/package-output-swap.log" 2>&1 &
    package_swap_pid=$!
    wait_for_file "$package_swap_hook/opened" \
      "package output-swap hook was not reached"
    mv "$package_swap_output" "$package_swap_held"
    ln -s "$package_swap_attacker" "$package_swap_output"
    : > "$package_swap_hook/continue"
    package_swap_status=0
    wait "$package_swap_pid" || package_swap_status=$?
    rm -f "$package_swap_output"
    mv "$package_swap_held" "$package_swap_output"
    [ "$package_swap_status" -ne 0 ] ||
      fail "concurrent package OUT_DIR replacement was accepted"
    if find "$package_swap_attacker" -mindepth 1 -maxdepth 1 -print -quit |
      grep -q .; then
      fail "package OUT_DIR swap wrote into the attacker directory"
    fi

    package_crash_output="$test_tmp/package-crash-output"
    package_crash_hook="$test_tmp/package-crash-hook"
    mkdir -p "$package_crash_output" "$package_crash_hook"
    chmod 0700 "$package_crash_output" "$package_crash_hook"
    /usr/bin/setsid bash "$PACKAGER" --selftest-package-publish \
      "$package_test_stage" "$package_crash_output" \
      'mir2-gateway-linux-x64-crash.tar.gz' \
      "$package_crash_hook" sidecar \
      > "$test_tmp/package-crash.log" 2>&1 &
    package_crash_pid=$!
    wait_for_file "$package_crash_hook/opened" \
      "package sidecar crash hook was not reached"
    [ -f "$package_crash_output/mir2-gateway-linux-x64-crash.tar.gz.sha256" ] ||
      fail "sidecar crash fixture did not reach tentative publication"
    kill -KILL -- "-$package_crash_pid"
    wait "$package_crash_pid" 2>/dev/null || true
    if ! compgen -G "$package_crash_output/package-incoming.*" >/dev/null; then
      fail "SIGKILL package fixture did not leave a recoverable transaction"
    fi
    run_success "next package run did not sweep SIGKILL residue" \
      "$test_tmp/package-crash-retry.log" \
      bash "$PACKAGER" --selftest-package-publish \
      "$package_test_stage" "$package_crash_output" \
      'mir2-gateway-linux-x64-crash.tar.gz' - none
    [ -f "$package_crash_output/mir2-gateway-linux-x64-crash.tar.gz" ] &&
      [ -f "$package_crash_output/mir2-gateway-linux-x64-crash.tar.gz.sha256" ] ||
      fail "retry did not commit an archive/sidecar pair"
    if compgen -G "$package_crash_output/package-incoming.*" >/dev/null; then
      fail "retry left package transaction residue"
    fi

    bash "$PACKAGER" --selftest-package-cleanup-stage \
      "$package_test_stage_name"
    package_test_stage_name=""

    stream_downloader="$test_tmp/fake-curl-stream"
    cat > "$stream_downloader" <<'SH'
#!/bin/sh
set -eu
count=0
while [ "$count" -lt 64 ]; do
  printf '%s' '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
  count=$((count + 1))
done
SH
    chmod 0700 "$stream_downloader"
    no_length_dir="$test_tmp/download-no-content-length"
    mkdir -p "$no_length_dir"
    chmod 0700 "$no_length_dir"
    run_success "headerless/no-Content-Length production downloader path failed" \
      "$test_tmp/download-no-content-length.log" \
      bash "$INSTALLER" --selftest-download \
      "$stream_downloader" "$no_length_dir" \
      'https://releases.example.invalid/gateway.tar.gz' 8192
    [ -f "$no_length_dir/release.tar.gz" ] ||
      fail "bounded downloader did not retain its successful body"
    no_length_size="$(stat -c '%s' -- "$no_length_dir/release.tar.gz")"
    [ "$no_length_size" -gt 0 ] && [ "$no_length_size" -le 8192 ] ||
      fail "headerless download escaped its kernel/file identity bound"

    chunked_overrun_downloader="$test_tmp/fake-curl-chunked-overrun"
    cat > "$chunked_overrun_downloader" <<'SH'
#!/bin/sh
set -eu
count=0
while [ "$count" -lt 4096 ]; do
  printf '%s' 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210'
  count=$((count + 1))
done
SH
    chmod 0700 "$chunked_overrun_downloader"
    chunked_overrun_dir="$test_tmp/download-chunked-overrun"
    mkdir -p "$chunked_overrun_dir"
    chmod 0700 "$chunked_overrun_dir"
    run_rejected "chunked/unknown-length body escaped RLIMIT_FSIZE" \
      "$test_tmp/download-chunked-overrun.log" \
      bash "$INSTALLER" --selftest-download \
      "$chunked_overrun_downloader" "$chunked_overrun_dir" \
      'https://releases.example.invalid/gateway.tar.gz' 4096
    [ ! -e "$chunked_overrun_dir/release.tar.gz" ] ||
      fail "over-limit downloader left a partial or oversized archive"

    for residue_kind in download incoming; do
      residue_term_root="$test_tmp/residue-$residue_kind-term"
      residue_term_hook="$test_tmp/residue-$residue_kind-term-hook"
      mkdir -p "$residue_term_root" "$residue_term_hook"
      chmod 0700 "$residue_term_root" "$residue_term_hook"
      /usr/bin/setsid bash "$INSTALLER" --selftest-residue-hold \
        "$residue_term_root" "$residue_kind" "$residue_term_hook" \
        > "$test_tmp/residue-$residue_kind-term.log" 2>&1 &
      residue_term_pid=$!
      wait_for_file "$residue_term_hook/ready" \
        "$residue_kind TERM residue fixture was not ready"
      kill -TERM -- "-$residue_term_pid"
      wait "$residue_term_pid" 2>/dev/null || true
      if find "$residue_term_root" -mindepth 1 -maxdepth 1 -print -quit |
        grep -q .; then
        fail "$residue_kind TERM did not clean its private transaction"
      fi

      residue_kill_root="$test_tmp/residue-$residue_kind-kill"
      residue_kill_hook="$test_tmp/residue-$residue_kind-kill-hook"
      mkdir -p "$residue_kill_root" "$residue_kill_hook"
      chmod 0700 "$residue_kill_root" "$residue_kill_hook"
      /usr/bin/setsid bash "$INSTALLER" --selftest-residue-hold \
        "$residue_kill_root" "$residue_kind" "$residue_kill_hook" \
        > "$test_tmp/residue-$residue_kind-kill.log" 2>&1 &
      residue_kill_pid=$!
      wait_for_file "$residue_kill_hook/ready" \
        "$residue_kind SIGKILL residue fixture was not ready"
      kill -KILL -- "-$residue_kill_pid"
      wait "$residue_kill_pid" 2>/dev/null || true
      if ! find "$residue_kill_root" -mindepth 1 -maxdepth 1 -print -quit |
        grep -q .; then
        fail "$residue_kind SIGKILL fixture left no sweepable residue"
      fi
      run_success "$residue_kind next-start stale sweep failed" \
        "$test_tmp/residue-$residue_kind-sweep.log" \
        bash "$INSTALLER" --selftest-residue-sweep \
        "$residue_kill_root" "$residue_kind"
      if find "$residue_kill_root" -mindepth 1 -maxdepth 1 -print -quit |
        grep -q .; then
        fail "$residue_kind stale sweep left verified dead-owner residue"
      fi
    done

    publication_root="$test_tmp/release-publication"
    mkdir -p "$publication_root"
    chmod 0700 "$publication_root"
    run_rejected "injected partial release unexpectedly published" \
      "$test_tmp/release-abort.log" \
      bash "$INSTALLER" --selftest-install-layout \
      "$valid_archive" "$valid_archive_sha" fixture-release \
      "$publication_root" 1048576 1048576 4 abort
    if find "$publication_root" -mindepth 1 -maxdepth 1 -print -quit |
      grep -q .; then
      fail "failed unpublished release was not safely cleaned"
    fi
    run_success "retry after unpublished release cleanup failed" \
      "$test_tmp/release-publish.log" \
      bash "$INSTALLER" --selftest-install-layout \
      "$valid_archive" "$valid_archive_sha" fixture-release \
      "$publication_root" 1048576 1048576 4 publish
    published_root="$publication_root/fixture-release"
    [ -d "$published_root" ] && [ ! -L "$published_root" ] ||
      fail "completed release was not atomically published"
    [ "$(stat -c '%a' -- "$published_root")" = "755" ] ||
      fail "published release directory mode is not 0755"
    for published_name in mir2-gateway zone_host RELEASE.json README.txt; do
      [ -f "$published_root/$published_name" ] &&
        [ ! -L "$published_root/$published_name" ] ||
        fail "published release whitelist is incomplete"
    done
    run_success "idempotent no-replace publication rejected identical release" \
      "$test_tmp/release-idempotent.log" \
      bash "$INSTALLER" --selftest-install-layout \
      "$valid_archive" "$valid_archive_sha" fixture-release \
      "$publication_root" 1048576 1048576 4 publish
    if compgen -G "$publication_root/incoming.*" >/dev/null; then
      fail "idempotent publication left an unpublished directory"
    fi
    printf '%s\n' 'tampered published bytes' > "$published_root/README.txt"
    run_rejected "no-replace publication accepted a mismatched existing release" \
      "$test_tmp/release-existing-mismatch.log" \
      bash "$INSTALLER" --selftest-install-layout \
      "$valid_archive" "$valid_archive_sha" fixture-release \
      "$publication_root" 1048576 1048576 4 publish
    grep -Fxq -- 'tampered published bytes' "$published_root/README.txt" ||
      fail "no-replace publication overwrote an existing release"
    if compgen -G "$publication_root/incoming.*" >/dev/null; then
      fail "mismatched no-replace publication left an unpublished directory"
    fi

    production_swap_root="$test_tmp/production-source-swap"
    production_swap_releases="$test_tmp/production-source-swap-releases"
    mkdir -p "$production_swap_root" "$production_swap_releases"
    chmod 0700 "$production_swap_root" "$production_swap_releases"
    cp "$valid_archive" "$production_swap_root/source.tar.gz"
    cp "$helper_archive" "$production_swap_root/replacement.tar.gz"
    run_success "production archive-FD source swap defense failed" \
      "$test_tmp/production-source-swap.log" \
      bash "$INSTALLER" --selftest-install-source-swap \
      "$production_swap_root/source.tar.gz" \
      "$production_swap_root/replacement.tar.gz" \
      "$valid_archive_sha" fixture-release \
      "$production_swap_releases" 1048576 1048576 4
    [ "$(sha256_file "$production_swap_root/source.tar.gz")" = "$helper_sha" ] ||
      fail "production source-swap fixture did not replace the pathname"
    cmp -s "$production_swap_releases/fixture-release/README.txt" \
      "$fixture_stage/README.txt" ||
      fail "production install path reopened the replaced archive pathname"

    source_root="$test_tmp/same-fd"
    mkdir -p "$source_root"
    printf '%s\n' 'authenticated source bytes' > "$source_root/source"
    printf '%s\n' 'attacker replacement bytes' > "$source_root/replacement"
    source_sha="$(sha256_file "$source_root/source")"
    cp "$source_root/source" "$source_root/expected"
    run_success "same-FD source-swap defense failed" \
      "$test_tmp/same-fd.log" \
      bash "$INSTALLER" --selftest-same-fd-swap \
      "$source_root" source replacement installed "$source_sha"
    cmp -s "$source_root/installed" "$source_root/expected" ||
      fail "same-FD copy reopened the attacker-replaced source path"
    grep -Fq 'attacker replacement bytes' "$source_root/source" ||
      fail "source-swap fixture did not replace the source pathname"

    recovery_data="$test_tmp/recovery-data"
    mkdir -p "$recovery_data"
    chmod 0700 "$recovery_data"
    run_success "Linux recovery dirfd creation failed" \
      "$test_tmp/recovery-dir-1.log" \
      bash "$INSTALLER" --selftest-recovery-dir "$recovery_data"
    [ "$(stat -c '%a' -- "$recovery_data/save-recovery")" = "711" ] ||
      fail "recovery namespace mode is not 0711"
    [ "$(stat -c '%a' -- "$recovery_data/save-recovery/v1")" = "711" ] ||
      fail "recovery version mode is not 0711"
    [ "$(stat -c '%a' -- "$recovery_data/save-recovery/v1/gateway")" = "700" ] ||
      fail "recovery leaf mode is not 0700"
    expected_owner="$(id -u):$(id -g)"
    [ "$(stat -c '%u:%g' -- "$recovery_data/save-recovery/v1/gateway")" = \
      "$expected_owner" ] ||
      fail "recovery leaf owner differs from the service fixture"
    chmod 0755 "$recovery_data/save-recovery/v1/gateway"
    run_success "repeat recovery dirfd preflight failed" \
      "$test_tmp/recovery-dir-2.log" \
      bash "$INSTALLER" --selftest-recovery-dir "$recovery_data"
    [ "$(stat -c '%a' -- "$recovery_data/save-recovery/v1/gateway")" = "700" ] ||
      fail "repeat recovery preflight did not restore mode 0700"

    symlink_data="$test_tmp/recovery-symlink-data"
    mkdir -p "$symlink_data/save-recovery" "$symlink_data/real-v1"
    chmod 0711 "$symlink_data/save-recovery" "$symlink_data/real-v1"
    ln -s "$symlink_data/real-v1" "$symlink_data/save-recovery/v1"
    run_rejected "intermediate recovery symlink was followed" \
      "$test_tmp/recovery-symlink.log" \
      bash "$INSTALLER" --selftest-recovery-dir "$symlink_data"
    [ ! -e "$symlink_data/real-v1/gateway" ] ||
      fail "symlink rejection mutated the link target"

    printf '%s\n' \
      'PASS: trust-root, files-only NSS identity/activation/env, URL/curl, archive/resource, stable-key, Linux RLIMIT/private-publisher/no-replace/production-atomic-config/dirfd/source+OUT_DIR-swap/TERM+SIGKILL-sweep/nlink contracts'
    ;;
  *)
    printf '%s\n' \
      "NOT RUN: Linux security gate (platform=$platform)"
    printf '%s\n' \
      "LOCAL CHECKS PASS: non-Linux trust-root, files-only NSS/activation/env fixtures, URL/curl, archive/resource, stable-key/nlink and packager-platform-rejection contracts"
    ;;
esac
