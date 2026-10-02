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
