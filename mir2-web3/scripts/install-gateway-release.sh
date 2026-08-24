#!/bin/bash -p
set -euo pipefail

trusted_installer_path=/usr/local/libexec/mir2/install-gateway-release.sh
trusted_pin_path=/etc/mir2/gateway-release.pin
trusted_env_template_path=/usr/local/share/mir2/gateway/mir2-gateway.env.example
python3_path=/usr/bin/python3
archive_max_bytes=536870912
expanded_max_bytes=536870912
archive_max_members=4

usage() {
  cat <<'TXT'
Usage (production, from a sanitized root execution context):
  /usr/local/libexec/mir2/install-gateway-release.sh [--activate]

Mandatory out-of-band bootstrap trust bundle:
  /usr/local/libexec/mir2/install-gateway-release.sh
    root:root, mode 0755, regular non-symlink, nlink=1
  /usr/local/share/mir2/gateway/mir2-gateway.env.example
    root:root, mode 0644, regular non-symlink, nlink=1
  /etc/mir2/gateway-release.pin
    root:root, mode 0600, regular non-symlink, nlink=1

The pin file must contain exactly:
  MIR2_GATEWAY_RELEASE_URL=https://...
  MIR2_GATEWAY_RELEASE_SHA256=<64-hex archive digest>
  MIR2_GATEWAY_RELEASE_TAG=<safe single component>

URL, digest, and tag environment variables are rejected. The pin file is the
independent authenticity root and must be provisioned through trusted
configuration management, separately from the release archive and release URL.
That channel must derive the digest independently from its authenticated
artifact registry; the publisher .sha256 sidecar is never pin authority.
The archive never supplies executable installer/helper code, a systemd unit,
or the environment template.

--activate additionally applies a weak-pattern heuristic to the Postgres,
Redis, and optional operator credentials without printing their values. This
heuristic cannot determine whether a credential is public or private: provision
all credentials from a trusted secret manager or a CSPRNG, keep them
independent, and treat the heuristic as a fail-closed deployment hygiene gate.
TXT
}

installer_error() {
  printf '%s\n' "gateway installer: $1" >&2
  return 1
}

reject_caller_release_authority() {
  local name
  for name in \
    MIR2_GATEWAY_RELEASE_URL \
    MIR2_GATEWAY_RELEASE_SHA256 \
    MIR2_GATEWAY_RELEASE_SHA256_URL \
    MIR2_GATEWAY_RELEASE_TAG \
    MIR2_GATEWAY_INSTALL_ROOT \
    MIR2_GATEWAY_DATA_DIR \
    MIR2_GATEWAY_LOG_DIR \
    MIR2_GATEWAY_ENV_PATH \
    MIR2_GATEWAY_SERVICE_PATH \
    MIR2_GATEWAY_USER; do
    if [[ -v "$name" ]]; then
      installer_error "$name is caller-controlled and is not accepted"
      return 1
    fi
  done
}

find_selftest_python() {
  local candidate
  local candidate_path
  for candidate in python3 python; do
    if command -v "$candidate" >/dev/null 2>&1; then
      candidate_path="$(command -v "$candidate")"
      if "$candidate_path" -I -c \
        'import sys; raise SystemExit(0 if sys.version_info.major == 3 else 1)' \
        >/dev/null 2>&1; then
        selftest_python="$candidate_path"
        return 0
      fi
    fi
  done
  installer_error "a working Python 3 is required for installer selftests"
  return 1
}

run_python_engine() {
  local interpreter="$1"
  shift
  /usr/bin/env -i \
    PATH=/usr/sbin:/usr/bin:/sbin:/bin \
    LC_ALL=C \
    "$interpreter" -I - "$@" <<'PY'
import gzip
import hashlib
import hmac
import json
import os
import re
import secrets
import signal
import stat
import subprocess
import sys
import time
import urllib.parse

try:
    import ctypes
    import errno
    import resource
except ImportError:
    ctypes = None
    errno = None
    resource = None

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(newline="\n")

TRUSTED_INSTALLER = "/usr/local/libexec/mir2/install-gateway-release.sh"
TRUSTED_PIN = "/etc/mir2/gateway-release.pin"
TRUSTED_ENV_TEMPLATE = (
    "/usr/local/share/mir2/gateway/mir2-gateway.env.example"
)
INSTALL_ROOT = "/opt/mir2/gateway"
RELEASES_ROOT = "/opt/mir2/gateway/releases"
DATA_ROOT = "/var/lib/mir2"
SERVICE_PATH = "/etc/systemd/system/mir2-gateway.service"
RECOVERY_DIR = "/var/lib/mir2/save-recovery/v1/gateway"
RECOVERY_PLACEHOLDER = "replace-with-stable-independent-64-hex-secret"
CURL_PATH = "/usr/bin/curl"
VAR_TMP = "/var/tmp"
DOWNLOAD_PREFIX = "mir2-gateway-install."
DOWNLOAD_NAME = "release.tar.gz"
DOWNLOAD_MARKER = ".mir2-gateway-download.json"
INCOMING_PREFIX = "incoming."
INCOMING_MARKER = ".mir2-gateway-release-transaction.json"
INCOMING_PAYLOAD = "payload"
RESIDUE_VERSION = 1
MAX_RESIDUE_COUNT = 8
MAX_RESIDUAL_BYTES = 1_073_741_824
MAX_ACTIVE_RESIDUE_AGE = 3600
MAX_FUTURE_CLOCK_SKEW = 300
EXPECTED_MEMBERS = {
    "mir2-gateway": (0o755, 268_435_456),
    "zone_host": (0o755, 268_435_456),
    "RELEASE.json": (0o644, 65_536),
    "README.txt": (0o644, 65_536),
}
CAPTURE_MEMBERS = {"RELEASE.json", "README.txt"}
ALLOWED_TARGETS = {
    "linux-x64",
    "linux-arm64",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
}
REQUIRED_ENV_KEYS = {
    "MIR2_ENV",
    "MIR2_GATEWAY_WEB_ADDR",
    "MIR2_GATEWAY_TCP_ADDR",
    "MIR2_ACCOUNT_STORE_BACKEND",
    "MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES",
    "MIR2_ACCOUNT_STORE_DATABASE_URL",
    "MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE",
    "MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS",
    "MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS",
    "MIR2_ACCOUNT_STORE_PATH",
    "MIR2_SAVE_RECOVERY_DIR",
    "MIR2_SAVE_RECOVERY_MAC_KEY",
    "MIR2_GATEWAY_MAX_WS_CONNECTIONS",
    "MIR2_GATEWAY_MAX_ACTIVE_SESSIONS",
    "MIR2_GATEWAY_MAX_RECONNECT_LEASES",
    "MIR2_GATEWAY_RECONNECT_GRACE_SECONDS",
    "MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY",
    "MIR2_GATEWAY_RUNTIME_TICK_MS",
    "MIR2_GATEWAY_TOKIO_WORKER_THREADS",
    "MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT",
    "MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT",
    "MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT",
    "MIR2_GATEWAY_REDIS_CACHE_URL",
    "MIR2_GATEWAY_REQUIRE_REDIS_CACHE",
    "MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS",
    "MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS",
    "MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS",
    "MIR2_PASSKEY_AUTH_SECRET",
    "MIR2_IDENTITY_POLICY",
    "MIR2_IDENTITY_SESSION_SECRET",
    "MIR2_IDENTITY_RECOVERY_PEPPER",
    "MIR2_IDENTITY_SESSION_TTL_SECONDS",
    "MIR2_ALLOWED_WEB_ORIGINS",
    "MIR2_TRUST_CF_CONNECTING_IP",
}
OPTIONAL_ENV_KEYS = {
    "CRYSTAL_CLIENT_ROOT",
    "ADMIN_API_BASE_URL",
    "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN",
    "MIR2_GAMEPLAY_EVENT_REDPANDA_URL",
    "MIR2_GAMEPLAY_EVENT_TOPIC",
}
SECRET_ENV_KEYS = {
    "MIR2_SAVE_RECOVERY_MAC_KEY",
    "MIR2_PASSKEY_AUTH_SECRET",
    "MIR2_IDENTITY_SESSION_SECRET",
    "MIR2_IDENTITY_RECOVERY_PEPPER",
}
NUMERIC_ENV_KEYS = {
    "MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE",
    "MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS",
    "MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS",
    "MIR2_GATEWAY_MAX_WS_CONNECTIONS",
    "MIR2_GATEWAY_MAX_ACTIVE_SESSIONS",
    "MIR2_GATEWAY_MAX_RECONNECT_LEASES",
    "MIR2_GATEWAY_RECONNECT_GRACE_SECONDS",
    "MIR2_GATEWAY_RUNTIME_TICK_MS",
    "MIR2_GATEWAY_TOKIO_WORKER_THREADS",
    "MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT",
    "MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT",
    "MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT",
    "MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS",
    "MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS",
    "MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS",
    "MIR2_IDENTITY_SESSION_TTL_SECONDS",
}
ROOT_FILE_MARKER = ".mir2-root-file-transaction.json"
ROOT_FILE_PAYLOAD = "payload"
SERVICE_UNIT = b"""[Unit]
Description=Mir2 Gateway
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=mir2
Group=mir2
UMask=0077
EnvironmentFile=/etc/mir2/gateway.env
WorkingDirectory=/var/lib/mir2/gateway-data
ExecStart=/opt/mir2/gateway/current/mir2-gateway
Restart=always
RestartSec=3
LimitNOFILE=65535
LimitCORE=0
LimitNPROC=4096
TasksMax=4096
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=true
ProtectProc=invisible
ProcSubset=pid
ProtectClock=true
ProtectControlGroups=true
ProtectHostname=true
ProtectKernelLogs=true
ProtectKernelModules=true
ProtectKernelTunables=true
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
LockPersonality=true
MemoryDenyWriteExecute=true
RemoveIPC=true
KeyringMode=private
CapabilityBoundingSet=
AmbientCapabilities=
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallErrorNumber=EPERM
InaccessiblePaths=/etc/shadow /etc/gshadow
ReadWritePaths=/var/lib/mir2/gateway-data /var/lib/mir2/save-recovery/v1/gateway /var/log/mir2
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
""".replace(b"\r\n", b"\n")

O_NOFOLLOW = getattr(os, "O_NOFOLLOW", 0)
O_DIRECTORY = getattr(os, "O_DIRECTORY", 0)
O_CLOEXEC = getattr(os, "O_CLOEXEC", 0)
O_BINARY = getattr(os, "O_BINARY", 0)
DIR_FLAGS = os.O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC
FILE_READ_FLAGS = os.O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_BINARY
FILE_CREATE_FLAGS = (
    os.O_WRONLY | os.O_CREAT | os.O_EXCL | O_NOFOLLOW | O_CLOEXEC | O_BINARY
)
FILE_CREATE_RW_FLAGS = (
    os.O_RDWR | os.O_CREAT | os.O_EXCL | O_NOFOLLOW | O_CLOEXEC | O_BINARY
)


class SecurityError(Exception):
    pass


def fail(message):
    raise SecurityError(message)


def require_linux_dirfd():
    if not sys.platform.startswith("linux"):
        fail("production installation requires Linux")
    if not O_NOFOLLOW or not O_DIRECTORY:
        fail("Linux O_NOFOLLOW and O_DIRECTORY are required")
    for function_name in ("open", "mkdir", "stat", "replace", "fchmod", "fchown", "fsync"):
        if not hasattr(os, function_name):
            fail("required Linux dirfd operations are unavailable")
    if resource is None or ctypes is None or errno is None:
        fail("Linux resource limits and renameat2 support are required")


def arm_parent_death_signal():
    require_linux_dirfd()
    parent_before = os.getppid()
    libc = ctypes.CDLL(None, use_errno=True)
    prctl = getattr(libc, "prctl", None)
    if prctl is None:
        fail("Linux PR_SET_PDEATHSIG support is required")
    prctl.argtypes = [
        ctypes.c_int,
        ctypes.c_ulong,
        ctypes.c_ulong,
        ctypes.c_ulong,
        ctypes.c_ulong,
    ]
    prctl.restype = ctypes.c_int
    if prctl(1, signal.SIGTERM, 0, 0, 0) != 0:
        fail("could not arm the installer parent-death signal")
    if os.getppid() != parent_before:
        fail("installer parent changed while arming parent-death handling")


def install_interrupt_handlers():
    handled_signals = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)

    def interrupted(signum, frame):
        del signum, frame
        for signal_number in handled_signals:
            signal.signal(signal_number, signal.SIG_IGN)
        raise SecurityError("installation interrupted; private transaction cleaned")

    for signal_number in handled_signals:
        signal.signal(signal_number, interrupted)


def safe_component(value, label, maximum=127):
    pattern = r"[A-Za-z0-9][A-Za-z0-9._-]{0,%d}" % maximum
    if not re.fullmatch(pattern, value) or value in {".", ".."}:
        fail(f"{label} must be one safe non-dot component")
    if label == "release tag" and value.startswith(INCOMING_PREFIX):
        fail("release tag uses the reserved unpublished-release prefix")
    return value


def validate_sha256(value):
    if not re.fullmatch(r"[0-9a-fA-F]{64}", value):
        fail("release SHA-256 must be exactly 64 hexadecimal characters")
    return value.lower()


def validate_https_url(value):
    if not value or len(value.encode("utf-8")) > 2048:
        fail("release URL length is invalid")
    if value.startswith("-"):
        fail("release URL must not be option-like")
    if "\\" in value or any(ord(ch) < 0x21 or ord(ch) > 0x7E for ch in value):
        fail("release URL contains unsupported characters")
    if "?" in value or "#" in value:
        fail("release URL must not contain a query or fragment delimiter")
    try:
        parsed = urllib.parse.urlsplit(value)
        port = parsed.port
    except ValueError:
        fail("release URL is malformed")
    if parsed.scheme != "https":
        fail("release URL must use HTTPS")
    if not parsed.hostname or parsed.username is not None or parsed.password is not None:
        fail("release URL must have a host and no embedded credentials")
    if parsed.query or parsed.fragment:
        fail("release URL must not contain a query or fragment")
    if port is not None and not (1 <= port <= 65535):
        fail("release URL port is invalid")
    return value


def path_components(path):
    if (
        not path.startswith("/")
        or path == "/"
        or "//" in path
        or len(os.fsencode(path)) > 4096
    ):
        fail("trusted path must be a narrow absolute path")
    parts = path.split("/")[1:]
    if len(parts) > 32 or any(
        not part
        or len(os.fsencode(part)) > 127
        or part in {".", ".."}
        or not re.fullmatch(r"[A-Za-z0-9._-]+", part)
        for part in parts
    ):
        fail("trusted path contains an unsafe component")
    return parts


def require_root_directory_stat(file_stat, label):
    mode = stat.S_IMODE(file_stat.st_mode)
    if not stat.S_ISDIR(file_stat.st_mode):
        fail(f"{label} is not a directory")
    if file_stat.st_uid != 0 or file_stat.st_gid != 0 or mode & 0o022:
        fail(f"{label} must be root:root and non-writable by group/other")
    if mode & 0o005 != 0o005:
        fail(f"{label} must remain traversable")


def open_root_directory(path):
    parts = path_components(path)
    current_fd = os.open("/", DIR_FLAGS)
    try:
        require_root_directory_stat(os.fstat(current_fd), "/")
        for component in parts:
            try:
                next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
            except OSError:
                fail("trusted path contains a missing or symbolic-link component")
            os.close(current_fd)
            current_fd = next_fd
            require_root_directory_stat(os.fstat(current_fd), component)
        return current_fd
    except Exception:
        os.close(current_fd)
        raise


def open_trusted_regular(path, expected_mode, maximum_bytes):
    parts = path_components(path)
    parent_fd = open_root_directory("/" + "/".join(parts[:-1]))
    try:
        try:
            file_fd = os.open(parts[-1], FILE_READ_FLAGS, dir_fd=parent_fd)
        except OSError:
            fail("trusted file is missing, unreadable, or a symbolic link")
    finally:
        os.close(parent_fd)
    file_stat = os.fstat(file_fd)
    mode = stat.S_IMODE(file_stat.st_mode)
    if (
        not stat.S_ISREG(file_stat.st_mode)
        or file_stat.st_uid != 0
        or file_stat.st_gid != 0
        or file_stat.st_nlink != 1
        or mode != expected_mode
        or file_stat.st_size <= 0
        or file_stat.st_size > maximum_bytes
    ):
        os.close(file_fd)
        fail("trusted regular-file owner/mode/nlink/size contract failed")
    return file_fd


def read_all_fd(file_fd, maximum_bytes):
    os.lseek(file_fd, 0, os.SEEK_SET)
    output = bytearray()
    while True:
        request = min(1_048_576, maximum_bytes + 1 - len(output))
        chunk = os.read(file_fd, request)
        if not chunk:
            break
        output.extend(chunk)
        if len(output) > maximum_bytes:
            fail("trusted file exceeds its size limit")
    return bytes(output)


