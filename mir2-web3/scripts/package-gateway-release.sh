#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PACKAGE="mir2-gateway"
BIN="mir2-gateway"
ZONE_BIN="zone_host"
TOOLCHAIN="${MIR2_RUST_TOOLCHAIN:-1.89.0}"
TARGET="${MIR2_RELEASE_TARGET:-}"
OUT_DIR="${MIR2_RELEASE_OUT_DIR:-$ROOT/dist/gateway-releases}"
TAG="${MIR2_RELEASE_TAG:-}"

validate_release_target() {
  case "$1" in
    ""|x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-unknown-linux-musl|aarch64-unknown-linux-musl)
      ;;
    *)
      echo "MIR2_RELEASE_TARGET is not an allowlisted Linux target" >&2
      return 1
      ;;
  esac
}

validate_release_tag() {
  local value="$1"
  if [[ ! "$value" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]] ||
    [ "$value" = "." ] || [ "$value" = ".." ] ||
    [[ "$value" = incoming.* ]]; then
    echo "MIR2_RELEASE_TAG must be one safe path component" >&2
    return 1
  fi
}

validate_publisher_identity() {
  ((EUID != 0)) || {
    echo "production packaging must run as a dedicated non-root publisher" >&2
    return 1
  }
  local publisher_uid
  publisher_uid="$(/usr/bin/id -u)"
  case "${MIR2_RELEASE_PUBLISHER_UID:-}" in
    ''|*[!0-9]*)
      echo "set MIR2_RELEASE_PUBLISHER_UID to the dedicated publisher UID" >&2
      return 1
      ;;
  esac
  [ "$MIR2_RELEASE_PUBLISHER_UID" = "$publisher_uid" ] || {
    echo "MIR2_RELEASE_PUBLISHER_UID differs from the effective publisher UID" >&2
    return 1
  }
}

publisher_python_engine() {
  local interpreter="$1"
  shift
  /usr/bin/env -i \
    PATH=/usr/sbin:/usr/bin:/sbin:/bin \
    LC_ALL=C \
    "$interpreter" -I - "$@" <<'PY'
import errno
import hashlib
import json
import os
import re
import secrets
import signal
import stat
import sys
import tarfile
import time
import ctypes

O_NOFOLLOW = getattr(os, "O_NOFOLLOW", 0)
O_DIRECTORY = getattr(os, "O_DIRECTORY", 0)
O_CLOEXEC = getattr(os, "O_CLOEXEC", 0)
DIR_FLAGS = os.O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC
READ_FLAGS = os.O_RDONLY | O_NOFOLLOW | O_CLOEXEC
CREATE_FLAGS = os.O_RDWR | os.O_CREAT | os.O_EXCL | O_NOFOLLOW | O_CLOEXEC
STAGE_PREFIX = "mir2-gateway-package."
TRANSACTION_PREFIX = "package-incoming."
STAGE_MARKER = ".mir2-package-stage.json"
TRANSACTION_MARKER = ".mir2-package-transaction.json"
STAGE_MEMBERS = {
    "mir2-gateway": (0o755, 268_435_456),
    "zone_host": (0o755, 268_435_456),
    "RELEASE.json": (0o644, 65_536),
    "README.txt": (0o644, 65_536),
}
MAX_ACTIVE = 8
MAX_RESIDUAL_BYTES = 1_073_741_824
STALE_AFTER_SECONDS = 3600
MAX_FUTURE_CLOCK_SKEW = 300


class PublishError(Exception):
    pass


def fail(message):
    raise PublishError(message)


def require_linux():
    if not sys.platform.startswith("linux"):
        fail("secure release publication requires Linux")
    if not O_NOFOLLOW or not O_DIRECTORY:
        fail("O_NOFOLLOW and O_DIRECTORY are required")


def arm_parent_death_signal():
    parent_before = os.getppid()
    libc = ctypes.CDLL(None, use_errno=True)
    call = getattr(libc, "prctl", None)
    if call is None:
        fail("Linux PR_SET_PDEATHSIG support is required")
    call.argtypes = [
        ctypes.c_int,
        ctypes.c_ulong,
        ctypes.c_ulong,
        ctypes.c_ulong,
        ctypes.c_ulong,
    ]
    call.restype = ctypes.c_int
    if call(1, signal.SIGTERM, 0, 0, 0) != 0:
        fail("could not arm publisher parent-death handling")
    if os.getppid() != parent_before:
        fail("publisher parent changed while arming parent-death handling")


def install_interrupt_handlers():
    handled_signals = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)

    def interrupted(signum, frame):
        del signum, frame
        for signal_number in handled_signals:
            signal.signal(signal_number, signal.SIG_IGN)
        raise PublishError("publication interrupted; private transaction cleaned")

    for signal_number in handled_signals:
        signal.signal(signal_number, interrupted)


def read_boot_id():
    file_fd = os.open(
        "/proc/sys/kernel/random/boot_id",
        READ_FLAGS,
    )
    try:
        value = read_fd(file_fd, 128).decode("ascii").strip()
    except UnicodeDecodeError:
        fail("Linux boot identity is malformed")
    finally:
        os.close(file_fd)
    if not re.fullmatch(
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        value,
    ):
        fail("Linux boot identity is malformed")
    return value


def process_start_ticks(process_id):
    if not isinstance(process_id, int) or process_id < 1:
        fail("publisher owner process id is invalid")
    try:
        file_fd = os.open(f"/proc/{process_id}/stat", READ_FLAGS)
    except FileNotFoundError:
        return None
    except OSError:
        fail("publisher owner process identity cannot be inspected")
    try:
        raw = read_fd(file_fd, 16_384)
    finally:
        os.close(file_fd)
    closing = raw.rfind(b") ")
    if closing < 1:
        fail("publisher owner process identity is malformed")
    fields = raw[closing + 2:].split()
    if len(fields) <= 19 or not fields[19].isdigit():
        fail("publisher owner process start time is malformed")
    return int(fields[19], 10)


def new_marker(kind, directory_name):
    owner_pid = os.getppid()
    owner_start = process_start_ticks(owner_pid)
    if owner_start is None:
        fail("publisher parent disappeared before transaction creation")
    return {
        "version": 1,
        "kind": kind,
        "directory": directory_name,
        "created": int(time.time()),
        "boot_id": read_boot_id(),
        "owner_pid": owner_pid,
        "owner_start_ticks": owner_start,
    }


def marker_owner_is_live(marker):
    if marker["boot_id"] != read_boot_id():
        return False
    observed = process_start_ticks(marker["owner_pid"])
    return observed is not None and observed == marker["owner_start_ticks"]


def safe_component(value, label, maximum=240):
    if (
        not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,%d}" % maximum, value)
        or value in {".", ".."}
    ):
        fail(f"{label} must be one safe non-dot component")
    return value


