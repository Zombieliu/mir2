"""Offline gates plus two tiny caller-owned Windows children; no game load.

Run with python -B -m unittest discover -s <this directory> -p
test_windows_service_job.py -v. Native test output stays in a new directory under
C:/.../Temp/mir2-comparable-capacity-20261113/shared-job-tests. It is retained.
The only real Job has 256 MiB aggregate commit and two finite Python children.
"""

from __future__ import annotations

import ast
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest import mock


SOURCE = Path(__file__).with_name("windows_service_job.py")
SPEC = importlib.util.spec_from_file_location("shared_service_job_test_subject", SOURCE)
jobmod = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(jobmod)


def example_sample():
    requested = jobmod.validate_budget(host_cpus=32)
    return {
        "requestedLimits": requested,
        "effectiveLimits": {**requested, "limitFlags": jobmod.LIMIT_FLAGS,
                            "cpuControlFlags": jobmod.CPU_FLAGS},
        "membershipStable": True, "jobActiveProcesses": 2,
        "jobPeakCommitBytes": 120 * 2**20, "memberPids": [101, 102],
        "members": [
            {"pid": 101, "parentPid": 90, "role": "gateway", "inOwnedJob": True,
             "queryError": None, "affinityMaskHex": "0xf", "creationTime100ns": 1001},
            {"pid": 102, "parentPid": 101, "role": "gateway", "inOwnedJob": True,
             "queryError": None, "affinityMaskHex": "0xf", "creationTime100ns": 1002}],
        "roles": {"gateway": 101},
        "roots": [{"role": "gateway", "pid": 101, "assignedBeforeResume": True,
                   "exited": False, "creationTime100ns": 1001}],
        "failedLaunches": [],
    }


def example_with_dependency():
    sample = example_sample()
    sample["jobActiveProcesses"] = 3
    sample["memberPids"].append(201)
    sample["members"].append({"pid": 201, "parentPid": 90, "role": "postgres",
                              "inOwnedJob": True, "queryError": None,
                              "affinityMaskHex": "0xf", "creationTime100ns": 2001})
    sample["roles"]["postgres"] = 201
    sample["roots"].append({"role": "postgres", "pid": 201, "assignedBeforeResume": True,
                            "exited": False, "creationTime100ns": 2001})
    return sample


