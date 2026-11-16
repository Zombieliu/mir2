#!/usr/bin/env python3
"""One candidate, TEST Gateway only: guarded release/capacity transactions.

No remote operation occurs on import or without an explicit action.
Same TEST unit and endpoint; all five prior release pairs remain protected.
Requires whole-green V7 artifact receipt and fresh protected-process proof.
"""
import argparse
import base64
import copy
from datetime import datetime, timezone
import getpass
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import signal
import socket
import threading
import stat
import subprocess
import sys
import tempfile
import time
import urllib.request
import uuid
import zlib

HOST = "165.154.65.136"
UNIT = "mir2-playtest.service"
OLD_GATEWAY = "mir2-gateway.service"
ZONE = "mir2-zone-ucloud.service"
SCHEMA = "mir2.test-gateway-release.v1"
RELEASE_ROOT = "/opt/mir2-playtest/releases"
RELEASE_SUFFIX = "-spectator-scene-20261113-v6"
RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 54 + "-spectator-scene-20261113-v6.conf"
RELEASE_ENV = "/etc/mir2-playtest-spectator-scene-20261113-v6.env"
CAPS_DROPIN = "/run/systemd/system/mir2-playtest.service.d/" + "z" * 56 + "-spectator-scene-capacity-20261113-v6.conf"
CAPS_ENV = "/run/mir2-playtest-spectator-scene-capacity-20261113-v6.env"
PREVIOUS_RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 38 + "-equip-logout-20261112-v2.conf"
PREVIOUS_RELEASE_ENV = "/etc/mir2-playtest-equip-logout-20261112-v2.env"
OLDER_RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 34 + "-equip-logout-20261112.conf"
OLDER_RELEASE_ENV = "/etc/mir2-playtest-equip-logout-20261112.env"
PREVIOUS_V3_RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 42 + "-equip-logout-20261112-v3.conf"
PREVIOUS_V3_RELEASE_ENV = "/etc/mir2-playtest-equip-logout-20261112-v3.env"
PREVIOUS_V4_RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 46 + "-equip-logout-20261112-v4.conf"
PREVIOUS_V4_RELEASE_ENV = "/etc/mir2-playtest-equip-logout-20261112-v4.env"
PREVIOUS_V5_RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 50 + "-equip-logout-20261112-v5.conf"
PREVIOUS_V5_RELEASE_ENV = "/etc/mir2-playtest-equip-logout-20261112-v5.env"
PREVIOUS_V5_CAPS_DROPIN = "/run/systemd/system/mir2-playtest.service.d/" + "z" * 52 + "-equip-logout-capacity-20261112-v5.conf"
PREVIOUS_V5_CAPS_ENV = "/run/mir2-playtest-equip-logout-capacity-20261112-v5.env"
OWN_PATHS = [RELEASE_DROPIN, RELEASE_ENV, CAPS_DROPIN, CAPS_ENV]
CAP_ENV = {"maxActiveSessions": "MIR2_GATEWAY_MAX_ACTIVE_SESSIONS",
           "maxWsConnections": "MIR2_GATEWAY_MAX_WS_CONNECTIONS",
           "maxReconnectLeases": "MIR2_GATEWAY_MAX_RECONNECT_LEASES"}
OLD_CAPS = dict(zip(CAP_ENV, [51, 66, 51]))
TEST_CAPS = dict(zip(CAP_ENV, [120, 150, 120]))
CURRENT = ["currentWsConnections", "currentActiveSessions", "currentReconnectLeases",
           "currentLoginInFlight", "currentNewCharacterInFlight", "currentStartGameInFlight"]
AUTH = dict(zip(["maxLoginInFlight", "maxNewCharacterInFlight", "maxStartGameInFlight"], [2, 1, 2]))
STABLE = ["KillSignal", "KillMode", "SendSIGKILL", "TimeoutStopUSec", "MemoryMax", "MemoryHigh",
          "CPUQuotaPerSecUSec", "TasksMax", "LimitNOFILE", "User", "Group", "Restart", "RestartUSec",
          "WorkingDirectory", "RootDirectory", "RootImage", "EnvironmentFiles"]
PROPS = ["MainPID", "InvocationID", "ActiveState", "SubState", "Result", "ExecMainCode", "ExecMainStatus",
         "ExecMainPID", "ExecMainStartTimestampMonotonic", "ExecMainExitTimestampMonotonic",
         "FragmentPath", "DropInPaths", "ExecStart"] + STABLE
SAFE_ARG = re.compile(r"[A-Za-z0-9_./:=,@+\-]+\Z")


class Refusal(Exception):
    pass


def require(value, code):
    if not value:
        raise Refusal(code)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def json_hash(value):
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def file_hash(path):
    value = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def parse_show(raw):
    result, rows = {}, []
    for row in raw.splitlines():
        require("=" in row, "invalid_systemctl_row")
        key, value = row.split("=", 1)
        if key == "EnvironmentFiles":
            rows.append(row)
        else:
            require(key not in result, "unexpected_duplicate_systemctl_property")
            result[key] = value
    result["EnvironmentFiles"] = " ".join(row.split("=", 1)[1] for row in rows)
    result["EnvironmentFilesRawRows"] = rows
    return result


def parse_environment_files(raw):
    tokens = shlex.split(raw)
    require(len(tokens) % 2 == 0, "invalid_environment_files_metadata")
    result = []
    for path, flag in zip(tokens[::2], tokens[1::2]):
        require(path.startswith("/") and flag in ["(ignore_errors=no)", "(ignore_errors=yes)"],
                "invalid_environment_files_metadata")
        result.append({"path": path, "ignoreErrors": flag == "(ignore_errors=yes)"})
    return result


def parse_exec_start(raw):
    # Deliberately bounded: reject ambiguous/expanded/quoted original argv rather than
    # guessing systemd escaping. The observed TEST service has simple literal argv.
    require(raw.count("argv[]=") == 1, "multiple_or_missing_exec_start")
    match = re.search(r"\{\s*path=(.*?)\s*;\s*argv\[\]=(.*?)\s*;\s*ignore_errors=(yes|no)", raw)
    require(match is not None and match[3] == "no", "unsupported_exec_start_metadata")
    executable, text = match[1], match[2]
    argv = text.split()
    require(argv and executable == argv[0] and executable.startswith("/") and
            all(SAFE_ARG.fullmatch(arg) for arg in argv), "unsupported_original_argv")
    require(len(argv) <= 128 and sum(map(len, argv)) <= 32768, "original_argv_too_large")
    prefix = re.split(r"\s*;\s*start_time=", raw, maxsplit=1)[0]
    return executable, argv, digest(prefix.encode())


def empty(snapshot):
    h = snapshot["health"]
    require(all(type(h["capacity"].get(key)) is int and h["capacity"][key] == 0 for key in CURRENT),
            "target_has_sessions_or_inflight")
    cache = h["sessionCache"]
    require(cache.get("backend") == "redis" and cache.get("configured") is True and cache.get("healthy") is True,
            "redis_not_healthy_and_configured")
    require(all(type(cache.get(key)) is int and cache[key] == 0
                for key in ["recordCount", "routeLeaseCount", "staleRecordCount"]), "redis_records_or_leases_not_empty")


def basic(snapshot):
    u, h = snapshot["unit"], snapshot["health"]
    require(u.get("ActiveState") == "active" and snapshot["identity"]["pid"] > 1, "target_not_active")
    require(u.get("SendSIGKILL") == "no" and u.get("KillSignal") in ["2", "SIGINT"], "normal_sigint_policy_required")
    require(u.get("KillMode") in ["control-group", "mixed", "process"], "unsafe_kill_mode")
    require(u.get("MemoryMax") == "1677721600", "memorymax_changed")
    require(h.get("ok") is True and h.get("http") == "ready" and h.get("ws") == "ready", "target_not_healthy")
    require({key: h["capacity"].get(key) for key in AUTH} == AUTH, "authentication_limits_changed")
    require(h["sessionCache"].get("backend") == "redis" and h["sessionCache"].get("configured") is True
            and h["sessionCache"].get("healthy") is True, "target_redis_unhealthy")
    old = snapshot["oldGateway"]
    require(old["identity"]["pid"] > 1 and old["health"].get("ok") is True and old["health"].get("http") == "ready"
            and old["health"].get("ws") == "ready" and old["health"]["sessionCache"].get("healthy") is True,
            "old_gateway_unhealthy")


def managed_layout(snapshot, baseline, phase, transition=False):
    original_env = baseline["originalEnvironmentFiles"]
    original_dropins = baseline["originalDropIns"]
    env_tail = [{"path": RELEASE_ENV, "ignoreErrors": False}]
    drop_tail = [RELEASE_DROPIN]
    if phase == "caps-applied":
        env_tail += [{"path": CAPS_ENV, "ignoreErrors": False}]
        drop_tail += [CAPS_DROPIN]
    elif phase in ["prepared", "rolled-back"]:
        env_tail, drop_tail = [], []
    if transition:
        allowed_env = [{"path": path, "ignoreErrors": False} for path in [RELEASE_ENV, CAPS_ENV]]
        actual = snapshot["environmentFiles"][len(original_env):]
        require(all(item in allowed_env for item in actual) and len(actual) == len({item["path"] for item in actual}),
                "foreign_managed_environment_entry")
        env_tail = sorted(actual, key=lambda item: [RELEASE_ENV, CAPS_ENV].index(item["path"]))
        actual_drop = snapshot["dropIns"][len(original_dropins):]
        require(all(path in [RELEASE_DROPIN, CAPS_DROPIN] for path in actual_drop)
                and len(actual_drop) == len(set(actual_drop)), "foreign_managed_dropin_entry")
        drop_tail = sorted(actual_drop, key=[RELEASE_DROPIN, CAPS_DROPIN].index)
    require(snapshot["environmentFiles"] == original_env + env_tail, "environment_file_order_or_flags_changed")
    require(snapshot["dropIns"] == original_dropins + drop_tail, "dropin_order_changed")


