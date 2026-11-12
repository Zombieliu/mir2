"""Offline only. No SSH, cloud service, credentials, or real deployment writes."""
import copy
from collections import deque
import importlib.util
import json
from pathlib import Path
import shlex
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("cloud_test_release", Path(__file__).with_name("cloud_test_release.py"))
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)


def identity(pid):
    return {"pid": pid, "invocationId": format(pid, "032x"), "startTicks": pid * 1000}


def health(revision="a" * 40, caps=None):
    return {"ok": True, "http": "ready", "ws": "ready", "revision": revision,
            "capacity": {**dict.fromkeys(m.CURRENT, 0), **m.AUTH, **(caps or m.OLD_CAPS)},
            "sessionCache": {"backend": "redis", "configured": True, "healthy": True,
                             "recordCount": 0, "routeLeaseCount": 0, "staleRecordCount": 0}}


class FakeHost:
    def __init__(self):
        self.config = {"expectedPid": 101, "oldPins": {"revision": "a" * 40, "binarySha256": "1" * 64},
                       "candidate": {"revision": "b" * 40, "binarySha256": "2" * 64, "binarySizeBytes": 12345,
                                     "manifestSha256": "3" * 64,
                                     "releasePath": m.RELEASE_ROOT + "/" + "b" * 40 + m.RELEASE_SUFFIX},
                       "mapsEvidence": {"path": "C:/private/maps.json", "sha256": "4" * 64,
                                        "matchingMapsExplicitlyConfirmedByOperator": True},
                       "artifact": {"directory": "C:/private/artifact"}, "toolSha256": "5" * 64,
                       "oldBinaryRollbackAllowed": True}
        self.original_env = [{"path": "/etc/test-" + str(n) + ".env", "ignoreErrors": False} for n in range(13)]
        self.original_dropins = ["/etc/systemd/system/mir2-playtest.service.d/old.conf"]
        self.protected = {"properties": {"WorkingDirectory": "/opt/mir2-playtest/data"},
                          "files": [{"path": "/etc/test.env", "sha256": "6" * 64}],
                          "originalEnvironmentFiles": self.original_env, "originalDropIns": self.original_dropins}
        self.unit = {"ActiveState": "active", "SendSIGKILL": "no", "KillSignal": "2", "KillMode": "control-group",
                     "MemoryMax": "1677721600", "WorkingDirectory": "/opt/mir2-playtest/data"}
        self.active = True
        self.pid = 101
        self.old_identity, self.zone_identity = identity(201), identity(301)
        self.current_identity = identity(self.pid)
        self.old_health = health()
        self.h = health()
        self.env_caps = {m.CAP_ENV[key]: str(value) for key, value in m.OLD_CAPS.items()}
        self.env_revision = "a" * 40
        self.env_hash = "7" * 64
        self.argv = ["/opt/old/mir2-gateway", "--http", "127.0.0.1:7210"]
        self.argv_hash = m.json_hash(self.argv[1:])
        self.binary_sha, self.executable = "1" * 64, self.argv[0]
        self.loaded_release, self.loaded_caps = False, False
        self.files, self.created_files = {}, {}
        self.staged = None
        self.actions = []
        self.last_started_identity = None
        self.snapshot_count = 0
        self.snapshot_hook = None
        self.after_start = None
        self.partial_write = False
        self.stop_timeout = False

    def snapshot(self):
        self.snapshot_count += 1
        if self.snapshot_hook:
            self.snapshot_hook(self, self.snapshot_count)
        return copy.deepcopy({"identity": self.current_identity, "unit": self.unit, "health": self.h,
                              "oldGateway": {"identity": self.old_identity, "health": self.old_health},
                              "zoneIdentity": self.zone_identity, "protectedConfig": self.protected,
                              "protectedEnvironmentSha256": self.env_hash, "argvTailSha256": self.argv_hash,
                              "binarySha256": self.binary_sha, "executablePath": self.executable,
                              "configuredExecutablePath": self.executable,
                              "processWorkingDirectory": "/opt/mir2-playtest/data",
                              "execStartSha256": "new-exec" if self.loaded_release else "old-exec",
                              "environmentRevision": self.env_revision, "environmentCaps": self.env_caps,
                              "originalEnvironmentFiles": self.original_env, "originalDropIns": self.original_dropins,
                              "environmentFiles": self.original_env +
                              ([{"path": m.RELEASE_ENV, "ignoreErrors": False}] if self.loaded_release else []) +
                              ([{"path": m.CAPS_ENV, "ignoreErrors": False}] if self.loaded_caps else []),
                              "dropIns": self.original_dropins + ([m.RELEASE_DROPIN] if self.loaded_release else []) +
                              ([m.CAPS_DROPIN] if self.loaded_caps else [])})

    def require_no_owned_files(self):
        m.require(not self.files, "owned_path_already_present")

    def require_candidate_absent(self):
        m.require(self.staged is None, "candidate_release_path_already_present")

    def plan(self, state):
        native = m.NativeHost(self.config)
        native.argv = self.argv
        return native.plan(state)

    def check_owned(self, state):
        m.require(self.files == state["ownedFiles"], "owned_file_cas_changed")
        for path, value in self.files.items():
            m.require(path in m.OWN_PATHS and value["sha256"] == state["plan"][path]["sha256"], "owned_content_not_planned")
        if state.get("stagedBinary"):
            m.require(state["stagedBinary"] == self.staged, "staged_binary_cas_changed")

    def stage_candidate(self, state):
        if self.staged is None:
            self.staged = {"path": self.config["candidate"]["releasePath"] + "/mir2-gateway",
                           "sha256": self.config["candidate"]["binarySha256"], "uid": 0, "mode": 0o755}
            self.actions.append(("stage", self.staged["path"]))
        return copy.deepcopy(self.staged)

    def create_files(self, state, kind):
        paths = [m.RELEASE_ENV, m.RELEASE_DROPIN] if kind == "release" else [m.CAPS_ENV, m.CAPS_DROPIN]
        for index, path in enumerate(paths):
            if self.partial_write and index == 1:
                raise m.Refusal("injected_pair_second_file_failure")
            m.require(path not in self.files, "exclusive_create_failed")
            value = {"path": path, "sha256": state["plan"][path]["sha256"], "uid": 0, "mode": 0o600,
                     "inode": 1000 + len(self.actions)}
            self.files[path] = value
            self.created_files[path] = copy.deepcopy(value)
            state["ownedFiles"][path] = copy.deepcopy(value)
            self.actions.append(("create", path))

    def remove_files(self, state, paths):
        self.check_owned(state)
        for path in paths:
            del self.files[path]
            self.actions.append(("unlink", path))

    def reload(self):
        self.actions.append(("daemon-reload",))
        self.loaded_release = m.RELEASE_DROPIN in self.files
        self.loaded_caps = m.CAPS_DROPIN in self.files

    def unit_status(self):
        return {"active": self.active}

    def stop(self, expected):
        m.require(self.current_identity == expected, "activation_changed_before_stop_command")
        self.actions.append(("stop", m.UNIT))
        if self.stop_timeout:
            raise m.Refusal("normal_stop_timeout_no_sigkill")
        self.active = False
        self.unit["ActiveState"] = "inactive"
        return {"identity": expected, "result": "success", "execMainCode": 1, "execMainStatus": 0}

    def guard_inactive(self, state, phase=None, transition=False):
        m.require(not self.active, "target_not_inactive")
        self.check_owned(state)
        snap = self.snapshot()
        m.require(snap["protectedConfig"] == state["baseline"]["protectedConfig"], "inactive_protected_config_changed")
        m.require(self.old_identity == state["baseline"]["oldGateway"]["identity"], "inactive_old_gateway_changed_or_unhealthy")
        m.managed_layout(snap, state["baseline"], phase or state["phase"], transition)
        return "new-exec" if self.loaded_release else "old-exec"

    def start(self):
        m.require(not self.active, "start_requires_inactive")
        self.actions.append(("start", m.UNIT))
        self.pid += 1
        self.current_identity = identity(self.pid)
        self.last_started_identity = copy.deepcopy(self.current_identity)
        self.active = True
        self.unit["ActiveState"] = "active"
        self.executable = self.config["candidate"]["releasePath"] + "/mir2-gateway" if self.loaded_release else self.argv[0]
        self.binary_sha = self.config["candidate"]["binarySha256"] if self.loaded_release else "1" * 64
        self.env_revision = self.config["candidate"]["revision"] if self.loaded_release else "a" * 40
        self.h = health(self.env_revision, m.TEST_CAPS if self.loaded_caps else m.OLD_CAPS)
        self.env_caps = {m.CAP_ENV[key]: str(value) for key, value in (m.TEST_CAPS if self.loaded_caps else m.OLD_CAPS).items()}
        if self.after_start:
            self.after_start(self)
        return self.snapshot()


