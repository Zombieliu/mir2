# Rounded health display and life authority — 2026-10-08

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

Exact Source f32 health percentage and event-driven GroupHero/PetHealth recipient
selection remain separate work. No protocol schema, signature, security gate,
online account/save, D/F installation or Gateway is changed by this slice.
The code is outside frozen R22 game7fea and its attested binaries. A later matched
client/Gateway build and publication is required before claiming it is live.
Native wheel acceptance remains pending for the already published R20, after
the user's normal-exit confirmation. FullP1–P7 and the missed12-hour deadline
remain unchanged.