def parse_pin_bytes(pin_bytes):
    if b"\0" in pin_bytes:
        fail("pin manifest contains NUL")
    try:
        text = pin_bytes.decode("ascii")
    except UnicodeDecodeError:
        fail("pin manifest must be ASCII")
    expected = {
        "MIR2_GATEWAY_RELEASE_URL",
        "MIR2_GATEWAY_RELEASE_SHA256",
        "MIR2_GATEWAY_RELEASE_TAG",
    }
    values = {}
    for raw_line in text.splitlines():
        line = raw_line.rstrip("\r")
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            fail("pin manifest lines must use exact KEY=VALUE syntax")
        key, value = line.split("=", 1)
        if key not in expected or key in values or value != value.strip():
            fail("pin manifest contains an unexpected, duplicate, or padded key")
        values[key] = value
    if set(values) != expected:
        fail("pin manifest must contain each required key exactly once")
    return (
        validate_https_url(values["MIR2_GATEWAY_RELEASE_URL"]),
        validate_sha256(values["MIR2_GATEWAY_RELEASE_SHA256"]),
        safe_component(values["MIR2_GATEWAY_RELEASE_TAG"], "release tag"),
    )


def read_pin_test(path):
    try:
        file_fd = os.open(path, FILE_READ_FLAGS)
    except OSError:
        fail("test pin file is missing or a symbolic link")
    try:
        file_stat = os.fstat(file_fd)
        if not stat.S_ISREG(file_stat.st_mode) or file_stat.st_nlink != 1:
            fail("test pin must be an independent regular file")
        return parse_pin_bytes(read_all_fd(file_fd, 4096))
    finally:
        os.close(file_fd)


def read_production_pin(installer_path, pin_path, template_path):
    if (
        installer_path != TRUSTED_INSTALLER
        or pin_path != TRUSTED_PIN
        or template_path != TRUSTED_ENV_TEMPLATE
    ):
        fail("bootstrap trust paths are not the compiled fixed paths")
    installer_fd = open_trusted_regular(installer_path, 0o755, 2_097_152)
    os.close(installer_fd)
    template_fd = open_trusted_regular(template_path, 0o644, 1_048_576)
    os.close(template_fd)
    pin_fd = open_trusted_regular(pin_path, 0o600, 4096)
    try:
        return parse_pin_bytes(read_all_fd(pin_fd, 4096))
    finally:
        os.close(pin_fd)


def open_trusted_identity_file(path, maximum, public):
    parts = path_components(path)
    parent_fd = open_root_directory("/" + "/".join(parts[:-1]))
    try:
        file_fd = os.open(parts[-1], FILE_READ_FLAGS, dir_fd=parent_fd)
    except OSError:
        os.close(parent_fd)
        fail("trusted local identity file is missing or unsafe")
    os.close(parent_fd)
    item = os.fstat(file_fd)
    mode = stat.S_IMODE(item.st_mode)
    if (
        not stat.S_ISREG(item.st_mode)
        or item.st_uid != 0
        or item.st_nlink != 1
        or item.st_size <= 0
        or item.st_size > maximum
        or mode & 0o022
        or (public and (item.st_gid != 0 or mode != 0o644))
        or (not public and mode not in {0o600, 0o640})
    ):
        os.close(file_fd)
        fail("trusted local identity file owner/mode/nlink/size contract failed")
    return file_fd


def identity_lines(data, label, fields):
    if b"\0" in data:
        fail(f"{label} contains NUL")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        fail(f"{label} must be UTF-8")
    lines = text.splitlines()
    if not lines or len(lines) > 100_000:
        fail(f"{label} record count is outside its bound")
    records = []
    for line in lines:
        if not line or "\r" in line:
            fail(f"{label} contains an empty or non-canonical record")
        record = line.split(":")
        if len(record) != fields:
            fail(f"{label} contains a malformed record")
        records.append((line, record))
    return records


def parse_login_defs(data):
    if b"\0" in data:
        fail("login.defs contains NUL")
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError:
        fail("login.defs must be ASCII")
    wanted = {"SYS_UID_MIN", "SYS_UID_MAX", "SYS_GID_MIN", "SYS_GID_MAX"}
    values = {}
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        fields = line.split()
        if fields[0] not in wanted:
            continue
        if len(fields) != 2 or fields[0] in values or not fields[1].isdigit():
            fail("login.defs system identity range is malformed or duplicate")
        values[fields[0]] = int(fields[1], 10)
    if set(values) != wanted:
        fail("login.defs must define all system UID/GID range bounds")
    if not (
        1 <= values["SYS_UID_MIN"] <= values["SYS_UID_MAX"] < 2**31
        and 1 <= values["SYS_GID_MIN"] <= values["SYS_GID_MAX"] < 2**31
    ):
        fail("login.defs system UID/GID ranges are invalid")
    return values


def one_named_record(records, name, label):
    matches = [(line, fields) for line, fields in records if fields[0] == name]
    if len(matches) != 1:
        fail(f"{label} must contain exactly one local mir2 record")
    return matches[0]


def parse_decimal(value, label):
    if not re.fullmatch(r"[0-9]+", value):
        fail(f"{label} is not decimal")
    return int(value, 10)


def locked_marker(value):
    return bool(value) and value[0] in {"!", "*"}


def validate_nsswitch_files_only(data):
    if b"\0" in data:
        fail("nsswitch.conf contains NUL")
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError:
        fail("nsswitch.conf must be ASCII")
    required = {"passwd", "group", "shadow", "gshadow"}
    configured = {}
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if ":" not in line:
            fail("nsswitch.conf contains a malformed service line")
        database, sources = (part.strip() for part in line.split(":", 1))
        if database not in required:
            continue
        if database in configured:
            fail("nsswitch.conf contains a duplicate identity database")
        tokens = sources.split()
        if tokens != ["files"]:
            fail(
                "production identity uniqueness requires files-only "
                f"{database} NSS"
            )
        configured[database] = tokens
    if set(configured) != required:
        fail("nsswitch.conf must explicitly define all files-only identity databases")


def parse_identity_name_list(value, label):
    if not value:
        return []
    names = value.split(",")
    if any(
        not name or not re.fullmatch(r"[A-Za-z0-9_.-]+", name)
        for name in names
    ) or len(set(names)) != len(names):
        fail(f"{label} is malformed or duplicate")
    return names


def validate_identity_sources(
    passwd_bytes,
    group_bytes,
    shadow_bytes,
    gshadow_bytes,
    login_defs_bytes,
    nsswitch_bytes,
    nss_passwd_bytes,
    nss_group_bytes,
    getent_shadow,
    getent_gshadow,
    id_uid,
    id_gid,
    id_primary_name,
    id_all_gids,
    shadow_file_gid,
    gshadow_file_gid,
):
    local_passwd = identity_lines(passwd_bytes, "local passwd", 7)
    local_group = identity_lines(group_bytes, "local group", 4)
    local_shadow = identity_lines(shadow_bytes, "local shadow", 9)
    local_gshadow = identity_lines(gshadow_bytes, "local gshadow", 4)
    nss_passwd = identity_lines(nss_passwd_bytes, "NSS passwd", 7)
    nss_group = identity_lines(nss_group_bytes, "NSS group", 4)
    ranges = parse_login_defs(login_defs_bytes)
    validate_nsswitch_files_only(nsswitch_bytes)

    passwd_line, passwd = one_named_record(local_passwd, "mir2", "local passwd")
    group_line, group = one_named_record(local_group, "mir2", "local group")
    shadow_line, shadow = one_named_record(local_shadow, "mir2", "local shadow")
    gshadow_line, gshadow = one_named_record(
        local_gshadow,
        "mir2",
        "local gshadow",
    )
    if passwd[1] not in {"x", "!", "*", "!!", "!*"}:
        fail("local mir2 passwd field is not an explicit lock/delegation marker")
    if group[1] not in {"x", "!", "*", "!!", "!*"}:
        fail("local mir2 group password field is not an explicit lock marker")
    if not locked_marker(shadow[1]):
        fail("local mir2 shadow password is not locked")
    if not locked_marker(gshadow[1]):
        fail("local mir2 gshadow password is not locked")
    if group[3] or gshadow[2] or gshadow[3]:
        fail("mir2 primary group must not contain members or administrators")
    for _, fields in local_gshadow:
        administrators = parse_identity_name_list(
            fields[2],
            "local gshadow administrator list",
        )
        members = parse_identity_name_list(
            fields[3],
            "local gshadow member list",
        )
        if fields[0] != "mir2" and "mir2" in administrators + members:
            fail("mir2 appears in another local gshadow record")
    if passwd[5] != "/var/lib/mir2/gateway-data":
        fail("local mir2 home differs from the fixed service home")
    if passwd[6] not in {
        "/usr/sbin/nologin",
        "/sbin/nologin",
        "/bin/false",
        "/usr/bin/false",
    }:
        fail("local mir2 shell is interactive or unsupported")

    uid = parse_decimal(passwd[2], "mir2 UID")
    gid = parse_decimal(passwd[3], "mir2 primary GID")
    group_gid = parse_decimal(group[2], "mir2 group GID")
    if gid != group_gid:
        fail("local mir2 passwd/group primary GID differs")
    if not ranges["SYS_UID_MIN"] <= uid <= ranges["SYS_UID_MAX"]:
        fail("mir2 UID is outside trusted login.defs system range")
    if not ranges["SYS_GID_MIN"] <= gid <= ranges["SYS_GID_MAX"]:
        fail("mir2 GID is outside trusted login.defs system range")
    private_identity_gids = {
        parse_decimal(shadow_file_gid, "shadow file GID"),
        parse_decimal(gshadow_file_gid, "gshadow file GID"),
    }
    if gid in private_identity_gids:
        fail("private identity files are readable by the mir2 service group")

    for _, fields in local_passwd:
        other_uid = parse_decimal(fields[2], "local UID")
        other_gid = parse_decimal(fields[3], "local primary GID")
        if fields[0] != "mir2" and other_uid == uid:
            fail("another local account shares the mir2 UID")
        if fields[0] != "mir2" and other_gid == gid:
            fail("another local account uses the dedicated mir2 primary GID")
    for _, fields in local_group:
        other_gid = parse_decimal(fields[2], "local group GID")
        if fields[0] != "mir2" and other_gid == gid:
            fail("another local group shares the mir2 GID")

    nss_named_passwd = [
        (line, fields) for line, fields in nss_passwd if fields[0] == "mir2"
    ]
    nss_named_group = [
        (line, fields) for line, fields in nss_group if fields[0] == "mir2"
    ]
    if [line for line, _ in nss_passwd] != [line for line, _ in local_passwd]:
        fail("files-only NSS passwd enumeration differs from the local database")
    if [line for line, _ in nss_group] != [line for line, _ in local_group]:
        fail("files-only NSS group enumeration differs from the local database")
    if len(nss_named_passwd) != 1 or nss_named_passwd[0][0] != passwd_line:
        fail("NSS mir2 passwd source is duplicate, remote, or differs from local")
    if len(nss_named_group) != 1 or nss_named_group[0][0] != group_line:
        fail("NSS mir2 group source is duplicate, remote, or differs from local")
    for _, fields in nss_passwd:
        other_uid = parse_decimal(fields[2], "NSS UID")
        other_gid = parse_decimal(fields[3], "NSS primary GID")
        if fields[0] != "mir2" and other_uid == uid:
            fail("another NSS account shares the mir2 UID")
        if fields[0] != "mir2" and other_gid == gid:
            fail("another NSS account uses the dedicated mir2 primary GID")
    for _, fields in nss_group:
        other_gid = parse_decimal(fields[2], "NSS group GID")
        if fields[0] != "mir2" and other_gid == gid:
            fail("another NSS group shares the mir2 GID")
        if fields[3]:
            members = parse_identity_name_list(fields[3], "NSS group member list")
            if fields[0] != "mir2" and "mir2" in members:
                fail("mir2 appears in an additional NSS group member list")

    if getent_shadow != shadow_line or getent_gshadow != gshadow_line:
        fail("NSS shadow/gshadow source differs from trusted local records")
    if (
        id_uid != str(uid)
        or id_gid != str(gid)
        or id_primary_name != "mir2"
        or id_all_gids != str(gid)
    ):
        fail("id(1) mir2 identity or supplementary groups differ")
    return uid, gid