class Transactions(unittest.TestCase):
    def setUp(self):
        self.host = FakeHost()
        self.config = self.host.config

    def call(self, action, state=None):
        if state:
            self.config["expectedPid"] = state["knownActivation"]["pid"]
        return m.transaction(action, self.host, self.config, state)

    def prepared(self):
        result = self.call("prepare")
        self.assertTrue(result["passed"], result)
        self.assertEqual(self.host.actions, [])
        return result["state"]

    def installed(self):
        result = self.call("install", self.prepared())
        self.assertTrue(result["passed"], result)
        return result["state"]

    def test_install_caps_restore_keeps_new_release_and_original_configuration(self):
        state = self.installed()
        protected = copy.deepcopy(state["baseline"]["protectedConfig"])
        result = self.call("apply-caps", state)
        self.assertTrue(result["passed"], result)
        self.assertEqual({key: self.host.h["capacity"][key] for key in m.CAP_ENV}, m.TEST_CAPS)
        result = self.call("restore-caps", result["state"])
        self.assertTrue(result["passed"], result)
        self.assertEqual(result["state"]["phase"], "installed")
        self.assertEqual(set(self.host.files), {m.RELEASE_DROPIN, m.RELEASE_ENV})
        self.assertEqual(self.host.binary_sha, self.config["candidate"]["binarySha256"])
        self.assertEqual({key: self.host.h["capacity"][key] for key in m.CAP_ENV}, m.OLD_CAPS)
        self.assertEqual(self.host.protected, protected)
        self.assertEqual(sum(action[0] == "stop" for action in self.host.actions), 3)
        self.assertTrue(all(action[1] == m.UNIT for action in self.host.actions if action[0] in ["stop", "start"]))

    def test_explicit_rollback_restores_old_binary_preserves_staged_candidate(self):
        result = self.call("rollback", self.installed())
        self.assertTrue(result["passed"], result)
        self.assertEqual(self.host.files, {})
        self.assertEqual(self.host.binary_sha, "1" * 64)
        self.assertIsNotNone(self.host.staged)

    def test_each_current_counter_blocks_without_mutation(self):
        for key in m.CURRENT:
            with self.subTest(key=key):
                host = FakeHost()
                host.h["capacity"][key] = 1
                result = m.transaction("prepare", host, host.config)
                self.assertFalse(result["passed"])
                self.assertEqual(host.actions, [])

    def test_each_redis_counter_blocks_without_mutation(self):
        for key in ["recordCount", "routeLeaseCount", "staleRecordCount"]:
            with self.subTest(key=key):
                host = FakeHost()
                host.h["sessionCache"][key] = 1
                result = m.transaction("prepare", host, host.config)
                self.assertFalse(result["passed"])
                self.assertEqual(host.actions, [])

    def test_pid_revision_binary_auth_memory_and_signal_pins(self):
        changes = [lambda h: h.config.update(expectedPid=999), lambda h: h.h.update(revision="c" * 40),
                   lambda h: setattr(h, "binary_sha", "9" * 64),
                   lambda h: h.h["capacity"].update(maxLoginInFlight=3),
                   lambda h: h.unit.update(MemoryMax="999"), lambda h: h.unit.update(SendSIGKILL="yes"),
                   lambda h: h.unit.update(KillSignal="15")]
        for change in changes:
            host = FakeHost()
            change(host)
            result = m.transaction("prepare", host, host.config)
            self.assertFalse(result["passed"], result)
            self.assertEqual(host.actions, [])

    def test_non_integer_or_missing_quiescence_counter_fails(self):
        for value in [None, False, "0"]:
            self.host.h["capacity"][m.CURRENT[0]] = value
            self.assertFalse(self.call("prepare")["passed"])
        self.assertEqual(self.host.actions, [])

    def test_thirteen_original_env_files_required(self):
        self.host.original_env.pop()
        self.assertEqual(self.call("prepare")["error"], "expected_thirteen_original_environment_files")

    def test_foreign_file_refused(self):
        self.host.files[m.RELEASE_ENV] = {"foreign": True}
        result = self.call("prepare")
        self.assertFalse(result["passed"])
        self.assertEqual(self.host.actions, [])

    def test_protected_drift_and_peers_refuse_install_without_service_action(self):
        changes = [lambda h: h.protected["files"][0].update(sha256="9" * 64),
                   lambda h: setattr(h, "env_hash", "changed"), lambda h: setattr(h, "argv_hash", "changed"),
                   lambda h: setattr(h, "old_identity", identity(999)), lambda h: setattr(h, "zone_identity", identity(999)),
                   lambda h: h.original_env.reverse()]
        for change in changes:
            with self.subTest(change=repr(change)):
                self.setUp()
                state = self.prepared()
                change(self.host)
                result = self.call("install", state)
                self.assertFalse(result["passed"], result)
                self.assertEqual(self.host.actions, [])

    def test_new_arrival_at_immediate_check_never_stops_live_target(self):
        state = self.prepared()
        count = self.host.snapshot_count
        def arrive(host, number):
            if number == count + 2:
                host.h["capacity"]["currentActiveSessions"] = 1
        self.host.snapshot_hook = arrive
        result = self.call("install", state)
        self.assertFalse(result["passed"])
        self.assertEqual([action[0] for action in self.host.actions], ["stage"])

    def test_pair_partial_write_rolls_back_exact_created_env(self):
        state = self.prepared()
        self.host.partial_write = True
        result = self.call("install", state)
        self.assertFalse(result["passed"])
        self.assertTrue(result["rollback"]["passed"], result)
        self.assertEqual(self.host.files, {})
        self.assertEqual(self.host.binary_sha, "1" * 64)

    def test_owned_file_replacement_blocks_restore_before_stop(self):
        state = self.installed()
        result = self.call("apply-caps", state)
        self.assertTrue(result["passed"], result)
        state = result["state"]
        self.host.actions.clear()
        self.host.files[m.CAPS_ENV] = {**self.host.files[m.CAPS_ENV], "inode": 98765}
        result = self.call("restore-caps", state)
        self.assertFalse(result["passed"])
        self.assertEqual(self.host.actions, [])

    def test_failed_caps_effective_old_values_can_guardedly_rollback(self):
        state = self.installed()
        def ignored_caps(host):
            if host.loaded_caps:
                host.h["capacity"].update(m.OLD_CAPS)
                host.env_caps = {m.CAP_ENV[key]: str(value) for key, value in m.OLD_CAPS.items()}
        self.host.after_start = ignored_caps
        result = self.call("apply-caps", state)
        self.assertFalse(result["passed"])
        self.assertTrue(result["rollback"]["passed"], result)
        self.assertEqual(self.host.files, {})
        self.assertEqual(self.host.binary_sha, "1" * 64)

    def test_new_session_after_failed_verification_defers_rollback(self):
        state = self.installed()
        def arrive(host):
            if host.loaded_caps:
                host.h["capacity"]["currentActiveSessions"] = 1
        self.host.after_start = arrive
        previous = sum(action[0] == "stop" for action in self.host.actions)
        result = self.call("apply-caps", state)
        self.assertFalse(result["passed"])
        self.assertTrue(result["rollback"]["deferred"], result)
        self.assertTrue(result["state"]["recoveryRequired"])
        self.assertEqual(sum(action[0] == "stop" for action in self.host.actions), previous + 1)
        self.assertEqual(set(self.host.files), set(m.OWN_PATHS))
        # After genuine drain, the explicit, identity-pinned rollback is usable.
        self.host.after_start = None
        self.host.h["capacity"]["currentActiveSessions"] = 0
        result = self.call("rollback", result["state"])
        self.assertTrue(result["passed"], result)
        self.assertEqual(self.host.files, {})

    def test_foreign_activation_after_failed_start_is_never_stopped(self):
        state = self.installed()
        def replacement(host):
            if host.loaded_caps:
                host.current_identity = identity(999)
        self.host.after_start = replacement
        result = self.call("apply-caps", state)
        # Host start returned a different identity than the identity actually
        # captured at start; real NativeHost also rechecks /proc identity.
        self.assertFalse(result["passed"])
        self.assertTrue(result["rollback"]["deferred"])
        self.assertEqual(result["rollback"]["error"], "target_activation_changed")

    def test_stop_timeout_never_deletes_or_forces_anything(self):
        state = self.prepared()
        self.host.stop_timeout = True
        result = self.call("install", state)
        self.assertFalse(result["passed"])
        self.assertTrue(result["rollback"]["deferred"])
        self.assertEqual(self.host.files, {})
        self.assertFalse(any(action[0] in ["unlink", "kill", "restart"] for action in self.host.actions))

    def test_source_or_plan_change_refuses(self):
        state = self.prepared()
        self.config["toolSha256"] = "changed"
        self.assertFalse(self.call("install", state)["passed"])
        self.assertEqual(self.host.actions, [])

    def test_denied_downgrade_still_installs_caps_and_restores_caps_on_fixed_binary(self):
        self.config["oldBinaryRollbackAllowed"] = False
        state = self.installed()
        self.assertIs(state["oldBinaryRollbackAllowed"], False)
        result = self.call("apply-caps", state)
        self.assertTrue(result["passed"], result)
        self.assertIs(result["state"]["oldBinaryRollbackAllowed"], False)
        result = self.call("restore-caps", result["state"])
        self.assertTrue(result["passed"], result)
        self.assertIs(result["state"]["oldBinaryRollbackAllowed"], False)
        self.assertEqual(self.host.binary_sha, self.config["candidate"]["binarySha256"])
        self.assertEqual(set(self.host.files), {m.RELEASE_DROPIN, m.RELEASE_ENV})
        self.assertEqual({key: self.host.h["capacity"][key] for key in m.CAP_ENV}, m.OLD_CAPS)

    def test_denied_explicit_rollback_refuses_before_any_service_or_file_mutation(self):
        self.config["oldBinaryRollbackAllowed"] = False
        state = self.installed()
        files, activation = copy.deepcopy(self.host.files), copy.deepcopy(self.host.current_identity)
        self.host.actions.clear()
        result = self.call("rollback", state)
        self.assertFalse(result["passed"])
        self.assertEqual(result["error"], "old_binary_rollback_denied_or_unproven")
        self.assertEqual(self.host.actions, [])
        self.assertEqual(self.host.files, files)
        self.assertEqual(self.host.current_identity, activation)

    def test_denied_fault_recovery_preserves_candidate_activation_and_all_owned_files(self):
        self.config["oldBinaryRollbackAllowed"] = False
        state = self.installed()
        def caps_not_effective(host):
            if host.loaded_caps:
                host.h["capacity"].update(m.OLD_CAPS)
                host.env_caps = {m.CAP_ENV[key]: str(value) for key, value in m.OLD_CAPS.items()}
        self.host.after_start = caps_not_effective
        self.host.actions.clear()
        result = self.call("apply-caps", state)
        self.assertFalse(result["passed"])
        self.assertTrue(result["rollback"]["deferred"])
        self.assertEqual(result["rollback"]["error"], "old_binary_rollback_denied_or_unproven")
        self.assertTrue(result["state"]["recoveryRequired"])
        self.assertEqual(self.host.current_identity, self.host.last_started_identity)
        self.assertEqual(self.host.binary_sha, self.config["candidate"]["binarySha256"])
        self.assertEqual(set(self.host.files), set(m.OWN_PATHS))
        self.assertEqual(sum(action[0] == "stop" for action in self.host.actions), 1)
        self.assertFalse(any(action[0] == "unlink" for action in self.host.actions))
        self.assertTrue(all(self.host.h["capacity"][key] == 0 for key in m.CURRENT))

    def test_recorded_denied_policy_cannot_be_lifted_by_later_request(self):
        self.config["oldBinaryRollbackAllowed"] = False
        state = self.prepared()
        self.config["oldBinaryRollbackAllowed"] = True
        result = self.call("install", state)
        self.assertFalse(result["passed"])
        self.assertEqual(result["error"], "rollback_policy_missing_or_changed")
        self.assertIs(result["state"]["oldBinaryRollbackAllowed"], False)
        self.assertEqual(self.host.actions, [])

    def test_old_recovery_entry_requires_explicit_true_before_any_host_access(self):
        for state in [{}, {"oldBinaryRollbackAllowed": False}, {"oldBinaryRollbackAllowed": None},
                      {"oldBinaryRollbackAllowed": 1}]:
            with self.subTest(state=state), patch.object(self.host, "check_owned") as access:
                with self.assertRaisesRegex(m.Refusal, "old_binary_rollback_denied_or_unproven"):
                    m.recover_original(self.host, state, [])
                access.assert_not_called()

    def test_default_mode_records_explicit_true(self):
        self.assertIs(self.prepared()["oldBinaryRollbackAllowed"], True)


