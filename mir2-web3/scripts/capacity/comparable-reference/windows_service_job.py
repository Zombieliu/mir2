"""Caller-owned native services sharing one Windows CPU/commit-memory budget.

Usage: job = SharedServiceJob(); server = job.launch("gateway", argv, cwd, logs)
gate = job.sample(required_roles=["gateway"], declared_dependencies=[])
Check gate["resourceQualification"]["qualified"] before starting an injector.
Declared dependencies identify distinct launch() service roots by exact role/PID,
not arbitrary descendants or another service's root PID.
An independent injector may use cpus=8, affinity_mask=0xff0, memory_bytes=4*2**30.
The caller performs its normal stop protocol, waits for every member to exit,
then calls close(). close() refuses live members and never treats a kill as normal.
Kill-on-job-close protects against controller loss; that is a failed shutdown.

No existing PID can be attached. Commands/environment are never serialized.
Only caller-owned fresh stdout/stderr files are redirected; child log contents
remain the caller's responsibility. Working/log paths are checked on C; callers
must direct all application data there too. Startup requests SW_HIDE and
CREATE_NO_WINDOW without altering GUI initialization or its message loop. A GUI
can override its startup show policy; validate its hidden readiness separately.
Job memory is aggregate committed memory, not a claim of total physical RSS.
OS/injector resources are separate. Resource qualification is not CCU acceptance.
Pure validators import on other platforms; native operations and CLI refuse them.
"""

from __future__ import annotations

import argparse
import ctypes as C
from ctypes import wintypes as W
import errno
import os
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time


ROLE = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}\Z")
LIMIT_FLAGS = 0x2000 | 0x200 | 0x10  # kill-on-close, aggregate Job memory, affinity
CPU_FLAGS = 1 | 4  # enabled, hard cap
MAX_MEMBERS = 4096


def validate_budget(cpus=4, memory_bytes=8 * 2**30, affinity_mask=0xf, *, host_cpus=None):
    host_cpus = os.cpu_count() if host_cpus is None else host_cpus
    if type(host_cpus) is not int or not 1 <= host_cpus <= 64:
        raise ValueError("A single processor group of at most 64 logical CPUs is required")
    if type(cpus) is not int or not 1 <= cpus <= host_cpus:
        raise ValueError("Invalid logical CPU count")
    if type(memory_bytes) is not int or not 0 < memory_bytes < 2**(8 * C.sizeof(C.c_size_t)):
        raise ValueError("Invalid aggregate commit limit")
    if type(affinity_mask) is not int or affinity_mask <= 0 or affinity_mask.bit_count() != cpus or affinity_mask >> host_cpus:
        raise ValueError("Affinity mask must match the logical CPU count and host")
    return {"logicalCpus": cpus, "hostLogicalCpus": host_cpus,
            "affinityMaskHex": hex(affinity_mask), "jobCommitLimitBytes": memory_bytes,
            "cpuRatePer10000": round(10000 * cpus / host_cpus)}


