# Classic terrain fixture recovery — 2026-10-10

The previous transfer source `348caa5e4b3b8bb3d63e3d2af80803e958adafee`
was actually pushed, but its own
[Linux run 38019332713](https://github.com/Zombieliu/mir2/actions/runs/38019332713)
failed the original SnakeTotem cold-recovery check. Four integration tests
passed and one failed; the separate siege job passed. No package or production
switch is claimed from that failed run.

This bounded round repairs eight retained test fixtures to use actual bundled
terrain. It changes no production collision, recovery, permission, clocks,
authentication or workflow content. Ordinary missing terrain still refuses
movement, and strict cold restoration still requires the same canonical root.

## Fixture changes and precise limits

- Snake unit/integration fixtures use ordinary map `0` with the original
  geometry uniformly translated by (+222,+199) into the starter open cells.
  The original IDs, immediate Master deaths, impacts and retired identities
  remain. The original strict root equality assertion is retained.
- Purification unit and crowded-escape fixtures use ordinary `D024`; every
  actor, ring, corridor and expected coordinate moves together by (+20,+42).
  This does not turn the existing trusted far `SyncPlayerTransform` negative
  case into proof that its out-of-bounds point is normally reachable.
- Owned-pet line hits and recalls use the real `d024` key, preserving the
  original coordinates. Lowercase matches the actual NPC configuration key,
  so the original NoPets negative case overrides that row rather than adding
  a shadowed uppercase duplicate. The explicitly static blocked-back branch
  remains a trusted synthetic arena; it does not attest ordinary cold terrain.
- Pet map transfers use distinct `D024`, `D022` and `D023` terrains with fixed
  translations (+36,+12), (+249,-4), (+225,-14). Purification transfers use
  the same three distinct terrains with translations (+17,+41), (+221,+183),
  (+196,+158). New spell target preparation reads the target's current Manager
  map, preserving the original same-map distances after transfer.
- Hell-Lord/bomb cases use `D024` (+16,-8); owned bombs use `D022` (+214,+172).
  Positions produced by actual monster spawning stay actual coordinates and
  are not translated twice.

All original test registrations and assertion bodies are retained with their
equivalent map/geometry references. No red case is deleted, ignored, renamed,
given a shorter deadline or converted to a successful result. The original
strict 500 ms optimized Gateway skill/scroll stories are unchanged.

## Actual final local checks

Selected checks observe **544 distinct Rust tests
passing at least once and 6 Node tests (550 total)**.
The non-diagnostic passing runs cover 540
distinct Rust tests. Duplicate executions are deduplicated by crate/binary/test
name. One original explicit collision-clone benchmark remains ignored and is
excluded. These counts do not establish all local gates as green: the original
strict purification/scroll test failed once and then passed a single identical
diagnostic run. That timing-sensitive failure remains unresolved.

| Serial run | Passed executions | Failed | Original ignored | Classification |
| --- | ---: | ---: | ---: | --- |
| `check-07-final-integration-workflow-gates` | 125 | 0 | 0 | selected-gate-passed |
| `check-08-final-simulation-workflow-gates` | 273 | 0 | 1 | selected-gate-passed |
| `check-09-final-classic-source-workflow-gates` | 50 | 0 | 0 | selected-gate-passed |
| `check-12-final-gateway-classic-workflow-jobs1` | 33 | 0 | 0 | selected-gate-passed |
| `check-13-final-gateway-owner-source-lib` | 11 | 0 | 0 | selected-gate-passed |
| `check-14-final-gateway-transfer-release-lib` | 7 | 0 | 0 | selected-gate-passed |
| `check-16-final-ordinary-snake-missing-map` | 10 | 0 | 0 | selected-gate-passed |
| `check-22-original-shared-zone-after-space-recovery` | 1 | 0 | 0 | selected-gate-passed |
| `check-18-final-gateway-movement-population-lib` | 30 | 0 | 0 | selected-gate-passed |
| `check-15-final-gateway-purification-release` | 3 | 1 | 0 | original-timing-gate-failed |
| `check-19-diagnostic-original-scroll-repeat` | 4 | 0 | 0 | single-diagnostic-repeat-passed |
| Original Default NPC generator | 6 | 0 | 0 | selected-gate-passed |

The failed original optimized Gateway test recorded two TownTeleport calls of
274 ms and 219 ms. Setup/checkpoint reads after the cast had already consumed
38 ms; the chain ended 31 ms after the unchanged 500 ms deadline. One diagnostic
repeat of the same four tests, command, clock, assertions and selected source
passed 4/4. No additional repeats until green were performed, no performance
fix was made, and the original 3-pass/1-fail result remains RED in the archive.
A complete new-source Linux workflow must still pass its original gate before
any artifact is qualified; a passing diagnostic cannot replace that workflow.

The final Cargo receipts hash 39 selected paths before and after execution;
all selected hashes agree with the archived final source. They do not cover
the complete workspace/toolchain input closure. The other retained Rust files
in Simulation/Gateway and the entire workflow match canonical parent Git
content. Existing `map.rs` has mixed working EOL bytes; the original freeze
receipt overstated raw-byte identity. Its separate correction and both hashes
are retained. No product file was reformatted to hide that distinction.

## Original failures remain inspectable

The [immutable receipt](generated/player-qa/terrain-fixture-recovery-20261010/candidate-01/EVIDENCE.json)
and [original evidence](generated/player-qa/terrain-fixture-recovery-20261010/candidate-01/original-evidence.zip)
retain the failed Linux job, pinned-parent cold-root failure, initial invalid
starter coordinates, adjacent cold/recall/movement failures and the uppercase
NoPets mistake, and the unresolved strict timing failure alongside its single
passing diagnostic repeat and the passing remaining checks. The archive is
2,051,140 bytes with 186 byte-verified entries,
SHA-256 `f9f719dc94433f27bd7954a7d609f58c19fc2b1adb96bc9c13c296dab188379a`. The precommit receipt is never rewritten
to retroactively claim a push, Linux qualification or release.

The first Gateway compilation failed with memory-allocation errors before any
tests started. The wrapper process receipt was unavailable after interruption;
its exit code is not fabricated. The exact log and observation are retained,
including the overlapping build-directory lock wait. The final rerun only
limits Cargo build concurrency to `-j 1`; no test condition was weakened and
no unrelated application was stopped.

The untouched shared-zone test initially could not compile: check17 reported
a memory-allocation failure before execution, and one reduced-debug build
attempt (check20) reported MSVC LNK1180 insufficient disk space. The direct
test execution planned after that failed build never ran. C: was observed
with only 20,160,512 bytes free. Automatic review rejected deleting generated
incremental cache without a more specific reason. A safer native PowerShell
move preserved its 21,570 files / 38,228,887,901 bytes in
`D:/mir2-build-cache-archive/p4-purification-20261010-incremental-01` after
checking absolute containment, no reparse points, no active compiler and
destination space. The cache inventory agrees before/after; no full cache-byte
hash claim is made. Available C: space also changed independently before the
move, so all recovered space is not attributed to this action. The final
shared-zone check returns to the original Cargo arguments, debug profile,
source, assertions and clocks after space recovery. Both failed builds and
the cache-preservation receipt remain in the archive.

## Publication and remaining work

At local closure the live Gateway is last confirmed `64b6b10c…`, client
R23/source `faef…`, signed feed18. A new exact commit/push, complete own Linux
workflow, genuine ZIP/TAR/ELF/digest/source verification, fresh drained
preflight and normal public packet checks are still required before switching.
This round changes no installer or R2 client feed.

P1 still needs actual dense-monster escape, continuous attack and stop/no-drift
Windows acceptance. The isolated opaque CheckMovement receipt draft is not
integrated into Gateway; accepted movement/queued-Turn producer, current owner
binding, original CanMove and intermediate Run checks remain separate work.
Replay identity genesis is not repaired by weakening checkpoint roots.

P2 natural late-map/Boss/equipment/skill-book trips for all three classes,
P3 natural party/Hero/Pet/Boss/PK ownership stories, P4 full summon-PK lifecycle,
P5 real two-client Guild/Mentor and restart acceptance, P6 Windows weapon
refining/failure recovery, and P7 successful ordered Source Map.Load,
population and same-Source-CAS world outbox remain open. No full P1–P7/Mentor,
100% Candidate or final human acceptance claim is made. The expired `mir2-12`
heartbeat remains paused and the blocked Goal is not marked complete.
