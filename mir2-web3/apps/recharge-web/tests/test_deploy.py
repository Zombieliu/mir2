"""Offline deployment regression; no services, credentials or network access."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SERVER = Path(__file__).resolve().parents[1] / "server"
SPEC = importlib.util.spec_from_file_location("recharge_deploy", SERVER / "deploy.py")
deploy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(deploy)


class VerifiedReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.stage = self.root / "stage"
        self.stage.mkdir()
        self.original = {name: ("verified:" + name + "\n").encode()
                         for name in deploy.NAMES}
        manifest = {"schemaVersion": 1, "application": "numeron-recharge-web",
                    "files": [{"name": name, "bytes": len(data), "sha256": deploy.sha(data)}
                              for name, data in sorted(self.original.items())]}
        self.manifest_bytes = (json.dumps(manifest) + "\n").encode()
        self.manifest_sha = deploy.sha(self.manifest_bytes)
        for name, data in self.original.items():
            (self.stage / name).write_bytes(data)
        (self.stage / "release.json").write_bytes(self.manifest_bytes)

    def test_staging_replacement_after_validation_cannot_change_release(self):
        manifest, content = deploy.verified_release(self.stage, self.manifest_sha)
        for name in deploy.NAMES | {"release.json"}:
            (self.stage / name).write_bytes(b"replacement after validation")
        release = self.root / "release"
        release.mkdir()
        deploy.write_verified_release(release, content)
        self.assertEqual((release / "release.json").read_bytes(), self.manifest_bytes)
        self.assertEqual(deploy.sha((release / "release.json").read_bytes()), self.manifest_sha)
        for file in manifest["files"]:
            actual = (release / file["name"]).read_bytes()
            self.assertEqual(actual, self.original[file["name"]])
            self.assertEqual(len(actual), file["bytes"])
            self.assertEqual(deploy.sha(actual), file["sha256"])

    def test_missing_staging_after_validation_does_not_require_reread(self):
        _, content = deploy.verified_release(self.stage, self.manifest_sha)
        for path in self.stage.iterdir():
            path.unlink()
        self.stage.rmdir()
        release = self.root / "release"
        release.mkdir()
        deploy.write_verified_release(release, content)
        self.assertEqual({p.name for p in release.iterdir()}, deploy.NAMES | {"release.json"})
        self.assertEqual((release / "activities.js").read_bytes(), self.original["activities.js"])

    def test_tampered_public_file_is_rejected_before_publication(self):
        (self.stage / "app.js").write_bytes(b"changed before verification")
        with self.assertRaisesRegex(RuntimeError, "^stagedHashMismatch$"):
            deploy.verified_release(self.stage, self.manifest_sha)

    def test_missing_public_file_is_rejected(self):
        (self.stage / "app.js").unlink()
        with self.assertRaisesRegex(RuntimeError, "^stagedFileRejected$"):
            deploy.verified_release(self.stage, self.manifest_sha)

    def test_manifest_hash_mismatch_is_rejected(self):
        (self.stage / "release.json").write_bytes(self.manifest_bytes + b" ")
        with self.assertRaisesRegex(RuntimeError, "^releaseManifestHashMismatch$"):
            deploy.verified_release(self.stage, self.manifest_sha)

    def test_non_allowlisted_file_is_rejected_even_with_matching_manifest_hash(self):
        manifest = json.loads(self.manifest_bytes)
        manifest["files"][0]["name"] = "../outside.js"
        changed = json.dumps(manifest).encode()
        (self.stage / "release.json").write_bytes(changed)
        with self.assertRaisesRegex(RuntimeError, "^releaseManifestRejected$"):
            deploy.verified_release(self.stage, deploy.sha(changed))

    def test_existing_release_files_are_never_overwritten(self):
        _, content = deploy.verified_release(self.stage, self.manifest_sha)
        release = self.root / "release"
        release.mkdir()
        for name in content:
            (release / name).write_bytes(b"preexisting release")
        with self.assertRaises(FileExistsError):
            deploy.write_verified_release(release, content)
        self.assertTrue(all(p.read_bytes() == b"preexisting release" for p in release.iterdir()))


class ProxyCandidateTests(unittest.TestCase):
    def setUp(self):
        self.snippet = (SERVER / "caddy.snippet").read_bytes().replace(b"\r\n", b"\n")
        self.crlf_snippet = self.snippet.replace(b"\n", b"\r\n")
        # Mixed endings in unrelated existing bytes must remain untouched.
        self.private_routes = b"# unrelated existing configuration\r\nheader X-Fixture test-only\r\n"

    def existing(self, addition, snippet=None):
        return (self.private_routes + deploy.PUBLIC_LINE + addition + b"\n"
                + (self.snippet if snippet is None else snippet) + deploy.MARKER + b"\n")

    def test_crlf_activity_upgrade_preserves_all_existing_bytes_except_exact_routes(self):
        original = self.existing(deploy.API_ADDITION)
        expected = self.existing(deploy.ACTIVITY_API_ADDITION)
        self.assertEqual(deploy.proxy_candidate(original, self.crlf_snippet, True,
                                                enable_activities_routes=True), expected)
        self.assertEqual(deploy.proxy_candidate(original, self.snippet, True,
                                                enable_activities_routes=True), expected)
        self.assertTrue(expected.startswith(self.private_routes))

    def test_initial_crlf_and_lf_installations_have_identical_output(self):
        original = self.private_routes + deploy.PUBLIC_LINE + b"\n" + deploy.MARKER + b"\n"
        actual = deploy.proxy_candidate(original, self.crlf_snippet, False)
        self.assertEqual(actual, deploy.proxy_candidate(original, self.snippet, False))
        restored = actual.replace(b"\n" + self.snippet + b"\n", b"", 1).replace(
            deploy.PUBLIC_LINE + deploy.ACTIVITY_API_ADDITION, deploy.PUBLIC_LINE, 1)
        self.assertEqual(restored, original)

    def test_static_update_does_not_rewrite_existing_configuration(self):
        original = self.existing(deploy.ACTIVITY_API_ADDITION)
        self.assertEqual(deploy.proxy_candidate(original, self.crlf_snippet, True), original)

    def test_existing_crlf_site_accepts_both_upload_endings_without_rewriting_it(self):
        original = self.existing(deploy.API_ADDITION, self.crlf_snippet)
        expected = self.existing(deploy.ACTIVITY_API_ADDITION, self.crlf_snippet)
        for uploaded in (self.snippet, self.crlf_snippet):
            with self.subTest(ending="CRLF" if b"\r\n" in uploaded else "LF"):
                self.assertEqual(deploy.proxy_candidate(original, uploaded, True,
                                                        enable_activities_routes=True), expected)
                self.assertEqual(deploy.proxy_candidate(expected, uploaded, True), expected)

    def test_exact_old_mixed_ending_upload_remains_accepted(self):
        mixed = self.snippet.replace(b"\n", b"\r\n", 1)
        original = self.existing(deploy.API_ADDITION, mixed)
        self.assertEqual(deploy.proxy_candidate(original, mixed, True, enable_activities_routes=True),
                         self.existing(deploy.ACTIVITY_API_ADDITION, mixed))

    def test_logout_upgrade_retains_all_unrelated_bytes(self):
        original = self.existing(deploy.PREVIOUS_API_ADDITION)
        self.assertEqual(deploy.proxy_candidate(original, self.crlf_snippet, True,
                                                enable_logout_route=True), self.existing(deploy.API_ADDITION))

    def test_duplicate_site_snippets_remain_rejected(self):
        original = self.existing(deploy.API_ADDITION, self.snippet + self.snippet)
        with self.assertRaisesRegex(RuntimeError, "^existingSiteRoutesChanged$"):
            deploy.proxy_candidate(original, self.crlf_snippet, True, enable_activities_routes=True)

    def test_duplicate_snippets_with_different_endings_remain_rejected(self):
        original = self.existing(deploy.API_ADDITION, self.snippet + self.crlf_snippet)
        with self.assertRaisesRegex(RuntimeError, "^existingSiteRoutesChanged$"):
            deploy.proxy_candidate(original, self.crlf_snippet, True, enable_activities_routes=True)

    def test_modified_site_snippet_remains_rejected(self):
        original = self.existing(deploy.API_ADDITION, self.snippet.replace(b"/srv/mir2-recharge/current",
                                                                         b"/srv/other/current"))
        with self.assertRaisesRegex(RuntimeError, "^existingSiteRoutesChanged$"):
            deploy.proxy_candidate(original, self.crlf_snippet, True, enable_activities_routes=True)

    def test_duplicate_authenticated_route_remains_rejected(self):
        original = self.existing(deploy.API_ADDITION) + deploy.PUBLIC_LINE + deploy.API_ADDITION + b"\n"
        with self.assertRaisesRegex(RuntimeError, "^previousAuthenticatedRoutesNotExpected$"):
            deploy.proxy_candidate(original, self.crlf_snippet, True, enable_activities_routes=True)

    def test_wrong_authenticated_route_remains_rejected(self):
        original = self.existing(deploy.PREVIOUS_API_ADDITION)
        with self.assertRaisesRegex(RuntimeError, "^previousAuthenticatedRoutesNotExpected$"):
            deploy.proxy_candidate(original, self.crlf_snippet, True, enable_activities_routes=True)

    def test_ambiguous_route_upgrade_remains_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "^routeUpgradeAmbiguous$"):
            deploy.proxy_candidate(self.existing(deploy.API_ADDITION), self.crlf_snippet, True,
                                   enable_logout_route=True, enable_activities_routes=True)

    def test_existing_site_is_not_admitted_as_a_first_install(self):
        with self.assertRaisesRegex(RuntimeError, "^existingRoutesNotExpected$"):
            deploy.proxy_candidate(self.existing(deploy.ACTIVITY_API_ADDITION), self.crlf_snippet, False)


if __name__ == "__main__":
    unittest.main()
