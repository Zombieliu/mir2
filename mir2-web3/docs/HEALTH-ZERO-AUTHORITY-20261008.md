# Rounded health display and life authority — 2026-10-08

## Current delivery boundary — 2026-10-08

Actual R22/game7fea, invited Gatewaya41 and signed feed17 are now public on
origin and R2/CDN. Strict drain, normal service stops, cold restart, exact aliases
and44 ordinary public WSS checks pass. The packaged native R22 process187896
was observed through first_main_update for manual login; human wheel/visual
and natural crowded-combat acceptance remain open. See the
[actual rollout evidence](generated/player-qa/classic-20261008/delivery-checkpoint-05/README.md).
The health changes below are in the separate mutable b235 lane and remain
outside frozen R22. Publication of R22 does not deploy or fully accept them.

## Session lifecycle and Source arithmetic follow-up at b235 — unpublished

The personal `SimulationSession` mirror also consumed `ObjectHealth(0)` as
death. That production route is now removed: only actual `ObjectDied`, explicit
`ObjectRevived` and trusted absolute vitals can change its monster life state.
Ordinary authentication and entry tests cover all three classes, unchanged
positive HP under display0, real death/revive, and the Harvest eligibility
boundary. The Harvest fixture supplies a trusted shared deer and position;
it is not a natural kill/drop/repeated-reward end-to-end acceptance.

Source single-precision percentage and expiry byte conversion are centralized.
The actual C# expressions independently produce the committed golden vectors:
53/100 displays52, 1/1000 displays0, and expiry minimum-five is applied before
the byte cast (256 seconds becomes0). Personal HP/Hero MP wrappers, Zone player
and monster wrappers, party entry, Revelation, same-life monster teleport and
zombie revival use these numerical helpers. Invalid pools, universal zero-MaxMP
conversion, full Revelation caster SC/RevTime state and full recipient/ordering
parity are not established by these vectors. Authoritative death, drops, reward
deduplication, delayed-action invalidation and PK guards are unchanged.

Final bounded checks: Session lifecycle3/0, C# vector wrappers5/0, Zone death6/0,
personal Revelation1/0, mirror projection5/0, group reducer8/0, genuine TCP/WS
health-zero transport7/0 and gateway monster projection2/0. Adjacent revival6/2,
special AI2/2 and player settlement10/1 retain five failures that reproduce at
the unmodified parent937ac636. Failure names and counts match; process-specific
checkpoint payloads are not asserted byte-identical. Initial compile failures
and two zero-test selections remain retained and do not count as coverage.

The baseline archive's 1375 selected tracked files remain byte-identical to
their extracted tar members and match parent Git content after declared text
EOL conversion. This is not a claim that Git archive CRLF bytes equal LF blobs.
Source hashes, commands, original logs and independent vectors are bound in
[root proof05](generated/player-qa/classic-20261008/health-authority-05/ROOT-FINAL-05.json)
and [archive05](generated/player-qa/classic-20261008/health-authority-05/ARCHIVE.json).
Read-only review found no new lifecycle/reward blocker within this scope.

Full Source health recipients, HP/MP owner packets, trusted unique shared Hero
identity, retained healing/expiry refresh, revival packet ordering and the Web
percentage0-to-exact-HP0 path remain open. Primary Web edits are unrelated and
were not changed. This backend follow-up is outside frozen R22/7fea and is not
deployed. Historically at10:23 UTC, R20 was observed running with one WebSocket
connection. The subsequent R22 rollout above passed strict drain; human
shop-wheel acceptance remains open.

## Historical Zone/Gateway/native cache slice at parent937ac636

This bounded slice fixes a low-health defect in shared Zone/Gateway and native
packet caches. Crystal can display0% while an actor still has positive absolute
HP. ObjectHealth carries display information; treating its percentage as death
can clear actions, suppress living actors, and create premature corpse/drop
authority. The Source percentage formulas are intentionally unchanged here.

## Source and resulting behavior

Crystal Server/MirObjects/MapObject.cs:51–54 derives PercentHealth with a byte
cast of Health/(float)MaxHealth*100. Client/MirScenes/GameScene.cs:5103–5112
sets PercentHealth and HealthTime on ObjectHealth, without changing Dead.
HealthChanged supplies absolute HP; Death/ObjectDied and Revived/ObjectRevived
are separate lifecycle events.

- Percentage0 does not create a retained dead ledger, corpse, PK or drop-owner
  proof. Known exact positive HP remains positive and movement can continue.
