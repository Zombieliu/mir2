"""V7 evidence, TEST-only OFFLINE intent planner. No service execution or network.

Fresh inventory must contain a complete V5 NativeHost.snapshot (with the V6
namespace configured), actual protected service/process records, every original
configuration fingerprint and explicit absent/present path observations. An
executor/collector is deliberately NOT supplied by this module. Plans are never
execution receipts. See input_contract() for the concrete required fields.
"""
import argparse
import copy
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import types

V5_SHA256 = "9f1f6437c50bc1d65df916ac0854d893468656a8647db321acae38fef835a35e"
VERIFIER_SHA256 = "66e843b535f08c34adf8971290f7005d8fbf078fb803ed88ceb43530797b42f5"
SCHEMA = "mir2.scene-candidate-intent.v7"
UNIT = "mir2-playtest.service"
SUFFIX = "-comparable-scene-20261113-v6"
RELEASE_ENV = "/etc/mir2-playtest-comparable-scene-20261113-v6.env"
RELEASE_DROPIN = "/etc/systemd/system/mir2-playtest.service.d/" + "z" * 54 + "-comparable-scene-20261113-v6.conf"
CAPS_ENV = "/run/mir2-playtest-comparable-scene-20261113-v6.env"
CAPS_DROPIN = "/run/systemd/system/mir2-playtest.service.d/" + "z" * 56 + "-comparable-scene-capacity-20261113-v6.conf"
OWN_PATHS = [RELEASE_ENV, RELEASE_DROPIN, CAPS_ENV, CAPS_DROPIN]
ACTIONS = ["prepare", "install", "apply-caps", "restore-caps"]


class Refusal(Exception):
    pass


def require(value, code):
    if not value:
        raise Refusal(code)


def is_sha(value, length=64):
    return isinstance(value, str) and re.fullmatch("[0-9a-f]{" + str(length) + "}", value) is not None


def canonical_sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def file_sha(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1048576), b""):
            value.update(block)
    return value.hexdigest()


def regular(path, limit):
    path = Path(path).absolute()
    for item in [path, *path.parents]:
        require(not item.is_symlink() and not (hasattr(item, "is_junction") and item.is_junction()), "linked_input_rejected")
    info = path.stat()
    require(path.is_file() and info.st_nlink == 1 and 0 < info.st_size <= limit, "bounded_regular_input_required")
    return path


def pinned_json(path, expected_sha, limit=4194304):
    path = regular(path, limit)
    require(is_sha(expected_sha) and file_sha(path) == expected_sha, "explicit_input_sha_mismatch")
    result = json.loads(path.read_bytes())
    require(isinstance(result, dict), "json_object_required")
    return result


def load_v5(path=None):
    """Load exact, unmodified V5 helpers; change only new owned namespace constants."""
    if path is None:
        frozen = Path(__file__).with_name("v5-core") / "cloud_test_release_v5.py"
        path = frozen if frozen.exists() else Path(__file__).resolve().parents[2] / "cloud-reference/cloud_test_release_v5.py"
    path = regular(path, 131072)
    require(file_sha(path) == V5_SHA256, "immutable_v5_source_hash_mismatch")
    module = types.ModuleType("scene_v6_pinned_v5")
    module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    previous = [(module.OLDER_RELEASE_ENV, module.OLDER_RELEASE_DROPIN),
                (module.PREVIOUS_RELEASE_ENV, module.PREVIOUS_RELEASE_DROPIN),
                (module.PREVIOUS_V3_RELEASE_ENV, module.PREVIOUS_V3_RELEASE_DROPIN),
                (module.PREVIOUS_V4_RELEASE_ENV, module.PREVIOUS_V4_RELEASE_DROPIN),
                (module.RELEASE_ENV, module.RELEASE_DROPIN)]
    for key, value in {"RELEASE_SUFFIX": SUFFIX, "RELEASE_ENV": RELEASE_ENV, "RELEASE_DROPIN": RELEASE_DROPIN,
                       "CAPS_ENV": CAPS_ENV, "CAPS_DROPIN": CAPS_DROPIN, "OWN_PATHS": OWN_PATHS}.items():
        setattr(module, key, value)
    return module, previous


