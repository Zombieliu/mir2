"""Offline successor scope tests; never SSH, signal or run game artifacts."""
import ast
import base64
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
import time
import os
import subprocess
import tempfile
from types import SimpleNamespace
from unittest.mock import patch

HERE = Path(__file__).parent


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


m = module("scene_v7_under_test", HERE / "cloud_test_release_scene_v7.py")
v6 = module("scene_v7_v6_fixtures", HERE / "test_cloud_test_release_scene_v6.py")
assert hashlib.sha256((HERE / "test_cloud_test_release_scene_v6.py").read_bytes()).hexdigest() == "19be8a624a7ef1e409dfef275452cd6d9aeb8e85915368960a8418e98fb32293"


def roles():
    return m.role_pids([{"role": r["role"], "pid": r["pid"]} for r in m.REVIEWED_PROTECTED_PINS if r["role"] not in ["old-gateway", "old-zone"]])


def rows(legacy=False):
    return [{**copy.deepcopy(r), "outsideTargetCgroup": True} for r in m.REVIEWED_PROTECTED_PINS
            if (r["role"] in ["old-gateway", "old-zone"]) == legacy]


def scope():
    return {"scopeReviewSha256": m.REVIEWED_SCOPE_SHA256, "boundarySha256": m.digest(m.TARGET_CGROUP.encode()),
            "relations": copy.deepcopy(m.REVIEWED_RELATIONS), "typedHooks": {k: m.typed_array(b"a(sasbttttuii) 0\n") for k in m.HOOKS}, "vpnIdentity": "unknown"}


class FakeHost(v6.FakeHost):
    def __init__(self):
        self.legacy_full = rows(True)
        self.scope = scope()
        self.legacy_bound = False
        super().__init__()
        self.config.pop("sceneV6Pins", None)
        self.config["sceneV7Pins"] = {"predecessorState": {"sha256": "f" * 64}, "artifactProof": {"sha256": "e" * 64},
                                     "protectedFingerprintSha256": m.json_hash(m.fence_payload(self.snapshot()))}

    def snapshot(self):
        if not self.legacy_bound:
            old = next(r for r in m.REVIEWED_PROTECTED_PINS if r["role"] == "old-gateway")
            zone = next(r for r in m.REVIEWED_PROTECTED_PINS if r["role"] == "old-zone")
            self.old_identity = {"pid": old["pid"], "invocationId": "a" * 32, "startTicks": old["startTicks"]}
            self.zone_identity = {"pid": zone["pid"], "invocationId": "b" * 32, "startTicks": zone["startTicks"]}
            self.legacy_bound = True
        value = super().snapshot()
        value.update(legacyProtectedProcesses=copy.deepcopy(self.legacy_full), stopScope=copy.deepcopy(self.scope))
        return value

    def proof(self):
        value = {"schema": m.PROTECTED_SCHEMA, "readonly": True, "sampledAtUnixMs": round(time.time() * 1000),
                 "roles": self.config["protectedRoles"], "targetIdentity": self.current_identity.copy(), "targetRevision": self.h["revision"],
                 "targetBinarySha256": self.binary_sha, **m.fence_payload(self.snapshot()), "scopeReviewSha256": m.REVIEWED_SCOPE_SHA256,
                 "vpnIdentity": "unknown"}
        value["stableFingerprintSha256"] = m.json_hash(m.fence_payload(value))
        return value