def _validate_v5(snapshot, state, phase=None, drained=True, identity=None, transition=False):
    basic(snapshot)
    baseline = state["baseline"]
    phase = phase or state["phase"]
    require(snapshot["protectedConfig"] == baseline["protectedConfig"], "protected_config_changed")
    require(snapshot["protectedEnvironmentSha256"] == baseline["protectedEnvironmentSha256"], "non_managed_environment_changed")
    require(snapshot["argvTailSha256"] == baseline["argvTailSha256"], "original_argv_tail_changed")
    require(snapshot["processWorkingDirectory"] == baseline["processWorkingDirectory"], "process_working_directory_changed")
    require(snapshot["oldGateway"]["identity"] == baseline["oldGateway"]["identity"], "old_gateway_activation_changed")
    require(snapshot["zoneIdentity"] == baseline["zoneIdentity"], "zone_activation_changed")
    managed_layout(snapshot, baseline, phase, transition)
    old, candidate = state["oldPins"], state["candidate"]
    actual = (snapshot["health"].get("revision"), snapshot["binarySha256"], snapshot["executablePath"])
    original = (old["revision"], old["binarySha256"], baseline["executablePath"])
    revised = (candidate["revision"], candidate["binarySha256"], candidate["releasePath"] + "/mir2-gateway")
    if transition:
        require(actual in [original, revised], "rollback_unapproved_binary_or_revision")
        caps = {key: snapshot["health"]["capacity"].get(key) for key in CAP_ENV}
        require(all(type(value) is int and value in [OLD_CAPS[key], TEST_CAPS[key]] for key, value in caps.items()),
                "rollback_unapproved_caps")
        expected_exec = baseline["execStartSha256"] if actual == original else state.get("candidateExecStartSha256")
        require(expected_exec is not None and snapshot["execStartSha256"] == expected_exec, "rollback_exec_start_changed")
    else:
        old_mode = phase in ["prepared", "rolled-back"]
        require(actual == (original if old_mode else revised), "binary_or_revision_pin_mismatch")
        caps = TEST_CAPS if phase == "caps-applied" else OLD_CAPS
        expected_exec = baseline["execStartSha256"] if old_mode else state.get("candidateExecStartSha256")
        require(snapshot["execStartSha256"] == expected_exec, "exec_start_changed")
    require(snapshot["environmentRevision"] == actual[0], "environment_revision_disagrees")
    require(snapshot["environmentCaps"] == {CAP_ENV[key]: str(value) for key, value in caps.items()}, "environment_caps_disagree")
    require({key: snapshot["health"]["capacity"].get(key) for key in CAP_ENV} == caps, "health_caps_disagree")
    if identity is not None:
        require(snapshot["identity"] == identity, "target_activation_changed")
    if drained:
        empty(snapshot)


def recover_original(host, state, steps):
    require(state.get("oldBinaryRollbackAllowed") is True, "old_binary_rollback_denied_or_unproven")
    # Never infer an activation from a PID alone; a failed start must have yielded
    # the identity captured by this helper. A foreign activation/config/file defers.
    host.check_owned(state)
    status = host.unit_status()
    if status["active"]:
        first = host.snapshot()
        validate(first, state, identity=state["knownActivation"], transition=True)
        immediate = host.snapshot()
        validate(immediate, state, identity=state["knownActivation"], transition=True)
        host.check_owned(state)
        steps.append({"step": "rollback_immediate_empty_known_activation", "snapshot": immediate})
        state["stopProof"] = host.stop(state["knownActivation"])
    else:
        require(state.get("stopProof") is not None, "rollback_inactive_without_normal_stop_proof")
        host.guard_inactive(state, transition=True)
    host.check_owned(state)
    host.remove_files(state, list(state["ownedFiles"]))
    state["ownedFiles"] = {}
    host.reload()
    host.guard_inactive(state, phase="rolled-back")
    after = host.start()
    require(getattr(host, "last_started_identity", None) == after["identity"], "replacement_activation_during_start")
    state["knownActivation"] = after["identity"]
    validate(after, state, phase="rolled-back", identity=state["knownActivation"])
    state["phase"] = "rolled-back"
    state["stopProof"] = None
    state.pop("recoveryRequired", None)
    steps.append({"step": "original_release_and_caps_restored", "snapshot": after})
    return after


def _transaction_core(action, host, config, state=None):
    out = {"schema": SCHEMA, "action": action, "passed": False, "unit": UNIT, "steps": [],
           "userDataOperations": False, "sigkillRequested": False,
           "quiescenceCheckIsAtomicAdmissionLock": False,
           "raceBoundary": "Clients can arrive after the final empty check and before SIGINT; no admission bypass or force kill is used."}
    state = copy.deepcopy(state)
    mutation = False
    try:
        require(action in ["prepare", "install", "apply-caps", "restore-caps", "rollback"], "unsupported_action")
        if action == "prepare":
            require(state is None, "prepare_state_already_exists")
            rollback_allowed = config.get("oldBinaryRollbackAllowed", True)
            require(type(rollback_allowed) is bool, "explicit_boolean_rollback_policy_required")
            host.require_no_owned_files()
            host.require_candidate_absent()
            before = host.snapshot()
            require(before["identity"]["pid"] == config["expectedPid"], "expected_pid_mismatch")
            require(len(before["originalEnvironmentFiles"]) == 18
                    and len({entry["path"] for entry in before["originalEnvironmentFiles"]}) == 18,
                    "expected_eighteen_original_environment_files")
            require(before["originalEnvironmentFiles"][-5:] == [{"path": OLDER_RELEASE_ENV, "ignoreErrors": False},
                                                               {"path": PREVIOUS_RELEASE_ENV, "ignoreErrors": False},
                                                               {"path": PREVIOUS_V3_RELEASE_ENV, "ignoreErrors": False},
                                                               {"path": PREVIOUS_V4_RELEASE_ENV, "ignoreErrors": False},
                                                                {"path": PREVIOUS_V5_RELEASE_ENV, "ignoreErrors": False}]
                    and all(path in before["originalDropIns"] for path in
                            [OLDER_RELEASE_DROPIN, PREVIOUS_RELEASE_DROPIN, PREVIOUS_V3_RELEASE_DROPIN, PREVIOUS_V4_RELEASE_DROPIN, PREVIOUS_V5_RELEASE_DROPIN])
                    and before["originalDropIns"].index(OLDER_RELEASE_DROPIN) < before["originalDropIns"].index(PREVIOUS_RELEASE_DROPIN)
                    < before["originalDropIns"].index(PREVIOUS_V3_RELEASE_DROPIN)
                    < before["originalDropIns"].index(PREVIOUS_V4_RELEASE_DROPIN)
                    < before["originalDropIns"].index(PREVIOUS_V5_RELEASE_DROPIN),
                    "v1_v2_v3_v4_v5_persistent_releases_not_protected_baseline")
            state = {"schema": SCHEMA, "phase": "prepared", "baseline": before, "oldPins": config["oldPins"],
                     "candidate": config["candidate"], "mapsEvidence": config["mapsEvidence"], "ownedFiles": {},
                     "knownActivation": before["identity"], "stopProof": None, "stagedBinary": None,
                     "toolSha256": config["toolSha256"], "artifact": config["artifact"],
                     "oldBinaryRollbackAllowed": rollback_allowed}
            validate(before, state)
            state["plan"] = host.plan(state)
            second = host.snapshot()
            validate(second, state, identity=state["knownActivation"])
            require(second == before, "prepare_snapshot_changed")
            out.update(passed=True, before=before, after=second)
        else:
            require(state is not None and state.get("schema") == SCHEMA, "prepared_transaction_required")
            require(state["toolSha256"] == config["toolSha256"], "tool_source_changed_since_prepare")
            require(state["candidate"] == config["candidate"] and state["oldPins"] == config["oldPins"]
                    and state["mapsEvidence"] == config["mapsEvidence"], "candidate_or_evidence_changed")
            require(type(state.get("oldBinaryRollbackAllowed")) is bool
                    and config.get("oldBinaryRollbackAllowed") is state["oldBinaryRollbackAllowed"],
                    "rollback_policy_missing_or_changed")
            if action == "rollback":
                require(state["oldBinaryRollbackAllowed"] is True, "old_binary_rollback_denied_or_unproven")
            require(state["knownActivation"]["pid"] == config["expectedPid"], "explicit_expected_pid_does_not_match_state")
            require(host.plan(state) == state["plan"], "owned_content_plan_changed")
            host.check_owned(state)
            require(not state.get("recoveryRequired") or action == "rollback", "guarded_rollback_required_before_other_actions")
            if action == "rollback" and not host.unit_status()["active"]:
                require(state.get("stopProof") is not None, "rollback_inactive_without_normal_stop_proof")
                host.guard_inactive(state, transition=True)
                before = {"inactive": True, "normalStopProof": state["stopProof"]}
            else:
                before = host.snapshot()
                validate(before, state, identity=state["knownActivation"], transition=action == "rollback")
            out["before"] = before
            if action == "rollback":
                require(state["phase"] in ["installed", "caps-applied"] or state.get("recoveryRequired"), "rollback_requires_installed_or_failed_transaction")
                mutation = True
                after = recover_original(host, state, out["steps"])
                out.update(passed=True, after=after)
            else:
                wanted = {"install": "prepared", "apply-caps": "installed", "restore-caps": "caps-applied"}[action]
                require(state["phase"] == wanted, "wrong_transaction_phase")
                if action == "install":
                    state["stagedBinary"] = host.stage_candidate(state)
                    out["steps"].append({"step": "candidate_binary_staged_without_touching_live_release", "file": state["stagedBinary"]})
                immediate = host.snapshot()
                validate(immediate, state, identity=state["knownActivation"])
                host.check_owned(state)
                out["steps"].append({"step": "immediate_pre_stop_empty_and_cas", "snapshot": immediate})
                mutation = True
                state["stopProof"] = host.stop(state["knownActivation"])
                out["steps"].append({"step": "normal_sigint_stop", "result": state["stopProof"]})
                host.guard_inactive(state)
                if action in ["install", "apply-caps"]:
                    kind = "release" if action == "install" else "caps"
                    host.create_files(state, kind)
                    next_phase = "installed" if kind == "release" else "caps-applied"
                else:
                    host.remove_files(state, [CAPS_DROPIN, CAPS_ENV])
                    for path in [CAPS_DROPIN, CAPS_ENV]:
                        del state["ownedFiles"][path]
                    next_phase = "installed"
                host.check_owned(state)
                host.reload()
                configured_exec = host.guard_inactive(state, phase=next_phase)
                if action == "install":
                    state["candidateExecStartSha256"] = configured_exec
                after = host.start()
                require(getattr(host, "last_started_identity", None) == after["identity"], "replacement_activation_during_start")
                state["knownActivation"] = after["identity"]
                validate(after, state, phase=next_phase, identity=state["knownActivation"])
                host.check_owned(state)
                state["phase"] = next_phase
                state["stopProof"] = None
                out.update(passed=True, after=after)
                out["steps"].append({"step": "new_activation_effective_configuration_verified", "snapshot": after})
    except Exception as error:
        out["error"] = str(error) if isinstance(error, Refusal) else type(error).__name__
        if state is not None:
            if mutation:
                state["recoveryRequired"] = True
            state["ownedFiles"].update(getattr(host, "created_files", {}))
            if getattr(host, "last_started_identity", None) is not None:
                state["knownActivation"] = host.last_started_identity
            if mutation and action != "rollback":
                try:
                    after = recover_original(host, state, out["steps"])
                    state.pop("recoveryRequired", None)
                    out["rollback"] = {"passed": True, "after": after}
                except Exception as failure:
                    state["recoveryRequired"] = True
                    out["rollback"] = {"passed": False, "deferred": True,
                                       "error": str(failure) if isinstance(failure, Refusal) else type(failure).__name__,
                                       "noForcedStopOrForeignDeletion": True}
            else:
                out["rollback"] = {"notAttempted": True, "reason": "no service mutation began" if not mutation else "explicit rollback failed closed"}
    if state is not None:
        out["state"] = state
    return out


