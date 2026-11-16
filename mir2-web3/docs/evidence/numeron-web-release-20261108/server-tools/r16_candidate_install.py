#!/usr/bin/env python3
"""One pinned TEST/QA candidate. No network, service, environment or database writes."""
import argparse
import ctypes
import datetime
import errno
import gzip
import hashlib
import json
import os
import re
import secrets
import signal
import stat
import struct
import sys
import zipfile
from dataclasses import dataclass, fields


class SafetyError(Exception):
    pass


def require(condition, code):
    if not condition:
        raise SafetyError(code)


@dataclass(frozen=True)
class Contract:
    source: str
    tag: str
    artifact_size: int
    artifact_sha: str
    gateway_tar_size: int
    gateway_tar_sha: str
    maps_tar_size: int
    maps_tar_sha: str
    gateway_size: int
    gateway_sha: str
    zone_size: int
    zone_sha: str
    map_count: int
    map_bytes: int
    latest_size: int
    manifest_size: int
    verification_size: int

    @property
    def gateway_archive(self):
        return f"mir2-gateway-linux-x64-{self.tag}.tar.gz"

    @property
    def maps_archive(self):
        return f"mir2-crystal-map-pack-{self.source}.tar.gz"

    @property
    def candidate(self):
        return f"{self.source}-r16-qa"


# No candidate/default/environment pin. Root supplies independent authority.
RELEASE_BASE = "/opt/mir2-playtest/releases"
MAX_FILE = 128 * 1024 * 1024
CHUNK = 1024 * 1024
HEX = re.compile(r"^[0-9a-f]{64}$")
COMPONENT = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,199}$")
READ_FLAGS = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_CLOEXEC", 0)
DIR_FLAGS = READ_FLAGS | getattr(os, "O_DIRECTORY", 0)
CREATE_FLAGS = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)


def relative(path):
    require(isinstance(path, str) and path.isascii() and not path.startswith("/"), "unsafe_path")
    parts = path.split("/")
    require(all(COMPONENT.fullmatch(part) and part not in (".", "..") for part in parts), "unsafe_path")
    return parts


def stable(st):
    return st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns, st.st_ctime_ns, st.st_nlink, st.st_uid, st.st_gid, stat.S_IMODE(st.st_mode)


def regular(fd, size=None, uid=None, mode=None):
    st = os.fstat(fd)
    require(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and 0 <= st.st_size <= MAX_FILE, "file_identity")
    require(size is None or st.st_size == size, "file_size")
    require(uid is None or st.st_uid == uid, "file_owner")
    require(mode is None or stat.S_IMODE(st.st_mode) == mode, "file_mode")
    return st


def write_all(fd, data):
    while data:
        written = os.write(fd, data)
        require(written > 0, "short_write")
        data = data[written:]


def digest_fd(fd, size, expected_sha, output_fd=None, uid=None, mode=None):
    before = regular(fd, size, uid, mode)
    os.lseek(fd, 0, os.SEEK_SET)
    digest, total = hashlib.sha256(), 0
    while True:
        data = os.read(fd, min(CHUNK, size - total + 1))
        if not data:
            break
        total += len(data)
        require(total <= size, "file_grew")
        digest.update(data)
        if output_fd is not None:
            write_all(output_fd, data)
    require(total == size and digest.hexdigest() == expected_sha, "file_digest")
    require(stable(before) == stable(regular(fd, size, uid, mode)), "file_changed")
    os.lseek(fd, 0, os.SEEK_SET)
    return before