class ScopedTransactions(unittest.TestCase):
    def setUp(self):
        for target, key, value in [(v6, "m", m), (v6.legacy, "m", m), (v6, "roles", roles), (v6, "fingerprints", rows)]:
            task_patch = patch.object(target, key, value)
            task_patch.start(); self.addCleanup(task_patch.stop)
        self.host = FakeHost()

    def call(self, action, state=None):
        self.host.config["expectedPid"] = self.host.current_identity["pid"]
        self.host.config["protectedProof"] = self.host.proof()
        return m.transaction(action, self.host, self.host.config, state)

    def prepare(self):
        value = self.call("prepare")
        self.assertTrue(value["passed"], value)
        return value["state"]

    def test_same_namespace_18_19_20_19_and_unknown_vpn_no_invention(self):
        state = self.prepare(); counts = [18]
        for action in ["install", "apply-caps", "restore-caps"]:
            result = self.call(action, state)
            self.assertTrue(result["passed"], result)
            state = result["state"]
            counts.append(len(result["after"]["environmentFiles"]))
            self.assertEqual(result["after"]["stopScope"]["vpnIdentity"], "unknown")
            self.assertEqual(len(result["after"]["protectedProcesses"]), 6)
            self.assertEqual(len(result["after"]["legacyProtectedProcesses"]), 2)
        self.assertEqual(counts, [18, 19, 20, 19])
        self.assertEqual(self.host.config["protectedRoles"], roles())
        for key in ["UNIT", "RELEASE_DROPIN", "RELEASE_ENV", "CAPS_DROPIN", "CAPS_ENV", "RELEASE_SUFFIX"]:
            self.assertEqual(getattr(m, key), getattr(v6.legacy.m, key))

    def test_aux_and_each_legacy_binary_or_start_drift_prevents_prepare(self):
        for group, index, key in [("additional", 0, "executableSha256"), ("additional", 0, "startTicks"),
                                  ("legacy_full", 0, "executableSha256"), ("legacy_full", 1, "commSha256"), ("legacy_full", 1, "cgroupSha256")]:
            original = copy.deepcopy(getattr(self.host, group))
            getattr(self.host, group)[index][key] = 1 if key == "startTicks" else "0" * 64
            result = self.call("prepare")
            self.assertFalse(result["passed"], (group, key))
            self.assertEqual(self.host.actions, [])
            setattr(self.host, group, original)

    def test_every_protected_inside_cgroup_flag_refuses(self):
        for group in ["additional", "legacy_full"]:
            for index in range(len(getattr(self.host, group))):
                getattr(self.host, group)[index]["outsideTargetCgroup"] = False
                result = self.call("prepare")
                self.assertFalse(result["passed"])
                self.assertEqual(self.host.actions, [])
                getattr(self.host, group)[index]["outsideTargetCgroup"] = True

    def test_nonempty_unknown_missing_hooks_refuse_without_actions(self):
        for key in m.HOOKS:
            for mutation in ["missing", "nonempty", "unknown"]:
                original = copy.deepcopy(self.host.scope)
                if mutation == "missing": self.host.scope["typedHooks"].pop(key)
                elif mutation == "nonempty": self.host.scope["typedHooks"][key].update(arrayCount=1, empty=False)
                else: self.host.scope["typedHooks"][key] = m.typed_unknown("property-unavailable", b"")
                self.assertFalse(self.call("prepare")["passed"], (key, mutation))
                self.assertEqual(self.host.actions, [])
                self.host.scope = original

    def test_every_new_or_missing_relation_refuses_before_stage(self):
        for key in m.REVIEWED_RELATIONS:
            old = copy.deepcopy(self.host.scope)
            self.host.scope["relations"][key].append("caddy.service")
            self.assertFalse(self.call("prepare")["passed"], key)
            self.assertEqual(self.host.actions, [])
            self.host.scope = old
        self.host.scope["relations"].pop("ConflictedBy")
        self.assertFalse(self.call("prepare")["passed"])

    def test_prepared_scope_or_optional_process_cannot_be_dropped(self):
        extra = {"role": "vpn-0", "pid": 99991, "startTicks": 9999100, "executableSha256": "9" * 64,
                 "commSha256": "8" * 64, "cgroupSha256": "7" * 64, "outsideTargetCgroup": True}
        self.host.additional.append(extra)
        self.host.config["protectedRoles"] = m.role_pids(roles() + [{"role": "vpn-0", "pid": 99991}])
        self.host.config["sceneV7Pins"]["protectedFingerprintSha256"] = m.json_hash(m.fence_payload(self.host.snapshot()))
        state = self.prepare()
        self.host.additional.pop()
        self.host.config["protectedRoles"] = roles()
        self.host.config["sceneV7Pins"]["protectedFingerprintSha256"] = m.json_hash(m.fence_payload(self.host.snapshot()))
        self.host.actions.clear()
        self.assertFalse(self.call("install", state)["passed"])
        self.assertEqual(self.host.actions, [])

    def test_new_session_cas_auth_limits_sigkill_and_rollback_guards_stay(self):
        for kind in ["session", "redis", "auth", "sigkill", "config"]:
            old = copy.deepcopy((self.host.h, self.host.unit, self.host.protected))
            if kind == "session": self.host.h["capacity"][m.CURRENT[0]] = 1
            elif kind == "redis": self.host.h["sessionCache"]["recordCount"] = 1
            elif kind == "auth": self.host.h["capacity"]["maxLoginInFlight"] = 20
            elif kind == "sigkill": self.host.unit["SendSIGKILL"] = "yes"
            else: self.host.env_hash = "0" * 64
            self.assertFalse(self.call("prepare")["passed"], kind)
            self.assertEqual(self.host.actions, [])
            self.host.h, self.host.unit, self.host.protected = old
            self.host.env_hash = "7" * 64
        state = self.prepare(); self.host.actions.clear()
        self.assertFalse(self.call("rollback", state)["passed"])
        self.assertEqual(self.host.actions, [])

    def test_status_binds_full_fence_and_freshness(self):
        result = self.call("status")
        self.assertTrue(result["passed"], result)
        proof = result["protectedProof"]
        self.assertEqual(proof["stableFingerprintSha256"], m.json_hash(m.fence_payload(proof)))
        self.assertEqual(m.validate_protected_proof(proof, self.host.pid), roles())
        proof["sampledAtUnixMs"] -= 300001
        with self.assertRaises(m.Refusal): m.validate_protected_proof(proof, self.host.pid)

    def test_native_stop_refuses_aux_or_legacy_drift_before_signal(self):
        config = {"protectedRoles": roles(), "protectedProof": self.host.proof()}
        native = m.SceneNativeHost(config)
        for which in ["extras", "legacy", "inside"]:
            extras, legacy = rows(), rows(True)
            if which == "extras": extras[0]["executableSha256"] = "0" * 64
            elif which == "legacy": legacy[0]["executableSha256"] = "0" * 64
            else: extras[0]["outsideTargetCgroup"] = False
            with patch.object(m, "read_stop_scope", return_value=scope()), patch.object(m, "protected_inventory", return_value=extras), \
                    patch.object(m, "legacy_inventory", return_value=legacy), patch.object(m.NativeHost, "stop") as stop:
                with self.assertRaises(m.Refusal): native.stop(self.host.current_identity)
                stop.assert_not_called()

    def test_native_guards_wrap_snapshot_stop_start_inactive_and_reload(self):
        native = m.SceneNativeHost({"protectedRoles": roles(), "protectedProof": self.host.proof()})
        proof = {"identity": self.host.current_identity, "activeState": "inactive", "result": "success", "execMainCode": 1, "execMainStatus": 0}
        fence = m.fence_payload(self.host.snapshot())
        with patch.object(native, "_guard", return_value=fence) as guard, patch.object(m.NativeHost, "snapshot", return_value=self.host.snapshot()):
            native.snapshot(); self.assertEqual(guard.call_count, 2)
        with patch.object(native, "_guard", return_value=fence) as guard, patch.object(m.NativeHost, "stop", return_value=proof) as stop:
            self.assertEqual(native.stop(self.host.current_identity), proof)
            self.assertEqual(guard.call_args_list, [unittest.mock.call(), unittest.mock.call(proof)])
            stop.assert_called_once()
        with patch.object(native, "_guard", return_value=fence) as guard, patch.object(m.NativeHost, "start", return_value=self.host.snapshot()):
            native.start(); self.assertEqual(guard.call_args_list, [unittest.mock.call(proof), unittest.mock.call()])
        with patch.object(native, "_guard", return_value=fence) as guard, patch.object(m.NativeHost, "guard_inactive", return_value="configured"):
            self.assertEqual(native.guard_inactive({"stopProof": proof}), "configured")
            self.assertEqual(guard.call_args_list, [unittest.mock.call(proof), unittest.mock.call(proof)])
        with patch.object(native, "_guard", return_value=fence) as guard, patch.object(m.NativeHost, "reload") as reload:
            native.reload(); self.assertEqual(guard.call_args_list, [unittest.mock.call(proof), unittest.mock.call(proof)])
            reload.assert_called_once()
        native.normal_stop_proof = None
        with patch.object(m.NativeHost, "start") as start, self.assertRaises(m.Refusal): native.start()
        start.assert_not_called()


