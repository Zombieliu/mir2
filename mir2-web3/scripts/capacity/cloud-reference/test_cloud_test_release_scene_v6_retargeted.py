"""Offline fake host/SSH only. No credentials, network, service, or artifact run."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shlex
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch


HERE = Path(__file__).parent


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


m = module("scene_v6_under_test", HERE / "cloud_test_release_scene_v8.py")
require_legacy_test_sha = "6ba5dfda75a2b407a340ae77b450c3f97d581fd0826e01951c4b902f51d12a3e"
assert hashlib.sha256((HERE / "test_cloud_test_release_v5.py").read_bytes()).hexdigest() == require_legacy_test_sha
legacy = module("scene_v6_legacy_fake_fixtures", HERE / "test_cloud_test_release_v5.py")
legacy.m = m


def artifact_proof():
    compatibility=json.loads((HERE/'fixtures/scene-7403/source-compatibility.json').read_text())
    return {"schema":m.ARTIFACT_SCHEMA,"passed":True,"candidateRevision":m.CANDIDATE_REVISION,
      "integrationCommit":m.INTEGRATION_COMMIT,"baselineRevision":m.OLD_REVISION,"baselineBinarySha256":m.OLD_BINARY_SHA256,
      "runId":m.CI_RUN_ID,"nativeJobId":m.CI_NATIVE_JOB_ID,"wholeWorkflowStatus":"completed","wholeWorkflowConclusion":"success",
      "nativeRequiredGates":m.REQUIRED_GATES[:],"nativeStartupActualExitCode":0,"verifierSourceSha256":m.V8_VERIFIER_SHA256,
      "deploymentApproved":False,"capacityQualified":False,"binarySha256":m.CANDIDATE_BINARY_SHA256,
      "binarySizeBytes":m.CANDIDATE_BINARY_BYTES,"artifactDirectory":"/synthetic/offline/artifact",
      "releaseManifestSha256":m.CANDIDATE_MANIFEST_SHA256,"candidatePinsSha256":"4"*64,"actionsMetadataSha256":"5"*64,
      "sourceInputManifestSha256":m.SOURCE_INPUT_MANIFEST_SHA256,
      "artifacts":{k:{"id":v,"name":"synthetic-"+k,"path":"/synthetic/"+k,
        "sha256":m.GATEWAY_ZIP_SHA256 if k=='gateway' else m.NATIVE_EVIDENCE_ZIP_SHA256} for k,v in m.ARTIFACT_IDS.items()},
      "maps":{"fileCount":1620,"totalFileBytes":42111309,"allGitArtifactLiveMatch":True,"liveProofSha256":"8"*64},
      "sourceCompatibility":compatibility}


def roles():
    return m.role_pids([{"role":r["role"],"pid":r["pid"]} for r in m.REVIEWED_PROTECTED_PINS if r["role"] not in ['old-gateway','old-zone']])


def fingerprints():
    return [{**copy.deepcopy(r),"outsideTargetCgroup":True} for r in m.REVIEWED_PROTECTED_PINS if r["role"] not in ['old-gateway','old-zone']]


def scope():
    return {"scopeReviewSha256":m.REVIEWED_SCOPE_SHA256,"boundarySha256":m.digest(m.TARGET_CGROUP.encode()),
       "relations":copy.deepcopy(m.REVIEWED_RELATIONS),"typedHooks":{k:m.typed_array(b'a(sasbttttuii) 0\n') for k in m.HOOKS},"vpnIdentity":"unknown"}


class FakeHost(legacy.FakeHost):
    def __init__(self):
        super().__init__()
        self.original_env.append({"path": m.PREVIOUS_V5_RELEASE_ENV, "ignoreErrors": False})
        self.original_dropins.append(m.PREVIOUS_V5_RELEASE_DROPIN)
        self.protected["files"] += [
            {"path": path, "sha256": str(i) * 64, "inode": 1600 + i, "mode": 0o600, "uid": 0}
            for i, path in enumerate([m.PREVIOUS_V5_RELEASE_ENV, m.PREVIOUS_V5_RELEASE_DROPIN], 1)]
        self.protected["files"].sort(key=lambda item: item["path"])
        self.h["revision"] = self.env_revision = m.OLD_REVISION
        self.binary_sha = m.OLD_BINARY_SHA256
        self.executable = m.RELEASE_ROOT + "/" + m.OLD_REVISION + "-equip-logout-20261112-v5/mir2-gateway"
        self.argv[0] = self.executable
        self.additional = fingerprints()
        self.legacy_full=[{**copy.deepcopy(r),"outsideTargetCgroup":True} for r in m.REVIEWED_PROTECTED_PINS if r["role"] in ['old-gateway','old-zone']]
        self.scope=scope()
        for key,role in [('old_identity','old-gateway'),('zone_identity','old-zone')]:
            p=next(r for r in m.REVIEWED_PROTECTED_PINS if r['role']==role)
            setattr(self,key,{'pid':p['pid'],'invocationId':('a' if role=='old-gateway' else 'b')*32,'startTicks':p['startTicks']})
        self.config.update(baseSourceSha256=m.V5_BASE_SHA256, oldBinaryRollbackAllowed=False,
                           protectedRoles=roles(), strictArtifactProof=artifact_proof(),
                           oldPins={"revision": m.OLD_REVISION, "binarySha256": m.OLD_BINARY_SHA256})
        self.config["candidate"].update(revision=m.CANDIDATE_REVISION, binarySha256=m.CANDIDATE_BINARY_SHA256,
            binarySizeBytes=m.CANDIDATE_BINARY_BYTES,manifestSha256=m.CANDIDATE_MANIFEST_SHA256,
            releasePath=m.RELEASE_ROOT + "/" + m.CANDIDATE_REVISION + m.RELEASE_SUFFIX)
        snap = self.snapshot()
        old_protected = copy.deepcopy(self.protected)
        old_protected["files"] = [x for x in old_protected["files"] if x["path"] not in [m.PREVIOUS_V5_RELEASE_ENV, m.PREVIOUS_V5_RELEASE_DROPIN]]
        old_protected["originalEnvironmentFiles"] = self.original_env[:-1]
        old_protected["originalDropIns"] = self.original_dropins[:-1]
        own = {x["path"]: x for x in self.protected["files"] if x["path"] in [m.PREVIOUS_V5_RELEASE_ENV, m.PREVIOUS_V5_RELEASE_DROPIN]}
        self.config["predecessor"] = {"stateFile": {"path": "/synthetic/v5-state.json", "sha256": "f" * 64},
            "phase": "installed", "knownActivation": self.current_identity.copy(),
            "candidate": {"revision": m.OLD_REVISION, "binarySha256": m.OLD_BINARY_SHA256,
                          "releasePath": self.executable.rsplit("/", 1)[0]}, "ownedFiles": own,
            "protectedEnvironmentSha256": self.env_hash,
            "oldGatewayIdentity": self.old_identity.copy(), "zoneIdentity": self.zone_identity.copy(),
            "protectedConfig": old_protected, "originalEnvironmentFiles": self.original_env[:-1],
            "originalDropIns": self.original_dropins[:-1], "argvTailSha256": self.argv_hash,
            "processWorkingDirectory": snap["processWorkingDirectory"], "candidateExecStartSha256": "a" * 64}
        self.config["sceneV7Pins"] = {"predecessorState": {"sha256": "f" * 64}, "artifactProof": {"sha256": "e" * 64},
                                    "protectedFingerprintSha256": m.json_hash(m.fence_payload(self.snapshot()))}

    def snapshot(self):
        result = super().snapshot()
        result.update(protectedProcesses=copy.deepcopy(self.additional),legacyProtectedProcesses=copy.deepcopy(self.legacy_full),stopScope=copy.deepcopy(self.scope))
        result["execStartSha256"] = ("b" if self.loaded_release else "a") * 64
        return result

    def guard_inactive(self, state, phase=None, transition=False):
        super().guard_inactive(state, phase, transition)
        return ("b" if self.loaded_release else "a") * 64

    def proof(self):
        value={"schema":m.PROTECTED_SCHEMA,"readonly":True,"sampledAtUnixMs":round(time.time()*1000),"roles":roles(),
          "targetIdentity":self.current_identity.copy(),"targetRevision":self.h['revision'],"targetBinarySha256":self.binary_sha,
          **m.fence_payload(self.snapshot()),"scopeReviewSha256":m.REVIEWED_SCOPE_SHA256,"vpnIdentity":"unknown"}
        value['stableFingerprintSha256']=m.json_hash(m.fence_payload(value))
        return value


class SuccessorTransactions(unittest.TestCase):
    def setUp(self):
        self.legacy_patch = patch.object(legacy, "m", m)
        self.legacy_patch.start(); self.addCleanup(self.legacy_patch.stop)
        self.host = FakeHost()

    def call(self, action, state=None):
        self.host.config["expectedPid"] = self.host.current_identity["pid"]
        self.host.config["protectedProof"] = self.host.proof()
        return m.transaction(action, self.host, self.host.config, state)

    def prepared(self):
        result = self.call("prepare")
        self.assertTrue(result["passed"], result)
        self.assertEqual(self.host.actions, [])
        return result["state"]

    def installed(self):
        result = self.call("install", self.prepared())
        self.assertTrue(result["passed"], result)
        return result["state"]

    def test_same_unit_install_caps_restore_preserves_five_pairs_and_new_binary(self):
        original = copy.deepcopy(self.host.protected)
        prior = copy.deepcopy(self.host.config["predecessor"])
        state = self.prepared()
        self.assertEqual(len(self.host.snapshot()["environmentFiles"]), 18)
        for action, count in [("install", 19), ("apply-caps", 20), ("restore-caps", 19)]:
            result = self.call(action, state); self.assertTrue(result["passed"], result); state = result["state"]
            self.assertEqual(len(self.host.snapshot()["environmentFiles"]), count)
            self.assertEqual(self.host.protected, original)
            self.assertEqual(self.host.config["predecessor"], prior)
            self.assertFalse(state["oldBinaryRollbackAllowed"])
        self.assertEqual(self.host.binary_sha, artifact_proof()["binarySha256"])
        self.assertEqual(set(self.host.files), {m.RELEASE_ENV, m.RELEASE_DROPIN})
        self.assertTrue(all(x[1] == m.UNIT for x in self.host.actions if x[0] in ["stop", "start"]))

    def test_status_is_readonly_and_produces_a_target_bound_proof(self):
        self.host.h["capacity"][m.CURRENT[0]] = 3
        result = self.call("status")
        self.assertTrue(result["passed"], result); self.assertEqual(self.host.actions, [])
        self.assertNotIn("state", result)
        m.validate_protected_proof(result["protectedProof"], 101)

    def test_missing_artifact_or_extra_process_proof_refuses_without_signal(self):
        for key in ["strictArtifactProof", "protectedProof", "protectedRoles"]:
            host = FakeHost(); host.config["protectedProof"] = host.proof(); host.config.pop(key)
            result = m.transaction("prepare", host, host.config)
            self.assertFalse(result["passed"]); self.assertEqual(host.actions, [])

    def test_each_nine_quiescence_counter_blocks_before_signal(self):
        for key in m.CURRENT + ["recordCount", "routeLeaseCount", "staleRecordCount"]:
            host = FakeHost(); host.config["protectedProof"] = host.proof()
            (host.h["capacity"] if key in m.CURRENT else host.h["sessionCache"])[key] = 1
            result = m.transaction("prepare", host, host.config)
            self.assertFalse(result["passed"], key); self.assertEqual(host.actions, [])

    def test_every_prior_pair_metadata_change_blocks_install_without_signal(self):
        for path in [m.OLDER_RELEASE_ENV, m.PREVIOUS_RELEASE_ENV, m.PREVIOUS_V3_RELEASE_ENV,
                     m.PREVIOUS_V4_RELEASE_ENV, m.PREVIOUS_V5_RELEASE_ENV, m.PREVIOUS_V5_RELEASE_DROPIN]:
            for field in ["sha256", "inode"]:
                with self.subTest(path=path, field=field):
                    self.setUp(); state = self.prepared()
                    row = next(x for x in self.host.protected["files"] if x["path"] == path)
                    row[field] = "changed" if field == "sha256" else row[field] + 1
                    result = self.call("install", state)
                    self.assertFalse(result["passed"]); self.assertEqual(self.host.actions, [])

    def test_wrong_count_order_flags_and_owned_collision_refuse(self):
        for mode in ["count", "order", "ignoreErrors", "collision"]:
            host = FakeHost(); host.config["protectedProof"] = host.proof()
            if mode == "count": host.original_env.pop()
            if mode == "order": host.original_env[-1], host.original_env[-2] = host.original_env[-2], host.original_env[-1]
            if mode == "ignoreErrors": host.original_env[-1]["ignoreErrors"] = True
            if mode == "collision": host.files[m.RELEASE_ENV] = {"foreign": True}
            self.assertFalse(m.transaction("prepare", host, host.config)["passed"], mode)
            self.assertEqual(host.actions, [])

    def test_prepare_cannot_adopt_changed_predecessor_world_identity_or_environment(self):
        for mode in ["oldGateway", "zone", "environment"]:
            host = FakeHost(); host.config["protectedProof"] = host.proof()
            if mode == "oldGateway": host.old_identity["startTicks"] += 1
            if mode == "zone": host.zone_identity["startTicks"] += 1
            if mode == "environment": host.env_hash = "0" * 64
            result = m.transaction("prepare", host, host.config)
            self.assertFalse(result["passed"], mode); self.assertEqual(host.actions, [])

    def test_every_additional_protected_fingerprint_drift_refuses_caps(self):
        for index in range(6):
            self.setUp(); state = self.installed(); self.host.actions.clear()
            self.host.additional[index]["startTicks"] += 1
            result = self.call("apply-caps", state)
            self.assertFalse(result["passed"]); self.assertEqual(self.host.actions, [])

    def test_stop_timeout_and_failed_candidate_never_downgrade_or_delete(self):
        state = self.prepared(); self.host.stop_timeout = True
        result = self.call("install", state)
        self.assertFalse(result["passed"]); self.assertTrue(result["rollback"]["deferred"])
        self.assertFalse(any(x[0] in ["unlink", "start"] for x in self.host.actions))
        self.setUp(); state = self.prepared()
        self.host.after_start = lambda h: h.h.update(revision="c" * 40)
        result = self.call("install", state)
        self.assertFalse(result["passed"]); self.assertTrue(result["rollback"]["deferred"])
        self.assertEqual(set(self.host.files), {m.RELEASE_ENV, m.RELEASE_DROPIN})
        self.assertFalse(any(x[0] == "unlink" for x in self.host.actions))

    def test_explicit_rollback_and_policy_lift_refuse_without_signal(self):
        state = self.installed(); self.host.actions.clear()
        self.assertFalse(self.call("rollback", state)["passed"])
        self.host.config["oldBinaryRollbackAllowed"] = True
        self.assertFalse(self.call("apply-caps", state)["passed"])
        self.assertEqual(self.host.actions, [])

    def test_immediate_new_session_refuses_after_staging_before_stop(self):
        state = self.prepared()
        def hook(host, count):
            if host.staged is not None: host.h["capacity"][m.CURRENT[0]] = 1
        self.host.snapshot_hook = hook
        result = self.call("install", state)
        self.assertFalse(result["passed"])
        self.assertFalse(any(x[0] == "stop" for x in self.host.actions))


class SourceAndProofGuards(unittest.TestCase):
    def test_v5_core_guards_and_native_operations_are_ast_identical(self):
        old = ast.parse((HERE / "cloud_test_release_v5.py").read_text(encoding="utf-8"))
        new = ast.parse(Path(m.__file__).read_text(encoding="utf-8"))
        before = {x.name: x for x in old.body if isinstance(x, (ast.FunctionDef, ast.ClassDef))}
        after = {x.name: x for x in new.body if isinstance(x, (ast.FunctionDef, ast.ClassDef))}
        for name in ["NativeHost", "command", "empty", "basic", "managed_layout", "recover_original", "metadata",
                     "parse_show", "parse_environment_files", "parse_exec_start", "process_identity", "inspect_artifact", "output_directory"]:
            self.assertEqual(ast.dump(before[name]), ast.dump(after[name]), name)
        original_validate = copy.deepcopy(before["validate"]); original_validate.name = "_validate_v5"
        self.assertEqual(ast.dump(original_validate), ast.dump(after["_validate_v5"]))
        a, z = copy.deepcopy(before["transaction"]), copy.deepcopy(after["_transaction_core"]); a.name = z.name
        codes = {"expected_seventeen_original_environment_files", "expected_eighteen_original_environment_files",
                 "v1_v2_v3_v4_persistent_releases_not_protected_baseline", "v1_v2_v3_v4_v5_persistent_releases_not_protected_baseline"}
        class RemoveOnlyPrepareLayoutGuards(ast.NodeTransformer):
            def visit_Expr(self, node):
                if isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name) and node.value.func.id == "require" \
                        and any(isinstance(arg, ast.Constant) and arg.value in codes for arg in node.value.args): return None
                return self.generic_visit(node)
        self.assertEqual(ast.dump(RemoveOnlyPrepareLayoutGuards().visit(a)), ast.dump(RemoveOnlyPrepareLayoutGuards().visit(z)))

    def test_wrong_or_source_only_artifact_proof_refused(self):
        m.validate_artifact_proof(artifact_proof())
        for key, value in [("schema", "source-only"), ("passed", False), ("nativeJobId", None),
                           ("wholeWorkflowConclusion", "failure"), ("candidateRevision", "5a356ae0d5b79b846675f623bd5d2dc21c09a10f"),
                           ("verifierSourceSha256", "0" * 64), ("nativeRequiredGates", []), ("nativeStartupActualExitCode", False)]:
            proof = artifact_proof(); proof[key] = value
            with self.subTest(key=key), self.assertRaises(m.Refusal): m.validate_artifact_proof(proof)
        proof = artifact_proof(); proof["sourceCompatibility"]["nativeInputIdentity"]["nativeBuildInputObjects"][0]["gitObject"] = "b" * 40
        with self.assertRaises(m.Refusal): m.validate_artifact_proof(proof)

    def test_freshness_and_all_caddy_validator_vpn_roles_required(self):
        value=FakeHost().proof();value['sampledAtUnixMs']=1000
        m.validate_protected_proof(value, 101, now_ms=301000)
        for stamp in [999, 301001]:
            with self.assertRaises(m.Refusal): m.validate_protected_proof(value, 101, now_ms=stamp)
        for role in range(6):
            bad = copy.deepcopy(value); bad["roles"].pop(role)
            with self.assertRaises(m.Refusal): m.validate_protected_proof(bad, 101, now_ms=1000)

    def test_no_default_action_and_no_unreviewed_frozen_copy_execution(self):
        with patch.object(m, "connect_pinned") as connect, self.assertRaises(SystemExit): m.main([])
        connect.assert_not_called()
        with patch.object(m, "__file__", "C:/offline/source/cloud_test_release_scene_v6.py"), self.assertRaises(m.Refusal): m.source_guard()

    def test_hashed_inputs_and_old_source_guard_are_fixed(self):
        self.assertEqual(hashlib.sha256((HERE / "cloud_test_release_v5.py").read_bytes()).hexdigest(), m.V5_BASE_SHA256)
        with tempfile.TemporaryDirectory(prefix="scene-v6-offline-") as temp:
            path = Path(temp) / "public.json"; path.write_bytes(b'{"roles":[]}')
            with self.assertRaises(m.Refusal): m.read_hashed_json(path, "f" * 64)
            value, _ = m.read_hashed_json(path, hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual(value, {"roles": []})

    def test_native_extra_process_guard_runs_before_signal_and_after_inactive_check(self):
        config = {"protectedRoles": roles(), "protectedProof": FakeHost().proof()}
        host = m.SceneNativeHost(config)
        changed = fingerprints(); changed[0]["startTicks"] += 1
        with patch.object(m, "protected_inventory", return_value=changed), patch.object(m.NativeHost, "stop") as stop, patch.object(m,"read_stop_scope",return_value=scope()), patch.object(m,"legacy_inventory",return_value=config["protectedProof"]["legacyProtectedProcesses"]):
            with self.assertRaises(m.Refusal): host.stop(legacy.identity(101))
            stop.assert_not_called()
        with patch.object(m, "protected_inventory", side_effect=[fingerprints(), changed]), \
                patch.object(m.NativeHost, "guard_inactive", return_value="a" * 64) as inactive, patch.object(m,"read_stop_scope",return_value=scope()), patch.object(m,"legacy_inventory",return_value=config["protectedProof"]["legacyProtectedProcesses"]):
            with self.assertRaises(m.Refusal): host.guard_inactive({"stopProof":{"identity":config["protectedProof"]["targetIdentity"]}})
            inactive.assert_called_once()


class SSHNoSudoPassword(unittest.TestCase):
    BINARY = b"\x7fELF\x02\x01" + b"\x00\x11\x22\x33" * 35

    def setUp(self):
        self.p = patch.object(legacy, "m", m); self.p.start(); self.addCleanup(self.p.stop)

    def call(self, mode, action="install"):
        outer = self
        class Client:
            def exec_command(self, command, timeout):
                self.command = command
                outer.assertEqual(shlex.split(command)[:3], ["sudo", "-n", "python3"])
                self.channel = legacy.FakeSSHChannel(mode, b"unexpected-sudo-marker", "synthetic-private", outer.BINARY, action)
                self.channel.command = command
                return (SimpleNamespace(channel=self.channel),) * 3
        self.client = Client()
        with tempfile.TemporaryDirectory(prefix="scene-v6-ssh-offline-") as temp:
            path = Path(temp) / "mir2-gateway"; path.write_bytes(self.BINARY)
            return m.ssh_call(self.client, None, action, {"candidate": {"binarySizeBytes": len(self.BINARY)}}, None, str(path))

    def test_nopasswd_upload_is_only_binary_and_readonly_stdin_is_closed(self):
        self.assertTrue(self.call("nopasswd")["passed"])
        self.assertEqual(b"".join(self.client.channel.artifact_writes), self.BINARY)
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertTrue(self.call("nopasswd", "status")["passed"])
        self.assertEqual(self.client.channel.artifact_writes, [])
        self.assertTrue(self.client.channel.shutdown_requested)

    def test_nopasswd_multiple_chunks_remain_exact_and_bounded(self):
        self.BINARY = b"\x7fELF\x02\x01" + b"\x11\x22\x33\x44" * (512 * 1024 + 7)
        self.assertTrue(self.call("nopasswd")["passed"])
        writes = self.client.channel.artifact_writes
        self.assertGreater(len(writes), 1)
        self.assertTrue(all(len(block) <= 1024 * 1024 for block in writes))
        self.assertEqual(b"".join(writes), self.BINARY)
        self.assertEqual(self.client.channel.password_writes, [])

    def test_any_password_prompt_stderr_or_disconnect_refuses_without_bytes(self):
        for mode in ["password", "secret-stderr", "wrong-prompt", "late-prompt", "close-before-root", "close-at-ready"]:
            with self.subTest(mode=mode), self.assertRaises(m.Refusal): self.call(mode)
            self.assertEqual(self.client.channel.password_writes, [])
            self.assertEqual(self.client.channel.artifact_writes, [])

    def test_wrong_host_key_never_prompts_or_authenticates(self):
        transport = SimpleNamespace(start_client=lambda timeout: None, close=lambda: None,
            get_remote_server_key=lambda: SimpleNamespace(get_name=lambda: "ssh-ed25519", asbytes=lambda: b"wrong"),
            auth_password=unittest.mock.Mock())
        with patch.object(m.socket, "create_connection", return_value=SimpleNamespace(close=lambda: None)), \
                patch.object(m.getpass, "getpass") as prompt, self.assertRaises(m.Refusal):
            m.connect_pinned(SimpleNamespace(Transport=lambda sock: transport))
        prompt.assert_not_called(); transport.auth_password.assert_not_called()

    def test_correct_host_key_is_checked_before_password_authentication(self):
        order = []
        key_bytes = b"synthetic-ed25519-host-key"
        pin = "SHA256:" + m.base64.b64encode(hashlib.sha256(key_bytes).digest()).decode().rstrip("=")
        transport = SimpleNamespace(start_client=lambda timeout: order.append("handshake"), close=lambda: None,
            get_remote_server_key=lambda: (order.append("host-key") or SimpleNamespace(get_name=lambda: "ssh-ed25519", asbytes=lambda: key_bytes)),
            auth_password=lambda user, password, fallback: order.append(("auth", user, password == "synthetic-memory-only", fallback)),
            is_authenticated=lambda: True)
        with patch.object(m, "HOST_KEY_SHA256", pin), \
                patch.object(m.socket, "create_connection", return_value=SimpleNamespace(close=lambda: None)), \
                patch.object(m.getpass, "getpass", side_effect=lambda _: (order.append("prompt") or "synthetic-memory-only")):
            client = m.connect_pinned(SimpleNamespace(Transport=lambda sock: transport)); client.close()
        self.assertEqual(order, ["handshake", "host-key", "prompt", ("auth", "ubuntu", True, False)])


if __name__ == "__main__":
    unittest.main(verbosity=2)
