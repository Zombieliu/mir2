"""Offline synthetic evidence only. No credentials, SSH, game or service actions."""
import contextlib
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec); spec.loader.exec_module(value)
    return value


m = module("scene_candidate_v6")
v = module("verify_scene_candidate_artifact")


def temporary():
    root = Path(tempfile.mkdtemp(prefix="mir2-scene-v6-test-"))
    if root.drive.lower() != "c:":
        raise RuntimeError("C-only SDK fixtures required")
    # Retain small task-owned synthetic evidence; never recursively remove a path.
    return root


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf8", newline="\n") as stream:
        stream.write(json.dumps(value) if not isinstance(value, str) else value)
    return {"path": path.name, "sha256": v.sha(path)}


def ident(pid):
    return {"pid": pid, "invocationId": format(pid, "032x"), "startTicks": pid * 1000}


def fingerprint(path, number=1, checksum="7" * 64, size=100, mode=0o600):
    return {"path": path, "sha256": checksum, "bytes": size, "device": 1, "inode": number,
            "mode": mode, "uid": 0, "gid": 0, "mtimeNs": number * 1000}


def elf():
    header = bytearray(64)
    header[:7] = b"\x7fELF\x02\x01\x01"; header[16:18] = b"\x03\x00"; header[18:24] = b"\x3e\x00\x01\x00\x00\x00"
    return bytes(header) + b"SYNTHETIC-NOT-EXECUTABLE\n"


