#!/usr/bin/env python3
"""One candidate, TEST Gateway only: guarded release/capacity transactions.

No remote operation occurs on import. See cloud_test_release.README.md before use.
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
RELEASE_SUFFIX = "-equip-logout-20261112"
RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 34 + "-equip-logout-20261112.conf"
RELEASE_ENV = "/etc/mir2-playtest-equip-logout-20261112.env"
CAPS_DROPIN = "/run/systemd/system/mir2-playtest.service.d/" + "z" * 36 + "-equip-logout-capacity-20261112.conf"
CAPS_ENV = "/run/mir2-playtest-equip-logout-capacity-20261112.env"
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


def validate(snapshot, state, phase=None, drained=True, identity=None, transition=False):
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


def transaction(action, host, config, state=None):
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
            require(len(before["originalEnvironmentFiles"]) == 13
                    and len({entry["path"] for entry in before["originalEnvironmentFiles"]}) == 13,
                    "expected_thirteen_original_environment_files")
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
    encoded = base64.b64encode(json.dumps(payload).encode()).decode()
    source = Path(__file__).read_text(encoding="utf-8")
    compressed = base64.b64encode(zlib.compress(source.encode())).decode()
    bootstrap = "import base64,zlib;exec(zlib.decompress(base64.b64decode(" + repr(compressed) + ")))"
    prompt = ("__MIR2_TEST_RELEASE_SUDO_PROMPT_" + uuid.uuid4().hex + "__").encode("ascii")
    stdin, stdout, stderr = client.exec_command("sudo -S -p " + shlex.quote(prompt.decode("ascii")) + " python3 -u -c " + shlex.quote(bootstrap) +
                                              " --remote-payload " + shlex.quote(encoded), timeout=480)
    channel = stdout.channel
    require(stdin.channel is channel and stderr.channel is channel, "ssh_rpc_streams_do_not_share_channel")
    # Never pre-write a password: cached/NOPASSWD sudo leaves stdin for Python.
    # Poll both SSH streams instead of blocking on stdout while sudo prompts on
    # stderr. Only this invocation's complete marker may authorize one password.
    pending_stdout, pending_stderr = bytearray(), bytearray()
    password_sent = rpc_started = artifact_sent = False
    receipt = None
    stdout_bytes = stderr_bytes = 0
    deadline = time.monotonic() + 480

    def check_time():
        require(time.monotonic() < deadline, "ssh_rpc_timeout_no_retry")

    def read_stderr():
        nonlocal password_sent, stderr_bytes
        while channel.recv_stderr_ready():
            block = channel.recv_stderr(4096)
            require(block, "ssh_stderr_stream_closed")
            stderr_bytes += len(block)
            require(stderr_bytes <= 16384, "sudo_stderr_unbounded")
            pending_stderr.extend(block)
        if not pending_stderr:
            return
        if len(pending_stderr) < len(prompt):
            require(prompt.startswith(pending_stderr), "unexpected_sudo_stderr_no_retry")
            return
        require(bytes(pending_stderr) == prompt, "unexpected_sudo_stderr_no_retry")
        require(not password_sent and not rpc_started, "repeated_or_late_sudo_prompt_no_retry")
        # A queued/partial stdout frame can already be Python's root-start ack.
        # Inspect it first; never authorize a password after stdout has begun.
        if channel.recv_ready():
            return
        require(stdout_bytes == 0 and not pending_stdout, "sudo_prompt_after_stdout_no_retry")
        require(isinstance(password, str) and not any(c in password for c in ["\n", "\r", "\0"]), "invalid_password_input")
        check_time()
        require(not channel.closed and not getattr(channel, "eof_received", False) and not channel.exit_status_ready(),
                "ssh_closed_before_sudo_response")
        channel.sendall((password + "\n").encode("utf-8"))
        password_sent = True
        pending_stderr.clear()

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


def remote_main(encoded):
    config = json.loads(base64.b64decode(encoded))
    require(os.geteuid() == 0, "root_required")
    # A flushed ack proves sudo already exec'd Python. After it, stdin is reserved
    # exclusively for the advertised binary body; a sudo prompt is always rejected.
    print(json.dumps({"rpcStarted": True}), flush=True)
    # Avoid creating remotely persisted source/password files.
    def interrupted(number, frame):
        raise Refusal("controller_signal_interrupted")
    for kind in [signal.SIGHUP, signal.SIGTERM, signal.SIGINT]:
        signal.signal(kind, interrupted)
    result = transaction(config["action"], NativeHost(config), config, config.get("state"))
    print(json.dumps(result, separators=(",", ":")), flush=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--action", required=True, choices=["prepare", "install", "apply-caps", "restore-caps", "rollback"])
    parser.add_argument("--output", required=True, help="Private outside-repository directory for this one candidate")
    parser.add_argument("--expected-pid", required=True, type=int)
    parser.add_argument("--old-revision")
    parser.add_argument("--old-binary-sha256")
    parser.add_argument("--candidate-revision")
    parser.add_argument("--artifact-dir", help="Verified extracted Actions Gateway artifact, not the archive")
    parser.add_argument("--maps-evidence")
    parser.add_argument("--maps-evidence-sha256")
    parser.add_argument("--confirm-maps-match", action="store_true")
    parser.add_argument("--deny-old-binary-rollback", action="store_true",
                        help="Prepare only: permanently deny this transaction's old-binary downgrade, including fault recovery")
    parser.add_argument("--paramiko-runtime", default=str(Path.home() / ".codex/tmp/mir2-deploy-ssh-runtime"))
    parser.add_argument("--host-key-sha256", help="Optional explicit SHA256:base64 host-key pin; otherwise known_hosts is required")
    args = parser.parse_args(argv)
    require(args.expected_pid > 1, "explicit_current_expected_pid_required")
    require(not args.deny_old_binary_rollback or args.action == "prepare", "rollback_policy_flag_is_prepare_only")
    output = output_directory(args.output)
    lock = output / ".transaction.lock"
    token = {"pid": os.getpid(), "createdAtUtc": datetime.now(timezone.utc).isoformat()}
    private_write_new(lock, token)
    lock_sha = file_hash(lock)
    client, password = None, None
    try:
        state_path = output / "state.json"
        old_state_sha = file_hash(state_path) if state_path.exists() else None
        state = json.loads(state_path.read_text(encoding="utf-8")) if state_path.exists() else None
        tool_sha = file_hash(__file__)
        if args.action == "prepare":
            require(state is None, "one_candidate_directory_already_initialized")
            require(re.fullmatch(r"[0-9a-f]{40}", args.old_revision or "") and re.fullmatch(r"[0-9a-f]{64}", args.old_binary_sha256 or ""), "explicit_old_revision_and_binary_pin_required")
            require(args.artifact_dir and args.maps_evidence, "artifact_and_matching_maps_evidence_required")
            candidate, maps, artifact = inspect_artifact(args.artifact_dir, args.candidate_revision, args.maps_evidence, args.maps_evidence_sha256, args.confirm_maps_match)
            require(candidate["revision"] != args.old_revision and candidate["binarySha256"] != args.old_binary_sha256, "candidate_must_differ_from_original")
            old_pins = {"revision": args.old_revision, "binarySha256": args.old_binary_sha256}
            rollback_allowed = not args.deny_old_binary_rollback
        else:
            require(state is not None, "prepare_first_in_same_private_output_directory")
            candidate, maps, artifact, old_pins = state["candidate"], state["mapsEvidence"], state["artifact"], state["oldPins"]
            rollback_allowed = state.get("oldBinaryRollbackAllowed")
            require(type(rollback_allowed) is bool, "explicit_boolean_rollback_policy_required")
            if args.action == "rollback":
                require(rollback_allowed is True, "old_binary_rollback_denied_or_unproven")
            if args.action == "install":
                actual = inspect_artifact(artifact["directory"], candidate["revision"], maps["path"], maps["sha256"], True)
                require(actual == (candidate, maps, artifact), "prepared_artifact_or_maps_evidence_changed")
        config = {"expectedPid": args.expected_pid, "oldPins": old_pins, "candidate": candidate,
                  "mapsEvidence": maps, "artifact": artifact, "toolSha256": tool_sha,
                  "oldBinaryRollbackAllowed": rollback_allowed}
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        # Durable local intent is written before connecting or requesting mutations.
        private_write_new(output / (args.action + "-intent-" + stamp + ".json"), {**config, "action": args.action, "priorStateSha256": old_state_sha})
        sys.path.insert(0, str(Path(args.paramiko_runtime).expanduser().resolve()))
        import paramiko
        client = paramiko.SSHClient()
        client.load_system_host_keys()
        if args.host_key_sha256:
            class PinnedPolicy(paramiko.MissingHostKeyPolicy):
                def missing_host_key(self, ssh, hostname, key):
                    actual = "SHA256:" + base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip("=")
                    require(actual == args.host_key_sha256, "ssh_host_key_pin_mismatch")
            client.set_missing_host_key_policy(PinnedPolicy())
        else:
            client.set_missing_host_key_policy(paramiko.RejectPolicy())
        password = getpass.getpass("SSH password (memory only): ")
        client.connect(HOST, username="ubuntu", password=password, look_for_keys=False, allow_agent=False,
                       timeout=20, auth_timeout=20, banner_timeout=20)
        if args.host_key_sha256:
            actual = "SHA256:" + base64.b64encode(hashlib.sha256(client.get_transport().get_remote_server_key().asbytes()).digest()).decode().rstrip("=")
            require(actual == args.host_key_sha256, "ssh_host_key_pin_mismatch")
        try:
            result = ssh_call(client, password, args.action, config, state,
                              str(Path(artifact["directory"]) / "mir2-gateway") if args.action == "install" else None)
        except Exception as error:
            # A lost transport does not prove the remote state. Never blindly retry,
            # stop an unknown activation, or manufacture an apply/rollback receipt.
            result = {"schema": SCHEMA, "action": args.action, "passed": False, "remoteOutcomeUnknown": True,
                      "error": str(error) if isinstance(error, Refusal) else type(error).__name__,
                      "operatorReadOnlyReconciliationRequired": True}
        result.update(toolSha256=tool_sha, observedAtUtc=datetime.now(timezone.utc).isoformat(), credentialsPersisted=False)
        receipt = output / (args.action + "-" + stamp + ".json")
        private_write_new(receipt, result)
        if result.get("state") is not None:
            require((file_hash(state_path) if state_path.exists() else None) == old_state_sha, "local_state_cas_changed")
            next_path = output / ("state-" + stamp + ".json")
            private_write_new(next_path, result["state"])
            replacement = output / (".state-" + stamp + ".tmp")
            private_write_new(replacement, result["state"])
            os.replace(replacement, state_path)
        print(json.dumps({"passed": result["passed"], "receipt": str(receipt), "phase": result.get("state", {}).get("phase"),
                          "error": result.get("error"), "rollback": result.get("rollback", {}).get("passed")}))
        return 0 if result["passed"] else 2
    finally:
        if client is not None:
            client.close()
        password = None
        if lock.exists() and file_hash(lock) == lock_sha:
            lock.unlink()


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--remote-payload":
        remote_main(sys.argv[2])
    else:
        raise SystemExit(main())
