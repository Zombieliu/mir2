"""Bound a large offline evidence check to a separate 8-CPU / 4-GiB Job.

This launches only a new verifier. It has no game control or network operation.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time

from windows_service_job import SharedServiceJob


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--plan", type=Path, required=True)
    p.add_argument("--case", type=Path, required=True)
    p.add_argument("--verifier", type=Path, required=True)
    p.add_argument("--job-output", type=Path, required=True)
    p.add_argument("--result-name", default="independent-verification.v3.json")
    a = p.parse_args()
    case = a.case.resolve(strict=True)
    root = a.job_output.resolve()
    if root.drive.upper() != "C:" or root.exists() or Path(a.result_name).name != a.result_name:
        raise ValueError("Fresh C-only Job output and simple new result filename required")
    result_path = case / a.result_name
    if result_path.exists():
        raise ValueError("Verification result already exists")
    root.mkdir(parents=True, exist_ok=False)
    command = [sys.executable, "-B", str(a.verifier.resolve(strict=True)), "--plan", str(a.plan.resolve(strict=True)),
               "--summary", str(case / "summary.json"), "--events", str(case / "events.jsonl"),
               "--controller", str(case / "controller-result.json"), "--resources", str(case / "resources.jsonl"),
               "--injector", str(case / "injector-resources.jsonl"), "--disk-readback", str(case / "disk-readback.json"),
               "--output", str(result_path)]
    job = SharedServiceJob(cpus=8, memory_bytes=4 * 2**30, affinity_mask=0xff0)
    began = time.monotonic()
    child = job.launch("verifier", command, str(case), str(root / "logs"))
    samples = []
    with (root / "resources.jsonl").open("x", encoding="utf-8") as f:
        while not child.exited():
            if time.monotonic() - began > 600:
                # Exiting this controller closes only this newly owned Job.
                # Such termination is explicitly a failed verifier, never a pass.
                (root / "timeout-failure.json").write_text(json.dumps({"passed": False, "ownedVerifierPid": child.pid,
                    "reason": "offline-check-wall-expired", "normalExit": False}), encoding="utf-8")
                raise TimeoutError("Offline verifier exceeded finite wall")
            row = job.sample(required_roles=["verifier"], declared_dependencies=[])
            samples.append(row)
            f.write(json.dumps(row, separators=(",", ":")) + "\n")
            f.flush()
            time.sleep(.5)
    code = child.exit_code()
    deadline = time.monotonic() + 5
    final = job.sample(required_roles=[], declared_dependencies=[])
    while final["jobActiveProcesses"] and time.monotonic() < deadline:
        time.sleep(.05)
        final = job.sample(required_roles=[], declared_dependencies=[])
    closed = job.close()
    result = {"schema": "mir2.comparable.offline-check-job.v1", "actualExitCode": code,
              "elapsedSeconds": time.monotonic() - began, "peakCommitBytes": final["jobPeakCommitBytes"],
              "requestedLimits": job.requested, "finalLimits": final["effectiveLimits"],
              "liveSampleCount": len(samples), "allOwnedMembersExited": closed["allMembersExited"],
              "killAppliedByClose": closed["killAppliedByClose"],
              "verificationResultSha256": hashlib.sha256(result_path.read_bytes()).hexdigest() if result_path.is_file() else None,
              "sourceSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    with (root / "job-result.json").open("x", encoding="utf-8") as f:
        json.dump(result, f, indent=2)
    print(json.dumps(result))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