def normal_stop():
    identity = {"pid": 803500, "invocationId": "2645494fe370413593ea1b04c29707bc", "startTicks": 1541658437}
    return {"identity": identity, "activeState": "inactive", "result": "success", "execMainCode": 1, "execMainStatus": 0,
            "inactiveObservation": {"InvocationID": identity["invocationId"], "ExecMainPID": str(identity["pid"]),
                "ExecMainStartTimestampMonotonic": "10000", "ExecMainExitTimestampMonotonic": "20000",
                "Result": "success", "ExecMainCode": "1", "ExecMainStatus": "0"}}


def show_scope(active=True, group=None):
    proof = normal_stop()
    fields = {"MainPID": "803500" if active else "0", "ActiveState": "active" if active else "inactive",
              "ControlGroup": (m.TARGET_CGROUP if active else "") if group is None else group,
              **proof["inactiveObservation"], **{k: " ".join(reversed(v)) for k, v in m.REVIEWED_RELATIONS.items()}}
    return fields


def scope_reader(fields, typed=b"a(sasbttttuii) 0\n"):
    def read(argv):
        if argv == m.SCOPE_SHOW:
            return SimpleNamespace(returncode=0, stdout="\n".join(k + "=" + v for k, v in fields.items()).encode() + b"\n")
        if argv in [m.BUS_PREFIX + [k] for k in m.HOOKS]:
            return SimpleNamespace(returncode=0, stdout=typed)
        raise AssertionError("unexpected command")
    return read