def assess_resource_sample(sample, *, required_roles=None, declared_dependencies=None):
    """Portable gate: no process access, inferred dependencies or zero-fill."""
    reasons = []
    if required_roles is None or declared_dependencies is None:
        reasons.append("explicit_role_and_dependency_declaration_required")
    required_roles = [] if required_roles is None else list(required_roles)
    declared_dependencies = [] if declared_dependencies is None else list(declared_dependencies)
    if not all(isinstance(role, str) and ROLE.fullmatch(role) for role in required_roles):
        raise ValueError("Invalid required role")
    members = sample.get("members", [])
    pids = [row.get("pid") for row in members]
    if any(type(pid) is not int or pid <= 0 for pid in pids) or len(pids) != len(set(pids)):
        reasons.append("invalid_or_duplicate_member_pid")
    if sample.get("membershipStable") is not True or type(sample.get("jobActiveProcesses")) is not int or sample.get("jobActiveProcesses") != len(pids) or sample.get("memberPids") != sorted(pids):
        reasons.append("membership_incomplete_or_changed")
    if not pids:
        reasons.append("no_active_owned_members")
    wanted, actual = sample.get("requestedLimits", {}), sample.get("effectiveLimits", {})
    if any(actual.get(key) != wanted.get(key) for key in ["affinityMaskHex", "jobCommitLimitBytes", "cpuRatePer10000"]):
        reasons.append("effective_limits_mismatch")
    if actual.get("limitFlags", 0) & LIMIT_FLAGS != LIMIT_FLAGS or actual.get("limitFlags", 0) & (0x800 | 0x1000):
        reasons.append("missing_limits_or_breakaway_allowed")
    if actual.get("cpuControlFlags", 0) != CPU_FLAGS:
        reasons.append("cpu_hard_cap_missing")
    peak, cap = sample.get("jobPeakCommitBytes"), actual.get("jobCommitLimitBytes")
    if type(peak) is not int or type(cap) is not int or peak < 0 or cap <= 0 or peak > cap:
        reasons.append("aggregate_memory_unverified_or_over_limit")
    for row in members:
        if row.get("inOwnedJob") is not True or row.get("queryError") is not None or not isinstance(row.get("role"), str) or not ROLE.fullmatch(row["role"]) or type(row.get("creationTime100ns")) is not int or row["creationTime100ns"] <= 0:
            reasons.append("member_ownership_resources_or_tree_unverified")
            continue
        try:
            mask, allowed = int(row["affinityMaskHex"], 16), int(wanted["affinityMaskHex"], 16)
            if mask <= 0 or mask & ~allowed:
                reasons.append("member_affinity_outside_budget")
        except (KeyError, TypeError, ValueError):
            reasons.append("member_affinity_unknown")
    roles = sample.get("roles", {})

    def active_owned_root(role, pid):
        matches = [row for row in members if row.get("pid") == pid and row.get("role") == role]
        roots = [row for row in sample.get("roots", []) if row.get("role") == role and row.get("pid") == pid]
        return (type(pid) is int and pid > 0 and roles.get(role) == pid
                and len(matches) == 1 and len(roots) == 1
                and matches[0].get("inOwnedJob") is True and matches[0].get("queryError") is None
                and type(matches[0].get("creationTime100ns")) is int and matches[0]["creationTime100ns"] > 0
                and roots[0].get("assignedBeforeResume") is True and roots[0].get("exited") is False
                and roots[0].get("creationTime100ns") == matches[0]["creationTime100ns"])

    for role in required_roles:
        if not active_owned_root(role, roles.get(role)):
            reasons.append("required_role_not_active:" + role)
    dependency_roles, dependency_pids = set(), set()
    for dependency in declared_dependencies:
        if not isinstance(dependency, dict) or not isinstance(dependency.get("role"), str) or not ROLE.fullmatch(dependency["role"]) or type(dependency.get("pid")) is not int or dependency["pid"] <= 0:
            raise ValueError("Dependencies need a safe role and explicit PID")
        role, pid = dependency["role"], dependency["pid"]
        if role in dependency_roles or pid in dependency_pids:
            reasons.append("duplicate_dependency_role_or_pid:" + role)
        dependency_roles.add(role); dependency_pids.add(pid)
        if pid not in pids:
            reasons.append("declared_dependency_outside_owned_job:" + role)
        elif not active_owned_root(role, pid):
            reasons.append("declared_dependency_identity_unverified:" + role)
    if sample.get("failedLaunches"):
        reasons.append("failed_launch_retained")
    return {"qualified": not reasons, "reasons": sorted(set(reasons)),
            "scope": "declared-resource-envelope-only", "capacityQualified": False}


class IO_COUNTERS(C.Structure):
    _fields_ = [(k, C.c_ulonglong) for k in ["readOps", "writeOps", "otherOps", "readBytes", "writeBytes", "otherBytes"]]