def run_identity_command(arguments, maximum):
    try:
        result = subprocess.run(
            arguments,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env={"PATH": "/usr/sbin:/usr/bin:/sbin:/bin", "LC_ALL": "C"},
            timeout=10,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        fail("fixed identity command failed")
    if result.returncode != 0 or len(result.stdout) > maximum:
        fail("fixed identity command failed or exceeded its output bound")
    try:
        return result.stdout.decode("utf-8").rstrip("\n")
    except UnicodeDecodeError:
        fail("fixed identity command output is not UTF-8")


def validate_production_identity():
    paths = (
        ("/etc/passwd", 4_194_304, True),
        ("/etc/group", 4_194_304, True),
        ("/etc/shadow", 4_194_304, False),
        ("/etc/gshadow", 4_194_304, False),
        ("/etc/login.defs", 1_048_576, True),
        ("/etc/nsswitch.conf", 1_048_576, True),
    )
    data = []
    identity_file_stats = []
    for path, maximum, public in paths:
        file_fd = open_trusted_identity_file(path, maximum, public)
        try:
            identity_file_stats.append(os.fstat(file_fd))
            data.append(read_all_fd(file_fd, maximum))
        finally:
            os.close(file_fd)
    nss_passwd = run_identity_command(["/usr/bin/getent", "passwd"], 4_194_304)
    nss_group = run_identity_command(["/usr/bin/getent", "group"], 4_194_304)
    values = (
        run_identity_command(["/usr/bin/getent", "shadow", "mir2"], 65_536),
        run_identity_command(["/usr/bin/getent", "gshadow", "mir2"], 65_536),
        run_identity_command(["/usr/bin/id", "-u", "mir2"], 128),
        run_identity_command(["/usr/bin/id", "-g", "mir2"], 128),
        run_identity_command(["/usr/bin/id", "-gn", "mir2"], 128),
        run_identity_command(["/usr/bin/id", "-G", "mir2"], 4096),
    )
    uid, gid = validate_identity_sources(
        *data,
        (nss_passwd + "\n").encode("utf-8"),
        (nss_group + "\n").encode("utf-8"),
        *values,
        str(identity_file_stats[2].st_gid),
        str(identity_file_stats[3].st_gid),
    )
    return str(uid), str(gid)


def inspect_identity_presence():
    local_data = []
    for path, maximum in (
        ("/etc/passwd", 4_194_304),
        ("/etc/group", 4_194_304),
        ("/etc/nsswitch.conf", 1_048_576),
    ):
        file_fd = open_trusted_identity_file(path, maximum, True)
        try:
            local_data.append(read_all_fd(file_fd, maximum))
        finally:
            os.close(file_fd)
    local_passwd = identity_lines(local_data[0], "local passwd", 7)
    local_group = identity_lines(local_data[1], "local group", 4)
    validate_nsswitch_files_only(local_data[2])
    nss_passwd_text = run_identity_command(
        ["/usr/bin/getent", "passwd"],
        4_194_304,
    )
    nss_group_text = run_identity_command(
        ["/usr/bin/getent", "group"],
        4_194_304,
    )
    nss_passwd = identity_lines(
        (nss_passwd_text + "\n").encode("utf-8"),
        "NSS passwd",
        7,
    )
    nss_group = identity_lines(
        (nss_group_text + "\n").encode("utf-8"),
        "NSS group",
        4,
    )
    if [line for line, _ in nss_passwd] != [line for line, _ in local_passwd]:
        fail("files-only NSS passwd enumeration differs from the local database")
    if [line for line, _ in nss_group] != [line for line, _ in local_group]:
        fail("files-only NSS group enumeration differs from the local database")
    local_users = [(line, fields) for line, fields in local_passwd if fields[0] == "mir2"]
    local_groups = [(line, fields) for line, fields in local_group if fields[0] == "mir2"]
    nss_users = [(line, fields) for line, fields in nss_passwd if fields[0] == "mir2"]
    nss_groups = [(line, fields) for line, fields in nss_group if fields[0] == "mir2"]
    if len(local_users) > 1 or len(local_groups) > 1:
        fail("local identity database contains duplicate mir2 names")
    if local_users:
        if len(nss_users) != 1 or nss_users[0][0] != local_users[0][0]:
            fail("NSS mir2 user is remote, duplicate, or differs from local")
    elif nss_users:
        fail("remote-only NSS mir2 user prevents safe local creation")
    if local_groups:
        if len(nss_groups) != 1 or nss_groups[0][0] != local_groups[0][0]:
            fail("NSS mir2 group is remote, duplicate, or differs from local")
    elif nss_groups:
        fail("remote-only NSS mir2 group prevents safe local creation")
    if bool(local_users) != bool(local_groups):
        fail("partial local mir2 user/group state requires manual recovery")
    return (
        "present" if local_users else "missing",
        "present" if local_groups else "missing",
    )


def build_curl_arguments(url):
    return [
        CURL_PATH,
        "-q",
        "--fail",
        "--show-error",
        "--silent",
        "--proto",
        "=https",
        "--connect-timeout",
        "10",
        "--max-time",
        "180",
        "--retry",
        "2",
        "--retry-delay",
        "1",
        "--retry-max-time",
        "180",
        "--",
        validate_https_url(url),
    ]


def require_download_limit(maximum):
    if not isinstance(maximum, int) or maximum < 1 or maximum > 536_870_912:
        fail("download byte limit is outside the fixed production bound")
    return maximum


def open_test_executable(path):
    try:
        executable_fd = os.open(path, FILE_READ_FLAGS)
    except OSError:
        fail("test downloader is missing or a symbolic link")
    executable_stat = os.fstat(executable_fd)
    mode = stat.S_IMODE(executable_stat.st_mode)
    if (
        not stat.S_ISREG(executable_stat.st_mode)
        or executable_stat.st_uid != os.geteuid()
        or executable_stat.st_nlink != 1
        or not mode & 0o100
        or mode & 0o022
    ):
        os.close(executable_fd)
        fail("test downloader must be an owner-executable independent regular file")
    return executable_fd


def download_child_limits(maximum):
    resource.setrlimit(resource.RLIMIT_FSIZE, (maximum, maximum))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    signal.signal(signal.SIGXFSZ, signal.SIG_DFL)
    os.umask(0o077)


def stream_download_to_directory(
    curl_fd,
    directory_fd,
    url,
    maximum,
    expected_uid,
    expected_gid,
):
    require_linux_dirfd()
    maximum = require_download_limit(maximum)
    output_fd = None
    completed = False
    try:
        try:
            output_fd = os.open(
                DOWNLOAD_NAME,
                FILE_CREATE_RW_FLAGS,
                0o600,
                dir_fd=directory_fd,
            )
        except OSError:
            fail("download destination was not fresh and exclusive")
        os.fchown(output_fd, expected_uid, expected_gid)
        os.fchmod(output_fd, 0o600)

        try:
            result = subprocess.run(
                build_curl_arguments(url),
                executable=f"/proc/self/fd/{curl_fd}",
                stdin=subprocess.DEVNULL,
                stdout=output_fd,
                stderr=None,
                close_fds=True,
                pass_fds=(curl_fd,),
                env={"PATH": "/usr/sbin:/usr/bin:/sbin:/bin", "LC_ALL": "C"},
                preexec_fn=lambda: download_child_limits(maximum),
                check=False,
            )
        except (OSError, subprocess.SubprocessError):
            fail("bounded release downloader could not be executed")
        if result.returncode != 0:
            fail("bounded release download failed")

        output_stat = os.fstat(output_fd)
        if (
            not stat.S_ISREG(output_stat.st_mode)
            or output_stat.st_uid != expected_uid
            or output_stat.st_gid != expected_gid
            or output_stat.st_nlink != 1
            or stat.S_IMODE(output_stat.st_mode) != 0o600
            or output_stat.st_size <= 0
            or output_stat.st_size > maximum
        ):
            fail("download output violated its owner/mode/nlink/size contract")
        os.fsync(output_fd)
        os.fsync(directory_fd)
        completed = True
    finally:
        if output_fd is not None:
            if not completed:
                try:
                    os.ftruncate(output_fd, 0)
                    os.fsync(output_fd)
                except OSError:
                    pass
            os.close(output_fd)
        if not completed:
            try:
                os.unlink(DOWNLOAD_NAME, dir_fd=directory_fd)
                os.fsync(directory_fd)
            except FileNotFoundError:
                pass


def open_var_tmp():
    var_fd = open_root_directory("/var")
    try:
        try:
            temp_fd = os.open("tmp", DIR_FLAGS, dir_fd=var_fd)
        except OSError:
            fail("/var/tmp is missing, non-directory, or a symbolic link")
    finally:
        os.close(var_fd)
    temp_stat = os.fstat(temp_fd)
    if (
        not stat.S_ISDIR(temp_stat.st_mode)
        or temp_stat.st_uid != 0
        or temp_stat.st_gid != 0
        or stat.S_IMODE(temp_stat.st_mode) != 0o1777
    ):
        os.close(temp_fd)
        fail("/var/tmp must be root:root mode 1777 with sticky-bit isolation")
    return temp_fd


def bounded_directory_names(directory_fd, maximum, label):
    names = []
    with os.scandir(directory_fd) as entries:
        for entry in entries:
            names.append(entry.name)
            if len(names) > maximum:
                fail(f"{label} entry-count bound exceeded")
    return names


def read_boot_id():
    try:
        boot_fd = os.open(
            "/proc/sys/kernel/random/boot_id",
            FILE_READ_FLAGS,
        )
    except OSError:
        fail("Linux boot identity is unavailable")
    try:
        value = read_all_fd(boot_fd, 128).decode("ascii").strip()
    except UnicodeDecodeError:
        fail("Linux boot identity is malformed")
    finally:
        os.close(boot_fd)
    if not re.fullmatch(
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        value,
    ):
        fail("Linux boot identity is malformed")
    return value


def process_start_ticks(process_id):
    if not isinstance(process_id, int) or process_id < 1:
        fail("residue owner process id is invalid")
    try:
        stat_fd = os.open(f"/proc/{process_id}/stat", FILE_READ_FLAGS)
    except FileNotFoundError:
        return None
    except OSError:
        fail("residue owner process identity cannot be inspected")
    try:
        raw = read_all_fd(stat_fd, 16_384)
    finally:
        os.close(stat_fd)
    closing = raw.rfind(b") ")
    if closing < 1:
        fail("residue owner process identity is malformed")
    fields = raw[closing + 2:].split()
    if len(fields) <= 19 or not fields[19].isdigit():
        fail("residue owner process start time is malformed")
    return int(fields[19], 10)


def residue_marker_value(kind, directory_name):
    owner_pid = os.getppid()
    owner_start = process_start_ticks(owner_pid)
    if owner_start is None:
        fail("installer parent process disappeared before transaction creation")
    return {
        "version": RESIDUE_VERSION,
        "kind": kind,
        "directory": directory_name,
        "created": int(time.time()),
        "boot_id": read_boot_id(),
        "owner_pid": owner_pid,
        "owner_start_ticks": owner_start,
    }


def write_residue_marker(directory_fd, marker_name, marker, uid, gid):
    marker_bytes = (
        json.dumps(marker, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")
    marker_fd = os.open(
        marker_name,
        FILE_CREATE_RW_FLAGS,
        0o600,
        dir_fd=directory_fd,
    )
    try:
        os.fchown(marker_fd, uid, gid)
        os.fchmod(marker_fd, 0o600)
        write_all(marker_fd, marker_bytes)
        os.fsync(marker_fd)
        marker_stat = os.fstat(marker_fd)
        if (
            not stat.S_ISREG(marker_stat.st_mode)
            or marker_stat.st_uid != uid
            or marker_stat.st_gid != gid
            or marker_stat.st_nlink != 1
            or stat.S_IMODE(marker_stat.st_mode) != 0o600
        ):
            fail("installer residue marker identity contract failed")
    finally:
        os.close(marker_fd)
    os.fsync(directory_fd)


def read_residue_marker(
    directory_fd,
    marker_name,
    expected_kind,
    expected_directory,
    uid,
    gid,
):
    marker_fd = os.open(marker_name, FILE_READ_FLAGS, dir_fd=directory_fd)
    try:
        marker_stat = os.fstat(marker_fd)
        if (
            not stat.S_ISREG(marker_stat.st_mode)
            or marker_stat.st_uid != uid
            or marker_stat.st_gid != gid
            or marker_stat.st_nlink != 1
            or stat.S_IMODE(marker_stat.st_mode) != 0o600
        ):
            fail("installer residue marker identity changed")
        try:
            marker = json.loads(read_all_fd(marker_fd, 16_384).decode("ascii"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            fail("installer residue marker is malformed")
    finally:
        os.close(marker_fd)
    if (
        not isinstance(marker, dict)
        or type(marker.get("version")) is not int
        or marker.get("version") != RESIDUE_VERSION
        or marker.get("kind") != expected_kind
        or marker.get("directory") != expected_directory
        or type(marker.get("created")) is not int
        or type(marker.get("owner_pid")) is not int
        or type(marker.get("owner_start_ticks")) is not int
        or not isinstance(marker.get("boot_id"), str)
    ):
        fail("installer residue marker schema mismatch")
    return marker, marker_stat.st_size


def residue_owner_is_live(marker):
    if marker["boot_id"] != read_boot_id():
        return False
    observed = process_start_ticks(marker["owner_pid"])
    return observed is not None and observed == marker["owner_start_ticks"]


def require_residue_directory(directory_fd, uid, gid, modes, label):
    item = os.fstat(directory_fd)
    if (
        not stat.S_ISDIR(item.st_mode)
        or item.st_uid != uid
        or item.st_gid != gid
        or item.st_nlink < 2
        or stat.S_IMODE(item.st_mode) not in modes
    ):
        fail(f"{label} directory identity contract failed")
    return item


def validate_residue_regular(
    directory_fd,
    name,
    uid,
    gid,
    expected_mode,
    maximum,
    allow_empty,
):
    allowed_modes = (
        {expected_mode}
        if isinstance(expected_mode, int)
        else set(expected_mode)
    )
    if not allowed_modes or any(mode & 0o022 for mode in allowed_modes):
        fail("installer residue mode contract is unsafe")
    file_fd = os.open(name, FILE_READ_FLAGS, dir_fd=directory_fd)
    try:
        item = os.fstat(file_fd)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != uid
            or item.st_gid != gid
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) not in allowed_modes
            or item.st_size < (0 if allow_empty else 1)
            or item.st_size > maximum
        ):
            fail("installer residue file identity/size contract failed")
        return item.st_size
    finally:
        os.close(file_fd)


def inspect_download_residue(directory_fd, directory_name, uid, gid, maximum):
    require_residue_directory(
        directory_fd,
        uid,
        gid,
        {0o700},
        "download residue",
    )
    names = bounded_directory_names(directory_fd, 2, "download residue")
    if any(name not in {DOWNLOAD_MARKER, DOWNLOAD_NAME} for name in names):
        fail("download residue contains an unknown entry")
    marker, marker_size = read_residue_marker(
        directory_fd,
        DOWNLOAD_MARKER,
        "gateway-download",
        directory_name,
        uid,
        gid,
    )
    total = marker_size
    if DOWNLOAD_NAME in names:
        total += validate_residue_regular(
            directory_fd,
            DOWNLOAD_NAME,
            uid,
            gid,
            0o600,
            maximum,
            True,
        )
    return marker, total


def cleanup_download_residue(
    parent_fd,
    directory_name,
    directory_fd,
    uid,
    gid,
    maximum,
):
    inspect_download_residue(
        directory_fd,
        directory_name,
        uid,
        gid,
        maximum,
    )
    for name in (DOWNLOAD_NAME, DOWNLOAD_MARKER):
        try:
            os.unlink(name, dir_fd=directory_fd)
        except FileNotFoundError:
            pass
    os.fsync(directory_fd)
    os.rmdir(directory_name, dir_fd=parent_fd)
    os.fsync(parent_fd)


def sweep_download_residues(parent_fd, uid, gid, maximum):
    now = int(time.time())
    matched = 0
    active = 0
    total = 0
    with os.scandir(parent_fd) as entries:
        for entry in entries:
            if not entry.name.startswith(DOWNLOAD_PREFIX):
                continue
            try:
                entry_stat = os.stat(
                    entry.name,
                    dir_fd=parent_fd,
                    follow_symlinks=False,
                )
            except FileNotFoundError:
                continue
            if reserved_prefix_owner_action(entry_stat.st_uid, uid) == "ignore":
                continue
            matched += 1
            if matched > MAX_RESIDUE_COUNT * 4:
                fail("download residue scan bound exceeded")
            if not re.fullmatch(r"mir2-gateway-install\.[0-9a-f]{24}", entry.name):
                fail("unknown entry uses the reserved download prefix")
            try:
                directory_fd = os.open(entry.name, DIR_FLAGS, dir_fd=parent_fd)
            except OSError:
                fail("download residue is a symlink or non-directory")
            try:
                directory_stat = require_residue_directory(
                    directory_fd,
                    uid,
                    gid,
                    {0o700},
                    "download residue",
                )
                try:
                    marker, residue_bytes = inspect_download_residue(
                        directory_fd,
                        entry.name,
                        uid,
                        gid,
                        maximum,
                    )
                except FileNotFoundError:
                    names = bounded_directory_names(
                        directory_fd,
                        1,
                        "unmarked download residue",
                    )
                    if names:
                        fail("unmarked download residue is not empty")
                    if now - int(directory_stat.st_ctime) >= MAX_ACTIVE_RESIDUE_AGE:
                        os.rmdir(entry.name, dir_fd=parent_fd)
                        os.fsync(parent_fd)
                        continue
                    active += 1
                    continue
                age = now - marker["created"]
                if age < -MAX_FUTURE_CLOCK_SKEW:
                    fail("download residue marker timestamp is in the future")
                if residue_owner_is_live(marker):
                    if age > MAX_ACTIVE_RESIDUE_AGE:
                        fail("live download residue exceeded its maximum age")
                    active += 1
                    total += residue_bytes
                else:
                    cleanup_download_residue(
                        parent_fd,
                        entry.name,
                        directory_fd,
                        uid,
                        gid,
                        maximum,
                    )
            finally:
                os.close(directory_fd)
    if active > MAX_RESIDUE_COUNT or total > MAX_RESIDUAL_BYTES:
        fail("active download residue count/byte bound exceeded")


def reserved_prefix_owner_action(observed_uid, expected_uid):
    if (
        not isinstance(observed_uid, int)
        or not isinstance(expected_uid, int)
        or observed_uid < 0
        or expected_uid < 0
    ):
        fail("reserved-prefix owner classifier received an invalid UID")
    return "owned" if observed_uid == expected_uid else "ignore"


def create_production_download(url, maximum):
    require_linux_dirfd()
    if os.geteuid() != 0 or os.getegid() != 0:
        fail("production downloader requires root identity")
    curl_fd = open_trusted_regular(CURL_PATH, 0o755, 67_108_864)
    temp_fd = open_var_tmp()
    directory_fd = None
    directory_name = None
    try:
        sweep_download_residues(temp_fd, 0, 0, maximum)
        for _ in range(32):
            candidate = DOWNLOAD_PREFIX + secrets.token_hex(12)
            try:
                os.mkdir(candidate, 0o700, dir_fd=temp_fd)
                directory_name = candidate
                break
            except FileExistsError:
                continue
        if directory_name is None:
            fail("could not allocate a fresh root-private download directory")
        directory_fd = os.open(directory_name, DIR_FLAGS, dir_fd=temp_fd)
        os.fchown(directory_fd, 0, 0)
        os.fchmod(directory_fd, 0o700)
        directory_stat = os.fstat(directory_fd)
        if (
            not stat.S_ISDIR(directory_stat.st_mode)
            or directory_stat.st_uid != 0
            or directory_stat.st_gid != 0
            or directory_stat.st_nlink < 2
            or stat.S_IMODE(directory_stat.st_mode) != 0o700
        ):
            fail("root-private download directory identity contract failed")
        write_residue_marker(
            directory_fd,
            DOWNLOAD_MARKER,
            residue_marker_value("gateway-download", directory_name),
            0,
            0,
        )
        os.fsync(temp_fd)
        stream_download_to_directory(
            curl_fd,
            directory_fd,
            url,
            maximum,
            0,
            0,
        )
        return f"{VAR_TMP}/{directory_name}/{DOWNLOAD_NAME}", directory_name
    except Exception:
        if directory_fd is not None:
            try:
                cleanup_download_residue(
                    temp_fd,
                    directory_name,
                    directory_fd,
                    0,
                    0,
                    maximum,
                )
            except FileNotFoundError:
                pass
            os.close(directory_fd)
            directory_fd = None
        elif directory_name is not None:
            try:
                os.rmdir(directory_name, dir_fd=temp_fd)
                os.fsync(temp_fd)
            except FileNotFoundError:
                pass
        raise
    finally:
        if directory_fd is not None:
            os.close(directory_fd)
        os.close(temp_fd)
        os.close(curl_fd)


def cleanup_production_download(directory_name):
    require_linux_dirfd()
    if os.geteuid() != 0 or os.getegid() != 0:
        fail("production download cleanup requires root identity")
    if not re.fullmatch(r"mir2-gateway-install\.[0-9a-f]{24}", directory_name):
        fail("download cleanup target is not an installer-owned component")
    temp_fd = open_var_tmp()
    try:
        try:
            directory_fd = os.open(directory_name, DIR_FLAGS, dir_fd=temp_fd)
        except FileNotFoundError:
            return
        except OSError:
            fail("download cleanup target is unsafe")
        try:
            cleanup_download_residue(
                temp_fd,
                directory_name,
                directory_fd,
                0,
                0,
                536_870_912,
            )
        finally:
            os.close(directory_fd)
    finally:
        os.close(temp_fd)


def download_test(curl_path, directory_path, url, maximum):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("download selftest must not run as root")
    directory_fd = os.open(directory_path, DIR_FLAGS)
    try:
        directory_stat = os.fstat(directory_fd)
        if (
            not stat.S_ISDIR(directory_stat.st_mode)
            or directory_stat.st_uid != os.geteuid()
            or directory_stat.st_gid != os.getegid()
            or stat.S_IMODE(directory_stat.st_mode) != 0o700
        ):
            fail("download selftest directory must be caller-owned mode 0700")
        curl_fd = open_test_executable(curl_path)
        try:
            stream_download_to_directory(
                curl_fd,
                directory_fd,
                url,
                int(maximum),
                os.geteuid(),
                os.getegid(),
            )
        finally:
            os.close(curl_fd)
    finally:
        os.close(directory_fd)


def parse_octal(field, label):
    if field and field[0] & 0x80:
        fail(f"{label} uses unsupported base-256 encoding")
    stripped = field.strip(b"\0 ")
    if not stripped:
        return 0
    if any(byte < ord("0") or byte > ord("7") for byte in stripped):
        fail(f"{label} is not canonical octal")
    return int(stripped, 8)


class BoundedGzipReader:
    def __init__(self, stream, maximum):
        self.stream = stream
        self.maximum = maximum
        self.total = 0

    def read_exact(self, size):
        if size < 0 or self.total + size > self.maximum:
            fail("archive expanded-byte limit exceeded")
        output = bytearray()
        while len(output) < size:
            chunk = self.stream.read(size - len(output))
            if not chunk:
                fail("archive ended before the declared member size")
            output.extend(chunk)
        self.total += size
        return bytes(output)

    def drain_zero_trailer(self):
        while True:
            remaining = self.maximum - self.total
            chunk = self.stream.read(min(65_536, remaining + 1))
            if not chunk:
                return
            self.total += len(chunk)
            if self.total > self.maximum:
                fail("archive expanded-byte limit exceeded")
            if any(chunk):
                fail("archive has non-zero data after its end markers")


def parse_header_name(header):
    raw_name = header[0:100].split(b"\0", 1)[0]
    raw_prefix = header[345:500].split(b"\0", 1)[0]
    if raw_prefix:
        fail("archive member prefix/PAX-style names are not allowed")
    if not raw_name or len(raw_name) > 100:
        fail("archive member name length is invalid")
    try:
        name = raw_name.decode("ascii")
    except UnicodeDecodeError:
        fail("archive member names must be ASCII")
    if name != os.path.basename(name) or name in {".", ".."}:
        fail("archive member name is not one safe component")
    return name


def validate_header_checksum(header):
    expected = parse_octal(header[148:156], "tar header checksum")
    calculated = sum(header[:148]) + (ord(" ") * 8) + sum(header[156:])
    if expected != calculated:
        fail("archive tar header checksum mismatch")


def write_all(file_fd, data):
    view = memoryview(data)
    while view:
        written = os.write(file_fd, view)
        if written <= 0:
            fail("short write while installing release file")
        view = view[written:]


def parse_archive_from_fd(
    archive_fd,
    expanded_limit,
    member_limit,
    release_fd=None,
    release_uid=0,
    release_gid=0,
):
    if member_limit < len(EXPECTED_MEMBERS):
        fail("archive member-count limit is below the required contract")
    os.lseek(archive_fd, 0, os.SEEK_SET)
    results = {}
    seen = set()
    member_count = 0
    declared_total = 0
    with os.fdopen(os.dup(archive_fd), "rb") as raw_file:
        gzip_stream = None
        try:
            gzip_stream = gzip.GzipFile(fileobj=raw_file, mode="rb")
            reader = BoundedGzipReader(gzip_stream, expanded_limit)
            while True:
                header = reader.read_exact(512)
                if header == b"\0" * 512:
                    second = reader.read_exact(512)
                    if second != b"\0" * 512:
                        fail("archive must end with two zero tar blocks")
                    reader.drain_zero_trailer()
                    break

                member_count += 1
                if member_count > member_limit:
                    fail("archive member-count limit exceeded")
                validate_header_checksum(header)
                if header[257:262] != b"ustar":
                    fail("archive must use bounded ustar headers")
                type_flag = header[156:157]
                if type_flag not in {b"\0", b"0"}:
                    fail("archive links, directories, PAX, and special members are forbidden")
                name = parse_header_name(header)
                if name not in EXPECTED_MEMBERS:
                    fail("archive contains an unexpected helper, unit, or file")
                if name in seen:
                    fail("archive contains a duplicate member")
                if header[157:257].split(b"\0", 1)[0]:
                    fail("archive regular members must not carry link targets")

                size = parse_octal(header[124:136], "tar member size")
                fixed_mode, member_maximum = EXPECTED_MEMBERS[name]
                if size < 0 or size > member_maximum:
                    fail("archive member exceeds its fixed size limit")
                declared_total += size
                if declared_total > expanded_limit:
                    fail("archive declared expanded bytes exceed the limit")

                output_fd = None
                if release_fd is not None:
                    try:
                        output_fd = os.open(
                            name,
                            FILE_CREATE_FLAGS,
                            fixed_mode,
                            dir_fd=release_fd,
                        )
                    except OSError:
                        fail("release destination is not fresh and exclusive")
                    os.fchown(output_fd, release_uid, release_gid)
                    os.fchmod(output_fd, fixed_mode)

                digest = hashlib.sha256()
                capture = bytearray()
                remaining = size
                try:
                    while remaining:
                        chunk = reader.read_exact(min(1_048_576, remaining))
                        remaining -= len(chunk)
                        digest.update(chunk)
                        if name in CAPTURE_MEMBERS:
                            capture.extend(chunk)
                        if output_fd is not None:
                            write_all(output_fd, chunk)
                    if output_fd is not None:
                        os.fsync(output_fd)
                        installed_stat = os.fstat(output_fd)
                        if (
                            not stat.S_ISREG(installed_stat.st_mode)
                            or installed_stat.st_uid != release_uid
                            or installed_stat.st_gid != release_gid
                            or installed_stat.st_nlink != 1
                            or stat.S_IMODE(installed_stat.st_mode) != fixed_mode
                        ):
                            fail("installed release file identity contract failed")
                finally:
                    if output_fd is not None:
                        os.close(output_fd)

                padding_size = (-size) % 512
                if padding_size:
                    padding = reader.read_exact(padding_size)
                    if any(padding):
                        fail("archive member padding must be zero")
                results[name] = {
                    "sha256": digest.hexdigest(),
                    "data": bytes(capture),
                    "size": size,
                }
                seen.add(name)
        except (OSError, EOFError, gzip.BadGzipFile) as exc:
            fail(f"archive gzip/ustar parse failed: {type(exc).__name__}")
        finally:
            if gzip_stream is not None:
                gzip_stream.close()

    if seen != set(EXPECTED_MEMBERS) or member_count != len(EXPECTED_MEMBERS):
        fail("archive must contain exactly the four allowlisted members")
    return results


def validate_release_manifest(results, expected_tag):
    try:
        manifest = json.loads(results["RELEASE.json"]["data"].decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        fail("RELEASE.json is not valid UTF-8 JSON")
    if not isinstance(manifest, dict):
        fail("RELEASE.json must be an object")
    if manifest.get("name") != "mir2-gateway":
        fail("RELEASE.json package name mismatch")
    if manifest.get("tag") != expected_tag:
        fail("RELEASE.json tag differs from the trusted pin")
    if manifest.get("target") not in ALLOWED_TARGETS:
        fail("RELEASE.json target is not allowlisted")
    if manifest.get("binarySha256") != results["mir2-gateway"]["sha256"]:
        fail("Gateway binary hash differs from RELEASE.json")
    if manifest.get("zoneHostBinarySha256") != results["zone_host"]["sha256"]:
        fail("Zone Host binary hash differs from RELEASE.json")
    installation = manifest.get("installation")
    if not isinstance(installation, dict):
        fail("RELEASE.json installation contract is missing")
    required_contract = {
        "archiveContainsInstaller": False,
        "archiveContainsSystemdUnit": False,
        "archiveContainsEnvironmentTemplate": False,
        "requiresRootOwnedPinManifest": True,
        "checksumSidecarIsAuthority": False,
        "rootPinRehashFromArchiveFdRequired": True,
        "publisherUidTrustBoundaryRequired": True,
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
    }
    for key, expected in required_contract.items():
        if installation.get(key) is not expected:
            fail("RELEASE.json trust-boundary contract mismatch")


def open_archive(path, expected_sha, archive_limit, require_root_owner):
    expected_sha = validate_sha256(expected_sha)
    try:
        archive_fd = os.open(path, FILE_READ_FLAGS)
    except OSError:
        fail("release archive is missing or a symbolic link")
    archive_stat = os.fstat(archive_fd)
    mode = stat.S_IMODE(archive_stat.st_mode)
    if (
        not stat.S_ISREG(archive_stat.st_mode)
        or archive_stat.st_nlink != 1
        or archive_stat.st_size <= 0
        or archive_stat.st_size > archive_limit
    ):
        os.close(archive_fd)
        fail("release archive type/nlink/size contract failed")
    if require_root_owner and (
        archive_stat.st_uid != 0
        or archive_stat.st_gid != 0
        or mode != 0o600
    ):
        os.close(archive_fd)
        fail("root-private release archive owner/mode contract failed")

    digest = hashlib.sha256()
    while True:
        chunk = os.read(archive_fd, 1_048_576)
        if not chunk:
            break
        digest.update(chunk)
    if not hmac.compare_digest(digest.hexdigest(), expected_sha):
        os.close(archive_fd)
        fail("release archive does not match the root-owned trust pin")
    os.lseek(archive_fd, 0, os.SEEK_SET)
    return archive_fd


def ensure_root_chain(path):
    parts = path_components(path)
    current_fd = os.open("/", DIR_FLAGS)
    try:
        require_root_directory_stat(os.fstat(current_fd), "/")
        for component in parts:
            created = False
            try:
                next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
            except FileNotFoundError:
                try:
                    os.mkdir(component, 0o755, dir_fd=current_fd)
                    created = True
                except FileExistsError:
                    pass
                try:
                    next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
                except OSError:
                    fail("root directory creation raced with an unsafe component")
            except OSError:
                fail("root directory traversal encountered an unsafe component")
            if created:
                os.fchown(next_fd, 0, 0)
                os.fchmod(next_fd, 0o755)
            require_root_directory_stat(os.fstat(next_fd), component)
            os.close(current_fd)
            current_fd = next_fd
        return current_fd
    except Exception:
        os.close(current_fd)
        raise


def ensure_service_leaf(parent_path, name, uid, gid, expected_mode):
    safe_component(name, "service directory", 63)
    parent_fd = ensure_root_chain(parent_path)
    try:
        created = False
        try:
            leaf_fd = os.open(name, DIR_FLAGS, dir_fd=parent_fd)
        except FileNotFoundError:
            try:
                os.mkdir(name, expected_mode, dir_fd=parent_fd)
                created = True
            except FileExistsError:
                pass
            try:
                leaf_fd = os.open(name, DIR_FLAGS, dir_fd=parent_fd)
            except OSError:
                fail("service directory creation raced with an unsafe component")
        except OSError:
            fail("service directory is a symbolic link or non-directory")
        leaf_stat = os.fstat(leaf_fd)
        if created:
            os.fchown(leaf_fd, uid, gid)
            os.fchmod(leaf_fd, expected_mode)
            leaf_stat = os.fstat(leaf_fd)
        if (
            not stat.S_ISDIR(leaf_stat.st_mode)
            or leaf_stat.st_uid != uid
            or leaf_stat.st_gid != gid
            or stat.S_IMODE(leaf_stat.st_mode) != expected_mode
        ):
            fail("existing service directory requires manual ownership migration")
        os.fsync(leaf_fd)
        os.close(leaf_fd)
        os.fsync(parent_fd)
    finally:
        os.close(parent_fd)


def hash_fd(file_fd):
    os.lseek(file_fd, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    while True:
        chunk = os.read(file_fd, 1_048_576)
        if not chunk:
            return digest.hexdigest()
        digest.update(chunk)


def require_release_directory_stat(file_stat, uid, gid, mode, label):
    if (
        not stat.S_ISDIR(file_stat.st_mode)
        or file_stat.st_uid != uid
        or file_stat.st_gid != gid
        or file_stat.st_nlink < 2
        or stat.S_IMODE(file_stat.st_mode) != mode
    ):
        fail(f"{label} owner/mode contract failed")


def inspect_release_payload(payload_fd, uid, gid):
    payload_stat = os.fstat(payload_fd)
    payload_mode = stat.S_IMODE(payload_stat.st_mode)
    if payload_mode not in {0o700, 0o755}:
        fail("unpublished release payload mode changed")
    require_release_directory_stat(
        payload_stat,
        uid,
        gid,
        payload_mode,
        "unpublished release payload",
    )
    names = bounded_directory_names(payload_fd, len(EXPECTED_MEMBERS), "release payload")
    if any(name not in EXPECTED_MEMBERS for name in names):
        fail("unpublished release payload contains an unknown entry")
    total = 0
    for name in names:
        expected_mode, maximum = EXPECTED_MEMBERS[name]
        total += validate_residue_regular(
            payload_fd,
            name,
            uid,
            gid,
            {expected_mode, expected_mode & 0o700},
            maximum,
            True,
        )
    return total


def inspect_unpublished_release(transaction_fd, name, uid, gid):
    require_residue_directory(
        transaction_fd,
        uid,
        gid,
        {0o700},
        "unpublished release transaction",
    )
    names = bounded_directory_names(
        transaction_fd,
        2,
        "unpublished release transaction",
    )
    if any(item not in {INCOMING_MARKER, INCOMING_PAYLOAD} for item in names):
        fail("unpublished release transaction contains an unknown entry")
    marker, marker_size = read_residue_marker(
        transaction_fd,
        INCOMING_MARKER,
        "gateway-release",
        name,
        uid,
        gid,
    )
    total = marker_size
    if INCOMING_PAYLOAD in names:
        payload_fd = os.open(INCOMING_PAYLOAD, DIR_FLAGS, dir_fd=transaction_fd)
        try:
            total += inspect_release_payload(payload_fd, uid, gid)
        finally:
            os.close(payload_fd)
    return marker, total


def cleanup_release_payload(transaction_fd, payload_fd, uid, gid):
    inspect_release_payload(payload_fd, uid, gid)
    for name in bounded_directory_names(
        payload_fd,
        len(EXPECTED_MEMBERS),
        "release payload cleanup",
    ):
        validate_residue_regular(
            payload_fd,
            name,
            uid,
            gid,
            {
                EXPECTED_MEMBERS[name][0],
                EXPECTED_MEMBERS[name][0] & 0o700,
            },
            EXPECTED_MEMBERS[name][1],
            True,
        )
        os.unlink(name, dir_fd=payload_fd)
    os.fsync(payload_fd)
    path_stat = os.stat(
        INCOMING_PAYLOAD,
        dir_fd=transaction_fd,
        follow_symlinks=False,
    )
    held_stat = os.fstat(payload_fd)
    if (
        not stat.S_ISDIR(path_stat.st_mode)
        or (path_stat.st_dev, path_stat.st_ino)
        != (held_stat.st_dev, held_stat.st_ino)
    ):
        fail("unpublished release payload identity changed during cleanup")
    os.rmdir(INCOMING_PAYLOAD, dir_fd=transaction_fd)
    os.fsync(transaction_fd)


def cleanup_unpublished_release(releases_fd, name, transaction_fd, uid, gid):
    if not re.fullmatch(r"incoming\.[0-9a-f]{24}", name):
        fail("unpublished release cleanup target is unsafe")
    inspect_unpublished_release(transaction_fd, name, uid, gid)
    try:
        payload_fd = os.open(INCOMING_PAYLOAD, DIR_FLAGS, dir_fd=transaction_fd)
    except FileNotFoundError:
        payload_fd = None
    except OSError:
        fail("unpublished release payload is unsafe")
    if payload_fd is not None:
        try:
            cleanup_release_payload(transaction_fd, payload_fd, uid, gid)
        finally:
            os.close(payload_fd)
    os.unlink(INCOMING_MARKER, dir_fd=transaction_fd)
    os.fsync(transaction_fd)
    os.rmdir(name, dir_fd=releases_fd)
    os.fsync(releases_fd)


def sweep_unpublished_releases(releases_fd, uid, gid):
    now = int(time.time())
    matched = 0
    active = 0
    total = 0
    with os.scandir(releases_fd) as entries:
        for entry in entries:
            if not entry.name.startswith(INCOMING_PREFIX):
                continue
            matched += 1
            if matched > MAX_RESIDUE_COUNT * 4:
                fail("unpublished release residue scan bound exceeded")
            if not re.fullmatch(r"incoming\.[0-9a-f]{24}", entry.name):
                fail("unknown entry uses the reserved unpublished-release prefix")
            try:
                transaction_fd = os.open(entry.name, DIR_FLAGS, dir_fd=releases_fd)
            except OSError:
                fail("unpublished release residue is a symlink or non-directory")
            try:
                transaction_stat = require_residue_directory(
                    transaction_fd,
                    uid,
                    gid,
                    {0o700},
                    "unpublished release transaction",
                )
                try:
                    marker, residue_bytes = inspect_unpublished_release(
                        transaction_fd,
                        entry.name,
                        uid,
                        gid,
                    )
                except FileNotFoundError:
                    names = bounded_directory_names(
                        transaction_fd,
                        1,
                        "unmarked unpublished release",
                    )
                    if names:
                        fail("unmarked unpublished release is not empty")
                    if now - int(transaction_stat.st_ctime) >= MAX_ACTIVE_RESIDUE_AGE:
                        os.rmdir(entry.name, dir_fd=releases_fd)
                        os.fsync(releases_fd)
                        continue
                    active += 1
                    continue
                age = now - marker["created"]
                if age < -MAX_FUTURE_CLOCK_SKEW:
                    fail("unpublished release marker timestamp is in the future")
                if residue_owner_is_live(marker):
                    if age > MAX_ACTIVE_RESIDUE_AGE:
                        fail("live unpublished release exceeded its maximum age")
                    active += 1
                    total += residue_bytes
                else:
                    cleanup_unpublished_release(
                        releases_fd,
                        entry.name,
                        transaction_fd,
                        uid,
                        gid,
                    )
            finally:
                os.close(transaction_fd)
    if active > MAX_RESIDUE_COUNT or total > MAX_RESIDUAL_BYTES:
        fail("active unpublished release count/byte bound exceeded")


def create_unpublished_release(releases_fd, uid, gid):
    sweep_unpublished_releases(releases_fd, uid, gid)
    for _ in range(32):
        name = INCOMING_PREFIX + secrets.token_hex(12)
        try:
            os.mkdir(name, 0o700, dir_fd=releases_fd)
            break
        except FileExistsError:
            continue
    else:
        fail("could not allocate an unpublished release transaction")
    transaction_fd = None
    payload_fd = None
    try:
        transaction_fd = os.open(name, DIR_FLAGS, dir_fd=releases_fd)
        os.fchown(transaction_fd, uid, gid)
        os.fchmod(transaction_fd, 0o700)
        require_release_directory_stat(
            os.fstat(transaction_fd),
            uid,
            gid,
            0o700,
            "unpublished release transaction",
        )
        write_residue_marker(
            transaction_fd,
            INCOMING_MARKER,
            residue_marker_value("gateway-release", name),
            uid,
            gid,
        )
        os.mkdir(INCOMING_PAYLOAD, 0o700, dir_fd=transaction_fd)
        payload_fd = os.open(INCOMING_PAYLOAD, DIR_FLAGS, dir_fd=transaction_fd)
        os.fchown(payload_fd, uid, gid)
        os.fchmod(payload_fd, 0o700)
        require_release_directory_stat(
            os.fstat(payload_fd),
            uid,
            gid,
            0o700,
            "unpublished release payload",
        )
        os.fsync(transaction_fd)
        os.fsync(releases_fd)
        return name, transaction_fd, payload_fd
    except Exception:
        if payload_fd is not None:
            os.close(payload_fd)
        if transaction_fd is not None:
            try:
                cleanup_unpublished_release(
                    releases_fd,
                    name,
                    transaction_fd,
                    uid,
                    gid,
                )
            except (OSError, SecurityError):
                pass
            os.close(transaction_fd)
        else:
            try:
                os.rmdir(name, dir_fd=releases_fd)
            except OSError:
                pass
        raise


def verify_published_release(releases_fd, tag, expected_results, uid, gid):
    try:
        release_fd = os.open(tag, DIR_FLAGS, dir_fd=releases_fd)
    except OSError:
        fail("published release is missing, unsafe, or a symbolic link")
    try:
        require_release_directory_stat(
            os.fstat(release_fd),
            uid,
            gid,
            0o755,
            "published release directory",
        )
        if set(
            bounded_directory_names(
                release_fd,
                len(EXPECTED_MEMBERS),
                "published release",
            )
        ) != set(EXPECTED_MEMBERS):
            fail("published release does not contain the exact file whitelist")
        for name, (expected_mode, _) in EXPECTED_MEMBERS.items():
            file_fd = os.open(name, FILE_READ_FLAGS, dir_fd=release_fd)
            try:
                file_stat = os.fstat(file_fd)
                if (
                    not stat.S_ISREG(file_stat.st_mode)
                    or file_stat.st_uid != uid
                    or file_stat.st_gid != gid
                    or file_stat.st_nlink != 1
                    or stat.S_IMODE(file_stat.st_mode) != expected_mode
                    or file_stat.st_size != expected_results[name]["size"]
                    or not hmac.compare_digest(
                        hash_fd(file_fd),
                        expected_results[name]["sha256"],
                    )
                ):
                    fail("published release file identity/content contract failed")
            finally:
                os.close(file_fd)
    finally:
        os.close(release_fd)


def rename_noreplace(source_directory_fd, source, destination_directory_fd, destination):
    libc = ctypes.CDLL(None, use_errno=True)
    renameat2 = getattr(libc, "renameat2", None)
    if renameat2 is None:
        fail("renameat2(RENAME_NOREPLACE) is required for release publication")
    renameat2.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    renameat2.restype = ctypes.c_int
    result = renameat2(
        source_directory_fd,
        os.fsencode(source),
        destination_directory_fd,
        os.fsencode(destination),
        1,
    )
    if result == 0:
        return True
    error_number = ctypes.get_errno()
    if error_number == errno.EEXIST:
        return False
    fail("atomic no-replace release publication failed")


def stage_and_publish_release(
    archive_fd,
    validation_results,
    tag,
    expanded_limit,
    member_limit,
    releases_fd,
    uid,
    gid,
    inject_abort=False,
):
    incoming_name, transaction_fd, release_fd = create_unpublished_release(
        releases_fd,
        uid,
        gid,
    )
    incoming_exists = True
    try:
        installed_results = parse_archive_from_fd(
            archive_fd,
            expanded_limit,
            member_limit,
            release_fd=release_fd,
            release_uid=uid,
            release_gid=gid,
        )
        validate_release_manifest(installed_results, tag)
        for name in EXPECTED_MEMBERS:
            if not hmac.compare_digest(
                installed_results[name]["sha256"],
                validation_results[name]["sha256"],
            ):
                fail("same-FD archive content changed between validation and copy")
        os.fchmod(release_fd, 0o755)
        os.fsync(release_fd)
        require_release_directory_stat(
            os.fstat(release_fd),
            uid,
            gid,
            0o755,
            "completed unpublished release directory",
        )
        if inject_abort:
            fail("injected pre-publication selftest abort")
        if rename_noreplace(
            transaction_fd,
            INCOMING_PAYLOAD,
            releases_fd,
            tag,
        ):
            os.fsync(releases_fd)
        else:
            cleanup_release_payload(transaction_fd, release_fd, uid, gid)
        cleanup_unpublished_release(
            releases_fd,
            incoming_name,
            transaction_fd,
            uid,
            gid,
        )
        incoming_exists = False
        verify_published_release(
            releases_fd,
            tag,
            validation_results,
            uid,
            gid,
        )
    except Exception:
        if incoming_exists:
            cleanup_unpublished_release(
                releases_fd,
                incoming_name,
                transaction_fd,
                uid,
                gid,
            )
        raise
    finally:
        os.close(release_fd)
        os.close(transaction_fd)


def root_file_transaction_prefix(final_name):
    safe_component(final_name, "root file", 127)
    return f".{final_name}.incoming."


def validate_root_file_fd(
    file_fd,
    expected_mode,
    maximum,
    label,
    content_validator,
    expected_uid=0,
    expected_gid=0,
):
    item = os.fstat(file_fd)
    if (
        not stat.S_ISREG(item.st_mode)
        or item.st_uid != expected_uid
        or item.st_gid != expected_gid
        or item.st_nlink != 1
        or stat.S_IMODE(item.st_mode) != expected_mode
        or item.st_size <= 0
        or item.st_size > maximum
    ):
        fail(f"{label} owner/mode/nlink/size contract failed")
    content = read_all_fd(file_fd, maximum)
    content_validator(content)
    return content


def inspect_root_file_transaction(
    transaction_fd,
    transaction_name,
    final_name,
    expected_mode,
    maximum,
    expected_uid=0,
    expected_gid=0,
):
    require_residue_directory(
        transaction_fd,
        expected_uid,
        expected_gid,
        {0o700},
        "root-file transaction",
    )
    names = bounded_directory_names(transaction_fd, 2, "root-file transaction")
    if any(name not in {ROOT_FILE_MARKER, ROOT_FILE_PAYLOAD} for name in names):
        fail("root-file transaction contains an unknown entry")
    marker, marker_size = read_residue_marker(
        transaction_fd,
        ROOT_FILE_MARKER,
        f"root-file:{final_name}",
        transaction_name,
        expected_uid,
        expected_gid,
    )
    total = marker_size
    if ROOT_FILE_PAYLOAD in names:
        total += validate_residue_regular(
            transaction_fd,
            ROOT_FILE_PAYLOAD,
            expected_uid,
            expected_gid,
            {expected_mode, expected_mode & 0o700},
            maximum,
            True,
        )
    return marker, total


def cleanup_root_file_transaction(
    parent_fd,
    transaction_name,
    transaction_fd,
    final_name,
    expected_mode,
    maximum,
    expected_uid=0,
    expected_gid=0,
):
    inspect_root_file_transaction(
        transaction_fd,
        transaction_name,
        final_name,
        expected_mode,
        maximum,
        expected_uid,
        expected_gid,
    )
    for name in (ROOT_FILE_PAYLOAD, ROOT_FILE_MARKER):
        try:
            os.unlink(name, dir_fd=transaction_fd)
        except FileNotFoundError:
            pass
    os.fsync(transaction_fd)
    os.rmdir(transaction_name, dir_fd=parent_fd)
    os.fsync(parent_fd)


def sweep_root_file_transactions(
    parent_fd,
    final_name,
    expected_mode,
    maximum,
    expected_uid=0,
    expected_gid=0,
):
    prefix = root_file_transaction_prefix(final_name)
    pattern = re.compile(re.escape(prefix) + r"[0-9a-f]{24}")
    now = int(time.time())
    matched = 0
    active = 0
    total = 0
    with os.scandir(parent_fd) as entries:
        for entry in entries:
            if not entry.name.startswith(prefix):
                continue
            matched += 1
            if matched > MAX_RESIDUE_COUNT * 4:
                fail("root-file transaction scan bound exceeded")
            if not pattern.fullmatch(entry.name):
                fail("unknown entry uses a reserved root-file prefix")
            try:
                transaction_fd = os.open(entry.name, DIR_FLAGS, dir_fd=parent_fd)
            except OSError:
                fail("root-file transaction is a symlink or non-directory")
            try:
                transaction_stat = require_residue_directory(
                    transaction_fd,
                    expected_uid,
                    expected_gid,
                    {0o700},
                    "root-file transaction",
                )
                try:
                    marker, residue_bytes = inspect_root_file_transaction(
                        transaction_fd,
                        entry.name,
                        final_name,
                        expected_mode,
                        maximum,
                        expected_uid,
                        expected_gid,
                    )
                except FileNotFoundError:
                    names = bounded_directory_names(
                        transaction_fd,
                        1,
                        "unmarked root-file transaction",
                    )
                    if names:
                        fail("unmarked root-file transaction is not empty")
                    if now - int(transaction_stat.st_ctime) >= MAX_ACTIVE_RESIDUE_AGE:
                        os.rmdir(entry.name, dir_fd=parent_fd)
                        os.fsync(parent_fd)
                        continue
                    active += 1
                    continue
                age = now - marker["created"]
                if age < -MAX_FUTURE_CLOCK_SKEW:
                    fail("root-file transaction timestamp is in the future")
                if residue_owner_is_live(marker):
                    if age > MAX_ACTIVE_RESIDUE_AGE:
                        fail("live root-file transaction exceeded its maximum age")
                    active += 1
                    total += residue_bytes
                else:
                    cleanup_root_file_transaction(
                        parent_fd,
                        entry.name,
                        transaction_fd,
                        final_name,
                        expected_mode,
                        maximum,
                        expected_uid,
                        expected_gid,
                    )
            finally:
                os.close(transaction_fd)
    if active > MAX_RESIDUE_COUNT or total > MAX_RESIDUAL_BYTES:
        fail("active root-file transaction count/byte bound exceeded")


def revalidate_root_parent(parent_path, held_fd):
    check_fd = ensure_root_chain(parent_path)
    try:
        held = os.fstat(held_fd)
        check = os.fstat(check_fd)
        if (held.st_dev, held.st_ino) != (check.st_dev, check.st_ino):
            fail("trusted root-file parent identity changed concurrently")
    finally:
        os.close(check_fd)


def open_atomic_test_parent(parent_path):
    try:
        parent_fd = os.open(parent_path, DIR_FLAGS)
    except OSError:
        fail("atomic root-file selftest parent is missing or a symlink")
    item = os.fstat(parent_fd)
    if (
        not stat.S_ISDIR(item.st_mode)
        or item.st_uid != os.geteuid()
        or item.st_gid != os.getegid()
        or item.st_nlink < 2
        or stat.S_IMODE(item.st_mode) != 0o700
    ):
        os.close(parent_fd)
        fail("atomic root-file selftest parent must be caller-owned mode 0700")
    return parent_fd


def revalidate_atomic_test_parent(parent_path, held_fd):
    check_fd = open_atomic_test_parent(parent_path)
    try:
        held = os.fstat(held_fd)
        check = os.fstat(check_fd)
        if (held.st_dev, held.st_ino) != (check.st_dev, check.st_ino):
            fail("atomic root-file selftest parent identity changed concurrently")
    finally:
        os.close(check_fd)


def atomic_root_file_hook(path, requested_phase, current_phase):
    if requested_phase not in {"none", "payload"}:
        fail("atomic root-file selftest hook phase is invalid")
    if path == "-" or requested_phase != current_phase:
        return
    hook_fd = open_atomic_test_parent(path)
    try:
        opened_fd = os.open("opened", FILE_CREATE_FLAGS, 0o600, dir_fd=hook_fd)
        os.close(opened_fd)
        os.fsync(hook_fd)
        deadline = time.monotonic() + 10
        while True:
            try:
                item = os.stat("continue", dir_fd=hook_fd, follow_symlinks=False)
                if (
                    not stat.S_ISREG(item.st_mode)
                    or item.st_uid != os.geteuid()
                    or item.st_gid != os.getegid()
                    or item.st_nlink != 1
                    or stat.S_IMODE(item.st_mode) & 0o022
                ):
                    fail("atomic root-file selftest continue marker is unsafe")
                break
            except FileNotFoundError:
                if time.monotonic() >= deadline:
                    fail("atomic root-file selftest hook timed out")
                time.sleep(0.01)
    finally:
        os.close(hook_fd)


def ensure_atomic_root_file(
    path,
    content,
    expected_mode,
    maximum,
    label,
    content_validator,
    test_parent_path=None,
    hook_path="-",
    hook_phase="none",
):
    parts = path_components(path)
    parent_path = "/" + "/".join(parts[:-1])
    final_name = parts[-1]
    if test_parent_path is None:
        expected_uid = 0
        expected_gid = 0
        parent_fd = ensure_root_chain(parent_path)
    else:
        if os.geteuid() == 0 or parent_path != test_parent_path:
            fail("atomic root-file selftest trust boundary is invalid")
        expected_uid = os.geteuid()
        expected_gid = os.getegid()
        parent_fd = open_atomic_test_parent(parent_path)
    transaction_fd = None
    transaction_name = None
    try:
        sweep_root_file_transactions(
            parent_fd,
            final_name,
            expected_mode,
            maximum,
            expected_uid,
            expected_gid,
        )
        try:
            existing_fd = os.open(final_name, FILE_READ_FLAGS, dir_fd=parent_fd)
        except FileNotFoundError:
            existing_fd = None
        except OSError:
            fail(f"{label} is a symbolic link or unsafe file")
        if existing_fd is not None:
            try:
                return validate_root_file_fd(
                    existing_fd,
                    expected_mode,
                    maximum,
                    label,
                    content_validator,
                    expected_uid,
                    expected_gid,
                )
            finally:
                os.close(existing_fd)

        if callable(content):
            content = content()
        if not isinstance(content, bytes) or not content or len(content) > maximum:
            fail(f"new {label} content is outside its fixed size bound")
        content_validator(content)
        prefix = root_file_transaction_prefix(final_name)
        for _ in range(32):
            candidate = prefix + secrets.token_hex(12)
            try:
                os.mkdir(candidate, 0o700, dir_fd=parent_fd)
                transaction_name = candidate
                break
            except FileExistsError:
                continue
        if transaction_name is None:
            fail(f"could not allocate a fresh {label} transaction")
        transaction_fd = os.open(transaction_name, DIR_FLAGS, dir_fd=parent_fd)
        os.fchown(transaction_fd, expected_uid, expected_gid)
        os.fchmod(transaction_fd, 0o700)
        require_residue_directory(
            transaction_fd,
            expected_uid,
            expected_gid,
            {0o700},
            "root-file transaction",
        )
        write_residue_marker(
            transaction_fd,
            ROOT_FILE_MARKER,
            residue_marker_value(f"root-file:{final_name}", transaction_name),
            expected_uid,
            expected_gid,
        )
        payload_fd = os.open(
            ROOT_FILE_PAYLOAD,
            FILE_CREATE_RW_FLAGS,
            expected_mode,
            dir_fd=transaction_fd,
        )
        try:
            os.fchown(payload_fd, expected_uid, expected_gid)
            os.fchmod(payload_fd, expected_mode)
            write_all(payload_fd, content)
            os.fsync(payload_fd)
            validate_root_file_fd(
                payload_fd,
                expected_mode,
                maximum,
                f"new {label}",
                content_validator,
                expected_uid,
                expected_gid,
            )
        finally:
            os.close(payload_fd)
        os.fsync(transaction_fd)
        os.fsync(parent_fd)
        atomic_root_file_hook(hook_path, hook_phase, "payload")
        if test_parent_path is None:
            revalidate_root_parent(parent_path, parent_fd)
        else:
            revalidate_atomic_test_parent(parent_path, parent_fd)
        published = rename_noreplace(
            transaction_fd,
            ROOT_FILE_PAYLOAD,
            parent_fd,
            final_name,
        )
        os.fsync(parent_fd)
        if not published:
            existing_fd = os.open(final_name, FILE_READ_FLAGS, dir_fd=parent_fd)
            try:
                validate_root_file_fd(
                    existing_fd,
                    expected_mode,
                    maximum,
                    label,
                    content_validator,
                    expected_uid,
                    expected_gid,
                )
            finally:
                os.close(existing_fd)
        cleanup_root_file_transaction(
            parent_fd,
            transaction_name,
            transaction_fd,
            final_name,
            expected_mode,
            maximum,
            expected_uid,
            expected_gid,
        )
        transaction_name = None
        final_fd = os.open(final_name, FILE_READ_FLAGS, dir_fd=parent_fd)
        try:
            return validate_root_file_fd(
                final_fd,
                expected_mode,
                maximum,
                label,
                content_validator,
                expected_uid,
                expected_gid,
            )
        finally:
            os.close(final_fd)
    except Exception:
        if transaction_fd is not None and transaction_name is not None:
            try:
                cleanup_root_file_transaction(
                    parent_fd,
                    transaction_name,
                    transaction_fd,
                    final_name,
                    expected_mode,
                    maximum,
                    expected_uid,
                    expected_gid,
                )
            except (OSError, SecurityError):
                pass
        raise
    finally:
        if transaction_fd is not None:
            os.close(transaction_fd)
        os.close(parent_fd)


def ensure_exact_root_file(path, expected_bytes, expected_mode):
    def validate_unit(actual):
        if actual != expected_bytes:
            fail("systemd unit differs from the trusted installer contract")

    ensure_atomic_root_file(
        path,
        expected_bytes,
        expected_mode,
        len(expected_bytes) + 1,
        "systemd unit",
        validate_unit,
    )


def recovery_key_is_weak(value):
    lowered = value.lower()
    compact = re.sub(r"[^0-9a-f]", "", lowered)
    if not re.fullmatch(r"[0-9a-f]{64}", compact):
        return True
    if len(set(compact)) < 8:
        return True
    if len(compact) != 64:
        return True
    if has_short_repetition(compact, max_period=6, min_total=16):
        return True
    if has_obvious_weak_pattern(compact) or has_obvious_weak_pattern(lowered):
        return True
    if has_arithmetic_run(compact, "0123456789abcdef", 8):
        return True
    for width in (1, 2, 4, 8, 16, 32):
        if compact == compact[:width] * (64 // width):
            return True
    return False


def has_obvious_weak_pattern(value):
    lowered = value.lower()
    compact = re.sub(r"[^a-z0-9]", "", lowered)
    if re.search(r"(0123456789abcdef|1234567890abcdef|fedcba0987654321)", compact):
        return True
    if re.search(
        r"(qwerty|asdf|zxcv|qwert|admin|password|secret|changeme|change-me|"
        r"replace|replace-with|example|demo|test|public|placeholder|token|"
        r"123456|654321|abcdefgh|hgfedcba)",
        lowered,
    ):
        return True
    if len(set(lowered)) <= 3:
        return True
    if re.search(r"(..)\1{3,}", lowered):
        return True
    if re.search(r"(.)\1{31,}", lowered):
        return True
    return False


def has_arithmetic_run(value, alphabet, window):
    if len(value) < window:
        return False
    indexes = {character: i for i, character in enumerate(alphabet)}
    for direction in (1, -1):
        for start in range(len(value)):
            start_index = indexes.get(value[start])
            if start_index is None:
                continue
            previous = start_index
            run = 1
            for next_position in range(start + 1, len(value)):
                current = indexes.get(value[next_position])
                if current is None or current - previous != direction:
                    break
                run += 1
                previous = current
                if run >= window:
                    return True
    return False


def has_short_repetition(value, max_period=12, min_total=24):
    if len(value) < min_total:
        return False
    for period in range(2, min(max_period + 1, len(value) // 4 + 1)):
        unit = value[:period]
        if not unit:
            continue
        repeated = unit * (len(value) // period)
        if repeated[:len(value)] == value:
            return True
    return False

def parse_env_assignments(env_bytes, trusted_template=False):
    if b"\0" in env_bytes:
        fail("gateway environment file contains NUL")
    if trusted_template:
        normalized = env_bytes.replace(b"\r\n", b"\n")
        if b"\r" in normalized:
            fail("trusted environment template contains a non-canonical CR")
        env_bytes = normalized
    elif b"\r" in env_bytes:
        fail("gateway environment file contains a non-canonical CR")
    try:
        text = env_bytes.decode("utf-8")
    except UnicodeDecodeError:
        fail("gateway environment file must be UTF-8")
    if not text.endswith("\n"):
        fail("gateway environment file must end with a complete LF line")
    assignments = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"([A-Z][A-Z0-9_]*)=([^\x00-\x20\x7f]+)", line)
        if match is None:
            fail("gateway environment lines must use exact unquoted KEY=VALUE syntax")
        key, value = match.groups()
        if key in assignments:
            fail(f"{key} must appear exactly once as KEY=VALUE")
        if any(character in value for character in ("\\", "'", '"')):
            fail("gateway environment values must not require systemd unescaping")
        assignments[key] = value
    return assignments


def exact_env_value(env_bytes, key):
    assignments = parse_env_assignments(env_bytes)
    if key not in assignments:
        fail(f"{key} must appear exactly once as KEY=VALUE")
    return assignments[key]


def validate_socket_address(value, label):
    if value.startswith("["):
        match = re.fullmatch(r"\[[0-9A-Fa-f:]+\]:([0-9]{1,5})", value)
    else:
        match = re.fullmatch(r"[A-Za-z0-9.-]+:([0-9]{1,5})", value)
    if match is None or not 1 <= int(match.group(1), 10) <= 65535:
        fail(f"{label} is not a bounded host:port address")


def validate_service_url(value, schemes, label):
    try:
        parsed = urllib.parse.urlsplit(value)
        port = parsed.port
    except ValueError:
        fail(f"{label} is malformed")
    if (
        parsed.scheme not in schemes
        or not parsed.hostname
        or parsed.fragment
        or port is not None and not 1 <= port <= 65535
    ):
        fail(f"{label} is malformed or uses an unsupported scheme")


def validate_raw_credential(value, label):
    lowered = value.lower()
    if (
        len(value) < 32
        or len(value) > 512
        or len(set(value)) < 8
        or has_obvious_weak_pattern(value)
        or has_short_repetition(value, min_total=24)
        or has_arithmetic_run(lowered, "0123456789", 8)
        or has_arithmetic_run(lowered, "abcdefghijklmnopqrstuvwxyz", 8)
        or has_arithmetic_run(lowered, "abcdef", 8)
        or any(ord(character) < 0x21 or ord(character) > 0x7E for character in value)
    ):
        fail(f"{label} credential is absent, weak, or a placeholder")
    return value


def decode_url_userinfo_credential(value, label):
    if re.search(r"%(?![0-9A-Fa-f]{2})", value):
        fail(f"{label} credential encoding is invalid")
    try:
        decoded = urllib.parse.unquote(value, errors="strict")
    except (UnicodeDecodeError, ValueError):
        fail(f"{label} credential encoding is invalid")
    return validate_raw_credential(decoded, label)


def validate_authenticated_service_url(value, schemes, label, database):
    try:
        parsed = urllib.parse.urlsplit(value)
        port = parsed.port
    except ValueError:
        fail(f"{label} is malformed")
    if (
        parsed.scheme not in schemes
        or not parsed.hostname
        or parsed.password is None
        or parsed.password == ""
        or parsed.query
        or parsed.fragment
        or port is not None and not 1 <= port <= 65535
    ):
        fail(f"{label} must use an authenticated fixed-form service URL")
    if parsed.username is not None:
        if re.search(r"%(?![0-9A-Fa-f]{2})", parsed.username):
            fail(f"{label} URL userinfo encoding is invalid")
        try:
            urllib.parse.unquote(parsed.username, errors="strict")
        except (UnicodeDecodeError, ValueError):
            fail(f"{label} URL userinfo encoding is invalid")
    if database:
        if not parsed.username or not re.fullmatch(r"/[A-Za-z0-9_.-]+", parsed.path):
            fail(f"{label} must name a database user and database")
    elif parsed.path not in {"", "/"} and not re.fullmatch(r"/[0-9]{1,2}", parsed.path):
        fail(f"{label} Redis database selector is malformed")
    return decode_url_userinfo_credential(parsed.password, label)


def validate_env_complete(env_bytes, template_bytes=None):
    assignments = parse_env_assignments(env_bytes)
    keys = set(assignments)
    if not REQUIRED_ENV_KEYS <= keys:
        fail("gateway environment is truncated or missing required fields")
    if not keys <= REQUIRED_ENV_KEYS | OPTIONAL_ENV_KEYS:
        fail("gateway environment contains a field outside the fixed contract")
    if template_bytes is not None:
        template = parse_env_assignments(template_bytes, trusted_template=True)
        if set(template) != REQUIRED_ENV_KEYS:
            fail("trusted environment template field set differs from the installer contract")

    fixed = {
        "MIR2_ACCOUNT_STORE_BACKEND": "postgres",
        "MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES": "1",
        "MIR2_ACCOUNT_STORE_PATH": "/var/lib/mir2/gateway-data/accounts.json",
        "MIR2_SAVE_RECOVERY_DIR": RECOVERY_DIR,
        "MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY": "1",
        "MIR2_GATEWAY_REQUIRE_REDIS_CACHE": "1",
        "MIR2_IDENTITY_POLICY": "commercial",
        "MIR2_TRUST_CF_CONNECTING_IP": "0",
    }
    for key, expected in fixed.items():
        if assignments[key] != expected:
            fail(f"{key} differs from the fail-closed production contract")
    if assignments["MIR2_ENV"] not in {"staging", "prod", "production"}:
        fail("MIR2_ENV must remain staging or production-like")
    validate_socket_address(assignments["MIR2_GATEWAY_WEB_ADDR"], "web address")
    validate_socket_address(assignments["MIR2_GATEWAY_TCP_ADDR"], "TCP address")
    validate_service_url(
        assignments["MIR2_ACCOUNT_STORE_DATABASE_URL"],
        {"postgres", "postgresql"},
        "Postgres URL",
    )
    validate_service_url(
        assignments["MIR2_GATEWAY_REDIS_CACHE_URL"],
        {"redis", "rediss"},
        "Redis URL",
    )
    for key in NUMERIC_ENV_KEYS:
        value = assignments[key]
        if not re.fullmatch(r"[0-9]{1,10}", value):
            fail(f"{key} must be a bounded decimal value")
        number = int(value, 10)
        if number < 1 or number > 2**31 - 1:
            fail(f"{key} is outside its fixed numeric bound")

    secrets_seen = set()
    for key in SECRET_ENV_KEYS:
        value = assignments[key]
        if recovery_key_is_weak(value):
            fail(f"{key} must be a strong non-placeholder 64-hex value")
        if value in secrets_seen:
            fail("gateway environment secrets must be independent")
        secrets_seen.add(value)
    if assignments["MIR2_SAVE_RECOVERY_MAC_KEY"] == RECOVERY_PLACEHOLDER:
        fail("save-recovery MAC key is still the placeholder")

    origins = assignments["MIR2_ALLOWED_WEB_ORIGINS"].split(",")
    if not origins or len(origins) > 32:
        fail("allowed Web origin count is outside its bound")
    for origin in origins:
        try:
            parsed = urllib.parse.urlsplit(origin)
            port = parsed.port
        except ValueError:
            fail("allowed Web origin is malformed")
        if (
            parsed.scheme != "https"
            or not parsed.hostname
            or parsed.username is not None
            or parsed.password is not None
            or parsed.path not in {"", "/"}
            or parsed.query
            or parsed.fragment
            or port is not None and not 1 <= port <= 65535
        ):
            fail("allowed Web origin is not an exact HTTPS origin")
    if "CRYSTAL_CLIENT_ROOT" in assignments:
        crystal_parts = path_components(assignments["CRYSTAL_CLIENT_ROOT"])
        if crystal_parts[:3] != ["var", "lib", "mir2"] or len(crystal_parts) < 4:
            fail("CRYSTAL_CLIENT_ROOT is outside the fixed data namespace")
    if "ADMIN_API_BASE_URL" in assignments:
        validate_service_url(
            assignments["ADMIN_API_BASE_URL"],
            {"http", "https"},
            "admin API URL",
        )
    if "MIR2_GAMEPLAY_EVENT_REDPANDA_URL" in assignments:
        validate_service_url(
            assignments["MIR2_GAMEPLAY_EVENT_REDPANDA_URL"],
            {"http", "https"},
            "gameplay event URL",
        )
    if "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" in assignments:
        operator_token = validate_raw_credential(
            assignments["MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN"],
            "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN",
        )
        if operator_token in secrets_seen:
            fail("Gateway admin operator token reuses another Gateway secret")
        secrets_seen.add(operator_token)
    return assignments


def validate_activation_credentials(env_bytes):
    assignments = validate_env_complete(env_bytes)
    database_secret = validate_authenticated_service_url(
        assignments["MIR2_ACCOUNT_STORE_DATABASE_URL"],
        {"postgres", "postgresql"},
        "Postgres",
        True,
    )
    redis_secret = validate_authenticated_service_url(
        assignments["MIR2_GATEWAY_REDIS_CACHE_URL"],
        {"redis", "rediss"},
        "Redis",
        False,
    )
    if hmac.compare_digest(database_secret, redis_secret):
        fail("Postgres and Redis credentials must be independent")
    for key in SECRET_ENV_KEYS:
        secret = assignments[key]
        if (
            hmac.compare_digest(database_secret, secret)
            or hmac.compare_digest(redis_secret, secret)
        ):
            fail("database/cache credentials must not reuse a Gateway secret")
    if "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN" in assignments:
        operator = assignments["MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN"]
        if (
            hmac.compare_digest(database_secret, operator)
            or hmac.compare_digest(redis_secret, operator)
        ):
            fail("database/cache credentials must not reuse the operator token")


def render_initial_env(template_bytes):
    try:
        template_text = template_bytes.decode("utf-8")
    except UnicodeDecodeError:
        fail("trusted environment template must be UTF-8")
    placeholders = {
        "MIR2_PASSKEY_AUTH_SECRET=replace-with-random-32-byte-secret":
            "MIR2_PASSKEY_AUTH_SECRET",
        "MIR2_IDENTITY_SESSION_SECRET=replace-with-independent-random-32-byte-secret":
            "MIR2_IDENTITY_SESSION_SECRET",
        "MIR2_IDENTITY_RECOVERY_PEPPER=replace-with-independent-random-32-byte-pepper":
            "MIR2_IDENTITY_RECOVERY_PEPPER",
        "MIR2_SAVE_RECOVERY_MAC_KEY=replace-with-stable-independent-64-hex-secret":
            "MIR2_SAVE_RECOVERY_MAC_KEY",
    }
    lines = template_text.splitlines()
    for placeholder in placeholders:
        if lines.count(placeholder) != 1:
            fail("trusted env template secret placeholders must appear exactly once")
    replacements = {}
    generated = set()
    for placeholder, key in placeholders.items():
        for _ in range(8):
            value = secrets.token_hex(32)
            if value not in generated and (
                key != "MIR2_SAVE_RECOVERY_MAC_KEY"
                or not recovery_key_is_weak(value)
            ):
                generated.add(value)
                replacements[placeholder] = f"{key}={value}"
                break
        else:
            fail("independent CSPRNG secret generation failed")
    rendered = "\n".join(replacements.get(line, line) for line in lines) + "\n"
    rendered_bytes = rendered.encode("utf-8")
    validate_env_complete(rendered_bytes, template_bytes)
    return rendered_bytes


def validate_env_recovery(env_bytes):
    validate_env_complete(env_bytes)


def validate_activation_env_test(env_path):
    env_fd = open_test_regular(env_path, 1_048_576)
    try:
        validate_activation_credentials(read_all_fd(env_fd, 1_048_576))
    finally:
        os.close(env_fd)


def ensure_gateway_env(template_bytes):
    def validate_env(actual):
        validate_env_complete(actual, template_bytes)

    return ensure_atomic_root_file(
        "/etc/mir2/gateway.env",
        lambda: render_initial_env(template_bytes),
        0o600,
        1_048_576,
        "gateway env",
        validate_env,
    )


def ensure_recovery_directory(uid, gid):
    data_fd = ensure_root_chain(DATA_ROOT)
    try:
        current_fd = data_fd
        namespace_fds = []
        for component in ("save-recovery", "v1"):
            created = False
            try:
                next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
            except FileNotFoundError:
                try:
                    os.mkdir(component, 0o711, dir_fd=current_fd)
                    created = True
                except FileExistsError:
                    pass
                try:
                    next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
                except OSError:
                    fail("recovery namespace creation raced with an unsafe component")
            except OSError:
                fail("recovery namespace contains a symbolic link")
            namespace_stat = os.fstat(next_fd)
            if created:
                os.fchown(next_fd, 0, 0)
                os.fchmod(next_fd, 0o711)
                namespace_stat = os.fstat(next_fd)
            if (
                namespace_stat.st_uid != 0
                or namespace_stat.st_gid != 0
                or stat.S_IMODE(namespace_stat.st_mode) != 0o711
            ):
                fail("existing recovery namespace requires manual migration")
            namespace_fds.append(next_fd)
            current_fd = next_fd

        created = False
        try:
            leaf_fd = os.open("gateway", DIR_FLAGS, dir_fd=current_fd)
        except FileNotFoundError:
            try:
                os.mkdir("gateway", 0o700, dir_fd=current_fd)
                created = True
            except FileExistsError:
                pass
            try:
                leaf_fd = os.open("gateway", DIR_FLAGS, dir_fd=current_fd)
            except OSError:
                fail("recovery leaf creation raced with an unsafe component")
        except OSError:
            fail("recovery leaf is a symbolic link or non-directory")
        leaf_stat = os.fstat(leaf_fd)
        if created:
            os.fchown(leaf_fd, uid, gid)
        elif leaf_stat.st_uid != uid or leaf_stat.st_gid != gid:
            fail("existing recovery leaf ownership requires manual migration")
        os.fchmod(leaf_fd, 0o700)
        leaf_stat = os.fstat(leaf_fd)
        if (
            leaf_stat.st_uid != uid
            or leaf_stat.st_gid != gid
            or stat.S_IMODE(leaf_stat.st_mode) != 0o700
        ):
            fail("recovery leaf must be service-owned with mode 0700")
        os.fsync(leaf_fd)
        os.close(leaf_fd)
        for opened_fd in reversed(namespace_fds):
            os.fsync(opened_fd)
            os.close(opened_fd)
    finally:
        os.close(data_fd)


def update_current_symlink(tag):
    install_fd = ensure_root_chain(INSTALL_ROOT)
    temporary = f".current.{os.getpid()}.{secrets.token_hex(4)}"
    target = f"releases/{tag}"
    try:
        try:
            existing = os.stat("current", dir_fd=install_fd, follow_symlinks=False)
        except FileNotFoundError:
            existing = None
        if existing is not None and (
            not stat.S_ISLNK(existing.st_mode)
            or existing.st_uid != 0
            or existing.st_gid != 0
            or existing.st_nlink != 1
        ):
            fail("existing current pointer is not a root-owned symbolic link")
        os.symlink(target, temporary, dir_fd=install_fd)
        os.chown(
            temporary,
            0,
            0,
            dir_fd=install_fd,
            follow_symlinks=False,
        )
        os.replace(
            temporary,
            "current",
            src_dir_fd=install_fd,
            dst_dir_fd=install_fd,
        )
        final_stat = os.stat("current", dir_fd=install_fd, follow_symlinks=False)
        final_target = os.readlink("current", dir_fd=install_fd)
        if (
            not stat.S_ISLNK(final_stat.st_mode)
            or final_stat.st_uid != 0
            or final_stat.st_gid != 0
            or final_stat.st_nlink != 1
            or final_target != target
        ):
            fail("current release pointer verification failed")
        os.fsync(install_fd)
    finally:
        try:
            os.unlink(temporary, dir_fd=install_fd)
        except FileNotFoundError:
            pass
        os.close(install_fd)


def install_release(
    archive_path,
    expected_sha,
    tag,
    service_uid,
    service_gid,
    archive_limit,
    expanded_limit,
    member_limit,
    activate,
):
    require_linux_dirfd()
    tag = safe_component(tag, "release tag")
    if service_uid <= 0 or service_gid <= 0:
        fail("mir2 service uid/gid must be non-root")

    template_fd = open_trusted_regular(TRUSTED_ENV_TEMPLATE, 0o644, 1_048_576)
    try:
        template_bytes = read_all_fd(template_fd, 1_048_576)
    finally:
        os.close(template_fd)

    archive_fd = open_archive(
        archive_path,
        expected_sha,
        archive_limit,
        require_root_owner=True,
    )
    try:
        validation_results = parse_archive_from_fd(
            archive_fd,
            expanded_limit,
            member_limit,
            release_fd=None,
        )
        validate_release_manifest(validation_results, tag)

        install_root_fd = ensure_root_chain(INSTALL_ROOT)
        os.close(install_root_fd)
        ensure_service_leaf(
            "/var/lib/mir2", "gateway-data", service_uid, service_gid, 0o700
        )
        ensure_service_leaf("/var/log", "mir2", service_uid, service_gid, 0o750)
        ensure_exact_root_file(SERVICE_PATH, SERVICE_UNIT, 0o644)
        gateway_env = ensure_gateway_env(template_bytes)
        if activate:
            validate_activation_credentials(gateway_env)
        ensure_recovery_directory(service_uid, service_gid)

        releases_fd = ensure_root_chain(RELEASES_ROOT)
        try:
            stage_and_publish_release(
                archive_fd,
                validation_results,
                tag,
                expanded_limit,
                member_limit,
                releases_fd,
                0,
                0,
            )
        finally:
            os.close(releases_fd)
        update_current_symlink(tag)
    finally:
        os.close(archive_fd)


def install_layout_test(
    archive_path,
    expected_sha,
    tag,
    releases_path,
    archive_limit,
    expanded_limit,
    member_limit,
    action,
):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("release publication selftest must not run as root")
    tag = safe_component(tag, "release tag")
    if action not in {"abort", "publish"}:
        fail("release publication selftest action is invalid")
    releases_fd = os.open(releases_path, DIR_FLAGS)
    try:
        require_release_directory_stat(
            os.fstat(releases_fd),
            os.geteuid(),
            os.getegid(),
            0o700,
            "release publication selftest root",
        )
        archive_fd = open_archive(
            archive_path,
            expected_sha,
            int(archive_limit),
            require_root_owner=False,
        )
        try:
            validation_results = parse_archive_from_fd(
                archive_fd,
                int(expanded_limit),
                int(member_limit),
                release_fd=None,
            )
            validate_release_manifest(validation_results, tag)
            stage_and_publish_release(
                archive_fd,
                validation_results,
                tag,
                int(expanded_limit),
                int(member_limit),
                releases_fd,
                os.geteuid(),
                os.getegid(),
                inject_abort=action == "abort",
            )
        finally:
            os.close(archive_fd)
    finally:
        os.close(releases_fd)


def install_source_swap_test(
    archive_path,
    replacement_path,
    expected_sha,
    tag,
    releases_path,
    archive_limit,
    expanded_limit,
    member_limit,
):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("release source-swap selftest must not run as root")
    tag = safe_component(tag, "release tag")
    releases_fd = os.open(releases_path, DIR_FLAGS)
    try:
        require_release_directory_stat(
            os.fstat(releases_fd),
            os.geteuid(),
            os.getegid(),
            0o700,
            "release source-swap root",
        )
        archive_fd = open_archive(
            archive_path,
            expected_sha,
            int(archive_limit),
            require_root_owner=False,
        )
        try:
            replacement_fd = open_test_regular(
                replacement_path,
                int(archive_limit),
            )
            os.close(replacement_fd)
            os.replace(replacement_path, archive_path)
            validation_results = parse_archive_from_fd(
                archive_fd,
                int(expanded_limit),
                int(member_limit),
                release_fd=None,
            )
            validate_release_manifest(validation_results, tag)
            stage_and_publish_release(
                archive_fd,
                validation_results,
                tag,
                int(expanded_limit),
                int(member_limit),
                releases_fd,
                os.geteuid(),
                os.getegid(),
            )
        finally:
            os.close(archive_fd)
    finally:
        os.close(releases_fd)


def same_fd_swap_test(root_path, source_name, replacement_name, output_name, expected_sha):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("same-FD selftest must not run as root")
    for name in (source_name, replacement_name, output_name):
        safe_component(name, "selftest filename", 63)
    root_fd = os.open(root_path, DIR_FLAGS)
    try:
        root_stat = os.fstat(root_fd)
        if not stat.S_ISDIR(root_stat.st_mode) or root_stat.st_uid != os.geteuid():
            fail("selftest root identity is unsafe")
        source_fd = os.open(source_name, FILE_READ_FLAGS, dir_fd=root_fd)
        try:
            source_stat = os.fstat(source_fd)
            if (
                not stat.S_ISREG(source_stat.st_mode)
                or source_stat.st_uid != os.geteuid()
                or source_stat.st_nlink != 1
            ):
                fail("same-FD source identity contract failed")
            digest = hashlib.sha256()
            while True:
                chunk = os.read(source_fd, 65_536)
                if not chunk:
                    break
                digest.update(chunk)
            if not hmac.compare_digest(
                digest.hexdigest(), validate_sha256(expected_sha)
            ):
                fail("same-FD source hash mismatch")
            os.replace(
                replacement_name,
                source_name,
                src_dir_fd=root_fd,
                dst_dir_fd=root_fd,
            )
            os.lseek(source_fd, 0, os.SEEK_SET)
            output_fd = os.open(
                output_name,
                FILE_CREATE_FLAGS,
                0o600,
                dir_fd=root_fd,
            )
            try:
                while True:
                    chunk = os.read(source_fd, 65_536)
                    if not chunk:
                        break
                    write_all(output_fd, chunk)
                os.fsync(output_fd)
            finally:
                os.close(output_fd)
        finally:
            os.close(source_fd)
        os.fsync(root_fd)
    finally:
        os.close(root_fd)


def regular_nlink_test(path):
    file_fd = os.open(path, FILE_READ_FLAGS)
    try:
        file_stat = os.fstat(file_fd)
        if not stat.S_ISREG(file_stat.st_mode) or file_stat.st_nlink != 1:
            fail("regular file must have nlink=1")
    finally:
        os.close(file_fd)


def write_service_test(path):
    output_fd = os.open(path, FILE_CREATE_FLAGS, 0o644)
    try:
        write_all(output_fd, SERVICE_UNIT)
        os.fsync(output_fd)
    finally:
        os.close(output_fd)


def open_test_regular(path, maximum):
    file_fd = os.open(path, FILE_READ_FLAGS)
    file_stat = os.fstat(file_fd)
    if (
        not stat.S_ISREG(file_stat.st_mode)
        or file_stat.st_nlink != 1
        or file_stat.st_size < 0
        or file_stat.st_size > maximum
    ):
        os.close(file_fd)
        fail("selftest input must be an independent bounded regular file")
    return file_fd


def render_env_test(template_path, output_path):
    template_fd = open_test_regular(template_path, 1_048_576)
    try:
        template_bytes = read_all_fd(template_fd, 1_048_576)
    finally:
        os.close(template_fd)
    try:
        output_fd = os.open(output_path, FILE_READ_FLAGS)
    except FileNotFoundError:
        rendered = render_initial_env(template_bytes)
        output_fd = os.open(output_path, FILE_CREATE_RW_FLAGS, 0o600)
        created = True
    else:
        created = False
    try:
        if created:
            os.fchmod(output_fd, 0o600)
            write_all(output_fd, rendered)
            os.fsync(output_fd)
        validate_env_complete(read_all_fd(output_fd, 1_048_576), template_bytes)
    finally:
        os.close(output_fd)


def atomic_root_file_test(parent_path, kind, template_path, hook_path, hook_phase):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("atomic root-file selftest must not run as root")
    if kind == "service":
        final_name = "mir2-gateway.service"
        expected_mode = 0o644
        maximum = len(SERVICE_UNIT) + 1
        content = SERVICE_UNIT

        def validator(actual):
            if actual != SERVICE_UNIT:
                fail("systemd unit differs from the trusted installer contract")

    elif kind == "env":
        final_name = "gateway.env"
        expected_mode = 0o600
        maximum = 1_048_576
        template_fd = open_test_regular(template_path, maximum)
        try:
            template_bytes = read_all_fd(template_fd, maximum)
        finally:
            os.close(template_fd)
        content = lambda: render_initial_env(template_bytes)

        def validator(actual):
            validate_env_complete(actual, template_bytes)

    else:
        fail("atomic root-file selftest kind is invalid")
    ensure_atomic_root_file(
        f"{parent_path}/{final_name}",
        content,
        expected_mode,
        maximum,
        f"selftest {kind}",
        validator,
        parent_path,
        hook_path,
        hook_phase,
    )


def validate_env_test(env_path):
    env_fd = open_test_regular(env_path, 1_048_576)
    try:
        validate_env_recovery(read_all_fd(env_fd, 1_048_576))
    finally:
        os.close(env_fd)


def validate_identity_fixture(arguments):
    if len(arguments) != 16:
        fail("identity fixture invocation is invalid")
    file_values = []
    for path in arguments[:8]:
        file_fd = open_test_regular(path, 4_194_304)
        try:
            file_values.append(read_all_fd(file_fd, 4_194_304))
        finally:
            os.close(file_fd)
    uid, gid = validate_identity_sources(
        *file_values[:6],
        file_values[6],
        file_values[7],
        *arguments[8:],
    )
    return str(uid), str(gid)


def open_residue_test_root(path):
    root_fd = os.open(path, DIR_FLAGS)
    item = os.fstat(root_fd)
    if (
        not stat.S_ISDIR(item.st_mode)
        or item.st_uid != os.geteuid()
        or item.st_gid != os.getegid()
        or item.st_nlink < 2
        or stat.S_IMODE(item.st_mode) != 0o700
    ):
        os.close(root_fd)
        fail("residue selftest root must be caller-owned mode 0700")
    return root_fd


def write_residue_test_ready(hook_path):
    hook_fd = os.open(hook_path, DIR_FLAGS)
    try:
        item = os.fstat(hook_fd)
        if (
            item.st_uid != os.geteuid()
            or item.st_gid != os.getegid()
            or stat.S_IMODE(item.st_mode) != 0o700
        ):
            fail("residue selftest hook is unsafe")
        ready_fd = os.open("ready", FILE_CREATE_FLAGS, 0o600, dir_fd=hook_fd)
        os.close(ready_fd)
        os.fsync(hook_fd)
    finally:
        os.close(hook_fd)


def create_residue_test(root_fd, kind):
    uid = os.geteuid()
    gid = os.getegid()
    if kind == "download":
        name = DOWNLOAD_PREFIX + secrets.token_hex(12)
        os.mkdir(name, 0o700, dir_fd=root_fd)
        directory_fd = os.open(name, DIR_FLAGS, dir_fd=root_fd)
        os.fchown(directory_fd, uid, gid)
        os.fchmod(directory_fd, 0o700)
        write_residue_marker(
            directory_fd,
            DOWNLOAD_MARKER,
            residue_marker_value("gateway-download", name),
            uid,
            gid,
        )
        payload_fd = os.open(
            DOWNLOAD_NAME,
            FILE_CREATE_RW_FLAGS,
            0o600,
            dir_fd=directory_fd,
        )
        try:
            os.fchown(payload_fd, uid, gid)
            os.fchmod(payload_fd, 0o600)
            write_all(payload_fd, b"residue-test\n")
            os.fsync(payload_fd)
        finally:
            os.close(payload_fd)
    elif kind == "incoming":
        name = INCOMING_PREFIX + secrets.token_hex(12)
        os.mkdir(name, 0o700, dir_fd=root_fd)
        directory_fd = os.open(name, DIR_FLAGS, dir_fd=root_fd)
        os.fchown(directory_fd, uid, gid)
        os.fchmod(directory_fd, 0o700)
        write_residue_marker(
            directory_fd,
            INCOMING_MARKER,
            residue_marker_value("gateway-release", name),
            uid,
            gid,
        )
        os.mkdir(INCOMING_PAYLOAD, 0o700, dir_fd=directory_fd)
        payload_directory_fd = os.open(
            INCOMING_PAYLOAD,
            DIR_FLAGS,
            dir_fd=directory_fd,
        )
        try:
            os.fchown(payload_directory_fd, uid, gid)
            os.fchmod(payload_directory_fd, 0o700)
            payload_fd = os.open(
                "README.txt",
                FILE_CREATE_RW_FLAGS,
                0o644,
                dir_fd=payload_directory_fd,
            )
            try:
                os.fchown(payload_fd, uid, gid)
                os.fchmod(payload_fd, 0o644)
                write_all(payload_fd, b"residue-test\n")
                os.fsync(payload_fd)
            finally:
                os.close(payload_fd)
            os.fsync(payload_directory_fd)
        finally:
            os.close(payload_directory_fd)
    else:
        fail("residue selftest kind is invalid")
    os.fsync(directory_fd)
    os.fsync(root_fd)
    return name, directory_fd


def cleanup_residue_test(root_fd, kind, name, directory_fd):
    if kind == "download":
        cleanup_download_residue(
            root_fd,
            name,
            directory_fd,
            os.geteuid(),
            os.getegid(),
            1_048_576,
        )
    else:
        cleanup_unpublished_release(
            root_fd,
            name,
            directory_fd,
            os.geteuid(),
            os.getegid(),
        )


def residue_hold_test(root_path, kind, hook_path):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("residue selftest must not run as root")
    root_fd = open_residue_test_root(root_path)
    directory_fd = None
    name = None
    try:
        name, directory_fd = create_residue_test(root_fd, kind)
        write_residue_test_ready(hook_path)
        while True:
            signal.pause()
    finally:
        if directory_fd is not None and name is not None:
            cleanup_residue_test(root_fd, kind, name, directory_fd)
            os.close(directory_fd)
        os.close(root_fd)


def residue_sweep_test(root_path, kind):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("residue selftest must not run as root")
    root_fd = open_residue_test_root(root_path)
    try:
        if kind == "download":
            sweep_download_residues(
                root_fd,
                os.geteuid(),
                os.getegid(),
                1_048_576,
            )
        elif kind == "incoming":
            sweep_unpublished_releases(
                root_fd,
                os.geteuid(),
                os.getegid(),
            )
        else:
            fail("residue selftest kind is invalid")
    finally:
        os.close(root_fd)


def recovery_dir_test(data_root):
    require_linux_dirfd()
    if os.geteuid() == 0:
        fail("recovery dirfd selftest must not run as root")
    root_fd = os.open(data_root, DIR_FLAGS)
    root_stat = os.fstat(root_fd)
    expected_uid = os.geteuid()
    expected_gid = os.getegid()
    if (
        not stat.S_ISDIR(root_stat.st_mode)
        or root_stat.st_uid != expected_uid
        or root_stat.st_gid != expected_gid
    ):
        os.close(root_fd)
        fail("recovery selftest root ownership is unsafe")
    try:
        current_fd = root_fd
        opened = []
        for component in ("save-recovery", "v1"):
            created = False
            try:
                next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
            except FileNotFoundError:
                try:
                    os.mkdir(component, 0o711, dir_fd=current_fd)
                    created = True
                except FileExistsError:
                    pass
                try:
                    next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
                except OSError:
                    fail("recovery selftest creation raced with an unsafe component")
            except OSError:
                fail("recovery selftest encountered an intermediate symbolic link")
            item_stat = os.fstat(next_fd)
            if created:
                os.fchmod(next_fd, 0o711)
                item_stat = os.fstat(next_fd)
            if (
                item_stat.st_uid != expected_uid
                or item_stat.st_gid != expected_gid
                or stat.S_IMODE(item_stat.st_mode) != 0o711
            ):
                fail("recovery selftest namespace owner/mode contract failed")
            opened.append(next_fd)
            current_fd = next_fd

        created = False
        try:
            leaf_fd = os.open("gateway", DIR_FLAGS, dir_fd=current_fd)
        except FileNotFoundError:
            try:
                os.mkdir("gateway", 0o700, dir_fd=current_fd)
                created = True
            except FileExistsError:
                pass
            try:
                leaf_fd = os.open("gateway", DIR_FLAGS, dir_fd=current_fd)
            except OSError:
                fail("recovery selftest leaf raced with an unsafe component")
        except OSError:
            fail("recovery selftest leaf is a symbolic link")
        leaf_stat = os.fstat(leaf_fd)
        if (
            leaf_stat.st_uid != expected_uid
            or leaf_stat.st_gid != expected_gid
        ):
            fail("recovery selftest leaf ownership contract failed")
        os.fchmod(leaf_fd, 0o700)
        os.fsync(leaf_fd)
        os.close(leaf_fd)
        for opened_fd in reversed(opened):
            os.close(opened_fd)
    finally:
        os.close(root_fd)


def main():
    if len(sys.argv) < 2:
        fail("missing internal engine mode")
    mode = sys.argv[1]
    args = sys.argv[2:]
    if mode == "validate-url" and len(args) == 1:
        print(validate_https_url(args[0]))
    elif mode == "curl-argv" and len(args) == 1:
        print("\n".join(build_curl_arguments(args[0])))
    elif mode == "reserved-owner-test" and len(args) == 2:
        print(reserved_prefix_owner_action(int(args[0]), int(args[1])))
    elif mode == "read-pin-test" and len(args) == 1:
        print("\n".join(read_pin_test(args[0])))
    elif mode == "read-pin" and len(args) == 3:
        require_linux_dirfd()
        print("\n".join(read_production_pin(*args)))
    elif mode == "validate-production-identity" and not args:
        require_linux_dirfd()
        print("\n".join(validate_production_identity()))
    elif mode == "inspect-identity-presence" and not args:
        require_linux_dirfd()
        print("\n".join(inspect_identity_presence()))
    elif mode == "validate-identity-fixture" and len(args) == 16:
        print("\n".join(validate_identity_fixture(args)))
    elif mode == "validate-archive" and len(args) == 5:
        archive_path, expected_sha, archive_limit, expanded_limit, member_limit = args
        archive_fd = open_archive(
            archive_path,
            expected_sha,
            int(archive_limit),
            require_root_owner=False,
        )
        try:
            results = parse_archive_from_fd(
                archive_fd,
                int(expanded_limit),
                int(member_limit),
                release_fd=None,
            )
            manifest = json.loads(results["RELEASE.json"]["data"].decode("utf-8"))
            validate_release_manifest(results, manifest.get("tag", ""))
        finally:
            os.close(archive_fd)
    elif mode == "install" and len(args) == 9:
        arm_parent_death_signal()
        install_interrupt_handlers()
        if args[8] not in {"0", "1"}:
            fail("activation mode is invalid")
        activate = args[8] == "1"
        install_release(
            args[0],
            args[1],
            args[2],
            int(args[3]),
            int(args[4]),
            int(args[5]),
            int(args[6]),
            int(args[7]),
            activate,
        )
    elif mode == "download-production" and len(args) == 2:
        arm_parent_death_signal()
        install_interrupt_handlers()
        print("\n".join(create_production_download(args[0], int(args[1]))))
    elif mode == "cleanup-production-download" and len(args) == 1:
        cleanup_production_download(args[0])
    elif mode == "download-test" and len(args) == 4:
        arm_parent_death_signal()
        install_interrupt_handlers()
        download_test(args[0], args[1], args[2], int(args[3]))
    elif mode == "install-layout-test" and len(args) == 8:
        arm_parent_death_signal()
        install_interrupt_handlers()
        install_layout_test(*args)
    elif mode == "install-source-swap-test" and len(args) == 8:
        arm_parent_death_signal()
        install_interrupt_handlers()
        install_source_swap_test(*args)
    elif mode == "same-fd-swap-test" and len(args) == 5:
        same_fd_swap_test(*args)
    elif mode == "regular-nlink-test" and len(args) == 1:
        regular_nlink_test(args[0])
    elif mode == "write-service-test" and len(args) == 1:
        write_service_test(args[0])
    elif mode == "render-env-test" and len(args) == 2:
        render_env_test(*args)
    elif mode == "atomic-root-file-test" and len(args) == 5:
        arm_parent_death_signal()
        install_interrupt_handlers()
        atomic_root_file_test(*args)
    elif mode == "validate-env-test" and len(args) == 1:
        validate_env_test(args[0])
    elif mode == "validate-activation-env-test" and len(args) == 1:
        validate_activation_env_test(args[0])
    elif mode == "residue-hold-test" and len(args) == 3:
        arm_parent_death_signal()
        install_interrupt_handlers()
        residue_hold_test(*args)
    elif mode == "residue-sweep-test" and len(args) == 2:
        residue_sweep_test(*args)
    elif mode == "recovery-dir-test" and len(args) == 1:
        recovery_dir_test(args[0])
    else:
        fail("invalid internal engine invocation")


try:
    main()
except SecurityError as exc:
    print(f"gateway installer: {exc}", file=sys.stderr)
    raise SystemExit(1)
except (OSError, ValueError, json.JSONDecodeError) as exc:
    print(
        f"gateway installer: fail-closed operating-system error ({type(exc).__name__})",
        file=sys.stderr,
    )
    raise SystemExit(1)
PY
}

sanitize_exported_environment() {
  local environment_name
  while IFS= read -r environment_name; do
    case "$environment_name" in
      PATH|LC_ALL|MIR2_INSTALLER_SANITIZED)
        ;;
      *)
        builtin unset -v "$environment_name" 2>/dev/null || true
        ;;
    esac
  done < <(builtin compgen -e)
  PATH=/usr/sbin:/usr/bin:/sbin:/bin
  LC_ALL=C
  MIR2_INSTALLER_SANITIZED=1
  export PATH LC_ALL MIR2_INSTALLER_SANITIZED
  unset BASH_ENV ENV CDPATH GLOBIGNORE PYTHONHOME PYTHONPATH PYTHONSTARTUP
  unset LD_AUDIT LD_LIBRARY_PATH LD_PRELOAD
  unset DBUS_SESSION_BUS_ADDRESS SYSTEMD_BUS_ADDRESS SYSTEMD_BUS_TIMEOUT
  unset CURL_HOME HOME XDG_CONFIG_HOME
  unset http_proxy https_proxy all_proxy no_proxy
  unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY
}

run_clean_command() {
  /usr/bin/env -i \
    PATH=/usr/sbin:/usr/bin:/sbin:/bin \
    LC_ALL=C \
    "$@"
}

case "${1:-}" in
  --selftest-reject-caller-authority)
    [ "$#" -eq 1 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    reject_caller_release_authority
    exit
    ;;
  --selftest-validate-url)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" validate-url "$2"
    exit
    ;;
  --selftest-read-pin)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" read-pin-test "$2"
    exit
    ;;
  --selftest-validate-archive)
    [ "$#" -eq 6 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" validate-archive \
      "$2" "$3" "$4" "$5" "$6"
    exit
    ;;
  --selftest-curl-argv)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" curl-argv "$2"
    exit
    ;;
  --selftest-reserved-prefix-owner)
    [ "$#" -eq 3 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" reserved-owner-test "$2" "$3"
    exit
    ;;
  --selftest-download)
    [ "$#" -eq 5 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" download-test "$2" "$3" "$4" "$5"
    exit
    ;;
  --selftest-install-layout)
    [ "$#" -eq 9 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" install-layout-test "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9"
    exit
    ;;
  --selftest-install-source-swap)
    [ "$#" -eq 9 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" install-source-swap-test \
      "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9"
    exit
    ;;
  --selftest-validate-identity)
    [ "$#" -eq 17 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" validate-identity-fixture \
      "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" \
      "${10}" "${11}" "${12}" "${13}" "${14}" "${15}" "${16}" "${17}"
    exit
    ;;
  --selftest-sanitize-environment)
    [ "$#" -eq 1 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    sanitize_exported_environment
    for unsafe_name in SYSTEMD_BUS_ADDRESS DBUS_SESSION_BUS_ADDRESS LD_LIBRARY_PATH PYTHONPATH CURL_HOME HTTPS_PROXY; do
      if [[ -v "$unsafe_name" ]]; then
        installer_error "unsafe inherited environment survived sanitization"
        exit 1
      fi
    done
    [ "$PATH" = "/usr/sbin:/usr/bin:/sbin:/bin" ] &&
      [ "$LC_ALL" = "C" ] ||
      { installer_error "sanitized environment allowlist is invalid"; exit 1; }
    exit
    ;;
  --selftest-same-fd-swap)
    [ "$#" -eq 6 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" same-fd-swap-test \
      "$2" "$3" "$4" "$5" "$6"
    exit
    ;;
  --selftest-regular-nlink)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" regular-nlink-test "$2"
    exit
    ;;
  --selftest-render-service)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" write-service-test "$2"
    exit
    ;;
  --selftest-render-env)
    [ "$#" -eq 3 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" render-env-test "$2" "$3"
    exit
    ;;
  --selftest-atomic-root-file)
    [ "$#" -eq 6 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" atomic-root-file-test \
      "$2" "$3" "$4" "$5" "$6"
    exit
    ;;
  --selftest-validate-env)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" validate-env-test "$2"
    exit
    ;;
  --selftest-validate-activation-env)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" validate-activation-env-test "$2"
    exit
    ;;
  --selftest-residue-hold)
    [ "$#" -eq 4 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" residue-hold-test "$2" "$3" "$4"
    exit
    ;;
  --selftest-residue-sweep)
    [ "$#" -eq 3 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" residue-sweep-test "$2" "$3"
    exit
    ;;
  --selftest-recovery-dir)
    [ "$#" -eq 2 ] || { installer_error "invalid selftest invocation"; exit 2; }
    ((EUID != 0)) || { installer_error "selftest modes refuse uid 0"; exit 1; }
    find_selftest_python
    run_python_engine "$selftest_python" recovery-dir-test "$2"
    exit
    ;;
