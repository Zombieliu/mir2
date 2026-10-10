# Classic map transfer terrain availability — 2026-10-10

The current Candidate closes two terrain availability gaps in ordinary map
transfer observation. A configured transfer cannot be triggered from an
unavailable installed Zone. Crystal movement destinations use the existing
raw-to-gzip terrain loader and reject missing, blocked or out-of-bounds terrain.
An explicitly installed open test arena remains supported, and a valid closed
door landing remains valid under the original terrain-only `Map.ValidPoint`
rule. This is a bounded Candidate, not full map-load or movement parity.

## Implementation and source scope

- `routing.rs::current_zone_transfer_key` reads installed collision and current
  presence under the existing Zone → movement lock order. It checks the teardown
  fence, both session indices, the real Manager route and the normalized cached,
  presence and route map names. It returns no transfer on a mismatched binding.
- `ZoneRuntime::has_available_collision` observes installed collision; it does
  not certify successful Crystal `Map.Load` or world population.
- `map.rs::crystal_manifest_movement_destination_is_valid` uses the existing
  raw/gzip loader. It checks bounds and base blocked cells, preserving the
  original separate door handling and the real 0120 shop exit to map 2.
- No ordinary missing-map fallback, admission bypass, shorter movement clock,
  authentication change or production admin path was added.

The parent source is `431520196068b710c6588bf53c75a599c2f726bd`.
Root owns product, test registration, workflow, docs, Git and publication.
Independent review covered the final eight changed product/test/workflow files;
their exact working-file hashes are in the evidence receipt. Local command
snapshots cover **24 selected paths**, not the entire workspace or toolchain.

## Actual local validation

The five final serial runs pass **146 distinct tests**: 11 new tests and 135
retained regressions. One original benchmark remains ignored and is excluded.
The source hashes before, after and at evidence closure agree for all 24
selected paths. The original strict 500 ms Gateway skill/scroll checks and
original movement clocks are unchanged.

| Final run | Passed | Scope |
| --- | ---: | --- |
| Gateway transfer and original movement | 36 | Seven new adapter checks, existing owner ordering, cadence, allowed Turn and map/poison regressions |
| Simulation terrain, transfer and cold recovery | 87 | Four new destination checks, Source decoding/I/O, collision, doors, metadata and checkpoint regressions; one original benchmark ignored |
| Ordinary missing-map collision | 5 | Missing terrain stays unavailable |
| Original shared movement | 14 | Existing Walk/Run/Turn and movement behavior |
| Optimized normal Gateway map flow | 4 | Actual poison/powder/cooldown/dead and strict Purification/TownScroll flow in both actor orders |

The new Gateway cases use authenticated ordinary NewAccount/Login/
NewCharacter/StartGame setup, Walk/Run/Turn, KeepAlive and acknowledged normal
logout/relogin. Trusted server-only transfer setup creates the missing source
map fixture. They inspect actual Manager routes, session indices, saved map and
authoritative position. These are in-process Gateway adapter tests, **not TCP,
WSS, native GUI or autonomous queued-Turn acceptance**. KeepAlive does not prove
that an idle queued Turn independently triggers a portal.

The immutable [receipt](generated/player-qa/map-transfer-availability-20261010/candidate-01/EVIDENCE.json)
and [original evidence archive](generated/player-qa/map-transfer-availability-20261010/candidate-01/original-evidence.zip)
retain all local failures and final logs, exact sources/runners, original owner
fixture, the failed parent CI job log and qualified predecessor metadata/log.
The archive contains 86 byte-verified entries, 1,204,683 bytes, SHA-256
`a1f75f30bab890e5133f20a532f420962fe816858fcaf18b98d572ccf6a1d49a`.
An original transient log-download error is kept privately; its archived copy
redacts only a temporary signed URL credential and records both fingerprints.
The original evidence receipt records the precommit state and is not rewritten
after later publication.

## Parent Linux failure and fixture repair

Source `431520196068b710c6588bf53c75a599c2f726bd` was actually pushed with an
exact guarded ref/commit readback. Its own
[Linux run 38013314930](https://github.com/Zombieliu/mir2/actions/runs/38013314930)
**failed** the original rejected-combat/owner-order gate: 16 passed, five failed.
The separate siege job passed. The complete failed-job log is retained; an
earlier failed full-log download is also retained and does not count as a log.

The five failures use the intentionally synthetic `owner-order-fixture` map.
It has no terrain files and previously depended on ordinary missing terrain
being treated as unbounded. A local parent baseline reproduces those same five
failures (three passes). The repair adds exactly seven constructor/comment
lines before Join to install the fixture's explicit open arena. Removing those
lines reproduces the parent file byte-for-byte. All original assertions,
coordinates, FIFO/ACK checks, clocks, grace and registration checks remain.
The production missing-map guard is retained. The original CI workflow gains
one additive transfer gate; no existing gate is removed or weakened.

The genuine new product red baselines are also retained: destination checks
were two passes/two failures; Gateway transfer/binding checks one pass/six
failures. Their final corresponding checks all pass.

## Publication state at Candidate closure

| Layer | Actual state |
| --- | --- |
| Bounded I/O predecessor `9fe7c45f…` | Exact Git publication and its own [Linux run 38006259110](https://github.com/Zombieliu/mir2/actions/runs/38006259110) passed; genuine ZIP/TAR/ELF/source/digests and full original logs qualified |
| Unavailable-collision parent `43152019…` | Exact Git publication succeeded; own Linux run failed as described above; not deployed |
| This transfer Candidate | Local checks and immutable evidence complete; new exact commit/push, Linux artifact and public packet validation still required |
| Public isolated Gateway | Last qualified deployed source `64b6b10c05732048da9c6bf5ccb0a491d2d053d3`; its prior 62 public protocol checks passed |
| Client | R23/source `faef…`, signed feed 18; this round changes no installer, R2 feed or native client |

No switch is inferred from an earlier successful source, a test count or a
Candidate binary. A new exact-source Linux run and genuine artifact must pass
before a fresh drained-state publication check. Existing real saves, original
realm/config/resources and other games are preserved; no forced kick is allowed.

## Remaining work

An accepted CheckMovement causal receipt is still required to bind a portal to
the actual consumed Walk/Run/Turn action rather than an unconditional current
foot position. Queued/same-position Turn, rejected/dead/paralyzed/cooldown
actions, intermediate Run portal semantics, claim-to-commit races, Join/load
admission and complete source map population remain open. No position/snapshot
or generic SaveTransform is treated as successful action proof.

Dense-monster right-click escape/continuous attack and native human acceptance,
natural late-map/Boss/equipment/skill-book stories, full Source ownership,
Taoist-pet PK/lifecycle, two-client Guild/Mentor/restart, Windows refining and
ordered durable map-event execution remain separate P1–P7/Mentor gates.
The expired 12-hour heartbeat stays paused. The blocked old Goal is not marked
complete and no full classic 100% claim is made.
