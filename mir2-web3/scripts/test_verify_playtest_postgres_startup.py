#!/usr/bin/env python3
"""Offline stdlib tests; no Gateway, database, account or public connection.

The HTTP/WebSocket test uses socketpair(), without a listening port. Public
profile files and synthetic health frames are fixtures, not live acceptance.
"""
import ast
import base64
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import threading
import types
import unittest
from unittest import mock

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "playtest_startup", Path(__file__).with_name("verify-playtest-postgres-startup.py"))
TOOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOL)
REVISION = "a" * 40


def event_for(gameplay):
    return {"type": "realmInfo", "payload": copy.deepcopy(gameplay["expectedRealm"])}


def health_fixture():
    return {"ok": True, "revision": REVISION, "http": "ready", "ws": "ready",
            "identity_backend": "postgres", "gate15": None,
            "session_cache": {"backend": "redis", "configured": True, "healthy": True},
            "channel_identity": {"backend": "postgres", "durable": True}}


def main_assignment(name):
    """Execute the real child-environment expressions, without running main."""
    source = ast.parse(Path(TOOL.__file__).read_text(encoding="utf-8"))
    main = next(node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == "main")
    matches = [node.value for node in ast.walk(main) if isinstance(node, ast.Assign)
               and any(isinstance(target, ast.Name) and target.id == name for target in node.targets)]
    assert len(matches) == 1, name
    return compile(ast.Expression(matches[0]), TOOL.__file__, "eval")