def open_chain(path, root_owned=False, create=False):
    require(isinstance(path, str) and path.startswith("/") and path != "/", "absolute_path_required")
    parts = relative(path[1:])
    current = os.open("/", DIR_FLAGS)
    try:
        if root_owned:
            initial = os.fstat(current)
            require(initial.st_uid == initial.st_gid == 0 and not stat.S_IMODE(initial.st_mode) & 0o022, "directory_trust")
        for part in parts:
            created = False
            if create:
                try:
                    os.mkdir(part, 0o755, dir_fd=current)
                    created = True
                except FileExistsError:
                    pass
            next_fd = os.open(part, DIR_FLAGS, dir_fd=current)
            try:
                st = os.fstat(next_fd)
                require(stat.S_ISDIR(st.st_mode), "directory_identity")
                if created:
                    require(st.st_uid == os.geteuid(), "created_directory_owner")
                    os.fchmod(next_fd, 0o755)
                    os.fsync(next_fd); os.fsync(current)
                    st = os.fstat(next_fd)
                if root_owned:
                    require(st.st_uid == 0 and st.st_gid == 0 and not stat.S_IMODE(st.st_mode) & 0o022, "directory_trust")
            except BaseException:
                os.close(next_fd)
                raise
            os.close(current)
            current = next_fd
        return current
    except BaseException:
        os.close(current)
        raise


def open_absolute_file(path, root_owned=False):
    parent, name = os.path.split(path)
    require(bool(name), "file_path_required")
    relative(name)
    parent_fd = open_chain(parent, root_owned=root_owned)
    try:
        return os.open(name, READ_FLAGS, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)


def child_dir(parent_fd, name, uid, mode=0o700, create=False):
    require(len(relative(name)) == 1, "directory_component")
    if create:
        os.mkdir(name, mode, dir_fd=parent_fd)
    fd = os.open(name, DIR_FLAGS, dir_fd=parent_fd)
    try:
        st = os.fstat(fd)
        require(stat.S_ISDIR(st.st_mode) and st.st_uid == uid and stat.S_IMODE(st.st_mode) == mode, "stage_directory_identity")
        return fd
    except BaseException:
        os.close(fd)
        raise


def bounded_names(fd, maximum):
    names = set()
    with os.scandir(fd) as entries:
        for entry in entries:
            require(len(names) < maximum and entry.name not in names, "directory_count")
            relative(entry.name)
            names.add(entry.name)
    return names


def zip_inventory(bundle, contract):
    required = {
        f"gateway-releases/{contract.gateway_archive}": contract.gateway_tar_size,
        f"playtest-wallet-release/{contract.maps_archive}": contract.maps_tar_size,
        "gateway-releases/latest-mir2-gateway-release.json": contract.latest_size,
        "playtest-wallet-release/crystal-map-pack-manifest.json": contract.manifest_size,
        "playtest-wallet-release/playtest-release-verification.json": contract.verification_size,
    }
    optional = {f"gateway-releases/{contract.gateway_archive}.sha256", f"playtest-wallet-release/{contract.maps_archive}.sha256"}
    seen, extents = {}, []
    require(not bundle.comment, "zip_comment")
    infos = bundle.infolist()
    require(len(infos) <= 9, "zip_member_count")
    for info in infos:
        require(info.filename == info.orig_filename and info.filename not in seen, "zip_name_duplicate")
        name = info.filename
        mode = info.external_attr >> 16
        require(not info.flag_bits & 1 and info.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED), "zip_encoding")
        require(not info.comment and len(info.extra) <= 4096 and info.compress_size <= contract.artifact_size, "zip_bounds")
        if info.is_dir():
            require(name in {"gateway-releases/", "playtest-wallet-release/"} and info.file_size == 0 and stat.S_IFMT(mode) in (0, stat.S_IFDIR), "zip_directory")
        else:
            relative(name)
            require(stat.S_IFMT(mode) in (0, stat.S_IFREG), "zip_special_member")
            require(name in required or name in optional, "zip_unexpected_member")
            require(info.file_size == required[name] if name in required else 0 < info.file_size <= 256, "zip_member_size")
        # Bound physical compressed extents and independently check local names.
        bundle.fp.seek(info.header_offset)
        header = bundle.fp.read(30)
        require(len(header) == 30 and header[:4] == b"PK\x03\x04", "zip_local_header")
        name_len, extra_len = struct.unpack_from("<HH", header, 26)
        require(extra_len <= 4096 and bundle.fp.read(name_len) == name.encode("ascii"), "zip_local_name")
        end = info.header_offset + 30 + name_len + extra_len + info.compress_size
        require(0 <= info.header_offset < end <= bundle.start_dir <= contract.artifact_size, "zip_physical_bounds")
        extents.append((info.header_offset, end))
        seen[name] = info
    require(set(required) <= set(seen), "zip_missing_member")
    extents.sort()
    require(extents and extents[0][0] == 0 and all(a[1] <= b[0] for a, b in zip(extents, extents[1:])), "zip_overlap")
    return seen