esac

activate=0
case "${1:-}" in
  "")
    ;;
  --activate)
    [ "$#" -eq 1 ] || { usage >&2; exit 2; }
    activate=1
    ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

((EUID == 0)) ||
  { installer_error "secure bootstrap requires a sanitized root execution context"; exit 1; }
case "$-" in
  *p*) ;;
  *)
    installer_error "production installer must run through its #!/bin/bash -p shebang"
    exit 1
    ;;
esac
[ "$0" = "$trusted_installer_path" ] &&
  [ "${BASH_SOURCE[0]}" = "$trusted_installer_path" ] ||
  {
    installer_error "run only the preinstalled fixed-path trusted installer"
    exit 1
  }

reject_caller_release_authority
if [ "${MIR2_INSTALLER_SANITIZED:-0}" != "1" ]; then
  sanitize_exported_environment
  exec /usr/bin/env -i \
    PATH=/usr/sbin:/usr/bin:/sbin:/bin \
    LC_ALL=C \
    MIR2_INSTALLER_SANITIZED=1 \
    "$trusted_installer_path" "$@"
fi
sanitize_exported_environment
umask 077
[ -x "$python3_path" ] ||
  { installer_error "/usr/bin/python3 is required"; exit 1; }
[ -x /usr/bin/curl ] ||
  { installer_error "/usr/bin/curl is required"; exit 1; }