def command(args):
    # This is the entire service-control surface. No arbitrary shell commands.
    allowed = (args == ["systemctl", "daemon-reload"] or
               args in [["systemctl", "--no-block", verb, UNIT] for verb in ["start", "stop"]] or
               (len(args) == 5 and args[:2] == ["systemctl", "show"] and args[2] in [UNIT, OLD_GATEWAY, ZONE]
                and args[3] == "--no-pager" and args[4].startswith("--property=")))
    require(allowed, "command_outside_test_service_allowlist")
    result = subprocess.run(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, timeout=15, check=False)
    require(result.returncode == 0 and len(result.stdout) <= 131072, "systemctl_command_failed_or_unbounded")
    return result.stdout


def show(unit):
    return parse_show(command(["systemctl", "show", unit, "--no-pager", "--property=" + ",".join(dict.fromkeys(PROPS))]))


def metadata(path):
    p = Path(path)
    info = p.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "file_not_regular_single_link")
    return {"path": str(p), "sha256": file_hash(p), "bytes": info.st_size, "device": info.st_dev,
            "inode": info.st_ino, "mode": stat.S_IMODE(info.st_mode), "uid": info.st_uid,
            "gid": info.st_gid, "mtimeNs": info.st_mtime_ns}


def process_identity(unit, allow_inactive=False):
    pid = int(unit.get("MainPID") or 0)
    if allow_inactive and pid == 0:
        return {"pid": 0, "invocationId": unit.get("InvocationID"), "startTicks": None, "activeState": unit.get("ActiveState")}
    require(pid > 1, "process_identity_missing")
    fields = Path("/proc", str(pid), "stat").read_text().rsplit(")", 1)[1].split()
    require(re.fullmatch(r"[0-9a-f]{32}", unit.get("InvocationID", "")), "invalid_invocation_id")
    return {"pid": pid, "invocationId": unit["InvocationID"], "startTicks": int(fields[19])}


def health(port):
    with urllib.request.urlopen("http://127.0.0.1:" + str(port) + "/health", timeout=4) as response:
        raw = response.read(524289)
    require(len(raw) <= 524288, "health_response_unbounded")
    value = json.loads(raw)
    return {"ok": value.get("ok"), "http": value.get("http"), "ws": value.get("ws"), "revision": value.get("revision"),
            "capacity": {key: value.get("capacity", {}).get(key) for key in CURRENT + list(CAP_ENV) + list(AUTH)},
            "sessionCache": {key: value.get("session_cache", {}).get(key)
                             for key in ["backend", "configured", "healthy", "recordCount", "routeLeaseCount", "staleRecordCount"]}}


