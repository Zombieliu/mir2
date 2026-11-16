# Tick diagnostic reader v2 — preparation only

This is the typed-frame robustness revision. The frozen v1 reader, tests,
README, logs and `FREEZE.json` remain unchanged. V2 explicitly rejects
non-object identity/health records, expected-pin configurations and events
before accessing their fields. Rejected frames are never persisted as parsed
query data. The transport, read-only scope and protocol schema are unchanged.
All 15 queued-phase labels and three input-class labels were compared with
the actual `web.rs` implementations; source hashes and exact sets are recorded
in `FREEZE-v2.json`.

No real query has been run. The future candidate PID, process start ticks,
invocation and revision are deliberately not defaulted. The coordinator must
review this artifact and supply all four pins after the candidate is built and
deployed. The helper does not build, deploy, restart, change a service, run SQL,
capture packets, or change a VPN or route.

`read_tick_diagnostics_v2.py` is self-contained. It copies/adapts the previously
reviewed v2 transport/status/framing path, with provenance pinned to:

- v2: `2e2e5514f499c3bb635065bbd952bf48a428ebf8fe5c3317aaf8a76b6f07c795`
- base: `60f4ca8fabc2341c803dca254281bba86b277ac0fd7eca722810ff27a3cac127`

Neither old reader is imported or modified. The only optional external Python
library is Paramiko, using the existing `.codex/tmp/mir2-deploy-ssh-runtime`
package fallback. Importing this helper performs no network access or writes.

The fixed remote target is the existing TEST host and
`mir2-playtest.service`; the health URL is fixed to loopback port 7210. There
are no host, service, command or health-URL overrides. The ED25519 fingerprint
is checked before authentication. A local PTY is mandatory; `getpass` keeps
the password in memory. Remote execution uses `sudo -n`, shell-quotes the
validated embedded Python program, and closes remote stdin. The password is
never supplied to sudo or persisted.

The required CLI arguments are `--output`, `--pid`, `--start-ticks`,
`--invocation-id`, `--revision`, `--since`, and `--until`. PID/start ticks must
be positive decimal numbers; invocation is exactly 32 lowercase hex digits;
revision is exactly 40 lowercase hex digits. UTC timestamps use ISO format
with `Z` or `+00:00`. The interval must be positive and at most 60 seconds.
The intended first observation window is 20 seconds during the future
25/50/100-player test. For example, after setting the coordinator-owned pins:

```powershell
python read_tick_diagnostics_v2.py --output $newOutput --pid $candidatePid --start-ticks $candidateStartTicks --invocation-id $candidateInvocation --revision $candidateRevision --since $windowSinceUtc --until $windowUntilUtc
```

The destination must be new and its parent must exist. It is opened with
exclusive-create semantics before SSH connects. A 60-second local transport
watchdog starts after the password prompt. Remote commands and health reads
share a 20-second query deadline. Journal stdout/stderr and local SSH streams
have 8 MiB bounds. Only the owned query subprocess/transport may be closed on
deadline or failure; there is no service control.

The remote program checks active service PID/invocation and `/proc` start
ticks, and the health revision, both before and after reading. Journalctl also
filters PID/invocation; projection independently checks both and the timestamp
on every row. The command uses argument lists rather than a shell. It selects
only these static stages:

- `personal_tick.leaves`
- `owner_health.prepare.zone_lock_wait` / `.zone_lock_held`
- `owner_health.commit.zone_lock_wait` / `.zone_lock_held`
- `runtime_tick.execute` / `runtime_tick.total`
- `serial_gate.zone_outbound_wait`
- `socket_input.queue_residence`
- `shared_session.inner_execute`

Projection retains each matching row with fixed numeric fields and known enum
values only. Missing fields remain null. Invalid/duplicate values become null
with fixed field-name diagnostics; unrecognized tokens are counted, never
copied. There are no raw messages, error strings, account/session/wallet/auth
values, payloads or environment contents in the output. The local frame
validator independently rejects extra fields, wrong pins, bad types/enums,
forged counts/completeness, unexpected stdout and malformed frames before
persisting a parsed result. Unparsed stdout and stderr are never persisted;
their observed byte counts and SHA-256 digests are retained.

`leaf_mask` must be an integer in 0..127. Bit order, matching the source enum:
0 billing, 1 gift, 2 checkpoint begin, 3 personal timers, 4 source dispatch,
5 finalize, 6 finish. A printed zero duration with its bit unset does not prove
that leaf executed. A missing mask is null; the reader does not infer execution
or substitute a zero. `outcome` is only `ok` or `error`; an error outcome is
retained and is not a successful Tick or SLO result.

Journalctl returns at most the newest 12,000 matching rows. Reaching 12,000
always sets `rowBoundReached=true` and
`boundedWindowExportComplete=false`, even if the window might contain exactly
that many. Missing/invalid metrics, malformed JSON/timestamps, or unexpected
filtered rows also make completeness false. Successful transport/query `ok`
is distinct from complete export, game correctness and SLO/capacity success.

Missing remote exit status remains missing. Exit-status method return `-1`
is recorded separately and never converted to exit code zero. The readback
also records actual stream endings and local deadline failures. Local request
and response UTC/monotonic timestamps and remote query UTC timestamps are
retained as clock evidence. No exact cross-clock latency or clock offset is
inferred; clock stability and network delay must be assessed separately.

Offline verification uses only fixtures and mocked sockets/Paramiko channels.
It covers every whitelist stage and mask bit, privacy filtering, null fields,
type/range/enum failures, source pins before/after, malformed framing, the
12,000-row/8 MiB boundary, ED25519 mismatch before authentication, required CLI
pins, exclusive output, and truthful exit status including `-1`.

```powershell
python -m py_compile read_tick_diagnostics_v2.py test_read_tick_diagnostics_v2.py
python -m unittest -v test_read_tick_diagnostics_v2
```
