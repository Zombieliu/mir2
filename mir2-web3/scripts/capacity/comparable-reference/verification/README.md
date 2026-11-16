# Independent offline comparison verifier

`verify_report.py` reads public projections only. It does not import either
adapter scorer, start processes, contact a server, read account stores or alter
inputs. Current scope is the shared `dense-movement-chat`, `game-core-local`
plan: functional 2 actors for 60 seconds, or declared 25/50/100 actors for 300
seconds. The latter still cannot qualify a production/full-stack capacity claim.

From the repository root, run:

```text
python -B mir2-web3/scripts/capacity/comparable-reference/verification/verify_report.py --plan <plan.json> --summary <summary.json> --events <events.jsonl> --controller <controller-result.json> --resources <resources.jsonl> --injector <injector-resources.jsonl> --disk-readback <disk-readback.json>
```

Both engines require `--disk-readback`. Crystal uses the public projection from
the original `LoadAccounts` reader after normal server shutdown, for **all**
actors. Modern uses `read_modern_disk.py` below. Default output is JSON on stdout. Optional `--output` creates a new file
exclusively (on C when running on Windows). It never overwrites an old result.
Exit 0 means this actual core-local evidence passes; exit 1 means SLO failure,
insufficient/contradictory evidence, or synthetic evidence. Synthetic consistency
is exposed separately and never makes `passed` or `capacityQualified` true.

The verifier independently enumerates the immutable plan denominator for each
actor and each half-open 60000 ms window. It computes completion/timeout/notSent,
completion ratio, and nearest-rank P95 movement/chat latency and send scheduling
delay. Scheduling samples use actual `sentAtMs - plannedAtMs`; unsent actions
remain in the offered denominator and are neither completion nor timeout. One
notSent is not an additional veto. Thresholds are the common workload definition,
with no separate requirement that every chat/walk complete.

Raw packet identity is `(receiver actor, connection epoch, receive sequence)`.
Receipts must join an actual packet with matching time/type, current admission,
fresh receive floor, exact target or actual chat nonce, and expected observers.
Repeated sends, reused packets, missing original outcomes, unknown send outcome,
or a new send before the preceding unique terminal are hard evidence failures.
Late packets may close a fence but cannot restore an original success.

Equipment verification requires all eight steps for the functional pair: actual
ordinary send/ACK within 15 seconds, normal logout and close each within 5 seconds,
fresh ordinary Login/StartGame/UserInformation, and full carried state within the
original ACK+15-second bound. All unaffected slots, complete projected item fields,
gold/capacities and item identities must remain equal. A hash alone is insufficient.
For the moved modern item only, the full 22-field original `tooltipSource.userItem`
is compared across the different inventory/equipment DTOs. The finite display/grid
field differences are those defined in `runtime/items.rs:484` and
`runtime/equipment.rs:121`; unrelated items and restored complete rosters still
match exactly. A missing original item field is insufficient evidence.
Final selected actors `[0, population-1]` must use fresh connections, preserve the
last authoritative transform and complete carried state, then exit normally.
For Crystal, `UserInformation.authoritativeItemState` must come directly from the
received packet, and disk readback must match all actors. `MirGridType.Inventory`
is **1**. Native full item bytes remain private: the original decoder's per-item
`UserItem.Save` SHA is the explicit trust boundary; this tool does not re-open them.

Resource gates require `mir2.comparable.stage-controller.v1`, the exact driver PID,
actual zero producer exits and normal Job closure, and controller-bound SHA/bytes
for the plan, summary/events, and resource streams. Every OS resource row is
reassessed with `windows_service_job.assess_resource_sample`, ignoring its stored
qualification flag. Server limits are aggregate 4 CPUs/mask `0xf`/8 GiB commit;
the separately reported injector is 8 CPUs/mask `0xff0`/4 GiB. Server samples cover
the entire owned driver process; both streams have the predeclared 1000 ms nominal
interval and at most 2500 ms gaps. Unknown ownership, memory or lifecycle data is
a hard failure. Driver/controller monotonic offsets are never mixed; durations
must still fit the actual owned-process lifetime. OS resources remain separate.