def bounded_zip(raw, contract):
    # Bound the central directory before ZipFile materializes its entries.
    raw.seek(-22, os.SEEK_END)
    ending = raw.read(22)
    require(len(ending) == 22 and ending[:4] == b"PK\x05\x06", "zip_ending")
    _, disk, start_disk, disk_count, count, size, offset, comment = struct.unpack("<4s4H2LH", ending)
    require(disk == start_disk == comment == 0 and 0 < count == disk_count <= 9
            and size <= 65536 and 0 <= offset < offset + size <= contract.artifact_size - 22, "zip_directory_bounds")
    raw.seek(0)
    return zipfile.ZipFile(raw)


def strict_json(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "json_duplicate_key")
            result[key] = value
        return result
    try:
        value = json.loads(data.decode("utf-8"), object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(SafetyError("json_nonfinite")))
    except (ValueError, UnicodeError, RecursionError):
        raise SafetyError("json_invalid") from None
    require(isinstance(value, dict), "json_object_required")
    return value


def validate_contract(value):
    require(isinstance(value, dict) and set(value) == {field.name for field in fields(Contract)}, "pin_fields")
    require(isinstance(value["source"], str) and re.fullmatch(r"[0-9a-f]{40}", value["source"]), "pin_source")
    require(isinstance(value["tag"], str) and COMPONENT.fullmatch(value["tag"]), "pin_tag")
    for key, item in value.items():
        if key.endswith("_sha"):
            require(isinstance(item, str) and HEX.fullmatch(item), "pin_sha")
        elif key not in {"source", "tag"}:
            require(type(item) is int and 0 < item <= MAX_FILE, "pin_number")
    require(value["map_count"] <= 2000 and value["latest_size"] <= 65536
            and value["manifest_size"] <= 1048576 and value["verification_size"] <= 65536, "pin_bounds")
    return Contract(**value)


def load_pin(path, publishing, stage_uid):
    fd = open_absolute_file(path, root_owned=publishing)
    try:
        before = regular(fd)
        require(0 < before.st_size <= 65536, "pin_size")
        mode = stat.S_IMODE(before.st_mode)
        require((before.st_uid == before.st_gid == 0 and mode == 0o600) if publishing
                else before.st_uid in (0, stage_uid) and not mode & 0o022, "pin_file_trust")
        data = os.read(fd, before.st_size + 1)
        require(len(data) == before.st_size and stable(before) == stable(os.fstat(fd)), "pin_changed")
        wrapper = strict_json(data)
        require(set(wrapper) == {"schemaVersion", "kind", "contract"} and type(wrapper["schemaVersion"]) is int and wrapper["schemaVersion"] == 1
                and wrapper["kind"] == "r16-qa-out-of-band-artifact-pin", "pin_schema")
        return validate_contract(wrapper["contract"]), hashlib.sha256(data).hexdigest()
    finally:
        os.close(fd)