class NativeHost:
    def __init__(self, config):
        self.config = config
        self.last_started_identity = None
        self.created_files = {}
        self.argv = None
        self.contents = None

    def unit_status(self):
        unit = show(UNIT)
        return {"active": int(unit.get("MainPID") or 0) > 0 or unit.get("ActiveState") != "inactive"}

    def config_projection(self, unit):
        env = parse_environment_files(unit["EnvironmentFiles"])
        dropins = shlex.split(unit.get("DropInPaths", ""))
        originals = [entry for entry in env if entry["path"] not in OWN_PATHS]
        original_dropins = [path for path in dropins if path not in OWN_PATHS]
        paths = [unit["FragmentPath"]] + original_dropins + [entry["path"] for entry in originals]
        files = [metadata(path) for path in sorted(set(paths))]
        require(all(item["uid"] == 0 and not item["mode"] & 0o022 for item in files), "original_configuration_not_root_protected")
        stable = {key: unit.get(key) for key in STABLE if key != "EnvironmentFiles"}
        return {"protectedConfig": {"properties": stable, "files": files,
                                     "originalEnvironmentFiles": originals, "originalDropIns": original_dropins,
                                     "fragmentPath": unit["FragmentPath"]},
                "originalEnvironmentFiles": originals, "originalDropIns": original_dropins,
                "environmentFiles": env, "dropIns": dropins}

    def peers(self):
        old_unit = show(OLD_GATEWAY)
        old = {"identity": process_identity(old_unit), "health": health(7110)}
        require(process_identity(show(OLD_GATEWAY)) == old["identity"], "old_gateway_changed_during_snapshot")
        zone = process_identity(show(ZONE), allow_inactive=True)
        return old, zone

    def snapshot(self):
        unit = show(UNIT)
        identity = process_identity(unit)
        root = Path("/proc", str(identity["pid"]))
        executable, configured_argv, exec_sha = parse_exec_start(unit.pop("ExecStart"))
        actual_argv = [entry.decode("utf-8") for entry in (root / "cmdline").read_bytes().split(b"\0") if entry]
        require(actual_argv == configured_argv, "configured_and_actual_argv_disagree")
        self.argv = configured_argv
        actual_executable = os.readlink(root / "exe")
        require(os.path.realpath(executable) == actual_executable and not actual_executable.endswith(" (deleted)"), "configured_and_actual_executable_disagree")
        environment = dict(entry.decode("utf-8").split("=", 1) for entry in (root / "environ").read_bytes().split(b"\0") if b"=" in entry)
        excluded = set(CAP_ENV.values()) | {"MIR2_DEPLOY_REVISION", "INVOCATION_ID", "JOURNAL_STREAM", "SYSTEMD_EXEC_PID"}
        protected = {key: value for key, value in environment.items() if key not in excluded}
        h = health(7210)
        old, zone = self.peers()
        result = {"identity": identity, "unit": unit, "health": h, "oldGateway": old, "zoneIdentity": zone,
                  "binarySha256": file_hash(root / "exe"), "executablePath": actual_executable,
                  "configuredExecutablePath": executable,
                  "processWorkingDirectory": os.readlink(root / "cwd"),
                  "argvTailSha256": json_hash(actual_argv[1:]), "execStartSha256": exec_sha,
                  "environmentRevision": environment.get("MIR2_DEPLOY_REVISION"),
                  "environmentCaps": {key: environment.get(key) for key in CAP_ENV.values()},
                  "protectedEnvironmentSha256": json_hash(protected), **self.config_projection(unit)}
        require(process_identity(show(UNIT)) == identity, "target_changed_during_snapshot")
        return result

    def plan(self, state):
        if self.argv is None:
            _, self.argv, _ = parse_exec_start(show(UNIT)["ExecStart"])
        require(json_hash(self.argv[1:]) == state["baseline"]["argvTailSha256"], "plan_original_argv_changed")
        binary = state["candidate"]["releasePath"] + "/mir2-gateway"
        self.contents = {
            RELEASE_ENV: "MIR2_DEPLOY_REVISION=" + state["candidate"]["revision"] + "\n",
            RELEASE_DROPIN: "[Service]\nExecStart=\nExecStart=" + " ".join([binary] + self.argv[1:]) + "\nEnvironmentFile=" + RELEASE_ENV + "\n",
            CAPS_ENV: "".join(CAP_ENV[key] + "=" + str(value) + "\n" for key, value in TEST_CAPS.items()),
            CAPS_DROPIN: "[Service]\nEnvironmentFile=" + CAPS_ENV + "\n"}
        return {path: {"sha256": digest(content.encode()), "bytes": len(content.encode())}
                for path, content in self.contents.items()}

    def require_no_owned_files(self):
        require(not any(os.path.lexists(path) for path in OWN_PATHS), "owned_path_already_present")

    def require_candidate_absent(self):
        require(not os.path.lexists(self.config["candidate"]["releasePath"]), "candidate_release_path_already_present")

    def check_owned(self, state):
        tracked = state["ownedFiles"]
        require(set(tracked).issubset(OWN_PATHS), "owned_path_outside_scope")
        for path in OWN_PATHS:
            require(os.path.lexists(path) == (path in tracked), "foreign_or_missing_owned_path")
        for path, descriptor in tracked.items():
            require(descriptor.get("path") == path and descriptor.get("uid") == 0 and descriptor.get("mode") == 0o600,
                    "owned_descriptor_scope_mismatch")
            require(metadata(path) == descriptor, "owned_file_cas_changed")
            require(descriptor["sha256"] == state["plan"][path]["sha256"], "owned_content_not_planned")
        if state.get("stagedBinary") is not None:
            require(metadata(state["stagedBinary"]["path"]) == state["stagedBinary"], "staged_binary_cas_changed")

    @staticmethod
    def root_directory(path, create=False):
        directory = Path(path)
        if create and not directory.exists():
            NativeHost.root_directory(directory.parent)
            directory.mkdir(mode=0o755)
        for item in [directory] + list(directory.parents):
            info = item.lstat()
            require(stat.S_ISDIR(info.st_mode) and info.st_uid == 0 and not stat.S_IMODE(info.st_mode) & 0o022,
                    "directory_not_root_owned_or_is_writable_or_symlink")

    def create_files(self, state, kind):
        paths = [RELEASE_ENV, RELEASE_DROPIN] if kind == "release" else [CAPS_ENV, CAPS_DROPIN]
        for path in paths:
            self.root_directory(Path(path).parent, create=True)
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            created = os.fstat(descriptor)
            try:
                with os.fdopen(descriptor, "wb") as stream:
                    stream.write(self.contents[path].encode())
                    stream.flush()
                    os.fsync(stream.fileno())
                value = metadata(path)
                self.created_files[path] = value
                state["ownedFiles"][path] = value
                require(value["sha256"] == state["plan"][path]["sha256"], "owned_write_hash_mismatch")
            except Exception:
                # A partial write is removed only while the exclusive inode and
                # an exact prefix of the planned non-secret content are proven.
                try:
                    now = Path(path).lstat()
                    if (now.st_dev, now.st_ino, now.st_uid, stat.S_IMODE(now.st_mode)) == (created.st_dev, created.st_ino, 0, 0o600):
                        require(stat.S_ISREG(now.st_mode) and self.contents[path].encode().startswith(Path(path).read_bytes()), "partial_write_foreign_content")
                        os.unlink(path)
                except FileNotFoundError:
                    pass
                raise

    def remove_files(self, state, paths):
        self.check_owned(state)
        # Drop-ins first, env files second; all entries are checked again before unlink.
        ordered = [path for path in [CAPS_DROPIN, RELEASE_DROPIN, CAPS_ENV, RELEASE_ENV] if path in paths]
        require(set(ordered) == set(paths), "remove_outside_owned_scope")
        for path in ordered:
            require(metadata(path) == state["ownedFiles"][path], "owned_file_changed_during_remove")
            os.unlink(path)

    def stage_candidate(self, state):
        if state.get("stagedBinary") is not None:
            self.check_owned(state)
            return state["stagedBinary"]
        candidate = state["candidate"]
        directory = Path(candidate["releasePath"])
        self.require_candidate_absent()
        self.root_directory(directory.parent, create=True)
        require(os.statvfs(directory.parent).f_bavail * os.statvfs(directory.parent).f_frsize > candidate["binarySizeBytes"] + 32 * 1024 * 1024,
                "remote_staging_space_insufficient")
        directory.mkdir(mode=0o755)
        target = directory / "mir2-gateway"
        descriptor = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o755)
        created = os.fstat(descriptor)
        try:
            print(json.dumps({"readyForArtifact": True, "bytes": candidate["binarySizeBytes"]}), flush=True)
            remaining = candidate["binarySizeBytes"]
            value = hashlib.sha256()
            header = b""
            with os.fdopen(descriptor, "wb") as output:
                while remaining:
                    block = sys.stdin.buffer.read(min(1024 * 1024, remaining))
                    require(block, "artifact_stream_ended_early")
                    if len(header) < 64:
                        header += block[:64 - len(header)]
                    output.write(block)
                    value.update(block)
                    remaining -= len(block)
                output.flush()
                os.fsync(output.fileno())
            require(value.hexdigest() == candidate["binarySha256"] and is_linux_x64(header), "uploaded_binary_hash_or_elf_mismatch")
            result = metadata(target)
            require(result["uid"] == 0 and result["mode"] == 0o755, "staged_binary_owner_mode_mismatch")
            return result
        except Exception:
            now = target.lstat()
            if (now.st_dev, now.st_ino, now.st_uid) == (created.st_dev, created.st_ino, 0) and stat.S_ISREG(now.st_mode):
                target.unlink()
                directory.rmdir()
            raise

    def guard_inactive(self, state, phase=None, transition=False):
        unit = show(UNIT)
        require(unit.get("MainPID") == "0" and unit.get("ActiveState") == "inactive", "target_not_inactive")
        stop_observation = (state.get("stopProof") or {}).get("inactiveObservation")
        require(stop_observation is not None and {key: unit.get(key) for key in stop_observation} == stop_observation,
                "inactive_activation_no_longer_matches_normal_stop")
        executable, argv, exec_sha = parse_exec_start(unit.pop("ExecStart"))
        projection = self.config_projection(unit)
        require(projection["protectedConfig"] == state["baseline"]["protectedConfig"], "inactive_protected_config_changed")
        require(json_hash(argv[1:]) == state["baseline"]["argvTailSha256"], "inactive_argv_tail_changed")
        managed_layout(projection, state["baseline"], phase or state["phase"], transition)
        original_executable = state["baseline"].get("configuredExecutablePath", state["baseline"]["executablePath"])
        expected = original_executable if (phase or state["phase"]) in ["prepared", "rolled-back"] else state["candidate"]["releasePath"] + "/mir2-gateway"
        if transition:
            require(executable in [original_executable, state["candidate"]["releasePath"] + "/mir2-gateway"], "inactive_foreign_exec_start")
        else:
            require(executable == expected, "inactive_exec_start_mismatch")
            pinned_exec = state["baseline"]["execStartSha256"] if (phase or state["phase"]) in ["prepared", "rolled-back"] else state.get("candidateExecStartSha256")
            if pinned_exec is not None:
                require(exec_sha == pinned_exec, "inactive_exec_start_static_changed")
        old, zone = self.peers()
        require(old["identity"] == state["baseline"]["oldGateway"]["identity"] and old["health"].get("ok") is True
                and old["health"].get("http") == "ready" and old["health"].get("ws") == "ready"
                and old["health"]["sessionCache"].get("healthy") is True, "inactive_old_gateway_changed_or_unhealthy")
        require(zone == state["baseline"]["zoneIdentity"], "inactive_zone_activation_changed")
        self.check_owned(state)
        return exec_sha

    def reload(self):
        command(["systemctl", "daemon-reload"])

    def stop(self, expected):
        unit = show(UNIT)
        require(process_identity(unit) == expected, "activation_changed_before_stop_command")
        require(unit.get("SendSIGKILL") == "no" and unit.get("KillSignal") in ["2", "SIGINT"], "unsafe_stop_policy")
        command(["systemctl", "--no-block", "stop", UNIT])
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            unit = show(UNIT)
            pid = int(unit.get("MainPID") or 0)
            if pid == 0 and unit.get("ActiveState") == "inactive":
                require(unit.get("Result") == "success" and unit.get("ExecMainCode") == "1" and unit.get("ExecMainStatus") == "0",
                        "normal_main_exit_not_proven")
                observation = {key: unit.get(key) for key in ["InvocationID", "ExecMainPID", "ExecMainStartTimestampMonotonic",
                                                              "ExecMainExitTimestampMonotonic", "Result", "ExecMainCode", "ExecMainStatus"]}
                require(unit.get("ExecMainPID") == str(expected["pid"]) and unit.get("ExecMainStartTimestampMonotonic") not in [None, "0"],
                        "stopped_main_identity_not_proven")
                return {"identity": expected, "activeState": "inactive", "result": "success", "execMainCode": 1,
                        "execMainStatus": 0, "inactiveObservation": observation}
            require(pid in [0, expected["pid"]] and unit.get("ActiveState") in ["active", "deactivating", "inactive"], "replacement_or_abnormal_stop")
            time.sleep(.25)
        raise Refusal("normal_stop_timeout_no_sigkill")

    def start(self):
        unit = show(UNIT)
        require(unit.get("MainPID") == "0" and unit.get("ActiveState") == "inactive", "start_requires_inactive")
        self.last_started_identity = None
        command(["systemctl", "--no-block", "start", UNIT])
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            unit = show(UNIT)
            if int(unit.get("MainPID") or 0) > 1 and unit.get("ActiveState") == "active":
                observed = process_identity(unit)
                if self.last_started_identity is None:
                    self.last_started_identity = observed
                require(observed == self.last_started_identity, "replacement_activation_during_start")
                try:
                    h = health(7210)
                    if h.get("ok") is True and h.get("http") == "ready" and h.get("ws") == "ready":
                        return self.snapshot()
                except (OSError, ValueError):
                    pass
            require(unit.get("ActiveState") != "failed", "service_start_failed_no_force_recovery")
            time.sleep(.25)
        raise Refusal("start_health_timeout_no_sigkill")


def is_linux_x64(header):
    return len(header) >= 20 and header[:6] == b"\x7fELF\x02\x01" and header[18:20] == b"\x3e\x00"


