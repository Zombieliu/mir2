# Cloud monitor v2 — canonical entry

The reviewed v2 monitor is available at
`mir2-web3/scripts/capacity/cloud-reference/cloud_monitor_v2.py`.
Promotion and offline checks do not establish a real monitor run or deployment.
The historical `cloud_monitor.py` remains byte-identical for provenance and
baseline tests; use the v2 entry for new monitoring campaigns. Tests read the
old file as source text without importing or executing its entry point.

Source provenance:

- Original monitor: `e9b3945f5d366e34741cbe9750d5b7b07b47bd1259151da97f07bb574c2a1048`
- Pinned SSH Transport proof: `326bfdc54a8c7af6133d6d9ad22d0f0fe3057db5910467e3f5421d420bbb4456`

The fixed SSH endpoint remains `165.154.65.136:22`, user `ubuntu`. The sole
accepted host key is ED25519 with fingerprint
`SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8`, checked before password
authentication. There is one password-auth attempt, with fallback disabled.
A local PTY is required for `getpass`; the password stays in memory. Remote
execution uses `sudo -n -- python3`, closes remote stdin and sends no password
to sudo. No automatic host-key enrollment or auth retry is performed.

The existing output/history/stop-file protocol and `schemaVersion: 1` are
retained. Memory, CPU, pressure, IO, host-primary-NIC, target/old-service health,
capacity and rate fields keep their existing names and sampling calculations.
The remote program's original AST is identical after removing only the new
activation-identity functions, records and checks. Existing resource guards
and baseline comparisons remain unchanged.

Launch now requires explicit `--expected-pid`, `--expected-revision`,
`--expected-invocation-id` and `--expected-start-ticks`. Revision is exactly
40 lowercase hex digits; invocation is exactly 32 lowercase hex digits; start
ticks are a positive u64. The target unit is fixed to `mir2-playtest.service`,
health port 7210. Existing old-service choices remain the two explicitly
listed Gateway units, with old health port 7110. Host/user/SSH/health-port
overrides accept only these fixed values. There are no new load-harness,
activity, gear, logout, retry or SLO command semantics.

PID/start/invocation/active state and the revision are checked before and after
remote monitoring initialization. Every sample includes `monitorIdentity`
with expected, initialization-before/after and sample-before/after snapshots.
A mismatch requests a guard stop. Local admission also requires matching
identity evidence and matching target fields/revision on nonterminal samples
and finite-duration terminal samples.

Unchanged protection defaults and bounds:

- Host available-memory reserve: at least 512 MiB.
- Target memory: at most 85% of its finite MemoryMax; callers may tighten it.
- Interval: 1..3 seconds, default 2.
- Staleness: greater than twice the interval and below 15 seconds, default 12.
- Sample timestamp: no older than the stale budget and no more than 5 seconds
  in the future; boundary equality remains allowed.
- Monitoring duration: 1..10800 seconds, default 1800.
- Existing old-service availability/health, target health/capacity, activation,
  MemoryMax, OOM, CPU counter and NIC counter checks remain in place.

SSH setup has a 20-second owned-connection deadline. The original local sample
duration starts after remote execution is acknowledged; the remote monitoring
duration retains its original clock location. A watchdog closes only this
monitor's socket/channel/transport for setup failure, local stop, duration or
stale stream. The remote producer only reads system metadata and loopback
health; disconnect/BrokenPipe ends its own query. No service, route, VPN or
network configuration is controlled.

The original 262144-byte stdout-buffer/line bound is retained. Total stdout is
also bounded by `(ceil(maxSeconds / intervalSeconds) + 3) * 262144`, covering
the finite maximum sample count. Stderr has an 8 MiB cumulative bound. Raw
stderr, unparsed stdout and credentials are not persisted. Observed byte counts
and SHA-256 digests are added under terminal `monitorReadback`.

The existing terminal fields and completed/guard-stop/failure outcomes remain.
`completedBy` distinguishes a local stop/duration/interrupt from an observed
finite remote terminal sample. EOF without a terminal sample is failure even
when an actual exit code zero is received. Exit-status method return `-1`
remains unknown (`remoteExitStatus=null`, received=false); it is never treated
as zero. An independently observed finite terminal sample can complete the
local monitor while exit status remains unknown, with both facts recorded.
An observed nonzero exit after a finite terminal is failure. Local requested
completion remains distinct from remote process-exit proof.

Output/history/control paths must be outside the canonical repository and use
fresh distinct names. The repository root is discovered from the script or
current directory, without a hardcoded checkout drive. Run from the canonical
workspace so exclusion can be verified. The
starting stop=true record, atomic latest-file replacement, history records and
terminal stop=true record are retained. Existing records are never reused.

Offline verification:

Run from the repository root; test fixtures use the system temporary directory
and `-B` prevents bytecode artifacts in the repository:

```powershell
python -B -m unittest discover -s mir2-web3/scripts/capacity/cloud-reference -p test_cloud_monitor_v2.py -v
```

Operator invocation, after setting the coordinator-verified activation pins and
fresh external evidence/control paths:

```powershell
python mir2-web3/scripts/capacity/cloud-reference/cloud_monitor_v2.py --expected-pid $candidatePid --expected-revision $candidateRevision --expected-invocation-id $candidateInvocation --expected-start-ticks $candidateStartTicks --output $monitorLatest --history $monitorHistory --stop-file $monitorStop
```

The final run passed 17 tests using synthetic resource samples, mocked Linux
clock ticks, sockets and Paramiko only. The first fixture run exposed missing
Linux `os.sysconf` on the Windows test host; the fixture now supplies a fixed
Linux tick frequency. That failed log is retained and is not validation of a
production failure. No public endpoint, private pool, ledger or credential was
read by these tests. This preparation supplies no cloud capacity qualification.