[ -x /usr/sbin/groupadd ] &&
  [ -x /usr/sbin/useradd ] &&
  [ -x /usr/bin/getent ] &&
  [ -x /usr/bin/id ] ||
  { installer_error "fixed system identity tools are required"; exit 1; }

pin_output="$(
  run_python_engine "$python3_path" read-pin \
    "$trusted_installer_path" "$trusted_pin_path" "$trusted_env_template_path"
)" || exit 1
mapfile -t pin_values <<< "$pin_output"
[ "${#pin_values[@]}" -eq 3 ] ||
  { installer_error "trusted pin parser returned an invalid contract"; exit 1; }
release_url="${pin_values[0]}"
release_sha256="${pin_values[1]}"
release_tag="${pin_values[2]}"

identity_presence_output="$(
  run_python_engine "$python3_path" inspect-identity-presence
)" || exit 1
mapfile -t identity_presence <<< "$identity_presence_output"
[ "${#identity_presence[@]}" -eq 2 ] ||
  { installer_error "identity presence preflight returned an invalid contract"; exit 1; }
case "${identity_presence[1]}" in
  present) ;;
  missing)
  run_clean_command /usr/sbin/groupadd --system mir2
    ;;
  *) installer_error "identity group presence preflight was invalid"; exit 1 ;;
