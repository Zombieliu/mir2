"""Fixed temporary trace pair on the existing TEST unit; no release/caps action."""
import argparse
import base64
import copy
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import stat
import sys
import threading
import time
import zlib

SCHEMA = "mir2.cloud8f-trace-control.v1"
PROOF_SCHEMA = "mir2.cloud8f-trace-control-status.v1"
BASE_SHA = "ba1e3075d6398fbbe7c6c68d988be70200d78b0618b868ab02f1403ce52e2ac2"
BASELINE_SHA = "b6e0ceb35fe42e2eee2179c5fa825c83238ccec0820120eb66b360f9b881789d"
MODEL_SHA = "27da5431a7fde5592d5a528efddb7349e23ad72c5e9eebe635102caf76120c19"
INSTALLED_STATE_SHA = "4004aa19ba177bf81a2e79e8b87d78319ef34c90f029db073db8b5ec28c161bb"
REVISION = "8f3099f4a0114a7825ff61db36b548d3bfb5b3fa"
BINARY = "edd9f48cdb36ddd96334598aca4abc2f2e212e46ef2cbb60d4a1f5ab15a7ded5"
NONTRACE_ENV_SHA = "7a57bfb2e947864837166dd7d4acbfb3ff75973d06ccf4f73a3372a04ac5e52d"
INITIAL = {"pid": 1159915, "invocationId": "7fc1d202504a4cbf82df22b5dc6080ed", "startTicks": 1551513392}
TRACE_ENV = "/run/mir2-playtest-zone-delivery-trace-20261114-v1.env"
TRACE_DROPIN = "/run/systemd/system/mir2-playtest.service.d/" + "z" * 62 + "-zone-delivery-trace-20261114-v1.conf"
OWN_PATHS = (TRACE_ENV, TRACE_DROPIN)
TRACE_KEYS = ("MIR2_GATEWAY_ZONE_DELIVERY_TRACE", "MIR2_GATEWAY_ZONE_DELIVERY_TRACE_RECIPIENTS")
RUNTIME_KEYS = ("INVOCATION_ID", "JOURNAL_STREAM", "SYSTEMD_EXEC_PID")
RECIPIENTS = "50006,50012"
CONTENTS = {TRACE_ENV: TRACE_KEYS[0] + "=1\n" + TRACE_KEYS[1] + "=" + RECIPIENTS + "\n",
            TRACE_DROPIN: "[Service]\nEnvironmentFile=" + TRACE_ENV + "\n"}
PLAN = {path: {"sha256": hashlib.sha256(content.encode()).hexdigest(), "bytes": len(content.encode())} for path, content in CONTENTS.items()}


class Refusal(ValueError): pass


def need(value, code):
    if not value: raise Refusal(code)


def sha(raw): return hashlib.sha256(raw).hexdigest()


def jhash(value): return sha(json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode())


def model(raw):
    value = {key: raw[key] for key in ["snapshot", "environment", "typedNineZero", "currentV7PairProtected", "ownedTracePaths"]}
    need(jhash(value) == MODEL_SHA, "historical_public_baseline_changed")
    return value


def normalized(snapshot):
    keys = ["binarySha256", "executablePath", "configuredExecutablePath", "processWorkingDirectory", "argvTailSha256", "execStartSha256",
            "protectedConfig", "originalEnvironmentFiles", "originalDropIns", "environmentRevision", "environmentCaps",
            "protectedProcesses", "legacyProtectedProcesses", "stopScope", "zoneIdentity"]
    result = {key: snapshot[key] for key in keys}
    result["oldGatewayIdentity"] = snapshot.get("oldGatewayIdentity", snapshot.get("oldGateway", {}).get("identity"))
    return result


def trace_flags(environment):
    result = {}
    for key in TRACE_KEYS:
        if key not in environment: result[key] = {"state": "unset"}; continue
        value = environment[key]
        if key == TRACE_KEYS[0] and value in ["0", "1"]: result[key] = {"state": "value", "value": value}
        elif key == TRACE_KEYS[1] and re.fullmatch(r"[0-9]{1,10}(,[0-9]{1,10}){0,255}", value) and all(int(x) <= 4294967295 for x in value.split(",")):
            result[key] = {"state": "numeric-allowlist", "ids": sorted(set(map(int, value.split(","))))}
        else: result[key] = {"state": "unexpected-hash", "valueSha256": sha(value.encode())}
    return result


