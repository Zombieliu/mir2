#!/usr/bin/env python3
"""Validate and archive immutable updater releases; never publish discovery files.

CMS signatures MUST first pass the parent's trusted PowerShell/package verifier.
This tool checks byte identities and release bindings, not signature authenticity.
On a write failure a partial, newly created archive is retained for investigation.
No existing output is overwritten or removed.
"""

from __future__ import annotations

import argparse
import ctypes
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tarfile
from dataclasses import dataclass
from typing import BinaryIO

MAX_TOTAL = 4 * 1024**3
MAX_FILE = 512 * 1024**2
MAX_META = 32 * 1024**2
MAX_FILES = 200_064
MAX_ENTRIES = 400_128
META = (
    "PACKAGE-MANIFEST.json", "VERSION.json", "RELEASE-STATEMENT.json",
    "RELEASE-STATEMENT.p7s",
)
ENGINE_FILES = {"ENGINE.json", "ENGINE.p7s", "Mir2Updater.exe"}
GAME_ROOT_FILES = {
    "mir2-platform-windows.exe", "mir2-client.toml", "README-START.txt",
    "CONTROLS.txt", "KNOWN-ISSUES.md", "NotoSansTC-OFL.txt", "OFL-NotoSans.txt",
    "OFL-NotoSansArabic.txt", "OFL-NotoSansDevanagari.txt", "OFL-NotoSansThai.txt",
    "MULTILINGUAL-SOURCES.json", "BUILD-ATTESTATION.json",
}
EXECUTABLE_TOKENS = {
    "exe", "dll", "com", "scr", "cpl", "msi", "msp", "bat", "cmd", "ps1",
    "psm1", "psd1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "reg",
    "lnk", "chm", "jar", "py", "pyw", "sh", "bash", "zsh", "fish", "pdb",
    "ilk", "dmp",
}
HEX = re.compile(r"[0-9A-F]{64}\Z")


class ValidationError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


def relative_path(value: str) -> str:
    require(isinstance(value, str) and 0 < len(value) <= 230 and value.isascii(),
            "path must be nonempty ASCII and at most 230 characters")
    require(not any(ord(c) < 32 or ord(c) == 127 or c in '\\:\"<>|?*' for c in value),
            f"unsafe Windows path: {value!r}")
    for part in value.split("/"):
        require(part not in ("", ".", "..") and not part.endswith((".", " ")),
                f"unsafe path segment: {value!r}")
        stem = part.split(".")[0].upper()
        require(stem not in {"CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"}
                and not re.fullmatch(r"(?:COM|LPT)[1-9]", stem),
                f"reserved Windows device: {value!r}")
    # Avoid PAX/GNU extension records, including their ownership metadata.
    try:
        tarfile.TarInfo(value).tobuf(format=tarfile.USTAR_FORMAT, encoding="ascii")
    except (ValueError, UnicodeError) as exc:
        raise ValidationError(f"path cannot be represented by plain USTAR: {value!r}") from exc
    return value


def game_path(value: str) -> None:
    relative_path(value)
    if value in GAME_ROOT_FILES or value in META:
        return
    require(value.startswith("mir2-assets/"), f"non-client payload: {value}")
    lower = value.lower()
    require("logs" not in lower.split("/"), "runtime logs cannot be payload")
    require(not any(token in EXECUTABLE_TOKENS for part in lower.split("/")
                    for token in part.split(".")[1:]), "executable asset rejected")
    require(lower.endswith((".png", ".json", ".map.gz", ".wav", ".ttf", ".otf", ".txt")),
            f"unsupported asset: {value}")


def uint(value: object, maximum: int, name: str, *, positive: bool = False) -> int:
    require(type(value) is int and (1 if positive else 0) <= value <= maximum,
            f"invalid {name}")
    return value


def hash_value(value: object, name: str) -> str:
    require(isinstance(value, str) and HEX.fullmatch(value) is not None,
            f"invalid uppercase SHA256: {name}")
    return value


