#!/usr/bin/env python3
"""Small offline fixtures. No game execution, networking or publication.

The fixture's explicit report pin is synthetic; it is never represented as the
production Candidate28 admission. PowerShell separately tests real pinned CMS.
"""
from __future__ import annotations
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import unittest

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("prepare_packed_fixture", Path(__file__).with_name("prepare-packed-generation.py"))
p = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = p
SPEC.loader.exec_module(p)


def sha(data):
    return hashlib.sha256(data).hexdigest().upper()


def write_json(path, value):
    path.write_bytes(p.compact(value))


class Fixture:
    """All inputs/outputs lie in a caller-owned fresh temporary directory."""
    def __init__(self, root):
        self.root = Path(root)
        self.source = self.root / "source"
        self.source.mkdir()
        (self.source / "src").mkdir()
        (self.source / "Cargo.toml").write_text('[package]\nname="signed-fixture"\nversion="0.0.0"\n', encoding="utf-8")
        (self.source / "src/lib.rs").write_text('pub fn fixture() -> bool { true }\n', encoding="utf-8")
        (self.source / ".gitignore").write_text("ignored.rs\n", encoding="utf-8")
        self.git("init", "--quiet")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "add", ".")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "synthetic source fixture")
        self.rev = self.git("rev-parse", "HEAD").decode().strip()
        self.objects = self.root / "input-objects"
        self.objects.mkdir()
        self.exe = self.root / "native-fixture.exe"
        self.exe.write_bytes(b"MZ\0synthetic fixture: NEVER EXECUTE\r\n")
        self.config = self.root / "mir2-client.toml"
        self.config.write_bytes(b'[network]\nendpoint="wss://fixture.invalid"\n')
        self.contents = {"images/a.png": b"PNG fixture bytes\x00", "images/alias.png": b"PNG fixture bytes\x00", "maps/a.map.gz": b"gzip fixture original bytes\0\xff"}
        payloads = {sha(data): data for data in self.contents.values()}
        cursor = 32 + 64 * len(payloads)
        index, body, extents = bytearray(), bytearray(), {}
        for key, data in sorted(payloads.items()):
            index += struct.pack("<32sQQQ8s", bytes.fromhex(key), cursor, len(data), len(data), b"\0" * 8)
            extents[key] = cursor
            body += data
            cursor += len(data)
        self.pack_bytes = struct.pack("<8sIIQQ", b"M2PACK01", 1, len(payloads), len(index), len(body)) + index + body
        self.pack_sha = sha(self.pack_bytes)
        self.pack_path = self.objects / (self.pack_sha.lower() + ".m2p")
        self.pack_path.write_bytes(self.pack_bytes)
        self.catalog_value = {"schema": "mir2.native.resource-catalog.v1", "resources": {
            key: {"sha256": sha(data), "size": len(data), "packSha256": self.pack_sha, "offset": extents[sha(data)], "storedSize": len(data), "codec": "stored"}
            for key, data in sorted(self.contents.items())}, "packs": [{"sha256": self.pack_sha, "size": len(self.pack_bytes)}]}
        self.catalog = self.root / "catalog.json"
        write_json(self.catalog, self.catalog_value)
        files = [{"path": "mir2-assets/" + key, "size": len(data), "sha256": sha(data)} for key, data in sorted(self.contents.items())]
        files += [{"path": "mir2-platform-windows.exe", "size": self.exe.stat().st_size, "sha256": sha(self.exe.read_bytes())},
                  {"path": "README-START.txt", "size": 7, "sha256": sha(b"fixture")}]
        self.manifest_value = {"schema": "mir2.windows.package-manifest.v4", "coverage": {"excludes": list(p.a.META), "rule": "all payloads except signed control metadata"},
                               "fileCount": len(files), "totalBytes": sum(x["size"] for x in files), "aggregateSha256": "", "files": files}
        self.manifest = self.root / "PACKAGE-MANIFEST.json"
        self.report = self.root / "synthetic-resource-admission.json"
        self.update_resource_proof()
        self.created = int(time.time())

    def git(self, *args):
        result = subprocess.run(["git", "-c", "core.autocrlf=false", "-C", str(self.source), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        if result.returncode:
            raise RuntimeError("fixture Git command failed: " + result.stderr.decode(errors="replace"))
        return result.stdout

    def update_resource_proof(self):
        entries = sorted(self.manifest_value["files"], key=lambda x: x["path"])
        self.manifest_value["fileCount"] = len(entries)
        self.manifest_value["totalBytes"] = sum(x["size"] for x in entries)
        self.manifest_value["aggregateSha256"] = sha(''.join(f"{x['path']}\t{x['size']}\t{x['sha256']}\n" for x in entries).encode("ascii"))
        write_json(self.manifest, self.manifest_value)
        write_json(self.report, {"schema": "mir2.windows.package-verification.v4", "candidate": "SYNTHETIC-NOT-CANDIDATE28", "passed": True, "failures": [], "detachedSignatureValid": True,
                                 "packageManifestSha256": sha(self.manifest.read_bytes()), "currentSourceRevision": "a" * 40, "contentProfileId": "fixture"})
        self.report_sha = sha(self.report.read_bytes())

    def arguments(self, output="packed"):
        return dict(native_exe=self.exe, source_root=self.source, source_revision=self.rev, engine_source_root=self.source, engine_revision=self.rev,
                    config=self.config, catalog_path=self.catalog, objects_root=self.objects, resource_manifest=self.manifest,
                    resource_strict_report=self.report, resource_report_sha256=self.report_sha, sequence=27,
                    output_root=self.root / output, created_unix=self.created)

    def prepare(self, output="packed", **changes):
        args = self.arguments(output)
        args.update(changes)
        return p.prepare(**args)


def unsigned_baseline(fixture_root, revision, created_unix):
    """Local integration helper; dummy executables are never executed."""
    root = Path(fixture_root) / "baseline"
    game = root / "game"
    game.mkdir(parents=True)
    exe = b"MZ\0legacy synthetic game: NEVER EXECUTE"
    launcher = b"MZ\0synthetic launcher: NEVER EXECUTE"
    engine = b"MZ\0synthetic version2 engine: NEVER EXECUTE"
    attest = p.compact({"synthetic": True, "neverExecute": True})
    (game / "mir2-platform-windows.exe").write_bytes(exe)
    (game / "BUILD-ATTESTATION.json").write_bytes(attest)
    (root / "Mir2Launcher.exe").write_bytes(launcher)
    files = [{"path": name, "size": len(data), "sha256": sha(data)} for name, data in sorted({"mir2-platform-windows.exe": exe, "BUILD-ATTESTATION.json": attest}.items())]
    aggregate = sha(''.join(f"{x['path']}\t{x['size']}\t{x['sha256']}\n" for x in files).encode("ascii"))
    manifest = {"schema": "mir2.windows.package-manifest.v4", "coverage": {"excludes": list(p.a.META), "rule": "complete synthetic payload"},
                "files": files, "fileCount": len(files), "totalBytes": sum(x["size"] for x in files), "aggregateSha256": aggregate}
    write_json(game / "PACKAGE-MANIFEST.json", manifest)
    manifest_sha = sha((game / "PACKAGE-MANIFEST.json").read_bytes())
    version = {"schema": "mir2.windows.candidate-version.v4", "candidate": "SYNTHETIC-LEGACY-BASELINE", "gitRevision": revision,
               "worktreeDirty": False, "clientOnly": True, "exeName": "mir2-platform-windows.exe", "packageManifestSha256": manifest_sha,
               "packageManifestAggregateSha256": aggregate, "packageManifestFileCount": 2, "packageFileCount": 6, "exeSha256": sha(exe), "exeSizeBytes": len(exe), "buildAttestationSha256": sha(attest)}
    write_json(game / "VERSION.json", version)
    statement = {"schema": "mir2.windows.release-statement.v1", "candidate": version["candidate"], "gitRevision": revision, "worktreeDirty": False,
                 "versionSha256": sha((game / "VERSION.json").read_bytes()), "packageManifestSha256": manifest_sha,
                 "packageManifestAggregateSha256": aggregate, "exeSha256": sha(exe), "buildAttestationSha256": sha(attest)}
    write_json(game / "RELEASE-STATEMENT.json", statement)
    engine_dir = root / "updater/engines" / sha(engine)
    engine_dir.mkdir(parents=True)
    (engine_dir / "Mir2Updater.exe").write_bytes(engine)
    write_json(engine_dir / "ENGINE.json", {"schema": "mir2.windows.updater-engine.v1", "version": 2, "minBootstrap": 1,
        "sourceRevision": revision, "builtUnix": created_unix, "exeSize": len(engine), "exeSha256": sha(engine), "launcherSha256": sha(launcher), "cargoLockSha256": sha(b"synthetic Cargo.lock")})
    (root / "updater/active.txt").write_text(sha(engine), encoding="ascii")
    return root, engine_dir


def baseline_seed(root, engine_dir, created_unix):
    root, engine_dir = Path(root), Path(engine_dir)
    def descriptor(path):
        data = path.read_bytes()
        return {"path": path.name, "size": len(data), "sha256": sha(data)}
    value = {"schema": "mir2.windows.update-feed.v1", "channel": "invited", "platform": "windows-x64", "sequence": 25,
             "createdUnix": created_unix, "expiresUnix": created_unix + 30 * 86400, "minBootstrap": 1, "protocol": "crystal-mir2-v1", "content": "mir2.windows.package-manifest.v4",
             "game": {"directory": "releases/synthetic-legacy", "identity": "SYNTHETIC-LEGACY-BASELINE", "metadata": [descriptor(root / "game" / name) for name in p.a.META]},
             "engine": {"directory": "releases/synthetic-engine", "identity": "2", "metadata": [descriptor(engine_dir / name) for name in ("ENGINE.json", "ENGINE.p7s")]}}
    p.put(root / "updater/seed-feed.json", p.compact(value))


class PackedPreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mir2-packed-tools-fixture-")
        self.addCleanup(self.temp.cleanup)
        self.f = Fixture(Path(self.temp.name))

    def test_repeatable_complete_inventory_and_separate_provenance(self):
        a, b = self.f.prepare("one"), self.f.prepare("two")
        self.assertEqual(a["manifestSha256"], b["manifestSha256"])
        self.assertEqual(a["stats"], dict(resourceCount=3, uniquePayloads=2, logicalBytes=sum(map(len, self.f.contents.values())),
                                           uniquePayloadBytes=sum(map(len, set(self.f.contents.values()))), packCount=1, packBytes=len(self.f.pack_bytes)))
        self.assertEqual(a["gameSource"]["revision"], self.f.rev)
        self.assertEqual(a["resourceProvenance"]["resourceSourceRevision"], "a" * 40)
        self.assertFalse(a["cmsVerified"])
        self.assertFalse((self.f.root / "one/latest.json").exists())
        self.assertEqual([x["path"] for x in a["resourceProvenance"]["nonResourcePayloads"]], ["README-START.txt", "mir2-platform-windows.exe"])

    def test_fresh_outputs_and_copy_no_overwrite(self):
        proof = self.f.prepare()
        with self.assertRaises(ValueError): self.f.prepare()
        target = self.f.root / "copy.bin"
        entry = p.file_entry(self.f.exe, "exe", 1024)
        p.copy_new(entry, target)
        p.copy_new(entry, target)
        target.write_bytes(b"different")
        with self.assertRaises(ValueError): p.copy_new(entry, target)
        self.assertEqual(target.read_bytes(), b"different")
        self.assertTrue(proof["unsigned"])

    def test_missing_resource_and_extra_resource_cannot_pass(self):
        for missing in (True, False):
            value = copy.deepcopy(self.f.catalog_value)
            if missing: del value["resources"]["images/alias.png"]
            else: value["resources"]["images/extra.png"] = value["resources"]["images/a.png"].copy()
            write_json(self.f.catalog, value)
            with self.subTest(missing=missing), self.assertRaisesRegex(ValueError, "complete mir2-assets"):
                self.f.prepare()

    def test_wrong_report_pin_or_inexact_coverage_rejected(self):
        with self.assertRaisesRegex(ValueError, "report pin"):
            self.f.prepare(resource_report_sha256="0" * 64)
        self.f.manifest_value["coverage"]["excludes"] = []
        self.f.update_resource_proof()
        with self.assertRaisesRegex(ValueError, "coverage"):
            self.f.prepare()

    def test_dirty_untracked_and_ignored_compiler_sources(self):
        for path in ("extra.rs", "ignored.rs"):
            item = self.f.source / path
            item.write_text("fn injected(){}")
            with self.subTest(path=path), self.assertRaises(ValueError): self.f.prepare()
            item.unlink()
        self.f.git("update-index", "--assume-unchanged", "src/lib.rs")
        (self.f.source / "src/lib.rs").write_text("pub fn hidden_mutation() {}\n")
        with self.assertRaisesRegex(ValueError, "committed blobs"):
            self.f.prepare()

    def test_wrong_revision_and_non_native_exe(self):
        with self.assertRaisesRegex(ValueError, "HEAD"):
            self.f.prepare(source_revision="b" * 40)
        self.f.exe.write_bytes(b"not an executable")
        with self.assertRaisesRegex(ValueError, "MZ"):
            self.f.prepare()

    def test_invalid_config_and_mutated_original_after_snapshot(self):
        self.f.config.write_bytes(b"broken [")
        with self.assertRaises(ValueError): self.f.prepare()
        self.f.config.write_bytes(b"fixture=true\n")
        self.f.prepare()
        self.f.exe.write_bytes(b"MZ different original")
        with self.assertRaises(ValueError): p.verify_draft(self.f.root / "packed")

    def test_pack_and_catalog_mutations_rejected(self):
        bad = bytearray(self.f.pack_bytes)
        bad[-1] ^= 1
        self.f.pack_path.write_bytes(bad)
        with self.assertRaisesRegex(ValueError, "pack size/SHA"):
            self.f.prepare()
        self.f.pack_path.write_bytes(self.f.pack_bytes)
        value = copy.deepcopy(self.f.catalog_value)
        value["resources"]["images/a.png"]["offset"] += 1
        with self.assertRaisesRegex(ValueError, "locator"):
            p.verify_catalog(p.compact(value), self.f.objects)
        value = copy.deepcopy(self.f.catalog_value)
        value["resources"]["images/A.png"] = value["resources"]["images/a.png"].copy()
        with self.assertRaisesRegex(ValueError, "case-colliding"):
            p.verify_catalog(p.compact(value), self.f.objects)
        with self.assertRaisesRegex(ValueError, "duplicate JSON"):
            p.decoded(b'{"schema":1,"schema":2}')

    def test_pack_self_consistent_digest_still_checks_content(self):
        data = bytearray(self.f.pack_bytes)
        data[-1] ^= 1
        key = sha(data)
        (self.f.objects / (key.lower() + ".m2p")).write_bytes(data)
        value = copy.deepcopy(self.f.catalog_value)
        value["packs"][0] = {"sha256": key, "size": len(data)}
        for item in value["resources"].values(): item["packSha256"] = key
        with self.assertRaisesRegex(ValueError, "content SHA"):
            p.verify_catalog(p.compact(value), self.f.objects)

    def test_self_consistent_bad_index_rejected(self):
        for offset, replacement in ((16, 1), (64, 0), (88, 1)):
            data = bytearray(self.f.pack_bytes)
            struct.pack_into("<Q", data, offset, replacement)
            key = sha(data)
            (self.f.objects / (key.lower() + ".m2p")).write_bytes(data)
            value = copy.deepcopy(self.f.catalog_value)
            value["packs"][0] = {"sha256": key, "size": len(data)}
            for item in value["resources"].values(): item["packSha256"] = key
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                p.verify_catalog(p.compact(value), self.f.objects)

    def test_limits_unknown_fields_and_byteproof_never_claims_cms(self):
        for changes in ({"sequence": True}, {"sequence": 0}, {"expires_unix": self.f.created + 32 * 86400}):
            with self.subTest(changes=changes), self.assertRaises(ValueError): self.f.prepare(**changes)
        proof = self.f.prepare()
        root = self.f.root / "packed"
        (root / proof["generationDirectory"] / "GENERATION.p7s").write_bytes(b"deliberately NOT a CMS signature")
        p.finalize_feed(root, proof["manifestSha256"])
        (root / "latest.p7s").write_bytes(b"also NOT a CMS signature")
        result = p.verify_release(root, proof["manifestSha256"], now=self.f.created)
        self.assertTrue(result["byteVerified"])
        self.assertFalse(result["cmsVerified"])
        with self.assertRaises(FileExistsError): p.finalize_feed(root, proof["manifestSha256"])
        with self.assertRaises(ValueError): p.verify_release(root, "0" * 64)
        with self.assertRaises(ValueError): p.verify_release(root, now=self.f.created + 32 * 86400)
        feed = json.loads((root / "latest.json").read_bytes())
        feed["unexpected"] = True
        write_json(root / "latest.json", feed)
        with self.assertRaisesRegex(ValueError, "feed fields"):
            p.verify_release(root, proof["manifestSha256"])

    @unittest.skipUnless(os.name == "nt", "actual Windows hardlink check")
    def test_hardlinked_control_rejected(self):
        other = self.f.root / "another-exe"
        os.link(self.f.exe, other)
        with self.assertRaises(ValueError): self.f.prepare()


if __name__ == "__main__":
    unittest.main(verbosity=2)