BASE_FLAGS = {key: {"state": "unset"} for key in TRACE_KEYS}
ACTIVE_FLAGS = trace_flags({TRACE_KEYS[0]: "1", TRACE_KEYS[1]: RECIPIENTS})


def validate_active(view, historical, empty=True):
    need(normalized(view) == normalized(historical["snapshot"]), "historical_configuration_or_protected_world_changed")
    need(view["health"]["revision"] == REVISION and view["binarySha256"] == BINARY, "fixed_release_changed")
    need(view["nonTraceEnvironmentSha256"] == NONTRACE_ENV_SHA, "nontrace_environment_changed")
    need(isinstance(view["health"]["capacity"], dict), "capacity_object_required")
    envs, drops = view["environmentFiles"], view["dropIns"]
    own = view["ownedFiles"]
    expected_env = historical["snapshot"]["environmentFiles"] + ([{"path": TRACE_ENV, "ignoreErrors": False}] if TRACE_ENV in own else [])
    expected_drop = historical["snapshot"]["dropIns"] + ([TRACE_DROPIN] if TRACE_DROPIN in own else [])
    need(envs == expected_env and drops == expected_drop, "trace_environment_precedence_changed")
    if empty: need(view["nineZero"] is True, "typed_ninezero_required")


def validate_proof(proof, config, state, now_ms=None):
    now_ms = time.time_ns() // 1_000_000 if now_ms is None else now_ms
    need(isinstance(proof, dict) and proof.get("schema") == PROOF_SCHEMA and proof.get("readonly") is True,
         "fresh_trace_status_required")
    need(proof.get("operatorSha256") == config["operatorSha256"] and proof.get("historicalModelSha256") == MODEL_SHA, "fresh_status_source_changed")
    need(type(proof.get("sampledAtUnixMs")) is int and 0 <= now_ms - proof["sampledAtUnixMs"] <= 300000, "fresh_status_expired")
    need(proof.get("expectedPid") == config["expectedPid"] and proof.get("stateSha256") == (jhash(state) if state is not None else None), "fresh_status_epoch_or_state_changed")
    need(proof.get("fingerprintSha256") == jhash(proof["view"]), "fresh_status_fingerprint_changed")
    return proof["view"]


def validate_state(state, config):
    need(isinstance(state, dict) and state.get("schema") == SCHEMA and state.get("operatorSha256") == config["operatorSha256"], "trace_state_source_changed")
    need(state.get("historicalModelSha256") == MODEL_SHA and state.get("installedStateSha256") == INSTALLED_STATE_SHA and state.get("plan") == PLAN,
         "trace_state_authority_changed")
    need(state.get("remoteOutcomeUnknown") is not True, "unknown_remote_outcome_no_blind_retry")
    need(set(state.get("ownedFiles", {})) <= set(OWN_PATHS), "owned_scope_changed")