def identity(value, inactive=False):
    require(isinstance(value, dict), "concrete_identity_required")
    if inactive and value.get("pid") == 0:
        require(value.get("activeState") == "inactive" and value.get("startTicks") is None, "inactive_identity_not_proven")
        return
    require(type(value.get("pid")) is int and value["pid"] > 1 and type(value.get("startTicks")) is int and value["startTicks"] > 0 and is_sha(value.get("invocationId"), 32), "full_pid_invocation_start_identity_required")


def metadata(value, path):
    require(isinstance(value, dict) and value.get("path") == path and is_sha(value.get("sha256")) and
            all(type(value.get(k)) is int and value[k] >= 0 for k in ["bytes", "device", "inode", "mode", "uid", "gid", "mtimeNs"]) and
            value["inode"] > 0 and value["uid"] == 0 and not value["mode"] & 0o022, "complete_root_owned_file_fingerprint_required")


def complete_snapshot(core, snapshot, previous):
    require(isinstance(snapshot, dict) and all(k in snapshot for k in ["identity", "unit", "health", "oldGateway", "zoneIdentity", "binarySha256", "executablePath", "configuredExecutablePath", "processWorkingDirectory", "argvTailSha256", "execStartSha256", "environmentRevision", "environmentCaps", "protectedEnvironmentSha256", "protectedConfig", "originalEnvironmentFiles", "originalDropIns", "environmentFiles", "dropIns"]), "complete_native_snapshot_required")
    identity(snapshot["identity"]); identity(snapshot["oldGateway"]["identity"]); identity(snapshot["zoneIdentity"], inactive=True)
    require(all(k in snapshot["unit"] for k in core.PROPS if k != "ExecStart"), "complete_systemd_properties_required")
    for key in ["binarySha256", "argvTailSha256", "execStartSha256", "protectedEnvironmentSha256"]:
        require(is_sha(snapshot[key]), "complete_snapshot_hash_required")
    require(snapshot["executablePath"] == snapshot["configuredExecutablePath"] and snapshot["executablePath"].startswith("/") and snapshot["processWorkingDirectory"].startswith("/"), "actual_executable_and_cwd_required")
    protected = snapshot["protectedConfig"]
    require(protected.get("originalEnvironmentFiles") == snapshot["originalEnvironmentFiles"] and protected.get("originalDropIns") == snapshot["originalDropIns"] and protected.get("properties") == {k: snapshot["unit"][k] for k in core.STABLE if k != "EnvironmentFiles"}, "complete_protected_configuration_required")
    envs, drops = snapshot["originalEnvironmentFiles"], snapshot["originalDropIns"]
    require(isinstance(envs, list) and len(envs) == len({r["path"] for r in envs}) and all(set(r) == {"path", "ignoreErrors"} and type(r["ignoreErrors"]) is bool and r["path"].startswith("/") for r in envs), "concrete_original_environment_list_required")
    require(isinstance(drops, list) and len(drops) == len(set(drops)) and all(isinstance(p, str) and p.startswith("/") for p in drops), "concrete_original_dropin_list_required")
    env_paths = [r["path"] for r in envs]
    require(all(e in env_paths and d in drops for e, d in previous) and [env_paths.index(e) for e, d in previous] == sorted(env_paths.index(e) for e, d in previous) and [drops.index(d) for e, d in previous] == sorted(drops.index(d) for e, d in previous), "all_v1_through_v5_pairs_and_order_required")
    require(all(envs[env_paths.index(e)]["ignoreErrors"] is False for e, d in previous), "prior_release_environment_cannot_be_optional")
    required_paths = set(env_paths + drops + [snapshot["unit"]["FragmentPath"]])
    records = protected.get("files")
    require(isinstance(records, list) and len(records) == len({r.get("path") for r in records}) and {r.get("path") for r in records} == required_paths, "every_original_configuration_file_required")
    for record in records:
        metadata(record, record["path"])
    try:
        core.basic(snapshot); core.empty(snapshot)
    except core.Refusal as error:
        raise Refusal(str(error)) from None