class BASIC_LIMIT(C.Structure):
    _fields_ = [("processTime", C.c_longlong), ("jobTime", C.c_longlong), ("LimitFlags", W.DWORD),
                ("minWs", C.c_size_t), ("maxWs", C.c_size_t), ("activeLimit", W.DWORD),
                ("Affinity", C.c_size_t), ("priority", W.DWORD), ("scheduling", W.DWORD)]


class EXTENDED_LIMIT(C.Structure):
    _fields_ = [("Basic", BASIC_LIMIT), ("io", IO_COUNTERS), ("processMemory", C.c_size_t),
                ("JobMemoryLimit", C.c_size_t), ("peakProcessMemory", C.c_size_t), ("PeakJobMemoryUsed", C.c_size_t)]


class CPU_RATE(C.Structure):
    _fields_ = [("ControlFlags", W.DWORD), ("CpuRate", W.DWORD)]


class ACCOUNTING(C.Structure):
    _fields_ = [(k, C.c_longlong) for k in ["user", "kernel", "periodUser", "periodKernel"]] + [(k, W.DWORD) for k in ["faults", "total", "active", "terminated"]]


class STARTUP(C.Structure):
    _fields_ = [("cb", W.DWORD), ("reserved", W.LPWSTR), ("desktop", W.LPWSTR), ("title", W.LPWSTR)] + [(k, W.DWORD) for k in ["x", "y", "xsize", "ysize", "xchars", "ychars", "fill", "flags"]] + [("show", W.WORD), ("reservedSize", W.WORD), ("reservedBytes", C.POINTER(C.c_byte)), ("stdin", W.HANDLE), ("stdout", W.HANDLE), ("stderr", W.HANDLE)]


class PROCESS_INFO(C.Structure):
    _fields_ = [("process", W.HANDLE), ("thread", W.HANDLE), ("pid", W.DWORD), ("tid", W.DWORD)]


class SECURITY(C.Structure):
    _fields_ = [("length", W.DWORD), ("descriptor", W.LPVOID), ("inherit", W.BOOL)]


class PROCESS_MEMORY(C.Structure):
    _fields_ = [("cb", W.DWORD), ("faults", W.DWORD)] + [(k, C.c_size_t) for k in ["peakWs", "ws", "peakPaged", "paged", "peakNonpaged", "nonpaged", "pagefile", "peakPagefile", "private"]]


class PROCESS_ENTRY(C.Structure):
    _fields_ = [("size", W.DWORD), ("usage", W.DWORD), ("pid", W.DWORD), ("heap", C.c_size_t),
                ("module", W.DWORD), ("threads", W.DWORD), ("parent", W.DWORD),
                ("priority", W.LONG), ("flags", W.DWORD), ("exe", W.WCHAR * 260)]


