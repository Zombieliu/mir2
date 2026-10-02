#!/usr/bin/env python3
"""Create bounded gzip bundles/deltas for an unchanged signed game Candidate.

This helper verifies the entire manifest byte closure, not CMS authenticity.
Use sign-native-delivery.ps1 before publication. Failed create-only outputs are
retained; no existing artifact, package, preference, or installation is edited.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import sys
import time

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "mir2_delivery_archive", Path(__file__).absolute().with_name("archive-update-release.py"))
archive = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = archive
SPEC.loader.exec_module(archive)
require = archive.require

MAX_COMPRESSED = 128 * 1024**2
MAX_EXPANDED = 512 * 1024**2
MAX_OPERATIONS = 65_536
BUNDLE_MAGIC = b"MIR2PK01"
DELTA_MAGIC = b"MIR2DP01"
GEAR = tuple(int.from_bytes(hashlib.sha256(b"mir2-delta-gear-v1" + bytes([n])).digest()[:8], "little")
             for n in range(256))


def compact(value: dict) -> bytes:
    return json.dumps(value, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def put(path: Path, data: bytes) -> None:
    with path.open("xb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def entry(path: Path, relative: str) -> dict:
    info = archive._plain(path, directory=False)
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest().upper()
    return {"path": relative, "size": info.st_size, "sha256": digest}


def partitions(files: list, target_bytes: int) -> list[list]:
    require(1 <= target_bytes <= MAX_COMPRESSED, "bundle target outside bounds")
    result, group, expanded = [], [], 12
    for item in files:
        cost = 10 + len(item.path.encode("ascii")) + item.size
        require(12 + cost <= MAX_EXPANDED, "file exceeds expanded bundle bound")
        if group and expanded + cost > target_bytes:
            result.append(group)
            group, expanded = [], 12
        group.append(item)
        expanded += cost
    if group:
        result.append(group)
    require(0 < len(result) <= 64, "bundle count exceeds 64")
    return result


def write_bundle(output: Path, files: list) -> dict:
    require(0 < len(files) <= 200_000, "bundle entry count outside bounds")
    names = set()
    for item in files:
        archive.game_path(item.path)
        require(item.path not in archive.META and item.path.lower() not in names,
                "metadata/duplicate path in bundle")
        names.add(item.path.lower())
    expanded = 12 + sum(10 + len(e.path.encode("ascii")) + e.size for e in files)
    require(expanded <= MAX_EXPANDED, "bundle expanded size outside bounds")
    with output.open("xb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=6) as packed:
            packed.write(BUNDLE_MAGIC + struct.pack("<I", len(files)))
            for item in files:
                path = item.path.encode("ascii")
                packed.write(struct.pack("<H", len(path)) + path + struct.pack("<Q", item.size))
                with archive._open_entry(item) as source:
                    reader = archive.CheckedReader(source, item)
                    while data := reader.read(1024 * 1024):
                        packed.write(data)
                    reader.verify()
        raw.flush()
        os.fsync(raw.fileno())
    require(output.stat().st_size <= MAX_COMPRESSED, "compressed bundle exceeds 128MiB; choose smaller target")
    return {"files": [e.path for e in files], "expandedBytes": expanded}


def chunks(data: bytes, minimum: int = 4096, average: int = 16_384,
           maximum: int = 65_536):
    require(64 <= minimum <= average <= maximum and average & (average - 1) == 0,
            "invalid content-defined chunk bounds")
    offset, length, mask = 0, len(data), average - 1
    while offset < length:
        cursor, end, rolling = min(offset + minimum, length), min(offset + maximum, length), 0
        while cursor < end:
            rolling = ((rolling << 1) + GEAR[data[cursor]]) & 0xFFFF_FFFF_FFFF_FFFF
            cursor += 1
            if rolling & mask == 0:
                break
        yield offset, cursor - offset
        offset = cursor


def delta_operations(base: bytes, target: bytes) -> list:
    require(0 < len(base) <= archive.MAX_FILE and 0 < len(target) <= archive.MAX_FILE,
            "delta base/target outside bounds")
    lookup = {}
    for offset, length in chunks(base):
        lookup.setdefault(hashlib.sha256(memoryview(base)[offset:offset + length]).digest(), (offset, length))
    operations = []
    for offset, length in chunks(target):
        data = memoryview(target)[offset:offset + length]
        match = lookup.get(hashlib.sha256(data).digest())
        if match and match[1] == length and memoryview(base)[match[0]:match[0] + length] == data:
            if operations and operations[-1][0] == 0 and operations[-1][1] + operations[-1][2] == match[0]:
                operations[-1] = (0, operations[-1][1], operations[-1][2] + length)
            else:
                operations.append((0, match[0], length))
        elif operations and operations[-1][0] == 1:
            operations[-1] = (1, operations[-1][1], operations[-1][2] + length)
        else:
            operations.append((1, offset, length))
    require(0 < len(operations) <= MAX_OPERATIONS, "delta operation count outside bounds")
    require(sum(op[2] for op in operations) == len(target), "delta output length mismatch")
    return operations


def write_delta(output: Path, base: bytes, target: bytes) -> dict:
    operations = delta_operations(base, target)
    with output.open("xb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=6) as packed:
            packed.write(DELTA_MAGIC + struct.pack("<IQ", len(operations), len(target)))
            for tag, offset, length in operations:
                packed.write(bytes([tag]))
                if tag == 0:
                    packed.write(struct.pack("<QQ", offset, length))
                else:
                    packed.write(struct.pack("<Q", length))
                    packed.write(memoryview(target)[offset:offset + length])
        raw.flush()
        os.fsync(raw.fileno())
    require(output.stat().st_size <= MAX_COMPRESSED, "delta compressed size outside bounds")
    return {"operationCount": len(operations),
            "copyBytes": sum(op[2] for op in operations if op[0] == 0),
            "literalBytes": sum(op[2] for op in operations if op[0] == 1)}


def read_exact(stream, count: int) -> bytes:
    data = stream.read(count)
    require(len(data) == count, "truncated delivery artifact")
    return data


def verify_delta(path: Path, base: bytes, target: bytes) -> None:
    # Independent producer-side reconstruction; the native reader also verifies
    # the artifact hash, gzip single-member EOF, bounds, base hash and target SHA.
    with gzip.open(path, "rb") as stream:
        require(read_exact(stream, 8) == DELTA_MAGIC, "delta magic")
        count, size = struct.unpack("<IQ", read_exact(stream, 12))
        require(0 < count <= MAX_OPERATIONS and size == len(target), "delta declared bounds")
        digest, produced = hashlib.sha256(), 0
        for _ in range(count):
            tag = read_exact(stream, 1)[0]
            if tag == 0:
                offset, length = struct.unpack("<QQ", read_exact(stream, 16))
                require(0 < length <= len(base) and offset + length <= len(base), "delta copy outside base")
                data = memoryview(base)[offset:offset + length]
            else:
                require(tag == 1, "unknown delta operation")
                length, = struct.unpack("<Q", read_exact(stream, 8))
                require(0 < length <= len(target), "delta literal outside target")
                data = read_exact(stream, length)
            produced += length
            require(produced <= size, "delta output overrun")
            digest.update(data)
        require(produced == size and digest.digest() == hashlib.sha256(target).digest()
                and stream.read(1) == b"", "delta reconstruction mismatch/trailing data")


def build(candidate_root: str, output: Path, bases: list[Path], bundle_bytes: int) -> dict:
    root = archive.absolute_directory(candidate_root)
    require(output.is_absolute() and not output.exists(), "fresh absolute delivery output required")
    archive.absolute_directory(str(output.parent))
    output.mkdir()
    started = time.monotonic()
    put(output / "INPUT.json", compact({"schema": "mir2.windows.delivery-input.v1",
        "candidateRoot": str(root), "baseExecutables": [str(p) for p in bases], "bundleTargetBytes": bundle_bytes}))
    scanned, _, _ = archive.scan({"releases/game": str(root)})
    files = scanned["releases/game"]
    identity = archive.validate_candidate(files)
    version, version_bytes = archive._metadata(files, "VERSION.json")
    _, manifest_bytes = archive._metadata(files, "PACKAGE-MANIFEST.json")
    payload = [files[p] for p in sorted(files) if p not in archive.META]
    artifact_dir = output / "delivery"
    artifact_dir.mkdir()
    bundles, bundle_proof = [], []
    for index, group in enumerate(partitions(payload, bundle_bytes)):
        partial = artifact_dir / f"bundle-{index:03}.building"
        result = write_bundle(partial, group)
        info = entry(partial, "placeholder")
        name = info["sha256"].lower() + ".m2b.gz"
        require(not (artifact_dir / name).exists(), "bundle identity collision")
        partial.rename(artifact_dir / name)
        info["path"] = "delivery/" + name
        bundles.append({"archive": info, "files": result["files"]})
        bundle_proof.append({"archive": info, "fileCount": len(group), "expandedBytes": result["expandedBytes"]})
        print(json.dumps({"stage": "bundle", "index": index, "files": len(group), "bytes": info["size"]}), flush=True)
    patches, patch_proof, base_hashes = [], [], set()
    require(len(bases) <= 16, "at most16 delta bases per invocation")
    if bases:
        with archive._open_entry(files["mir2-platform-windows.exe"]) as stream:
            target = stream.read(archive.MAX_FILE + 1)
        require(archive.sha256(target) == files["mir2-platform-windows.exe"].sha256, "target changed before delta")
        for index, path in enumerate(bases):
            path = Path(path)
            require(path.is_absolute(), "absolute delta base path required")
            archive.absolute_directory(str(path.parent))
            info = archive._plain(path, directory=False)
            require(0 < info.st_size <= archive.MAX_FILE, "delta base size outside bounds")
            with path.open("rb") as stream:
                base = stream.read(archive.MAX_FILE + 1)
                require(archive._fingerprint(os.fstat(stream.fileno())) == archive._fingerprint(info), "base changed during read")
            digest = archive.sha256(base)
            require(digest not in base_hashes, "duplicate delta base")
            base_hashes.add(digest)
            if base == target:
                patch_proof.append({"baseSha256": digest, "skipped": "base already matches target"})
                continue
            partial = artifact_dir / f"patch-{index:03}.building"
            result = write_delta(partial, base, target)
            verify_delta(partial, base, target)
            artifact = entry(partial, "placeholder")
            # Incompressible targets or unrelated versions are still correct,
            # but are omitted if they do not reduce network bytes.
            if artifact["size"] >= len(target):
                patch_proof.append({"baseSha256": digest, "skipped": "patch is not smaller than target", **result})
                continue
            name = artifact["sha256"].lower() + ".m2d.gz"
            require(not (artifact_dir / name).exists(), "delta identity collision")
            partial.rename(artifact_dir / name)
            artifact["path"] = "delivery/" + name
            patches.append({"path": "mir2-platform-windows.exe", "baseSize": len(base), "baseSha256": digest, "patch": artifact})
            patch_proof.append({"baseSha256": digest, "patch": artifact, "targetBytes": len(target), **result})
            print(json.dumps({"stage": "delta", "baseSha256": digest, "bytes": artifact["size"], **result}), flush=True)
    descriptor = {"schema": "mir2.windows.delivery.v1", "candidate": identity["identity"],
        "packageManifestSha256": archive.sha256(manifest_bytes), "versionSha256": archive.sha256(version_bytes),
        "builtUnix": int(time.time()), "bundles": bundles, "patches": patches}
    encoded = compact(descriptor)
    require(len(encoded) <= archive.MAX_META, "delivery descriptor exceeds32MiB")
    put(output / "DELIVERY.json", encoded)
    receipt = {"schema": "mir2.windows.delivery-build.v1", "passed": True,
        "cmsVerified": False, "candidate": identity["identity"], "gameSourceRevision": version["gitRevision"],
        "manifestSha256": archive.sha256(manifest_bytes), "deliverySha256": archive.sha256(encoded),
        "payloadFiles": len(payload), "payloadBytes": sum(e.size for e in payload),
        "compressedBundleBytes": sum(b["archive"]["size"] for b in bundles),
        "bundles": bundle_proof, "patches": patch_proof, "elapsedSeconds": round(time.monotonic() - started, 3)}
    put(output / "BUILD-DELIVERY.json", compact(receipt))
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-root", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--base-exe", type=Path, action="append", default=[])
    parser.add_argument("--bundle-mib", type=int, default=64)
    args = parser.parse_args()
    try:
        proof = build(args.candidate_root, args.output, args.base_exe, args.bundle_mib * 1024**2)
        print(json.dumps({key: value for key, value in proof.items() if key not in ("bundles", "patches")}, indent=2))
        return 0
    except Exception as error:
        # Paths and validation failures contain no credentials. Preserve partial
        # output; future attempts must select a new directory.
        print(f"delivery build failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
