# Classic Outcome Clock and Respawn Performance, 2026-10-10

This is a bounded local Candidate for the actual TownTeleport command path.
The exact parent is `6153cb2b802fa700f6a182ce704f76e7568bdd08`. Its actual
Linux run `38043237500` failed the original optimized two-scroll Purification
check by 6 ms; siege passed. The complete failed main-job log is retained:
1,330,442 bytes, SHA256
`0e5303d480b079f2b504a1dc85e93f2e9cd91f5c1b279eb2b63a02f9baf43127`.
That parent was not qualified or deployed; there was no repeat-until-green.

## Implemented scope

`SimulationSession::current_tick` reads the existing RuntimeClockResource.
InProcess and SharedInProcess outcome construction delegate to that projection
instead of composing a complete inventory/quest/entity/AOI snapshot solely to
read its tick. The trait default retains the original full-snapshot fallback
for other runtimes. The command still executes first; actual tick, identity,
packet count, rejection acknowledgement and typed purchase results are retained.
The existing movement/KeepAlive/Version/Tick skip set is unchanged. No new Zone
lock, clock advancement, authentication fallback or acknowledgement is added.

One visible spawn-table rebuild now reuses walkable counts only for the same
inclusive rectangle and the same immutable collision Arc. The allocation is
retained until that build ends, preventing pointer reuse. Distinct rectangles,
different door images, missing terrain, zero counts and early-return behavior
keep the original algorithm. Visible candidates, random selection, slot IDs,
direction, ordering, quantity and full-map spawn logic are unchanged. This is
local reuse, not a global cache or a stale action queue.

## Actual verification

Final selected checks passed **137 distinct Rust tests**, including 15 new
tests. They cover legacy runtime fallback, pure clock reads before/during/after
play, all three classes' ordinary item outcomes, authentication rejection,
movement skip, typed shop results, owner priority/backpressure, the unchanged
optimized Gateway strict500ms story, original spawn/movement/collision checks,
and crowded/pet/Purification/Snake integration. All 49 selected inputs were
equal before/after each final command. This is not whole-workspace closure or
a claim that all older unrelated gates are green.

The independent retained parent respawn algorithm matches every slot, point,
direction and group order in the additional real Bichon field comparison at
(300,410), using 360 imported groups. A single unoptimized component observation
reported scans21->4 and time99,912us->30,221us. Loading is excluded; this is not
a claimed percentage improvement for complete TownTeleport or combat.

The first added comparison incorrectly assumed an arrival-protected town
viewport (345,362) would contain monsters. Both algorithms returned no spawns;
that original26-pass/1-fail run and source remain. Only the new component
fixture moved to the imported field group center. Original Gateway test source,
UID preparation, wall clock, +500ms deadline and assertions remain unchanged.
The preparation anchor failure before any Cargo start is also retained.
The initial integration invocation omitted the original test-support feature;
its actual compile101 ran no tests and is retained. The corrected feature was
already required by the original workflow, without opening test fault APIs in
production. Its final test results are separate from that execution failure.

Original CI commands/gates are preserved; one additional CI step runs clock,
respawn, purchase and hot-path regressions. Source review returned file
ownership before compilation. Root remains sole product/common-doc/Git writer.

## Publication and remaining work

| Layer | At this Candidate evidence freeze |
| --- | --- |
| Implementation | Bounded changes above implemented |
| Local selected tests | Passed; immutable raw evidence below |
| Git | New commit/push still separate |
| Exact new Linux / package | Required; parent's failed CI cannot substitute |
| Public gateway | Last read11:33:25 UTC:64b6, six capacity counters/TCP/viewers0; no switch |
| Native / human acceptance | Not performed by these protocol tests |

Any future production stage/switch requires another fresh drain and exact
artifact/source verification. No real saves, other games, installer, R2 objects
or R23/feed18 were changed in this round. A configured51-player admission limit
does not prove50-player load stability.

P1 next: original CanMove/Fishing/SlowTurn admission and the unfinished opaque
movement receipt/world action boundaries. Actual dense-monster right-mouse
escape, continuous attack and no-drift Windows acceptance remain open. P2-P7
and Mentor retain their natural/native/public/restart gates; Source population,
same-Source-CAS world outbox and full1:1 remain unfinished. The expired heartbeat
stays paused and the old blocked Goal is not marked complete.

[Immutable scope, source hashes, commands, counts and raw results](generated/player-qa/outcome-respawn-performance-20261010/candidate-01/EVIDENCE.json).
Archive SHA256 `a0e52dbce6938d8d4a397d0613939ca46850921315340060d3a195614d91a88d`, 874600 bytes,
77 individually verified entries; ZIP integrity checked.