def run(action, host, config, state=None):
    need(action in ["status", "prepare", "apply", "restore"], "fixed_trace_actions_only")
    historical = config["historical"]
    need(jhash(historical) == MODEL_SHA and config.get("installedStateSha256") == INSTALLED_STATE_SHA, "fixed_historical_authority_required")
    out = {"schema": SCHEMA, "action": action, "passed": False, "unit": "mir2-playtest.service", "serviceOperations": [],
           "capacityQualified": False, "binaryChanged": False, "oldBinaryRollback": False}
    if action == "status":
        view = host.view(state)
        need(view["unitPid"] == config["expectedPid"], "caller_pid_changed")
        if view["active"]: validate_active(view, historical, empty=False)
        proof = {"schema": PROOF_SCHEMA, "readonly": True, "operatorSha256": config["operatorSha256"],
                 "historicalModelSha256": MODEL_SHA, "sampledAtUnixMs": time.time_ns() // 1_000_000,
                 "expectedPid": config["expectedPid"], "stateSha256": jhash(state) if state is not None else None,
                 "view": view, "fingerprintSha256": jhash(view)}
        return {**out, "passed": True, "readonly": True, "proof": proof, "stateUnchanged": True,
                "currentPhase": state.get("phase") if state is not None else "unprepared",
                "knownActivationMatches": state is None or view.get("identity") == state.get("knownActivation"),
                "ownedFilesMatchState": view["ownedFiles"] == (state["ownedFiles"] if state is not None else {})}
    if state is not None: validate_state(state, config)
    before = validate_proof(config.get("freshStatus"), config, state)
    need(host.view(state) == before, "immediate_status_or_ownership_changed")
    out["before"] = before
    if action == "prepare":
        need(state is None and before["active"] and before["identity"] == INITIAL and before["ownedFiles"] == {}, "new_trace_prepare_required")
        validate_active(before, historical)
        need(before["traceFlags"] == BASE_FLAGS, "preexisting_trace_flags_not_owned")
        need(all(Path(TRACE_DROPIN).name > Path(path).name for path in before["dropIns"]), "trace_dropin_must_be_last")
        state = {"schema": SCHEMA, "phase": "prepared", "operatorSha256": config["operatorSha256"], "historicalModelSha256": MODEL_SHA,
                 "installedStateSha256": INSTALLED_STATE_SHA, "knownActivation": before["identity"], "ownedFiles": {}, "plan": PLAN,
                 "stopProof": None, "remoteOutcomeUnknown": False, "installedV7StateIsHistorical": False,
                 "history": [{"action": "prepare", "viewSha256": jhash(before)}]}
        return {**out, "passed": True, "state": state, "after": before}
    need(state is not None, "prepare_required")
    state = copy.deepcopy(state)
    need(before["ownedFiles"] == state["ownedFiles"], "owned_file_inode_or_hash_changed")
    if before["active"]:
        need(before["identity"] == state["knownActivation"], "known_activation_changed")
        validate_active(before, historical)
    else:
        need(action == "restore" and state.get("stopProof") and before.get("controlledInactive") is True, "owned_normal_stop_required")
    if action == "apply":
        need(state["phase"] == "prepared" and before["ownedFiles"] == {} and before["traceFlags"] == BASE_FLAGS, "trace_apply_not_repeatable")
    else:
        need(state["phase"] in ["prepared", "applied", "apply-failed", "restore-failed"], "trace_restore_phase_required")
        if not state["ownedFiles"] and before["active"] and before["traceFlags"] == BASE_FLAGS:
            state.update(phase="restored", stopProof=None, terminalSnapshot=before,
                         futureControlRequiresSuccessorChain=state["installedV7StateIsHistorical"])
            return {**out, "passed": True, "state": state, "after": before, "noServiceMutationNeeded": True}
    mutation = False
    try:
        if before["active"]:
            mutation = True
            state["stopProof"] = host.stop(state["knownActivation"])
            out["serviceOperations"].append("normal-stop-TEST")
        host.inactive(state)
        if action == "apply": host.create(state)
        else: host.remove(state)
        host.inactive(state)
        host.reload(state); out["serviceOperations"].append("daemon-reload")
        host.inactive(state)
        after_identity = host.start(state)
        state["knownActivation"] = after_identity
        state["installedV7StateIsHistorical"] = True
        out["serviceOperations"].append("normal-start-TEST")
        after = host.view(state); validate_active(after, historical)
        need(after["identity"] == after_identity, "first_started_activation_changed")
        need(after["ownedFiles"] == state["ownedFiles"], "owned_after_start_changed")
        need(after["traceFlags"] == (ACTIVE_FLAGS if action == "apply" else BASE_FLAGS), "trace_flags_not_effective")
        state.update(phase="applied" if action == "apply" else "restored", stopProof=None)
        if action == "restore": state.update(terminalSnapshot=after, futureControlRequiresSuccessorChain=True)
        state["history"].append({"action": action, "viewSha256": jhash(after), "identity": after_identity})
        out.update(passed=True, after=after, state=state)
    except Exception as error:
        state["phase"] = action + "-failed"
        state["failure"] = {"action": action, "error": str(error) if isinstance(error, Refusal) else type(error).__name__, "serviceMutationBegan": mutation}
        state["ownedFiles"].update(getattr(host, "created_files", {}))
        if state.get("stopProof") is None and getattr(host, "normal_stop_proof", None) is not None:
            state["stopProof"] = host.normal_stop_proof
        if getattr(host, "last_started_identity", None) is not None:
            state["knownActivation"] = host.last_started_identity; state["installedV7StateIsHistorical"] = True
        out.update(error=state["failure"]["error"], state=state, restoreRequiresFreshStatus=True, automaticRollbackAttempted=False)
    return out