def authenticated_metadata(bundle, inventory, contract):
    specs = {
        "latest-mir2-gateway-release.json": ("gateway-releases/", contract.latest_size),
        "crystal-map-pack-manifest.json": ("playtest-wallet-release/", contract.manifest_size),
        "playtest-release-verification.json": ("playtest-wallet-release/", contract.verification_size),
    }
    raw = {}
    for name, (prefix, size) in specs.items():
        with bundle.open(inventory[prefix + name]) as source:
            data = source.read(size + 1)
            require(len(data) == size and source.read(1) == b"", "metadata_size")
            raw[name] = data
    latest, maps, verification = (strict_json(raw[name]) for name in specs)
    require(latest.get("gitRevision") == contract.source and latest.get("gitDirty") is False and latest.get("tag") == contract.tag
            and latest.get("name") == "mir2-gateway" and latest.get("target") == "linux-x64"
            and latest.get("archive") == contract.gateway_archive and latest.get("archiveSha256") == contract.gateway_tar_sha,
            "release_metadata")
    for field, expected in [("binarySizeBytes", contract.gateway_size), ("binarySha256", contract.gateway_sha),
                            ("zoneHostBinarySizeBytes", contract.zone_size), ("zoneHostBinarySha256", contract.zone_sha)]:
        require(latest.get(field) == expected, "binary_metadata")
    require(maps.get("sourceRevision") == contract.source and maps.get("sourceTree") == f"{contract.source}:mir2-web3/apps/web/lib/generated/crystal-map-pack"
            and maps.get("archive") == contract.maps_archive and maps.get("archiveSha256") == contract.maps_tar_sha
            and maps.get("archiveBytes") == contract.maps_tar_size and maps.get("fileCount") == contract.map_count
            and maps.get("totalFileBytes") == contract.map_bytes, "maps_metadata")
    rows = maps.get("files")
    require(isinstance(rows, list) and len(rows) == contract.map_count, "maps_count")
    pins, last, total = {}, "", 0
    for row in rows:
        require(isinstance(row, dict) and set(row) == {"path", "bytes", "sha256"}, "map_row")
        parts = relative(row["path"])
        require(len(parts) == 2 and parts[0] == "crystal-map-pack" and parts[1].endswith(".map.gz") and row["path"] > last, "map_path_order")
        require(type(row["bytes"]) is int and 0 < row["bytes"] <= contract.map_bytes
                and isinstance(row["sha256"], str) and HEX.fullmatch(row["sha256"]), "map_pin")
        pins[row["path"]] = (row["bytes"], row["sha256"], 0o644)
        last = row["path"]
        total += row["bytes"]
    require(total == contract.map_bytes, "map_bytes")
    phases = verification.get("passed")
    require(verification.get("sourceRevision") == contract.source and verification.get("releaseTag") == contract.tag
            and verification.get("rustToolchain") == "1.95.0" and verification.get("buildEnvironment") == "native-linux-non-root"
            and verification.get("containersStarted") is False and verification.get("productionPublish") is False
            and verification.get("mapManifest") == "crystal-map-pack-manifest.json" and isinstance(phases, list)
            and all(isinstance(value, str) for value in phases) and len(phases) == len(set(phases))
            and {"schema_union", "monthly_card_postgres", "billing_postgres", "activities_postgres", "linux-release-security", "gateway-package", "gateway-postgres-startup"} <= set(phases), "verification_metadata")
    return raw, pins


class HashedReader:
    def __init__(self, source, size, sha):
        self.source, self.size, self.sha = source, size, sha
        self.total, self.digest = 0, hashlib.sha256()

    def read(self, count=-1):
        count = self.size - self.total + 1 if count < 0 else min(count, self.size - self.total + 1)
        data = self.source.read(count)
        self.total += len(data)
        require(self.total <= self.size, "archive_grew")
        self.digest.update(data)
        return data

    def finish(self):
        while self.read(CHUNK):
            pass
        require(self.total == self.size and self.digest.hexdigest() == self.sha, "archive_digest")