class StopScopeBoundaries(unittest.TestCase):
    def test_active_group_and_relation_order_normalize_to_reviewed_set(self):
        with patch.object(m, "scope_read", side_effect=scope_reader(show_scope())), \
                patch.object(m, "bounded_proc", return_value=b"0::" + m.TARGET_CGROUP.encode() + b"\n"):
            actual = m.read_stop_scope()
        self.assertEqual(actual["relations"], m.REVIEWED_RELATIONS)
        self.assertEqual(actual, scope())

    def test_inactive_empty_group_requires_exact_normal_proof_and_fixed_boundary(self):
        with patch.object(m, "scope_read", side_effect=scope_reader(show_scope(False))):
            value = m.read_stop_scope(normal_stop())
            self.assertEqual(value["boundarySha256"], m.digest(m.TARGET_CGROUP.encode()))
            self.assertNotEqual(value["boundarySha256"], m.digest(b"/"))
            with self.assertRaises(m.Refusal): m.read_stop_scope()
            bad = normal_stop();bad["inactiveObservation"].pop("ExecMainStatus")
            with self.assertRaises(m.Refusal): m.read_stop_scope(bad)
            bad = normal_stop();bad["identity"]["pid"] += 1
            with self.assertRaises(m.Refusal): m.read_stop_scope(bad)
        for invalid in ["/", m.TARGET_CGROUP + "-other"]:
            with patch.object(m, "scope_read", side_effect=scope_reader(show_scope(False, invalid))), self.assertRaises(m.Refusal):
                m.read_stop_scope(normal_stop())
        with patch.object(m, "scope_read", side_effect=scope_reader(show_scope(True))), self.assertRaises(m.Refusal):
            m.read_stop_scope(normal_stop())

    def test_cgroup_component_checks_refuse_inside_before_any_executable_read(self):
        self.assertFalse(m.outside_target_cgroup(m.TARGET_CGROUP.encode()))
        self.assertFalse(m.outside_target_cgroup(m.TARGET_CGROUP.encode() + b"/child"))
        self.assertTrue(m.outside_target_cgroup(m.TARGET_CGROUP.encode() + b"-other"))
        for bad in [b"0::/\n", b"1:cpu:/old\n", b"0::/a/../b\n", b"0::/a//b\n"]:
            with self.assertRaises(m.Refusal): m.cgroup_path(bad)
        row = m.REVIEWED_PROTECTED_PINS[0]
        stat_data = b"1 (private) " + b" ".join([b"S"] + [b"0"] * 18 + [str(row["startTicks"]).encode()])
        def proc(path, maximum=16384):
            if path.name == "stat": return stat_data
            if path.name == "comm": return b"private\n"
            return b"0::" + m.TARGET_CGROUP.encode() + b"/child\n"
        with patch.object(m, "bounded_proc", side_effect=proc), patch.object(m, "file_hash") as hashed, self.assertRaises(m.Refusal):
            m.process_fingerprint({"role": row["role"], "pid": row["pid"]})
        hashed.assert_not_called()

    def test_missing_and_new_relation_or_typed_unknown_refuses(self):
        for key in m.REVIEWED_RELATIONS:
            fields = show_scope();fields.pop(key)
            with patch.object(m, "scope_read", side_effect=scope_reader(fields)), \
                    patch.object(m, "bounded_proc", return_value=b"0::" + m.TARGET_CGROUP.encode()), self.assertRaises(m.Refusal):
                m.read_stop_scope()
        for raw in [b"s empty\n", b"a(sasbttttuii) 0 extra\n", b"a(sasbttttuii) 1\n"]:
            with patch.object(m, "scope_read", side_effect=scope_reader(show_scope(), raw)), \
                    patch.object(m, "bounded_proc", return_value=b"0::" + m.TARGET_CGROUP.encode()), self.assertRaises(m.Refusal):
                m.read_stop_scope()

    def test_readonly_command_surface_and_original_control_allowlist(self):
        denied = [["systemctl", "--no-block", "stop", "caddy.service"], ["systemctl", "restart", "vpn.service"],
                  ["ip", "route", "flush", "all"], ["nft", "flush", "ruleset"], ["kill", "803500"],
                  ["busctl", "--system", "call", "org.freedesktop.systemd1", "RestartUnit"], m.BUS_PREFIX + ["ExecStart"]]
        with patch.object(m.subprocess, "run") as run:
            for argv in denied:
                with self.subTest(argv=argv), self.assertRaises(m.Refusal): m.scope_read(argv)
                with self.assertRaises(m.Refusal): m.command(argv)
            run.assert_not_called()
        with patch.object(m.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout=b"a(sasbttttuii) 0\n")) as run:
            m.scope_read(m.BUS_PREFIX + ["ExecStop"])
            self.assertEqual(run.call_args.args[0], m.BUS_PREFIX + ["ExecStop"])
            self.assertEqual(run.call_args.kwargs["timeout"], 1)
        self.assertEqual(m.UNIT, "mir2-playtest.service")

    def test_frozen_core_nativehost_transaction_ast_and_sourceguards(self):
        def definitions(path):
            return {x.name: x for x in ast.parse(path.read_text(encoding="utf8")).body if isinstance(x, (ast.FunctionDef, ast.ClassDef))}
        before, after = definitions(HERE / "cloud_test_release_scene_v6.py"), definitions(HERE / "cloud_test_release_scene_v7.py")
        for name in ["NativeHost", "command", "empty", "basic", "managed_layout", "recover_original", "_transaction_core", "_validate_v5", "source_guard", "validate_artifact_proof"]:
            self.assertEqual(ast.dump(before[name]), ast.dump(after[name]), name)
        self.assertEqual(m.V5_BASE_SHA256, hashlib.sha256((HERE / "cloud_test_release_v5.py").read_bytes()).hexdigest())
        self.assertEqual(m.V7_VERIFIER_SHA256, hashlib.sha256((HERE.parent / "comparable-reference/cloud/verify_scene_candidate_artifact.py").read_bytes()).hexdigest())
        with patch.object(m, "__file__", "C:/offline/frozen/cloud_test_release_scene_v7.py"), self.assertRaises(m.Refusal): m.source_guard()

    def test_role_contract_does_not_turn_node_into_vpn_or_accept_pid_reuse(self):
        base = roles()
        self.assertEqual({x["role"] for x in base}, {"aux-0", "caddy", "validator-0", "validator-1", "validator-2", "validator-3"})
        for key in range(len(base)):
            invalid = copy.deepcopy(base);invalid.pop(key)
            with self.assertRaises(m.Refusal): m.role_pids(invalid)
        invalid = copy.deepcopy(base);invalid[0]["pid"] += 1
        with self.assertRaises(m.Refusal): m.role_pids(invalid)
        self.assertEqual(len(m.role_pids(base + [{"role": "vpn-0", "pid": 99991}])), 7)
        with self.assertRaises(m.Refusal): m.role_pids(base + [{"role": "vpn-0", "pid": base[0]["pid"]}])

    def test_cli_requires_explicit_action_before_any_connection(self):
        with patch.object(m, "connect_pinned") as connected, self.assertRaises(SystemExit):
            m.main([])
        connected.assert_not_called()