class PlanFixture:
    def __init__(self, extra_env=3):
        self.root = temporary(); self.now = datetime.now(timezone.utc); self.core, previous = m.load_v5()
        env = [{"path": "/etc/synthetic-" + str(i) + ".env", "ignoreErrors": False} for i in range(extra_env)] + [{"path": e, "ignoreErrors": False} for e, d in previous]
        drops = [d for e, d in previous]
        unit = {k: "" for k in self.core.PROPS if k != "ExecStart"}
        unit.update(ActiveState="active", SubState="running", MainPID="101", SendSIGKILL="no", KillSignal="2", KillMode="control-group", MemoryMax="1677721600", MemoryHigh="1468006400", WorkingDirectory="/opt/mir2-playtest/data", FragmentPath="/etc/systemd/system/mir2-playtest.service")
        files = [fingerprint(p, i + 1) for i, p in enumerate(sorted({e["path"] for e in env} | set(drops) | {unit["FragmentPath"]}))]
        health = {"ok": True, "http": "ready", "ws": "ready", "revision": "a" * 40,
                  "capacity": {**dict.fromkeys(self.core.CURRENT, 0), **self.core.AUTH, **self.core.OLD_CAPS},
                  "sessionCache": {"backend": "redis", "configured": True, "healthy": True, "recordCount": 0, "routeLeaseCount": 0, "staleRecordCount": 0}}
        snapshot = {"identity": ident(101), "unit": unit, "health": health, "oldGateway": {"identity": ident(201), "health": copy.deepcopy(health)}, "zoneIdentity": ident(301),
                    "binarySha256": "1" * 64, "executablePath": "/opt/old/mir2-gateway", "configuredExecutablePath": "/opt/old/mir2-gateway", "processWorkingDirectory": unit["WorkingDirectory"],
                    "argvTailSha256": "2" * 64, "execStartSha256": "3" * 64, "environmentRevision": "a" * 40,
                    "environmentCaps": {self.core.CAP_ENV[k]: str(val) for k, val in self.core.OLD_CAPS.items()}, "protectedEnvironmentSha256": "4" * 64,
                    "originalEnvironmentFiles": env, "originalDropIns": drops, "environmentFiles": env, "dropIns": drops,
                    "protectedConfig": {"properties": {k: unit[k] for k in self.core.STABLE if k != "EnvironmentFiles"}, "files": files, "originalEnvironmentFiles": env, "originalDropIns": drops}}
        roles = [("oldGateway", self.core.OLD_GATEWAY, 201), ("oldZone", self.core.ZONE, 301), ("reverseProxy", "caddy.service", 401)] + [("validator", "validator" + str(i) + ".service", 500 + i) for i in range(4)] + [("vpn", "synthetic-vpn.service", 601)]
        services = [{"role": role, "instance": name, "unit": name, "state": "active", "identity": ident(pid), "binarySha256": "5" * 64, "configurationSha256": "6" * 64} for role, name, pid in roles]
        units = sorted({m.UNIT, *[r[1] for r in roles]})
        binary = self.root / "release/mir2-gateway"; binary.parent.mkdir(); binary.write_bytes(elf())
        release = binary.with_name("RELEASE.json"); write(release, {"name": "mir2-gateway", "gitRevision": "b" * 40, "gitDirty": False, "binarySha256": v.sha(binary), "binarySizeBytes": binary.stat().st_size})
        self.proof = {"schema": v.SCHEMA, "passed": True, "candidateRevision": "b" * 40, "integrationCommit": "c" * 40, "wholeWorkflowStatus": "completed", "wholeWorkflowConclusion": "success", "sourceCompatibility": {"integrationCommit": "c" * 40, "candidateSource": "b" * 40, "canonicalNonNativeDelta": {"path": v.NON_NATIVE_SCRIPT, "nativeJobDirectCallExcluded": True, "nativeInputScriptReferenceExcluded": True}}, "baselineRevision": "a" * 40, "baselineBinarySha256": "1" * 64,
                      "runId": 10, "nativeJobId": 20, "candidatePinsSha256": "7" * 64, "actionsMetadataSha256": "8" * 64, "sourceInputManifestSha256": "9" * 64,
                      "maps": {"allGitArtifactLiveMatch": True}, "nativeStartupActualExitCode": 0, "nativeRequiredGates": sorted(v.GATES),
                      "artifacts": {"gateway": {"id": 11, "sha256": "e" * 64}, "nativeEvidence": {"id": 12, "sha256": "f" * 64}},
                      "artifactDirectory": str(binary.parent), "binarySha256": v.sha(binary), "binarySizeBytes": binary.stat().st_size, "releaseManifestSha256": v.sha(release), "verifierSourceSha256": m.VERIFIER_SHA256}
        self.expected = {"schema": "mir2.scene-candidate-expectations.v6", "identity": ident(101), "sourceRevision": "a" * 40, "binarySha256": "1" * 64,
                         "inventoryReaderSha256": "0" * 64, "artifactProofSha256": m.canonical_sha(self.proof)}
        self.candidate_path = "/opt/mir2-playtest/releases/" + "b" * 40 + m.SUFFIX
        self.fresh = {"schema": "mir2.scene-candidate-fresh-inventory.v6", "observedAtUtc": self.now.isoformat(), "readOnly": True, "inventoryComplete": True,
                      "readerSourceSha256": "0" * 64, "identityBefore": ident(101), "identityAfter": ident(101), "snapshot": snapshot,
                      "serviceSetComplete": True, "serviceUnitNames": units, "serviceEnumerationSha256": m.canonical_sha(units), "protectedServices": services,
                      "pathObservations": {p: {"exists": False} for p in m.OWN_PATHS + [self.candidate_path, self.candidate_path + "/mir2-gateway"]},
                      "plannedOwnedFiles": {p: {"sha256": str(i + 1) * 64, "bytes": 100 + i} for i, p in enumerate(m.OWN_PATHS)}}

    def plan(self, action="prepare", state=None):
        return m.make_plan(action, self.fresh, self.expected, self.proof, state=state, now=self.now)

    def prepared(self):
        return self.plan()["state"]

    def active(self, phase="installed"):
        state = self.prepared(); state["phase"] = phase; state["knownActivation"] = ident(102); state["candidateExecStartSha256"] = "d" * 64
        state["executionProof"] = {"executed": True, "normalStopProven": True, "forceKillUsed": False, "intentSha256": "c" * 64, "afterIdentity": ident(102)}
        paths = [m.RELEASE_ENV, m.RELEASE_DROPIN] + ([m.CAPS_ENV, m.CAPS_DROPIN] if phase == "caps-applied" else [])
        state["ownedFiles"] = {p: fingerprint(p, 100 + i, state["plan"][p]["sha256"], state["plan"][p]["bytes"]) for i, p in enumerate(paths)}
        state["stagedDirectory"] = {"path": self.candidate_path, "kind": "directory", "uid": 0, "mode": 0o755, "device": 1, "inode": 4000}
        state["stagedBinary"] = fingerprint(self.candidate_path + "/mir2-gateway", 4001, self.proof["binarySha256"], self.proof["binarySizeBytes"], 0o755)
        snapshot = self.fresh["snapshot"]; snapshot["identity"] = ident(102); snapshot["unit"]["MainPID"] = "102"
        snapshot["health"]["revision"] = snapshot["environmentRevision"] = "b" * 40; snapshot["binarySha256"] = self.proof["binarySha256"]
        snapshot["executablePath"] = snapshot["configuredExecutablePath"] = self.candidate_path + "/mir2-gateway"; snapshot["execStartSha256"] = state["candidateExecStartSha256"]
        snapshot["environmentFiles"] = snapshot["originalEnvironmentFiles"] + [{"path": m.RELEASE_ENV, "ignoreErrors": False}]
        snapshot["dropIns"] = snapshot["originalDropIns"] + [m.RELEASE_DROPIN]
        caps = self.core.TEST_CAPS if phase == "caps-applied" else self.core.OLD_CAPS
        snapshot["health"]["capacity"].update(caps); snapshot["environmentCaps"] = {self.core.CAP_ENV[k]: str(val) for k, val in caps.items()}
        if phase == "caps-applied":
            snapshot["environmentFiles"].append({"path": m.CAPS_ENV, "ignoreErrors": False}); snapshot["dropIns"].append(m.CAPS_DROPIN)
        self.expected.update(identity=ident(102), sourceRevision="b" * 40, binarySha256=self.proof["binarySha256"])
        self.fresh.update(identityBefore=ident(102), identityAfter=ident(102))
        for p, item in state["ownedFiles"].items():
            self.fresh["pathObservations"][p] = {"exists": True, "metadata": copy.deepcopy(item)}
        self.fresh["pathObservations"][self.candidate_path] = {"exists": True, "metadata": copy.deepcopy(state["stagedDirectory"])}
        self.fresh["pathObservations"][self.candidate_path + "/mir2-gateway"] = {"exists": True, "metadata": copy.deepcopy(state["stagedBinary"])}
        return state


