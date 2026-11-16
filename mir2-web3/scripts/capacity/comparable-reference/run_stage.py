"""Run one isolated local stage under one aggregate server Job and a separate injector Job.

The input specification is private. Only newly launched descendants are managed.
Every server uses service_host.py for normal shutdown; unrelated services are never touched.
"""
from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import socket
import sys
import struct
import time

from service_host import write_new
from windows_service_job import SharedServiceJob
from adapter_source_binding import adapter_sources_proof

SCHEMA = "mir2.comparable.stage-controller.v1"
INTERVAL_MS = 1000
MAX_GAP_MS = 2500  # Declared before collecting data; resource evidence only.


def ipv4_listeners() -> set[tuple[str, int, int]]:
    """Read listening sockets and owner PIDs without opening a game connection."""
    query = ctypes.WinDLL("iphlpapi", use_last_error=True).GetExtendedTcpTable
    query.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.DWORD), wintypes.BOOL,
                      wintypes.ULONG, ctypes.c_int, wintypes.ULONG]
    query.restype = wintypes.DWORD
    length = wintypes.DWORD()
    code = query(None, ctypes.byref(length), True, 2, 3, 0)  # AF_INET, OWNER_PID_LISTENER
    if code not in (0, 122) or not 4 <= length.value <= 16 * 1024 * 1024:
        raise OSError(code, "Listening socket inventory unavailable")
    for _ in range(3):
        data = ctypes.create_string_buffer(length.value)
        code = query(data, ctypes.byref(length), True, 2, 3, 0)
        if code == 122:
            if length.value > 16 * 1024 * 1024:
                break
            continue
        if code:
            raise OSError(code, "Listening socket inventory query failed")
        raw = data.raw[:length.value]
        count = struct.unpack_from("<I", raw)[0]
        if 4 + count * 24 > len(raw):
            raise RuntimeError("Incomplete listening socket inventory")
        rows = set()
        for offset in range(4, 4 + count * 24, 24):
            state, address, port, _, _, pid = struct.unpack_from("<6I", raw, offset)
            if state != 2:
                raise RuntimeError("Non-listening row in listener inventory")
            rows.add((socket.inet_ntoa(struct.pack("<I", address)), socket.ntohs(port & 0xffff), pid))
        return rows
    raise RuntimeError("Listening socket inventory changed repeatedly")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(spec: dict, root: Path) -> None:
    if spec.get("schema") != "mir2.comparable.stage-spec.v1" or spec.get("engine") not in ("crystal", "modern"):
        raise ValueError("Explicit local stage specification required")
    plan_path = Path(spec["plan"])
    if digest(plan_path) != spec["planSha256"]:
        raise ValueError("Plan hash mismatch")
    plan = json.loads(plan_path.read_text(encoding="utf-8-sig"))
    if plan.get("qualificationLayer") != "game-core-local" or plan.get("capacityClaimAllowed") is not False:
        raise ValueError("This controller is game-core-local only")
    if root.drive.upper() != "C:" or not root.is_dir():
        raise ValueError("Fresh C-only run root required")
    for name in ("controller-result.json", "resources.jsonl", "injector-resources.jsonl", "controller-stop.txt", "service-started.json", "service-result.json", "service-private.json"):
        if (root / name).exists():
            raise ValueError("Run receipt already exists: " + name)
    if not 60 <= spec.get("wallSeconds", 0) <= 7200:
        raise ValueError("Finite wall limit required")
    ports = spec["ready"]["ports"]
    if not ports or len(set(ports)) != len(ports) or any(type(p) is not int or not 1024 <= p <= 65535 for p in ports):
        raise ValueError("Explicit distinct unprivileged loopback ports required")
    for port in ports:
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", port))
    ready_file = spec["ready"].get("file")
    if ready_file and Path(ready_file).exists():
        raise ValueError("Readiness file must be fresh")
    if set(spec["driver"]["outputs"]) != {"summary", "events"}:
        raise ValueError("Predeclared driver summary and events required")
    for output in spec["driver"]["outputs"].values():
        path = Path(output).resolve()
        if path.exists() or not path.is_relative_to(root):
            raise ValueError("Fresh driver evidence within owned run root required")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--run-root", type=Path, required=True)
    args = parser.parse_args()
    root = args.run_root.resolve(strict=True)
    spec = json.loads(args.spec.read_text(encoding="utf-8-sig"))
    validate(spec, root)
    source_proof = adapter_sources_proof(spec)
    origin = time.monotonic()
    stamp = lambda: (time.monotonic() - origin) * 1000
    result = {"schema": SCHEMA, "engine": spec["engine"], "qualificationLayer": "game-core-local",
              "planSha256": spec["planSha256"], "resourceNominalIntervalMs": INTERVAL_MS,
              "resourceSampleMaxGapMs": MAX_GAP_MS, "requiredRoles": ["gameServer"], "declaredDependencies": [],
              "readyAtMs": None, "driverStartedAtMs": None, "driverExitedAtMs": None,
              "normalShutdownStartedAtMs": None, "resourcesRowCount": 0, "resourceEvidenceComplete": False,
              "actualExitCode": 1, "driverActualExitCode": None, "serverActualExitCode": None,
              "normalStopProven": False, "capacityQualified": False, "capacityClaimAllowed": False,
              "driverEvidence": {},
              "osAndInjectorExcludedFromServerBudget": True, "abnormalCleanup": False,
              "controllerSourceSha256": digest(Path(__file__)),
              "serviceHostSourceSha256": digest(Path(__file__).with_name("service_host.py")),
              "resourceHelperSourceSha256": digest(Path(__file__).with_name("windows_service_job.py"))}
    result["stageSpecificationSha256"] = digest(args.spec)
    if source_proof is not None:
        result.update(source_proof)
        result["adapterSourceBindingStable"] = False
    service_spec = dict(spec["server"], schema="mir2.comparable.service-host.v1",
                        wallSeconds=spec["wallSeconds"], stopFile=str(root / "controller-stop.txt"))
    service_path = root / "service-private.json"
    write_new(service_path, service_spec)
    server_job = SharedServiceJob()
    injector_job = None
    server = driver = None
    resource_rows = []
    resource_file = (root / "resources.jsonl").open("x", encoding="utf-8")
    injector_file = (root / "injector-resources.jsonl").open("x", encoding="utf-8")
    def sample_server():
        row = server_job.sample(required_roles=["gameServer"], declared_dependencies=[])
        row["resourceSampleAtMs"] = stamp()
        resource_rows.append(row)
        resource_file.write(json.dumps(row, separators=(",", ":")) + "\n")
        resource_file.flush()
        return row["resourceQualification"]["qualified"]
    try:
        server = server_job.launch("gameServer", [sys.executable, "-B", str(Path(__file__).with_name("service_host.py")),
                                "--spec", str(service_path), "--run-root", str(root)], str(root), str(root / "wrapper-logs"))
        result["gameJobRootPid"] = server.pid
        ready_until = time.monotonic() + 120
        while True:
            if server.exited():
                raise RuntimeError("Owned server exited before readiness")
            if time.monotonic() > ready_until:
                raise RuntimeError("Readiness deadline exceeded")
            file = spec["ready"].get("file")
            if (not file or Path(file).is_file()) and (root / "service-started.json").is_file():
                child = json.loads((root / "service-started.json").read_text(encoding="utf-8"))
                listeners = ipv4_listeners()
                if all(("127.0.0.1", port, child["gamePid"]) in listeners for port in spec["ready"]["ports"]):
                    result["gamePid"] = child["gamePid"]
                    result["serviceStartedSha256"] = digest(root / "service-started.json")
                    result["readinessMode"] = "read-only-exact-owned-ipv4-listener"
                    break
            time.sleep(0.1)
        result["readyAtMs"] = stamp()
        if not sample_server():
            raise RuntimeError("Resource gate failed before injector admission")
        injector_job = SharedServiceJob(cpus=8, memory_bytes=4 * 2**30, affinity_mask=0xff0)
        if adapter_sources_proof(spec) != source_proof:
            raise RuntimeError("Adapter source binding changed before launch")
        result["driverStartedAtMs"] = stamp()
        driver = injector_job.launch("injector", spec["driver"]["command"], spec["driver"]["cwd"], str(root / "injector-logs"))
        result["injectorJobRootPid"] = driver.pid
        next_sample = time.monotonic()
        while not driver.exited():
            if server.exited():
                raise RuntimeError("Server exited during injector activity")
            if time.monotonic() - origin > spec["wallSeconds"]:
                raise RuntimeError("Controller wall expired while injector remained live")
            if time.monotonic() >= next_sample:
                sample_server()
                row = injector_job.sample(required_roles=["injector"], declared_dependencies=[])
                row["resourceSampleAtMs"] = stamp()
                injector_file.write(json.dumps(row, separators=(",", ":")) + "\n")
                injector_file.flush()
                next_sample = time.monotonic() + INTERVAL_MS / 1000
            time.sleep(0.05)
        result["driverExitedAtMs"] = stamp()
        result["driverActualExitCode"] = driver.exit_code()
        if source_proof is not None:
            result["adapterSourceBindingStable"] = adapter_sources_proof(spec) == source_proof
            if not result["adapterSourceBindingStable"]:
                raise RuntimeError("Adapter source binding changed during activity")
        for name, output in spec["driver"]["outputs"].items():
            path = Path(output)
            if path.is_file():
                result["driverEvidence"][name] = {"relativePath": str(path.relative_to(root)),
                    "sha256": digest(path), "bytes": path.stat().st_size}
        sample_server()
        gaps = [b["resourceSampleAtMs"] - a["resourceSampleAtMs"] for a, b in zip(resource_rows, resource_rows[1:])]
        result["resourceEvidenceComplete"] = (len(resource_rows) >= 2
            and all(r["resourceQualification"]["qualified"] for r in resource_rows)
            and max(gaps, default=0) <= MAX_GAP_MS
            and resource_rows[0]["resourceSampleAtMs"] <= result["driverStartedAtMs"]
            and resource_rows[-1]["resourceSampleAtMs"] >= result["driverExitedAtMs"])
        result["resourceObservedMaxGapMs"] = max(gaps, default=0)
    except BaseException as error:
        # No argv/environment/credential-bearing exception text is exported.
        result["errorClass"] = type(error).__name__
        result["errorCode"] = str(error) if isinstance(error, RuntimeError) else "controller-failed"
    finally:
        result["normalShutdownStartedAtMs"] = stamp()
        if server is not None and not server.exited():
            with (root / "controller-stop.txt").open("x", encoding="utf-8") as stream:
                stream.write("owned controller requests ordinary shutdown\n")
            until = time.monotonic() + 60
            while not server.exited() and time.monotonic() < until:
                time.sleep(0.1)
        if server is not None:
            result["serverActualExitCode"] = server.exit_code()
        service_result = root / "service-result.json"
        if service_result.is_file():
            sr = json.loads(service_result.read_text(encoding="utf-8"))
            result["normalStopProven"] = (sr["actualExitCode"] == 0 and sr["stopReason"] == "controller-marker"
                                           and sr["forceKillUsed"] is False and result["serverActualExitCode"] == 0)
        for name, job in (("serverJobClose", server_job), ("injectorJobClose", injector_job)):
            if job is not None:
                try:
                    drain_until = time.monotonic() + 5
                    while job.sample(required_roles=[], declared_dependencies=[])["jobActiveProcesses"] and time.monotonic() < drain_until:
                        time.sleep(0.05)
                    result[name] = job.close()
                except RuntimeError:
                    result[name] = {"closed": False, "liveMembersRemain": True}
                    result["abnormalCleanup"] = True
        resource_file.close()
        injector_file.close()
        result["resourcesRowCount"] = len(resource_rows)
        result["resourcesSha256"] = digest(root / "resources.jsonl")
        result["injectorResourcesSha256"] = digest(root / "injector-resources.jsonl")
        result["actualExitCode"] = 0 if (result["driverActualExitCode"] == 0 and result["normalStopProven"]
                                       and result["resourceEvidenceComplete"] and not result["abnormalCleanup"]
                                       and result.get("adapterSourceBindingStable", True)) else 1
        result["elapsedMs"] = stamp()
        write_new(root / "controller-result.json", result)
    print(json.dumps({key: result[key] for key in ("engine", "actualExitCode", "driverActualExitCode", "serverActualExitCode", "normalStopProven", "resourceEvidenceComplete", "resourcesRowCount", "elapsedMs")}))
    return result["actualExitCode"]


if __name__ == "__main__":
    raise SystemExit(main())
