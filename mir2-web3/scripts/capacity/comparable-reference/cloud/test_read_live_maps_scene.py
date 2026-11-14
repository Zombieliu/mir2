"""Offline SDK/transport fixtures only. Never authenticate or run remote code."""
import base64
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path, PurePosixPath
import shlex
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


spec = importlib.util.spec_from_file_location("read_live_maps_scene", Path(__file__).with_name("read_live_maps_scene.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def inventory():
    target = {"pid": 803500, "invocationId": "2645494fe370413593ea1b04c29707bc", "activeState": "active", "subState": "running",
              "processStartTicks": 1541658437, "controlGroup": "/system.slice/mir2-playtest.service", "systemdLimits": {}}
    return {"schema": "mir2.cloud-readonly-feature-inventory-readback.v2", "ok": True, "hostKeyPin": m.PIN,
        "hostKeyVerifiedBeforeCredentialInput": True, "authenticationConfirmed": True, "remoteExitStatus": 0,
        "queryResult": {"schema": "mir2.cloud-fresh-readonly-feature-inventory.v2", "ok": True,
            "identityStableAcrossRead": True, "targetBefore": target, "targetAfter": copy.deepcopy(target),
            "executable": {"path": "/opt/mir2-playtest/releases/" + m.BASELINE + "-equip-logout-20261112-v5/mir2-gateway",
                           "bytesHashed": 1234, "sha256": m.BASELINE_BINARY}, "health": {"revision": m.BASELINE, "ok": True}}}


def manifest():
    rows = [{"path": f"crystal-map-pack/{i:04}.map.gz", "bytes": 1, "sha256": "a" * 64} for i in range(m.FILE_COUNT)]
    rows[-1]["bytes"] += m.FILE_BYTES - m.FILE_COUNT
    # Keep every fixture member inside the real per-file 16 MiB bound.
    excess = rows[-1]["bytes"] - 1000000
    rows[-1]["bytes"] = 1000000
    for row in rows[:100]:
        take = min(excess, 500000)
        row["bytes"] += take
        excess -= take
    assert excess == 0
    return {"sourceRevision": m.CANDIDATE, "sourceTree": m.CANDIDATE + ":" + m.MAP_TREE,
            "fileCount": m.FILE_COUNT, "totalFileBytes": m.FILE_BYTES, "files": rows}


def configured():
    expected, rows = m.expectation(inventory(), manifest())
    return {"expected": expected, "roleHints": m.ROLE_HINTS, "inventorySha256": "1" * 64, "mapManifestSha256": "2" * 64}, rows


def success(config, rows):
    e = config["expected"]
    frame = {"target": e["target"], "executable": {"path": e["executablePath"], "bytes": e["binaryBytes"], "sha256": m.BASELINE_BINARY},
             "sourceRevision": m.BASELINE, "workingDirectory": "/opt/mir2-playtest", "mapRoot": "/opt/mir2-playtest/maps",
             "mapRootSource": "MIR2_CRYSTAL_MAP_PACK", "mapValueWasRelative": False}
    ident = {"pid": e["target"]["pid"], "invocationId": e["target"]["invocationId"], "startTicks": e["target"]["processStartTicks"]}
    fingerprints = [{**h, "startTicks": 1000, "executableSha256": "3" * 64, "commSha256": "4" * 64, "cgroupSha256": "5" * 64} for h in sorted(m.ROLE_HINTS, key=lambda h: h["role"])]
    return {"schema": m.SCHEMA, "complete": True, "mapsComplete": True, "readOnly": True, "sourceRevision": m.BASELINE,
        "binarySha256": m.BASELINE_BINARY, "identityBefore": ident, "identityAfter": copy.deepcopy(ident),
        "targetBefore": frame, "targetAfter": copy.deepcopy(frame), "files": rows, "fileCount": m.FILE_COUNT,
        "totalFileBytes": m.FILE_BYTES, "normalizedFilesSha256": e["normalizedFilesSha256"],
        "expectedManifestSha256": config["mapManifestSha256"], "expectedInventorySha256": config["inventorySha256"],
        "protectedDiscovery": [{**h, "status": "identified"} for h in m.ROLE_HINTS],
        "legacyRolesComplete": True, "protectedRolesComplete": True, "protectedFingerprintsBefore": fingerprints, "protectedFingerprintsAfter": copy.deepcopy(fingerprints),
        "rolesJSON": {"roles": [h for h in m.ROLE_HINTS if h["role"] not in ["old-gateway", "old-zone"]]},
        "startedAtUnixMs": 100000, "atUnixMs": 100500, "serviceOperations": [], "configurationChanges": [], "remoteQueryMaxSeconds": 20}


class SceneReadTests(unittest.TestCase):
    def setUp(self):
        self.remote = m.remote_namespace()
        self.remote["DEADLINE"] = time.monotonic() + 20

    def test_real_count_manifest_and_expected_identity(self):
        config, rows = configured()
        self.assertEqual(len(rows), 1620)
        self.assertEqual(sum(r["bytes"] for r in rows), 42111309)
        self.assertEqual(m.validate_result(success(config, rows), config, rows)["identityBefore"]["pid"], 803500)

    def test_old_inventory_is_expectation_not_fresh_proof(self):
        old = inventory()
        old["queryResult"]["targetAfter"]["pid"] = 999
        with self.assertRaisesRegex(ValueError, "stable_expected_inventory_required"):
            m.expectation(old, manifest())

    def test_actual_v5_state_cwd_and_historical_absolute_map_root(self):
        r = self.remote
        actual_cwd = "/var/lib/mir2-playtest/state"
        actual_root = "/opt/mir2-playtest/releases/d53f9e3b22e2b92c2dc86644b6ae370fc1acab53-r16-qa/crystal-map-pack"
        raw = b"PRIVATE_AUTH=not-for-output\0MIR2_CRYSTAL_MAP_PACK=" + actual_root.encode() + b"\0"
        with patch.dict(r, {"P": PurePosixPath, "read": lambda *a: raw, "clean_absolute": lambda p: p}), patch.object(r["os"], "readlink", return_value=actual_cwd):
            value = r["map_location"](803500)
        self.assertEqual(value["workingDirectory"], actual_cwd)
        self.assertEqual(value["mapRoot"], actual_root)
        self.assertFalse(value["mapValueWasRelative"])
        self.assertNotIn("PRIVATE_AUTH", json.dumps(value))
        config, rows = configured()
        proof = success(config, rows)
        for key in ["targetBefore", "targetAfter"]:
            proof[key]["workingDirectory"] = actual_cwd
            proof[key]["mapRoot"] = actual_root
        self.assertTrue(m.validate_result(proof, config, rows)["complete"])

    def test_cwd_exception_is_exact_and_does_not_expand_map_scope(self):
        r = self.remote
        self.assertTrue(r["cwd_allowed"]("/var/lib/mir2-playtest/state"))
        for invalid in ["/var/lib/mir2-playtest/state-other", "/var/lib/another/state", "/opt/mir2-playtest-other", "/var/lib/mir2-playtest/state/../secret", "relative"]:
            with self.subTest(invalid=invalid):
                self.assertFalse(r["cwd_allowed"](invalid))
        with patch.dict(r, {"P": PurePosixPath, "read": lambda *a: b"MIR2_CRYSTAL_MAP_PACK=crystal-map-pack\0", "clean_absolute": lambda p: p}), patch.object(r["os"], "readlink", return_value="/var/lib/mir2-playtest/state"):
            with self.assertRaisesRegex(r["AuditError"], "map_root_outside_readonly_scope"):
                r["map_location"](803500)

    def test_wrong_source_and_binary_and_health_rejected(self):
        for key in ["source", "binary", "health"]:
            with self.subTest(key=key):
                old = inventory()
                q = old["queryResult"]
                if key == "source":
                    q["health"]["revision"] = m.CANDIDATE
                elif key == "binary":
                    q["executable"]["sha256"] = "0" * 64
                else:
                    q["health"]["ok"] = False
                with self.assertRaises(ValueError):
                    m.expectation(old, manifest())

    def test_missing_map_or_duplicate_or_wrong_total_rejected(self):
        for kind in ["missing", "duplicate", "total", "traversal", "wrong_tree"]:
            with self.subTest(kind=kind):
                value = manifest()
                if kind == "missing":
                    value["files"].pop()
                elif kind == "duplicate":
                    value["files"][1] = value["files"][0]
                elif kind == "total":
                    value["totalFileBytes"] += 1
                elif kind == "traversal":
                    value["files"][0]["path"] = "crystal-map-pack/../secret.map.gz"
                else:
                    value["sourceTree"] = m.CANDIDATE + ":other"
                with self.assertRaises((ValueError, self.remote["AuditError"])):
                    m.expectation(inventory(), value)

    def test_fresh_identity_source_and_inode_samples_cannot_be_relabelled(self):
        config, rows = configured()
        for kind in ["identity", "source", "bytes", "snapshot", "protected", "timestamp"]:
            with self.subTest(kind=kind):
                value = success(config, rows)
                if kind == "identity":
                    value["identityAfter"]["startTicks"] += 1
                elif kind == "source":
                    value["sourceRevision"] = m.CANDIDATE
                elif kind == "bytes":
                    value["files"] = copy.deepcopy(rows)
                    value["files"][0]["sha256"] = "0" * 64
                elif kind == "snapshot":
                    value["targetAfter"]["mapRoot"] += "-changed"
                elif kind == "protected":
                    value["protectedFingerprintsAfter"] = [{"pid": 999}]
                else:
                    value["atUnixMs"] += 30000
                with self.assertRaises(ValueError):
                    m.validate_result(value, config, rows)

    def test_unknown_vpn_cannot_be_promoted_complete(self):
        config, rows = configured()
        value = success(config, rows)
        value["protectedDiscovery"][-1]["status"] = "unknown_family"
        value["protectedRolesComplete"] = False
        value["protectedFingerprintsBefore"] = [f for f in value["protectedFingerprintsBefore"] if f["role"] != "vpn-0"]
        value["protectedFingerprintsAfter"] = copy.deepcopy(value["protectedFingerprintsBefore"])
        value["rolesJSON"] = {"roles": []}
        with self.assertRaisesRegex(ValueError, "complete_protected_roles_required"):
            m.validate_result(value, config, rows)
        value["complete"] = False
        self.assertFalse(m.validate_result(value, config, rows)["complete"])

    def test_only_exact_known_vpn_comm_and_executable_pair(self):
        family = self.remote["family"]
        self.assertEqual(family("vpn-0", "xray-linux-amd6", "/usr/local/bin/xray-linux-amd64"), "known-vpn")
        self.assertEqual(family("vpn-0", "sing-box", "/usr/local/bin/sing-box"), "known-vpn")
        self.assertIsNone(family("vpn-0", "unknown-sensitive-name", "/sensitive/secret"))
        self.assertIsNone(family("vpn-0", "xray", "/private/unrecognised-daemon"))
        self.assertEqual(family("validator-0", "gate14_validato", "/opt/gate14_validator"), "gate14-validator")

    def test_unknown_process_output_contains_no_name_exe_or_cgroup(self):
        r = self.remote
        with patch.dict(r, {"proc_state": lambda p: 100, "read": lambda *a: b"PRIVATE-NAME\n"}), patch.object(r["os"], "readlink", return_value="/private/path/PRIVATE-EXE"):
            rows, fps = r["protected"]([{"role": "vpn-0", "pid": 123}])
        self.assertEqual(rows, [{"role": "vpn-0", "pid": 123, "status": "unknown_family"}])
        self.assertEqual(fps, [])
        self.assertNotIn("PRIVATE", json.dumps(rows))

    def test_map_member_path_gate_rejects_traversal_and_non_map(self):
        r = self.remote
        # Test the production name gate before any open of an unexpected file.
        for name in ["../wallet.map.gz", "/absolute.map.gz", "secret.json", "a/../../x.map.gz", "x\\y.map.gz"]:
            with self.subTest(name=name), self.assertRaises(r["AuditError"]):
                r["relative_name"](name)
        self.assertEqual(r["relative_name"]("nested/D341.map.gz"), "nested/D341.map.gz")

    def test_real_all_bytes_hash_and_mutation_detection(self):
        r = self.remote
        root = Path(tempfile.mkdtemp(prefix="mir2-scene-live-reader-test-"))
        self.assertEqual(root.drive.lower(), "c:")
        path = root / "actual.map.gz"
        data = bytes(range(256)) * 9000
        path.write_bytes(data)
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_BINARY", 0))
        try:
            row, prefix = r["hash_fd"](fd, len(data))
        finally:
            os.close(fd)
        self.assertEqual(row, {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
        self.assertEqual(prefix, data[:128])
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_BINARY", 0))
        actual = os.fstat(fd)
        changed = SimpleNamespace(**{k: getattr(actual, k) for k in ["st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_mode", "st_nlink"]})
        changed.st_mtime_ns += 1
        try:
            with patch.object(r["os"], "fstat", side_effect=[actual, changed]), self.assertRaisesRegex(r["AuditError"], "file_changed_during_hash"):
                r["hash_fd"](fd, len(data))
        finally:
            os.close(fd)

    def test_map_file_size_and_total_bound(self):
        r = self.remote
        expected = {"fileCount": 2, "totalFileBytes": 2, "normalizedFilesSha256": "0" * 64}
        with patch.dict(r, {"tree": lambda p: ([("one.map.gz", ())], [])}), self.assertRaisesRegex(r["AuditError"], "map_count_mismatch"):
            r["map_files"](Path("unused"), expected)

    def test_remote_deadline_is_enforced(self):
        r = self.remote
        r["DEADLINE"] = time.monotonic() - 1
        with self.assertRaisesRegex(r["AuditError"], "remote_query_budget"):
            r["left"]()
        self.assertEqual(m.REMOTE_SECONDS, 20)
        self.assertEqual(m.LOCAL_SECONDS, 60)

    def test_missing_live_manifest_is_explicit_not_fabricated(self):
        r = self.remote
        with patch.object(Path, "exists", return_value=False), patch.object(Path, "is_symlink", return_value=False):
            result = r["live_manifest"]({"executablePath": "/opt/mir2-playtest/releases/test/mir2-gateway"})
        self.assertEqual(result["present"], False)
        self.assertEqual(result["reason"], "not_installed_at_release_root")
        self.assertNotIn("sha256", result)

    def test_single_complete_frame_output_bound(self):
        raw = m.MARKER + b'{"schema":"mir2.scene-live-map-proof.v6","complete":false}\n'
        self.assertFalse(m.parse_frame(raw)["complete"])
        for broken in [raw[:-1], raw + raw, b"other", raw + b"x" * m.OUTPUT_BOUND]:
            with self.subTest(length=len(broken)), self.assertRaises(ValueError):
                m.parse_frame(broken)

    def test_remote_command_closed_stdin_and_readonly_scope(self):
        config, unused = configured()
        argv = shlex.split(m.command(config))
        self.assertEqual(argv[:5], ["sudo", "-n", "--", "python3", "-u"])
        self.assertEqual(argv[5], "-c")
        self.assertEqual(argv[6], m.REMOTE)
        self.assertEqual(json.loads(base64.b64decode(argv[7])), config)
        self.assertNotIn("systemctl start", m.REMOTE)
        self.assertNotIn("cmdline", m.REMOTE)
        self.assertIn("MIR2_CRYSTAL_MAP_PACK=", m.REMOTE)

    def test_default_entry_has_no_network_or_credential_input(self):
        with patch.object(m, "collect", side_effect=AssertionError("network forbidden")), contextlib.redirect_stdout(io.StringIO()):
            result = m.main(["--expected-inventory", "C:/unused", "--expected-inventory-sha256", "1" * 64,
                             "--map-manifest", "C:/unused2", "--map-manifest-sha256", "2" * 64, "--output", "C:/unused3"])
        self.assertEqual(result, 2)

    def fake_transport(self, host_ok=True, actual_status=0):
        key = SimpleNamespace(get_name=lambda: "ssh-ed25519", asbytes=lambda: b"synthetic-public-host-key")
        transport = Mock()
        transport.get_remote_server_key.return_value = key
        transport.is_authenticated.return_value = True
        channel = Mock()
        channel.recv_stderr_ready.return_value = False
        channel.recv_ready.side_effect = [True, False, False]
        channel.recv.return_value = m.MARKER + b'{"schema":"mir2.scene-live-map-proof.v6","complete":false}\n'
        channel.exit_status_ready.return_value = True
        channel.recv_exit_status.return_value = actual_status
        transport.open_session.return_value = channel
        package = SimpleNamespace(Transport=Mock(return_value=transport))
        correct_pin = "SHA256:" + base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip("=")
        return transport, channel, package, correct_pin if host_ok else m.PIN

    def test_host_pin_before_credential_and_no_retry_or_stdin_password(self):
        transport, channel, package, pin = self.fake_transport()
        config, unused = configured()
        credential = Mock(return_value="DO-NOT-PERSIST-THIS-SSH-SECRET")
        with patch.dict(sys.modules, {"paramiko": package}), patch.object(m, "PIN", pin), patch.object(m.socket, "create_connection", return_value=Mock()), patch.object(m.getpass, "getpass", credential), patch.object(m.threading, "Timer", return_value=Mock()), contextlib.redirect_stdout(io.StringIO()):
            raw, evidence = m.collect(config)
        transport.auth_password.assert_called_once_with("ubuntu", "DO-NOT-PERSIST-THIS-SSH-SECRET", fallback=False)
        channel.shutdown_write.assert_called_once()
        channel.exec_command.assert_called_once()
        self.assertTrue(channel.exec_command.call_args.args[0].startswith("sudo -n -- "))
        self.assertEqual(evidence["remoteExitStatus"], 0)
        self.assertEqual(evidence["remoteStdinBytes"], 0)
        self.assertNotIn("DO-NOT-PERSIST", json.dumps(evidence) + str(raw))
        self.assertFalse(evidence["credentialsPersisted"])

    def test_wrong_host_key_never_asks_credential(self):
        transport, channel, package, unused = self.fake_transport(False)
        config, unused = configured()
        with patch.dict(sys.modules, {"paramiko": package}), patch.object(m.socket, "create_connection", return_value=Mock()), patch.object(m.getpass, "getpass", side_effect=AssertionError("must not prompt")) as prompt:
            raw, evidence = m.collect(config)
        prompt.assert_not_called()
        transport.auth_password.assert_not_called()
        channel.exec_command.assert_not_called()
        self.assertFalse(evidence["hostKeyVerifiedBeforeCredentialInput"])
        self.assertIsNone(evidence["remoteExitStatus"])

    def test_unknown_exit_is_not_zero(self):
        transport, channel, package, pin = self.fake_transport(actual_status=-1)
        config, unused = configured()
        with patch.dict(sys.modules, {"paramiko": package}), patch.object(m, "PIN", pin), patch.object(m.socket, "create_connection", return_value=Mock()), patch.object(m.getpass, "getpass", return_value="synthetic"), patch.object(m.threading, "Timer", return_value=Mock()), contextlib.redirect_stdout(io.StringIO()):
            unused, evidence = m.collect(config)
        self.assertIsNone(evidence["remoteExitStatus"])
        self.assertFalse(evidence["remoteExitStatusReceived"])


if __name__ == "__main__":
    unittest.main()