def path_parts(path):
    if (
        not path.startswith("/")
        or path == "/"
        or "//" in path
        or len(os.fsencode(path)) > 4096
    ):
        fail("publisher path must be a narrow absolute path")
    parts = path.split("/")[1:]
    if len(parts) > 32 or any(
        not part
        or len(os.fsencode(part)) > 127
        or part in {".", ".."}
        or not re.fullmatch(r"[A-Za-z0-9._-]+", part)
        for part in parts
    ):
        fail("publisher path contains an unsupported component")
    return parts


def write_all(file_fd, data):
    view = memoryview(data)
    while view:
        count = os.write(file_fd, view)
        if count <= 0:
            fail("short write in publisher")
        view = view[count:]


def hash_fd(file_fd):
    os.lseek(file_fd, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    while True:
        chunk = os.read(file_fd, 1_048_576)
        if not chunk:
            return digest.hexdigest()
        digest.update(chunk)


def read_fd(file_fd, maximum):
    os.lseek(file_fd, 0, os.SEEK_SET)
    output = bytearray()
    while True:
        chunk = os.read(file_fd, min(65_536, maximum + 1 - len(output)))
        if not chunk:
            return bytes(output)
        output.extend(chunk)
        if len(output) > maximum:
            fail("publisher metadata exceeds its fixed size bound")


def bounded_names(directory_fd, maximum, label):
    names = []
    with os.scandir(directory_fd) as entries:
        for entry in entries:
            names.append(entry.name)
            if len(names) > maximum:
                fail(f"{label} entry-count bound exceeded")
    return names


def open_tmp():
    root_fd = os.open("/", DIR_FLAGS)
    try:
        tmp_fd = os.open("tmp", DIR_FLAGS, dir_fd=root_fd)
    except Exception:
        os.close(root_fd)
        raise
    os.close(root_fd)
    item = os.fstat(tmp_fd)
    if (
        not stat.S_ISDIR(item.st_mode)
        or item.st_uid != 0
        or item.st_gid != 0
        or stat.S_IMODE(item.st_mode) != 0o1777
    ):
        os.close(tmp_fd)
        fail("/tmp must be root:root mode 1777")
    return tmp_fd


def open_output_directory(path, create):
    parts = path_parts(path)
    publisher_uid = os.geteuid()
    publisher_gid = os.getegid()
    current_fd = os.open("/", DIR_FLAGS)
    try:
        root_stat = os.fstat(current_fd)
        if root_stat.st_uid != 0 or stat.S_IMODE(root_stat.st_mode) & 0o022:
            fail("filesystem root is not a trusted publisher ancestor")
        for index, component in enumerate(parts):
            final = index == len(parts) - 1
            try:
                next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
            except FileNotFoundError:
                if not create:
                    fail("publisher output path is missing")
                parent_stat = os.fstat(current_fd)
                if parent_stat.st_uid != publisher_uid:
                    fail("only publisher-owned ancestors may be extended")
                os.mkdir(component, 0o700, dir_fd=current_fd)
                next_fd = os.open(component, DIR_FLAGS, dir_fd=current_fd)
                os.fchown(next_fd, publisher_uid, publisher_gid)
                os.fchmod(next_fd, 0o700)
                os.fsync(current_fd)
            except OSError:
                fail("publisher output path contains a symlink or non-directory")
            item = os.fstat(next_fd)
            mode = stat.S_IMODE(item.st_mode)
            trusted_sticky_tmp = (
                index == 0
                and component == "tmp"
                and item.st_uid == 0
                and item.st_gid == 0
                and mode == 0o1777
            )
            if not stat.S_ISDIR(item.st_mode) or item.st_nlink < 2:
                os.close(next_fd)
                fail("publisher output ancestor type/nlink contract failed")
            if not trusted_sticky_tmp and (
                item.st_uid not in {0, publisher_uid} or mode & 0o022
            ):
                os.close(next_fd)
                fail("publisher output ancestor owner/mode/nlink contract failed")
            if final and (
                item.st_uid != publisher_uid
                or item.st_gid != publisher_gid
                or mode != 0o700
            ):
                os.close(next_fd)
                fail("publisher output directory must be private mode 0700")
            os.close(current_fd)
            current_fd = next_fd
        return current_fd
    except Exception:
        os.close(current_fd)
        raise


def revalidate_output_directory(path, held_fd):
    check_fd = open_output_directory(path, False)
    try:
        held = os.fstat(held_fd)
        check = os.fstat(check_fd)
        if (held.st_dev, held.st_ino) != (check.st_dev, check.st_ino):
            fail("publisher output directory identity changed concurrently")
    finally:
        os.close(check_fd)


def write_json_exclusive(directory_fd, name, value, mode=0o600):
    data = (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")
    file_fd = os.open(name, CREATE_FLAGS, mode, dir_fd=directory_fd)
    try:
        os.fchown(file_fd, os.geteuid(), os.getegid())
        os.fchmod(file_fd, mode)
        write_all(file_fd, data)
        os.fsync(file_fd)
    finally:
        os.close(file_fd)


def read_marker(directory_fd, name, kind, directory_name):
    marker_fd = os.open(name, READ_FLAGS, dir_fd=directory_fd)
    try:
        item = os.fstat(marker_fd)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != os.geteuid()
            or item.st_gid != os.getegid()
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) != 0o600
        ):
            fail("publisher marker identity contract failed")
        try:
            marker = json.loads(read_fd(marker_fd, 16_384).decode("ascii"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            fail("publisher marker is malformed")
    finally:
        os.close(marker_fd)
    if (
        not isinstance(marker, dict)
        or type(marker.get("version")) is not int
        or marker.get("version") != 1
        or marker.get("kind") != kind
        or marker.get("directory") != directory_name
        or type(marker.get("created")) is not int
        or not isinstance(marker.get("boot_id"), str)
        or type(marker.get("owner_pid")) is not int
        or type(marker.get("owner_start_ticks")) is not int
    ):
        fail("publisher marker schema mismatch")
    return marker


def replace_marker(directory_fd, marker):
    temporary = ".marker." + secrets.token_hex(12)
    try:
        write_json_exclusive(directory_fd, temporary, marker)
        existing_fd = os.open(TRANSACTION_MARKER, READ_FLAGS, dir_fd=directory_fd)
        try:
            item = os.fstat(existing_fd)
            if (
                not stat.S_ISREG(item.st_mode)
                or item.st_uid != os.geteuid()
                or item.st_gid != os.getegid()
                or item.st_nlink != 1
                or stat.S_IMODE(item.st_mode) != 0o600
            ):
                fail("transaction marker changed identity")
        finally:
            os.close(existing_fd)
        os.replace(
            temporary,
            TRANSACTION_MARKER,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
        )
        os.fsync(directory_fd)
    finally:
        try:
            os.unlink(temporary, dir_fd=directory_fd)
        except FileNotFoundError:
            pass


def open_owned_regular(directory_fd, name, expected_mode, maximum):
    file_fd = os.open(name, READ_FLAGS, dir_fd=directory_fd)
    item = os.fstat(file_fd)
    if (
        not stat.S_ISREG(item.st_mode)
        or item.st_uid != os.geteuid()
        or item.st_gid != os.getegid()
        or item.st_nlink != 1
        or stat.S_IMODE(item.st_mode) != expected_mode
        or item.st_size <= 0
        or item.st_size > maximum
    ):
        os.close(file_fd)
        fail("publisher source file owner/mode/nlink/size contract failed")
    return file_fd, item


def cleanup_directory(parent_fd, name, directory_fd, allowed):
    entries = bounded_names(directory_fd, len(allowed), "publisher cleanup")
    if any(entry not in allowed for entry in entries):
        fail("publisher residue contains an unknown entry")
    for entry in entries:
        item_fd = os.open(entry, READ_FLAGS, dir_fd=directory_fd)
        try:
            item = os.fstat(item_fd)
            if (
                not stat.S_ISREG(item.st_mode)
                or item.st_uid != os.geteuid()
                or item.st_gid != os.getegid()
                or item.st_nlink != 1
                or stat.S_IMODE(item.st_mode) & 0o022
            ):
                fail("publisher residue file identity changed")
        finally:
            os.close(item_fd)
        os.unlink(entry, dir_fd=directory_fd)
    os.fsync(directory_fd)
    os.rmdir(name, dir_fd=parent_fd)
    os.fsync(parent_fd)


def residue_size(directory_fd, allowed):
    total = 0
    entries = bounded_names(directory_fd, len(allowed), "publisher residue")
    if any(entry not in allowed for entry in entries):
        fail("publisher residue contains an unknown entry")
    for entry in entries:
        item = os.stat(entry, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != os.geteuid()
            or item.st_gid != os.getegid()
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) & 0o022
        ):
            fail("publisher residue file owner/mode/nlink contract failed")
        total += item.st_size
        if total > MAX_RESIDUAL_BYTES:
            fail("publisher residue exceeds its total byte bound")
    return total


def verify_final_file(output_fd, name, expected_mode, expected_sha=None, expected=None):
    file_fd = os.open(name, READ_FLAGS, dir_fd=output_fd)
    try:
        item = os.fstat(file_fd)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != os.geteuid()
            or item.st_gid != os.getegid()
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) != expected_mode
        ):
            fail("published file identity contract failed")
        if expected_sha is not None and hash_fd(file_fd) != expected_sha:
            fail("published archive differs from same-FD digest")
        if expected is not None and read_fd(file_fd, len(expected) + 1) != expected:
            fail("published sidecar differs from transaction marker")
    finally:
        os.close(file_fd)