def inspect_artifact(directory, revision, maps_path, maps_sha, confirm):
    require(re.fullmatch(r"[0-9a-f]{40}", revision or ""), "candidate_full_git_sha_required")
    require(confirm and re.fullmatch(r"[0-9a-f]{64}", maps_sha or ""), "explicit_matching_maps_evidence_required")
    directory = Path(directory).expanduser().resolve()
    manifest_path, binary = directory / "RELEASE.json", directory / "mir2-gateway"
    for path in [manifest_path, binary, Path(maps_path).expanduser()]:
        require(not path.is_symlink() and path.is_file(), "artifact_or_evidence_must_be_regular_file")
    require(manifest_path.stat().st_size <= 65536, "release_manifest_too_large")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    require(manifest.get("name") == "mir2-gateway" and manifest.get("gitRevision") == revision and manifest.get("gitDirty") is False,
            "release_manifest_revision_dirty_or_name_mismatch")
    size = binary.stat().st_size
    require(type(manifest.get("binarySizeBytes")) is int and size == manifest["binarySizeBytes"] and 0 < size <= 512 * 1024 * 1024,
            "release_binary_size_mismatch_or_too_large")
    require(re.fullmatch(r"[0-9a-f]{64}", manifest.get("binarySha256", "")) and file_hash(binary) == manifest["binarySha256"], "release_binary_sha_mismatch")
    with binary.open("rb") as stream:
        require(is_linux_x64(stream.read(64)), "candidate_must_be_linux_x64_elf")
    evidence = Path(maps_path).expanduser().resolve()
    require(file_hash(evidence) == maps_sha, "maps_evidence_hash_mismatch")
    return ({"revision": revision, "binarySha256": manifest["binarySha256"], "binarySizeBytes": size,
             "manifestSha256": file_hash(manifest_path), "releasePath": RELEASE_ROOT + "/" + revision + RELEASE_SUFFIX},
            {"path": str(evidence), "sha256": maps_sha, "bytes": evidence.stat().st_size,
             "matchingMapsExplicitlyConfirmedByOperator": True, "contentsReviewedExternally": True},
            {"directory": str(directory)})


def private_write_new(path, value):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    if os.name != "nt":
        require(stat.S_IMODE(Path(path).stat().st_mode) == 0o600, "local_receipt_not_private")


def output_directory(value):
    path = Path(value).expanduser().resolve()
    repository = Path(__file__).resolve().parents[4]
    require(path != repository and repository not in path.parents, "receipts_must_stay_outside_repository")
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    require(not path.is_symlink(), "output_directory_must_not_be_symlink")
    return path


def ssh_call(client, password, action, config, state, artifact_binary=None):
    payload = {**config, "action": action, "state": state}
    if state is not None:
        # These immutable inputs already occur inside state. Avoid duplicate
        # command-line payloads; remote_main restores only from the same state.
        payload.pop("predecessor", None)
        payload.pop("strictArtifactProof", None)
    encoded = base64.b64encode(json.dumps(payload).encode()).decode()
    source = Path(__file__).read_text(encoding="utf-8")
    compressed = base64.b64encode(zlib.compress(source.encode())).decode()
    bootstrap = "import base64,zlib;exec(zlib.decompress(base64.b64decode(" + repr(compressed) + ")))"
    require(password is None, "sudo_password_forbidden")
    require(len(encoded) <= 98304, "remote_payload_unbounded")
    stdin, stdout, stderr = client.exec_command("sudo -n python3 -u -c " + shlex.quote(bootstrap) +
                                              " --remote-payload " + shlex.quote(encoded), timeout=480)
    channel = stdout.channel
    require(stdin.channel is channel and stderr.channel is channel, "ssh_rpc_streams_do_not_share_channel")
    # Never pre-write a password: cached/NOPASSWD sudo leaves stdin for Python.
    # Poll both SSH streams instead of blocking on stdout while sudo prompts on
    # stderr. Only this invocation's complete marker may authorize one password.
    pending_stdout, pending_stderr = bytearray(), bytearray()
    rpc_started = artifact_sent = False
    receipt = None
    stdout_bytes = stderr_bytes = 0
    deadline = time.monotonic() + 480

    def check_time():
        require(time.monotonic() < deadline, "ssh_rpc_timeout_no_retry")

    def read_stderr():
        nonlocal stderr_bytes
        while channel.recv_stderr_ready():
            block = channel.recv_stderr(4096)
            require(block, "ssh_stderr_stream_closed")
            stderr_bytes += len(block)
            require(stderr_bytes <= 16384, "sudo_stderr_unbounded")
            pending_stderr.extend(block)
        require(not pending_stderr, "sudo_n_or_remote_stderr_refused_no_retry")

    def send_artifact(value):
        nonlocal artifact_sent
        require(rpc_started and not artifact_sent and receipt is None and action == "install" and artifact_binary is not None,
                "unexpected_artifact_stream_request")
        size = config["candidate"]["binarySizeBytes"]
        require(type(value.get("bytes")) is int and value["bytes"] == size and 0 < size <= 512 * 1024 * 1024,
                "artifact_stream_size_mismatch")
        require(Path(artifact_binary).stat().st_size == size, "local_artifact_size_changed")
        remaining = size
        with open(artifact_binary, "rb") as stream:
            while remaining:
                check_time()
                read_stderr()
                require(not pending_stderr, "incomplete_sudo_prompt_before_artifact")
                require(not channel.closed and not getattr(channel, "eof_received", False) and not channel.exit_status_ready(),
                        "ssh_closed_before_artifact_completed")
                block = stream.read(min(1024 * 1024, remaining))
                require(block, "local_artifact_ended_early")
                channel.sendall(block)
                remaining -= len(block)
            require(not stream.read(1), "local_artifact_grew_during_upload")
        artifact_sent = True
        channel.shutdown_write()

    try:
        channel.settimeout(30)
        if action != "install":
            channel.shutdown_write()
        while True:
            check_time()
            read_stderr()
            while channel.recv_ready():
                block = channel.recv(65536)
                require(block, "ssh_stdout_stream_closed")
                stdout_bytes += len(block)
                require(stdout_bytes <= 2 * 524288 + 256, "remote_rpc_output_unbounded")
                pending_stdout.extend(block)
                require(len(pending_stdout) <= 524288, "remote_rpc_line_unbounded")
                while b"\n" in pending_stdout:
                    line, _, rest = pending_stdout.partition(b"\n")
                    pending_stdout[:] = rest
                    if not line.strip():
                        continue
                    try:
                        value = json.loads(line)
                    except (ValueError, UnicodeError):
                        raise Refusal("remote_rpc_invalid_json") from None
                    require(isinstance(value, dict), "remote_rpc_invalid_message")
                    if not rpc_started:
                        require(set(value) == {"rpcStarted"} and value["rpcStarted"] is True, "remote_rpc_start_ack_required")
                        rpc_started = True
                    else:
                        read_stderr()
                        require(not pending_stderr, "incomplete_sudo_prompt_at_rpc_boundary")
                        if value.get("readyForArtifact") is True:
                            send_artifact(value)
                        else:
                            require(receipt is None and value.get("schema") == SCHEMA and value.get("action") == action
                                    and type(value.get("passed")) is bool, "remote_receipt_invalid")
                            receipt = value
                    read_stderr()
                    require(not pending_stderr, "incomplete_sudo_prompt_at_rpc_boundary")
            read_stderr()
            if channel.exit_status_ready() and not channel.recv_ready() and not channel.recv_stderr_ready():
                require(not pending_stderr and not pending_stdout.strip(), "ssh_closed_with_partial_rpc_or_prompt")
                require(rpc_started and receipt is not None and channel.recv_exit_status() == 0, "remote_receipt_missing_or_failed")
                return receipt
            require(not channel.closed, "ssh_channel_closed_before_complete_receipt")
            require(receipt is not None or not getattr(channel, "eof_received", False), "ssh_eof_before_complete_receipt")
            time.sleep(.01)
    finally:
        channel.close()


# Reviewed source/CI contract. No arbitrary candidate, host, unit, or rollback.
V5_BASE_SHA256 = "9f1f6437c50bc1d65df916ac0854d893468656a8647db321acae38fef835a35e"
V7_VERIFIER_SHA256 = "66e843b535f08c34adf8971290f7005d8fbf078fb803ed88ceb43530797b42f5"
OLD_REVISION = "82efff2c8c0eef72a950501d84acad3c6c3de958"
OLD_BINARY_SHA256 = "6ed4ee817b2577858860a9bf53cea12845d5ab2cb0d855eb5ffa678ba9ccc01a"
CANDIDATE_REVISION = "5a356ae0d5b79b846675f623bd5d2dc21c09a10f"
INTEGRATION_COMMIT = "6f04b710766a9e7a2deb553e0f5ec0e1761db598"
CI_RUN_ID = 43432291642
CI_NATIVE_JOB_ID = 131908522957
ARTIFACT_IDS = {"gateway": 14042061989, "nativeEvidence": 14042696568}
GATEWAY_ZIP_SHA256 = "2d0d6fad24bb9f1449ae5bb84f71c83bf4f11393dbfd6d78065b0bc812a9f8fe"
HOST_KEY_SHA256 = "SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8"
PROTECTED_SCHEMA = "mir2.scene-successor-protected-baseline.v1"
ARTIFACT_SCHEMA = "mir2.scene-candidate-artifact-verification.v7"
REQUIRED_GATES = sorted(["activities_postgres", "billing_postgres", "gateway-package", "gateway-postgres-startup",
                         "linux-release-security", "monthly_card_postgres", "schema_union"])
SOURCE_PATHS = sorted(["mir2-web3/apps/gateway/src/" + x for x in [
    "routing.rs", "session.rs", "web.rs", "spectator.rs", "routing/tests/spectator_scene_projection_tests.rs"]] +
    ["mir2-web3/apps/simulation/src/" + x for x in ["runtime/packets.rs", "runtime/session.rs", "world_runtime.rs",
                                                 "runtime/spectator_scene_snapshot_tests.rs"]])


def sha_string(value, length=64):
    return isinstance(value, str) and re.fullmatch("[0-9a-f]{" + str(length) + "}", value) is not None