def _json_pairs(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON property: {key}")
        result[key] = value
    return result


def json_bytes(data: bytes, name: str, limit: int = MAX_META) -> dict:
    require(0 < len(data) <= limit, f"JSON size outside bounds: {name}")
    try:
        result = json.loads(data.decode("utf-8"), object_pairs_hook=_json_pairs,
                            parse_constant=lambda _: (_ for _ in ()).throw(
                                ValidationError("nonfinite JSON value")))
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise ValidationError(f"invalid UTF-8 JSON: {name}") from exc
    require(isinstance(result, dict), f"JSON object required: {name}")
    return result


def _no_ads(path: Path) -> None:
    if os.name != "nt":
        return
    from ctypes import wintypes

    class StreamData(ctypes.Structure):
        _fields_ = [("size", ctypes.c_longlong), ("name", wintypes.WCHAR * 296)]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    first = kernel.FindFirstStreamW
    first.argtypes = [wintypes.LPCWSTR, ctypes.c_int, ctypes.POINTER(StreamData), wintypes.DWORD]
    first.restype = wintypes.HANDLE
    next_stream = kernel.FindNextStreamW
    next_stream.argtypes = [wintypes.HANDLE, ctypes.POINTER(StreamData)]
    next_stream.restype = wintypes.BOOL
    close = kernel.FindClose
    close.argtypes = [wintypes.HANDLE]
    close.restype = wintypes.BOOL
    data = StreamData()
    handle = first(str(path), 0, ctypes.byref(data), 0)
    if handle == ctypes.c_void_p(-1).value:
        require(ctypes.get_last_error() == 38, f"cannot enumerate NTFS streams: {path}")
        return
    try:
        while True:
            require(data.name == "::$DATA", f"alternate data stream rejected: {path}")
            if not next_stream(handle, ctypes.byref(data)):
                require(ctypes.get_last_error() == 38, f"cannot finish stream enumeration: {path}")
                break
    finally:
        close(handle)


def _plain(path: Path, *, directory: bool) -> os.stat_result:
    info = path.lstat()
    require(not stat.S_ISLNK(info.st_mode)
            and not (getattr(info, "st_file_attributes", 0) & 0x400),
            f"symlink/junction/reparse point rejected: {path}")
    require(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode),
            f"special file or wrong type rejected: {path}")
    if not directory:
        require(info.st_nlink == 1, f"hardlinked file rejected: {path}")
    _no_ads(path)
    return info


def absolute_directory(value: str) -> Path:
    require(isinstance(value, str) and Path(value).is_absolute(), "source directory must be absolute")
    path = Path(os.path.abspath(value))
    require(not (os.name == "nt" and path.drive.startswith("\\\\")), "UNC sources are unsupported")
    for ancestor in reversed((path, *path.parents)):
        _plain(ancestor, directory=True)
    return path


def _fingerprint(info: os.stat_result) -> tuple:
    # Python 3.13's Windows lstat reports legacy creation-time ctime, while
    # fstat can report the NTFS metadata-change time. Birthtime is consistent
    # between the two; inode/size/mtime and the streamed hash still bind bytes.
    identity_time = getattr(info, "st_birthtime_ns", info.st_ctime_ns)
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, identity_time)


@dataclass
class Entry:
    path: str
    source: Path
    size: int
    sha256: str
    fingerprint: tuple


def _open_entry(entry: Entry) -> BinaryIO:
    require(_fingerprint(_plain(entry.source, directory=False)) == entry.fingerprint,
            f"source changed before archiving: {entry.path}")
    fd = os.open(entry.source, os.O_RDONLY | getattr(os, "O_BINARY", 0) | getattr(os, "O_NOFOLLOW", 0))
    try:
        info = os.fstat(fd)
        require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                and _fingerprint(info) == entry.fingerprint,
                f"source changed while opening: {entry.path}")
        return os.fdopen(fd, "rb")
    except BaseException:
        os.close(fd)
        raise


def _hash_entry(entry: Entry) -> str:
    with _open_entry(entry) as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest().upper()
        require(_fingerprint(os.fstat(stream.fileno())) == entry.fingerprint,
                f"source changed while hashing: {entry.path}")
        return digest