def rename_noreplace(source_fd, source, destination_fd, destination):
    libc = ctypes.CDLL(None, use_errno=True)
    call = getattr(libc, "renameat2", None)
    if call is None:
        fail("renameat2(RENAME_NOREPLACE) is required")
    call.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    call.restype = ctypes.c_int
    result = call(
        source_fd,
        os.fsencode(source),
        destination_fd,
        os.fsencode(destination),
        1,
    )
    if result == 0:
        return True
    number = ctypes.get_errno()
    if number == errno.EEXIST:
        return False
    fail("atomic no-replace publisher rename failed")


def cleanup_stage_fd(tmp_fd, name, stage_fd):
    cleanup_directory(
        tmp_fd,
        name,
        stage_fd,
        set(STAGE_MEMBERS) | {STAGE_MARKER},
    )


def sweep_stage_residues(tmp_fd):
    now = int(time.time())
    active = 0
    total = 0
    for entry in os.scandir(tmp_fd):
        if not entry.name.startswith(STAGE_PREFIX):
            continue
        try:
            entry_stat = os.stat(
                entry.name,
                dir_fd=tmp_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            continue
        if reserved_prefix_owner_action(
            entry_stat.st_uid,
            os.geteuid(),
        ) == "ignore":
            continue
        if not re.fullmatch(r"mir2-gateway-package\.[0-9a-f]{24}", entry.name):
            fail("unknown package-stage prefix entry")
        stage_fd = os.open(entry.name, DIR_FLAGS, dir_fd=tmp_fd)
        try:
            item = os.fstat(stage_fd)
            if (
                item.st_uid != os.geteuid()
                or item.st_gid != os.getegid()
                or stat.S_IMODE(item.st_mode) != 0o700
                or item.st_nlink < 2
            ):
                fail("package-stage residue identity contract failed")
            try:
                marker = read_marker(
                    stage_fd,
                    STAGE_MARKER,
                    "package-stage",
                    entry.name,
                )
            except FileNotFoundError:
                if bounded_names(stage_fd, 1, "unmarked package stage"):
                    fail("unmarked package-stage residue is not empty")
                if now - int(item.st_ctime) >= STALE_AFTER_SECONDS:
                    os.rmdir(entry.name, dir_fd=tmp_fd)
                    os.fsync(tmp_fd)
                    continue
                active += 1
                continue
            age = now - marker["created"]
            if age < -MAX_FUTURE_CLOCK_SKEW:
                fail("package-stage marker timestamp is in the future")
            if not marker_owner_is_live(marker):
                cleanup_stage_fd(tmp_fd, entry.name, stage_fd)
                continue
            if age > STALE_AFTER_SECONDS:
                fail("live package-stage residue exceeded its maximum age")
            active += 1
            total += residue_size(
                stage_fd,
                set(STAGE_MEMBERS) | {STAGE_MARKER},
            )
        finally:
            os.close(stage_fd)
        if active > MAX_ACTIVE or total > MAX_RESIDUAL_BYTES:
            fail("active package-stage residue bound exceeded")


def reserved_prefix_owner_action(observed_uid, expected_uid):
    if (
        not isinstance(observed_uid, int)
        or not isinstance(expected_uid, int)
        or observed_uid < 0
        or expected_uid < 0
    ):
        fail("reserved-prefix owner classifier received an invalid UID")
    return "owned" if observed_uid == expected_uid else "ignore"


def create_stage():
    require_linux()
    tmp_fd = open_tmp()
    name = None
    stage_fd = None
    try:
        sweep_stage_residues(tmp_fd)
        for _ in range(32):
            name = STAGE_PREFIX + secrets.token_hex(12)
            try:
                os.mkdir(name, 0o700, dir_fd=tmp_fd)
                break
            except FileExistsError:
                continue
        else:
            fail("could not allocate private package stage")
        stage_fd = os.open(name, DIR_FLAGS, dir_fd=tmp_fd)
        os.fchown(stage_fd, os.geteuid(), os.getegid())
        os.fchmod(stage_fd, 0o700)
        write_json_exclusive(
            stage_fd,
            STAGE_MARKER,
            new_marker("package-stage", name),
        )
        os.fsync(stage_fd)
        os.fsync(tmp_fd)
        print(f"/tmp/{name}")
        print(name)
    except Exception:
        if stage_fd is not None and name is not None:
            try:
                cleanup_stage_fd(tmp_fd, name, stage_fd)
            except (OSError, PublishError):
                pass
        elif name is not None:
            try:
                os.rmdir(name, dir_fd=tmp_fd)
                os.fsync(tmp_fd)
            except OSError:
                pass
        raise
    finally:
        if stage_fd is not None:
            os.close(stage_fd)
        os.close(tmp_fd)


def open_stage(path):
    parts = path_parts(path)
    if (
        len(parts) != 2
        or parts[0] != "tmp"
        or not re.fullmatch(r"mir2-gateway-package\.[0-9a-f]{24}", parts[1])
    ):
        fail("package stage path is outside the fixed /tmp namespace")
    tmp_fd = open_tmp()
    try:
        stage_fd = os.open(parts[1], DIR_FLAGS, dir_fd=tmp_fd)
    except Exception:
        os.close(tmp_fd)
        raise
    item = os.fstat(stage_fd)
    if (
        item.st_uid != os.geteuid()
        or item.st_gid != os.getegid()
        or stat.S_IMODE(item.st_mode) != 0o700
        or item.st_nlink < 2
    ):
        os.close(stage_fd)
        os.close(tmp_fd)
        fail("package stage identity contract failed")
    read_marker(stage_fd, STAGE_MARKER, "package-stage", parts[1])
    return tmp_fd, parts[1], stage_fd


def cleanup_stage(name):
    require_linux()
    if not re.fullmatch(r"mir2-gateway-package\.[0-9a-f]{24}", name):
        fail("package-stage cleanup target is unsafe")
    tmp_fd = open_tmp()
    try:
        try:
            stage_fd = os.open(name, DIR_FLAGS, dir_fd=tmp_fd)
        except FileNotFoundError:
            return
        try:
            item = os.fstat(stage_fd)
            if (
                item.st_uid != os.geteuid()
                or item.st_gid != os.getegid()
                or stat.S_IMODE(item.st_mode) != 0o700
            ):
                fail("package-stage cleanup identity changed")
            read_marker(stage_fd, STAGE_MARKER, "package-stage", name)
            cleanup_stage_fd(tmp_fd, name, stage_fd)
        finally:
            os.close(stage_fd)
    finally:
        os.close(tmp_fd)


def cleanup_transaction(output_fd, name, transaction_fd):
    cleanup_directory(
        output_fd,
        name,
        transaction_fd,
        {
            TRANSACTION_MARKER,
            "payload.tar.gz",
            "payload.sha256",
            "latest.json",
        },
    )


def sweep_output_residues(output_fd):
    now = int(time.time())
    active = 0
    total = 0
    for entry in os.scandir(output_fd):
        if not entry.name.startswith(TRANSACTION_PREFIX):
            continue
        if not re.fullmatch(r"package-incoming\.[0-9a-f]{24}", entry.name):
            fail("unknown package transaction prefix entry")
        transaction_fd = os.open(entry.name, DIR_FLAGS, dir_fd=output_fd)
        try:
            item = os.fstat(transaction_fd)
            if (
                item.st_uid != os.geteuid()
                or item.st_gid != os.getegid()
                or stat.S_IMODE(item.st_mode) != 0o700
                or item.st_nlink < 2
            ):
                fail("package transaction residue identity contract failed")
            try:
                marker = read_marker(
                    transaction_fd,
                    TRANSACTION_MARKER,
                    "package-transaction",
                    entry.name,
                )
            except FileNotFoundError:
                if bounded_names(
                    transaction_fd,
                    1,
                    "unmarked package transaction",
                ):
                    fail("unmarked package transaction is not empty")
                if now - int(item.st_ctime) >= STALE_AFTER_SECONDS:
                    os.rmdir(entry.name, dir_fd=output_fd)
                    os.fsync(output_fd)
                    continue
                active += 1
                continue
            age = now - marker["created"]
            if age < -MAX_FUTURE_CLOCK_SKEW:
                fail("package transaction marker timestamp is in the future")
            if marker_owner_is_live(marker):
                if age > STALE_AFTER_SECONDS:
                    fail("live package transaction exceeded its maximum age")
                active += 1
                total += residue_size(
                    transaction_fd,
                    {
                        TRANSACTION_MARKER,
                        "payload.tar.gz",
                        "payload.sha256",
                        "latest.json",
                    },
                )
                continue
            archive_name = safe_component(marker.get("archive", ""), "archive")
            sidecar_name = safe_component(marker.get("sidecar", ""), "sidecar")
            archive_sha = marker.get("archive_sha")
            sidecar_bytes = marker.get("sidecar_bytes")
            try:
                os.stat(archive_name, dir_fd=output_fd, follow_symlinks=False)
                archive_exists = True
            except FileNotFoundError:
                archive_exists = False
            if archive_exists:
                if (
                    not isinstance(archive_sha, str)
                    or not re.fullmatch(r"[0-9a-f]{64}", archive_sha)
                    or not isinstance(sidecar_bytes, str)
                ):
                    fail("committed stale transaction lacks verification data")
                verify_final_file(output_fd, archive_name, 0o644, archive_sha)
                verify_final_file(
                    output_fd,
                    sidecar_name,
                    0o644,
                    expected=sidecar_bytes.encode("ascii"),
                )
            else:
                try:
                    os.stat(sidecar_name, dir_fd=output_fd, follow_symlinks=False)
                    sidecar_exists = True
                except FileNotFoundError:
                    sidecar_exists = False
                if sidecar_exists:
                    if not isinstance(sidecar_bytes, str):
                        fail("orphan sidecar lacks transaction verification data")
                    expected = sidecar_bytes.encode("ascii")
                    verify_final_file(
                        output_fd,
                        sidecar_name,
                        0o644,
                        expected=expected,
                    )
                    os.unlink(sidecar_name, dir_fd=output_fd)
                    os.fsync(output_fd)
            cleanup_transaction(output_fd, entry.name, transaction_fd)
        finally:
            os.close(transaction_fd)
        if active > MAX_ACTIVE or total > MAX_RESIDUAL_BYTES:
            fail("active package transaction residue bound exceeded")


def hook_wait(path, requested_phase, current_phase):
    phases = {"none", "sources", "transaction", "sidecar"}
    if requested_phase not in phases:
        fail("publisher selftest hook phase is invalid")
    if path == "-" or requested_phase != current_phase:
        return
    hook_fd = os.open(path, DIR_FLAGS)
    try:
        item = os.fstat(hook_fd)
        if (
            item.st_uid != os.geteuid()
            or item.st_gid != os.getegid()
            or stat.S_IMODE(item.st_mode) != 0o700
        ):
            fail("publisher selftest hook directory is unsafe")
        ready_fd = os.open("opened", CREATE_FLAGS, 0o600, dir_fd=hook_fd)
        os.close(ready_fd)
        deadline = time.monotonic() + 10
        while True:
            try:
                marker = os.stat("continue", dir_fd=hook_fd, follow_symlinks=False)
                if not stat.S_ISREG(marker.st_mode):
                    fail("publisher selftest continue marker is unsafe")
                break
            except FileNotFoundError:
                if time.monotonic() >= deadline:
                    fail("publisher selftest hook timed out")
                time.sleep(0.01)
    finally:
        os.close(hook_fd)


def add_tar_member(bundle, name, file_fd, item):
    info = tarfile.TarInfo(name)
    info.size = item.st_size
    info.mode = STAGE_MEMBERS[name][0]
    info.uid = 0
    info.gid = 0
    info.mtime = 0
    info.type = tarfile.REGTYPE
    info.uname = ""
    info.gname = ""
    os.lseek(file_fd, 0, os.SEEK_SET)
    with os.fdopen(os.dup(file_fd), "rb") as source:
        bundle.addfile(info, source)


def publish_stage(stage_path, output_path, archive_name, hook_path, hook_phase):
    require_linux()
    archive_name = safe_component(archive_name, "archive")
    if not archive_name.endswith(".tar.gz"):
        fail("archive name must end in .tar.gz")
    sidecar_name = safe_component(archive_name + ".sha256", "sidecar")
    tmp_fd, stage_name, stage_fd = open_stage(stage_path)
    output_fd = open_output_directory(output_path, True)
    source_fds = {}
    transaction_fd = None
    transaction_name = None
    sidecar_created = False
    archive_committed = False
    sidecar_bytes = b""
    try:
        if set(
            bounded_names(
                stage_fd,
                len(STAGE_MEMBERS) + 1,
                "private package stage",
            )
        ) != set(STAGE_MEMBERS) | {STAGE_MARKER}:
            fail("private package stage does not contain the exact whitelist")
        source_stats = {}
        source_hashes = {}
        for name, (mode, maximum) in STAGE_MEMBERS.items():
            file_fd, item = open_owned_regular(stage_fd, name, mode, maximum)
            source_fds[name] = file_fd
            source_stats[name] = item
            source_hashes[name] = hash_fd(file_fd)
        try:
            manifest = json.loads(
                read_fd(source_fds["RELEASE.json"], 65_536).decode("utf-8")
            )
        except (UnicodeDecodeError, json.JSONDecodeError):
            fail("private stage RELEASE.json is invalid")
        if (
            manifest.get("binarySha256") != source_hashes["mir2-gateway"]
            or manifest.get("zoneHostBinarySha256") != source_hashes["zone_host"]
            or manifest.get("binarySizeBytes") != source_stats["mir2-gateway"].st_size
            or manifest.get("zoneHostBinarySizeBytes") != source_stats["zone_host"].st_size
        ):
            fail("private stage metadata differs from same-FD binaries")

        sweep_output_residues(output_fd)
        hook_wait(hook_path, hook_phase, "sources")
        revalidate_output_directory(output_path, output_fd)

        for _ in range(32):
            candidate = TRANSACTION_PREFIX + secrets.token_hex(12)
            try:
                os.mkdir(candidate, 0o700, dir_fd=output_fd)
                transaction_name = candidate
                break
            except FileExistsError:
                continue
        if transaction_name is None:
            fail("could not allocate package publication transaction")
        transaction_fd = os.open(transaction_name, DIR_FLAGS, dir_fd=output_fd)
        os.fchown(transaction_fd, os.geteuid(), os.getegid())
        os.fchmod(transaction_fd, 0o700)
        transaction_marker = new_marker(
            "package-transaction",
            transaction_name,
        )
        transaction_marker.update(
            {
                "state": "building",
                "archive": archive_name,
                "sidecar": sidecar_name,
            }
        )
        write_json_exclusive(
            transaction_fd,
            TRANSACTION_MARKER,
            transaction_marker,
        )
        os.fsync(transaction_fd)
        os.fsync(output_fd)
        hook_wait(hook_path, hook_phase, "transaction")

        archive_fd = os.open(
            "payload.tar.gz",
            CREATE_FLAGS,
            0o600,
            dir_fd=transaction_fd,
        )
        try:
            os.fchown(archive_fd, os.geteuid(), os.getegid())
            with os.fdopen(os.dup(archive_fd), "wb") as archive_stream:
                with tarfile.open(
                    fileobj=archive_stream,
                    mode="w:gz",
                    format=tarfile.USTAR_FORMAT,
                ) as bundle:
                    for name in (
                        "mir2-gateway",
                        "zone_host",
                        "RELEASE.json",
                        "README.txt",
                    ):
                        add_tar_member(
                            bundle,
                            name,
                            source_fds[name],
                            source_stats[name],
                        )
            archive_stat = os.fstat(archive_fd)
            if archive_stat.st_size <= 0 or archive_stat.st_size > 536_870_912:
                fail("package archive exceeds the fixed compressed-byte limit")
            archive_sha = hash_fd(archive_fd)
            os.fchmod(archive_fd, 0o644)
            os.fsync(archive_fd)
            archive_size = archive_stat.st_size
        finally:
            os.close(archive_fd)

        sidecar_bytes = f"{archive_sha}  {archive_name}\n".encode("ascii")
        sidecar_fd = os.open(
            "payload.sha256",
            CREATE_FLAGS,
            0o600,
            dir_fd=transaction_fd,
        )
        try:
            os.fchown(sidecar_fd, os.geteuid(), os.getegid())
            os.fchmod(sidecar_fd, 0o644)
            write_all(sidecar_fd, sidecar_bytes)
            os.fsync(sidecar_fd)
        finally:
            os.close(sidecar_fd)

        transaction_marker.update(
            {
                "state": "publishing",
                "archive_sha": archive_sha,
                "archive_size": archive_size,
                "sidecar_bytes": sidecar_bytes.decode("ascii"),
            }
        )
        replace_marker(transaction_fd, transaction_marker)
        os.fsync(transaction_fd)

        revalidate_output_directory(output_path, output_fd)
        if rename_noreplace(
            transaction_fd,
            "payload.sha256",
            output_fd,
            sidecar_name,
        ):
            sidecar_created = True
        else:
            verify_final_file(
                output_fd,
                sidecar_name,
                0o644,
                expected=sidecar_bytes,
            )
            os.unlink("payload.sha256", dir_fd=transaction_fd)
        os.fsync(output_fd)
        hook_wait(hook_path, hook_phase, "sidecar")

        if rename_noreplace(
            transaction_fd,
            "payload.tar.gz",
            output_fd,
            archive_name,
        ):
            archive_committed = True
        else:
            verify_final_file(output_fd, archive_name, 0o644, archive_sha)
            os.unlink("payload.tar.gz", dir_fd=transaction_fd)
            archive_committed = True
        os.fsync(output_fd)
        revalidate_output_directory(output_path, output_fd)

        latest = dict(manifest)
        latest["archive"] = archive_name
        latest["archiveSha256"] = archive_sha
        latest_bytes = (
            json.dumps(latest, ensure_ascii=True, indent=2) + "\n"
        ).encode("ascii")
        latest_fd = os.open(
            "latest.json",
            CREATE_FLAGS,
            0o600,
            dir_fd=transaction_fd,
        )
        try:
            os.fchown(latest_fd, os.geteuid(), os.getegid())
            os.fchmod(latest_fd, 0o644)
            write_all(latest_fd, latest_bytes)
            os.fsync(latest_fd)
        finally:
            os.close(latest_fd)
        latest_name = "latest-mir2-gateway-release.json"
        try:
            existing = os.stat(
                latest_name,
                dir_fd=output_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            existing = None
        if existing is not None and (
            not stat.S_ISREG(existing.st_mode)
            or existing.st_uid != os.geteuid()
            or existing.st_gid != os.getegid()
            or existing.st_nlink != 1
            or stat.S_IMODE(existing.st_mode) != 0o644
        ):
            fail("existing latest release metadata is unsafe")
        os.replace(
            "latest.json",
            latest_name,
            src_dir_fd=transaction_fd,
            dst_dir_fd=output_fd,
        )
        os.fsync(output_fd)

        cleanup_transaction(
            output_fd,
            transaction_name,
            transaction_fd,
        )
        transaction_name = None
        print(f"{output_path}/{archive_name}")
        print(str(archive_size))
        print(archive_sha)
        print(source_hashes["mir2-gateway"])
        print(source_hashes["zone_host"])
    except Exception:
        if transaction_fd is not None and transaction_name is not None:
            if sidecar_created and not archive_committed:
                try:
                    os.stat(
                        sidecar_name,
                        dir_fd=output_fd,
                        follow_symlinks=False,
                    )
                    verify_final_file(
                        output_fd,
                        sidecar_name,
                        0o644,
                        expected=sidecar_bytes,
                    )
                    os.unlink(sidecar_name, dir_fd=output_fd)
                    os.fsync(output_fd)
                except FileNotFoundError:
                    pass
            cleanup_transaction(
                output_fd,
                transaction_name,
                transaction_fd,
            )
        raise
    finally:
        for file_fd in source_fds.values():
            os.close(file_fd)
        if transaction_fd is not None:
            os.close(transaction_fd)
        os.close(output_fd)
        os.close(stage_fd)
        os.close(tmp_fd)


def main():
    require_linux()
    arm_parent_death_signal()
    install_interrupt_handlers()
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    args = sys.argv[2:]
    if mode == "create-stage" and not args:
        create_stage()
    elif mode == "platform-check" and not args:
        return
    elif mode == "reserved-owner-test" and len(args) == 2:
        print(reserved_prefix_owner_action(int(args[0]), int(args[1])))
    elif mode == "cleanup-stage" and len(args) == 1:
        cleanup_stage(args[0])
    elif mode == "publish-stage" and len(args) == 5:
        publish_stage(*args)
    else:
        fail("invalid publisher engine invocation")


try:
    main()
except PublishError as exc:
    print(f"gateway packager: {exc}", file=sys.stderr)
    raise SystemExit(1)
except (OSError, ValueError, json.JSONDecodeError) as exc:
    print(
        f"gateway packager: fail-closed operating-system error ({type(exc).__name__})",
        file=sys.stderr,
    )
    raise SystemExit(1)
PY
}

if [ "${1:-}" = "--selftest-validate-token" ]; then
  [ "$#" -eq 3 ] || exit 2
  validate_release_target "$2"
  validate_release_tag "$3"
  exit
fi

if [ "${1:-}" = "--selftest-package-platform-check" ]; then
  [ "$#" -eq 1 ] || exit 2
  selftest_python=""
  for candidate in python3 python; do
    if command -v "$candidate" >/dev/null 2>&1; then
      candidate_path="$(command -v "$candidate")"
      if "$candidate_path" -I -c \
        'import sys; raise SystemExit(0 if sys.version_info.major == 3 else 1)' \
        >/dev/null 2>&1; then
        selftest_python="$candidate_path"
        break
      fi
    fi
  done
  [ -n "$selftest_python" ] || exit 1
  publisher_python_engine "$selftest_python" platform-check
  exit
fi

if [ "${1:-}" = "--selftest-package-reserved-prefix-owner" ]; then
  [ "$#" -eq 3 ] || exit 2
  [ -x /usr/bin/python3 ] || exit 1
  publisher_python_engine /usr/bin/python3 reserved-owner-test "$2" "$3"
  exit
fi

if [ "${1:-}" = "--selftest-package-publisher-identity" ]; then
  [ "$#" -eq 1 ] || exit 2
  validate_publisher_identity
  exit
fi

case "${1:-}" in
  --selftest-package-create-stage)
    [ "$#" -eq 1 ] || exit 2
    [ -x /usr/bin/python3 ] || exit 1
    publisher_python_engine /usr/bin/python3 create-stage
    exit
    ;;
  --selftest-package-cleanup-stage)
    [ "$#" -eq 2 ] || exit 2
    [ -x /usr/bin/python3 ] || exit 1
    publisher_python_engine /usr/bin/python3 cleanup-stage "$2"
    exit
    ;;
  --selftest-package-publish)
    [ "$#" -eq 6 ] || exit 2
    [ -x /usr/bin/python3 ] || exit 1
    publisher_python_engine /usr/bin/python3 publish-stage \
      "$2" "$3" "$4" "$5" "$6"
    exit
    ;;
