#!/usr/bin/env python3
"""Offline byte verifier. Run root's trusted CMS/package gates separately.

Hashes every decoded bundle target and reconstructed delta without extraction.
Only the four Candidate metadata files and build attestation are read locally;
the 123,000 candidate assets are not rescanned. Fresh receipts retain failures.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import struct
import sys
import time
import zlib

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "mir2_native_verify_archive", Path(__file__).absolute().with_name("archive-update-release.py"))
archive = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = archive
SPEC.loader.exec_module(archive)
require = archive.require
MAX_COMPRESSED = 128 * 1024**2
MAX_EXPANDED = 512 * 1024**2
MAX_OPERATIONS = 65_536
CHUNK = 1024 * 1024
SCHEMA = "mir2.windows.delivery-verification.v1"


def exact(value, fields, message):
    require(isinstance(value, dict) and set(value) == set(fields), message)


def file_entry(path, relative, maximum):
    path = Path(path)
    require(path.is_absolute(), "absolute input file required")
    archive.absolute_directory(str(path.parent))
    info = archive._plain(path, directory=False)
    require(0 < info.st_size <= maximum, "input file size outside bounds")
    entry = archive.Entry(relative, path, info.st_size, "", archive._fingerprint(info))
    entry.sha256 = archive._hash_entry(entry)
    return entry


def read_entry(entry, maximum):
    require(entry.size <= maximum, "input read size outside bounds")
    with archive._open_entry(entry) as stream:
        data = stream.read(maximum + 1)
        require(archive._fingerprint(os.fstat(stream.fileno())) == entry.fingerprint,
                "input changed during read")
    require(len(data) == entry.size and archive.sha256(data) == entry.sha256,
            "input byte binding changed")
    return data


def candidate_binding(candidate_root):
    root = archive.absolute_directory(str(candidate_root))
    files = {name: file_entry(root / name, name, archive.MAX_META) for name in archive.META}
    manifest_bytes = read_entry(files["PACKAGE-MANIFEST.json"], archive.MAX_META)
    manifest = archive.json_bytes(manifest_bytes, "PACKAGE-MANIFEST.json")
    require(isinstance(manifest.get("files"), list), "manifest files array required")
    for item in manifest["files"]:
        exact(item, {"path", "size", "sha256"}, "manifest entry fields")
        path = archive.relative_path(item["path"])
        archive.game_path(path)
        require(path not in archive.META, "manifest metadata overlap")
        size = archive.uint(item["size"], archive.MAX_FILE, "manifest size")
        digest = archive.hash_value(item["sha256"], "manifest hash")
        # Declared entries are validated against decoded bundle bytes below.
        files[path] = archive.Entry(path, root / path, size, digest, ())
    require("BUILD-ATTESTATION.json" in files, "build attestation absent from manifest")
    build = file_entry(root / "BUILD-ATTESTATION.json", "BUILD-ATTESTATION.json", archive.MAX_META)
    require(build.size == files[build.path].size and build.sha256 == files[build.path].sha256,
            "local build attestation differs from manifest")
    files[build.path] = build
    identity = archive.validate_candidate(files)
    version_bytes = read_entry(files["VERSION.json"], archive.MAX_META)
    version = archive.json_bytes(version_bytes, "VERSION.json")
    return root, files, identity, version, manifest_bytes, version_bytes


def artifact(item, suffix, names):
    exact(item, {"path", "size", "sha256"}, "delivery artifact fields")
    path = archive.relative_path(item["path"])
    digest = archive.hash_value(item["sha256"], "delivery artifact hash")
    require(path == "delivery/" + digest.lower() + suffix and path.lower() not in names,
            "noncanonical or duplicate delivery artifact")
    names.add(path.lower())
    archive.uint(item["size"], MAX_COMPRESSED, "delivery compressed size", positive=True)


def descriptor_binding(data, files, identity, manifest_bytes, version_bytes, now=None):
    value = archive.json_bytes(data, "DELIVERY.json")
    exact(value, {"schema", "candidate", "packageManifestSha256", "versionSha256",
                  "builtUnix", "bundles", "patches"}, "delivery descriptor fields")
    require(value["schema"] == "mir2.windows.delivery.v1" and value["candidate"] == identity["identity"]
            and value["packageManifestSha256"] == archive.sha256(manifest_bytes)
            and value["versionSha256"] == archive.sha256(version_bytes), "delivery Candidate binding")
    archive.uint(value["builtUnix"], int(now if now is not None else time.time()) + 300,
                 "delivery build time", positive=True)
    require(isinstance(value["bundles"], list) and 0 < len(value["bundles"]) <= 64
            and isinstance(value["patches"], list) and len(value["patches"]) <= 64,
            "delivery object count outside bounds")
    artifacts, paths, total = set(), [], 0
    for bundle in value["bundles"]:
        exact(bundle, {"archive", "files"}, "bundle descriptor fields")
        artifact(bundle["archive"], ".m2b.gz", artifacts)
        require(isinstance(bundle["files"], list) and 0 < len(bundle["files"]) <= 200_000,
                "bundle entry count outside bounds")
        expanded = 12
        for path in bundle["files"]:
            archive.game_path(path)
            require(path in files and path not in archive.META, "bundle path absent from manifest")
            paths.append(path)
            expanded += 10 + len(path.encode("ascii")) + files[path].size
        require(expanded <= MAX_EXPANDED, "expanded bundle including headers exceeds bound")
        total += bundle["archive"]["size"]
    require(paths == sorted(set(files) - set(archive.META)),
            "bundle coverage/order differs from complete manifest")
    require(len({path.lower() for path in paths}) == len(paths), "case-colliding bundle paths")
    bases = set()
    for patch in value["patches"]:
        exact(patch, {"path", "baseSize", "baseSha256", "patch"}, "patch descriptor fields")
        require(patch["path"] == "mir2-platform-windows.exe", "unsupported delta target")
        archive.uint(patch["baseSize"], archive.MAX_FILE, "delta base size", positive=True)
        digest = archive.hash_value(patch["baseSha256"], "delta base")
        require(digest not in bases, "duplicate delta base")
        bases.add(digest)
        artifact(patch["patch"], ".m2d.gz", artifacts)
        total += patch["patch"]["size"]
    require(total <= archive.MAX_TOTAL, "compressed delivery total exceeds bound")
    return value


class SingleGzip:
    """Bounded zlib gzip reader: exactly one member, CRC, raw and compressed EOF."""
    def __init__(self, entry):
        self.entry = entry
        self.stream = archive._open_entry(entry)
        self.decoder = zlib.decompressobj(31)
        self.pending = b""
        self.buffer = b""
        self.compressed = self.expanded = 0
        self.digest = hashlib.sha256()
        self.finished = False

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.stream.close()

    def pump(self):
        if self.finished:
            return
        if not self.pending:
            self.pending = self.stream.read(64 * 1024)
            require(bool(self.pending), "truncated gzip member")
            self.compressed += len(self.pending)
            require(self.compressed <= MAX_COMPRESSED and self.compressed <= self.entry.size,
                    "compressed artifact exceeds bound")
            self.digest.update(self.pending)
        try:
            decoded = self.decoder.decompress(self.pending, 64 * 1024)
        except zlib.error:
            raise archive.ValidationError("invalid gzip member/CRC") from None
        self.pending = self.decoder.unconsumed_tail
        self.expanded += len(decoded)
        require(self.expanded <= MAX_EXPANDED, "expanded artifact including headers exceeds bound")
        self.buffer += decoded
        if self.decoder.eof:
            require(not self.decoder.unused_data and not self.pending and self.stream.read(1) == b"",
                    "extra gzip member or trailing compressed bytes")
            require(self.compressed == self.entry.size
                    and self.digest.hexdigest().upper() == self.entry.sha256,
                    "compressed artifact hash/size mismatch")
            require(archive._fingerprint(os.fstat(self.stream.fileno())) == self.entry.fingerprint,
                    "compressed artifact changed during verification")
            self.finished = True

    def read(self, count):
        require(0 <= count <= CHUNK, "unbounded decoded read")
        while len(self.buffer) < count and not self.finished:
            self.pump()
        result, self.buffer = self.buffer[:count], self.buffer[count:]
        return result

    def exact(self, count):
        result = self.read(count)
        require(len(result) == count, "truncated raw delivery stream")
        return result

    def finish(self):
        require(self.read(1) == b"" and self.finished, "trailing raw delivery bytes")


def compressed_entry(root, declared):
    path = root / declared["path"]
    archive.absolute_directory(str(path.parent))
    info = archive._plain(path, directory=False)
    require(info.st_size == declared["size"], "compressed artifact local size mismatch")
    return archive.Entry(declared["path"], path, info.st_size, declared["sha256"], archive._fingerprint(info))


def verify_bundle(root, bundle, files):
    entry = compressed_entry(root, bundle["archive"])
    with SingleGzip(entry) as stream:
        require(stream.exact(8) == b"MIR2PK01", "bundle magic")
        count, = struct.unpack("<I", stream.exact(4))
        require(count == len(bundle["files"]), "bundle raw entry count")
        for expected in bundle["files"]:
            length, = struct.unpack("<H", stream.exact(2))
            require(0 < length <= 230, "bundle path length")
            try:
                path = stream.exact(length).decode("ascii")
            except UnicodeError:
                raise archive.ValidationError("non-ASCII bundle path") from None
            archive.game_path(path)
            require(path == expected, "bundle raw path/order mismatch")
            size, = struct.unpack("<Q", stream.exact(8))
            target = files[path]
            require(size == target.size, "bundle raw target size mismatch")
            digest, remaining = hashlib.sha256(), size
            while remaining:
                data = stream.exact(min(CHUNK, remaining))
                digest.update(data)
                remaining -= len(data)
            require(digest.hexdigest().upper() == target.sha256, "decoded target hash mismatch")
        stream.finish()
        expected_expanded = 12 + sum(10 + len(p) + files[p].size for p in bundle["files"])
        require(stream.expanded == expected_expanded, "bundle expanded size mismatch")
        return {"path": entry.path, "fileCount": count, "expandedBytes": stream.expanded}


def verify_patch(root, patch, target, base):
    entry = compressed_entry(root, patch["patch"])
    with SingleGzip(entry) as stream, archive._open_entry(base) as original:
        require(stream.exact(8) == b"MIR2DP01", "delta magic")
        count, size = struct.unpack("<IQ", stream.exact(12))
        require(0 < count <= MAX_OPERATIONS and size == target.size, "delta raw bounds")
        digest, produced, copies, literals = hashlib.sha256(), 0, 0, 0
        for _ in range(count):
            tag = stream.exact(1)[0]
            if tag == 0:
                offset, length = struct.unpack("<QQ", stream.exact(16))
                require(0 < length <= base.size and offset + length <= base.size, "delta copy outside base")
                original.seek(offset)
                copies += length
            else:
                require(tag == 1, "unknown delta operation")
                length, = struct.unpack("<Q", stream.exact(8))
                require(0 < length <= size, "delta literal outside target")
                literals += length
            require(produced + length <= size, "delta output overrun")
            remaining = length
            while remaining:
                amount = min(CHUNK, remaining)
                data = original.read(amount) if tag == 0 else stream.exact(amount)
                require(len(data) == amount, "truncated delta base")
                digest.update(data)
                remaining -= amount
            produced += length
        stream.finish()
        require(produced == size and digest.hexdigest().upper() == target.sha256,
                "delta reconstructed target hash/size mismatch")
        require(archive._fingerprint(os.fstat(original.fileno())) == base.fingerprint,
                "delta base changed during reconstruction")
        return {"path": entry.path, "baseSha256": base.sha256, "baseSize": base.size,
                "targetSha256": target.sha256, "targetBytes": size, "operationCount": count,
                "copyBytes": copies, "literalBytes": literals, "expandedBytes": stream.expanded}


def verify(candidate_root, delivery_root, bases, receipt, journal=lambda: None):
    root, files, identity, version, manifest_bytes, version_bytes = candidate_binding(candidate_root)
    delivery_root = archive.absolute_directory(str(delivery_root))
    descriptor = file_entry(delivery_root / "DELIVERY.json", "DELIVERY.json", archive.MAX_META)
    data = read_entry(descriptor, archive.MAX_META)
    value = descriptor_binding(data, files, identity, manifest_bytes, version_bytes)
    receipt.update(candidate=identity["identity"], sourceRevision=version["gitRevision"],
                   candidateRoot=str(root), deliveryRoot=str(delivery_root),
                   manifestSha256=archive.sha256(manifest_bytes), versionSha256=archive.sha256(version_bytes),
                   deliverySha256=descriptor.sha256,
                   metadata=[{"path": name, "size": files[name].size, "sha256": files[name].sha256} for name in archive.META])
    supplied = {}
    require(len(bases) <= 64, "too many supplied delta bases")
    for path in bases:
        entry = file_entry(path, "mir2-platform-windows.exe", archive.MAX_FILE)
        key = (entry.size, entry.sha256)
        require(key not in supplied, "duplicate supplied delta base")
        supplied[key] = entry
    declared = {(p["baseSize"], p["baseSha256"]) for p in value["patches"]}
    require(declared == set(supplied), "declared delta base was not verified or supplied base is undeclared")
    used = set()
    for bundle in value["bundles"]:
        receipt["bundles"].append(verify_bundle(delivery_root, bundle, files))
        receipt["checkedArtifacts"].append(bundle["archive"])
        journal()
    for patch in value["patches"]:
        key = (patch["baseSize"], patch["baseSha256"])
        require(key in supplied, "declared delta base was not verified")
        used.add(key)
        receipt["patches"].append(verify_patch(delivery_root, patch, files[patch["path"]], supplied[key]))
        receipt["checkedArtifacts"].append(patch["patch"])
        journal()
    require(used == set(supplied), "supplied delta base is undeclared")
    receipt.update(payloadFiles=len(files) - 4, payloadBytes=sum(e.size for p, e in files.items() if p not in archive.META),
                   compressedBytes=sum(item["size"] for item in receipt["checkedArtifacts"]), passed=True)


def run(candidate_root, delivery_root, bases, receipt_path=None):
    result = {"schema": SCHEMA, "passed": False, "cmsVerified": False,
              "requiresExternalSignatureVerification": True, "bundles": [], "patches": [],
              "checkedArtifacts": [], "startedUnix": int(time.time())}
    def attempt(journal=lambda: None):
        try:
            verify(candidate_root, delivery_root, bases, result, journal)
        except archive.ValidationError as error:
            result["failure"] = str(error)
        except Exception:
            result["failure"] = "local input or delivery verification failed"
        result["finishedUnix"] = int(time.time())
        journal()
    if receipt_path is None:
        attempt()
        return result
    receipt_path = Path(receipt_path)
    require(receipt_path.is_absolute() and not os.path.lexists(receipt_path), "fresh absolute receipt required")
    archive.absolute_directory(str(receipt_path.parent))
    with receipt_path.open("x+", encoding="utf-8", newline="\n") as output:
        def journal():
            output.seek(0)
            output.write(json.dumps(result, indent=2) + "\n")
            output.truncate()
            output.flush()
            os.fsync(output.fileno())
        journal()
        attempt(journal)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-root", type=Path, required=True)
    parser.add_argument("--delivery-root", type=Path, required=True)
    parser.add_argument("--base-exe", type=Path, action="append", default=[])
    parser.add_argument("--receipt", type=Path, help="Fresh retained proof; omit for stdout-only verification.")
    args = parser.parse_args()
    try:
        result = run(args.candidate_root, args.delivery_root, args.base_exe, args.receipt)
        print(json.dumps(result if args.receipt is None else
                         {"passed": result["passed"], "receipt": str(args.receipt),
                          "cmsVerified": False, "failure": result.get("failure")}))
        return 0 if result["passed"] else 1
    except Exception:
        print(json.dumps({"passed": False, "failure": "receipt or local input guard failed"}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