- Real Death/ObjectDied and already typed dead/HP0 keep their separate guards.
  Stale positive health cannot resurrect a known dead overlay or repeat rewards.
- Native self HP0/dead is captured before display normalization. In the absence
  of an actual alive/new-life packet, stale positive percentages cannot replace
  that exact zero. HP0 by itself does not manufacture a death event.
- Local Revived clears the current-self old ObjectDied overlay using the existing
  helper, so subsequent health is accepted. A later genuine Death wins over the
  earlier Revived, stale positive health and an old live personal snapshot.
- Display0 retains an empty health-bar border and the original5000ms timeout.
  Repeated snapshots do not renew it. Actual dead actors remain separate.

## Current raw-only incarnation acknowledgement

After an actual raw live snapshot confirms positive root/self HP and dead=false,
the adapter retires earlier alive packet precedence. A later typed HP0/dead now
wins without needing an accompanying Death echo. Only current-map worldSnapshot
ingress observes this acknowledgement, before map/packet overlays; already
overlaid packet-first cache replay cannot acknowledge it. Dedicated Death=true
still outranks stale typed live data. HP0 with dead=false does not invent death.

The independent read-only review found both the earlier indefinite alive flag
and a first attempted cache-replay regression. Both findings are addressed in
the final raw-only design. Final authority17/17, bridge102/102, health display7/7
and map-ingress5/5 pass against the11-file current source binding.
[Root binding](generated/player-qa/classic-20261008/health-zero-authority-04/ROOT-FINAL-04.json),
[original-byte archive](generated/player-qa/classic-20261008/health-zero-authority-04/ARCHIVE.json).
The original -01/-02 receipts and the observed -03 explicit-Death append remain
unchanged; they do not by themselves bind the final native changes. The append's
author is not established. The root's RED14/2 and superseded GREEN16/0 are retained.

## Historical verification and retained failures

Original Round1 source/log receipts, follow-up02 and the root lifecycle regression
are retained byte-for-byte in
[the archive](generated/player-qa/classic-20261008/health-zero-authority-03/ARCHIVE.json).
[Earlier root source hashes and selection](generated/player-qa/classic-20261008/health-zero-authority-03/ROOT-REVIEW.json)
record the earlier10-file review. The original -03 format failure/green logs
remain outside that earlier archive; the current full native format check
passes and the11 source hashes still match the final tests.
[Current format addendum](generated/player-qa/classic-20261008/health-zero-authority-04/FORMAT-ADDENDUM-04.json)
binds the unchanged final source to that check without rewriting old receipts.
Earlier selected evidence is225 executions /224 distinct names, not225 new tests
or a percentage of game parity. Native authority14/14, original bridge102/102 and
health display7/7 pass. Server, owner-ID, death/PK/drop and cold-restore focused
regressions also pass. Existing rejected-backpressure identity mapping remains.

The ordinary TCP/WS story logs in, groups the accounts and receives percentage0
health plus both accepted Walk acknowledgements and peer movement. Its map and
HP are privately prepared. It does not prove natural battle death, crowded
escape, packet-loss death, native rendering or human acceptance. Real native
attack/poison death and Source reward/fault tests are separate focused suites,
not one continuous end-to-end player story.

All earlier compile, fixture, backpressure and native RED attempts remain.
The statue sleep checkpoint test fails both current source and baseline724e64d:
cold restore intentionally revokes experience_owner with old online epoch/session
proof. No test is skipped and no stale ownership authority is restored. The
baseline failure remains open independently of the percentage display correction.

## Explicit remaining limits

The acknowledged-live ordering gap is closed in the latest raw-only follow-up.
If no raw live acknowledgement arrives, there is still no comparable life/revision
identity to distinguish a delayed old-dead snapshot from a later genuinely dead
one. Dedicated Death remains authoritative; full causal life synchronization is
not claimed. Timeout or snapshot-count guesses must not replace real authority.
The terminal corpse animation without Death has not been verified; a retained
Revived standing animation hint may still affect that presentation.

At this historical checkpoint, exact Source f32 health percentage and
event-driven GroupHero/PetHealth recipient selection remained separate work;
the current follow-up above closes only the bounded numerical part. No protocol
schema, signature, security gate,
online account/save, D/F installation or Gateway is changed by this slice.
The code is outside frozen R22 game7fea and its attested binaries. A later matched
client/Gateway build and publication is required before claiming it is live.
Native wheel acceptance remains pending for the already published R20, after
the user's normal-exit confirmation. FullP1–P7 and the missed12-hour deadline
remain unchanged.