class PlannerTests(unittest.TestCase):
    def test_prepare_uses_actual_complete_layout_not_17_or_18(self):
        for count in [1, 3, 9]:
            f = PlanFixture(count); p = f.plan()
            self.assertEqual(p["originalEnvironmentFileCountObserved"], count + 5)
            self.assertFalse(p["executed"]); self.assertEqual(p["writePaths"], []); self.assertEqual(p["serviceCommandsIfExecutedLater"], [])

    def test_default_cli_and_describe_inputs_never_run_transport_or_command(self):
        with patch("subprocess.run", side_effect=AssertionError("no command")), contextlib.redirect_stdout(io.StringIO()):
            m.main(["--describe-inputs"])
            with self.assertRaises(m.Refusal):
                m.main([])

    def test_immutable_v5_and_verifier_source_pins(self):
        f = PlanFixture(); core, _ = m.load_v5()
        path = f.root / "wrong-v5.py"; path.write_bytes(Path(core.__file__).read_bytes() + b"\n")
        with self.assertRaisesRegex(m.Refusal, "v5_source_hash"):
            m.load_v5(path)
        self.assertEqual(v.sha(Path(v.__file__)), m.VERIFIER_SHA256)

    def test_missing_source_binary_reader_or_artifact_never_inferred(self):
        for key in ["sourceRevision", "binarySha256", "inventoryReaderSha256", "artifactProofSha256", "identity"]:
            f = PlanFixture(); del f.expected[key]
            with self.assertRaises((m.Refusal, KeyError)):
                f.plan()

    def test_every_prior_pair_requires_complete_original_fingerprints(self):
        for change in [lambda f: f.fresh["snapshot"]["originalEnvironmentFiles"].pop(),
                       lambda f: f.fresh["snapshot"]["protectedConfig"]["files"].pop(),
                       lambda f: f.fresh["snapshot"]["protectedConfig"]["files"][0].pop("inode"),
                       lambda f: f.fresh["snapshot"]["originalEnvironmentFiles"].reverse()]:
            f = PlanFixture(); change(f)
            with self.assertRaises(m.Refusal):
                f.plan()

    def test_namespace_must_be_absent_including_candidate_directory(self):
        f = PlanFixture()
        for path in list(f.fresh["pathObservations"]):
            f.fresh["pathObservations"][path]["exists"] = True
            with self.assertRaisesRegex(m.Refusal, "namespace_path"):
                f.plan()
            f.fresh["pathObservations"][path]["exists"] = False

    def test_stale_future_or_changed_full_activation_rejected(self):
        for seconds in [-31, 3]:
            f = PlanFixture(); f.fresh["observedAtUtc"] = (f.now + timedelta(seconds=seconds)).isoformat()
            with self.assertRaisesRegex(m.Refusal, "fresh"):
                f.plan()
        f = PlanFixture(); f.expected["identity"]["startTicks"] += 1
        with self.assertRaises(m.Refusal):
            f.plan()

    def test_nine_counters_auth_limits_and_sigint_policy_preserved(self):
        for key in [*m.load_v5()[0].CURRENT, "recordCount", "routeLeaseCount", "staleRecordCount", "maxLoginInFlight"]:
            f = PlanFixture(); h = f.fresh["snapshot"]["health"]
            (h["sessionCache"] if key in ["recordCount", "routeLeaseCount", "staleRecordCount"] else h["capacity"])[key] = 3
            with self.assertRaises(m.Refusal):
                f.plan()
        f = PlanFixture(); f.fresh["snapshot"]["unit"]["SendSIGKILL"] = "yes"
        f.fresh["snapshot"]["protectedConfig"]["properties"]["SendSIGKILL"] = "yes"
        with self.assertRaisesRegex(m.Refusal, "sigint"):
            f.plan()

    def test_missing_or_duplicate_protected_services_fail_closed(self):
        f = PlanFixture(); f.fresh["protectedServices"].pop()
        with self.assertRaises(m.Refusal):
            f.plan()
        f = PlanFixture(); f.fresh["protectedServices"][4]["identity"] = copy.deepcopy(f.fresh["protectedServices"][3]["identity"])
        with self.assertRaisesRegex(m.Refusal, "duplicated"):
            f.plan()

    def test_install_plan_only_names_new_test_paths_and_never_legacy_services(self):
        f = PlanFixture(); state = f.prepared(); p = f.plan("install", state)
        self.assertFalse(p["executed"]); self.assertEqual(p["state"]["phase"], "prepared")
        self.assertEqual(p["writePaths"], [f.candidate_path + "/mir2-gateway", m.RELEASE_ENV, m.RELEASE_DROPIN])
        for command in p["serviceCommandsIfExecutedLater"]:
            self.assertIn(command, [["systemctl", "--no-block", "stop", m.UNIT], ["systemctl", "daemon-reload"], ["systemctl", "--no-block", "start", m.UNIT]])
        self.assertFalse(p["oldBinaryRollbackAllowed"])

    def test_foreign_service_old_identity_or_config_change_rejected_after_prepare(self):
        for change in [lambda f: f.fresh["protectedServices"][2].update(configurationSha256="f" * 64),
                       lambda f: f.fresh["snapshot"].update(protectedEnvironmentSha256="e" * 64),
                       lambda f: f.fresh["snapshot"].update(argvTailSha256="e" * 64),
                       lambda f: f.fresh["snapshot"]["protectedConfig"]["files"][0].update(inode=999)]:
            f = PlanFixture(); state = f.prepared(); change(f)
            with self.assertRaises(m.Refusal):
                f.plan("install", state)

    def test_new_service_unit_enumeration_is_not_silently_ignored(self):
        f = PlanFixture(); state = f.prepared(); f.fresh["serviceUnitNames"].append("zzz-foreign.service")
        f.fresh["serviceEnumerationSha256"] = m.canonical_sha(f.fresh["serviceUnitNames"])
        with self.assertRaisesRegex(m.Refusal, "service_set"):
            f.plan("install", state)

    def test_caps_require_actual_prior_execution_not_an_intent(self):
        f = PlanFixture(); state = f.active(); del state["executionProof"]
        with self.assertRaisesRegex(m.Refusal, "actual_prior_execution"):
            f.plan("apply-caps", state)

    def test_apply_and_restore_caps_retain_candidate_and_have_no_rollback(self):
        f = PlanFixture(); state = f.active(); p = f.plan("apply-caps", state)
        self.assertEqual(p["desiredCaps"], f.core.TEST_CAPS); self.assertEqual(p["writePaths"], [m.CAPS_ENV, m.CAPS_DROPIN])
        f = PlanFixture(); state = f.active("caps-applied"); p = f.plan("restore-caps", state)
        self.assertEqual(p["desiredCaps"], f.core.OLD_CAPS); self.assertEqual(p["removePaths"], [m.CAPS_ENV, m.CAPS_DROPIN]); self.assertTrue(p["restorationKeepsCandidateBinary"])
        with self.assertRaises(m.Refusal):
            f.plan("rollback", state)

    def test_same_hash_owned_file_inode_and_staged_directory_swaps_rejected(self):
        for path in [m.RELEASE_ENV, m.RELEASE_DROPIN, "directory", "binary"]:
            f = PlanFixture(); state = f.active()
            target = f.candidate_path if path == "directory" else f.candidate_path + "/mir2-gateway" if path == "binary" else path
            f.fresh["pathObservations"][target]["metadata"]["inode"] += 1
            with self.assertRaises(m.Refusal):
                f.plan("apply-caps", state)

    def test_changed_release_binary_or_missing_native_gate_rejected(self):
        f = PlanFixture(); Path(f.proof["artifactDirectory"], "mir2-gateway").write_bytes(b"MZ" + b"0" * 80)
        with self.assertRaises(m.Refusal):
            f.plan()

    def test_planner_requires_v7_whole_green_and_integration_ci_mapping(self):
        for change in [lambda p: p.update(schema="mir2.scene-candidate-artifact-verification.v6"),
                       lambda p: p.update(wholeWorkflowConclusion="failure"),
                       lambda p: p.update(wholeWorkflowStatus="in_progress"),
                       lambda p: p.update(integrationCommit=p["candidateRevision"]),
                       lambda p: p["sourceCompatibility"]["canonicalNonNativeDelta"].update(nativeInputScriptReferenceExcluded=False)]:
            f = PlanFixture(); change(f.proof); f.expected["artifactProofSha256"] = m.canonical_sha(f.proof)
            with self.assertRaises(m.Refusal):
                f.plan()
        f = PlanFixture(); f.proof["nativeRequiredGates"].pop(); f.expected["artifactProofSha256"] = m.canonical_sha(f.proof)
        with self.assertRaises(m.Refusal):
            f.plan()