def tar_stream(source, compressed_size, compressed_sha, pins, emit, directory=None, captures=()):
    """Only the non-root stage calls this. No tarfile/extractall, PAX or links."""
    reader = HashedReader(source, compressed_size, compressed_sha)
    maximum = sum(row[0] if row[0] is not None else 65536 for row in pins.values()) + len(pins) * 1024 + 20480
    count, total, seen, captured = 0, 0, set(), {}
    def exact(stream, size):
        nonlocal total
        chunks, remaining = [], size
        while remaining:
            part = stream.read(min(remaining, CHUNK))
            require(bool(part), "tar_truncated")
            remaining -= len(part); total += len(part)
            require(total <= maximum, "tar_expansion")
            chunks.append(part)
        return b"".join(chunks)
    def octal(raw):
        value = raw.rstrip(b"\x00 ").lstrip(b" ")
        require(bool(value) and all(c in b"01234567" for c in value), "tar_octal")
        return int(value, 8)
    def field(raw):
        value, _, tail = raw.partition(b"\0")
        require(not any(tail), "tar_string_padding")
        try:
            return value.decode("ascii")
        except UnicodeError:
            raise SafetyError("tar_name_encoding") from None
    with gzip.GzipFile(fileobj=reader, mode="rb") as stream:
        while True:
            header = exact(stream, 512)
            if header == bytes(512):
                require(exact(stream, 512) == bytes(512), "tar_terminator")
                while True:
                    padding = stream.read(8192)
                    if not padding:
                        break
                    total += len(padding)
                    require(total <= maximum and not any(padding), "tar_trailer")
                break
            count += 1
            require(count <= len(pins) + bool(directory), "tar_member_count")
            require(header[257:263] in (b"ustar\0", b"ustar "), "tar_format")
            require(octal(header[148:156]) == sum(header[:148]) + 8 * 32 + sum(header[156:]), "tar_checksum")
            name = field(header[:100]); prefix = field(header[345:500])
            if prefix:
                name = prefix + "/" + name
            kind, size = header[156:157], octal(header[124:136])
            require(not field(header[157:257]), "tar_link")
            if kind == b"5":
                require(directory and name in (directory, directory + "/") and size == 0 and directory not in seen, "tar_directory")
                seen.add(directory)
                continue
            require(kind in (b"0", b"\0") and name in pins and name not in seen, "tar_member")
            relative(name)
            expected_size, expected_sha, _ = pins[name]
            require(size == expected_size if expected_size is not None else 0 < size <= 65536, "tar_size")
            digest, capture = hashlib.sha256(), bytearray()
            with emit(name, size) as sink:
                remaining = size
                while remaining:
                    data = exact(stream, min(remaining, CHUNK)); remaining -= len(data)
                    digest.update(data); sink.write(data)
                    if name in captures:
                        capture.extend(data)
            require(expected_sha is None or digest.hexdigest() == expected_sha, "tar_file_digest")
            padding = exact(stream, (-size) % 512)
            require(not any(padding), "tar_padding")
            if name in captures:
                captured[name] = bytes(capture)
            seen.add(name)
    reader.finish()
    require(seen == set(pins) | ({directory} if directory else set()), "tar_missing_member")
    return captured


class FileSink:
    def __init__(self, directory_fd, name):
        self.fd = os.open(name, CREATE_FLAGS, 0o600, dir_fd=directory_fd)
        os.fchmod(self.fd, 0o600)

    def __enter__(self):
        return self

    def write(self, data):
        write_all(self.fd, data)

    def __exit__(self, *_):
        try:
            os.fsync(self.fd)
            regular(self.fd, uid=os.geteuid(), mode=0o600)
        finally:
            os.close(self.fd)


def write_new(directory_fd, name, data):
    with FileSink(directory_fd, name) as sink:
        sink.write(data)


def metadata_paths():
    return ("latest-mir2-gateway-release.json", "crystal-map-pack-manifest.json", "playtest-release-verification.json")


def gateway_pins(contract):
    return {"mir2-gateway": (contract.gateway_size, contract.gateway_sha, 0o755), "zone_host": (contract.zone_size, contract.zone_sha, 0o755)}