class _Windows:
    def __init__(self):
        if os.name != "nt":
            raise OSError(errno.ENOSYS, "Native Windows Job runtime required; validators are portable")
        self.k = C.WinDLL("kernel32", use_last_error=True)
        self.ps = C.WinDLL("psapi", use_last_error=True)
        definitions = {
            "CreateJobObjectW": (W.HANDLE, W.LPVOID, W.LPCWSTR),
            "SetInformationJobObject": (W.BOOL, W.HANDLE, C.c_int, W.LPVOID, W.DWORD),
            "QueryInformationJobObject": (W.BOOL, W.HANDLE, C.c_int, W.LPVOID, W.DWORD, W.LPVOID),
            "AssignProcessToJobObject": (W.BOOL, W.HANDLE, W.HANDLE),
            "IsProcessInJob": (W.BOOL, W.HANDLE, W.HANDLE, C.POINTER(W.BOOL)),
            "CreateProcessW": (W.BOOL, W.LPCWSTR, W.LPWSTR, W.LPVOID, W.LPVOID, W.BOOL, W.DWORD, W.LPVOID, W.LPCWSTR, C.POINTER(STARTUP), C.POINTER(PROCESS_INFO)),
            "CreateFileW": (W.HANDLE, W.LPCWSTR, W.DWORD, W.DWORD, C.POINTER(SECURITY), W.DWORD, W.DWORD, W.HANDLE),
            "ResumeThread": (W.DWORD, W.HANDLE), "CloseHandle": (W.BOOL, W.HANDLE),
            "WaitForSingleObject": (W.DWORD, W.HANDLE, W.DWORD),
            "GetExitCodeProcess": (W.BOOL, W.HANDLE, C.POINTER(W.DWORD)),
            "TerminateProcess": (W.BOOL, W.HANDLE, W.UINT),
            "OpenProcess": (W.HANDLE, W.DWORD, W.BOOL, W.DWORD),
            "GetProcessTimes": (W.BOOL, W.HANDLE, C.POINTER(W.FILETIME), C.POINTER(W.FILETIME), C.POINTER(W.FILETIME), C.POINTER(W.FILETIME)),
            "GetProcessAffinityMask": (W.BOOL, W.HANDLE, C.POINTER(C.c_size_t), C.POINTER(C.c_size_t)),
            "QueryFullProcessImageNameW": (W.BOOL, W.HANDLE, W.DWORD, W.LPWSTR, C.POINTER(W.DWORD)),
            "CreateToolhelp32Snapshot": (W.HANDLE, W.DWORD, W.DWORD),
            "Process32FirstW": (W.BOOL, W.HANDLE, C.POINTER(PROCESS_ENTRY)),
            "Process32NextW": (W.BOOL, W.HANDLE, C.POINTER(PROCESS_ENTRY)),
            "GetActiveProcessorGroupCount": (W.WORD,), "GetActiveProcessorCount": (W.DWORD, W.WORD),
        }
        for name, (result, *args) in definitions.items():
            fn = getattr(self.k, name); fn.restype = result; fn.argtypes = args
        self.ps.GetProcessMemoryInfo.restype = W.BOOL
        self.ps.GetProcessMemoryInfo.argtypes = [W.HANDLE, C.POINTER(PROCESS_MEMORY), W.DWORD]

    @staticmethod
    def check(ok, operation):
        if not ok:
            error = C.WinError(C.get_last_error())
            error.add_note(operation)  # static operation only, never argv/environment
            raise error

    def close(self, handle):
        if handle:
            self.check(self.k.CloseHandle(handle), "CloseHandle")

    def query(self, job, kind, value):
        self.check(self.k.QueryInformationJobObject(job, kind, C.byref(value), C.sizeof(value), None), "QueryInformationJobObject")
        return value

    def wait_exit(self, handle, milliseconds):
        result = self.k.WaitForSingleObject(handle, milliseconds)
        if result == 0xffffffff:
            self.check(False, "WaitForSingleObject")
        if result != 0:
            raise TimeoutError("Owned process did not exit within the bounded wait")

    def creation_time(self, handle):
        created, exited, kernel, user = W.FILETIME(), W.FILETIME(), W.FILETIME(), W.FILETIME()
        self.check(self.k.GetProcessTimes(handle, C.byref(created), C.byref(exited), C.byref(kernel), C.byref(user)), "GetProcessTimes")
        return (created.dwHighDateTime << 32) | created.dwLowDateTime

    def pids(self, job):
        size = 16
        while size <= MAX_MEMBERS:
            raw = C.create_string_buffer(8 + C.sizeof(C.c_size_t) * size)
            ok = self.k.QueryInformationJobObject(job, 3, raw, C.sizeof(raw), None)
            error = C.get_last_error() if not ok else 0
            if ok:
                assigned, listed = (W.DWORD * 2).from_buffer(raw)
                if listed > size or assigned != listed:
                    size *= 2; continue
                return sorted((C.c_size_t * listed).from_buffer(raw, 8))
            if error != 234:
                self.check(False, "Query Job PID membership")
            size *= 2
        raise RuntimeError("Job membership exceeds finite enumeration bound")

    def parents(self, wanted):
        snapshot = self.k.CreateToolhelp32Snapshot(2, 0)
        self.check(snapshot != C.c_void_p(-1).value, "CreateToolhelp32Snapshot")
        found = {}
        try:
            row = PROCESS_ENTRY(); row.size = C.sizeof(row)
            ok = self.k.Process32FirstW(snapshot, C.byref(row))
            while ok:
                if row.pid in wanted: found[row.pid] = row.parent
                ok = self.k.Process32NextW(snapshot, C.byref(row))
            if C.get_last_error() != 18: self.check(False, "Process32 enumeration")
        finally:
            self.close(snapshot)
        return found

    def process_info(self, handle, job):
        flag = W.BOOL(); self.check(self.k.IsProcessInJob(handle, job, C.byref(flag)), "IsProcessInJob")
        if not flag.value:
            raise RuntimeError("PID no longer belongs to this owned Job")
        created, exited, kernel, user = W.FILETIME(), W.FILETIME(), W.FILETIME(), W.FILETIME()
        self.check(self.k.GetProcessTimes(handle, C.byref(created), C.byref(exited), C.byref(kernel), C.byref(user)), "GetProcessTimes")
        def ticks(value): return (value.dwHighDateTime << 32) | value.dwLowDateTime
        mask, system = C.c_size_t(), C.c_size_t()
        self.check(self.k.GetProcessAffinityMask(handle, C.byref(mask), C.byref(system)), "GetProcessAffinityMask")
        path = C.create_unicode_buffer(32768); length = W.DWORD(len(path))
        self.check(self.k.QueryFullProcessImageNameW(handle, 0, path, C.byref(length)), "QueryFullProcessImageNameW")
        memory = PROCESS_MEMORY(); memory.cb = C.sizeof(memory)
        self.check(self.ps.GetProcessMemoryInfo(handle, C.byref(memory), C.sizeof(memory)), "GetProcessMemoryInfo")
        return {"creationTime100ns": ticks(created), "executablePath": path.value,
                "affinityMaskHex": hex(mask.value), "cpuSeconds": (ticks(kernel) + ticks(user)) / 10**7,
                "workingSetBytes": memory.ws, "privateCommitBytes": memory.private,
                "inOwnedJob": True, "queryError": None}