class ResourceGateTests(unittest.TestCase):
    def gate(self, sample, **kwargs):
        return jobmod.assess_resource_sample(sample, required_roles=["gateway"],
                                            declared_dependencies=[], **kwargs)

    def test_budget_and_independent_injector(self):
        budget = jobmod.validate_budget(host_cpus=32)
        self.assertEqual((budget["affinityMaskHex"], budget["cpuRatePer10000"],
                          budget["jobCommitLimitBytes"]), ("0xf", 1250, 8 * 2**30))
        injector = jobmod.validate_budget(8, 4 * 2**30, 0xff0, host_cpus=32)
        self.assertEqual((injector["cpuRatePer10000"], injector["affinityMaskHex"]),
                         (2500, "0xff0"))
        self.assertEqual(int(budget["affinityMaskHex"], 16) & int(injector["affinityMaskHex"], 16), 0)

    def test_invalid_budgets_fail(self):
        for kwargs in [{"cpus": True}, {"cpus": 0}, {"cpus": 33},
                       {"memory_bytes": -1}, {"memory_bytes": False},
                       {"affinity_mask": 3}, {"affinity_mask": 0x10000000f},
                       {"host_cpus": 65}, {"host_cpus": False}]:
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                jobmod.validate_budget(**{"host_cpus": 32, **kwargs})

    def test_complete_owned_tree_qualifies_only_resource_envelope(self):
        gate = self.gate(example_sample())
        self.assertTrue(gate["qualified"])
        self.assertFalse(gate["capacityQualified"])
        self.assertFalse(jobmod.assess_resource_sample(example_sample())["qualified"])

    def test_external_dependency_rejected(self):
        sample = example_sample()
        result = jobmod.assess_resource_sample(sample, required_roles=["gateway"],
                  declared_dependencies=[{"role": "postgres", "pid": 999}])
        self.assertIn("declared_dependency_outside_owned_job:postgres", result["reasons"])
        inside = jobmod.assess_resource_sample(sample, required_roles=["gateway"],
                  declared_dependencies=[{"role": "gateway", "pid": 101}])
        self.assertTrue(inside["qualified"])
        with self.assertRaises(ValueError):
            jobmod.assess_resource_sample(sample, required_roles=["gateway"],
                                         declared_dependencies=[{"role": "bad role", "pid": 1}])

    def test_dependency_role_and_root_must_match_actual_member(self):
        valid = jobmod.assess_resource_sample(example_with_dependency(), required_roles=["gateway"],
                  declared_dependencies=[{"role": "postgres", "pid": 201}])
        self.assertTrue(valid["qualified"])
        for dependency in [{"role": "postgres", "pid": 101},
                           {"role": "postgres", "pid": 102},
                           {"role": "gateway", "pid": 102}]:
            with self.subTest(dependency=dependency):
                result = jobmod.assess_resource_sample(example_sample(), required_roles=["gateway"],
                                                      declared_dependencies=[dependency])
                self.assertFalse(result["qualified"])
                self.assertIn("declared_dependency_identity_unverified:" + dependency["role"], result["reasons"])

    def test_dependency_root_lifecycle_and_owned_membership_required(self):
        for change in [lambda s: s["roots"][1].update(assignedBeforeResume=False),
                       lambda s: s["roots"][1].update(exited=True),
                       lambda s: s["roots"][1].update(creationTime100ns=2000),
                       lambda s: s["roles"].update(postgres=101),
                       lambda s: s["roots"].pop(),
                       lambda s: s["members"][2].update(inOwnedJob=False),
                       lambda s: s["members"][2].update(queryError={"winerror": 5})]:
            sample = example_with_dependency(); change(sample)
            result = jobmod.assess_resource_sample(sample, required_roles=["gateway"],
                                                  declared_dependencies=[{"role": "postgres", "pid": 201}])
            with self.subTest(sample=sample):
                self.assertFalse(result["qualified"])
                self.assertIn("declared_dependency_identity_unverified:postgres", result["reasons"])

    def test_dependency_duplicates_and_same_pid_alias_rejected(self):
        for second in [{"role": "postgres", "pid": 201},
                       {"role": "redis", "pid": 201},
                       {"role": "postgres", "pid": 101}]:
            result = jobmod.assess_resource_sample(example_with_dependency(), required_roles=["gateway"],
                      declared_dependencies=[{"role": "postgres", "pid": 201}, second])
            with self.subTest(second=second):
                self.assertFalse(result["qualified"])
                self.assertIn("duplicate_dependency_role_or_pid:" + second["role"], result["reasons"])

    def test_unknown_and_racing_members_never_zero_filled(self):
        for change in [lambda s: s.update(membershipStable=False),
                       lambda s: s.update(jobActiveProcesses=1),
                       lambda s: s.update(memberPids=[101]),
                       lambda s: s["members"][1].update(inOwnedJob=None),
                       lambda s: s["members"][1].update(queryError={"winerror": 5}),
                       lambda s: s["members"][1].update(role=None),
                       lambda s: s["members"][1].pop("creationTime100ns"),
                       lambda s: s["members"][1].pop("affinityMaskHex"),
                       lambda s: s["members"][1].update(pid=101),
                       lambda s: s.update(jobPeakCommitBytes=None)]:
            sample = example_sample(); change(sample)
            with self.subTest(sample=sample):
                self.assertFalse(self.gate(sample)["qualified"])

    def test_effective_limits_aggregate_peak_and_breakaway_rejected(self):
        for change in [lambda s: s["effectiveLimits"].update(cpuRatePer10000=10000),
                       lambda s: s["effectiveLimits"].update(cpuControlFlags=1),
                       lambda s: s["effectiveLimits"].update(limitFlags=jobmod.LIMIT_FLAGS | 0x800),
                       lambda s: s["effectiveLimits"].update(limitFlags=0x2000),
                       lambda s: s["effectiveLimits"].update(jobCommitLimitBytes=16 * 2**30),
                       lambda s: s["members"][0].update(affinityMaskHex="0xff"),
                       lambda s: s.update(jobPeakCommitBytes=8 * 2**30 + 1)]:
            sample = example_sample(); change(sample)
            with self.subTest(sample=sample):
                self.assertFalse(self.gate(sample)["qualified"])

    def test_root_lifecycle_and_failed_launch_sticky(self):
        for change in [lambda s: s["roots"][0].update(assignedBeforeResume=False),
                       lambda s: s["roots"][0].update(exited=True),
                       lambda s: s["roots"][0].update(creationTime100ns=999),
                       lambda s: s["members"][0].update(role="other"),
                       lambda s: s.update(failedLaunches=[{"preResumeFailure": True}])]:
            sample = example_sample(); change(sample)
            self.assertFalse(self.gate(sample)["qualified"])

    def test_launch_order_no_attach_and_portable_refusal(self):
        tree = ast.parse(SOURCE.read_text(encoding="utf-8"))
        cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "SharedServiceJob")
        self.assertFalse(any(isinstance(n, ast.FunctionDef) and "attach" in n.name for n in cls.body))
        launch = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == "launch")
        calls = [n for n in ast.walk(launch) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)]
        line = lambda name: min(n.lineno for n in calls if n.func.attr == name)
        self.assertLess(line("CreateProcessW"), line("AssignProcessToJobObject"))
        self.assertLess(line("AssignProcessToJobObject"), line("ResumeThread"))
        create = next(n for n in calls if n.func.attr == "CreateProcessW")
        self.assertIsInstance(create.args[5], ast.BinOp)
        self.assertIsInstance(create.args[5].op, ast.BitOr)
        self.assertEqual((create.args[5].left.value, create.args[5].right.value), (4, 0x08000000))
        with mock.patch.object(jobmod.os, "name", "posix"):
            with self.assertRaisesRegex(OSError, "Native Windows Job runtime required"):
                jobmod.SharedServiceJob()
            with self.assertRaises(SystemExit) as caught:
                jobmod.main([])
            self.assertEqual(caught.exception.code, 2)


