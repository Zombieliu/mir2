#!/usr/bin/env python3
"""Small real-filesystem release/archive regressions; no live releases or CMS keys."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name("archive-update-release.py")
SPEC = importlib.util.spec_from_file_location("mir2_archive_update_release", SCRIPT)
archive = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = archive
SPEC.loader.exec_module(archive)


def dumped(value):
    return json.dumps(value, separators=(",", ":")).encode("ascii")


def fixture_candidate(root):
    root.mkdir()
    exe = b"MZ fixture game executable; never executed"
    dirty_hash = archive.sha256(b"clean-worktree-fixture")
    build = {"schema": "mir2.windows.build-attestation.v2", "exeSha256": archive.sha256(exe),
             "exeSizeBytes": len(exe), "gitRevision": "a" * 40, "worktreeDirty": False,
             "worktreeStatusSha256": dirty_hash}
    payload = {"mir2-platform-windows.exe": exe, "BUILD-ATTESTATION.json": dumped(build),
               "README-START.txt": b"fixture instructions\n", "mir2-assets/nested/data.txt": b"exact asset bytes\x00\xff"}
    for name, data in payload.items():
        file = root / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(data)
    entries = [{"path": name, "size": len(data), "sha256": archive.sha256(data)} for name, data in sorted(payload.items())]
    canonical = "".join(f"{e['path']}\t{e['size']}\t{e['sha256']}\n" for e in entries)
    aggregate = archive.sha256(canonical.encode("ascii"))
    manifest = {"schema": "mir2.windows.package-manifest.v4", "coverage": {"excludes": list(archive.META), "rule": "Payload hashes; detached signature binds metadata."},
                "fileCount": len(entries), "totalBytes": sum(e["size"] for e in entries), "aggregateSha256": aggregate, "files": entries}
    manifest_bytes = dumped(manifest)
    version = {"schema": "mir2.windows.candidate-version.v4", "candidate": "WN-CANDIDATE-fixture-06",
               "gitRevision": "a" * 40, "worktreeDirty": False, "clientOnly": True,
               "worktreeStatusSha256": dirty_hash, "exeName": "mir2-platform-windows.exe",
               "exeSha256": archive.sha256(exe), "exeSizeBytes": len(exe),
               "packageManifestSchema": manifest["schema"], "packageManifestSha256": archive.sha256(manifest_bytes),
               "packageManifestAggregateSha256": aggregate, "packageManifestFileCount": len(entries),
               "packageFileCount": len(entries) + 4, "buildAttestationSha256": archive.sha256(payload["BUILD-ATTESTATION.json"]),
               "releaseStatementSchema": "mir2.windows.release-statement.v1", "signatureFormat": "CMS/PKCS7-detached"}
    version_bytes = dumped(version)
    statement = {"schema": "mir2.windows.release-statement.v1", "candidate": version["candidate"],
                 "exeSha256": version["exeSha256"], "packageManifestSha256": version["packageManifestSha256"],
                 "packageManifestAggregateSha256": aggregate, "versionSha256": archive.sha256(version_bytes),
                 "buildAttestationSha256": version["buildAttestationSha256"], "gitRevision": version["gitRevision"],
                 "worktreeDirty": False, "worktreeStatusSha256": dirty_hash}
    for name, data in {"PACKAGE-MANIFEST.json": manifest_bytes, "VERSION.json": version_bytes,
                       "RELEASE-STATEMENT.json": dumped(statement), "RELEASE-STATEMENT.p7s": b"NOT A CMS SIGNATURE; fixture only"}.items():
        (root / name).write_bytes(data)
    return root


def fixture_engine(root):
    root.mkdir()
    exe = b"MZ fixture updater executable; never executed"
    engine = {"schema": "mir2.windows.updater-engine.v1", "version": 1, "minBootstrap": 1,
              "sourceRevision": "b" * 40, "builtUnix": 1_790_785_200, "exeSize": len(exe),
              "exeSha256": archive.sha256(exe), "launcherSha256": archive.sha256(b"launcher fixture"),
              "cargoLockSha256": archive.sha256(b"Cargo.lock fixture")}
    (root / "ENGINE.json").write_bytes(dumped(engine))
    (root / "ENGINE.p7s").write_bytes(b"NOT A CMS SIGNATURE; fixture only")
    (root / "Mir2Updater.exe").write_bytes(exe)
    return root


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mir2-update-archive-test-")
        self.root = Path(self.temporary.name)
        self.candidate = fixture_candidate(self.root / "candidate")
        self.engine = fixture_engine(self.root / "engine")
        self.mapping = {"releases/game-r6": str(self.candidate), "releases/updater-v1": str(self.engine)}
        self.output = self.root / "release.tar.gz"

    def tearDown(self):
        # Cleanup applies only to this fixture's verified absolute temporary root.
        assert self.root.is_absolute() and self.root.name.startswith("mir2-update-archive-test-")
        assert self.root.parent.resolve() == Path(tempfile.gettempdir()).resolve()
        self.temporary.cleanup()

    def reject(self, mapping=None):
        with self.assertRaises(archive.ValidationError):
            archive.build_archive(self.mapping if mapping is None else mapping, self.output)
        self.assertFalse(self.output.exists(), "validation must finish before output creation")

    def edit_json(self, root, name, mutate):
        value = json.loads((root / name).read_bytes())
        mutate(value)
        (root / name).write_bytes(dumped(value))

    def test_valid_archive_exact_bytes_and_receipt(self):
        receipt = archive.build_archive(self.mapping, self.output)
        self.assertEqual(receipt["fileCount"], 11)
        self.assertFalse(receipt["cmsVerified"])
        self.assertTrue(receipt["requiresExternalSignatureVerification"])
        self.assertEqual(receipt["archiveSha256"], archive.sha256(self.output.read_bytes()))
        self.assertEqual(receipt, json.loads(Path(str(self.output) + ".receipt.json").read_bytes()))
        self.assertEqual({c["kind"] for c in receipt["components"]}, {"game", "engine"})
        self.assertEqual(receipt["components"][1]["engineIdentitySha256"], archive.sha256((self.engine / "Mir2Updater.exe").read_bytes()))
        actual = {}
        with tarfile.open(self.output, "r:gz") as packaged:
            for member in packaged:
                self.assertTrue(member.isfile() or member.isdir())
                self.assertFalse(member.pax_headers)
                self.assertEqual((member.uid, member.gid, member.uname, member.gname, member.mtime), (0, 0, "", "", 0))
                if member.isfile():
                    actual[member.name] = packaged.extractfile(member).read()
                    prefix = next(p for p in self.mapping if member.name.startswith(p + "/"))
                    self.assertEqual(actual[member.name], (Path(self.mapping[prefix]) / member.name[len(prefix) + 1:]).read_bytes())
        self.assertEqual(set(actual), {e["path"] for e in receipt["files"]})
        self.assertEqual(sum(map(len, actual.values())), receipt["uncompressedBytes"])

    def test_deterministic_plain_ustar_gzip(self):
        archive.build_archive(self.mapping, self.output)
        second = self.root / "different-name.tar.gz"
        archive.build_archive(self.mapping, second)
        self.assertEqual(self.output.read_bytes(), second.read_bytes())

    def test_existing_output_preserved(self):
        self.output.write_bytes(b"existing release")
        with self.assertRaisesRegex(archive.ValidationError, "already exists"):
            archive.build_archive(self.mapping, self.output)
        self.assertEqual(self.output.read_bytes(), b"existing release")

    def test_existing_receipt_preserved(self):
        receipt = Path(str(self.output) + ".receipt.json")
        receipt.write_bytes(b"existing receipt")
        self.reject()
        self.assertEqual(receipt.read_bytes(), b"existing receipt")

    def test_unknown_release_directory_rejected(self):
        other = self.root / "arbitrary"
        other.mkdir()
        (other / "file.txt").write_text("not a release")
        self.reject({"releases/arbitrary": str(other)})

    def test_mapping_traversal_devices_and_aliases_rejected(self):
        for prefix in ["../release", "/release", "releases/../x", "releases/a:b", "releases/a\\b", "releases/NUL", "releases/a.", "latest.json", "releases/a//b"]:
            with self.subTest(prefix=prefix):
                self.reject({prefix: str(self.candidate)})

    def test_case_colliding_mapping_rejected(self):
        self.reject({"releases/game": str(self.candidate), "releases/GAME": str(self.engine)})

    def test_overlapping_mapping_rejected(self):
        self.reject({"releases/game": str(self.candidate), "releases/game/engine": str(self.engine)})

    def test_relative_source_rejected(self):
        self.reject({"releases/game": "relative/candidate"})

    def test_output_inside_source_rejected(self):
        with self.assertRaisesRegex(archive.ValidationError, "outside every source"):
            archive.build_archive(self.mapping, self.candidate / "release.tar.gz")
        self.assertFalse((self.candidate / "release.tar.gz").exists())

    def test_normalized_output_inside_source_rejected(self):
        output = self.candidate / "mir2-assets" / ".." / "release.tar.gz"
        with self.assertRaisesRegex(archive.ValidationError, "outside every source"):
            archive.build_archive(self.mapping, output)
        self.assertFalse(output.exists())

    def test_file_count_and_total_bounds(self):
        for name, value in [("MAX_FILES", 1), ("MAX_TOTAL", 8), ("MAX_ENTRIES", 1), ("MAX_FILE", 1)]:
            with self.subTest(bound=name), mock.patch.object(archive, name, value):
                self.reject()

    def test_modified_payload_rejected(self):
        (self.candidate / "README-START.txt").write_bytes(b"modified")
        self.reject()

    def test_unlisted_file_rejected(self):
        (self.candidate / "extra.txt").write_bytes(b"unlisted")
        self.reject()

    def test_missing_payload_rejected(self):
        (self.candidate / "README-START.txt").unlink()
        self.reject()

    def test_manifest_aggregate_count_bytes_and_coverage_rejected(self):
        original = (self.candidate / "PACKAGE-MANIFEST.json").read_bytes()
        for field, value in [("aggregateSha256", "0" * 64), ("fileCount", 999), ("totalBytes", 1), ("coverage", {"excludes": [], "rule": "broken"})]:
            with self.subTest(field=field):
                (self.candidate / "PACKAGE-MANIFEST.json").write_bytes(original)
                self.edit_json(self.candidate, "PACKAGE-MANIFEST.json", lambda j: j.update({field: value}))
                self.reject()

    def test_manifest_case_collision_and_executable_asset_rejected(self):
        original = (self.candidate / "PACKAGE-MANIFEST.json").read_bytes()
        self.edit_json(self.candidate, "PACKAGE-MANIFEST.json", lambda j: j["files"].append({**j["files"][-1], "path": "mir2-assets/nested/DATA.txt"}))
        self.edit_json(self.candidate, "PACKAGE-MANIFEST.json", lambda j: j.update({"fileCount": len(j["files"])}))
        with self.assertRaisesRegex(archive.ValidationError, "case-colliding manifest"):
            archive.build_archive(self.mapping, self.output)
        self.assertFalse(self.output.exists())
        (self.candidate / "PACKAGE-MANIFEST.json").write_bytes(original)
        self.edit_json(self.candidate, "PACKAGE-MANIFEST.json", lambda j: j["files"][0].update({"path": "mir2-assets/file.cmd.png"}))
        self.reject()

    def test_statement_and_version_binding_rejected(self):
        for root, name, field, value in [(self.candidate, "RELEASE-STATEMENT.json", "candidate", "WN-CANDIDATE-other"),
                                          (self.candidate, "VERSION.json", "exeSha256", "0" * 64),
                                          (self.candidate, "VERSION.json", "worktreeDirty", True),
                                          (self.candidate, "VERSION.json", "packageFileCount", True)]:
            original = (root / name).read_bytes()
            with self.subTest(field=field):
                self.edit_json(root, name, lambda j: j.update({field: value}))
                self.reject()
                (root / name).write_bytes(original)

    def test_engine_executable_hash_size_schema_and_identity_rejected(self):
        original = (self.engine / "ENGINE.json").read_bytes()
        for field, value in [("exeSha256", "0" * 64), ("exeSize", 1), ("schema", "unexpected"), ("launcherSha256", "A" * 63),
                             ("cargoLockSha256", "a" * 64), ("version", True), ("minBootstrap", 2), ("builtUnix", 0), ("sourceRevision", "bad")]:
            with self.subTest(field=field):
                (self.engine / "ENGINE.json").write_bytes(original)
                self.edit_json(self.engine, "ENGINE.json", lambda j: j.update({field: value}))
                self.reject()

    def test_engine_unknown_file_and_missing_signature_rejected(self):
        (self.engine / "extra.exe").write_bytes(b"not signed")
        self.reject()
        (self.engine / "extra.exe").unlink()
        (self.engine / "ENGINE.p7s").unlink()
        self.reject()

    def test_hardlinked_payload_rejected(self):
        linked = self.root / "linked-copy.txt"
        os.link(self.candidate / "README-START.txt", linked)
        self.reject()
        self.assertTrue(linked.exists())

    def test_symlink_source_and_ancestor_rejected(self):
        linked = self.root / "linked-source"
        try:
            linked.symlink_to(self.candidate, target_is_directory=True)
        except OSError as exc:
            self.skipTest(f"Windows symlink privilege unavailable: {exc}")
        self.reject({"releases/game": str(linked)})
        self.reject({"releases/game": str(linked / "mir2-assets")})

    def test_symlink_payload_rejected(self):
        linked = self.candidate / "mir2-assets" / "linked.txt"
        try:
            linked.symlink_to(self.root / "outside.txt")
        except OSError as exc:
            self.skipTest(f"Windows symlink privilege unavailable: {exc}")
        with self.assertRaisesRegex(archive.ValidationError, "symlink/junction/reparse"):
            archive.build_archive(self.mapping, self.output)
        self.assertFalse(self.output.exists())

    @unittest.skipUnless(os.name == "nt", "Windows junction fixture")
    def test_windows_junction_source_rejected(self):
        linked = self.root / "junction-source"
        made = subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J", str(linked), str(self.candidate)], capture_output=True)
        if made.returncode:
            self.skipTest("junction creation unavailable")
        self.reject({"releases/game": str(linked)})
        self.assertTrue(self.candidate.exists())

    @unittest.skipUnless(os.name == "nt", "NTFS alternate data stream fixture")
    def test_ntfs_alternate_stream_rejected(self):
        with open(str(self.candidate / "README-START.txt") + ":hidden", "wb") as stream:
            stream.write(b"hidden payload")
        self.reject()

    @unittest.skipIf(os.name == "nt", "colon is an NTFS stream on Windows")
    def test_ads_style_filename_rejected(self):
        (self.candidate / "ads:stream").write_bytes(b"bad")
        self.reject()

    @unittest.skipIf(os.name == "nt", "Windows filename lookup is case-insensitive")
    def test_actual_case_colliding_files_rejected(self):
        (self.candidate / "readme-start.txt").write_bytes(b"ambiguous")
        self.reject()

    @unittest.skipIf(os.name == "nt", "backslash cannot be a Windows filename")
    def test_posix_backslash_filename_rejected(self):
        (self.candidate / "mir2-assets" / "back\\slash.txt").write_bytes(b"ambiguous")
        self.reject()

    def test_write_time_source_mutation_retains_partial_without_receipt(self):
        original = archive._write_tar

        def mutate_then_write(output, files, directories):
            (self.candidate / "README-START.txt").write_bytes(b"changed after validation")
            return original(output, files, directories)

        with mock.patch.object(archive, "_write_tar", mutate_then_write):
            with self.assertRaisesRegex(archive.ValidationError, "source changed"):
                archive.build_archive(self.mapping, self.output)
        self.assertTrue(self.output.exists())
        self.assertFalse(Path(str(self.output) + ".receipt.json").exists())

    def test_duplicate_json_keys_rejected(self):
        with self.assertRaisesRegex(archive.ValidationError, "duplicate JSON"):
            archive.json_bytes(b'{"a":1,"a":2}', "fixture")

    def test_cli_archive_and_cms_limit_report(self):
        mapping = self.root / "source-map.json"
        mapping.write_bytes(dumped(self.mapping))
        result = subprocess.run([sys.executable, "-B", str(SCRIPT), "--source-map", str(mapping), "--output", str(self.output)],
                                capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("CMS signatures were not checked", result.stderr)
        self.assertEqual(json.loads(result.stdout)["fileCount"], 11)


if __name__ == "__main__":
    unittest.main(verbosity=2)