def stage(artifact, output, uid, contract):
    archive_fd = open_absolute_file(artifact)
    root_fd = None
    try:
        artifact_stat = regular(archive_fd, contract.artifact_size)
        require(artifact_stat.st_uid in (0, uid) and not stat.S_IMODE(artifact_stat.st_mode) & 0o022, "artifact_owner_mode")
        stamp = digest_fd(archive_fd, contract.artifact_size, contract.artifact_sha)
        with os.fdopen(os.dup(archive_fd), "rb") as raw, bounded_zip(raw, contract) as bundle:
            inventory = zip_inventory(bundle, contract)
            metadata, maps = authenticated_metadata(bundle, inventory, contract)
            parent, name = os.path.split(output); relative(name)
            parent_fd = open_chain(parent)
            try:
                root_fd = child_dir(parent_fd, name, uid, create=True)
            finally:
                os.close(parent_fd)
            gateway_fd = child_dir(root_fd, "gateway", uid, create=True)
            maps_fd = child_dir(root_fd, "crystal-map-pack", uid, create=True)
            meta_fd = child_dir(root_fd, "metadata", uid, create=True)
            try:
                for filename, data in metadata.items():
                    write_new(meta_fd, filename, data)
                pins = {**gateway_pins(contract), "RELEASE.json": (None, None, 0o600), "README.txt": (None, None, 0o600)}
                with bundle.open(inventory[f"gateway-releases/{contract.gateway_archive}"]) as member:
                    captured = tar_stream(member, contract.gateway_tar_size, contract.gateway_tar_sha, pins,
                                          lambda filename, _: FileSink(gateway_fd, filename), captures=("RELEASE.json",))
                inner = strict_json(captured["RELEASE.json"])
                require(inner.get("gitRevision") == contract.source and inner.get("tag") == contract.tag
                        and inner.get("binarySha256") == contract.gateway_sha and inner.get("zoneHostBinarySha256") == contract.zone_sha, "inner_release_metadata")
                with bundle.open(inventory[f"playtest-wallet-release/{contract.maps_archive}"]) as member:
                    tar_stream(member, contract.maps_tar_size, contract.maps_tar_sha, maps,
                               lambda filename, _: FileSink(maps_fd, relative(filename)[1]), directory="crystal-map-pack")
                digest_fd(archive_fd, contract.artifact_size, contract.artifact_sha)
                require(stable(stamp) == stable(os.fstat(archive_fd)), "artifact_changed")
                receipt = {"schemaVersion": 1, "kind": "r16-qa-unprivileged-stage", "passed": True,
                           "sourceSha": contract.source, "artifactSha256": contract.artifact_sha,
                           "gatewayFiles": 4, "mapFiles": len(maps), "mapFileBytes": contract.map_bytes,
                           "mapHashSemantics": "exact .map.gz file bytes, not decompressed map bytes", "rootAuthority": False}
                write_new(root_fd, "stage-receipt.json", (json.dumps(receipt, indent=2) + "\n").encode())
                for fd in (gateway_fd, maps_fd, meta_fd, root_fd):
                    os.fsync(fd)
                return receipt
            finally:
                for fd in (gateway_fd, maps_fd, meta_fd):
                    os.close(fd)
    finally:
        if root_fd is not None:
            os.close(root_fd)
        os.close(archive_fd)


def copy_verified(source_dir, source_name, destination_dir, name, pin, source_uid, destination_uid):
    size, sha, mode = pin
    relative(source_name); relative(name)
    require("/" not in source_name and "/" not in name, "copy_component")
    source_fd = os.open(source_name, READ_FLAGS, dir_fd=source_dir)
    try:
        before = regular(source_fd, size, source_uid, 0o600)
        destination_fd = os.open(name, CREATE_FLAGS, 0o600, dir_fd=destination_dir)
        try:
            require(os.fstat(destination_fd).st_uid == destination_uid, "destination_owner")
            digest_fd(source_fd, size, sha, destination_fd, source_uid, 0o600)
            now = os.stat(source_name, dir_fd=source_dir, follow_symlinks=False)
            require(stable(before) == stable(now), "source_path_changed")
            os.fchmod(destination_fd, mode); os.fsync(destination_fd)
            regular(destination_fd, size, destination_uid, mode)
        finally:
            os.close(destination_fd)
    finally:
        os.close(source_fd)
    fd = os.open(name, READ_FLAGS, dir_fd=destination_dir)
    try:
        digest_fd(fd, size, sha, uid=destination_uid, mode=mode)
    finally:
        os.close(fd)


def rename_noreplace(parent_fd, source_parent_fd, source_name, name):
    libc = ctypes.CDLL(None, use_errno=True)
    function = getattr(libc, "renameat2", None)
    require(function is not None, "rename_noreplace_unavailable")
    function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    function.restype = ctypes.c_int
    result = function(source_parent_fd, os.fsencode(source_name), parent_fd, os.fsencode(name), 1)
    if result:
        raise SafetyError("release_exists" if ctypes.get_errno() == errno.EEXIST else "rename_noreplace_failed")
    try:
        os.fsync(parent_fd)
    except OSError:
        raise SafetyError("publication_durability_unknown") from None