esac
case "${identity_presence[0]}" in
  present) ;;
  missing)
  run_clean_command /usr/sbin/useradd \
    --system \
    --gid mir2 \
    --home /var/lib/mir2/gateway-data \
    --shell /usr/sbin/nologin \
    --no-create-home \
    mir2
    ;;
  *) installer_error "identity user presence preflight was invalid"; exit 1 ;;
esac
identity_output="$(
  run_python_engine "$python3_path" validate-production-identity
)" || exit 1
mapfile -t identity_values <<< "$identity_output"
[ "${#identity_values[@]}" -eq 2 ] ||
  { installer_error "trusted identity validator returned an invalid contract"; exit 1; }
service_uid="${identity_values[0]}"
service_gid="${identity_values[1]}"

download_output="$(
  run_python_engine "$python3_path" download-production \
    "$release_url" "$archive_max_bytes"
)" || exit 1
mapfile -t download_values <<< "$download_output"
[ "${#download_values[@]}" -eq 2 ] ||
  { installer_error "bounded downloader returned an invalid contract"; exit 1; }
archive="${download_values[0]}"
download_temp_name="${download_values[1]}"
[ "$archive" = "/var/tmp/$download_temp_name/release.tar.gz" ] ||
  { installer_error "bounded downloader returned an unsafe path"; exit 1; }