def read_hashed_json(path, expected, maximum=4 * 1024 * 1024):
    require(sha_string(expected), "explicit_input_sha256_required")
    original = Path(path).expanduser().absolute()
    for part in [original, *original.parents]:
        require(not part.is_symlink() and not (hasattr(part, "is_junction") and part.is_junction()), "linked_input_refused")
    p = original.resolve()
    info = p.stat()
    require(p.is_file() and info.st_nlink == 1 and 0 < info.st_size <= maximum, "bounded_regular_json_required")
    raw = p.read_bytes()
    require(digest(raw) == expected, "input_sha256_mismatch")
    value = json.loads(raw)
    require(isinstance(value, dict), "input_json_object_required")
    return value, {"path": str(p), "sha256": expected, "bytes": len(raw)}


def source_guard():
    here = Path(__file__).resolve()
    require(len(here.parents) > 3, "canonical_operator_source_required")
    repository = here.parents[3]
    require(repository.name == "mir2-web3" and here.parent == repository / "scripts/capacity/cloud-reference",
            "canonical_operator_source_required")
    require(file_hash(here.with_name("cloud_test_release_v5.py")) == V5_BASE_SHA256, "frozen_v5_source_guard_changed")
    require(file_hash(repository / "scripts/capacity/comparable-reference/cloud/verify_scene_candidate_artifact.py") == V7_VERIFIER_SHA256,
            "frozen_v7_verifier_source_guard_changed")
    result = subprocess.run([sys.executable, str(repository / "scripts/verify_unified_codeline.py")],
                            cwd=repository.parent, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=20, check=False)
    require(result.returncode == 0 and len(result.stdout) <= 131072, "unified_codeline_source_guard_failed")


def guard_private_output(output):
    if os.name != "nt":
        info = output.stat()
        require(info.st_uid == os.getuid() and not stat.S_IMODE(info.st_mode) & 0o077, "private_output_mode_required")
        return
    # Inspect only: the operator precreates an explicit operator/SYSTEM ACL.
    script = "$p=$env:MIR2_SCENE_OUTPUT;$a=Get-Acl -LiteralPath $p;$sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value;" + \
             "$ok=$a.AreAccessRulesProtected;foreach($r in $a.Access){$id=$r.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value;" + \
             "if($r.AccessControlType -eq 'Allow' -and $id -notin @($sid,'S-1-5-18')){$ok=$false}};if(-not $ok){exit 7}"
    env = dict(os.environ, MIR2_SCENE_OUTPUT=str(output))
    result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand",
                             base64.b64encode(script.encode("utf-16le")).decode()], env=env,
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                            timeout=20, check=False, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    require(result.returncode == 0, "private_output_acl_required")


def role_pids(value):
    require(isinstance(value, list) and 6 <= len(value) <= 13, "protected_roles_required")
    roles, pids = [], []
    for row in value:
        require(isinstance(row, dict) and set(row) == {"role", "pid"}, "protected_role_fields_invalid")
        role, pid = row["role"], row["pid"]
        require(isinstance(role, str) and re.fullmatch(r"caddy|validator-[0-3]|vpn-[0-7]", role)
                and type(pid) is int and pid > 1, "protected_role_or_pid_invalid")
        roles.append(role); pids.append(pid)
    require(len(set(roles)) == len(roles) and len(set(pids)) == len(pids), "duplicate_protected_identity")
    require(set(roles) >= {"caddy", "validator-0", "validator-1", "validator-2", "validator-3"}
            and any(x.startswith("vpn-") for x in roles), "caddy_four_validators_and_vpn_required")
    return sorted(value, key=lambda row: row["role"])


def protected_inventory(roles):
    result = []
    for row in role_pids(roles):
        root = Path("/proc", str(row["pid"]))
        first = root.joinpath("stat").read_bytes().rsplit(b")", 1)[1].split()
        require(first[0] not in [b"Z", b"X"], "protected_process_not_live")
        start = int(first[19])
        value = {"role": row["role"], "pid": row["pid"], "startTicks": start,
                 "executableSha256": file_hash(root / "exe"), "commSha256": digest((root / "comm").read_bytes()),
                 "cgroupSha256": digest((root / "cgroup").read_bytes())}
        again = root.joinpath("stat").read_bytes().rsplit(b")", 1)[1].split()
        require(int(again[19]) == start and again[0] not in [b"Z", b"X"], "protected_process_changed_during_sample")
        result.append(value)
    return result


def validate_protected_proof(value, expected_pid, now_ms=None):
    require(isinstance(value, dict) and value.get("schema") == PROTECTED_SCHEMA and value.get("readonly") is True,
            "protected_status_proof_required")
    roles = role_pids(value.get("roles"))
    identity = value.get("targetIdentity")
    require(isinstance(identity, dict) and set(identity) == {"pid", "invocationId", "startTicks"}
            and identity["pid"] == expected_pid and type(identity["startTicks"]) is int and identity["startTicks"] > 0
            and re.fullmatch(r"[0-9a-f]{32}", identity.get("invocationId", "")), "protected_proof_target_identity_invalid")
    rows = value.get("protectedProcesses")
    require(isinstance(rows, list) and len(rows) == len(roles), "complete_protected_fingerprints_required")
    for row, role in zip(rows, roles):
        require(isinstance(row, dict) and row.get("role") == role["role"] and row.get("pid") == role["pid"]
                and type(row.get("startTicks")) is int and row["startTicks"] > 0
                and all(sha_string(row.get(key)) for key in ["executableSha256", "commSha256", "cgroupSha256"]),
                "protected_fingerprint_invalid")
    require(value.get("stableFingerprintSha256") == json_hash(rows), "protected_fingerprint_hash_mismatch")
    now_ms = round(time.time() * 1000) if now_ms is None else now_ms
    require(type(value.get("sampledAtUnixMs")) is int and 0 <= now_ms - value["sampledAtUnixMs"] <= 300000,
            "fresh_protected_status_proof_required")
    require(value.get("targetRevision") in [OLD_REVISION, CANDIDATE_REVISION] and sha_string(value.get("targetBinarySha256")),
            "protected_target_binary_revision_invalid")
    return roles


def validate_artifact_proof(proof):
    require(isinstance(proof, dict) and proof.get("schema") == ARTIFACT_SCHEMA and proof.get("passed") is True,
            "strict_v7_artifact_proof_required")
    require((proof.get("candidateRevision"), proof.get("integrationCommit"), proof.get("baselineRevision"), proof.get("baselineBinarySha256")) ==
            (CANDIDATE_REVISION, INTEGRATION_COMMIT, OLD_REVISION, OLD_BINARY_SHA256), "reviewed_source_chain_required")
    require(proof.get("runId") == CI_RUN_ID and proof.get("nativeJobId") == CI_NATIVE_JOB_ID
            and type(proof["nativeJobId"]) is int and proof.get("wholeWorkflowStatus") == "completed"
            and proof.get("wholeWorkflowConclusion") == "success", "whole_green_exact_ci_and_native_job_required")
    require(proof.get("nativeRequiredGates") == REQUIRED_GATES and type(proof.get("nativeStartupActualExitCode")) is int
            and proof["nativeStartupActualExitCode"] == 0, "all_seven_native_gates_and_actual_startup_required")
    require(proof.get("verifierSourceSha256") == V7_VERIFIER_SHA256 and proof.get("deploymentApproved") is False
            and proof.get("capacityQualified") is False, "artifact_scope_or_verifier_pin_invalid")
    require(all(sha_string(proof.get(key)) for key in ["binarySha256", "releaseManifestSha256", "candidatePinsSha256",
            "actionsMetadataSha256", "sourceInputManifestSha256"]) and type(proof.get("binarySizeBytes")) is int
            and 0 < proof["binarySizeBytes"] <= 512 * 1024 * 1024 and isinstance(proof.get("artifactDirectory"), str),
            "complete_actual_artifact_pins_required")
    artifacts = proof.get("artifacts")
    require(isinstance(artifacts, dict), "actual_artifact_ids_required")
    for key, expected_id in ARTIFACT_IDS.items():
        row = artifacts.get(key)
        require(isinstance(row, dict) and type(row.get("id")) is int and row["id"] == expected_id
                and sha_string(row.get("sha256")) and isinstance(row.get("name"), str) and row["name"]
                and isinstance(row.get("path"), str), "actual_artifact_ids_required")
    require(artifacts["gateway"]["sha256"] == GATEWAY_ZIP_SHA256, "reviewed_gateway_zip_required")
    maps = proof.get("maps")
    require(isinstance(maps, dict) and maps.get("fileCount") == 1620 and maps.get("totalFileBytes") == 42111309
            and maps.get("allGitArtifactLiveMatch") is True and sha_string(maps.get("liveProofSha256")), "all_original_maps_match_required")
    compat = proof.get("sourceCompatibility")
    require(isinstance(compat, dict) and (compat.get("baselineSource"), compat.get("integrationCommit"), compat.get("candidateSource")) ==
            (OLD_REVISION, INTEGRATION_COMMIT, CANDIDATE_REVISION) and compat.get("protectedProtocolDataCargoAndMapsIdentical") is True
            and sorted(compat.get("reviewedSourcePaths", [])) == SOURCE_PATHS, "exact_reviewed_backend_equivalence_required")
    delta = compat.get("canonicalNonNativeDelta")
    require(isinstance(delta, dict) and delta.get("path") == ".github/scripts/numeron-r16-release.mjs"
            and delta.get("exactWholeRepositoryDelta") == [delta["path"]]
            and delta.get("nativeJobDirectCallExcluded") is True and delta.get("nativeInputScriptReferenceExcluded") is True,
            "only_reviewed_non_native_ci_delta_required")
    objects = delta.get("nativeBuildInputObjects")
    require(isinstance(objects, list) and len(objects) == 11
            and len({x.get("path") for x in objects if isinstance(x, dict)}) == 11
            and all(isinstance(x, dict) and sha_string(x.get("integrationObject"), 40)
                    and x["integrationObject"] == x.get("ciObject") for x in objects), "all_native_build_inputs_equal_required")
    return proof