class ProfileTests(unittest.TestCase):
    def source_fixture(self):
        owned = tempfile.TemporaryDirectory(prefix="mir2-startup-source-test-")
        self.addCleanup(owned.cleanup)
        root = Path(owned.name)
        paths = [TOOL.BASELINE_SOURCE]
        for name in ("platinum_fiveclass", "platinum_176"):
            paths += [f"packages/game-data/data/content_profiles/{name}.json",
                      f"packages/game-data/data/generated/{name}_bundle.json"]
        for relative in paths:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((TOOL.PROJECT_ROOT / relative).read_bytes())
        return root

    def test_default_profile_comes_from_the_shared_baseline_and_five_class_sources(self):
        gameplay = TOOL.resolve_gameplay_profile()
        baseline = json.loads((TOOL.PROJECT_ROOT / TOOL.BASELINE_SOURCE).read_text(encoding="utf-8"))
        self.assertEqual(gameplay["contentProfile"], baseline["contentProfile"])
        self.assertEqual(gameplay["questCadence"], baseline["questCadence"])
        self.assertEqual(gameplay["allowedClasses"], ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"])
        receipt = TOOL.validate_realm_info(event_for(gameplay), gameplay)
        self.assertTrue(receipt["r16GameplayBaselineMatched"])
        self.assertFalse(receipt["allowedClassesAdvertised"])
        for field in ("standardAuthentication", "ordinaryRegistrationLogin", "newCharacterCreation",
                      "existingSaveUpgrade", "ordinaryRelogin"):
            self.assertIs(receipt["coverage"][field], False, field)

    def test_legacy_is_explicit_and_cannot_be_reported_as_r16(self):
        legacy = TOOL.resolve_gameplay_profile("legacy-platinum-176")
        self.assertEqual(legacy["contentProfile"], "platinum_176")
        self.assertEqual(legacy["allowedClasses"], ["Warrior", "Wizard", "Taoist"])
        self.assertFalse(TOOL.validate_realm_info(event_for(legacy), legacy)["r16GameplayBaselineMatched"])
        with self.assertRaises(RuntimeError):
            TOOL.validate_realm_info(event_for(legacy), TOOL.resolve_gameplay_profile())
        for mode in ("", "auto", "platinum_176", "crystal_full"):
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                TOOL.resolve_gameplay_profile(mode)

    def test_baseline_drift_missing_extra_duplicate_and_boolean_schema_fail_closed(self):
        root = self.source_fixture()
        path = root / TOOL.BASELINE_SOURCE
        original = json.loads(path.read_text(encoding="utf-8"))
        variants = [{**original, "contentProfile": "platinum_176"},
                    {**original, "questCadence": "newcomer-v1"},
                    {**original, "questGuidance": "crystal"},
                    {**original, "onboarding": "web"}, {**original, "id": "other"},
                    {**original, "schema": True}, {**original, "extra": 1},
                    {key: value for key, value in original.items() if key != "onboarding"}, []]
        for value in variants:
            path.write_text(json.dumps(value), encoding="utf-8")
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                TOOL.resolve_gameplay_profile(project_root=root)
        path.write_text(json.dumps(original)[:-1] + ', "schema": 1}', encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            TOOL.resolve_gameplay_profile(project_root=root)

    def test_baseline_pin_is_portable_across_checkout_line_endings(self):
        root = self.source_fixture()
        path = root / TOOL.BASELINE_SOURCE
        before = TOOL.resolve_gameplay_profile(project_root=root)
        path.write_bytes(path.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        after = TOOL.resolve_gameplay_profile(project_root=root)
        self.assertEqual(before["baselineSha256"], after["baselineSha256"])

    def test_profile_class_order_and_bundle_identity_cannot_drift(self):
        root = self.source_fixture()
        path = root / "packages/game-data/data/content_profiles/platinum_fiveclass.json"
        original = json.loads(path.read_text(encoding="utf-8"))
        for classes in (["Warrior", "Wizard", "Taoist"],
                        ["Warrior", "Wizard", "Taoist", "Archer", "Assassin"],
                        ["Warrior", "Wizard", "Taoist", "Assassin", "Assassin"]):
            path.write_text(json.dumps({**original, "allowedClasses": classes}), encoding="utf-8")
            with self.subTest(classes=classes), self.assertRaises(RuntimeError):
                TOOL.resolve_gameplay_profile(project_root=root)
        path.write_text(json.dumps(original), encoding="utf-8")
        path = root / "packages/game-data/data/generated/platinum_fiveclass_bundle.json"
        bundle = json.loads(path.read_text(encoding="utf-8"))
        for field, value in (("profileId", "platinum_176"), ("profileVersion", True),
                             ("contentHash", "bad"), ("source", "wrong"), ("hashAlgorithm", "md5")):
            path.write_text(json.dumps({**bundle, field: value}), encoding="utf-8")
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                TOOL.resolve_gameplay_profile(project_root=root)

    def test_every_advertised_realm_identity_field_is_checked_without_taking_a_class_list_on_trust(self):
        gameplay = TOOL.resolve_gameplay_profile()
        for field in gameplay["expectedRealm"]:
            event = event_for(gameplay)
            event["payload"][field] = None
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                TOOL.validate_realm_info(event, gameplay)
        for value in (True, 1.0):
            event = event_for(gameplay)
            event["payload"]["profileVersion"] = value
            with self.assertRaises(RuntimeError):
                TOOL.validate_realm_info(event, gameplay)
        event = event_for(gameplay)
        event["payload"]["ratePolicy"]["goldMultiplier"] = True
        with self.assertRaises(RuntimeError):
            TOOL.validate_realm_info(event, gameplay)
        event = event_for(gameplay)
        event["payload"]["allowedClasses"] = gameplay["allowedClasses"]
        self.assertTrue(TOOL.validate_realm_info(event, gameplay)["allowedClassesAdvertised"])
        event["payload"]["allowedClasses"] = ["Warrior", "Wizard", "Taoist"]
        with self.assertRaises(RuntimeError):
            TOOL.validate_realm_info(event, gameplay)
        for value in ({"type": "other", "payload": {}}, {"type": "realmInfo", "payload": []}, []):
            with self.assertRaises(RuntimeError):
                TOOL.validate_realm_info(value, gameplay)

    def test_real_cli_defaults_and_legacy_selection_resolve_before_any_process_or_database(self):
        class StopBeforeSideEffects(Exception):
            pass
        args = ["verify", "--binary", "/fixture/gateway", "--maps", "/fixture/maps", "--psql", "/fixture/psql",
                "--redis-binary", "/fixture/redis", "--evidence", "/fixture/evidence", "--revision", REVISION]
        for tail, expected in (([], "r16"), (["--profile-mode", "legacy-platinum-176"], "legacy-platinum-176")):
            with mock.patch.object(sys, "argv", args + tail), \
                    mock.patch.object(TOOL, "resolve_gameplay_profile", side_effect=StopBeforeSideEffects) as resolve, \
                    mock.patch.object(TOOL.subprocess, "Popen") as popen, mock.patch.object(TOOL, "psql") as psql:
                with self.assertRaises(StopBeforeSideEffects):
                    TOOL.main()
                resolve.assert_called_once_with(expected)
                popen.assert_not_called()
                psql.assert_not_called()

    def test_actual_child_environment_ignores_inherited_credentials_and_profile_overrides(self):
        inherited = {"PATH": "fixture-bin", "MIR2_CONTENT_PROFILE": "crystal_full",
                     "MIR2_QUEST_CADENCE": "newcomer-v1", "MIR2_ALLOW_DEV_IDENTITY_SECRETS": "1",
                     "MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS": "1", "OPENAI_API_KEY": "SYNTHETIC_NOT_A_KEY"}
        context = {"root": Path("fixture-root"), "state": Path("fixture-state"), "recovery": Path("fixture-recovery"),
                   "args": types.SimpleNamespace(revision=REVISION, maps=Path("fixture-maps")),
                   "database_url": "postgres://startup_qa@127.0.0.1:23450/mir2_startup_qa_1_2",
                   "web_port": 23451, "tcp_port": 23452, "redis_port": 23453}
        with mock.patch.dict(os.environ, inherited, clear=True):
            context["base_env"] = eval(main_assignment("base_env"), vars(TOOL), context)
            self.assertEqual(set(context["base_env"]), {"PATH", "LANG", "HOME"})
            for mode in TOOL.PROFILE_MODES:
                context["gameplay"] = TOOL.resolve_gameplay_profile(mode)
                env = eval(main_assignment("env"), vars(TOOL), context)
                self.assertEqual(env["MIR2_CONTENT_PROFILE"], context["gameplay"]["contentProfile"])
                self.assertEqual(env["MIR2_QUEST_CADENCE"], context["gameplay"]["questCadence"])
                for key in inherited.keys() - {"PATH", "MIR2_CONTENT_PROFILE", "MIR2_QUEST_CADENCE"}:
                    self.assertNotIn(key, env)
                for key in ("MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES", "MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES",
                            "MIR2_REQUIRE_CHANNEL_IDENTITY_STORE", "MIR2_GATEWAY_REQUIRE_REDIS_CACHE"):
                    self.assertEqual(env[key], "1", key)
                for key in ("MIR2_PROCUREMENT_ENABLED", "MIR2_PROCUREMENT_TEST_CREDITS", "MIR2_SUI_USDC_ENABLED",
                            "MIR2_STRIPE_ENABLED", "MIR2_SPECTATOR_ENABLED", "MIR2_AI_LIVE_ENABLED"):
                    self.assertEqual(env[key], "0", key)
                self.assertEqual(env["MIR2_ALLOWED_WEB_ORIGINS"], TOOL.FIXTURE_ORIGIN)
                secret_names = ("MIR2_SAVE_RECOVERY_MAC_KEY", "MIR2_PASSKEY_AUTH_SECRET",
                                "MIR2_IDENTITY_SESSION_SECRET", "MIR2_IDENTITY_RECOVERY_PEPPER")
                self.assertEqual(len({env[key] for key in secret_names}), 4)

    def test_example_profile_values_match_the_canonical_default(self):
        text = (TOOL.PROJECT_ROOT / "infra/playtest/gateway.env.example").read_text(encoding="utf-8")
        values = dict(line.split("=", 1) for line in text.splitlines() if line and not line.startswith("#"))
        baseline = TOOL.resolve_gameplay_profile()["baseline"]
        self.assertEqual(values["MIR2_CONTENT_PROFILE"], baseline["contentProfile"])
        self.assertEqual(values["MIR2_QUEST_CADENCE"], baseline["questCadence"])
        self.assertIn("REQUIRED_PRIVATE_DB_PASSWORD", values["MIR2_ACCOUNT_STORE_DATABASE_URL"])

    def run_main_database_rejection(self, database_url, responses, expected):
        with tempfile.TemporaryDirectory(prefix="mir2-startup-gate-test-") as owned:
            root = Path(owned)
            binary = root / "never-executed-fixture"
            binary.write_bytes(b"offline fixture, not executable code")
            binary.chmod(0o700)
            maps = root / "maps"
            maps.mkdir()
            (maps / "0.map.gz").write_bytes(b"never read before the database gate")
            args = types.SimpleNamespace(binary=binary, psql=binary, redis_binary=binary,
                maps=maps, evidence=root, revision=REVISION, profile_mode="r16")
            with mock.patch.object(TOOL.argparse.ArgumentParser, "parse_args", return_value=args), \
                    mock.patch.object(TOOL.sys, "platform", "linux"), \
                    mock.patch.object(TOOL.os, "geteuid", return_value=1001, create=True), \
                    mock.patch.dict(os.environ, {"MIR2_STARTUP_TEST_DATABASE_URL": database_url}), \
                    mock.patch.object(TOOL, "psql", side_effect=responses) as psql, \
                    mock.patch.object(TOOL.subprocess, "Popen") as popen, \
                    mock.patch.object(TOOL, "read_runtime_profile") as handshake:
                with self.assertRaisesRegex(RuntimeError, expected):
                    TOOL.main()
                self.assertEqual(psql.call_count, len(responses))
                popen.assert_not_called()
                handshake.assert_not_called()
                self.assertFalse((root / "gateway-postgres-startup.json").exists())

    def test_actual_main_rejects_a_privileged_owner_or_nonempty_database_before_any_child(self):
        url = "postgres://startup_qa@127.0.0.1:23450/mir2_startup_qa_1_2"
        self.run_main_database_rejection(url, ["f"], "low-privilege owner")
        self.run_main_database_rejection(url, ["t", "1"], "fresh and empty")

    def test_actual_main_rejects_non_fixture_database_urls_before_query_or_process(self):
        for url in ("postgres://startup_qa@remote.invalid:5432/mir2_startup_qa_1_2",
                    "postgres://mir2_playtest@127.0.0.1:5432/mir2_playtest",
                    "postgres://startup_qa@127.0.0.1:5432/mir2_startup_qa_1_2?sslmode=disable",
                    "file:///fixture.json"):
            with self.subTest(url=url):
                self.run_main_database_rejection(url, [], "fresh private CI startup database")


class TransportAndHealthTests(unittest.TestCase):
    def test_existing_full_health_contract_rejects_each_required_durability_and_identity_gate(self):
        health = health_fixture()
        self.assertEqual(TOOL.validate_health(health, REVISION), (health["session_cache"], health["channel_identity"]))
        for path, value in ((["ok"], False), (["revision"], "b" * 40), (["http"], "not-ready"),
                            (["ws"], "not-ready"), (["identity_backend"], "file"), (["gate15"], {}),
                            (["session_cache", "backend"], "memory"), (["session_cache", "configured"], False),
                            (["session_cache", "healthy"], False), (["channel_identity", "backend"], "memory"),
                            (["channel_identity", "durable"], False)):
            changed = copy.deepcopy(health)
            target = changed
            for field in path[:-1]:
                target = target[field]
            target[path[-1]] = value
            with self.subTest(path=path), self.assertRaises(RuntimeError):
                TOOL.validate_health(changed, REVISION)

    def test_real_http_websocket_parser_over_socketpair_reads_profile_and_sends_only_close(self):
        gameplay = TOOL.resolve_gameplay_profile()
        client, server = socket.socketpair()
        client.settimeout(3)
        server.settimeout(3)
        self.addCleanup(client.close)
        self.addCleanup(server.close)
        captured = {}

        def peer():
            try:
                request = b""
                while not request.endswith(b"\r\n\r\n"):
                    part = server.recv(1)
                    if not part:
                        raise RuntimeError("fixture request closed early")
                    request += part
                captured["request"] = request
                lines = request.decode("ascii").split("\r\n")
                headers = dict(line.split(": ", 1) for line in lines[1:] if ": " in line)
                accept = base64.b64encode(hashlib.sha1(
                    (headers["Sec-WebSocket-Key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest())
                body = json.dumps(event_for(gameplay)).encode()
                frame = b"\x81\x7e" + len(body).to_bytes(2, "big") + body
                server.sendall(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
                               b"Connection: keep-alive, Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n" + frame)
                close = b""
                while len(close) < 8:
                    part = server.recv(8 - len(close))
                    if not part:
                        break
                    close += part
                captured["close"] = close
            except Exception as error:
                captured["error"] = error

        worker = threading.Thread(target=peer, daemon=True)
        worker.start()
        real_connection = TOOL.http.client.HTTPConnection("127.0.0.1", 23451, timeout=3)
        real_connection.sock = client
        with mock.patch.object(TOOL.http.client, "HTTPConnection", return_value=real_connection) as connection:
            receipt = TOOL.read_runtime_profile(23451, gameplay)
        worker.join(timeout=4)
        self.assertFalse(worker.is_alive())
        self.assertNotIn("error", captured)
        connection.assert_called_once_with("127.0.0.1", 23451, timeout=5)
        self.assertTrue(captured["request"].startswith(b"GET /ws HTTP/1.1\r\n"))
        self.assertIn(b"Origin: https://startup.invalid\r\n", captured["request"])
        close = captured["close"]
        self.assertEqual(close[:2], b"\x88\x82")
        self.assertEqual(bytes(value ^ close[2 + i] for i, value in enumerate(close[6:])), (1000).to_bytes(2, "big"))
        self.assertEqual(len(close), 8, "no authentication or gameplay message is sent")
        self.assertTrue(receipt["r16GameplayBaselineMatched"])

    def test_upgrade_frame_and_json_failures_cannot_produce_a_profile_receipt(self):
        gameplay = TOOL.resolve_gameplay_profile()
        expected_accept = base64.b64encode(hashlib.sha1(
            (base64.b64encode(b"0" * 16).decode() + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        valid_headers = {"Upgrade": "websocket", "Connection": "Upgrade", "Sec-WebSocket-Accept": expected_accept}
        cases = [(403, valid_headers, b""), (101, {**valid_headers, "Sec-WebSocket-Accept": "wrong"}, b""),
                 (101, valid_headers, b"\x81\x05{}"), (101, valid_headers, b"\x82\x02{}"),
                 (101, valid_headers, b"\x01\x02{}"), (101, valid_headers, b"\x81\x82{}"),
                 (101, valid_headers, b"\x81\x7f" + (65537).to_bytes(8, "big")),
                 (101, valid_headers, b"\x81\x04oops"), (101, valid_headers, b"\x81\x02{}")]
        for status, headers, frame in cases:
            response = types.SimpleNamespace(status=status, fp=io.BytesIO(frame),
                                             getheader=lambda key, default=None: headers.get(key, default), close=mock.Mock())
            connection = mock.Mock()
            connection.getresponse.return_value = response
            with self.subTest(status=status, frame=frame[:2]), \
                    mock.patch.object(TOOL.http.client, "HTTPConnection", return_value=connection), \
                    mock.patch.object(TOOL.secrets, "token_bytes", return_value=b"0" * 16):
                with self.assertRaises(RuntimeError):
                    TOOL.read_runtime_profile(23451, gameplay)
                connection.sock.sendall.assert_not_called()
                response.close.assert_called_once()
                connection.close.assert_called_once()


if __name__ == "__main__":
    unittest.main()
