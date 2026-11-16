"""Offline adversarial publication tests: no credentials or cloud operations."""
import hashlib
import importlib.util
import io
import json
import os
import tempfile
import unittest
import urllib.error
from datetime import datetime, timezone
from pathlib import Path
from unittest import mock

spec = importlib.util.spec_from_file_location("publisher", Path(__file__).with_name("publish-native-r2.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class FakeS3:
    def __init__(self):
        self.rows = {}
        self.calls = []
        self.race = None
        self.fail_after_cas = False

    def seed(self, path, body, encoding=None):
        self.rows[path] = (body, {"etag": '"' + m.sha256(body) + '"',
                                  **({"content-encoding": encoding} if encoding else {})})

    def get(self, path, maximum):
        self.calls.append(("get", path))
        if path not in self.rows:
            raise m.PublishError("http_request_failed", 404)
        body, headers = self.rows[path]
        if len(body) > maximum:
            raise m.PublishError("remote_object_size_limit")
        return body, dict(headers)

    def put(self, path, body, condition, policy):
        self.calls.append(("put", path, dict(condition), policy))
        if path == m.POINTER_PATH and self.race:
            self.race(self)
            self.race = None
        existing = self.rows.get(path)
        if (condition.get("if-none-match") == "*" and existing) or (
                "if-match" in condition and (not existing or condition["if-match"] != existing[1]["etag"])):
            raise m.PublishError("http_request_failed", 412)
        self.seed(path, body)
        if path == m.POINTER_PATH and self.fail_after_cas:
            self.fail_after_cas = False
            raise m.PublishError("http_request_failed", 500)
        return dict(self.rows[path][1])

    def public(self, method, url, headers, body, maximum):
        assert method == "GET" and url.startswith(m.PUBLIC_BASE)
        assert not any(key.lower() in ("authorization", "x-amz-security-token") for key in headers)
        path = url[len(m.PUBLIC_BASE):]
        alias = path in ("latest.json", "latest.p7s")
        if alias:
            pointer = m.parse_json(self.rows[m.POINTER_PATH][0])
            path = m.feed_directory(pointer) + path
        payload, metadata = self.get(path, maximum)
        return payload, {**metadata, "cache-control": "no-store" if alias else m.IMMUTABLE}


class PublisherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mir2-native-publication-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).absolute()
        self.s3 = FakeS3()
        self.objects = []
        feed = b'{"schema":"root-signed-fixture","sequence":7}\n'
        signature = b"externally-verified-CMS-fixture"
        self.candidate = {
            "schema": m.POINTER_SCHEMA, "channel": "invited", "platform": "windows-x64",
            "sequence": 7, "sourceRevision": "a" * 40, "feedSha256": m.sha256(feed),
            "signatureSha256": m.sha256(signature),
        }
        directory = m.feed_directory(self.candidate)
        for path, data in [(directory + "latest.json", feed), (directory + "latest.p7s", signature),
                           ("releases/r7/engine.gz", b"\x1f\x8b\x08literal-archive")]:
            source = self.root / ("source-" + str(len(self.objects)))
            source.write_bytes(data)
            self.objects.append({"path": path, "source": str(source), "size": len(data), "sha256": m.sha256(data)})
        self.verification_path = self.root / "verification.json"
        self.plan_path = self.root / "plan.json"
        self.write_plan()
        self.credential_path = self.root / "private-credentials.json"
        self.credential_path.write_bytes(m.canonical_json({
            "accountId": m.ACCOUNT_ID, "accessKeyId": "1" * 32, "secretAccessKey": "2" * 64,
        }))
        self.counter = 0

    def write_plan(self, expected=None):
        verification = {
            "schema": m.VERIFICATION_SCHEMA, "passed": True, "candidateVerified": True, "cmsVerified": True,
            **{k: self.candidate[k] for k in ["sourceRevision", "sequence", "feedSha256", "signatureSha256"]},
            "objectsSha256": m.sha256(m.canonical_json(m.object_closure(self.objects))),
        }
        self.verification_path.write_bytes(m.canonical_json(verification))
        self.plan = {
            "schema": m.PLAN_SCHEMA, "expectedCurrent": expected, "candidate": self.candidate,
            "verificationReceipt": {"path": str(self.verification_path), "sha256": m.sha256(self.verification_path.read_bytes())},
            "objects": self.objects,
        }
        self.plan_path.write_bytes(m.canonical_json(self.plan))

    def run_mode(self, mode="stage", stage=None, public=None):
        self.counter += 1
        output = self.root / f"attempt-{self.counter}.json"
        with mock.patch.object(m, "load_credentials", return_value={"private": "test only"}):
            receipt = m.run(self.plan_path, self.credential_path, output, mode, stage,
                            client_factory=lambda _: self.s3, public_transport=public or self.s3.public)
        return receipt, output

    def test_stage_is_upload_only_and_promote_is_single_pointer_cas(self):
        stage, path = self.run_mode()
        self.assertTrue(stage["passed"])
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)
        self.assertTrue(all(c[2] == {"if-none-match": "*"} for c in self.s3.calls if c[0] == "put"))
        before = len(self.s3.calls)
        promoted, _ = self.run_mode("promote", path)
        self.assertTrue(promoted["passed"])
        self.assertTrue(promoted["aliasesVerified"])
        self.assertTrue(promoted["pointerChanged"])
        puts = [c for c in self.s3.calls[before:] if c[0] == "put"]
        self.assertEqual(len(puts), 1)
        self.assertEqual(puts[0][1:], (m.POINTER_PATH, {"if-none-match": "*"}, "no-store"))
        self.assertEqual(m.parse_json(self.s3.rows[m.POINTER_PATH][0]), self.candidate)

    def test_owned_same_bytes_resume_requires_full_remote_hash(self):
        first, _ = self.run_mode()
        second, _ = self.run_mode()
        self.assertTrue(first["passed"] and second["passed"])
        self.assertEqual(sorted(second["resumedObjects"]), sorted(item["path"] for item in self.objects))
        self.assertEqual(second["uploadedObjects"], [])
        self.s3.seed(self.objects[0]["path"], b"x" * self.objects[0]["size"])
        failure, _ = self.run_mode()
        self.assertFalse(failure["passed"])
        self.assertEqual(failure["failure"]["code"], "immutable_object_conflict")
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)

    def test_public_verification_failure_preserves_receipt_and_cannot_promote(self):
        def corrupt(method, url, headers, body, maximum):
            data, meta = self.s3.public(method, url, headers, body, maximum)
            return b"x" * len(data), meta
        failed, path = self.run_mode(public=corrupt)
        self.assertFalse(failed["passed"])
        self.assertTrue(path.exists())
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)
        promoted, _ = self.run_mode("promote", path)
        self.assertEqual(promoted["failure"]["code"], "passed_bound_stage_receipt_required")
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)

    def test_promote_rehashes_public_objects_before_cas(self):
        stage, path = self.run_mode()
        self.assertTrue(stage["passed"])
        self.s3.seed(self.objects[-1]["path"], b"broken")
        result, _ = self.run_mode("promote", path)
        self.assertFalse(result["passed"])
        self.assertFalse(result["pointerAttempted"])
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)

    def test_exact_prior_pointer_bytes_are_required_and_if_match_is_used(self):
        previous = {**self.candidate, "sequence": 6, "sourceRevision": "b" * 40}
        self.write_plan(previous)
        self.s3.seed(m.POINTER_PATH, m.canonical_json(previous))
        stage, path = self.run_mode()
        self.assertTrue(stage["passed"])
        etag = self.s3.rows[m.POINTER_PATH][1]["etag"]
        result, _ = self.run_mode("promote", path)
        self.assertTrue(result["passed"])
        last_put = [c for c in self.s3.calls if c[0] == "put"][-1]
        self.assertEqual(last_put[2], {"if-match": etag})
        self.s3.seed(m.POINTER_PATH, m.canonical_json({**previous, "sourceRevision": "c" * 40}))
        mismatch, _ = self.run_mode()
        self.assertEqual(mismatch["failure"]["code"], "current_pointer_mismatch")

    def test_downgrade_or_same_sequence_and_stale_plan_are_rejected(self):
        for sequence in [7, 8]:
            previous = {**self.candidate, "sequence": sequence}
            self.write_plan(previous)
            result, _ = self.run_mode()
            self.assertEqual(result["failure"]["code"], "sequence_downgrade_or_reuse")
        self.assertEqual(self.s3.calls, [])

    def test_initial_and_existing_pointer_races_fail_single_cas(self):
        for previous in [None, {**self.candidate, "sequence": 6}]:
            self.s3 = FakeS3()
            if previous:
                self.s3.seed(m.POINTER_PATH, m.canonical_json(previous))
            self.write_plan(previous)
            stage, path = self.run_mode()
            self.assertTrue(stage["passed"])
            winner = {**self.candidate, "sequence": 8, "sourceRevision": "c" * 40}
            self.s3.race = lambda store: store.seed(m.POINTER_PATH, m.canonical_json(winner))
            failed, _ = self.run_mode("promote", path)
            self.assertEqual(failed["failure"]["code"], "pointer_compare_and_swap_failed")
            self.assertEqual(m.parse_json(self.s3.rows[m.POINTER_PATH][0]), winner)
            self.assertFalse(failed["pointerChanged"])

    def test_uncertain_commit_is_retained_and_explicit_retry_is_idempotent(self):
        _, stage_path = self.run_mode()
        self.s3.fail_after_cas = True
        failure, failed_path = self.run_mode("promote", stage_path)
        self.assertFalse(failure["passed"])
        self.assertTrue(failure["pointerOutcomeUnknown"])
        self.assertTrue(failed_path.exists())
        before = len([c for c in self.s3.calls if c[0] == "put"])
        success, _ = self.run_mode("promote", stage_path)
        self.assertTrue(success["passed"])
        self.assertTrue(success["alreadyPromoted"])
        self.assertFalse(success["pointerChanged"])
        self.assertEqual(before, len([c for c in self.s3.calls if c[0] == "put"]))
        self.assertFalse(json.loads(failed_path.read_text())["passed"])

    def test_promote_requires_bound_successful_stage_and_same_plan(self):
        result, _ = self.run_mode("promote")
        self.assertEqual(result["failure"]["code"], "separate_promote_stage_receipt_required")
        _, path = self.run_mode()
        self.plan_path.write_bytes(json.dumps(self.plan, indent=2).encode())
        changed, _ = self.run_mode("promote", path)
        self.assertEqual(changed["failure"]["code"], "passed_bound_stage_receipt_required")
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)

    def test_existing_receipt_is_never_overwritten(self):
        result, path = self.run_mode()
        preserved = path.read_bytes()
        with self.assertRaises(m.PublishError):
            m.run(self.plan_path, self.credential_path, path, "stage")
        self.assertEqual(path.read_bytes(), preserved)
        self.assertTrue(result["passed"])

    def test_root_receipt_claims_binding_and_object_closure_are_required(self):
        for field, value in [("cmsVerified", False), ("candidateVerified", False),
                             ("sourceRevision", "c" * 40), ("objectsSha256", "0" * 64)]:
            self.write_plan()
            verification = m.parse_json(self.verification_path.read_bytes())
            verification[field] = value
            self.verification_path.write_bytes(m.canonical_json(verification))
            self.plan["verificationReceipt"]["sha256"] = m.sha256(self.verification_path.read_bytes())
            self.plan_path.write_bytes(m.canonical_json(self.plan))
            result, _ = self.run_mode()
            self.assertFalse(result["passed"])
            self.assertEqual(self.s3.calls, [])

    def test_invalid_paths_duplicate_keys_sizes_and_local_hash_are_rejected(self):
        for path in ["channels/invited.json", "releases/../a", "feeds//a", "installers/.hidden",
                     "releases/%2e%2e/a", "https://attacker/a", "releases/a?token=x"]:
            with self.assertRaises(m.PublishError):
                m.relative_path(path)
        for data in [b'{"x":1,"x":2}', b'{"x":NaN}', b"\xff"]:
            with self.assertRaises(m.PublishError):
                m.parse_json(data)
        self.plan["objects"][0]["size"] = m.MAX_METADATA + 1
        self.plan_path.write_bytes(m.canonical_json(self.plan))
        self.assertFalse(self.run_mode()[0]["passed"])
        self.objects[0]["size"] = Path(self.objects[0]["source"]).stat().st_size
        self.write_plan()
        Path(self.objects[-1]["source"]).write_bytes(b"wrong")
        self.assertEqual(self.run_mode()[0]["failure"]["code"], "local_object_hash_mismatch")

    def test_hardlinked_local_sources_are_rejected_before_upload(self):
        source = Path(self.objects[-1]["source"])
        linked = self.root / "hardlinked-source"
        linked.hardlink_to(source)
        failed, _ = self.run_mode()
        self.assertEqual(failed["failure"]["code"], "regular_single_link_input_required")
        self.assertEqual(self.s3.calls, [])

    def test_public_encoding_or_mutable_policy_blocks_stage_and_pointer_write(self):
        for metadata in [{"content-encoding": "gzip"}, {"cache-control": "no-store"}]:
            self.s3 = FakeS3()
            def invalid_public(method, url, headers, body, maximum):
                payload, actual = self.s3.public(method, url, headers, body, maximum)
                return payload, {**actual, **metadata}
            failed, _ = self.run_mode(public=invalid_public)
            self.assertFalse(failed["passed"])
            self.assertFalse(failed["pointerAttempted"])
            self.assertNotIn(m.POINTER_PATH, self.s3.rows)

    def test_pointer_encoding_malformed_schema_and_noncanonical_bytes_fail_closed(self):
        for data, encoding in [(b"{}", None), (m.canonical_json(self.candidate), "gzip"),
                               (json.dumps(self.candidate, indent=2).encode(), None)]:
            self.s3 = FakeS3()
            self.s3.seed(m.POINTER_PATH, data, encoding)
            failed, _ = self.run_mode()
            self.assertFalse(failed["passed"])
            self.assertFalse(any(c[0] == "put" for c in self.s3.calls))

    def test_credentials_are_private_json_only_and_no_raw_errors_escape(self):
        with mock.patch.object(m, "check_private_file") as private:
            value = m.load_credentials(self.credential_path)
            self.assertEqual(value["accountId"], m.ACCOUNT_ID)
            self.assertEqual(private.call_count, 2)
        with mock.patch.object(m, "check_private_file", side_effect=m.PublishError("credential_file_not_private")):
            with mock.patch.object(m, "safe_local_file") as read:
                with self.assertRaises(m.PublishError):
                    m.load_credentials(self.credential_path)
                read.assert_not_called()
        secret = "SECRET_MUST_NOT_ESCAPE"
        for error in [urllib.error.URLError(secret),
                      urllib.error.HTTPError("https://example/" + secret, 403, secret, {}, None)]:
            with mock.patch.object(urllib.request.OpenerDirector, "open", side_effect=error):
                with self.assertRaises(m.PublishError) as captured:
                    m.send_http("GET", m.PUBLIC_BASE + "latest.json", {}, None, 10)
                self.assertNotIn(secret, str(captured.exception))
        with mock.patch("sys.stdout", new_callable=io.StringIO) as stdout:
            with self.assertRaises(SystemExit):
                m.SafeArgumentParser().parse_args(["--secret", secret])
            self.assertNotIn(secret, stdout.getvalue())

    def test_sigv4_binds_fixed_origin_prefix_payload_condition_and_session_token(self):
        credentials = {"accountId": m.ACCOUNT_ID, "accessKeyId": "1" * 32, "secretAccessKey": "2" * 64,
                       "sessionToken": "SESSION_TEST_VALUE"}
        now = datetime(2026, 10, 2, 1, 2, 3, tzinfo=timezone.utc)
        url, headers = m.sign_v4(credentials, "PUT", "releases/r7/a.gz", b"archive",
                                 {"if-none-match": "*", "cache-control": m.IMMUTABLE}, now)
        self.assertEqual(url, m.S3_ORIGIN + "/" + m.BUCKET + "/" + m.PREFIX + "releases/r7/a.gz")
        self.assertEqual(headers["x-amz-content-sha256"], hashlib.sha256(b"archive").hexdigest())
        self.assertIn("20261002/auto/s3/aws4_request", headers["authorization"])
        self.assertIn("if-none-match", headers["authorization"])
        self.assertIn("x-amz-security-token", headers["authorization"])
        self.assertEqual(headers["x-amz-date"], "20261002T010203Z")
        _, changed = m.sign_v4(credentials, "PUT", "releases/r7/a.gz", b"different",
                               {"if-none-match": "*", "cache-control": m.IMMUTABLE}, now)
        self.assertNotEqual(headers["authorization"], changed["authorization"])
        for unsafe in ["../../other", "mir2/v/web/a", "releases/a?token=x"]:
            with self.assertRaises(m.PublishError):
                m.sign_v4(credentials, "PUT", unsafe, b"a", {}, now)

    def test_redirects_and_nonconditional_puts_are_forbidden(self):
        self.assertIsNone(m.NoRedirect().redirect_request(None, None, None, None, None, None))
        client = m.S3Client({"accessKeyId": "1" * 32, "secretAccessKey": "2" * 64})
        for condition in [{}, {"if-match": "*"}, {"if-none-match": "other"}]:
            with self.assertRaises(m.PublishError):
                client.put("releases/r7/a.gz", b"a", condition, m.IMMUTABLE)