def scan(source_map: dict) -> tuple[dict[str, dict[str, Entry]], dict[str, Path], dict[str, tuple]]:
    require(0 < len(source_map) <= 16, "source map must contain 1 through 16 release directories")
    roots: dict[str, Path] = {}
    for prefix, value in source_map.items():
        relative_path(prefix)
        require(prefix.startswith("releases/") and prefix != "releases/", "only immutable releases/ directories are allowed")
        for previous in roots:
            a, b = prefix.lower(), previous.lower()
            require(a != b and not a.startswith(b + "/") and not b.startswith(a + "/"),
                    "case-colliding or overlapping release directory mapping")
        roots[prefix] = absolute_directory(value)
    files: dict[str, dict[str, Entry]] = {}
    directories: dict[str, tuple] = {}
    names: dict[str, tuple[str, str]] = {}
    file_count = total = 0

    def name(path: str, kind: str) -> None:
        relative_path(path)
        previous = names.get(path.lower())
        require(previous is None or previous == (path, "directory") and kind == "directory",
                f"case-colliding or duplicate archive path: {path}")
        names[path.lower()] = (path, kind)
        require(len(names) <= MAX_ENTRIES, "archive entry count exceeds bound")

    for prefix, root in sorted(roots.items()):
        parts = prefix.split("/")
        for i in range(1, len(parts) + 1):
            name("/".join(parts[:i]), "directory")
        files[prefix] = {}
        stack = [(root, "")]
        while stack:
            folder, relative = stack.pop()
            archive_path = prefix + ("/" + relative if relative else "")
            info = _plain(folder, directory=True)
            name(archive_path, "directory")
            directories[archive_path] = (folder, _fingerprint(info))
            with os.scandir(folder) as children:
                children = sorted(children, key=lambda child: child.name)
            for child in children:
                relative_path(child.name)
                child_relative = relative + "/" + child.name if relative else child.name
                full_path = prefix + "/" + child_relative
                relative_path(child_relative)
                child_info = child.stat(follow_symlinks=False)
                if stat.S_ISDIR(child_info.st_mode):
                    name(full_path, "directory")
                    stack.append((Path(child.path), child_relative))
                else:
                    info = _plain(Path(child.path), directory=False)
                    require(info.st_size <= MAX_FILE, f"file exceeds size bound: {child_relative}")
                    name(full_path, "file")
                    file_count += 1
                    total += info.st_size
                    require(file_count <= MAX_FILES and total <= MAX_TOTAL, "archive file count/total size exceeds bound")
                    entry = Entry(child_relative, Path(child.path), info.st_size, "", _fingerprint(info))
                    entry.sha256 = _hash_entry(entry)
                    files[prefix][child_relative] = entry
    return files, roots, directories


def _metadata(files: dict[str, Entry], name: str, *, limit: int = MAX_META) -> tuple[dict, bytes]:
    require(name in files and files[name].size <= limit, f"missing/oversized metadata: {name}")
    with _open_entry(files[name]) as stream:
        data = stream.read(limit + 1)
    require(sha256(data) == files[name].sha256, f"metadata changed: {name}")
    return json_bytes(data, name, limit), data