@unittest.skipUnless(os.name == "nt", "Real Windows ACL boundary")
class WindowsAclBoundary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(tempfile.mkdtemp(prefix="mir2-scene-v7-acl-"))
        if cls.root.drive.lower() != "c:":
            raise RuntimeError("Retained ACL fixtures must be on C")
        # Task-owned new fixtures only; no existing ACL, PSModulePath or policy is modified.
        script = "$ErrorActionPreference='Stop';$root=$env:MIR2_V7_ACL_FIXTURES;" + \
            "$sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User;" + \
            "$system=[System.Security.Principal.SecurityIdentifier]::new('S-1-5-18');" + \
            "foreach($name in @('private','broad')){$p=[System.IO.Path]::Combine($root,$name);" + \
            "[void][System.IO.Directory]::CreateDirectory($p);$a=[System.Security.AccessControl.DirectorySecurity]::new();" + \
            "$a.SetOwner($sid);$a.SetAccessRuleProtection($true,$false);" + \
            "foreach($id in @($sid,$system)){$r=[System.Security.AccessControl.FileSystemAccessRule]::new(" + \
            "$id,[System.Security.AccessControl.FileSystemRights]::FullControl," + \
            "[System.Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'," + \
            "[System.Security.AccessControl.PropagationFlags]::None,[System.Security.AccessControl.AccessControlType]::Allow);[void]$a.AddAccessRule($r)};" + \
            "if($name -eq 'broad'){$everyone=[System.Security.Principal.SecurityIdentifier]::new('S-1-1-0');" + \
            "$r=[System.Security.AccessControl.FileSystemAccessRule]::new($everyone,[System.Security.AccessControl.FileSystemRights]::ReadAndExecute," + \
            "[System.Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'," + \
            "[System.Security.AccessControl.PropagationFlags]::None,[System.Security.AccessControl.AccessControlType]::Allow);[void]$a.AddAccessRule($r)};" + \
            "[System.IO.Directory]::SetAccessControl($p,$a);$actual=[System.IO.Directory]::GetAccessControl($p);" + \
            "[Console]::WriteLine('PROTECTED|'+$name+'|'+$actual.AreAccessRulesProtected);" + \
            "foreach($r in $actual.GetAccessRules($true,$true,[System.Security.Principal.SecurityIdentifier])){" + \
            "[Console]::WriteLine('RULE|'+$name+'|'+$r.IdentityReference.Value+'|'+$r.AccessControlType+'|'+$r.FileSystemRights)}};" + \
            "[Console]::WriteLine('OWNER|'+$sid.Value)"
        env = dict(os.environ, MIR2_V7_ACL_FIXTURES=str(cls.root))
        run = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", base64.b64encode(script.encode("utf-16le")).decode()],
            env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=20,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        cls.setup_result = run
        (cls.root / "setup.stdout.log").write_bytes(run.stdout)
        (cls.root / "setup.stderr.log").write_bytes(run.stderr)
        if run.returncode != 0:
            raise RuntimeError("C-only ACL fixture setup failed; retained logs: " + str(cls.root))
        text = run.stdout.decode("utf8", "strict")
        cls.owner = next(line.split("|", 1)[1] for line in text.splitlines() if line.startswith("OWNER|"))
        cls.rules = {name: [line.split("|") for line in text.splitlines() if line.startswith("RULE|" + name + "|")] for name in ["private", "broad"]}
        cls.protected = {name: "PROTECTED|" + name + "|True" in text for name in ["private", "broad"]}

    def test_actual_private_acl_passes_and_broad_acl_is_rejected(self):
        self.assertTrue(all(self.protected.values()))
        self.assertEqual({row[2] for row in self.rules["private"] if row[3] == "Allow"}, {self.owner, "S-1-5-18"})
        self.assertIn("S-1-1-0", {row[2] for row in self.rules["broad"] if row[3] == "Allow"})
        with patch.object(m, "connect_pinned", side_effect=AssertionError("No SSH from local ACL test")):
            m.guard_private_output(self.root / "private")
            with self.assertRaisesRegex(m.Refusal, "private_output_acl_required"):
                m.guard_private_output(self.root / "broad")
        (self.root / "guard-results.json").write_text(json.dumps({"privatePassed": True, "broadRefused": True,
            "ownerSid": self.owner, "aclProtected": self.protected, "allowSids": {k: [r[2] for r in v if r[3] == "Allow"] for k, v in self.rules.items()},
            "fixturesRetained": True, "remoteExecutionPerformed": False}, indent=2), encoding="utf8")

    def test_acl_read_error_is_failclosed(self):
        with self.assertRaisesRegex(m.Refusal, "private_output_acl_required"):
            m.guard_private_output(self.root / "never-created")


class MissingScopeRed(unittest.TestCase):
    def test_unscoped_old_prepare_must_refuse_before_any_operation(self):
        with patch.object(v6.legacy, "m", v6.m):
            host = v6.FakeHost()
            host.config["expectedPid"] = host.current_identity["pid"]
            host.config["protectedProof"] = host.proof()
            result = m.transaction("prepare", host, host.config)
        self.assertFalse(result["passed"], "Unscoped V6 proof must not qualify the stronger V7 contract")
        self.assertEqual(host.actions, [])


if __name__ == "__main__":
    unittest.main()