def protected_set(fresh, snapshot, core):
    require(fresh.get("serviceSetComplete") is True and isinstance(fresh.get("serviceUnitNames"), list), "fresh_complete_service_enumeration_required")
    units = fresh["serviceUnitNames"]
    require(units == sorted(set(units)) and all(isinstance(u, str) and re.fullmatch(r"[A-Za-z0-9@_.-]+\.service", u) for u in units) and {UNIT, core.OLD_GATEWAY, core.ZONE}.issubset(units), "actual_service_unit_enumeration_required")
    require(fresh.get("serviceEnumerationSha256") == canonical_sha(units), "service_enumeration_fingerprint_mismatch")
    records = fresh.get("protectedServices")
    require(isinstance(records, list) and records and len(records) == len({r.get("instance") for r in records}), "concrete_unique_protected_service_records_required")
    roles = [r.get("role") for r in records]
    require(roles.count("oldGateway") == roles.count("oldZone") == roles.count("reverseProxy") == 1 and roles.count("validator") == 4 and roles.count("vpn") >= 1 and set(roles) == {"oldGateway", "oldZone", "reverseProxy", "validator", "vpn"}, "actual_old_gateway_zone_proxy_four_validators_vpn_set_required")
    for record in records:
        require(isinstance(record.get("instance"), str) and record["instance"] and is_sha(record.get("configurationSha256")), "protected_service_fingerprint_required")
        if record.get("state") == "absent-confirmed":
            require(record["role"] == "vpn" and is_sha(record.get("absenceObservationSha256")) and isinstance(record.get("checkedUnitNames"), list) and record["checkedUnitNames"], "explicit_vpn_absence_proof_required")
        else:
            require(record.get("state") in ["active", "inactive"] and record.get("unit") in units and is_sha(record.get("binarySha256")), "protected_unit_binary_and_state_required")
            identity(record.get("identity"), inactive=record["state"] == "inactive")
        if record["role"] == "oldGateway":
            require(record.get("unit") == core.OLD_GATEWAY and record.get("identity") == snapshot["oldGateway"]["identity"], "old_gateway_inventory_disagrees")
        if record["role"] == "oldZone":
            require(record.get("unit") == core.ZONE and record.get("identity") == snapshot["zoneIdentity"], "old_zone_inventory_disagrees")
    active = [r["identity"] for r in records if r.get("state") == "active"]
    require(len(active) == len({(r["pid"], r["startTicks"]) for r in active}), "protected_processes_cannot_be_duplicated")
    return {"serviceUnitNames": units, "protectedServices": sorted(records, key=lambda r: r["instance"])}


def validate_fresh(core, previous, fresh, expected, now=None):
    require(expected.get("schema") == "mir2.scene-candidate-expectations.v6" and is_sha(expected.get("sourceRevision"), 40) and is_sha(expected.get("binarySha256")) and is_sha(expected.get("inventoryReaderSha256")), "explicit_expected_source_binary_reader_required")
    identity(expected.get("identity"))
    require(fresh.get("schema") == "mir2.scene-candidate-fresh-inventory.v6" and fresh.get("readOnly") is True and fresh.get("inventoryComplete") is True and fresh.get("readerSourceSha256") == expected["inventoryReaderSha256"], "pinned_complete_fresh_inventory_required")
    observed = datetime.fromisoformat(fresh["observedAtUtc"].replace("Z", "+00:00"))
    require(observed.tzinfo is not None and -2 <= ((now or datetime.now(timezone.utc)) - observed).total_seconds() <= 30, "inventory_is_not_fresh_within_30_seconds")
    snapshot = fresh.get("snapshot"); complete_snapshot(core, snapshot, previous)
    require(fresh.get("identityBefore") == fresh.get("identityAfter") == snapshot["identity"] == expected["identity"], "fresh_full_activation_cas_mismatch")
    require(snapshot["health"].get("revision") == snapshot["environmentRevision"] == expected["sourceRevision"] and snapshot["binarySha256"] == expected["binarySha256"], "fresh_expected_source_or_binary_mismatch")
    return snapshot, protected_set(fresh, snapshot, core)