class OwnedProcess:
    def __init__(self, api, handle, pid, role, created):
        self._api, self._handle = api, handle
        self.pid, self.role, self.creation_time_100ns = pid, role, created
        self.assigned_before_resume = True

    def exited(self):
        if self._handle is None: raise RuntimeError("Owned process handle is closed")
        result = self._api.k.WaitForSingleObject(self._handle, 0)
        if result == 0xffffffff: self._api.check(False, "WaitForSingleObject")
        return result == 0

    def exit_code(self):
        if not self.exited(): return None
        value = W.DWORD(); self._api.check(self._api.k.GetExitCodeProcess(self._handle, C.byref(value)), "GetExitCodeProcess")
        return value.value


class SharedServiceJob:
    def __init__(self, cpus=4, memory_bytes=8 * 2**30, affinity_mask=0xf):
        self._api = _Windows()
        if self._api.k.GetActiveProcessorGroupCount() != 1:
            raise ValueError("Multiple processor groups are not supported by this fixed-mask launcher")
        self.requested = validate_budget(cpus, memory_bytes, affinity_mask, host_cpus=self._api.k.GetActiveProcessorCount(0))
        self._lock = threading.RLock(); self._roots = {}; self._failed = []; self._stranded = []
        self._job = self._api.k.CreateJobObjectW(None, None)
        self._api.check(self._job, "CreateJobObjectW")
        try:
            limits = EXTENDED_LIMIT(); limits.Basic.LimitFlags = LIMIT_FLAGS
            limits.Basic.Affinity = affinity_mask; limits.JobMemoryLimit = memory_bytes
            self._api.check(self._api.k.SetInformationJobObject(self._job, 9, C.byref(limits), C.sizeof(limits)), "Set aggregate Job memory and affinity")
            rate = CPU_RATE(CPU_FLAGS, self.requested["cpuRatePer10000"])
            self._api.check(self._api.k.SetInformationJobObject(self._job, 15, C.byref(rate), C.sizeof(rate)), "Set Job CPU hard cap")
        except BaseException:
            self._api.close(self._job); self._job = None; raise

    def launch(self, role, command, cwd, output):
        with self._lock:
            if self._job is None: raise RuntimeError("Job is closed")
            if not isinstance(role, str) or not ROLE.fullmatch(role) or role in self._roots:
                raise ValueError("A unique safe role is required")
            if not isinstance(command, (list, tuple)) or not command or not all(isinstance(x, str) and x and '\0' not in x for x in command):
                raise ValueError("A nonempty argv list is required")
            cwd, output = Path(cwd).resolve(strict=True), Path(output).resolve()
            if not cwd.is_dir() or cwd.drive.upper() != "C:" or output.drive.upper() != "C:":
                raise ValueError("Caller-owned working/log directories must be on C")
            if output.exists() and any(output.iterdir()): raise ValueError("Fresh empty log directory required")
            output.mkdir(parents=True, exist_ok=True)
            handles = []; process = PROCESS_INFO(); resumed = False
            try:
                security = SECURITY(C.sizeof(SECURITY), None, True)
                for name, access, disposition in [("NUL", 0x80000000, 3), (str(output / "stdout.log"), 0x40000000, 1), (str(output / "stderr.log"), 0x40000000, 1)]:
                    handle = self._api.k.CreateFileW(name, access, 3, C.byref(security), disposition, 0x80, None)
                    self._api.check(handle != C.c_void_p(-1).value, "Create exclusive child stdio")
                    handles.append(handle)
                start = STARTUP(); start.cb = C.sizeof(start); start.flags = 0x100 | 1; start.show = 0
                start.stdin, start.stdout, start.stderr = handles
                executable = shutil.which(command[0]) or str(Path(command[0]).absolute())
                line = C.create_unicode_buffer(subprocess.list2cmdline(command))
                self._api.check(self._api.k.CreateProcessW(executable, line, None, None, True, 4 | 0x08000000, None, str(cwd), C.byref(start), C.byref(process)), "CreateProcessW suspended owned service")
                self._api.check(self._api.k.AssignProcessToJobObject(self._job, process.process), "Assign new service before resume")
                created = self._api.creation_time(process.process)
                self._api.check(self._api.k.ResumeThread(process.thread) != 0xffffffff, "Resume assigned service")
                resumed = True
                owned = OwnedProcess(self._api, process.process, process.pid, role, created)
                self._roots[role] = owned
                return owned
            except BaseException as error:
                if process.process and not resumed:
                    entry = {"role": role, "pid": process.pid, "normalExit": False,
                             "preResumeFailure": True, "winerror": getattr(error, "winerror", None)}
                    try:
                        self._api.check(self._api.k.TerminateProcess(process.process, 125), "Terminate only newly created suspended failure")
                        self._api.wait_exit(process.process, 5000)
                        entry["terminatedWhileSuspended"] = True
                    except (OSError, TimeoutError) as cleanup:
                        entry["cleanupErrorClass"] = type(cleanup).__name__
                        entry["cleanupWinerror"] = getattr(cleanup, "winerror", None)
                        error.add_note("Suspended-child cleanup failed; retained owned handle, Win32 error=" + str(getattr(cleanup, "winerror", None)))
                        self._stranded.append((process.process, process.pid))
                    self._failed.append(entry)
                    if entry.get("terminatedWhileSuspended"):
                        self._api.close(process.process)
                raise
            finally:
                self._api.close(process.thread)
                for handle in handles: self._api.close(handle)

    def sample(self, *, required_roles=None, declared_dependencies=None):
        with self._lock:
            if self._job is None: raise RuntimeError("Job is closed")
            before = self._api.pids(self._job); parents = self._api.parents(set(before))
            limits = self._api.query(self._job, 9, EXTENDED_LIMIT()); rate = self._api.query(self._job, 15, CPU_RATE())
            members = []
            roots = {p.pid: p for p in self._roots.values()}
            for pid in before:
                handle = self._api.k.OpenProcess(0x0400 | 0x0010, False, pid)
                row = {"pid": pid, "parentPid": parents.get(pid), "role": None, "inOwnedJob": None, "queryError": None}
                try:
                    self._api.check(handle, "Open owned Job member")
                    row.update(self._api.process_info(handle, self._job))
                    ancestor, visited = pid, set()
                    while ancestor not in visited:
                        visited.add(ancestor)
                        if ancestor in roots:
                            root = roots[ancestor]
                            if row["creationTime100ns"] >= root.creation_time_100ns:
                                row["role"] = root.role
                            break
                        ancestor = parents.get(ancestor)
                        if ancestor is None: break
                    if pid in roots and row["creationTime100ns"] != roots[pid].creation_time_100ns:
                        row["role"] = None; row["queryError"] = {"category": "root_lifecycle_mismatch"}
                except (OSError, RuntimeError) as error:
                    row["queryError"] = {"category": type(error).__name__, "winerror": getattr(error, "winerror", None)}
                finally:
                    self._api.close(handle)
                members.append(row)
            after = self._api.pids(self._job); account = self._api.query(self._job, 1, ACCOUNTING())
            result = {"schema": "mir2.shared-service-job-sample.v1", "atUnixMs": round(time.time() * 1000),
                      "requestedLimits": dict(self.requested), "effectiveLimits": {"affinityMaskHex": hex(limits.Basic.Affinity), "jobCommitLimitBytes": limits.JobMemoryLimit, "cpuRatePer10000": rate.CpuRate, "cpuControlFlags": rate.ControlFlags, "limitFlags": limits.Basic.LimitFlags},
                      "jobActiveProcesses": account.active, "jobTotalAssignedProcesses": account.total,
                      "jobCpuSeconds": (account.user + account.kernel) / 10**7, "jobPeakCommitBytes": limits.PeakJobMemoryUsed,
                      "memberPids": before, "membershipStable": before == after and account.active == len(after),
                      "members": members, "roles": {role: p.pid for role, p in self._roots.items()},
                      "roots": [{"role": role, "pid": p.pid, "creationTime100ns": p.creation_time_100ns, "assignedBeforeResume": p.assigned_before_resume, "exited": p.exited(), "exitCode": p.exit_code()} for role, p in self._roots.items()],
                      "failedLaunches": list(self._failed), "commandArgumentsExported": False, "environmentExported": False,
                      "osAndInjectorIncluded": False, "killOnControllerLossIsNotNormalExit": True}
            result["resourceQualification"] = assess_resource_sample(result, required_roles=required_roles, declared_dependencies=declared_dependencies)
            return result

    def close(self):
        with self._lock:
            if self._job is None: return {"closed": True, "killAppliedByClose": False}
            for handle, unused_pid in self._stranded:
                result = self._api.k.WaitForSingleObject(handle, 0)
                if result == 0xffffffff: self._api.check(False, "Wait stranded owned child")
                if result != 0:
                    raise RuntimeError("Failed suspended-child cleanup remains live; close refused")
            active = self._api.pids(self._job); account = self._api.query(self._job, 1, ACCOUNTING())
            if active or account.active or any(not p.exited() for p in self._roots.values()):
                raise RuntimeError("Live owned Job members; caller must normal-stop and wait before close")
            outcomes = {role: p.exit_code() for role, p in self._roots.items()}
            for p in self._roots.values(): self._api.close(p._handle); p._handle = None
            for handle, unused_pid in self._stranded: self._api.close(handle)
            self._stranded.clear()
            self._api.close(self._job); self._job = None
            return {"closed": True, "allMembersExited": True, "killAppliedByClose": False,
                    "rootExitCodes": outcomes, "failedLaunches": list(self._failed), "normalStopMustBeProvenByCaller": True}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args(argv)
    if os.name != "nt": parser.exit(2, "Native Windows Job runtime required; pure validators can be imported.\n")
    parser.exit(2, "Use SharedServiceJob from a caller-owned lifecycle controller; no PID-attach CLI.\n")


if __name__ == "__main__":
    main()