class FiveClassPublisherTests(unittest.TestCase):
    # Reuse only the original fixture construction, not its test methods.
    setUp = PublisherTests.setUp
    write_plan = PublisherTests.write_plan

    def modern_plan(self):
        self.write_plan()
        verification = m.parse_json(self.verification_path.read_bytes())
        verification.update(schema=m.FIVE_CLASS_VERIFICATION_SCHEMA, downloadScope="five-class")
        self.verification_path.write_bytes(m.canonical_json(verification))
        self.plan.update(schema=m.FIVE_CLASS_PLAN_SCHEMA, downloadScope="five-class")
        self.plan["verificationReceipt"]["sha256"] = m.sha256(self.verification_path.read_bytes())
        self.plan_path.write_bytes(m.canonical_json(self.plan))

    def run_modern(self, mode="stage", stage=None, factory=None):
        self.counter += 1
        output = self.root / f"modern-attempt-{self.counter}.json"
        self.s3.download_scope = "five-class"
        urls = []
        def public(method, url, headers, body, maximum):
            base = m.PUBLIC_BASE + "five-class/"
            self.assertTrue(url.startswith(base))
            urls.append(url)
            return self.s3.public(method, m.PUBLIC_BASE + url[len(base):], headers, body, maximum)
        with mock.patch.object(m, "load_credentials", return_value={"private": "fixture only"}):
            result = m.run(self.plan_path, self.credential_path, output, mode, stage,
                           client_factory=factory or (lambda _, download_scope: self.s3), public_transport=public)
        return result, output, urls

    def test_modern_stage_and_cas_use_only_bound_scope_and_aliases(self):
        self.modern_plan()
        staged, path, urls = self.run_modern()
        self.assertTrue(staged["passed"])
        self.assertEqual(staged["schema"], m.FIVE_CLASS_RECEIPT_SCHEMA)
        self.assertEqual(staged["downloadScope"], "five-class")
        self.assertNotIn(m.POINTER_PATH, self.s3.rows)
        promoted, _, promotion_urls = self.run_modern("promote", path)
        self.assertTrue(promoted["passed"] and promoted["aliasesVerified"])
        self.assertEqual([c[1] for c in self.s3.calls if c[0] == "put"].count(m.POINTER_PATH), 1)
        self.assertEqual(promotion_urls[-2:], [m.PUBLIC_BASE + "five-class/latest.json", m.PUBLIC_BASE + "five-class/latest.p7s"])
        self.assertTrue(urls)
        # An otherwise identical passed stage receipt cannot be reused across scopes.
        changed = m.parse_json(path.read_bytes())
        changed["downloadScope"] = "legacy"
        path.write_bytes(m.canonical_json(changed))
        before = len([c for c in self.s3.calls if c[0] == "put"])
        rejected, _, _ = self.run_modern("promote", path)
        self.assertEqual(rejected["failure"]["code"], "stage_download_scope_mismatch")
        self.assertEqual(len([c for c in self.s3.calls if c[0] == "put"]), before)

    def test_fixed_s3_routes_have_independent_pointers_and_signatures(self):
        credentials = {"accessKeyId": "1" * 32, "secretAccessKey": "2" * 64}
        now = datetime(2026, 10, 11, tzinfo=timezone.utc)
        legacy, legacy_headers = m.sign_v4(credentials, "GET", m.POINTER_PATH, None, now=now)
        modern, modern_headers = m.sign_v4(credentials, "GET", m.POINTER_PATH, None, now=now, download_scope="five-class")
        self.assertEqual(legacy, m.S3_ORIGIN + "/" + m.BUCKET + "/" + m.PREFIX + m.POINTER_PATH)
        self.assertEqual(modern, m.S3_ORIGIN + "/" + m.BUCKET + "/" + m.PREFIX + "five-class/" + m.POINTER_PATH)
        self.assertNotEqual(legacy_headers["authorization"], modern_headers["authorization"])
        calls = []
        client = m.S3Client(credentials, lambda method, url, headers, body, maximum: (calls.append(url) or b"", {}), "five-class")
        client.get(m.POINTER_PATH, m.MAX_POINTER)
        client.put(m.POINTER_PATH, b"{}", {"if-none-match": "*"}, "no-store")
        self.assertEqual(calls, [modern, modern])
        for scope in ["other", "../", m.PUBLIC_BASE, "five-class/", None]:
            with self.assertRaises(m.PublishError):
                m.sign_v4(credentials, "GET", m.POINTER_PATH, None, download_scope=scope)

    def test_legacy_verification_and_injected_scope_cannot_authorize_modern_plan(self):
        self.modern_plan()
        verification = m.parse_json(self.verification_path.read_bytes())
        verification.pop("downloadScope")
        verification["schema"] = m.VERIFICATION_SCHEMA
        self.verification_path.write_bytes(m.canonical_json(verification))
        self.plan["verificationReceipt"]["sha256"] = m.sha256(self.verification_path.read_bytes())
        self.plan_path.write_bytes(m.canonical_json(self.plan))
        with self.assertRaises(m.PublishError):
            m.load_plan(self.plan_path)
        for scope in ["legacy", "../five-class", "https://example.com/"]:
            self.plan["downloadScope"] = scope
            self.plan_path.write_bytes(m.canonical_json(self.plan))
            with self.assertRaises(m.PublishError):
                m.load_plan(self.plan_path)
        self.assertEqual(self.s3.calls, [])

    def test_new_metadata_limit_is_scoped_and_old_limit_remains(self):
        path = "releases/r7/PACKAGE-MANIFEST.json"
        source = self.root / "large-metadata"
        body = b" " * (m.MAX_METADATA + 1)
        source.write_bytes(body)
        self.objects.append({"path": path, "source": str(source), "size": len(body), "sha256": m.sha256(body)})
        self.write_plan()
        with self.assertRaises(m.PublishError) as old:
            m.load_plan(self.plan_path)
        self.assertEqual(old.exception.code, "invalid_object_size_or_hash")
        self.modern_plan()
        loaded, _ = m.load_plan(self.plan_path)
        self.assertEqual(loaded["objects"][-1]["size"], len(body))
        self.assertEqual(m.object_limit(path), 32 * 1024 * 1024)
        self.assertEqual(m.object_limit(path, "five-class"), 64 * 1024 * 1024)
        self.assertEqual(m.object_limit("releases/r7/game.gz", "five-class"), m.MAX_ARTIFACT)
        self.plan["objects"][-1]["size"] = m.MAX_FIVE_CLASS_METADATA + 1
        self.plan_path.write_bytes(m.canonical_json(self.plan))
        with self.assertRaises(m.PublishError) as oversized:
            m.load_plan(self.plan_path)
        self.assertEqual(oversized.exception.code, "invalid_object_size_or_hash")

    def test_misconfigured_client_cannot_touch_other_pointer(self):
        self.modern_plan()
        def factory(_, download_scope):
            self.assertEqual(download_scope, "five-class")
            return FakeS3()  # Its implicit legacy scope must be rejected.
        rejected, _, urls = self.run_modern(factory=factory)
        self.assertEqual(rejected["failure"]["code"], "client_download_scope_mismatch")
        self.assertEqual(self.s3.calls, [])
        self.assertEqual(urls, [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