def artifact_proof(proof, expected, verifier_path=None):
    path = regular(verifier_path or Path(__file__).with_name("verify_scene_candidate_artifact.py"), 131072)
    require(file_sha(path) == VERIFIER_SHA256 and proof.get("verifierSourceSha256") == VERIFIER_SHA256, "pinned_v6_verifier_required")
    require(proof.get("schema") == "mir2.scene-candidate-artifact-verification.v7" and proof.get("passed") is True and
            is_sha(proof.get("candidateRevision"), 40) and is_sha(proof.get("baselineRevision"), 40) and is_sha(proof.get("baselineBinarySha256")) and
            is_sha(proof.get("candidatePinsSha256")) and is_sha(proof.get("actionsMetadataSha256")) and is_sha(proof.get("sourceInputManifestSha256")) and
            type(proof.get("runId")) is int and proof["runId"] > 0 and type(proof.get("nativeJobId")) is int and proof["nativeJobId"] > 0 and
            proof.get("maps", {}).get("allGitArtifactLiveMatch") is True and proof.get("nativeStartupActualExitCode") == 0 and
            is_sha(proof.get("releaseManifestSha256")) and is_sha(proof.get("binarySha256")) and type(proof.get("binarySizeBytes")) is int,
            "complete_actual_artifact_proof_required")
    source = proof.get("sourceCompatibility", {})
    delta = source.get("canonicalNonNativeDelta", {})
    require(is_sha(proof.get("integrationCommit"), 40) and proof["integrationCommit"] != proof["candidateRevision"] and
            proof.get("wholeWorkflowStatus") == "completed" and proof.get("wholeWorkflowConclusion") == "success" and
            source.get("integrationCommit") == proof["integrationCommit"] and source.get("candidateSource") == proof["candidateRevision"] and
            delta.get("path") == ".github/scripts/numeron-r16-release.mjs" and delta.get("nativeJobDirectCallExcluded") is True and
            delta.get("nativeInputScriptReferenceExcluded") is True, "whole_green_ci_and_exact_integration_source_mapping_required")
    gates = {"schema_union", "monthly_card_postgres", "billing_postgres", "activities_postgres", "linux-release-security", "gateway-package", "gateway-postgres-startup"}
    require(isinstance(proof.get("nativeRequiredGates"), list) and len(proof["nativeRequiredGates"]) == 7 and set(proof["nativeRequiredGates"]) == gates and
            type(proof.get("nativeStartupActualExitCode")) is int and proof["nativeStartupActualExitCode"] == 0 and
            isinstance(proof.get("artifacts"), dict) and set(proof["artifacts"]) == {"gateway", "nativeEvidence"} and
            all(type(r.get("id")) is int and r["id"] > 0 and is_sha(r.get("sha256")) for r in proof["artifacts"].values()), "native_gates_and_actual_artifact_ids_required")
    require(is_sha(expected.get("artifactProofSha256")) and canonical_sha(proof) == expected["artifactProofSha256"], "reviewed_artifact_proof_content_mismatch")
    directory = Path(proof["artifactDirectory"])
    binary = regular(directory / "mir2-gateway", 268435456); release = regular(directory / "RELEASE.json", 65536)
    require(file_sha(binary) == proof["binarySha256"] and binary.stat().st_size == proof["binarySizeBytes"] and file_sha(release) == proof["releaseManifestSha256"], "verified_artifact_files_changed")
    manifest = json.loads(release.read_bytes())
    require(manifest.get("gitRevision") == proof["candidateRevision"] and manifest.get("gitDirty") is False and manifest.get("binarySha256") == proof["binarySha256"], "artifact_release_manifest_disagrees")
    with binary.open("rb") as stream:
        header = stream.read(24)
    require(header[:7] == b"\x7fELF\x02\x01\x01" and header[18:24] == b"\x3e\x00\x01\x00\x00\x00", "artifact_not_linux_x64_elf")
    return {"revision": proof["candidateRevision"], "binarySha256": proof["binarySha256"], "binarySizeBytes": proof["binarySizeBytes"],
            "manifestSha256": proof["releaseManifestSha256"], "releasePath": "/opt/mir2-playtest/releases/" + proof["candidateRevision"] + SUFFIX}