cleanup_download_temp() {
  if [ -n "${download_temp_name:-}" ]; then
    run_python_engine "$python3_path" cleanup-production-download \
      "$download_temp_name"
    download_temp_name=""
  fi
}
trap 'cleanup_download_temp || installer_error "root-private download cleanup failed"' EXIT
installer_signal_exit() {
  local status="$1"
  trap - EXIT HUP INT TERM
  cleanup_download_temp || installer_error "root-private download cleanup failed"
  exit "$status"
}
trap 'installer_signal_exit 129' HUP
trap 'installer_signal_exit 130' INT
trap 'installer_signal_exit 143' TERM

run_python_engine "$python3_path" install \
  "$archive" "$release_sha256" "$release_tag" \
  "$service_uid" "$service_gid" \
  "$archive_max_bytes" "$expanded_max_bytes" "$archive_max_members" "$activate"

cleanup_download_temp
trap - EXIT HUP INT TERM

if [ "$activate" -eq 1 ]; then
  run_clean_command /usr/bin/systemctl daemon-reload
  run_clean_command /usr/bin/systemctl enable --now mir2-gateway
  run_clean_command /usr/bin/systemctl restart mir2-gateway
fi

printf '%s\n' "installed trusted mir2-gateway release $release_tag"
printf '%s\n' "current: /opt/mir2/gateway/current"
printf '%s\n' "env: /etc/mir2/gateway.env"