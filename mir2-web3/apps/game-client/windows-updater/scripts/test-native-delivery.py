#!/usr/bin/env python3
"""Producer roundtrip / boundary tests; native adversarial readers test separately."""
import gzip
import hashlib
import importlib.util
from pathlib import Path
import random
import struct
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("delivery_producer", Path(__file__).with_name("prepare-native-delivery.py"))
delivery = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = delivery
spec.loader.exec_module(delivery)


class DeliveryProducerTests(unittest.TestCase):
    def setUp(self):
        # Retain owned test fixtures; never recursively delete an inferred path.
        self.root = Path(tempfile.mkdtemp(prefix="mir2-native-delivery-test-"))

    def source(self, path, data):
        source = self.root / path
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(data)
        info = delivery.archive._plain(source, directory=False)
        return delivery.archive.Entry(path, source, len(data), delivery.archive.sha256(data),
                                      delivery.archive._fingerprint(info))

    def test_bundle_exact_headers_bytes_and_determinism(self):
        files = [self.source("mir2-assets/a.png", b"image" * 100),
                 self.source("mir2-assets/b.json", b"{}"), self.source("CONTROLS.txt", b"")]
        first, second = self.root / "first.gz", self.root / "second.gz"
        receipt = delivery.write_bundle(first, files)
        delivery.write_bundle(second, files)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with gzip.open(first, "rb") as stream:
            self.assertEqual(stream.read(8), delivery.BUNDLE_MAGIC)
            self.assertEqual(struct.unpack("<I", stream.read(4))[0], len(files))
            for item in files:
                length, = struct.unpack("<H", stream.read(2))
                self.assertEqual(stream.read(length).decode("ascii"), item.path)
                self.assertEqual(struct.unpack("<Q", stream.read(8))[0], item.size)
                self.assertEqual(hashlib.sha256(stream.read(item.size)).hexdigest().upper(), item.sha256)
            self.assertEqual(stream.read(1), b"")
        self.assertEqual(receipt["expandedBytes"], 12 + sum(10 + len(f.path) + f.size for f in files))

    def test_bundle_refuses_changed_source_and_keeps_failed_artifact(self):
        item = self.source("mir2-assets/a.png", b"initial")
        item.source.write_bytes(b"tampered")
        target = self.root / "failed.gz"
        with self.assertRaises(ValueError):
            delivery.write_bundle(target, [item])
        self.assertTrue(target.exists())

    def test_bundle_refuses_overwrite(self):
        item = self.source("CONTROLS.txt", b"abc")
        target = self.root / "exists.gz"
        target.write_bytes(b"retained")
        with self.assertRaises(FileExistsError):
            delivery.write_bundle(target, [item])
        self.assertEqual(target.read_bytes(), b"retained")

    def test_bundle_rejects_traversal_metadata_and_case_collision(self):
        original = self.source("CONTROLS.txt", b"abc")
        for path in ("../evil.png", "mir2-assets/evil.exe", "VERSION.json"):
            changed = delivery.archive.Entry(path, original.source, original.size, original.sha256, original.fingerprint)
            with self.subTest(path=path), self.assertRaises(ValueError):
                delivery.write_bundle(self.root / "unsafe.gz", [changed])
        duplicate = delivery.archive.Entry("controls.txt", original.source, original.size, original.sha256, original.fingerprint)
        with self.assertRaises(ValueError):
            delivery.write_bundle(self.root / "duplicate.gz", [original, duplicate])

    def test_partition_budget_includes_headers_and_count_limit(self):
        files = [self.source(f"mir2-assets/{i}.png", b"x" * 20) for i in range(5)]
        groups = delivery.partitions(files, 80)
        self.assertEqual(len(groups), 5)
        self.assertEqual([f.path for group in groups for f in group], [f.path for f in files])
        with self.assertRaises(ValueError):
            delivery.partitions(files * 14, 1)
        with self.assertRaises(ValueError):
            delivery.partitions(files, delivery.MAX_COMPRESSED + 1)

    def delta(self, base, target, name="patch.gz"):
        path = self.root / name
        receipt = delivery.write_delta(path, base, target)
        delivery.verify_delta(path, base, target)
        return path, receipt

    def test_delta_recovers_insertions_deletions_and_modified_chunks(self):
        base = random.Random(9482).randbytes(2 * 1024**2)
        target = base[:180_000] + b"inserted" * 130 + base[180_007:750_000] + b"changed" * 151 + base[751_013:]
        path, receipt = self.delta(base, target)
        self.assertGreater(receipt["copyBytes"], len(target) * 0.80)
        self.assertLess(path.stat().st_size, len(target) * 0.20)
        self.assertEqual(receipt["copyBytes"] + receipt["literalBytes"], len(target))

    def test_delta_small_unrelated_and_zero_chunk_data(self):
        for index, (base, target) in enumerate(((b"a", b"b"), (b"\0" * 200_000, b"\0" * 250_001),
                                               (b"abc" * 40_000, b"completely different" * 9000))):
            self.delta(base, target, f"patch{index}.gz")
        with self.assertRaises(ValueError):
            delivery.delta_operations(b"", b"a")

    def test_delta_wrong_base_truncated_or_trailing_stream_rejects(self):
        base = random.Random(15).randbytes(200_000)
        path, _ = self.delta(base, base[:100_000] + b"new" + base[100_000:])
        target = base[:100_000] + b"new" + base[100_000:]
        with self.assertRaises(ValueError):
            delivery.verify_delta(path, b"q" * len(base), target)
        compressed = path.read_bytes()
        for index, bad in enumerate((compressed[:-9], gzip.compress(gzip.decompress(compressed) + b"extra", mtime=0))):
            invalid = self.root / f"invalid{index}.gz"
            invalid.write_bytes(bad)
            with self.assertRaises((ValueError, EOFError)):
                delivery.verify_delta(invalid, base, target)

    def test_content_defined_chunk_bounds_and_stable_repeat(self):
        data = random.Random(88).randbytes(1_000_000)
        chunks = list(delivery.chunks(data))
        self.assertEqual(chunks, list(delivery.chunks(data)))
        self.assertEqual(sum(length for _, length in chunks), len(data))
        self.assertTrue(all(4096 <= length <= 65_536 for _, length in chunks[:-1]))
        with self.assertRaises(ValueError):
            list(delivery.chunks(data, average=5000))


if __name__ == "__main__":
    unittest.main()