def path_guard(fresh, candidate, phase, state=None):
    observations = fresh.get("pathObservations")
    required = set(OWN_PATHS + [candidate["releasePath"], candidate["releasePath"] + "/mir2-gateway"])
    require(isinstance(observations, dict) and required.issubset(observations), "complete_namespace_path_observations_required")
    present = set() if phase == "prepared" else {RELEASE_ENV, RELEASE_DROPIN, candidate["releasePath"], candidate["releasePath"] + "/mir2-gateway"}
    if phase == "caps-applied":
        present |= {CAPS_ENV, CAPS_DROPIN}
    for path in required:
        observation = observations[path]
        require(type(observation.get("exists")) is bool and observation["exists"] == (path in present), "namespace_path_presence_mismatch")
        if path in OWN_PATHS and path in present:
            metadata(observation.get("metadata"), path)
            require(state and observation["metadata"]["mode"] == 0o600 and observation["metadata"] == state["ownedFiles"].get(path) and observation["metadata"]["sha256"] == state["plan"][path]["sha256"] and observation["metadata"]["bytes"] == state["plan"][path]["bytes"], "owned_file_identity_or_planned_hash_changed")
        if path == candidate["releasePath"] and path in present:
            value = observation.get("metadata", {})
            require(value.get("path") == path and value.get("kind") == "directory" and value.get("uid") == 0 and value.get("mode") == 0o755 and
                    all(type(value.get(k)) is int and value[k] > 0 for k in ["device", "inode"]) and state and value == state.get("stagedDirectory"), "staged_directory_identity_or_mode_changed")
        if path.endswith("/mir2-gateway") and path in present:
            metadata(observation.get("metadata"), path)
            require(observation["metadata"]["mode"] == 0o755 and state and observation["metadata"] == state.get("stagedBinary") and observation["metadata"]["sha256"] == candidate["binarySha256"] and observation["metadata"]["bytes"] == candidate["binarySizeBytes"], "staged_gateway_binary_changed")


