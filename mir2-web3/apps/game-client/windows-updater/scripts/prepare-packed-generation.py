#!/usr/bin/env python3
"""Create-only, offline packed-generation byte preparation. No CMS or publishing.

The explicit resource-report digest is a trust input, not a hash inferred from an
untrusted report. Production Candidate28 uses the previously admitted 4CDB...66
report. A clean source snapshot and an exact EXE digest are recorded separately
from resource provenance; this tool does not claim reproducible compilation.
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
import subprocess
import sys
import time
import tomllib

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("packed_generation_archive", Path(__file__).with_name("archive-update-release.py"))
a = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = a
SPEC.loader.exec_module(a)
require = a.require
MAX_CATALOG = 128 * 1024**2
MAX_PACK = 64 * 1024**2
MAX_TOTAL = 4 * 1024**3
MAX_SEQUENCE = 9007199254740991
PREPARED = "mir2.windows.packed-generation-preparation.v1"


def compact(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, "duplicate JSON property")
        result[key] = value
    return result


def decoded(data):
    return json.loads(data, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("nonfinite JSON")))


def exact(value, fields, message):
    require(isinstance(value, dict) and set(value) == set(fields), message)


def uint(value, maximum, message, positive=False):
    require(type(value) is int and (1 if positive else 0) <= value <= maximum, message)
    return value


def digest(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9A-Fa-f]{64}", value), "invalid SHA256")
    return value.upper()


def revision(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value), "lowercase source revision required")
    return value


def file_entry(path, relative, maximum, positive=True):
    path = Path(path)
    a.absolute_directory(str(path.parent))
    info = a._plain(path, directory=False)
    uint(info.st_size, maximum, "input file size bound", positive)
    entry = a.Entry(relative, path, info.st_size, "", a._fingerprint(info))
    entry.sha256 = a._hash_entry(entry)
    return entry


def entry_dict(entry):
    return {"path": entry.path, "size": entry.size, "sha256": entry.sha256}


def read(entry, maximum):
    require(entry.size <= maximum, "bounded read")
    with a._open_entry(entry) as stream:
        data = stream.read(maximum + 1)
    require(len(data) == entry.size and a.sha256(data) == entry.sha256, "input changed while reading")
    return data


def put(path, data):
    path = Path(path)
    a.absolute_directory(str(path.parent))
    with path.open("xb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def copy_new(entry, path):
    path = Path(path)
    a.absolute_directory(str(path.parent))
    if os.path.lexists(path):
        existing = file_entry(path, entry.path, entry.size)
        require(existing.size == entry.size and existing.sha256 == entry.sha256,
                "immutable output conflict; refusing overwrite")
        return False
    with a._open_entry(entry) as source, path.open("xb") as output:
        state, copied = hashlib.sha256(), 0
        while data := source.read(1024 * 1024):
            copied += len(data)
            require(copied <= entry.size, "copy exceeds declared size")
            state.update(data)
            output.write(data)
        require(copied == entry.size and state.hexdigest().upper() == entry.sha256,
                "copy source size/SHA mismatch")
        output.flush()
        os.fsync(output.fileno())
    actual = file_entry(path, entry.path, entry.size)
    require(actual.size == entry.size and actual.sha256 == entry.sha256, "copy readback mismatch")
    return True


def git(root, *args, data=None):
    command = ["git", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false", "-C", str(root), *args]
    result = subprocess.run(command, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    require(result.returncode == 0 and len(result.stdout) <= 32 * 1024**2, "source Git gate failed")
    return result.stdout


def source_snapshot(root, expected):
    root = a.absolute_directory(str(root))
    expected = revision(expected)
    require(git(root, "rev-parse", "HEAD").decode().strip() == expected, "source HEAD differs from declared revision")
    status = git(root, "status", "--porcelain=v1", "--untracked-files=all")
    require(status == b"", "source must be clean, including untracked files")
    # ls-tree does not expand '*.rs' as the recursive pathspec glob that ls-files
    # accepts. Select compiler inputs from the bounded authoritative HEAD tree.
    records = git(root, "ls-tree", "-r", "-z", "HEAD")
    tracked = {}
    for record in records.split(b"\0"):
        if record:
            prefix, name = record.split(b"\t", 1)
            mode, kind, oid = prefix.split()
            name = name.decode("utf-8")
            if not (name.endswith(".rs") or Path(name).name in ("Cargo.toml", "Cargo.lock")):
                continue
            require(kind == b"blob" and mode in (b"100644", b"100755"), "plain committed source files required")
            require("\n" not in name and "\r" not in name, "unsafe source filename")
            tracked[name] = oid.decode("ascii")
    require(tracked, "committed Rust/Cargo source inventory required")
    # Reject ignored, compiled-looking source files that git status omits.
    skipped = {".git", "target", "node_modules", "dist", ".next", ".venv", "public"}
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if d not in skipped]
        for name in dirs:
            a._plain(Path(directory) / name, directory=True)
        for name in files:
            if name.endswith(".rs") or name in ("Cargo.toml", "Cargo.lock"):
                path = Path(directory) / name
                a._plain(path, directory=False)
                require(path.relative_to(root).as_posix() in tracked, "uncommitted or ignored compiler source")
    names = sorted(tracked)
    actual = git(root, "hash-object", "--no-filters", "--stdin-paths",
                 data="".join(name + "\n" for name in names).encode()).decode().splitlines()
    require(actual == [tracked[name] for name in names], "source bytes differ from committed blobs")
    return {"root": str(root), "revision": expected, "clean": True,
            "statusSha256": a.sha256(status), "sourceFilesChecked": len(names),
            "sourceInventorySha256": a.sha256(compact([[name, tracked[name]] for name in names]))}


def logical(path):
    a.relative_path(path)
    require(isinstance(path, str) and len(path) <= 230 and path.isascii(), "resource path bound")
    forbidden = {"exe", "dll", "com", "scr", "cpl", "msi", "msp", "bat", "cmd", "ps1", "psm1", "psd1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "reg", "lnk", "chm", "jar", "py", "pyw", "sh", "bash", "zsh", "fish"}
    require(not any(token in forbidden for part in path.lower().split('/') for token in part.split('.')[1:]), "code cannot be a resource")


def verify_catalog(catalog_bytes, objects_root):
    require(0 < len(catalog_bytes) <= MAX_CATALOG, "catalog byte bound")
    value = decoded(catalog_bytes)
    exact(value, {"schema", "resources", "packs"}, "catalog fields")
    require(value["schema"] == "mir2.native.resource-catalog.v1" and isinstance(value["resources"], dict)
            and len(value["resources"]) <= 300_000 and isinstance(value["packs"], list)
            and 0 < len(value["packs"]) <= 256, "catalog schema/count bound")
    root = a.absolute_directory(str(objects_root))
    packs, indexes, physical = {}, {}, 0
    for pack in value["packs"]:
        exact(pack, {"sha256", "size"}, "pack descriptor fields")
        sha = digest(pack["sha256"])
        size = uint(pack["size"], MAX_PACK, "pack size bound", True)
        require(sha not in packs and size >= 32, "duplicate/short pack")
        entry = file_entry(root / (sha.lower() + ".m2p"), "objects/packs/" + sha.lower() + ".m2p", MAX_PACK)
        require(entry.sha256 == sha and entry.size == size, "pack size/SHA mismatch")
        with a._open_entry(entry) as stream:
            header = stream.read(32)
            require(len(header) == 32, "truncated pack header")
            magic, version, count, index_bytes, payload = struct.unpack("<8sIIQQ", header)
            require(magic == b"M2PACK01" and version == 1 and 0 < count <= 300_000
                    and index_bytes == count * 64 and 32 + index_bytes + payload == size,
                    "pack header/count/length mismatch")
            data = stream.read(index_bytes)
            require(len(data) == index_bytes, "truncated pack index")
            cursor, prior, index = 32 + index_bytes, "", {}
            for offset in range(0, index_bytes, 64):
                raw_sha, position, length, stored, codec = struct.unpack("<32sQQQ8s", data[offset:offset + 64])
                item_sha = raw_sha.hex().upper()
                require(item_sha > prior and codec == b"\0" * 8 and position == cursor and stored == length,
                        "pack index order/duplicate/codec/overlap")
                cursor += length
                require(cursor <= size, "pack index extent out of bounds")
                index[item_sha] = (position, length)
                prior = item_sha
            require(cursor == size, "unindexed pack trailing bytes")
            # Independent full decoded-content verification, including retained
            # pack entries not visible in this generation's logical namespace.
            for item_sha, (_, length) in index.items():
                state, remaining = hashlib.sha256(), length
                while remaining:
                    block = stream.read(min(65536, remaining))
                    require(bool(block), "truncated pack payload")
                    remaining -= len(block)
                    state.update(block)
                require(state.hexdigest().upper() == item_sha, "pack entry content SHA mismatch")
            require(stream.read(1) == b"", "pack trailing data")
        packs[sha], indexes[sha] = entry, index
        physical += size
    require(physical <= MAX_TOTAL, "physical pack total bound")
    used, names, contents, logical_bytes = set(), set(), {}, 0
    for path, item in value["resources"].items():
        logical(path)
        require(path.lower() not in names, "case-colliding logical resource")
        names.add(path.lower())
        exact(item, {"sha256", "size", "packSha256", "offset", "storedSize", "codec"}, "resource reference fields")
        sha, pack_sha = digest(item["sha256"]), digest(item["packSha256"])
        size = uint(item["size"], MAX_PACK, "resource size")
        offset = uint(item["offset"], MAX_PACK, "resource offset")
        require(item["codec"] == "stored" and type(item["storedSize"]) is int
                and item["storedSize"] == size and pack_sha in packs
                and indexes[pack_sha].get(sha) == (offset, size), "resource/index locator mismatch")
        locator = (pack_sha, offset, size)
        require(sha not in contents or contents[sha] == locator, "inconsistent content alias")
        contents[sha] = locator
        used.add(pack_sha)
        logical_bytes += size
    for path in names:
        parts = path.split('/')
        require(not any('/'.join(parts[:i]) in names for i in range(1, len(parts))), "file/directory collision")
    require(used == set(packs) and logical_bytes <= MAX_TOTAL, "catalog exact closure/total bound")
    stats = {"resourceCount": len(value["resources"]), "uniquePayloads": len(contents),
             "logicalBytes": logical_bytes, "uniquePayloadBytes": sum(item[2] for item in contents.values()),
             "packCount": len(packs), "packBytes": physical}
    return value, packs, stats


def resource_binding(manifest_path, report_path, expected_report_sha, catalog):
    manifest_entry = file_entry(manifest_path, "PACKAGE-MANIFEST.json", a.MAX_META)
    report_entry = file_entry(report_path, "resource strict report", a.MAX_META)
    require(report_entry.sha256 == digest(expected_report_sha), "resource strict report pin mismatch")
    report = decoded(read(report_entry, a.MAX_META))
    require(report.get("schema") == "mir2.windows.package-verification.v4" and report.get("passed") is True
            and report.get("failures") == [] and report.get("detachedSignatureValid") is True
            and report.get("packageManifestSha256") == manifest_entry.sha256,
            "pinned resource report does not admit this manifest")
    manifest = decoded(read(manifest_entry, a.MAX_META))
    exact(manifest, {"schema", "coverage", "fileCount", "totalBytes", "aggregateSha256", "files"}, "resource manifest fields")
    require(manifest["schema"] == "mir2.windows.package-manifest.v4" and isinstance(manifest["files"], list)
            and type(manifest["fileCount"]) is int and 0 < manifest["fileCount"] <= 300_000
            and len(manifest["files"]) == manifest["fileCount"], "resource manifest schema/count")
    coverage = manifest["coverage"]
    exact(coverage, {"excludes", "rule"}, "resource manifest coverage fields")
    require(coverage["excludes"] == list(a.META) and isinstance(coverage["rule"], str)
            and bool(coverage["rule"]), "resource manifest coverage mismatch")
    require(isinstance(report.get("candidate"), str) and bool(report["candidate"]), "resource candidate identity")
    revision(report.get("currentSourceRevision"))
    expected, other, all_entries, cases = {}, [], {}, set()
    for item in manifest["files"]:
        exact(item, {"path", "size", "sha256"}, "resource source entry fields")
        a.game_path(item["path"])
        require(item["path"] not in a.META, "metadata cannot be source payload")
        require(item["path"].lower() not in cases, "resource source duplicate/case collision")
        cases.add(item["path"].lower())
        uint(item["size"], a.MAX_FILE, "resource source size")
        require(item["sha256"] == digest(item["sha256"]), "canonical source digest required")
        all_entries[item["path"]] = item
        if item["path"].startswith("mir2-assets/"):
            expected[item["path"][len("mir2-assets/"):]] = (item["size"], item["sha256"])
        else:
            other.append(item)
    canonical = ''.join(f"{path}\t{all_entries[path]['size']}\t{all_entries[path]['sha256']}\n" for path in sorted(all_entries)).encode('ascii')
    require(manifest["aggregateSha256"] == a.sha256(canonical)
            and type(manifest["totalBytes"]) is int and 0 <= manifest["totalBytes"] <= MAX_TOTAL
            and manifest["totalBytes"] == sum(item["size"] for item in all_entries.values()),
            "resource manifest aggregate/total mismatch")
    found = {path: (item["size"], digest(item["sha256"])) for path, item in catalog["resources"].items()}
    require(expected == found, "catalog differs from complete mir2-assets source inventory")
    return {"candidate": report["candidate"], "resourceSourceRevision": report.get("currentSourceRevision"),
            "manifestPath": str(manifest_entry.source), "manifestSha256": manifest_entry.sha256,
            "reportPath": str(report_entry.source), "reportSha256": report_entry.sha256,
            "contentProfile": report.get("contentProfileId", report.get("contentProfile")),
            "mapping": "all mir2-assets/ entries, strip prefix, exact logical size/SHA set",
            "nonResourcePayloads": sorted(other, key=lambda item: item["path"])}


def validate_manifest(value):
    exact(value, {"schema", "sequence", "minEngine", "gameSourceRevision", "engineSourceRevision", "protocol", "contentProfile", "exe", "signedConfig", "catalog", "packs"}, "generation manifest fields")
    require(value["schema"] == "mir2.windows.generation.v1" and value["minEngine"] == 2
            and type(value["minEngine"]) is int and value["protocol"] == "crystal-mir2-v1", "generation compatibility")
    uint(value["sequence"], MAX_SEQUENCE, "generation sequence", True)
    revision(value["gameSourceRevision"]); revision(value["engineSourceRevision"])
    require(isinstance(value["contentProfile"], str) and re.fullmatch(r"[A-Za-z0-9_]{1,64}", value["contentProfile"]), "content profile")
    for name, path, limit in [("exe", "mir2-platform-windows.exe", a.MAX_FILE), ("signedConfig", "mir2-client.toml", 32768), ("catalog", "catalog.json", MAX_CATALOG)]:
        item = value[name]
        exact(item, {"path", "size", "sha256"}, "control descriptor fields")
        require(item["path"] == path and item["sha256"] == digest(item["sha256"]), "control name/digest binding")
        uint(item["size"], limit, "control size bound", True)
    require(isinstance(value["packs"], list) and 0 < len(value["packs"]) <= 256, "generation pack count")
    seen, total = set(), 0
    for item in value["packs"]:
        exact(item, {"path", "size", "sha256"}, "generation pack fields")
        require(item["sha256"] == digest(item["sha256"])
                and item["path"] == "objects/packs/" + item["sha256"].lower() + ".m2p"
                and item["path"] not in seen, "generation pack name/digest/duplicate")
        seen.add(item["path"])
        total += uint(item["size"], MAX_PACK, "generation pack size", True)
    require(total <= MAX_TOTAL, "generation pack total")


def prepare(*, native_exe, source_root, source_revision, engine_source_root, engine_revision, config,
            catalog_path, objects_root, resource_manifest, resource_strict_report, resource_report_sha256,
            sequence, output_root, created_unix=None, expires_unix=None, content_profile=None):
    game_source = source_snapshot(source_root, source_revision)
    engine_source = source_snapshot(engine_source_root, engine_revision)
    uint(sequence, MAX_SEQUENCE, "sequence", True)
    created = int(time.time()) if created_unix is None else created_unix
    expires = created + 30 * 86400 if expires_unix is None else expires_unix
    uint(created, 2**63 - 1, "creation time", True)
    require(type(expires) is int and created < expires <= created + 31 * 86400, "feed lifetime")
    exe = file_entry(native_exe, "mir2-platform-windows.exe", a.MAX_FILE)
    with a._open_entry(exe) as stream:
        require(stream.read(2) == b"MZ", "native EXE must have MZ header")
    config_entry = file_entry(config, "mir2-client.toml", 32768)
    tomllib.loads(read(config_entry, 32768).decode("utf-8"))
    catalog_entry = file_entry(catalog_path, "catalog.json", MAX_CATALOG)
    catalog_bytes = read(catalog_entry, MAX_CATALOG)
    catalog, packs, stats = verify_catalog(catalog_bytes, objects_root)
    provenance = resource_binding(resource_manifest, resource_strict_report, resource_report_sha256, catalog)
    profile = provenance["contentProfile"] if content_profile is None else content_profile
    require(profile == provenance["contentProfile"], "resource content profile differs from admitted report")
    manifest = {"schema": "mir2.windows.generation.v1", "sequence": sequence, "minEngine": 2,
                "gameSourceRevision": source_revision, "engineSourceRevision": engine_revision,
                "protocol": "crystal-mir2-v1", "contentProfile": profile,
                "exe": entry_dict(exe), "signedConfig": entry_dict(config_entry), "catalog": entry_dict(catalog_entry),
                "packs": [entry_dict(packs[sha]) for sha in sorted(packs)]}
    validate_manifest(manifest)
    manifest_bytes = compact(manifest)
    require(len(manifest_bytes) <= 4 * 1024**2, "generation metadata bound")
    manifest_sha = a.sha256(manifest_bytes)
    output = Path(output_root)
    require(output.is_absolute() and not os.path.lexists(output), "fresh absolute output root required")
    a.absolute_directory(str(output.parent))
    output.mkdir()
    (output / "objects").mkdir(); (output / "objects/packs").mkdir()
    (output / "generations").mkdir()
    directory = "generations/" + manifest_sha.lower()
    (output / directory).mkdir()
    for entry in packs.values():
        copy_new(entry, output / entry.path)
    for entry in (exe, config_entry, catalog_entry):
        copy_new(entry, output / directory / entry.path)
    put(output / directory / "GENERATION.json", manifest_bytes)
    draft = {"schema": "mir2.windows.pack-feed-draft.v1", "channel": "invited", "platform": "windows-x64",
             "sequence": sequence, "createdUnix": created, "expiresUnix": expires, "minEngine": 2,
             "manifest": {"path": directory + "/GENERATION.json", "size": len(manifest_bytes), "sha256": manifest_sha},
             "signaturePath": directory + "/GENERATION.p7s"}
    put(output / "FEED-DRAFT.json", compact(draft))
    proof = {"schema": PREPARED, "unsigned": True, "cmsVerified": False, "published": False,
             "sourceClean": True, "gameSource": game_source, "engineSource": engine_source,
             "manifestSha256": manifest_sha, "generationDirectory": directory,
             "feedDraftSha256": a.sha256(compact(draft)), "createdUnix": created, "expiresUnix": expires,
             "inputBindings": [{**entry_dict(entry), "source": str(entry.source)} for entry in (exe, config_entry, catalog_entry)],
             "resourceProvenance": provenance, "stats": stats,
             "artifactBinding": "exact EXE/config/catalog SHA and clean declared source; reproducible compilation is not certified"}
    put(output / "PREPARED.json", compact(proof))
    verify_draft(output)
    return proof


def verify_draft(output_root):
    root = a.absolute_directory(str(output_root))
    proof = decoded(read(file_entry(root / "PREPARED.json", "PREPARED.json", 4 * 1024**2), 4 * 1024**2))
    exact(proof, {"schema", "unsigned", "cmsVerified", "published", "sourceClean", "gameSource", "engineSource", "manifestSha256", "generationDirectory", "feedDraftSha256", "createdUnix", "expiresUnix", "inputBindings", "resourceProvenance", "stats", "artifactBinding"}, "preparation proof fields")
    require(proof["schema"] == PREPARED and proof["unsigned"] is True and proof["cmsVerified"] is False
            and proof["published"] is False and proof["sourceClean"] is True, "unsigned preparation provenance required")
    require(proof["generationDirectory"] == "generations/" + digest(proof["manifestSha256"]).lower(), "generation directory digest binding")
    for field in ("gameSource", "engineSource"):
        snapshot = proof[field]
        require(source_snapshot(snapshot["root"], snapshot["revision"]) == snapshot, "source snapshot changed")
    directory = root / proof["generationDirectory"]
    manifest_entry = file_entry(directory / "GENERATION.json", "GENERATION.json", 4 * 1024**2)
    require(manifest_entry.sha256 == proof["manifestSha256"], "generation manifest changed")
    manifest = decoded(read(manifest_entry, 4 * 1024**2))
    validate_manifest(manifest)
    require(manifest["gameSourceRevision"] == proof["gameSource"]["revision"]
            and manifest["engineSourceRevision"] == proof["engineSource"]["revision"], "source/artifact revision binding")
    for field in ("exe", "signedConfig", "catalog"):
        item = manifest[field]
        entry = file_entry(directory / item["path"], item["path"], item["size"])
        require(entry_dict(entry) == item, "generation control bytes differ")
    require(len(proof["inputBindings"]) == 3, "input binding set")
    for binding, field in zip(proof["inputBindings"], ("exe", "signedConfig", "catalog")):
        exact(binding, {"path", "size", "sha256", "source"}, "input binding fields")
        require({k: binding[k] for k in ("path", "size", "sha256")} == manifest[field], "input/control binding")
        actual = file_entry(binding["source"], binding["path"], binding["size"])
        require(entry_dict(actual) == manifest[field], "original input changed after snapshot")
    catalog_bytes = read(file_entry(directory / "catalog.json", "catalog.json", MAX_CATALOG), MAX_CATALOG)
    catalog, packs, stats = verify_catalog(catalog_bytes, root / "objects/packs")
    require(manifest["packs"] == [entry_dict(packs[sha]) for sha in sorted(packs)] and stats == proof["stats"], "generation/catalog closure or stats mismatch")
    provenance = proof["resourceProvenance"]
    require(resource_binding(provenance["manifestPath"], provenance["reportPath"], provenance["reportSha256"], catalog) == provenance,
            "resource provenance changed")
    require(manifest["contentProfile"] == provenance["contentProfile"], "generation resource profile mismatch")
    draft_bytes = read(file_entry(root / "FEED-DRAFT.json", "FEED-DRAFT.json", 32768), 32768)
    require(a.sha256(draft_bytes) == proof["feedDraftSha256"], "feed draft changed")
    draft = decoded(draft_bytes)
    exact(draft, {"schema", "channel", "platform", "sequence", "createdUnix", "expiresUnix", "minEngine", "manifest", "signaturePath"}, "feed draft fields")
    require(draft == {"schema": "mir2.windows.pack-feed-draft.v1", "channel": "invited", "platform": "windows-x64",
            "sequence": manifest["sequence"], "createdUnix": proof["createdUnix"], "expiresUnix": proof["expiresUnix"], "minEngine": 2,
            "manifest": {"path": proof["generationDirectory"] + "/GENERATION.json", "size": manifest_entry.size, "sha256": manifest_entry.sha256},
            "signaturePath": proof["generationDirectory"] + "/GENERATION.p7s"}, "feed draft exact binding")
    uint(draft["createdUnix"], 2**63 - 1, "draft creation", True)
    require(type(draft["expiresUnix"]) is int and 0 < draft["expiresUnix"] - draft["createdUnix"] <= 31 * 86400, "draft expiry bound")
    return proof, manifest, draft


def finalize_feed(output_root, expected_manifest_sha):
    proof, _, draft = verify_draft(output_root)
    require(proof["manifestSha256"] == digest(expected_manifest_sha), "reviewed manifest pin mismatch")
    signature = file_entry(Path(output_root) / draft["signaturePath"], draft["signaturePath"], 32768)
    feed = {key: value for key, value in draft.items() if key not in ("signaturePath",)}
    feed["schema"] = "mir2.windows.pack-feed.v1"
    feed["signature"] = entry_dict(signature)
    put(Path(output_root) / "latest.json", compact(feed))
    return feed


def verify_release(output_root, expected_manifest_sha=None, now=None):
    proof, manifest, draft = verify_draft(output_root)
    if expected_manifest_sha is not None:
        require(proof["manifestSha256"] == digest(expected_manifest_sha), "reviewed manifest pin mismatch")
    root = Path(output_root)
    feed_entry = file_entry(root / "latest.json", "latest.json", 32768)
    feed = decoded(read(feed_entry, 32768))
    exact(feed, {"schema", "channel", "platform", "sequence", "createdUnix", "expiresUnix", "minEngine", "manifest", "signature"}, "pack feed fields")
    signature = file_entry(root / draft["signaturePath"], draft["signaturePath"], 32768)
    expected = {key: value for key, value in draft.items() if key != "signaturePath"}
    expected.update(schema="mir2.windows.pack-feed.v1", signature=entry_dict(signature))
    require(feed == expected, "signed feed object/draft binding")
    now = int(time.time()) if now is None else now
    require(feed["createdUnix"] <= now + 300 and feed["expiresUnix"] >= now, "expired/future release")
    feed_signature = file_entry(root / "latest.p7s", "latest.p7s", 32768)
    return {"schema": "mir2.windows.packed-generation-byte-verification.v1", "byteVerified": True,
            "cmsVerified": False, "metadataOnly": False, "sourceClean": True,
            "manifestSha256": proof["manifestSha256"], "generationDirectory": proof["generationDirectory"],
            "gameSourceRevision": manifest["gameSourceRevision"], "engineSourceRevision": manifest["engineSourceRevision"],
            "sequence": manifest["sequence"], "createdUnix": feed["createdUnix"],
            "manifest": feed["manifest"], "manifestSignature": feed["signature"],
            "feed": entry_dict(feed_entry), "feedSignature": entry_dict(feed_signature),
            "catalogSha256": manifest["catalog"]["sha256"], **proof["stats"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("prepare")
    for name in ("native-exe", "source-root", "engine-source-root", "config", "catalog-path", "objects-root", "resource-manifest", "resource-strict-report", "output-root"):
        p.add_argument("--" + name, type=Path, required=True)
    for name in ("source-revision", "engine-revision", "resource-report-sha256"):
        p.add_argument("--" + name, required=True)
    p.add_argument("--sequence", type=int, required=True)
    p.add_argument("--created-unix", type=int)
    p.add_argument("--expires-unix", type=int)
    p.add_argument("--content-profile")
    for name in ("verify-draft", "finalize-feed", "verify-release"):
        p = commands.add_parser(name)
        p.add_argument("--output-root", type=Path, required=True)
        if name != "verify-draft":
            p.add_argument("--expected-manifest-sha", required=True)
    args = vars(parser.parse_args())
    command = args.pop("command")
    try:
        if command == "prepare": result = prepare(**args)
        elif command == "verify-draft": result = verify_draft(**args)[0]
        elif command == "finalize-feed": result = finalize_feed(**args)
        else: result = verify_release(**args)
        print(json.dumps(result, separators=(",", ":")))
        return 0
    except Exception as error:
        print(json.dumps({"passed": False, "failure": str(error)}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