class TraceHost:
    def __init__(self, B, config):
        self.B, self.config = B, config
        # B functions close over their own module dictionary; change only that
        # isolated loaded module, never the installed V10 or release constants.
        B.NativeHost.config_projection.__globals__["OWN_PATHS"] = list(OWN_PATHS)
        self.native = B.SceneNativeHost({"protectedRoles": [{"role": r["role"], "pid": r["pid"]}
            for r in config["historical"]["snapshot"]["protectedProcesses"]], "protectedProof": None})
        self.created_files = {}; self.last_started_identity = None; self.normal_stop_proof = None

    def owned(self):
        return {path: self.B.metadata(path) for path in OWN_PATHS if os.path.lexists(path)}

    def environment(self, pid):
        raw = Path("/proc", str(pid), "environ").read_bytes(); need(len(raw) <= 1048576, "environment_bound")
        env = {}
        for entry in raw.split(b"\0"):
            if not entry: continue
            key, value = entry.decode("utf8", "strict").split("=", 1)
            need(key not in env, "duplicate_environment_key"); env[key] = value
        return trace_flags(env), jhash({k: v for k, v in env.items() if k not in TRACE_KEYS + RUNTIME_KEYS})

    def view(self, state=None):
        unit = self.B.show(self.B.UNIT); pid = int(unit.get("MainPID") or 0)
        if pid == 0:
            need(state and state.get("stopProof"), "inactive_without_owned_stop")
            self.inactive(state)
            return {"active": False, "unitPid": 0, "identity": None, "controlledInactive": True, "ownedFiles": self.owned(),
                    "stopProof": state["stopProof"], "nineZero": None}
        snapshot = self.native.snapshot(); self.B.basic(snapshot)
        need(pid == snapshot["identity"]["pid"], "snapshot_unit_epoch_changed")
        flags, environment_hash = self.environment(snapshot["identity"]["pid"])
        need(self.B.process_identity(self.B.show(self.B.UNIT)) == snapshot["identity"], "environment_epoch_changed")
        zero = True
        try: self.B.empty(snapshot)
        except self.B.Refusal: zero = False
        snapshot.update(active=True, unitPid=pid, traceFlags=flags, nonTraceEnvironmentSha256=environment_hash, ownedFiles=self.owned(), nineZero=zero)
        return snapshot

    def stop(self, identity):
        self.native.config["protectedProof"] = {"targetIdentity": identity, **self.B.fence_payload(self.config["historical"]["snapshot"])}
        globals_ = self.B.NativeHost.stop.__globals__
        original = globals_["command"]
        def guarded_command(args):
            if args == ["systemctl", "--no-block", "stop", self.B.UNIT]:
                need(self.B.process_identity(self.B.show(self.B.UNIT)) == identity, "last_signal_epoch_changed")
                health = self.B.health(7210); self.B.empty({"health": health})
                need(health["revision"] == REVISION and {k: health["capacity"].get(k) for k in self.B.OLD_CAPS} == self.B.OLD_CAPS,
                     "last_signal_release_or_caps_changed")
            return original(args)
        globals_["command"] = guarded_command
        try: return self.native.stop(identity)
        finally:
            globals_["command"] = original
            self.normal_stop_proof = self.native.normal_stop_proof

    def cgroup_empty(self):
        root = Path("/sys/fs/cgroup" + self.B.TARGET_CGROUP)
        if not root.exists(): return
        need((root / "cgroup.procs").is_file(), "cgroup_process_file_missing")
        count = 0
        for current, dirs, files in os.walk(root, followlinks=False):
            count += 1; need(count <= 128 and len(dirs) <= 128, "cgroup_subtree_bound")
            need(not any((Path(current) / name).is_symlink() for name in dirs), "cgroup_subtree_linked")
            if "cgroup.procs" in files:
                raw = (Path(current) / "cgroup.procs").read_bytes()
                need(len(raw) <= 16384 and raw.strip() == b"", "test_cgroup_not_empty")
        need(count > 0, "cgroup_subtree_not_observed")

    def inactive(self, state):
        proof = state["stopProof"]; need(proof is not None, "normal_stop_proof_required")
        self.native.normal_stop_proof = proof
        self.native.config["protectedProof"] = {"targetIdentity": proof["identity"], **self.B.fence_payload(self.config["historical"]["snapshot"])}
        self.native._guard(proof)
        unit = self.B.show(self.B.UNIT)
        need(unit.get("MainPID") == "0" and unit.get("ActiveState") == "inactive" and
             {k: unit.get(k) for k in proof["inactiveObservation"]} == proof["inactiveObservation"], "inactive_activation_changed")
        executable, argv, exec_sha = self.B.parse_exec_start(unit.pop("ExecStart"))
        projection = self.native.config_projection(unit); baseline = self.config["historical"]["snapshot"]
        need(projection["protectedConfig"] == baseline["protectedConfig"] and projection["originalEnvironmentFiles"] == baseline["originalEnvironmentFiles"] and
             projection["originalDropIns"] == baseline["originalDropIns"], "inactive_protected_configuration_changed")
        need(executable == baseline["configuredExecutablePath"] and self.B.json_hash(argv[1:]) == baseline["argvTailSha256"] and
             exec_sha == baseline["execStartSha256"], "inactive_executable_or_argv_changed")
        old, zone = self.native.peers()
        need(old["identity"] == baseline["oldGatewayIdentity"] and old["health"].get("ok") is True and
             old["health"].get("http") == "ready" and old["health"].get("ws") == "ready" and
             old["health"]["sessionCache"].get("healthy") is True and zone == baseline["zoneIdentity"], "inactive_legacy_world_changed_or_unhealthy")
        need(self.owned() == state["ownedFiles"], "inactive_owned_cas_changed")
        self.cgroup_empty(); self.native._guard(proof)

    def create(self, state):
        for path in OWN_PATHS:
            self.native.root_directory(Path(path).parent, create=True)
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            first = os.fstat(descriptor)
            try:
                with os.fdopen(descriptor, "wb") as output: output.write(CONTENTS[path].encode()); output.flush(); os.fsync(output.fileno())
                item = self.B.metadata(path)
                need(item["uid"] == 0 and item["mode"] == 0o600 and item["sha256"] == PLAN[path]["sha256"] and item["bytes"] == PLAN[path]["bytes"], "owned_trace_write_changed")
                state["ownedFiles"][path] = item; self.created_files[path] = item
            except Exception:
                try:
                    now = Path(path).lstat()
                    if (now.st_dev, now.st_ino, now.st_uid, stat.S_IMODE(now.st_mode)) == (first.st_dev, first.st_ino, 0, 0o600):
                        need(stat.S_ISREG(now.st_mode) and CONTENTS[path].encode().startswith(Path(path).read_bytes()), "partial_trace_write_foreign")
                        os.unlink(path)
                except FileNotFoundError: pass
                raise

    def remove(self, state):
        need(self.owned() == state["ownedFiles"], "restore_owned_cas_changed")
        for path in [TRACE_DROPIN, TRACE_ENV]:
            if path not in state["ownedFiles"]: continue
            item = self.B.metadata(path)
            need(item == state["ownedFiles"][path] and item["uid"] == 0 and item["mode"] == 0o600 and
                 item["sha256"] == PLAN[path]["sha256"] and item["bytes"] == PLAN[path]["bytes"], "restore_trace_inode_or_hash_changed")
            os.unlink(path); state["ownedFiles"].pop(path)

    def reload(self, state): self.native.reload()

    def start(self, state):
        try: self.native.start()
        finally: self.last_started_identity = self.native.last_started_identity
        need(self.last_started_identity is not None, "new_activation_not_observed")
        return self.last_started_identity