def predecessor_excerpt(value, descriptor):
    require(isinstance(value, dict) and value.get("schema") == SCHEMA and value.get("phase") == "installed"
            and value.get("toolSha256") == V5_BASE_SHA256 and value.get("oldBinaryRollbackAllowed") is False
            and not value.get("recoveryRequired") and value.get("stopProof") is None,
            "complete_installed_v5_predecessor_required")
    candidate = value.get("candidate", {})
    require(candidate.get("revision") == OLD_REVISION and candidate.get("binarySha256") == OLD_BINARY_SHA256
            and candidate.get("releasePath") == RELEASE_ROOT + "/" + OLD_REVISION + "-equip-logout-20261112-v5",
            "exact_v5_predecessor_candidate_required")
    owned = value.get("ownedFiles", {})
    require(set(owned) == {PREVIOUS_V5_RELEASE_ENV, PREVIOUS_V5_RELEASE_DROPIN},
            "v5_default_caps_and_only_persistent_pair_required")
    for path, item in owned.items():
        require(isinstance(item, dict) and item.get("path") == path and item.get("uid") == 0 and item.get("mode") == 0o600
                and item.get("sha256") == value.get("plan", {}).get(path, {}).get("sha256"), "predecessor_owned_plan_invalid")
    baseline = value.get("baseline", {})
    require(len(baseline.get("originalEnvironmentFiles", [])) == 17 and sha_string(value.get("candidateExecStartSha256")),
            "v5_predecessor_baseline_required")
    return {"stateFile": descriptor, "phase": "installed", "knownActivation": value.get("knownActivation"),
            "candidate": candidate, "ownedFiles": owned, "protectedConfig": baseline["protectedConfig"],
            "protectedEnvironmentSha256": baseline["protectedEnvironmentSha256"],
            "oldGatewayIdentity": baseline["oldGateway"]["identity"], "zoneIdentity": baseline["zoneIdentity"],
            "originalEnvironmentFiles": baseline["originalEnvironmentFiles"], "originalDropIns": baseline["originalDropIns"],
            "argvTailSha256": baseline["argvTailSha256"], "processWorkingDirectory": baseline["processWorkingDirectory"],
            "candidateExecStartSha256": value["candidateExecStartSha256"]}


def validate_predecessor_snapshot(snapshot, predecessor):
    require(snapshot["identity"] == predecessor["knownActivation"], "v5_predecessor_activation_changed")
    candidate = predecessor["candidate"]
    require((snapshot["health"]["revision"], snapshot["binarySha256"], snapshot["executablePath"]) ==
            (OLD_REVISION, OLD_BINARY_SHA256, candidate["releasePath"] + "/mir2-gateway"), "v5_predecessor_runtime_changed")
    require(snapshot["execStartSha256"] == predecessor["candidateExecStartSha256"]
            and snapshot["argvTailSha256"] == predecessor["argvTailSha256"]
            and snapshot["processWorkingDirectory"] == predecessor["processWorkingDirectory"],
            "v5_predecessor_exec_or_working_directory_changed")
    require(snapshot["protectedEnvironmentSha256"] == predecessor["protectedEnvironmentSha256"],
            "v5_predecessor_non_managed_environment_changed")
    require(snapshot["oldGateway"]["identity"] == predecessor["oldGatewayIdentity"]
            and snapshot["zoneIdentity"] == predecessor["zoneIdentity"], "v5_predecessor_protected_world_identity_changed")
    expected = copy.deepcopy(predecessor["protectedConfig"])
    expected["files"] = sorted(expected["files"] + list(predecessor["ownedFiles"].values()), key=lambda item: item["path"])
    expected["originalEnvironmentFiles"] = predecessor["originalEnvironmentFiles"] + [{"path": PREVIOUS_V5_RELEASE_ENV, "ignoreErrors": False}]
    expected["originalDropIns"] = predecessor["originalDropIns"] + [PREVIOUS_V5_RELEASE_DROPIN]
    require(snapshot["protectedConfig"] == expected, "v5_predecessor_protected_configuration_changed")


def validate(snapshot, state, phase=None, drained=True, identity=None, transition=False):
    _validate_v5(snapshot, state, phase, drained, identity, transition)
    require(snapshot.get("protectedProcesses") == state["baseline"].get("protectedProcesses")
            and isinstance(snapshot.get("protectedProcesses"), list), "additional_protected_processes_changed")


class SceneNativeHost(NativeHost):
    def _protected(self):
        rows = protected_inventory(self.config["protectedRoles"])
        if self.config.get("protectedProof") is not None:
            require(rows == self.config["protectedProof"]["protectedProcesses"], "additional_protected_processes_changed")
        return rows

    def snapshot(self):
        before = self._protected()
        result = super().snapshot()
        require(self._protected() == before, "additional_protected_processes_changed_during_snapshot")
        require(not {x["pid"] for x in before}.intersection({result["identity"]["pid"], result["oldGateway"]["identity"]["pid"], result["zoneIdentity"]["pid"]}),
                "protected_roles_overlap_test_or_old_world")
        result["protectedProcesses"] = before
        if self.config.get("checkPredecessor"):
            require(not any(os.path.lexists(path) for path in [PREVIOUS_V5_CAPS_ENV, PREVIOUS_V5_CAPS_DROPIN]),
                    "v5_temporary_caps_paths_must_be_absent")
            validate_predecessor_snapshot(result, self.config["predecessor"])
        return result

    def stop(self, expected):
        self._protected()
        result = super().stop(expected)
        self._protected()
        return result

    def start(self):
        self._protected()
        result = super().start()
        self._protected()
        return result

    def guard_inactive(self, state, phase=None, transition=False):
        self._protected()
        value = super().guard_inactive(state, phase, transition)
        self._protected()
        return value


def transaction(action, host, config, state=None):
    refusal = {"schema": SCHEMA, "action": action, "passed": False, "unit": UNIT,
               "userDataOperations": False, "sigkillRequested": False, "noServiceMutationBegan": True}
    try:
        require(action in ["prepare", "install", "apply-caps", "restore-caps", "status"], "scene_v6_action_required_no_downgrade")
        require(config.get("baseSourceSha256") == V5_BASE_SHA256, "frozen_v5_source_guard_changed")
        require(config.get("oldBinaryRollbackAllowed") is False, "scene_v6_old_binary_rollback_must_be_denied")
        roles = role_pids(config.get("protectedRoles"))
        require(roles == config["protectedRoles"], "protected_roles_must_be_canonical")
        if action == "status":
            snapshot = host.snapshot()
            require(snapshot["identity"]["pid"] == config["expectedPid"], "expected_pid_mismatch")
            basic(snapshot)
            proof = {"schema": PROTECTED_SCHEMA, "readonly": True, "sampledAtUnixMs": round(time.time() * 1000),
                     "roles": roles, "targetIdentity": snapshot["identity"], "targetRevision": snapshot["health"]["revision"],
                     "targetBinarySha256": snapshot["binarySha256"], "protectedProcesses": snapshot["protectedProcesses"],
                     "stableFingerprintSha256": json_hash(snapshot["protectedProcesses"])}
            return {**refusal, "passed": True, "readonly": True, "snapshot": snapshot, "protectedProof": proof}
        validate_protected_proof(config.get("protectedProof"), config["expectedPid"])
        proof = validate_artifact_proof(config.get("strictArtifactProof"))
        candidate = config.get("candidate", {})
        require((candidate.get("revision"), candidate.get("binarySha256"), candidate.get("binarySizeBytes"), candidate.get("manifestSha256"), candidate.get("releasePath")) ==
                (CANDIDATE_REVISION, proof["binarySha256"], proof["binarySizeBytes"], proof["releaseManifestSha256"], RELEASE_ROOT + "/" + CANDIDATE_REVISION + RELEASE_SUFFIX),
                "candidate_not_bound_to_strict_proof")
        require(config.get("oldPins") == {"revision": OLD_REVISION, "binarySha256": OLD_BINARY_SHA256}
                and config.get("predecessor", {}).get("candidate", {}).get("revision") == OLD_REVISION,
                "predecessor_link_required")
        require(isinstance(config.get("sceneV6Pins"), dict) and config["sceneV6Pins"].get("protectedFingerprintSha256") ==
                config["protectedProof"]["stableFingerprintSha256"], "scene_successor_pins_required")
        if state is not None:
            require(state.get("sceneV6Pins") == config["sceneV6Pins"] and state.get("predecessor") == config["predecessor"]
                    and state.get("strictArtifactProof") == proof, "scene_successor_proof_or_lineage_changed")
        elif action != "prepare":
            raise Refusal("prepare_first_in_fresh_successor_directory")
        snapshot = host.snapshot()
        require(snapshot["identity"] == config["protectedProof"]["targetIdentity"]
                and snapshot["health"]["revision"] == config["protectedProof"]["targetRevision"]
                and snapshot["binarySha256"] == config["protectedProof"]["targetBinarySha256"], "fresh_protected_proof_target_changed")
        require(snapshot.get("protectedProcesses") == config["protectedProof"]["protectedProcesses"], "additional_protected_processes_changed")
        if action == "prepare":
            validate_predecessor_snapshot(snapshot, config["predecessor"])
    except Exception as error:
        return {**refusal, "error": str(error) if isinstance(error, Refusal) else type(error).__name__}
    # All service mutations remain inside the V5 transaction state machine.
    result = _transaction_core(action, host, config, state)
    if result.get("state") is not None:
        result["state"]["sceneV6Pins"] = config["sceneV6Pins"]
        result["state"]["predecessor"] = config["predecessor"]
        result["state"]["strictArtifactProof"] = proof
    return result


