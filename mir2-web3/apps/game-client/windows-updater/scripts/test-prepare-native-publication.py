"""Offline closure/plan tests. All signatures and root attestations are fixtures."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.dont_write_bytecode = True
def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result

m = load("native_publication_test", "prepare-native-publication.py")
p = load("native_publication_producer", "prepare-native-delivery.py")
fixtures = load("native_publication_fixtures", "test-archive-update-release.py")
a = m.archive


def add_config(root):
    config = b"[client]\nfixture = true\n"
    (root / "mir2-client.toml").write_bytes(config)
    manifest = json.loads((root / "PACKAGE-MANIFEST.json").read_bytes())
    manifest["files"].append({"path": "mir2-client.toml", "size": len(config), "sha256": a.sha256(config)})
    manifest["files"].sort(key=lambda item: item["path"])
    manifest.update(fileCount=len(manifest["files"]), totalBytes=sum(i["size"] for i in manifest["files"]),
        aggregateSha256=a.sha256("".join(f"{i['path']}\t{i['size']}\t{i['sha256']}\n" for i in manifest["files"]).encode()))
    raw_manifest = fixtures.dumped(manifest)
    (root / "PACKAGE-MANIFEST.json").write_bytes(raw_manifest)
    version = json.loads((root / "VERSION.json").read_bytes())
    version.update(packageManifestSha256=a.sha256(raw_manifest), packageManifestAggregateSha256=manifest["aggregateSha256"],
        packageManifestFileCount=len(manifest["files"]), packageFileCount=len(manifest["files"]) + 4)
    raw_version = fixtures.dumped(version)
    (root / "VERSION.json").write_bytes(raw_version)
    statement = json.loads((root / "RELEASE-STATEMENT.json").read_bytes())
    statement.update(packageManifestSha256=a.sha256(raw_manifest), packageManifestAggregateSha256=manifest["aggregateSha256"],
        versionSha256=a.sha256(raw_version))
    (root / "RELEASE-STATEMENT.json").write_bytes(fixtures.dumped(statement))


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mir2-publication-plan-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).absolute()
        self.game = fixtures.fixture_candidate(self.root / "game")
        add_config(self.game)
        self.engine = fixtures.fixture_engine(self.root / "engine")
        self.delivery = self.root / "delivery-root"
        with contextlib.redirect_stdout(io.StringIO()):
            p.build(str(self.game), self.delivery, [], 1024 * 1024)
        (self.delivery / "DELIVERY.p7s").write_bytes(b"FIXTURE NOT A CMS SIGNATURE")
        self.proof = self.root / "byte-verification.json"
        self.assertTrue(m.verify.run(self.game, self.delivery, [], self.proof)["passed"])
        self.feed_root = self.root / "feed"
        self.feed_root.mkdir()
        now = int(time.time())
        def metadata(root, names):
            return [{"path": n, "size": (root / n).stat().st_size, "sha256": a.sha256((root / n).read_bytes())} for n in names]
        self.feed = {"schema": "mir2.windows.update-feed.v1", "channel": "invited", "platform": "windows-x64", "sequence": 10,
            "createdUnix": now, "expiresUnix": now + 86400, "minBootstrap": 1, "protocol": "crystal-mir2-v1",
            "content": "mir2.windows.package-manifest.v4",
            "game": {"directory": "releases/legacy-game-r10", "identity": "WN-CANDIDATE-fixture-06", "metadata": metadata(self.game, a.META)},
            "engine": {"directory": "releases/new-engine-s10", "identity": "1", "metadata": metadata(self.engine, ("ENGINE.json", "ENGINE.p7s"))}}
        self.write_feed()
        self.inputs = dict(candidate_root=self.game, delivery_root=self.delivery, delivery_verification=self.proof,
                           engine_root=self.engine, feed_root=self.feed_root)

    def write_feed(self):
        (self.feed_root / "latest.json").write_bytes(fixtures.dumped(self.feed))
        (self.feed_root / "latest.p7s").write_bytes(b"FIXTURE DETACHED FEED SIGNATURE")

    def closure(self, **extra):
        return m.build_closure(**{**self.inputs, **extra})

    def attestation(self, closure):
        pointer = closure["candidate"]
        value = {"schema": m.publisher.VERIFICATION_SCHEMA, "passed": True, "candidateVerified": True, "cmsVerified": True,
                 **{name: pointer[name] for name in ("sourceRevision", "sequence", "feedSha256", "signatureSha256")},
                 "objectsSha256": closure["objectsSha256"]}
        path = self.root / "root-fixture-attestation.json"
        path.write_bytes(m.publisher.canonical_json(value))
        return path

    def test_exact_approved_closure_preserves_feed_and_engine_source_revision(self):
        closure = self.closure()
        self.assertEqual(closure["candidate"]["sourceRevision"], "b" * 40)
        self.assertEqual(closure["bindings"]["gameSourceRevision"], "a" * 40)
        self.assertEqual(closure["bindings"]["gameDirectory"], self.feed["game"]["directory"])
        self.assertEqual(closure["candidate"]["feedSha256"], a.sha256((self.feed_root / "latest.json").read_bytes()).lower())
        paths = {item["path"] for item in closure["objects"]}
        self.assertTrue(all("/mir2-assets/" not in path for path in paths))
        self.assertTrue(any(path.endswith("/mir2-client.toml") for path in paths))
        self.assertTrue(any(path.endswith("/mir2-platform-windows.exe") for path in paths))
        self.assertEqual(sum(path.endswith("/ENGINE.json") for path in paths), 1)
        self.assertEqual(closure["objectsSha256"],
            m.publisher.sha256(m.publisher.canonical_json(m.publisher.object_closure(closure["objects"]))))
        for item in closure["objects"]:
            self.assertEqual(item["sha256"], item["sha256"].lower())
            self.assertEqual(item["sha256"], m.publisher.sha256(Path(item["source"]).read_bytes()))

    def test_two_stage_root_attestation_produces_publisher_accepted_exact_plan(self):
        closure_path = self.root / "closure.json"
        closure = m.run(closure_path, **self.inputs)
        verification = self.attestation(closure)
        plan_path = self.root / "plan.json"
        result = m.run(plan_path, verification_receipt=verification, **self.inputs)
        self.assertEqual(result["schema"], m.publisher.PLAN_SCHEMA)
        plan, _ = m.publisher.load_plan(plan_path)
        self.assertEqual(plan["objects"], closure["objects"])
        self.assertEqual(plan["candidate"], closure["candidate"])
        self.assertEqual(plan["verificationReceipt"]["sha256"], m.publisher.sha256(verification.read_bytes()))

    def test_external_proof_partial_files_assets_and_unused_payloads_are_excluded(self):
        (self.delivery / "delivery/failed.building").write_bytes(b"partial")
        (self.delivery / "delivery/unused.gz").write_bytes(b"not declared")
        closure = self.closure()
        sources = {Path(item["source"]).name for item in closure["objects"]}
        self.assertFalse({"INPUT.json", "BUILD-DELIVERY.json", "byte-verification.json",
                          "failed.building", "unused.gz", "data.txt"} & sources)

    def test_optional_safe_root_repair_files_and_small_bootstrap_are_bound(self):
        bootstrap = self.root / "Mir2Setup.exe"
        bootstrap.write_bytes(b"MZ FIXTURE SMALL BOOTSTRAP; NEVER EXECUTED")
        closure = self.closure(include_root_files=True, bootstrap_exe=bootstrap)
        self.assertTrue(any(item["path"].endswith("/README-START.txt") for item in closure["objects"]))
        self.assertTrue(any(item["path"].startswith("installers/") for item in closure["objects"]))
        with bootstrap.open("wb") as stream:
            stream.truncate(m.publisher.MAX_ARTIFACT + 1)
        with self.assertRaises(ValueError):
            self.closure(bootstrap_exe=bootstrap)

    def test_wrong_feed_metadata_identity_schema_expiry_and_overlap_rejected(self):
        original = json.loads(json.dumps(self.feed))
        changes = [
            lambda f: f.update(extra=True),
            lambda f: f.update(sequence=True),
            lambda f: f.update(expiresUnix=1),
            lambda f: f["game"].update(identity="other"),
            lambda f: f["game"]["metadata"][0].update(sha256="0" * 64),
            lambda f: f["engine"].update(directory=f["game"]["directory"] + "/nested"),
        ]
        for change in changes:
            self.feed = json.loads(json.dumps(original))
            change(self.feed)
            self.write_feed()
            with self.assertRaises((ValueError, m.publisher.PublishError)):
                self.closure()

    def test_source_byte_changes_engine_extras_and_unbound_delivery_proof_rejected(self):
        exe = self.game / "mir2-platform-windows.exe"
        original = exe.read_bytes()
        exe.write_bytes(b"wrong")
        with self.assertRaises(ValueError):
            self.closure()
        exe.write_bytes(original)
        extra = self.engine / "EXTERNAL-PROOF.json"
        extra.write_bytes(b"{}")
        with self.assertRaises(ValueError):
            self.closure()
        extra.unlink()
        proof = json.loads(self.proof.read_bytes())
        proof["checkedArtifacts"] = []
        self.proof.write_bytes(fixtures.dumped(proof))
        with self.assertRaises(ValueError):
            self.closure()

    def test_sequence_downgrade_and_root_closure_claim_mismatch_rejected(self):
        closure = self.closure()
        previous = self.root / "previous.json"
        previous.write_bytes(m.publisher.canonical_json(closure["candidate"]))
        with self.assertRaises(ValueError):
            self.closure(expected_current=previous)
        verification = self.attestation(closure)
        value = json.loads(verification.read_bytes())
        value["objectsSha256"] = "0" * 64
        verification.write_bytes(m.publisher.canonical_json(value))
        with self.assertRaises(ValueError):
            m.make_plan(closure, verification)

    def test_fresh_outputs_and_failed_attempts_are_retained(self):
        output = self.root / "retained.json"
        result = m.run(output, **self.inputs)
        preserved = output.read_bytes()
        self.assertEqual(result["schema"], m.CLOSURE_SCHEMA)
        with self.assertRaises(ValueError):
            m.run(output, **self.inputs)
        self.assertEqual(output.read_bytes(), preserved)
        self.feed["schema"] = "bad"
        self.write_feed()
        failed = self.root / "failed.json"
        result = m.run(failed, **self.inputs)
        self.assertFalse(result["passed"])
        self.assertEqual(json.loads(failed.read_bytes()), result)


if __name__ == "__main__":
    unittest.main(verbosity=2)