def load_base(path):
    path = Path(path)
    need(sha(path.read_bytes()) == BASE_SHA, "frozen_v10_source_changed")
    spec = importlib.util.spec_from_file_location("isolated_trace_v10_primitives", path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def source_guard(B):
    here = Path(__file__).resolve()
    need(here.name == "cloud_test_delivery_trace_v1.py" and here.parent.name == "cloud-reference" and
         here.parents[3].name == "mir2-web3" and here.parent == here.parents[3] / "scripts/capacity/cloud-reference", "canonical_trace_source_required")
    B.source_guard()


def installed_pin(B, path):
    need(B.guard_local_state(path), "installed_v7_state_required")
    p = Path(path).resolve()
    need(p.name == "state.json" and p.parent.name == "release-scene-v7" and p.parent.parent.name == "private", "fixed_installed_v7_state_path_required")
    need(0 < p.stat().st_size <= 4 * 2**20 and B.file_hash(p) == INSTALLED_STATE_SHA, "immutable_installed_v7_state_changed")
    return {"path": str(p), "sha256": INSTALLED_STATE_SHA, "bytes": p.stat().st_size}


def remote_entry(B, encoded, operator_sha):
    config = B.decode_remote_payload(encoded)
    need(config.get("operatorSha256") == operator_sha and type(config.get("expectedPid")) is int and config["expectedPid"] >= 0,
         "remote_source_or_caller_identity_changed")
    state = config.pop("state", None)
    host = TraceHost(B, config)
    try: result = run(config["action"], host, config, state)
    except Exception as error:
        result = {"schema": SCHEMA, "action": config.get("action"), "passed": False,
                  "error": str(error) if isinstance(error, (Refusal, B.Refusal)) else type(error).__name__,
                  "serviceMutationBegan": False, "capacityQualified": False}
    print(json.dumps(result, separators=(",", ":"), allow_nan=False), flush=True)


def ssh_request(B, client, config, state):
    payload = {**config, "state": state}
    encoded = B.encode_remote_payload(payload)
    own = Path(__file__).read_bytes()
    need(sha(own) == config["operatorSha256"], "operator_source_changed_before_rpc")
    base = Path(B.__file__).read_bytes(); need(sha(base) == BASE_SHA, "base_source_changed_before_rpc")
    packed_base = base64.b64encode(zlib.compress(base)).decode()
    packed_own = base64.b64encode(zlib.compress(own)).decode()
    bootstrap = "import base64,zlib,types;g={'__name__':'trace_base','__file__':'/trace-v10.py'};exec(zlib.decompress(base64.b64decode(" + repr(packed_base) + ")),g);" + \
        "o={'__name__':'trace_operator','__file__':'/trace-operator.py'};exec(zlib.decompress(base64.b64decode(" + repr(packed_own) + ")),o);" + \
        "o['remote_entry'](types.SimpleNamespace(**g)," + repr(encoded) + "," + repr(config["operatorSha256"]) + ")"
    command = "sudo -n -- python3 -B -u -c " + shlex.quote(bootstrap)
    need(len(command.encode()) <= 131072, "bounded_trace_command_required")
    stdin, stdout, stderr = client.exec_command(command, timeout=400)
    stdin.channel.shutdown_write()
    raw = stdout.read(2097153); errors = stderr.read(16385)
    code = stdout.channel.recv_exit_status()
    need(code == 0 and len(raw) <= 2097152 and len(errors) <= 16384, "remote_trace_result_unknown")
    result = json.loads(raw)
    need(isinstance(result, dict) and result.get("schema") == SCHEMA and result.get("action") == config["action"] and
         type(result.get("passed")) is bool, "remote_trace_receipt_invalid")
    result.update(actualRemoteExitCode=code, remoteExitStatusReceived=True, stderrBytes=len(errors), stderrSha256=sha(errors))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--action", required=True, choices=["status", "prepare", "apply", "restore"])
    parser.add_argument("--output", required=True)
    parser.add_argument("--baseline-public", required=True)
    parser.add_argument("--expected-pid", required=True, type=int)
    parser.add_argument("--status-proof"); parser.add_argument("--status-proof-sha256")
    parser.add_argument("--installed-state", help="Prepare only: immutable private/release-scene-v7/state.json; hash only")
    parser.add_argument("--paramiko-runtime", default=str(Path.home() / ".codex/tmp/mir2-deploy-ssh-runtime"))
    args = parser.parse_args(argv)
    need(args.expected_pid >= 0, "explicit_caller_pid_required")
    need(not args.installed_state or args.action == "prepare", "installed_anchor_prepare_only")
    need(args.action != "status" or not args.status_proof and not args.status_proof_sha256, "status_never_consumes_mutation_proof")
    B = load_base(Path.cwd() / "mir2-web3/scripts/capacity/cloud-reference/cloud_test_release_scene_v10.py")
    source_guard(B)
    baseline, _ = B.read_hashed_json(args.baseline_public, BASELINE_SHA)
    historical = model(baseline)
    output = B.output_directory(args.output); B.guard_private_output(output)
    need(output.name == "cloud8f-delivery-trace-v1" and output.parent.name == "private", "fixed_new_trace_operator_output_required")
    need(args.action == "status" or not (output / "reconciliation-required.json").exists(), "unknown_prior_rpc_requires_readonly_reconciliation")
    state_path = output / "state.json"
    exists = B.guard_local_state(state_path)
    prior_sha = B.file_hash(state_path) if exists else None
    state = json.loads(state_path.read_text(encoding="utf8")) if exists else None
    operator_sha = sha(Path(__file__).read_bytes())
    need(state is None or state.get("operatorSha256") == operator_sha, "local_trace_tool_changed")
    if args.action == "prepare":
        need(state is None and args.installed_state, "fresh_trace_state_and_installed_anchor_required")
        descriptor = installed_pin(B, args.installed_state)
    elif state is not None:
        descriptor = state["installedStateDescriptor"]
        need(installed_pin(B, descriptor["path"]) == descriptor, "installed_state_descriptor_changed")
    else:
        need(args.action == "status", "trace_prepare_required")
        descriptor = None
    fresh = None
    if args.action != "status":
        need(args.status_proof and args.status_proof_sha256, "explicit_fresh_status_proof_required")
        raw_proof, _ = B.read_hashed_json(args.status_proof, args.status_proof_sha256)
        fresh = raw_proof.get("proof", raw_proof)
    config = {"action": args.action, "expectedPid": args.expected_pid, "operatorSha256": operator_sha, "historical": historical,
              "installedStateSha256": INSTALLED_STATE_SHA, "freshStatus": fresh}
    lock = output / ".transaction.lock"
    B.private_write_new(lock, {"pid": os.getpid(), "atUnixMs": time.time_ns() // 1_000_000})
    lock_sha, client, watchdog = B.file_hash(lock), None, None
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    try:
        B.private_write_new(output / (args.action + "-intent-" + stamp + ".json"), {"config": config, "priorStateSha256": prior_sha,
            "installedStateDescriptor": descriptor, "requestSourceSha256": operator_sha})
        need((B.file_hash(state_path) if B.guard_local_state(state_path) else None) == prior_sha, "local_trace_state_changed_before_rpc")
        if descriptor is not None: need(installed_pin(B, descriptor["path"]) == descriptor, "installed_state_changed_before_rpc")
        sys.path.insert(0, str(Path(args.paramiko_runtime).expanduser().resolve()))
        import paramiko
        client = B.connect_pinned(paramiko)
        watchdog = threading.Timer(500, client.close); watchdog.daemon = True; watchdog.start()
        try: result = ssh_request(B, client, config, state)
        except Exception as error:
            result = {"schema": SCHEMA, "action": args.action, "passed": False, "remoteOutcomeUnknown": True,
                "errorClass": type(error).__name__, "capacityQualified": False, "noBlindRetry": True}
            if args.action != "status":
                B.private_write_new(output / "reconciliation-required.json", {"action": args.action, "atUtc": stamp,
                    "operatorSha256": operator_sha, "priorStateSha256": prior_sha, "remoteOutcomeUnknown": True})
            if args.action != "status" and state is not None:
                state = copy.deepcopy(state); state.update(remoteOutcomeUnknown=True, phase="remote-outcome-unknown")
                result["state"] = state
        if result.get("state") is not None:
            result["state"]["installedStateDescriptor"] = descriptor
        result.update(operatorSha256=operator_sha, baseSourceSha256=BASE_SHA, historicalPublicSha256=BASELINE_SHA,
                      credentialsPersisted=False, receiptAtUtc=datetime.now(timezone.utc).isoformat())
        receipt = output / (args.action + "-" + stamp + ".json"); B.private_write_new(receipt, result)
        if args.action != "status" and result.get("state") is not None:
            need((B.file_hash(state_path) if B.guard_local_state(state_path) else None) == prior_sha, "local_trace_state_cas_changed")
            need(descriptor is not None and installed_pin(B, descriptor["path"]) == descriptor, "installed_state_changed_during_rpc")
            B.private_write_new(output / ("state-" + stamp + ".json"), result["state"])
            temporary = output / (".state-" + stamp + ".tmp"); B.private_write_new(temporary, result["state"]); os.replace(temporary, state_path)
        print(json.dumps({"passed": result["passed"], "receipt": str(receipt), "phase": result.get("state", {}).get("phase"),
            "remoteOutcomeUnknown": result.get("remoteOutcomeUnknown", False), "capacityQualified": False}))
        return 0 if result["passed"] else 2
    finally:
        if watchdog is not None: watchdog.cancel()
        if client is not None: client.close()
        if lock.exists() and B.file_hash(lock) == lock_sha: lock.unlink()


if __name__ == "__main__":
    try: raise SystemExit(main())
    except (Refusal, ValueError) as error:
        print(json.dumps({"passed": False, "error": str(error), "capacityQualified": False})); raise SystemExit(2)

