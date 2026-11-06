"""Offline adversarial tests; fixture signatures are never claimed as CMS."""
import contextlib
import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result

v = load("delivery_verifier_test", "verify-native-delivery.py")
p = load("delivery_producer_fixture", "prepare-native-delivery.py")
fixtures = load("delivery_candidate_fixture", "test-archive-update-release.py")


class VerifyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mir2-delivery-verify-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).absolute()
        self.candidate = fixtures.fixture_candidate(self.root / "candidate")
        self.delivery = self.root / "delivery-root"
        with contextlib.redirect_stdout(io.StringIO()):
            p.build(str(self.candidate), self.delivery, [], 1024 * 1024)
        self.descriptor = json.loads((self.delivery / "DELIVERY.json").read_bytes())
        self.files = v.candidate_binding(self.candidate)[1]

    def write_descriptor(self):
        (self.delivery / "DELIVERY.json").write_bytes(p.compact(self.descriptor))

    def rebind_manifest(self, entries, padded_size=None):
        manifest_path = self.candidate / "PACKAGE-MANIFEST.json"
        manifest = json.loads(manifest_path.read_bytes())
        entries = sorted(entries, key=lambda item: item["path"])
        canonical = "".join(f"{item['path']}\t{item['size']}\t{item['sha256']}\n" for item in entries)
        aggregate = v.archive.sha256(canonical.encode("ascii"))
        manifest.update(fileCount=len(entries), totalBytes=sum(item["size"] for item in entries),
                        aggregateSha256=aggregate, files=entries)
        manifest_bytes = p.compact(manifest)
        if padded_size is not None:
            self.assertLessEqual(len(manifest_bytes), padded_size)
            manifest_bytes += b" " * (padded_size - len(manifest_bytes))
        manifest_path.write_bytes(manifest_bytes)
        version_path = self.candidate / "VERSION.json"
        version = json.loads(version_path.read_bytes())
        version.update(packageManifestSha256=v.archive.sha256(manifest_bytes),
                       packageManifestAggregateSha256=aggregate,
                       packageManifestFileCount=len(entries), packageFileCount=len(entries) + 4)
        version_bytes = p.compact(version)
        version_path.write_bytes(version_bytes)
        statement_path = self.candidate / "RELEASE-STATEMENT.json"
        statement = json.loads(statement_path.read_bytes())
        statement.update(packageManifestSha256=version["packageManifestSha256"],
                         packageManifestAggregateSha256=aggregate,
                         versionSha256=v.archive.sha256(version_bytes))
        statement_path.write_bytes(p.compact(statement))
        self.descriptor.update(packageManifestSha256=version["packageManifestSha256"],
                               versionSha256=statement["versionSha256"])
        self.write_descriptor()

    def change_archive(self, data, patch=False):
        item = self.descriptor["patches"][0]["patch"] if patch else self.descriptor["bundles"][0]["archive"]
        suffix = ".m2d.gz" if patch else ".m2b.gz"
        digest = v.archive.sha256(data)
        name = "delivery/" + digest.lower() + suffix
        (self.delivery / name).write_bytes(data)
        item.update(path=name, size=len(data), sha256=digest)
        self.write_descriptor()

    def add_patch(self):
        base = self.root / "old-game.exe"
        base.write_bytes(b"old game fixture bytes")
        target = (self.candidate / "mir2-platform-windows.exe").read_bytes()
        output = self.root / "fixture-patch.gz"
        p.write_delta(output, base.read_bytes(), target)
        data = output.read_bytes()
        digest = v.archive.sha256(data)
        path = "delivery/" + digest.lower() + ".m2d.gz"
        (self.delivery / path).write_bytes(data)
        self.descriptor["patches"] = [{"path": "mir2-platform-windows.exe", "baseSize": base.stat().st_size,
            "baseSha256": v.archive.sha256(base.read_bytes()), "patch": {"path": path, "size": len(data), "sha256": digest}}]
        self.write_descriptor()
        return base, target

    def test_complete_verified_bytes_and_fresh_receipt_without_cms_claim(self):
        receipt = self.root / "proof.json"
        result = v.run(self.candidate, self.delivery, [], receipt)
        self.assertTrue(result["passed"])
        self.assertFalse(result["cmsVerified"])
        self.assertEqual(result["payloadFiles"], 4)
        self.assertEqual(len(result["checkedArtifacts"]), 1)
        self.assertEqual(json.loads(receipt.read_bytes()), result)
        original = receipt.read_bytes()
        with self.assertRaises(ValueError):
            v.run(self.candidate, self.delivery, [], receipt)
        self.assertEqual(receipt.read_bytes(), original)

    def test_candidate_assets_are_not_rescanned_or_required_locally(self):
        (self.candidate / "mir2-assets/nested/data.txt").unlink()
        self.assertTrue(v.run(self.candidate, self.delivery, [])["passed"])

    def test_actual_300000_file_manifest_and_single_bundle_roundtrip(self):
        original_entries = json.loads((self.candidate / "PACKAGE-MANIFEST.json").read_bytes())["files"]
        source = self.root / "capacity-source.bin"
        source.write_bytes(b"x")
        info = p.archive._plain(source, directory=False)
        fingerprint = p.archive._fingerprint(info)
        digest = p.archive.sha256(b"x")
        entries = original_entries + [
            {"path": f"mir2-assets/capacity/{index:06d}.png", "size": 1, "sha256": digest}
            for index in range(300_000 - len(original_entries))]
        self.assertEqual(len(entries), 300_000)
        self.rebind_manifest(entries)
        manifest_size = (self.candidate / "PACKAGE-MANIFEST.json").stat().st_size
        self.assertGreater(manifest_size, 32 * 1024**2)
        self.assertLess(manifest_size, 64 * 1024**2)
        _, files, identity, _, manifest_bytes, version_bytes = v.candidate_binding(self.candidate)
        payload = []
        for item in sorted(entries, key=lambda item: item["path"]):
            path = item["path"]
            if path.startswith("mir2-assets/capacity/"):
                payload.append(p.archive.Entry(path, source, 1, digest, fingerprint))
            else:
                original = self.candidate / path
                payload.append(p.archive.Entry(path, original, item["size"], item["sha256"],
                                               p.archive._fingerprint(p.archive._plain(original, directory=False))))
        partial = self.delivery / "capacity-bundle.gz"
        original_open = p.archive._open_entry
        source_entry = p.archive.Entry("mir2-assets/capacity/000000.png", source, 1, digest, fingerprint)
        # Reuse one verified real file handle for identical one-byte fixtures;
        # keep every CheckedReader hash/size/fstat check and every encoded entry.
        with original_open(source_entry) as shared:
            @contextlib.contextmanager
            def open_fixture(item):
                if item.source == source:
                    self.assertEqual(item.fingerprint, fingerprint)
                    shared.seek(0)
                    yield shared
                else:
                    with original_open(item) as current:
                        yield current
            with mock.patch.object(p.archive, "_open_entry", new=open_fixture):
                built = p.write_bundle(partial, payload)
        artifact = p.entry(partial, "unused")
        artifact["path"] = "delivery/" + artifact["sha256"].lower() + ".m2b.gz"
        partial.rename(self.delivery / artifact["path"])
        self.descriptor["bundles"] = [{"archive": artifact, "files": built["files"]}]
        self.write_descriptor()
        result = v.run(self.candidate, self.delivery, [])
        self.assertTrue(result["passed"], result.get("failure"))
        self.assertFalse(result["cmsVerified"])
        self.assertEqual(result["payloadFiles"], 300_000)
        self.assertEqual(result["bundles"][0]["fileCount"], 300_000)
        self.assertEqual(result["bundles"][0]["expandedBytes"], built["expandedBytes"])
        self.assertEqual(result["checkedArtifacts"], [artifact])
        too_many = json.loads(json.dumps(self.descriptor))
        too_many["bundles"][0]["files"].append("mir2-assets/capacity/extra.png")
        self.assertEqual(len(too_many["bundles"][0]["files"]), 300_001)
        with self.assertRaisesRegex(ValueError, "bundle entry count outside bounds"):
            v.descriptor_binding(p.compact(too_many), files, identity, manifest_bytes, version_bytes)
        entries.append({"path": "mir2-assets/capacity/extra.png", "size": 1, "sha256": digest})
        self.assertEqual(len(entries), 300_001)
        self.rebind_manifest(entries)
        rejected = v.run(self.candidate, self.delivery, [])
        self.assertFalse(rejected["passed"])
        self.assertEqual(rejected["failure"], "invalid manifest file count")

    def test_metadata_exact_64mib_and_first_byte_over_rejected(self):
        entries = json.loads((self.candidate / "PACKAGE-MANIFEST.json").read_bytes())["files"]
        self.rebind_manifest(entries, padded_size=64 * 1024**2)
        manifest = self.candidate / "PACKAGE-MANIFEST.json"
        self.assertEqual(manifest.stat().st_size, 64 * 1024**2)
        result = v.run(self.candidate, self.delivery, [])
        self.assertTrue(result["passed"], result.get("failure"))
        self.assertFalse(result["cmsVerified"])
        self.assertEqual(result["payloadFiles"], len(entries))
        metadata = {item["path"]: item for item in result["metadata"]}
        self.assertEqual(metadata["PACKAGE-MANIFEST.json"]["size"], 64 * 1024**2)
        with manifest.open("ab") as stream:
            stream.write(b" ")
        self.assertEqual(manifest.stat().st_size, 64 * 1024**2 + 1)
        rejected = v.run(self.candidate, self.delivery, [])
        self.assertFalse(rejected["passed"])
        self.assertEqual(rejected["failure"], "input file size outside bounds")

    def test_descriptor_unknown_fields_types_time_and_manifest_bindings(self):
        original = json.loads(json.dumps(self.descriptor))
        for field, bad in [("unknown", 1), ("builtUnix", 0), ("builtUnix", True),
                           ("builtUnix", 9_999_999_999), ("candidate", "other"),
                           ("packageManifestSha256", "0" * 64), ("bundles", {})]:
            self.descriptor = {**original, field: bad}
            self.write_descriptor()
            self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"], field)

    def test_missing_duplicate_case_unknown_and_reordered_bundle_paths(self):
        original = json.loads(json.dumps(self.descriptor))
        paths = original["bundles"][0]["files"]
        for bad in [paths[:-1], paths + [paths[0]], list(reversed(paths)),
                    [paths[0].lower(), *paths[1:]], ["mir2-assets/unknown.png", *paths[1:]]]:
            self.descriptor = json.loads(json.dumps(original))
            self.descriptor["bundles"][0]["files"] = bad
            self.write_descriptor()
            self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"])

    def test_raw_header_path_size_or_decoded_hash_tampering_fails(self):
        source = self.delivery / self.descriptor["bundles"][0]["archive"]["path"]
        raw = gzip.decompress(source.read_bytes())
        length = struct.unpack("<H", raw[12:14])[0]
        for bad in [b"BADMAGIC" + raw[8:], raw[:8] + struct.pack("<I", 999) + raw[12:],
                    raw[:14] + b"X" * length + raw[14 + length:],
                    raw[:14 + length] + struct.pack("<Q", 999) + raw[22 + length:],
                    raw[:-1] + bytes([raw[-1] ^ 1])]:
            self.change_archive(gzip.compress(bad, mtime=0))
            self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"])

    def test_single_gzip_member_crc_truncation_and_both_eof_guards(self):
        source = self.delivery / self.descriptor["bundles"][0]["archive"]["path"]
        compressed = source.read_bytes()
        crc_bad = compressed[:-8] + bytes([compressed[-8] ^ 1]) + compressed[-7:]
        for bad in [compressed + gzip.compress(b"", mtime=0), compressed + b"\0",
                    compressed[:-1], crc_bad, gzip.compress(gzip.decompress(compressed) + b"x", mtime=0)]:
            self.change_archive(bad)
            self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"])

    def test_bounds_include_headers_and_compressed_hash_is_verified(self):
        with mock.patch.object(v, "MAX_EXPANDED", 12):
            self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"])
        self.descriptor["bundles"][0]["archive"]["size"] = v.MAX_COMPRESSED + 1
        self.write_descriptor()
        self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"])
        declared = self.descriptor["bundles"][0]["archive"]
        source = self.delivery / declared["path"]
        declared["size"] = source.stat().st_size
        self.write_descriptor()
        source.write_bytes(source.read_bytes()[:-1] + b"x")
        self.assertFalse(v.run(self.candidate, self.delivery, [])["passed"])

    def test_all_declared_delta_bases_must_be_verified_and_target_stream_hash_matches(self):
        base, target = self.add_patch()
        missing = v.run(self.candidate, self.delivery, [])
        self.assertFalse(missing["passed"])
        good = v.run(self.candidate, self.delivery, [base])
        self.assertTrue(good["passed"])
        self.assertEqual(good["patches"][0]["targetSha256"], v.archive.sha256(target))
        base.write_bytes(b"x" * base.stat().st_size)
        self.assertFalse(v.run(self.candidate, self.delivery, [base])["passed"])

    def test_copy_literal_operation_output_and_raw_eof_bounds(self):
        base, target = self.add_patch()
        valid_literal = b"MIR2DP01" + struct.pack("<IQ", 1, len(target)) + b"\1" + struct.pack("<Q", len(target)) + target
        invalid = [
            b"MIR2DP01" + struct.pack("<IQ", v.MAX_OPERATIONS + 1, len(target)),
            b"MIR2DP01" + struct.pack("<IQ", 1, len(target)) + b"\2",
            b"MIR2DP01" + struct.pack("<IQ", 1, len(target)) + b"\0" + struct.pack("<QQ", base.stat().st_size, 1),
            b"MIR2DP01" + struct.pack("<IQ", 1, len(target)) + b"\1" + struct.pack("<Q", len(target) + 1),
            valid_literal[:-1], valid_literal + b"extra",
            valid_literal[:-1] + bytes([target[-1] ^ 1]),
        ]
        for raw in invalid:
            self.change_archive(gzip.compress(raw, mtime=0), patch=True)
            self.assertFalse(v.run(self.candidate, self.delivery, [base])["passed"])

    def test_failed_attempt_receipt_is_retained(self):
        self.descriptor["candidate"] = "wrong"
        self.write_descriptor()
        receipt = self.root / "failed.json"
        self.assertFalse(v.run(self.candidate, self.delivery, [], receipt)["passed"])
        self.assertIn("failure", json.loads(receipt.read_bytes()))


if __name__ == "__main__":
    unittest.main(verbosity=2)