def remote_main(encoded):
    config = json.loads(base64.b64decode(encoded, validate=True))
    require(os.geteuid() == 0 and isinstance(config, dict), "root_and_json_required")
    if isinstance(config.get("state"), dict):
        config["predecessor"] = config["state"].get("predecessor")
        config["strictArtifactProof"] = config["state"].get("strictArtifactProof")
    print(json.dumps({"rpcStarted": True}), flush=True)
    def interrupted(number, frame):
        raise Refusal("controller_signal_interrupted")
    for kind in [signal.SIGHUP, signal.SIGTERM, signal.SIGINT]:
        signal.signal(kind, interrupted)
    result = transaction(config["action"], SceneNativeHost(config), config, config.get("state"))
    print(json.dumps(result, separators=(",", ":")), flush=True)


def connect_pinned(paramiko):
    sock = socket.create_connection((HOST, 22), timeout=20)
    transport = paramiko.Transport(sock)
    transport.auth_timeout = transport.banner_timeout = 20
    try:
        transport.start_client(timeout=20)
        key = transport.get_remote_server_key()
        actual = "SHA256:" + base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip("=")
        require(key.get_name() == "ssh-ed25519" and actual == HOST_KEY_SHA256,
                "ssh_ed25519_host_key_pin_mismatch_before_auth")
        password = getpass.getpass("SSH ubuntu password (memory only; no sudo password): ")
        try:
            transport.auth_password("ubuntu", password, fallback=False)
        finally:
            password = None
        require(transport.is_authenticated(), "ssh_authentication_failed")
        class Client:
            def exec_command(self, command, timeout):
                channel = transport.open_session(timeout=timeout)
                channel.settimeout(timeout)
                channel.exec_command(command)
                return channel.makefile_stdin("wb"), channel.makefile("rb"), channel.makefile_stderr("rb")
            def close(self):
                transport.close(); sock.close()
        return Client()
    except Exception:
        transport.close(); sock.close()
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--action", required=True, choices=["prepare", "install", "apply-caps", "restore-caps", "status"])
    parser.add_argument("--output", required=True, help="Precreated private outside-repository release-scene-v6 directory")
    parser.add_argument("--expected-pid", required=True, type=int)
    parser.add_argument("--protected-pids", help="Status only: public JSON {roles:[{role,pid}]} caddy+validator-0..3+vpn-0...")
    parser.add_argument("--protected-pids-sha256")
    parser.add_argument("--protected-proof", help="Fresh status receipt containing protectedProof, or that exact proof object")
    parser.add_argument("--protected-proof-sha256")
    parser.add_argument("--predecessor-state", help="Prepare only: unchanged V5 installed state")
    parser.add_argument("--predecessor-state-sha256")
    parser.add_argument("--artifact-proof", help="Prepare only: actual passed V7 artifact receipt, never source-only")
    parser.add_argument("--artifact-proof-sha256")
    parser.add_argument("--artifact-dir", help="Prepare only: exact extracted release directory from artifact receipt")
    parser.add_argument("--deny-old-binary-rollback", action="store_true", help="Required on prepare; immutable thereafter")
    parser.add_argument("--paramiko-runtime", default=str(Path.home() / ".codex/tmp/mir2-deploy-ssh-runtime"))
    args = parser.parse_args(argv)
    require(args.expected_pid > 1, "explicit_expected_pid_required")
    require(not args.deny_old_binary_rollback or args.action == "prepare", "rollback_policy_flag_is_prepare_only")
    source_guard()
    output = output_directory(args.output)
    guard_private_output(output)
    state_path = output / "state.json"
    old_state_sha = file_hash(state_path) if state_path.exists() else None
    state = json.loads(state_path.read_text(encoding="utf-8")) if state_path.exists() else None
    if args.action == "status":
        require(args.protected_pids and args.protected_pids_sha256, "explicit_status_role_pids_required")
        role_file, _ = read_hashed_json(args.protected_pids, args.protected_pids_sha256, 65536)
        roles, protected = role_pids(role_file.get("roles")), None
    else:
        require(args.protected_proof and args.protected_proof_sha256, "fresh_protected_proof_required_before_remote_mutation")
        proof_file, _ = read_hashed_json(args.protected_proof, args.protected_proof_sha256)
        protected = proof_file.get("protectedProof", proof_file)
        roles = validate_protected_proof(protected, args.expected_pid)
    if args.action == "prepare":
        require(state is None and args.deny_old_binary_rollback, "fresh_successor_state_and_denied_rollback_required")
        require(args.predecessor_state and args.predecessor_state_sha256 and args.artifact_proof and args.artifact_proof_sha256 and args.artifact_dir,
                "complete_explicit_predecessor_and_artifact_inputs_required")
        old_state, descriptor = read_hashed_json(args.predecessor_state, args.predecessor_state_sha256)
        predecessor = predecessor_excerpt(old_state, descriptor)
        proof, proof_descriptor = read_hashed_json(args.artifact_proof, args.artifact_proof_sha256)
        validate_artifact_proof(proof)
        require(Path(args.artifact_dir).resolve() == Path(proof["artifactDirectory"]).resolve(), "artifact_directory_not_receipt_bound")
        candidate, maps, artifact = inspect_artifact(args.artifact_dir, CANDIDATE_REVISION, args.artifact_proof, args.artifact_proof_sha256, True)
        require(candidate["binarySha256"] == proof["binarySha256"] and candidate["binarySizeBytes"] == proof["binarySizeBytes"]
                and candidate["manifestSha256"] == proof["releaseManifestSha256"], "actual_release_not_strict_proof_bound")
        scene_pins = {"predecessorState": descriptor, "artifactProof": proof_descriptor, "baseSourceSha256": V5_BASE_SHA256,
                      "protectedFingerprintSha256": protected["stableFingerprintSha256"]}
    elif args.action != "status":
        require(isinstance(state, dict) and isinstance(state.get("sceneV6Pins"), dict), "prepared_scene_successor_required")
        predecessor, proof, scene_pins = state["predecessor"], state["strictArtifactProof"], state["sceneV6Pins"]
        read_hashed_json(scene_pins["predecessorState"]["path"], scene_pins["predecessorState"]["sha256"])
        actual_proof, _ = read_hashed_json(scene_pins["artifactProof"]["path"], scene_pins["artifactProof"]["sha256"])
        require(actual_proof == proof, "prepared_strict_artifact_proof_changed")
        validate_artifact_proof(proof)
        require(protected["stableFingerprintSha256"] == scene_pins["protectedFingerprintSha256"], "protected_baseline_changed")
        candidate, maps, artifact = state["candidate"], state["mapsEvidence"], state["artifact"]
        if args.action == "install":
            actual = inspect_artifact(artifact["directory"], CANDIDATE_REVISION, maps["path"], maps["sha256"], True)
            require(actual == (candidate, maps, artifact), "prepared_artifact_or_maps_changed")
            require(candidate["binarySha256"] == proof["binarySha256"] and candidate["manifestSha256"] == proof["releaseManifestSha256"],
                    "strict_artifact_binary_changed")
    else:
        predecessor = proof = scene_pins = candidate = maps = artifact = None
    config = {"expectedPid": args.expected_pid, "oldPins": {"revision": OLD_REVISION, "binarySha256": OLD_BINARY_SHA256},
              "candidate": candidate, "mapsEvidence": maps, "artifact": artifact, "toolSha256": file_hash(__file__),
              "baseSourceSha256": V5_BASE_SHA256, "oldBinaryRollbackAllowed": False, "protectedRoles": roles,
              "protectedProof": protected, "predecessor": predecessor, "strictArtifactProof": proof,
              "sceneV6Pins": scene_pins, "checkPredecessor": args.action == "prepare"}
    lock = output / ".transaction.lock"
    private_write_new(lock, {"pid": os.getpid(), "createdAtUtc": datetime.now(timezone.utc).isoformat()})
    lock_sha, client, watchdog = file_hash(lock), None, None
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    try:
        private_write_new(output / (args.action + "-intent-" + stamp + ".json"), {**config, "action": args.action, "priorStateSha256": old_state_sha})
        sys.path.insert(0, str(Path(args.paramiko_runtime).expanduser().resolve()))
        import paramiko
        client = connect_pinned(paramiko)
        # Hard close also bounds exec_command's internal waits.
        watchdog = threading.Timer(500, client.close); watchdog.daemon = True; watchdog.start()
        try:
            result = ssh_call(client, None, args.action, config, state,
                              str(Path(artifact["directory"]) / "mir2-gateway") if args.action == "install" else None)
        except Exception as error:
            result = {"schema": SCHEMA, "action": args.action, "passed": False, "remoteOutcomeUnknown": True,
                      "error": str(error) if isinstance(error, Refusal) else type(error).__name__,
                      "operatorReadOnlyReconciliationRequired": True}
        result.update(toolSha256=config["toolSha256"], baseSourceSha256=V5_BASE_SHA256,
                      observedAtUtc=datetime.now(timezone.utc).isoformat(), credentialsPersisted=False)
        receipt = output / (args.action + "-" + stamp + ".json")
        private_write_new(receipt, result)
        if args.action != "status" and result.get("state") is not None:
            require((file_hash(state_path) if state_path.exists() else None) == old_state_sha, "local_state_cas_changed")
            private_write_new(output / ("state-" + stamp + ".json"), result["state"])
            replacement = output / (".state-" + stamp + ".tmp")
            private_write_new(replacement, result["state"]); os.replace(replacement, state_path)
        print(json.dumps({"passed": result["passed"], "receipt": str(receipt), "phase": result.get("state", {}).get("phase"), "error": result.get("error")}))
        return 0 if result["passed"] else 2
    finally:
        if watchdog is not None: watchdog.cancel()
        if client is not None: client.close()
        if lock.exists() and file_hash(lock) == lock_sha: lock.unlink()


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--remote-payload":
        remote_main(sys.argv[2])
    else:
        try:
            raise SystemExit(main())
        except Exception as error:
            print(json.dumps({"passed": False, "error": str(error) if isinstance(error, Refusal) else type(error).__name__}))
            raise SystemExit(2)