def publish(artifact, stage_path, stage_uid, contract, authority_pin_sha):
    # CLI checks root/trusted helper first. Root does not invoke tar_stream.
    base_fd = open_chain(RELEASE_BASE, root_owned=True, create=True)
    try:
        os.stat(contract.candidate, dir_fd=base_fd, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        os.close(base_fd)
        raise SafetyError("release_exists")
    transaction = "incoming-r16-" + secrets.token_hex(12)
    os.mkdir(transaction, 0o700, dir_fd=base_fd)
    transaction_fd = child_dir(base_fd, transaction, 0)
    handles = []
    try:
        artifact_fd = open_absolute_file(artifact); handles.append(artifact_fd)
        source_stat = regular(artifact_fd, contract.artifact_size)
        require(source_stat.st_uid in (0, stage_uid) and not stat.S_IMODE(source_stat.st_mode) & 0o022, "artifact_owner_mode")
        snapshot_fd = os.open("artifact.auth.zip", os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=transaction_fd); handles.append(snapshot_fd)
        digest_fd(artifact_fd, contract.artifact_size, contract.artifact_sha, snapshot_fd)
        os.fsync(snapshot_fd)
        digest_fd(snapshot_fd, contract.artifact_size, contract.artifact_sha, uid=0, mode=0o600)
        with os.fdopen(os.dup(snapshot_fd), "rb") as raw, bounded_zip(raw, contract) as bundle:
            inventory = zip_inventory(bundle, contract)
            metadata, maps = authenticated_metadata(bundle, inventory, contract)
        digest_fd(snapshot_fd, contract.artifact_size, contract.artifact_sha, uid=0, mode=0o600)
        stage_fd = open_chain(stage_path); handles.append(stage_fd)
        st = os.fstat(stage_fd)
        require(st.st_uid == stage_uid and stat.S_IMODE(st.st_mode) == 0o700, "stage_root_identity")
        require(bounded_names(stage_fd, 4) == {"gateway", "crystal-map-pack", "metadata", "stage-receipt.json"}, "stage_root_members")
        gateway_fd = child_dir(stage_fd, "gateway", stage_uid); handles.append(gateway_fd)
        maps_fd = child_dir(stage_fd, "crystal-map-pack", stage_uid); handles.append(maps_fd)
        meta_fd = child_dir(stage_fd, "metadata", stage_uid); handles.append(meta_fd)
        require(bounded_names(gateway_fd, 4) == {"mir2-gateway", "zone_host", "RELEASE.json", "README.txt"}, "stage_gateway_members")
        require(bounded_names(maps_fd, contract.map_count) == {relative(path)[1] for path in maps}, "stage_map_members")
        require(bounded_names(meta_fd, 3) == set(metadata_paths()), "stage_metadata_members")
        # These untrusted, unpublished files are checked as objects, not read as authority.
        for parent, name in ((gateway_fd, "RELEASE.json"), (gateway_fd, "README.txt"), (stage_fd, "stage-receipt.json")):
            fd = os.open(name, READ_FLAGS, dir_fd=parent)
            try:
                require(regular(fd, uid=stage_uid, mode=0o600).st_size <= 65536, "stage_unpublished_size")
            finally:
                os.close(fd)
        payload_fd = child_dir(transaction_fd, "payload", 0, create=True); handles.append(payload_fd)
        out_maps = child_dir(payload_fd, "crystal-map-pack", 0, create=True); handles.append(out_maps)
        out_meta = child_dir(payload_fd, "metadata", 0, create=True); handles.append(out_meta)
        published = []
        for name, pin in gateway_pins(contract).items():
            copy_verified(gateway_fd, name, payload_fd, name, pin, stage_uid, 0)
            published.append({"path": name, "bytes": pin[0], "sha256": pin[1]})
        for path, pin in maps.items():
            name = relative(path)[1]
            copy_verified(maps_fd, name, out_maps, name, pin, stage_uid, 0)
            published.append({"path": path, "bytes": pin[0], "sha256": pin[1]})
        for name, data in metadata.items():
            pin = len(data), hashlib.sha256(data).hexdigest(), 0o644
            copy_verified(meta_fd, name, out_meta, name, pin, stage_uid, 0)
            published.append({"path": "metadata/" + name, "bytes": pin[0], "sha256": pin[1]})
        receipt = {"schemaVersion": 1, "kind": "r16-qa-root-authenticated-publication", "passed": True,
                   "sourceSha": contract.source, "candidate": contract.candidate, "artifactSha256": contract.artifact_sha,
                   "authorityPinFileSha256": authority_pin_sha,
                   "gatewayArchiveSha256": contract.gateway_tar_sha, "mapsArchiveSha256": contract.maps_tar_sha,
                   "createdAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   "stageReceiptTrusted": False, "rootExtractedTar": False, "servicesChanged": False,
                   "mapHashSemantics": "exact .map.gz file bytes, not decompressed map bytes", "files": sorted(published, key=lambda row: row["path"])}
        write_new(payload_fd, "provenance.json", (json.dumps(receipt, indent=2) + "\n").encode())
        # No archive or unprivileged receipt/inner README/RELEASE enters payload.
        for fd in (out_maps, out_meta, payload_fd):
            os.fchmod(fd, 0o755); os.fsync(fd)
        os.fsync(transaction_fd)
        rename_noreplace(base_fd, transaction_fd, "payload", contract.candidate)
        # Keep the private transaction/authenticated ZIP as bounded audit residue.
        return {"passed": True, "candidate": contract.candidate, "publishedFiles": len(published) + 1,
                "publishedPayloadBytes": sum(row["bytes"] for row in published), "atomicNoReplace": True,
                "privateTransaction": transaction, "servicesChanged": False}
    finally:
        for fd in reversed(handles):
            os.close(fd)
        os.close(transaction_fd); os.close(base_fd)


def kernel_limits():
    import resource
    hard = resource.getrlimit(resource.RLIMIT_FSIZE)[1]
    require(hard == resource.RLIM_INFINITY or hard >= MAX_FILE, "fsize_hard_limit")
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_FILE, MAX_FILE))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    signal.signal(signal.SIGXFSZ, signal.SIG_IGN)