esac

validate_release_target "$TARGET"

[ "$(uname -s)" = "Linux" ] || {
  echo "secure Gateway release packaging requires Linux" >&2
  exit 1
}
[ -x /usr/bin/python3 ] || {
  echo "/usr/bin/python3 is required for secure release publication" >&2
  exit 1
}
validate_publisher_identity
python_bin=/usr/bin/python3

if [ -z "$TAG" ]; then
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    revision="$(git rev-parse --short HEAD)"
  else
    revision="nogit"
  fi
  TAG="${timestamp}-${revision}"
fi
validate_release_tag "$TAG"

if [ -n "$TARGET" ] && command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET"
fi

build_args=(+"$TOOLCHAIN" build --locked --release -p "$PACKAGE" --bin "$BIN" --bin "$ZONE_BIN")
if [ -n "$TARGET" ]; then
  build_args+=(--target "$TARGET")
  target_slug="$TARGET"
  binary_dir="$ROOT/target/$TARGET/release"
else
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os" in
    linux) os="linux" ;;
    *)
      echo "host packaging without MIR2_RELEASE_TARGET requires Linux" >&2
      exit 2
      ;;
  esac
  case "$arch" in
    x86_64|amd64) arch="x64" ;;
    aarch64|arm64) arch="arm64" ;;
    *)
      echo "host architecture is not allowlisted for Gateway release packaging" >&2
      exit 2
      ;;
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
zone_binary_path="$binary_dir/$ZONE_BIN"
if [ ! -x "$zone_binary_path" ]; then
  echo "missing release binary: $zone_binary_path" >&2
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
zone_binary_size_bytes="$(wc -c < "$zone_binary_path" | tr -d '[:space:]')"
zone_binary_sha256="$(sha256_file "$zone_binary_path")"
if [ "$size_bytes" -le 0 ] || [ "$size_bytes" -gt 268435456 ] ||
  [ "$zone_binary_size_bytes" -le 0 ] ||
  [ "$zone_binary_size_bytes" -gt 268435456 ]; then
  echo "release binaries exceed the fixed installer member-size contract" >&2
  exit 1