def make_plan(action, fresh, expected, proof, state=None, v5_source=None, now=None):
    require(action in ACTIONS, "only_prepare_install_apply_restore_supported")
    core, previous = load_v5(v5_source)
    snapshot, services = validate_fresh(core, previous, fresh, expected, now)
    candidate = artifact_proof(proof, expected)
    state = copy.deepcopy(state)
    if action == "prepare":
        require(state is None and proof["baselineRevision"] == expected["sourceRevision"] and proof["baselineBinarySha256"] == expected["binarySha256"] and candidate["revision"] != expected["sourceRevision"] and candidate["binarySha256"] != expected["binarySha256"], "new_candidate_and_exact_baseline_required")
        require(all(snapshot["health"]["capacity"][key] == value for key, value in core.OLD_CAPS.items()), "prepare_requires_original_51_66_51_caps")
        path_guard(fresh, candidate, "prepared")
        planned = fresh.get("plannedOwnedFiles")
        require(isinstance(planned, dict) and set(planned) == set(OWN_PATHS) and all(is_sha(v.get("sha256")) and type(v.get("bytes")) is int and 0 < v["bytes"] <= 65536 for v in planned.values()), "actual_trusted_host_plan_fingerprints_required")
        state = {"schema": "mir2.scene-candidate-state.v6", "phase": "prepared", "baseline": snapshot, "protectedServiceSet": services,
                 "oldPins": {"revision": expected["sourceRevision"], "binarySha256": expected["binarySha256"]}, "candidate": candidate,
                 "knownActivation": snapshot["identity"], "ownedFiles": {}, "plan": planned, "oldBinaryRollbackAllowed": False,
                 "artifactProofContentSha256": canonical_sha(proof), "inventoryReaderSha256": expected["inventoryReaderSha256"]}
    else:
        required_phase = {"install": "prepared", "apply-caps": "installed", "restore-caps": "caps-applied"}[action]
        require(isinstance(state, dict) and state.get("schema") == "mir2.scene-candidate-state.v6" and state.get("phase") == required_phase and state.get("candidate") == candidate and state.get("oldBinaryRollbackAllowed") is False and state.get("artifactProofContentSha256") == canonical_sha(proof) and state.get("inventoryReaderSha256") == expected["inventoryReaderSha256"], "exact_prior_state_candidate_and_phase_required")
        require(state.get("protectedServiceSet") == services, "protected_service_set_identity_or_configuration_changed")
        if required_phase != "prepared":
            executed = state.get("executionProof", {})
            require(executed.get("executed") is True and executed.get("normalStopProven") is True and executed.get("forceKillUsed") is False and is_sha(executed.get("intentSha256")) and executed.get("afterIdentity") == state.get("knownActivation"), "actual_prior_execution_receipt_required_not_a_plan")
        try:
            core.validate(snapshot, state, identity=state["knownActivation"])
        except core.Refusal as error:
            raise Refusal(str(error)) from None
        path_guard(fresh, candidate, required_phase, state)
    writes, removals = [], []
    if action == "install":
        writes = [candidate["releasePath"] + "/mir2-gateway", RELEASE_ENV, RELEASE_DROPIN]
    elif action == "apply-caps":
        writes = [CAPS_ENV, CAPS_DROPIN]
    elif action == "restore-caps":
        removals = [CAPS_ENV, CAPS_DROPIN]
    commands = [] if action == "prepare" else [["systemctl", "--no-block", "stop", UNIT], ["systemctl", "daemon-reload"], ["systemctl", "--no-block", "start", UNIT]]
    return copy.deepcopy({"schema": SCHEMA, "action": action, "validated": True, "executed": False, "deploymentApproved": False,
            "unit": UNIT, "inventoryObservedAtUtc": fresh["observedAtUtc"], "inventoryContentSha256": canonical_sha(fresh), "expectationsContentSha256": canonical_sha(expected),
            "candidate": candidate, "integrationCommit": proof["integrationCommit"], "expectedActivation": snapshot["identity"], "state": state,
            "originalEnvironmentFileCountObserved": len(state["baseline"]["originalEnvironmentFiles"]),
            "writePaths": writes, "removePaths": removals, "serviceCommandsIfExecutedLater": commands,
            "plannedOwnedFileFingerprints": state["plan"], "onlyStageGatewayNeverZoneHost": True,
            "normalStopOnly": True, "sigkillAllowed": False, "oldBinaryRollbackAllowed": False,
            "desiredCaps": core.TEST_CAPS if action == "apply-caps" else core.OLD_CAPS,
            "restorationKeepsCandidateBinary": action == "restore-caps",
            "executionRequirements": ["Freshly repeat the complete inventory, nine zero counters and full activation/config CAS immediately before any mutation", "Use V5 normal SIGINT stop and exact inactive identity/normal exit proof; never SIGKILL", "Stage only the pinned Gateway via exclusive no-follow file, rehash size/ELF/uid/mode and fsync", "Create only planned root-owned namespace files; verify their exact bytes/hash/inode; retain original argv tail and cwd", "Verify post-start full identity, source/executable SHA, all protected services/configuration, limits/auth, nine zero counters and owned files", "Publish actual execution receipt before advancing phase; this intent is not an installation receipt"],
            "v5SourceSha256": V5_SHA256, "artifactVerifierSourceSha256": VERIFIER_SHA256,
            "transportImplemented": False, "freshRemoteCollectorImplemented": False, "capacityQualified": False})