def validate_candidate(files: dict[str, Entry]) -> dict:
    require(all(name in files and 0 < files[name].size <= MAX_META for name in META), "candidate metadata incomplete")
    manifest, _ = _metadata(files, META[0])
    require(set(manifest) == {"schema", "coverage", "fileCount", "totalBytes", "aggregateSha256", "files"}
            and manifest["schema"] == "mir2.windows.package-manifest.v4", "unsupported manifest schema/fields")
    coverage = manifest["coverage"]
    require(isinstance(coverage, dict) and set(coverage) == {"excludes", "rule"}
            and coverage["excludes"] == list(META) and isinstance(coverage["rule"], str)
            and bool(coverage["rule"]), "manifest metadata coverage mismatch")
    count = uint(manifest["fileCount"], 200_000, "manifest file count", positive=True)
    require(isinstance(manifest["files"], list) and len(manifest["files"]) == count, "manifest count mismatch")
    declared: dict[str, dict] = {}
    case_paths = set()
    for item in manifest["files"]:
        require(isinstance(item, dict) and set(item) == {"path", "size", "sha256"}, "invalid manifest entry")
        path = relative_path(item["path"])
        game_path(path)
        require(path not in META and path.lower() not in case_paths, "case-colliding manifest or metadata/payload overlap")
        case_paths.add(path.lower())
        size = uint(item["size"], MAX_FILE, "payload size")
        digest = hash_value(item["sha256"], path)
        require(path in files and files[path].size == size and files[path].sha256 == digest,
                f"payload hash/size mismatch: {path}")
        declared[path] = item
    require(set(files) == set(declared) | set(META), "actual candidate file closure differs from manifest")
    require({"mir2-platform-windows.exe", "BUILD-ATTESTATION.json"} <= set(declared), "missing candidate executable/attestation")
    canonical = "".join(f"{p}\t{declared[p]['size']}\t{declared[p]['sha256']}\n" for p in sorted(declared))
    aggregate = sha256(canonical.encode("ascii"))
    require(uint(manifest["totalBytes"], MAX_TOTAL, "manifest total bytes") == sum(e["size"] for e in declared.values())
            and hash_value(manifest["aggregateSha256"], "aggregate") == aggregate, "manifest aggregate/bytes mismatch")
    version, version_bytes = _metadata(files, "VERSION.json")
    statement, statement_bytes = _metadata(files, "RELEASE-STATEMENT.json")
    require(version.get("schema") == "mir2.windows.candidate-version.v4"
            and statement.get("schema") == "mir2.windows.release-statement.v1", "candidate version/statement schema mismatch")
    identity = version.get("candidate")
    require(isinstance(identity, str) and len(identity) < 100
            and re.fullmatch(r"WN-CANDIDATE-[A-Za-z0-9._-]+", identity) is not None, "invalid candidate identity")
    revision = version.get("gitRevision")
    require(isinstance(revision, str) and re.fullmatch(r"[0-9a-f]{40}", revision) is not None, "invalid source revision")
    require(version.get("worktreeDirty") is False and version.get("clientOnly") is True
            and version.get("exeName") == "mir2-platform-windows.exe", "candidate must be a clean client-only build")
    exe = declared["mir2-platform-windows.exe"]
    attestation = declared["BUILD-ATTESTATION.json"]
    expected = {
        "schema": "mir2.windows.release-statement.v1", "candidate": identity,
        "exeSha256": exe["sha256"], "packageManifestSha256": files["PACKAGE-MANIFEST.json"].sha256,
        "packageManifestAggregateSha256": aggregate, "versionSha256": sha256(version_bytes),
        "buildAttestationSha256": attestation["sha256"], "gitRevision": revision,
        "worktreeDirty": False,
        "worktreeStatusSha256": hash_value(version.get("worktreeStatusSha256"), "worktree status"),
    }
    require(statement == expected and statement_bytes == json.dumps(expected, separators=(",", ":")).encode("ascii"),
            "release statement is not the exact canonical version/payload binding")
    for field in ("exeSha256", "packageManifestSha256", "packageManifestAggregateSha256", "buildAttestationSha256"):
        require(version.get(field) == expected[field], f"version binding mismatch: {field}")
    require(type(version.get("exeSizeBytes")) is int and version["exeSizeBytes"] == exe["size"]
            and type(version.get("packageManifestFileCount")) is int and version["packageManifestFileCount"] == count
            and type(version.get("packageFileCount")) is int and version["packageFileCount"] == count + 4
            and version.get("packageManifestSchema") == manifest["schema"]
            and version.get("releaseStatementSchema") == statement["schema"]
            and version.get("signatureFormat") == "CMS/PKCS7-detached", "version count/size/signature contract mismatch")
    build, _ = _metadata(files, "BUILD-ATTESTATION.json")
    require(build.get("schema") == "mir2.windows.build-attestation.v2"
            and build.get("exeSha256") == exe["sha256"]
            and type(build.get("exeSizeBytes")) is int and build["exeSizeBytes"] == exe["size"]
            and build.get("gitRevision") == revision and build.get("worktreeDirty") is False
            and build.get("worktreeStatusSha256") == expected["worktreeStatusSha256"], "build attestation identity mismatch")
    return {"kind": "game", "identity": identity, "descriptorSha256": files["VERSION.json"].sha256}


def validate_engine(files: dict[str, Entry]) -> dict:
    require(set(files) == ENGINE_FILES, "engine release must contain exactly ENGINE.json, ENGINE.p7s and Mir2Updater.exe")
    require(0 < files["ENGINE.p7s"].size <= MAX_META, "engine signature missing/oversized")
    engine, _ = _metadata(files, "ENGINE.json", limit=32_768)
    require(set(engine) == {"schema", "version", "minBootstrap", "sourceRevision", "builtUnix", "exeSize", "exeSha256", "launcherSha256", "cargoLockSha256"}
            and engine["schema"] == "mir2.windows.updater-engine.v1", "unsupported engine descriptor schema/fields")
    uint(engine["version"], 2**32 - 1, "engine version", positive=True)
    uint(engine["minBootstrap"], 1, "minimum bootstrap")
    uint(engine["builtUnix"], 2**63 - 1, "engine build time", positive=True)
    require(isinstance(engine["sourceRevision"], str) and re.fullmatch(r"[0-9A-Fa-f]{40}", engine["sourceRevision"]) is not None,
            "invalid engine source revision")
    exe = files["Mir2Updater.exe"]
    require(uint(engine["exeSize"], MAX_FILE, "engine size", positive=True) == exe.size
            and hash_value(engine["exeSha256"], "engine executable") == exe.sha256, "engine executable identity mismatch")
    hash_value(engine["launcherSha256"], "launcher")
    hash_value(engine["cargoLockSha256"], "Cargo.lock")
    return {"kind": "engine", "identity": str(engine["version"]), "engineIdentitySha256": exe.sha256,
            "descriptorSha256": files["ENGINE.json"].sha256}