class ParsingAndLocalGuards(unittest.TestCase):
    def test_downgrade_policy_cli_flag_is_prepare_only(self):
        with patch.object(m, "output_directory") as output:
            with self.assertRaisesRegex(m.Refusal, "rollback_policy_flag_is_prepare_only"):
                m.main(["--action", "restore-caps", "--output", "unused", "--expected-pid", "101", "--deny-old-binary-rollback"])
            output.assert_not_called()

    def test_native_stop_captures_proof_of_the_exited_main_process(self):
        host = m.NativeHost({})
        active = {"MainPID": "101", "KillSignal": "2", "SendSIGKILL": "no"}
        inactive = {"MainPID": "0", "ActiveState": "inactive", "Result": "success", "ExecMainCode": "1",
                    "ExecMainStatus": "0", "InvocationID": "abcd", "ExecMainPID": "101",
                    "ExecMainStartTimestampMonotonic": "123456", "ExecMainExitTimestampMonotonic": "223456"}
        with patch.object(m, "show", side_effect=[active, inactive]), patch.object(m, "process_identity", return_value=identity(101)), \
                patch.object(m, "command") as control:
            proof = host.stop(identity(101))
        self.assertEqual(proof["inactiveObservation"]["ExecMainPID"], "101")
        control.assert_called_once_with(["systemctl", "--no-block", "stop", m.UNIT])
        inactive["ExecMainPID"] = "999"
        with patch.object(m, "show", side_effect=[active, inactive]), patch.object(m, "process_identity", return_value=identity(101)), patch.object(m, "command"):
            with self.assertRaisesRegex(m.Refusal, "stopped_main_identity_not_proven"):
                host.stop(identity(101))

    def test_native_inactive_guard_refuses_intervening_activation_even_if_now_stopped(self):
        host = m.NativeHost({})
        state = {"stopProof": {"inactiveObservation": {"ExecMainPID": "101", "ExecMainStartTimestampMonotonic": "123456"}}}
        changed = {"MainPID": "0", "ActiveState": "inactive", "ExecMainPID": "999", "ExecMainStartTimestampMonotonic": "999999"}
        with patch.object(m, "show", return_value=changed):
            with self.assertRaisesRegex(m.Refusal, "inactive_activation_no_longer_matches_normal_stop"):
                host.guard_inactive(state)

    def test_native_start_binds_first_observed_activation_and_refuses_replacement(self):
        host = m.NativeHost({})
        initial = {"MainPID": "0", "ActiveState": "inactive"}
        first, foreign = {"MainPID": "102", "ActiveState": "active"}, {"MainPID": "999", "ActiveState": "active"}
        with patch.object(m, "show", side_effect=[initial, first, foreign]), \
                patch.object(m, "process_identity", side_effect=lambda unit: identity(int(unit["MainPID"]))), \
                patch.object(m, "health", return_value={"ok": False}), patch.object(m, "command") as control, patch.object(m.time, "sleep"):
            with self.assertRaisesRegex(m.Refusal, "replacement_activation_during_start"):
                host.start()
        self.assertEqual(host.last_started_identity, identity(102))
        control.assert_called_once_with(["systemctl", "--no-block", "start", m.UNIT])

    def test_all_repeated_environment_rows_are_kept_in_order(self):
        rows = ["EnvironmentFiles=/etc/test-" + str(n) + ".env (ignore_errors=no)" for n in range(13)]
        rows += ["EnvironmentFiles=" + m.CAPS_ENV + " (ignore_errors=no)"]
        parsed = m.parse_show("MainPID=123\n" + "\n".join(rows))
        self.assertEqual(len(parsed["EnvironmentFilesRawRows"]), 14)
        env = m.parse_environment_files(parsed["EnvironmentFiles"])
        self.assertEqual(len(env), 14)
        self.assertEqual(env[0]["path"], "/etc/test-0.env")
        self.assertEqual(env[-1]["path"], m.CAPS_ENV)

    def test_other_duplicate_properties_rejected(self):
        with self.assertRaises(m.Refusal):
            m.parse_show("MainPID=1\nMainPID=2")

    def test_exec_static_hash_ignores_restart_timestamps(self):
        prefix = "{ path=/opt/test/mir2-gateway ; argv[]=/opt/test/mir2-gateway --http 127.0.0.1:7210 ; ignore_errors=no"
        one = m.parse_exec_start(prefix + " ; start_time=[Monday] ; pid=1 ; }")
        two = m.parse_exec_start(prefix + " ; start_time=[Tuesday] ; pid=2 ; }")
        self.assertEqual(one, two)
        self.assertEqual(one[1][1:], ["--http", "127.0.0.1:7210"])

    def test_ambiguous_argv_and_variable_expansion_rejected(self):
        for text in ['"two words"', "$TOKEN", "%i", "`command`", "foo;bar"]:
            with self.subTest(text=text), self.assertRaises(m.Refusal):
                m.parse_exec_start("{ path=/opt/test/gw ; argv[]=/opt/test/gw " + text + " ; ignore_errors=no ; start_time=x ; }")

    def test_systemctl_control_allowlist_is_test_only(self):
        for args in [["systemctl", "--no-block", "stop", m.OLD_GATEWAY], ["systemctl", "stop", m.ZONE],
                     ["systemctl", "restart", m.UNIT], ["kill", "-9", "123"], ["systemctl", "stop", "openvpn.service"]]:
            with self.subTest(args=args), patch.object(m.subprocess, "run") as call:
                with self.assertRaises(m.Refusal):
                    m.command(args)
                call.assert_not_called()

    def test_artifact_and_maps_pins(self):
        with tempfile.TemporaryDirectory(prefix="mir2-test-release-offline-") as temporary:
            root = Path(temporary)
            binary = bytearray(100)
            binary[:6], binary[18:20] = b"\x7fELF\x02\x01", b"\x3e\x00"
            (root / "mir2-gateway").write_bytes(binary)
            evidence = root / "maps.json"
            evidence.write_text('{"passed":true}', encoding="utf-8")
            manifest = {"name": "mir2-gateway", "gitRevision": "b" * 40, "gitDirty": False,
                        "binarySha256": m.digest(binary), "binarySizeBytes": len(binary)}
            (root / "RELEASE.json").write_text(json.dumps(manifest), encoding="utf-8")
            args = [str(root), "b" * 40, str(evidence), m.file_hash(evidence), True]
            candidate, maps, _ = m.inspect_artifact(*args)
            self.assertEqual(candidate["binarySha256"], m.digest(binary))
            self.assertTrue(maps["contentsReviewedExternally"])
            for mutation in [lambda: manifest.update(gitDirty=True), lambda: manifest.update(gitRevision="c" * 40),
                             lambda: manifest.update(binarySha256="0" * 64)]:
                original = copy.deepcopy(manifest)
                mutation()
                (root / "RELEASE.json").write_text(json.dumps(manifest), encoding="utf-8")
                with self.assertRaises(m.Refusal):
                    m.inspect_artifact(*args)
                manifest = original
            (root / "RELEASE.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(m.Refusal):
                m.inspect_artifact(str(root), "b" * 40, str(evidence), "0" * 64, True)
            with self.assertRaises(m.Refusal):
                m.inspect_artifact(str(root), "b" * 40, str(evidence), m.file_hash(evidence), False)

    def test_receipts_cannot_be_written_in_repository(self):
        with self.assertRaises(m.Refusal):
            m.output_directory(Path(__file__).parent)


class FakeSSHChannel:
    """Sudo consumes a line only in password mode; Python consumes artifact bytes.

    The NOPASSWD/cached modes deliberately do not consume an unsolicited password.
    Calling blocking ChannelFile readline/read/write is unsupported by this fake.
    """
    def __init__(self, mode, marker, password, binary, action):
        self.mode, self.marker, self.password = mode, marker, password
        self.binary, self.action = binary, action
        self.stdout, self.stderr = deque(), deque()
        self.closed = False
        self.eof_received = False
        self.exit_ready = False
        self.password_writes, self.artifact_writes = [], []
        self.waiting_password = mode in ["password", "second-prompt"]
        self.finished = False
        self.shutdown_requested = False
        self.timeout = None
        if self.waiting_password:
            self.stderr.extend([marker[:7], marker[7:]])
        elif mode == "wrong-prompt":
            self.stderr.append(b"[sudo] another prompt: ")
        elif mode == "secret-stderr":
            self.stderr.append(("sudo diagnostic " + password).encode())
        elif mode == "partial-prompt-close":
            self.stderr.append(marker[:7])
            self.exit_ready = True
        elif mode == "close-before-root":
            self.closed = True
            self.exit_ready = True
        elif mode == "timeout":
            pass
        else:
            self.announce_root()
            if mode == "late-prompt":
                self.stderr.append(marker)
            elif mode == "wrong-prompt-with-ready":
                self.stderr.append(b"sudo: unexpected diagnostic")

    def announce_root(self):
        ack = json.dumps({"rpcStarted": True}).encode() + b"\n"
        if self.action == "install":
            ready = json.dumps({"readyForArtifact": True, "bytes": len(self.binary)}).encode() + b"\n"
            self.stdout.append(ack + ready)
        else:
            self.stdout.append(ack + self.receipt())
            self.finished = True
            self.exit_ready = True

    def receipt(self):
        return json.dumps({"schema": m.SCHEMA, "action": self.action, "passed": True}).encode() + b"\n"

    def recv_stderr_ready(self):
        return bool(self.stderr)

    def recv_stderr(self, size):
        value = self.stderr.popleft()
        if len(value) > size:
            self.stderr.appendleft(value[size:])
        return value[:size]

    def recv_ready(self):
        return bool(self.stdout)

    def recv(self, size):
        value = self.stdout.popleft()
        if len(value) > size:
            self.stdout.appendleft(value[size:])
        if self.mode == "close-at-ready":
            self.closed = True
            self.exit_ready = True
        elif self.mode == "eof-at-ready":
            self.eof_received = True
        return value[:size]

    def sendall(self, data):
        if self.waiting_password:
            self.password_writes.append(bytes(data))
            if self.mode == "second-prompt":
                self.stderr.append(self.marker)
                return
            if data != (self.password + "\n").encode():
                raise AssertionError("synthetic sudo password line was not exact")
            self.waiting_password = False
            self.announce_root()
        else:
            self.artifact_writes.append(bytes(data))
            total = sum(map(len, self.artifact_writes))
            if total >= len(self.binary):
                self.stdout.append(self.receipt())
                self.finished = True
                self.exit_ready = True

    def shutdown_write(self):
        self.shutdown_requested = True

    def exit_status_ready(self):
        return self.exit_ready

    def recv_exit_status(self):
        return 0

    def settimeout(self, value):
        self.timeout = value

    def close(self):
        self.closed = True


class FakeSSHClient:
    def __init__(self, mode, password, binary, action="install"):
        self.mode, self.password, self.binary, self.action = mode, password, binary, action
        self.channel = None
        self.command = None

    def exec_command(self, command, timeout):
        self.command = command
        argv = shlex.split(command)
        if argv[:3] != ["sudo", "-S", "-p"]:
            raise AssertionError("transport did not use explicit sudo prompt")
        self.channel = FakeSSHChannel(self.mode, argv[3].encode(), self.password, self.binary, self.action)
        stream = SimpleNamespace(channel=self.channel)
        return stream, stream, stream


class SSHAuthenticationHandshake(unittest.TestCase):
    PASSWORD = "synthetic-not-a-real-credential"
    BINARY = b"\x7fELF\x02\x01" + b"\x00\x11\x22\x33" * 35

    def call(self, mode, action="install"):
        client = FakeSSHClient(mode, self.PASSWORD, self.BINARY, action)
        with tempfile.TemporaryDirectory(prefix="mir2-ssh-transport-offline-") as temporary:
            binary = Path(temporary) / "mir2-gateway"
            binary.write_bytes(self.BINARY)
            self.client = client
            return m.ssh_call(client, self.PASSWORD, action, {"candidate": {"binarySizeBytes": len(self.BINARY)}}, None, str(binary))

    def test_password_required_prompt_sends_one_password_then_exact_binary(self):
        result = self.call("password")
        channel = self.client.channel
        self.assertTrue(result["passed"])
        self.assertEqual(channel.password_writes, [(self.PASSWORD + "\n").encode()])
        self.assertEqual(b"".join(channel.artifact_writes), self.BINARY)
        self.assertTrue(channel.shutdown_requested)
        self.assertNotIn(self.PASSWORD, self.client.command)

    def test_nopasswd_sends_no_password_only_exact_binary(self):
        self.assertTrue(self.call("nopasswd")["passed"])
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertEqual(b"".join(self.client.channel.artifact_writes), self.BINARY)
        self.assertNotIn(self.PASSWORD.encode(), b"".join(self.client.channel.artifact_writes))

    def test_cached_sudo_sends_no_password_only_exact_binary(self):
        self.assertTrue(self.call("cached")["passed"])
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertEqual(b"".join(self.client.channel.artifact_writes), self.BINARY)

    def test_nopasswd_readonly_rpc_never_writes_stdin(self):
        self.assertTrue(self.call("nopasswd", "prepare")["passed"])
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertEqual(self.client.channel.artifact_writes, [])

    def test_nopasswd_multi_chunk_upload_remains_bounded_exact_binary(self):
        self.BINARY = b"\x7fELF\x02\x01" + b"\x01\x23\x45\x67" * (512 * 1024 + 7)
        self.assertTrue(self.call("nopasswd")["passed"])
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertGreater(len(self.client.channel.artifact_writes), 1)
        self.assertTrue(all(len(block) <= 1024 * 1024 for block in self.client.channel.artifact_writes))
        self.assertEqual(b"".join(self.client.channel.artifact_writes), self.BINARY)

    def test_wrong_prompt_fails_without_password_or_artifact(self):
        for mode in ["wrong-prompt", "wrong-prompt-with-ready", "late-prompt"]:
            with self.subTest(mode=mode):
                with self.assertRaises(m.Refusal):
                    self.call(mode)
                self.assertEqual(self.client.channel.password_writes, [])
                self.assertEqual(self.client.channel.artifact_writes, [])
                self.assertTrue(self.client.channel.closed)

    def test_second_prompt_never_retries_password_or_starts_artifact(self):
        with self.assertRaisesRegex(m.Refusal, "repeated_or_late_sudo_prompt_no_retry"):
            self.call("second-prompt")
        self.assertEqual(len(self.client.channel.password_writes), 1)
        self.assertEqual(self.client.channel.artifact_writes, [])

    def test_disconnect_before_or_at_ready_never_sends_artifact(self):
        for mode in ["partial-prompt-close", "close-before-root", "close-at-ready", "eof-at-ready"]:
            with self.subTest(mode=mode):
                with self.assertRaises(m.Refusal):
                    self.call(mode)
                self.assertEqual(self.client.channel.password_writes, [])
                self.assertEqual(self.client.channel.artifact_writes, [])

    def test_unexpected_raw_stderr_is_never_in_failure_message(self):
        with self.assertRaises(m.Refusal) as failure:
            self.call("secret-stderr")
        self.assertNotIn(self.PASSWORD, str(failure.exception))
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertEqual(self.client.channel.artifact_writes, [])

    def test_timeout_never_guesses_a_password_or_sends_artifact(self):
        with patch.object(m.time, "monotonic", side_effect=[0, 481]):
            with self.assertRaisesRegex(m.Refusal, "ssh_rpc_timeout_no_retry"):
                self.call("timeout")
        self.assertEqual(self.client.channel.password_writes, [])
        self.assertEqual(self.client.channel.artifact_writes, [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