fi
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

stage_output="$(publisher_python_engine "$python_bin" create-stage)" || exit 1
mapfile -t stage_values <<< "$stage_output"
[ "${#stage_values[@]}" -eq 2 ] || {
  echo "secure publisher returned an invalid stage contract" >&2
  exit 1
}
stage="${stage_values[0]}"
stage_name="${stage_values[1]}"
cleanup_package_stage() {
  if [ -n "${stage_name:-}" ]; then
    publisher_python_engine "$python_bin" cleanup-stage "$stage_name"
    stage_name=""
  fi
}
trap 'cleanup_package_stage || echo "package stage cleanup failed" >&2' EXIT
package_signal_exit() {
  local status="$1"
  trap - EXIT HUP INT TERM
  cleanup_package_stage || echo "package stage cleanup failed" >&2
  exit "$status"
}
trap 'package_signal_exit 129' HUP
trap 'package_signal_exit 130' INT
trap 'package_signal_exit 143' TERM

/usr/bin/install -m 0755 "$binary_path" "$stage/$BIN"
/usr/bin/install -m 0755 "$zone_binary_path" "$stage/$ZONE_BIN"

env_template="$ROOT/infra/systemd/mir2-gateway.env.example"
if [ "$(grep -c '^MIR2_SAVE_RECOVERY_MAC_KEY=' "$env_template")" -ne 1 ] ||
  ! grep -Fxq \
    'MIR2_SAVE_RECOVERY_MAC_KEY=replace-with-stable-independent-64-hex-secret' \
    "$env_template"; then
  echo "refusing to package a Gateway env template containing recovery key material" >&2
  exit 1