def input_contract():
    return {"artifactProofSchema": "mir2.scene-candidate-artifact-verification.v7", "actualCISourceAndIntegrationCommitAreSeparate": True, "wholeWorkflowAndNativeJobMustSucceed": True, "remoteNamespaceUnchangedFromV6": True, "inventorySchema": "mir2.scene-candidate-fresh-inventory.v6", "requiredInventoryFields": ["observedAtUtc", "readOnly=true", "inventoryComplete=true", "readerSourceSha256", "identityBefore", "identityAfter", "snapshot=complete V5 NativeHost.snapshot with V6 namespaces", "serviceSetComplete=true", "serviceUnitNames=all actual unique sorted units", "serviceEnumerationSha256", "protectedServices=oldGateway/oldZone/reverseProxy/four validator/vpn records with identity/binary/configuration fingerprints", "pathObservations=all four V6 files plus candidate release directory and binary", "plannedOwnedFiles=trusted-host exact hashes/bytes of all four V6 files"], "expectationsSchema": "mir2.scene-candidate-expectations.v6", "requiredExpectationFields": ["identity=pid/invocationId/startTicks", "sourceRevision=actualCIrevision", "binarySha256", "inventoryReaderSha256", "artifactProofSha256=canonical JSON content hash"], "noDefaultSourcePidArtifactOrServiceSet": True}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--describe-inputs", action="store_true")
    parser.add_argument("--action", choices=ACTIONS)
    for name in ["inventory", "expectations", "artifact-proof", "state"]:
        parser.add_argument("--" + name, type=Path)
        parser.add_argument("--" + name + "-sha256")
    parser.add_argument("--v5-source", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    if args.describe_inputs:
        print(json.dumps(input_contract(), indent=2)); return
    require(args.action and args.output and all(getattr(args, key) and getattr(args, key + "_sha256") for key in ["inventory", "expectations", "artifact_proof"]), "explicit_action_output_and_all_pinned_inputs_required")
    require(bool(args.state) == bool(args.state_sha256), "state_and_explicit_sha_must_be_paired")
    fresh, expected, proof = [pinned_json(getattr(args, k), getattr(args, k + "_sha256")) for k in ["inventory", "expectations", "artifact_proof"]]
    state = pinned_json(args.state, args.state_sha256) if args.state else None
    result = make_plan(args.action, fresh, expected, proof, state, args.v5_source)
    output = args.output.absolute()
    require(output.drive.replace("\\\\?\\", "").lower() == "c:" and not output.exists() and output.parent.is_dir(), "fresh_c_only_output_required")
    for item in [output.parent, *output.parent.parents]:
        require(not item.is_symlink() and not (hasattr(item, "is_junction") and item.is_junction()), "linked_output_parent")
    with output.open("x", encoding="utf8", newline="\n") as stream:
        stream.write(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"validated": True, "executed": False, "action": args.action, "output": str(output)}))


if __name__ == "__main__":
    try:
        main()
    except (Refusal, OSError, ValueError, KeyError, TypeError) as error:
        print(json.dumps({"validated": False, "executed": False, "errorClass": type(error).__name__, "error": str(error) if isinstance(error, Refusal) else "invalid_or_incomplete_local_evidence"}))
        raise SystemExit(1)