Small JSON inputs/resources retain their 128 MiB/two-million-row bounds. Events
use **two complete streaming scans**, up to 1 GiB/eight million rows, with a 2 MiB
line bound. Each scan parses every line and hashes every byte; file identity,
mtime, size, full SHA and full row count must agree between scans. Duplicate or
regressed receive sequences are audited even for packets that will not be kept.
Only exact referenced packets, all control/send records and critical bootstrap,
owner-transform, equipment and lifecycle packets remain in memory, in source order.
The retained set is bounded at 250000 rows/128 MiB of serialized data; no bound
silently truncates a file or selects fewer required receipts. Full producer
`eventRows` is compared with full scanned rows, never with the filtered count.
Duplicate JSON keys, nonfinite numbers, changed inputs and truncated final lines
are rejected. Missing schema fields are reported as insufficient evidence;
there is no compatibility default that inserts success, zero latency or zero use.
The verifier does not establish navigation eligibility, binary/source authenticity
or private fixture identity beyond the provided packet/controller projections;
those remain the coordinator's pinned preparation/release gates.

The maintained output schema is `mir2.comparable.independent-verification.v3`.
The coordinator's frozen v1/v2 sources and old results remain historical evidence.
V2's gear rules remain: bind state to the exact authenticated `admitted.snapshot`
namespace, successful normal bootstrap, and pre-mutation event order. It also
requires the last owner snapshot before that mutation to retain the same carried
fingerprint. Pre-login scene snapshots are not authenticated owner evidence;
searching for an arbitrary matching hash is never allowed.
Both rounds must move the same initial Weapon221 UID; vector order cannot select
another equipment slot. V3 separately supports legacy plans without scheduling
metadata and explicit scheduling v2, whose `workload.v2.json` file/canonical hashes
must match. It confirms the definition changes only schema/chat-phase fields from
v1. For v2 the chat phase uses the same actor primary interval phase plus its
declared midpoint offset; all same-actor offers must be at least half an interval
apart. Counts, deadlines, thresholds and resources stay identical. Old 10 ms
collision plans retain old semantics and their original failed windows.

Large actual files have **not** been memory-qualified by the offline unit tests.
Run their verification inside the coordinator's separate 4 GiB diagnostic Job
and record its actual peak/exit. Reader size/selection/memory failures are evidence
tool limits, not a finding that the game server failed or met a capacity limit.
Do not replace a failed run, relax SLOs, or reinterpret it using another plan.

After modern normal shutdown, generate the separate File authority projection:

```text
python -B mir2-web3/scripts/capacity/comparable-reference/verification/read_modern_disk.py --plan <plan.json> --summary <modern.report.json> --events <modern.events.jsonl> --controller <controller-result.json> --resources <resources.jsonl> --injector <injector-resources.jsonl> --runtime <runtime-receipt.json> --accounts <isolated-private/accounts.json> --output <new-public-disk-readback.json>
```

This reader first independently checks the public protocol and resource evidence,
then requires the explicitly selected File path to match the runtime receipt and
lie within the isolated temporary `modern-audit` tree. It reads actual character
saves and compares map, final position/direction/vitals, gold/capacity, UID/grid and
complete original item metadata to every actor's fresh owner state. Only numeric
facts, match booleans and whole-file/semantic hashes are exported; passwords,
account keys, authentication fields, ledgers and rental names are never emitted.
The initial reader supports the simple common fixture carrier; nonempty sockets,
rental/sealed overlays or merged legacy stat carriers fail explicitly as
unsupported. It does not approximate them or change the store. The verifier binds
this receipt to the exact plan/report/events/controller and reviewed reader code.

Run the in-memory tests with:

```text
python -B -m unittest discover -s mir2-web3/scripts/capacity/comparable-reference/verification -p test_verify_report.py -v
python -B -m unittest discover -s mir2-web3/scripts/capacity/comparable-reference/verification -p test_read_modern_disk.py -v
```

The tests include full synthetic native packet/equipment/disk schemas, both action
schemas, late/fenced/stale/ambiguous receipts, permitted notSent, resource failures,
short fake elapsed time and bounded input parsing. Two additional read-only tests
use the modern owner's SDKMock projection when the already-created
`%TEMP%/mir2-comparable-capacity-20261113/modern-audit/synthetic-v2` fixture exists;
they are explicitly skipped elsewhere. These tests launch nothing and write no
runtime artifacts. `test_streaming.py` additionally checks all v2 population phases,
old-plan preservation, two-pass equivalence, discarded-packet auditing, mutation
between passes and explicit bounds using sub-1-MiB in-memory files only. Always
use `-B` to avoid bytecode on the source drive. For all tests use `-p 'test_*.py'`.

Freeze the maintained `verification/` directory together with sibling
`windows_service_job.py`, `workload.v1.json` and `workload.v2.json` at the same
relative layout. The verifier imports neither the generator nor an adapter scorer.