class CheckedReader:
    def __init__(self, stream: BinaryIO, entry: Entry):
        self.stream, self.entry = stream, entry
        self.digest, self.count = hashlib.sha256(), 0

    def read(self, size: int = -1) -> bytes:
        data = self.stream.read(size)
        self.digest.update(data)
        self.count += len(data)
        return data

    def verify(self) -> None:
        require(self.count == self.entry.size and self.digest.hexdigest().upper() == self.entry.sha256
                and _fingerprint(os.fstat(self.stream.fileno())) == self.entry.fingerprint,
                f"source changed during archive write: {self.entry.path}")


def _write_tar(output: Path, files: dict[str, Entry], directories: dict[str, tuple]) -> None:
    directory_names = set(directories)
    for name in [*files, *directories]:
        parts = name.split("/")
        directory_names.update("/".join(parts[:i]) for i in range(1, len(parts)))
    with output.open("xb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, compresslevel=1, mtime=0) as zipped:
            with tarfile.open(mode="w|", fileobj=zipped, format=tarfile.USTAR_FORMAT, encoding="ascii") as archive:
                for name in sorted(directory_names | set(files)):
                    member = tarfile.TarInfo(name)
                    member.uid = member.gid = member.mtime = 0
                    member.uname = member.gname = ""
                    if name in files:
                        entry = files[name]
                        member.mode, member.size = 0o644, entry.size
                        with _open_entry(entry) as stream:
                            reader = CheckedReader(stream, entry)
                            archive.addfile(member, reader)
                            reader.verify()
                    else:
                        if name in directories:
                            source, expected = directories[name]
                            require(_fingerprint(_plain(source, directory=True)) == expected,
                                    f"source directory changed before archiving: {name}")
                        member.type, member.mode = tarfile.DIRTYPE, 0o755
                        archive.addfile(member)
        raw.flush()
        os.fsync(raw.fileno())


def build_archive(source_map: dict, output: Path) -> dict:
    require(output.is_absolute() and output.name.endswith(".tar.gz"), "output must be an absolute fresh .tar.gz file")
    output = Path(os.path.abspath(output))
    relative_path(output.name)
    absolute_directory(str(output.parent))
    receipt_path = output.with_name(output.name + ".receipt.json")
    require(not os.path.lexists(output) and not os.path.lexists(receipt_path), "archive or receipt already exists; refusing overwrite")
    files_by_release, roots, directories = scan(source_map)
    for root in roots.values():
        require(not output.is_relative_to(root), "archive output must be outside every source tree")
    components, files = [], {}
    for prefix, entries in sorted(files_by_release.items()):
        if "PACKAGE-MANIFEST.json" in entries:
            result = validate_candidate(entries)
        elif "ENGINE.json" in entries:
            result = validate_engine(entries)
        else:
            raise ValidationError("source is neither a candidate package nor an engine release")
        components.append({"directory": prefix, **result, "fileCount": len(entries),
                           "totalBytes": sum(e.size for e in entries.values())})
        files.update((prefix + "/" + path, entry) for path, entry in entries.items())
    _write_tar(output, files, directories)
    with output.open("rb") as archived:
        digest = hashlib.file_digest(archived, "sha256").hexdigest().upper()
    receipt = {
        "schema": "mir2.windows.update-archive-receipt.v1", "archiveName": output.name,
        "archiveSha256": digest, "archiveSizeBytes": output.stat().st_size,
        "fileCount": len(files), "uncompressedBytes": sum(e.size for e in files.values()),
        "compression": "gzip-level-1", "tarFormat": "USTAR", "cmsVerified": False,
        "requiresExternalSignatureVerification": True, "components": components,
        "files": [{"path": path, "size": e.size, "sha256": e.sha256} for path, e in sorted(files.items())],
    }
    with receipt_path.open("x", encoding="utf-8", newline="\n") as saved:
        json.dump(receipt, saved, ensure_ascii=True, indent=2)
        saved.write("\n")
        saved.flush()
        os.fsync(saved.fileno())
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-map", required=True, type=Path,
                        help='JSON object, e.g. {"releases/game-r6":"C:/.../dist/r6","releases/updater-ABCD":"C:/.../engine"}')
    parser.add_argument("--output", required=True, type=Path, help="absolute, nonexistent .tar.gz output")
    args = parser.parse_args()
    try:
        with args.source_map.open("rb") as mapping_stream:
            source_map = json_bytes(mapping_stream.read(65_537), "source map", 65_536)
        receipt = build_archive(source_map, args.output)
    except (ValidationError, OSError, tarfile.TarError) as exc:
        print(f"Archive rejected: {exc}", file=sys.stderr)
        return 1
    print("CMS signatures were not checked by this helper; use the trusted external verifier before publication.", file=sys.stderr)
    print(json.dumps({k: v for k, v in receipt.items() if k != "files"}, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