class ArtifactFixture:
    def __init__(self, native_reference=None):
        self.root = temporary(); self.repo = self.root / "repo"; self.repo.mkdir()
        self.git("init", "-q"); self.git("config", "user.name", "Synthetic"); self.git("config", "user.email", "synthetic@invalid"); self.git("config", "core.autocrlf", "false")
        for path in v.SOURCE_PATHS:
            write(self.repo / path, "before\n")
        for path in ["mir2-web3/packages/protocol/lib.rs", "mir2-web3/packages/game-data/lib.rs", "mir2-web3/vendor/dubhe-network-core/lib.rs", "mir2-web3/Cargo.toml", "mir2-web3/Cargo.lock"]:
            write(self.repo / path, "unchanged\n")
        workflow = "jobs:\n  build-playtest-wallet-gateway:\n    steps:\n      - run: bash scripts/build-playtest-wallet-release.sh\n"
        if native_reference == "job":
            workflow += "      - run: node " + v.NON_NATIVE_SCRIPT + "\n"
        write(self.repo / v.WORKFLOW, workflow)
        write(self.repo / "mir2-web3/scripts/build-playtest-wallet-release.sh", "native-build\n" + ("node ../" + v.NON_NATIVE_SCRIPT + "\n" if native_reference == "script" else ""))
        write(self.repo / "mir2-web3/infra/synthetic.conf", "unchanged\n")
        write(self.repo / v.NON_NATIVE_SCRIPT, "integration-numeron\n")
        self.maps = [{"path": "A.map", "content": b"synthetic-map-a"}, {"path": "B.map", "content": b"synthetic-map-b"}]
        for item in self.maps:
            path = self.repo / v.MAP_ROOT / item["path"]; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(item["content"])
        self.commit("baseline"); self.baseline = self.git("rev-parse", "HEAD").decode().strip()
        rawroot = self.root / "windows-source"; source_rows = []
        for path in sorted(v.SOURCE_PATHS):
            write(self.repo / path, "candidate\n")
            raw = b"candidate\r\n"; target = rawroot / path; target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(raw)
            source_rows.append({"path": path, "bytes": len(raw), "sha256": v.digest(raw)})
        write(self.repo / v.TOPIC_README, "synthetic topic evidence\n"); self.commit("integration"); self.integration = self.git("rev-parse", "HEAD").decode().strip()
        source = {"changesKind": "spectator-scene-projection", "baseRevision": self.baseline, "buildSourceSnapshot": str(rawroot), "files": source_rows}
        source_pin = write(self.root / "source.json", source)
        integration = {"schema": "mir2.comparable.code-integration.v1", "canonicalCommit": self.integration, "previousCanonicalHead": self.baseline, "paths": sorted(v.SOURCE_PATHS | {v.TOPIC_README}), "sourceInputManifestSha256": source_pin["sha256"], "normalizedLFSourceEqualsFrozenWindowsBuildInputs": True}
        integration_pin = write(self.root / "integration.json", integration)
        write(self.repo / v.NON_NATIVE_SCRIPT, "canonical-numeron\n"); self.commit("canonical-source"); self.revision = self.git("rev-parse", "HEAD").decode().strip()
        binary = elf(); binary_sha = v.digest(binary); tag = "synthetic-v6"; package = self.root / ("mir2-gateway-linux-x64-" + tag + ".tar.gz")
        manifest = {"name": "mir2-gateway", "gitRevision": self.revision, "gitDirty": False, "binarySha256": binary_sha, "binarySizeBytes": len(binary)}
        with tarfile.open(package, "w:gz") as archive:
            for name, content in [("mir2-gateway", binary), ("zone_host", b"synthetic-zone-never-installed"), ("README.txt", b"synthetic"), ("RELEASE.json", json.dumps(manifest).encode())]:
                item = tarfile.TarInfo(name); item.size = len(content); archive.addfile(item, io.BytesIO(content))
        maparchive = self.root / "maps.tar.gz"; maprows = []
        with tarfile.open(maparchive, "w:gz") as archive:
            for item in self.maps:
                name = "crystal-map-pack/" + item["path"]; info = tarfile.TarInfo(name); info.size = len(item["content"]); archive.addfile(info, io.BytesIO(item["content"]))
                maprows.append({"path": name, "bytes": info.size, "sha256": v.digest(item["content"])})
        maps = {"sourceRevision": self.revision, "sourceTree": self.revision + ":" + v.MAP_ROOT, "archive": maparchive.name, "archiveSha256": v.sha(maparchive), "archiveBytes": maparchive.stat().st_size, "files": maprows, "fileCount": len(maprows), "totalFileBytes": sum(r["bytes"] for r in maprows)}
        gates = {"sourceRevision": self.revision, "passed": sorted(v.GATES), "buildEnvironment": "native-linux-non-root", "productionPublish": False}
        startup = {"sourceRevision": self.revision, "binarySha256": binary_sha, "passed": True, "actualReleaseSubprocess": True, "gatewayExitCode": 0, "redisExitCode": 0}
        gateway = self.root / "gateway.zip"; native = self.root / "native.zip"
        with zipfile.ZipFile(gateway, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.write(package, package.name); archive.write(maparchive, maparchive.name)
            archive.writestr("crystal-map-pack-manifest.json", json.dumps(maps)); archive.writestr("playtest-release-verification.json", json.dumps(gates))
        with zipfile.ZipFile(native, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("gateway-postgres-startup.json", json.dumps(startup))
        artifact_rows = {"gateway": {"id": 11, "name": "mir2-playtest-wallet-gateway-linux-x64-" + self.revision, "path": gateway.name, "sha256": v.sha(gateway)}, "nativeEvidence": {"id": 12, "name": "mir2-playtest-wallet-build-evidence-" + self.revision, "path": native.name, "sha256": v.sha(native)}}
        api = {"workflowRun": {"id": 10, "head_sha": self.revision, "head_branch": v.BRANCH, "path": v.WORKFLOW, "status": "completed", "conclusion": "success"}, "nativeJob": {"id": 20, "run_id": 10, "head_sha": self.revision, "name": "build-playtest-wallet-gateway", "status": "completed", "conclusion": "success"}, "artifacts": []}
        for row in artifact_rows.values():
            api["artifacts"].append({"id": row["id"], "name": row["name"], "digest": "sha256:" + row["sha256"], "size_in_bytes": (self.root / row["path"]).stat().st_size, "expired": False, "workflow_run": {"id": 10, "head_sha": self.revision, "head_branch": v.BRANCH}})
        api_pin = write(self.root / "api.json", api)
        liverows = [{"path": r["path"].removeprefix("crystal-map-pack/"), "bytes": r["bytes"], "sha256": r["sha256"]} for r in maprows]
        live = {"schema": "mir2.scene-live-map-proof.v6", "sourceRevision": self.baseline, "binarySha256": "1" * 64, "identityBefore": ident(101), "identityAfter": ident(101), "complete": True, "readerSourceSha256": "9" * 64, "files": liverows}
        live_pin = write(self.root / "live.json", live); live_pin["readerSourceSha256"] = "9" * 64
        self.pins = {"schema": "mir2.scene-candidate-pins.v7", "templateOnly": False, "reviewedByOperator": True, "candidateSource": self.revision, "integrationCommit": self.integration, "integrationParent": self.baseline, "baselineSource": self.baseline, "baselineBinarySha256": "1" * 64, "sourceNormalization": "crlf-to-lf", "canonicalNonNativeDelta": {"path": v.NON_NATIVE_SCRIPT, "integrationSha256": v.digest(b"integration-numeron\n"), "ciSha256": v.digest(b"canonical-numeron\n")}, "runId": 10, "nativeJobId": 20, "workflow": v.WORKFLOW, "branch": v.BRANCH, "releaseTag": tag,
                     "workflowInputs": {"release_tag": tag, "playtest_wallet_no_docker": True, "publish_r2": False, "numeron_operation": "off"}, "binary": {"sha256": binary_sha, "bytes": len(binary)}, "actionsMetadata": api_pin, "sourceInputs": source_pin, "integrationEvidence": integration_pin, "liveMaps": live_pin, "artifacts": artifact_rows, "packageSha256": v.sha(package), "maps": {"fileCount": len(maprows), "totalFileBytes": maps["totalFileBytes"]}}

    def git(self, *args):
        process = subprocess.run(["git", *args], cwd=self.repo, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        if process.returncode:
            raise AssertionError("synthetic Git command failed")
        return process.stdout

    def commit(self, name):
        self.git("add", "."); self.git("commit", "-qm", name)

    def verify(self, name, pins=None):
        pins = copy.deepcopy(pins or self.pins); path = self.root / (name + "-pins.json"); write(path, pins)
        return v.verify(self.root, path, v.sha(path), self.repo, self.root / name)


class ArtifactTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.f = ArtifactFixture()

    def test_real_synthetic_zip_tar_git_lf_native_and_live_map_chain(self):
        result = self.f.verify("valid")
        self.assertTrue(result["passed"]); self.assertFalse(result["deploymentApproved"]); self.assertFalse(result["capacityQualified"])
        self.assertEqual(result["sourceCompatibility"]["integrationParent"], self.f.baseline)
        self.assertEqual(result["integrationCommit"], self.f.integration)
        self.assertEqual(result["candidateRevision"], self.f.revision)
        self.assertEqual(result["wholeWorkflowConclusion"], "success")
        self.assertEqual(result["sourceCompatibility"]["sourceNormalization"], "crlf-to-lf")
        self.assertEqual(result["maps"]["fileCount"], 2)

    def test_explicit_pins_do_not_default_source_parent_run_id_or_artifact(self):
        for key in ["candidateSource", "integrationParent", "integrationCommit", "canonicalNonNativeDelta", "baselineSource", "runId", "nativeJobId", "sourceNormalization"]:
            pins = copy.deepcopy(self.f.pins); pins.pop(key)
            with self.assertRaises(v.Refusal):
                v.validate_pins(pins)
        pins = copy.deepcopy(self.f.pins); pins["reviewedByOperator"] = False
        with self.assertRaises(v.Refusal):
            v.validate_pins(pins)

    def test_wrong_actual_parent_is_not_mistaken_for_intermediate_ui_history(self):
        pins = copy.deepcopy(self.f.pins); pins["integrationParent"] = "f" * 40
        with self.assertRaisesRegex(v.Refusal, "actual_single_parent"):
            self.f.verify("bad-parent", pins)

    def test_api_job_or_artifact_digest_mismatch_refused_before_extract(self):
        original = json.loads((self.f.root / "api.json").read_bytes())
        for i, change in enumerate([lambda a: a["nativeJob"].update(conclusion="failure"), lambda a: a["artifacts"][0].update(digest="sha256:" + "0" * 64), lambda a: a["workflowRun"].update(head_sha="f" * 40)]):
            bad = copy.deepcopy(original); change(bad); pin = write(self.f.root / ("api-bad-" + str(i) + ".json"), bad)
            pins = copy.deepcopy(self.f.pins); pins["actionsMetadata"] = pin
            with self.assertRaises(v.Refusal):
                self.f.verify("bad-api-" + str(i), pins)
            self.assertFalse((self.f.root / ("bad-api-" + str(i))).exists())

    def test_failed_whole_workflow_cannot_qualify_a_successful_native_job(self):
        api = json.loads((self.f.root / "api.json").read_bytes())
        api["workflowRun"].update(status="completed", conclusion="failure")
        pins = copy.deepcopy(self.f.pins)
        pins["actionsMetadata"] = write(self.f.root / "api-whole-failed.json", api)
        with self.assertRaisesRegex(v.Refusal, "whole_workflow"):
            self.f.verify("whole-run-failed", pins)

    def test_native_success_while_whole_workflow_in_progress_is_not_final(self):
        api = json.loads((self.f.root / "api.json").read_bytes())
        api["workflowRun"].update(status="in_progress", conclusion=None)
        pins = copy.deepcopy(self.f.pins); pins["actionsMetadata"] = write(self.f.root / "api-whole-pending.json", api)
        with self.assertRaisesRegex(v.Refusal, "whole_workflow"):
            self.f.verify("whole-run-pending", pins)

    def test_green_numeron_workflow_with_skipped_native_job_is_refused(self):
        api = json.loads((self.f.root / "api.json").read_bytes())
        api["nativeJob"].update(conclusion="skipped")
        pins = copy.deepcopy(self.f.pins); pins["actionsMetadata"] = write(self.f.root / "api-native-skipped.json", api)
        with self.assertRaisesRegex(v.Refusal, "native_job_not_exact_success"):
            self.f.verify("native-skipped", pins)

    def test_metadata_must_bind_actual_ci_source_not_integration_commit(self):
        api = json.loads((self.f.root / "api.json").read_bytes())
        api["workflowRun"].update(head_sha=self.f.integration)
        pins = copy.deepcopy(self.f.pins); pins["actionsMetadata"] = write(self.f.root / "api-integration-instead-ci.json", api)
        with self.assertRaisesRegex(v.Refusal, "run_metadata_mismatch"):
            self.f.verify("wrong-ci-source", pins)

    def test_non_native_script_hashes_must_be_explicit_not_any_descendant(self):
        for key in ["integrationSha256", "ciSha256"]:
            pins = copy.deepcopy(self.f.pins); pins["canonicalNonNativeDelta"][key] = "0" * 64
            with self.assertRaisesRegex(v.Refusal, "old_new_hash_mismatch"):
                self.f.verify("bad-numeron-hash-" + key, pins)
        tree = self.f.git("rev-parse", self.f.revision + "^{tree}").decode().strip()
        descendant = self.f.git("commit-tree", tree, "-p", self.f.revision, "-m", "synthetic extra descendant").decode().strip()
        pins = copy.deepcopy(self.f.pins); pins["candidateSource"] = descendant
        with self.assertRaisesRegex(v.Refusal, "exact_direct_child"):
            v.non_native_delta(self.f.repo, pins)

    def test_any_unlisted_backend_or_build_input_delta_is_rejected(self):
        original = v.git
        for added in ["mir2-web3/Cargo.lock", "mir2-web3/apps/gateway/src/config.rs", "mir2-web3/scripts/build-playtest-wallet-release.sh"]:
            def query(repo, *args):
                if args[:2] == ("diff", "--name-only"):
                    return (v.NON_NATIVE_SCRIPT + "\n" + added + "\n").encode()
                return original(repo, *args)
            with patch.object(v, "git", side_effect=query), self.assertRaisesRegex(v.Refusal, "only_exact_reviewed_non_native"):
                v.non_native_delta(self.f.repo, self.f.pins)

    def test_changed_script_is_rejected_if_native_job_or_shell_could_call_it(self):
        for location, expected in [("job", "native_job_must_not_invoke"), ("script", "referenced_by_native_inputs")]:
            fixture = ArtifactFixture(native_reference=location)
            with self.assertRaisesRegex(v.Refusal, expected):
                fixture.verify("native-reference-" + location)

    def test_wrong_raw_source_pin_or_unapproved_normalization_rejected(self):
        pins = copy.deepcopy(self.f.pins); pins["sourceNormalization"] = "exact"
        with self.assertRaisesRegex(v.Refusal, "source_blob"):
            self.f.verify("wrong-normalization", pins)
        path = self.f.root / "windows-source" / sorted(v.SOURCE_PATHS)[0]; original = path.read_bytes()
        try:
            path.write_bytes(original + b" ")
            with self.assertRaisesRegex(v.Refusal, "raw_windows_source"):
                self.f.verify("wrong-raw-source")
        finally:
            path.write_bytes(original)

    def test_mismatched_actual_native_startup_is_not_passed_by_metadata(self):
        source = self.f.root / "native.zip"; badzip = self.f.root / "native-bad.zip"
        with zipfile.ZipFile(source) as original, zipfile.ZipFile(badzip, "w") as archive:
            data = json.loads(original.read("gateway-postgres-startup.json")); data["gatewayExitCode"] = 1
            archive.writestr("gateway-postgres-startup.json", json.dumps(data))
        pins = copy.deepcopy(self.f.pins); row = pins["artifacts"]["nativeEvidence"]; row.update(path=badzip.name, sha256=v.sha(badzip))
        api = json.loads((self.f.root / "api.json").read_bytes()); api["artifacts"][1].update(digest="sha256:" + row["sha256"], size_in_bytes=badzip.stat().st_size)
        pins["actionsMetadata"] = write(self.f.root / "api-native-bad.json", api)
        with self.assertRaisesRegex(v.Refusal, "actual_native_startup"):
            self.f.verify("bad-native-startup", pins)
        self.assertFalse((self.f.root / "bad-native-startup/candidate-verification.json").exists())

    def test_zip_traversal_and_tar_link_are_never_extracted(self):
        root = temporary(); source = root / "unsafe.zip"
        with zipfile.ZipFile(source, "w") as archive:
            archive.writestr("../escape", b"no")
        with self.assertRaises(v.Refusal):
            v.unzip(source, root / "zip-output")
        self.assertFalse((root / "escape").exists())
        source = root / "unsafe.tar.gz"
        with tarfile.open(source, "w:gz") as archive:
            entry = tarfile.TarInfo("mir2-gateway"); entry.type = tarfile.SYMTYPE; entry.linkname = "/etc/passwd"; archive.addfile(entry)
        with self.assertRaises(v.Refusal):
            v.unpack_release(source, root / "tar-output")

    def test_lfs_map_pointer_is_not_a_collision_map(self):
        path = self.f.repo / v.MAP_ROOT / "A.map"; path.write_text("version https://git-lfs.github.com/spec/v1\noid sha256:" + "0" * 64 + "\nsize 42\n", encoding="utf8")
        self.f.commit("synthetic-lfs-negative"); revision = self.f.git("rev-parse", "HEAD").decode().strip()
        with self.assertRaisesRegex(v.Refusal, "lfs_pointer"):
            v.git_maps(self.f.repo, revision)


if __name__ == "__main__":
    unittest.main()