CHILD = r'''import json, os, sys, time
from pathlib import Path
ready, stop, allocate = (Path(v) for v in sys.argv[1:4])
with ready.with_suffix(".started").open("x", encoding="utf-8") as out:
    json.dump({"pid": os.getpid()}, out)
deadline = time.monotonic() + 12
while not allocate.exists() and not stop.exists() and time.monotonic() < deadline: time.sleep(.02)
if not allocate.exists(): sys.exit(0 if stop.exists() else 3)
request = 160 * 2**20
try:
    hold = bytearray(request)
    for i in range(0, request, 4096): hold[i] = 1
    allocated = True
except MemoryError:
    hold = None
    allocated = False
with ready.open("x", encoding="utf-8") as out:
    json.dump({"allocationSucceeded": allocated, "requestBytes": request, "pid": os.getpid()}, out)
while not stop.exists() and time.monotonic() < deadline: time.sleep(.02)
sys.exit(0 if stop.exists() else 3)
'''


@unittest.skipUnless(os.name == "nt", "Native Windows quota test")
class NativeSharedJobTests(unittest.TestCase):
    def test_two_processes_share_affinity_and_aggregate_memory(self):
        base = Path(tempfile.gettempdir()) / "mir2-comparable-capacity-20261113" / "shared-job-tests"
        self.assertEqual(base.resolve().drive.upper(), "C:")
        base.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix="native-shared-", dir=base))
        script = directory / "owned_child.py"
        script.write_text(CHILD, encoding="utf-8")
        stop = directory / "normal-stop"
        evidence = {"schema": "mir2.shared-service-job-test.v1", "directory": str(directory),
                    "sourceSha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
                    "testSourceSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                    "gameLoadRun": False, "pythonRootCount": 2, "aggregateCommitLimitBytes": 256 * 2**20}
        job = jobmod.SharedServiceJob(memory_bytes=256 * 2**20)
        owned = []
        def read_ready(path):
            deadline = time.monotonic() + 6
            while time.monotonic() < deadline:
                if path.exists():
                    try: return json.loads(path.read_text(encoding="utf-8"))
                    except json.JSONDecodeError: pass
                time.sleep(.02)
            self.fail("Owned child did not publish bounded ready result")
        try:
            # Two .launch calls prove both new service roots enter the same Job.
            first = job.launch("first", [sys.executable, "-B", str(script), str(directory / "first.json"), str(stop), str(directory / "first-allocate"), "argv-privacy-sentinel"], directory, directory / "first-logs")
            owned.append(first)
            read_ready(directory / "first.started")
            second = job.launch("second", [sys.executable, "-B", str(script), str(directory / "second.json"), str(stop), str(directory / "second-allocate")], directory, directory / "second-logs")
            owned.append(second)
            read_ready(directory / "second.started")
            sample = job.sample(required_roles=["first", "second"], declared_dependencies=[])
            evidence["beforePressureSample"] = sample
            self.assertGreaterEqual(sample["jobActiveProcesses"], 2)
            self.assertEqual(sample["jobActiveProcesses"], len(sample["members"]))
            self.assertTrue({first.pid, second.pid}.issubset(sample["memberPids"]))
            self.assertEqual(len(sample["roots"]), 2)
            self.assertTrue(sample["resourceQualification"]["qualified"], sample["resourceQualification"])
            self.assertEqual(sample["effectiveLimits"]["cpuRatePer10000"], round(40000 / sample["requestedLimits"]["hostLogicalCpus"]))
            self.assertEqual({row["affinityMaskHex"] for row in sample["members"]}, {"0xf"})
            self.assertTrue(all(p.assigned_before_resume for p in owned))
            self.assertLessEqual(sample["jobPeakCommitBytes"], 256 * 2**20)
            self.assertNotIn("argv-privacy-sentinel", json.dumps(sample))
            external = job.sample(required_roles=["first", "second"], declared_dependencies=[{"role": "external", "pid": os.getpid()}])
            self.assertFalse(external["resourceQualification"]["qualified"])
            evidence["externalDependencyRejected"] = external["resourceQualification"]
            with self.assertRaisesRegex(RuntimeError, "caller must normal-stop"):
                job.close()
            self.assertTrue(all(not p.exited() for p in owned))
            evidence["closeRefusedWhileLive"] = True
            (directory / "first-allocate").touch(exist_ok=False)
            evidence["firstAllocation"] = read_ready(directory / "first.json")
            self.assertTrue(evidence["firstAllocation"]["allocationSucceeded"])
            (directory / "second-allocate").touch(exist_ok=False)
            evidence["secondAllocation"] = read_ready(directory / "second.json")
            self.assertFalse(evidence["secondAllocation"]["allocationSucceeded"])
            pressure = job.sample(required_roles=["first", "second"], declared_dependencies=[])
            evidence["afterRejectedAllocationSample"] = pressure
            self.assertEqual(pressure["effectiveLimits"]["jobCommitLimitBytes"], 256 * 2**20)
            self.assertGreater(sum(row["privateCommitBytes"] for row in pressure["members"]), 160 * 2**20)
            # Preserve an above-cap reported peak after this intentional rejection;
            # this observation must keep the resource gate failed.
            if pressure["jobPeakCommitBytes"] > 256 * 2**20:
                self.assertFalse(pressure["resourceQualification"]["qualified"])
                self.assertIn("aggregate_memory_unverified_or_over_limit", pressure["resourceQualification"]["reasons"])
        finally:
            # Normal test protocol, finite even after an assertion fails. No kill.
            stop.write_text("caller-owned normal test stop\n", encoding="utf-8")
            evidence["normalStopRequested"] = True
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                drained = job.sample()
                if drained["jobActiveProcesses"] == 0 and not drained["memberPids"] and all(p.exited() for p in owned):
                    break
                time.sleep(.02)
            evidence["rootExitCodes"] = {p.role: p.exit_code() for p in owned}
            evidence["afterStopSample"] = drained
            if drained["jobActiveProcesses"] or drained["memberPids"] or any(not p.exited() for p in owned):
                evidence["normalCloseCompleted"] = False
                with (directory / "test-result.json").open("x", encoding="utf-8") as out:
                    json.dump(evidence, out, indent=2)
                self.fail("Owned test child still active; close refused, no normal-success claim")
            try:
                evidence["close"] = job.close()
                evidence["normalCloseCompleted"] = True
            except BaseException as error:
                evidence["normalCloseCompleted"] = False
                evidence["closeErrorClass"] = type(error).__name__
                raise
            finally:
                with (directory / "test-result.json").open("x", encoding="utf-8") as out:
                    json.dump(evidence, out, indent=2)
            print(json.dumps({"nativeEvidenceDirectory": str(directory), "rootExitCodes": evidence["rootExitCodes"],
                              "normalCloseCompleted": evidence["normalCloseCompleted"]}))
        self.assertEqual(evidence["rootExitCodes"], {"first": 0, "second": 0})
        self.assertFalse(evidence["close"]["killAppliedByClose"])


if __name__ == "__main__":
    unittest.main()