fi

write_release_json() {
  local output_path="$1"
  local archive_name_value="${2:-}"
  local archive_sha_value="${3:-}"
  /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin LC_ALL=C \
    "$python_bin" -I - \
    "$output_path" "$TAG" "$target_slug" "$built_at" "$git_revision" \
    "$git_dirty" "$size_bytes" "$binary_sha256" \
    "$zone_binary_size_bytes" "$zone_binary_sha256" \
    "$archive_name_value" "$archive_sha_value" <<'PY'
import json
import os
import sys

(
    output_path,
    tag,
    target,
    built_at,
    git_revision,
    git_dirty_text,
    binary_size,
    binary_sha,
    zone_size,
    zone_sha,
    archive_name,
    archive_sha,
) = sys.argv[1:]

dirty_values = {"null": None, "true": True, "false": False}
if git_dirty_text not in dirty_values:
    raise SystemExit("invalid git dirty state")

metadata = {
    "name": "mir2-gateway",
    "tag": tag,
    "target": target,
    "builtAt": built_at,
    "gitRevision": git_revision,
    "gitDirty": dirty_values[git_dirty_text],
    "binarySizeBytes": int(binary_size),
    "binarySha256": binary_sha,
    "zoneHostBinarySizeBytes": int(zone_size),
    "zoneHostBinarySha256": zone_sha,
    "saveRecovery": {
        "keyIncluded": False,
        "sidecarsIncluded": False,
        "restoreRequirement": (
            "Restore the original MAC key and its recovery sidecars together."
        ),
    },
    "installation": {
        "requiresRootOwnedPinManifest": True,
        "archiveContainsInstaller": False,
        "archiveContainsSystemdUnit": False,
        "archiveContainsEnvironmentTemplate": False,
        "checksumSidecarIsAuthority": False,
        "rootPinRehashFromArchiveFdRequired": True,
        "publisherUidTrustBoundaryRequired": True,
        "rootExtractionAllowed": False,
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
if archive_name:
    metadata["archive"] = archive_name
    metadata["archiveSha256"] = archive_sha

file_fd = os.open(
    output_path,
    os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
    0o600,
)
try:
    os.fchmod(file_fd, 0o644)
    with os.fdopen(file_fd, "w", encoding="utf-8", newline="\n") as output:
        file_fd = -1
        json.dump(metadata, output, ensure_ascii=True, indent=2)
        output.write("\n")
        output.flush()
        os.fsync(output.fileno())
finally:
    if file_fd >= 0:
        os.close(file_fd)
PY
}

write_release_json "$stage/RELEASE.json"

cat > "$stage/README.txt" <<'TXT'
Mir2 Gateway release package.

The Gateway and authoritative Zone Host are built from the same revision and
must be deployed together when account or simulation behavior changes.

Threat boundary:
  The release URL and archive are untrusted transport. Authenticity comes only
  from /etc/mir2/gateway-release.pin, provisioned root:root mode 0600 through a
  separately authenticated configuration-management channel. Caller
  environment URL/SHA/tag values, the adjacent .sha256 file, RELEASE.json, and
  files fetched from the release URL are never an authenticity root.
  Configuration management must derive the exact pin digest from an
  independently authenticated artifact-registry API/channel, never by copying
  the publisher .sha256 sidecar. The root installer hashes the downloaded
  archive again from its already-open FD and compares that digest to the pin;
  it never reads the sidecar.

  Packaging must run as a dedicated non-root publisher and requires
  MIR2_RELEASE_PUBLISHER_UID to exactly equal that effective UID. That UID,
  every process able to act as it, and its source/build tree are the publisher
  trust boundary. Do not package from a shared or attacker-writable checkout.

  This archive contains exactly four non-link ustar members: mir2-gateway,
  zone_host, RELEASE.json, and README.txt. It intentionally contains no
  installer/helper, systemd unit, environment template, recovery key, or
  recovery sidecar. Archive bytes are never executed as installer code. The
  fixed trusted installer generates the exact systemd unit itself.

  Packaging uses a publisher-private mode-0700 stage and a mode-0700 output
  directory whose complete ancestry, owner, mode, nlink, and held directory FD
  are verified. Source members stay open while ustar bytes are produced; the
  archive digest is calculated from that same archive FD. The sidecar is
  published no-replace first and is tentative until the archive is published
  no-replace as the transaction commit record. A boot/process-bound marker and
  bounded next-run sweep remove a verified orphan sidecar or finish cleanup
  after SIGKILL; unknown, linked, over-age live, or over-limit residue fails
  closed. Thus a crash exposes either no committed archive or an archive plus
  its exact matching sidecar, never a different archive blessed by that hash.

Mandatory out-of-band bootstrap:
  Provision these independently of the archive and release URL:
    /usr/local/libexec/mir2/install-gateway-release.sh
      root:root mode 0755, nlink=1
    /usr/local/share/mir2/gateway/mir2-gateway.env.example
      root:root mode 0644, nlink=1
    /etc/mir2/gateway-release.pin
      root:root mode 0600, nlink=1

  Every ancestor must be a real root-owned directory that is not writable by
  group/other. Invoke the fixed installer through its #!/bin/bash -p shebang
  from a sanitized root execution context. If this bootstrap cannot be proved,
  installation fails closed; there is no workspace-script or archive-helper
  fallback.

Install layout:
  root-owned:    /opt/mir2/gateway/releases/<tag>/{mir2-gateway,zone_host}
  root-owned:    /opt/mir2/gateway/current -> releases/<tag>
  service-owned: /var/lib/mir2/gateway-data
  root-owned:    /var/lib/mir2/save-recovery and its v1 namespace
  service-owned: /var/lib/mir2/save-recovery/v1/gateway (mode 0700)
  service-owned: /var/log/mir2
  root-owned:    /etc/mir2/gateway.env (mode 0600)

Installer runtime assumptions:
  Linux/systemd, /bin/bash with privileged mode, /usr/bin/python3 with dir_fd,
  O_NOFOLLOW/fchown/fchmod/fsync, RLIMIT_FSIZE, renameat2(RENAME_NOREPLACE),
  prctl(PR_SET_PDEATHSIG), a mounted readable procfs boot_id and process stat,
  /usr/bin/curl, GNU getent/id/user-management tools, local passwd/group/shadow/
  gshadow databases, files-only passwd/group/shadow/gshadow NSS configured in
  root-owned /etc/nsswitch.conf, and login.defs with all SYS_UID/GID bounds.
  The systemd manager must support ProtectProc/ProcSubset,
  namespace/device/kernel
  protections, RestrictAddressFamilies, and @system-service syscall groups.
  Filesystems containing /tmp, /var/tmp, /opt, /etc, and the artifact OUT_DIR
  must provide O_NOFOLLOW dirfd operations, directory fsync, and same-filesystem
  atomic renameat2. Curl runs with -q, never follows
  redirects, accepts a pinned HTTPS URL without query/fragment/credentials, and
  writes its body to an already-open root-private FD under a kernel-enforced
  file-size limit. The installer bounds compressed bytes, expanded bytes,
  member count, header/name forms, and individual member sizes before copying
  from the same authenticated archive FD into O_EXCL destinations. A completed
  release is published from an unreferenced temporary directory with atomic
  no-replace rename; failed temporary releases are safely removed and retryable.
  Download and release/config transactions carry root-owned markers tied to the
  initiating process start time and Linux boot ID. Every startup performs a
  count/age/byte-bounded dirfd sweep; dead-owner residues are removed through
  the already-open parent and unknown entries are never traversed or deleted.
  First creation of gateway.env and the fixed unit uses a fully written,
  validated, fsynced private payload followed by no-replace publication and a
  parent-directory fsync. Existing or truncated files are never overwritten or
  silently accepted.
  --activate additionally requires independently strong authenticated
  Postgres and Redis URLs; template/public/default credentials and
  unauthenticated Redis are rejected without logging credential material.

Ownership migration:
  Existing service-owned /opt/mir2/gateway or /var/lib/mir2 roots are refused.
  Migrate legacy ownership manually: release/data/recovery namespaces remain
  root-owned and non-writable by group/other; only gateway-data, the final
  recovery instance leaf, and the log directory belong to the service account.
  An existing mir2 identity is accepted only when root-owned local passwd,
  group, shadow, and gshadow records exactly agree with full NSS/getent output;
  all four identity NSS databases must be configured as exactly files-only so
  enumeration proves numeric uniqueness (non-enumerating remote backends are
  unsupported and fail closed); login.defs supplies the supported system
  UID/GID ranges; password and group
  markers are explicit and both shadow databases are locked; UID/GID values are
  unique; the mir2 group has no members/admins; the home and nologin/false shell
  are fixed; and id(1) reports only the mir2 primary group. Remote, duplicate,
  interactive, shared-ID, or reused identities fail closed before directory
  ownership is created.

Save-recovery backup and restore:
  MIR2_SAVE_RECOVERY_MAC_KEY is generated once during the first install and is
  not included in this package. Recovery sidecars under MIR2_SAVE_RECOVERY_DIR
  are also not included. Back up the original key and all sidecars as one
  protected recovery set, and restore both together before Gateway startup.
  Rotating or losing the key makes retained sidecars unusable; restoring only
  the key or only the sidecars is insufficient.

Linux security gate:
  CI must run scripts/selftest-gateway-save-recovery.sh on a non-root Linux
  worker with MIR2_REQUIRE_LINUX_SECURITY_GATE=1. MINGW results do not validate
  Linux RLIMIT_FSIZE streaming, atomic publication, dirfd, owner/mode/nlink,
  symlink-race, or same-FD copy behavior.
TXT

archive_name="mir2-gateway-${target_slug}-${TAG}.tar.gz"
publish_output="$(
  publisher_python_engine "$python_bin" publish-stage \
    "$stage" "$OUT_DIR" "$archive_name" "-" "none"
)" || exit 1
mapfile -t publish_values <<< "$publish_output"
[ "${#publish_values[@]}" -eq 5 ] || {
  echo "secure publisher returned an invalid publication contract" >&2
  exit 1
}
archive_path="${publish_values[0]}"
archive_size_bytes="${publish_values[1]}"
archive_sha256="${publish_values[2]}"
binary_sha256="${publish_values[3]}"
zone_binary_sha256="${publish_values[4]}"

cleanup_package_stage
trap - EXIT HUP INT TERM

echo "Gateway release package written:"
echo "  archive: $archive_path"
echo "  archive bytes: $archive_size_bytes"
echo "  archive sha256: $archive_sha256"
echo "  binary bytes: $size_bytes"
echo "  binary sha256: $binary_sha256"
echo "  zone host bytes: $zone_binary_size_bytes"
echo "  zone host sha256: $zone_binary_sha256"