def linux_context(mode):
    require(sys.platform == "linux" and sys.flags.isolated == 1 and hasattr(os, "O_NOFOLLOW") and os.open in os.supports_dir_fd, "isolated_linux_required")
    import pwd
    ubuntu = pwd.getpwnam("ubuntu")
    require(ubuntu.pw_uid >= 1000 and ubuntu.pw_gid >= 1000 and ubuntu.pw_dir == "/home/ubuntu", "staging_uid_invalid")
    require(os.geteuid() == (0 if mode == "publish" else ubuntu.pw_uid), "execution_uid_invalid")
    require(os.getegid() == (0 if mode == "publish" else ubuntu.pw_gid), "execution_gid_invalid")
    os.umask(0o077)
    kernel_limits()
    if mode == "publish":
        script_fd = open_absolute_file(os.path.abspath(__file__))
        try:
            st = regular(script_fd, uid=0)
            require(not stat.S_IMODE(st.st_mode) & 0o022, "publisher_script_mode")
            parent_fd = open_chain(os.path.dirname(os.path.abspath(__file__)), root_owned=True)
            os.close(parent_fd)
        finally:
            os.close(script_fd)
    return ubuntu.pw_uid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="mode", required=True)
    staging = commands.add_parser("stage")
    staging.add_argument("--pin", required=True)
    staging.add_argument("--artifact", required=True)
    staging.add_argument("--output", required=True)
    publishing = commands.add_parser("publish")
    publishing.add_argument("--pin", required=True)
    publishing.add_argument("--artifact", required=True)
    publishing.add_argument("--stage", required=True)
    args = parser.parse_args()
    uid = linux_context(args.mode)
    contract, pin_sha = load_pin(args.pin, args.mode == "publish", uid)
    receipt = stage(args.artifact, args.output, uid, contract) if args.mode == "stage" else publish(args.artifact, args.stage, uid, contract, pin_sha)
    receipt["pinFileSha256"] = pin_sha
    print(json.dumps(receipt, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps({"passed": False, "code": str(error) if isinstance(error, SafetyError) else type(error).__name__}), file=sys.stderr)
        raise SystemExit(1)
