# Shared monster integration — work in progress

The requested scope is all personal special monster behavior migrated to the shared Zone update and combat/lifecycle authority. This is an intermediate checkpoint, not completion or Windows acceptance.

Verified first batch: 23 new integration tests (Wooma 4, visibility/dig-out 8, stage summons 3, revival 4, AxeSkeleton 4). Shared-zone regression 204 and checkpoint library 15 passed on preceding integration states; additional production edits require a final rerun.

Implemented boundaries: Zone-only ticking and saved optional monster state; Wooma rage/whole-map teleport and AOI ordering; plants/centipede hide, Zuma wake, dig-out effects; BoneLord/Zuma stage waves; AI25 randomized saved revival lifecycle and fresh per-death drops; AI8 retreat/ranged window. Stale personal monster spawn/state broadcasts cannot replace shared authority. Harvest transaction acknowledgements remain supported. Hidden/stone damage and control admission are checked centrally.

Remaining differences and work:
- AI25 ordinary respawn uses a single slot and defers replacement while its corpse can revive. Crystal can replenish the slot before the old corpse revives; separate instances/counting remain open.
- Deterministic Zone RNG differs from Crystal's global stream. Owner drop bonuses and runtime custom map drop overrides are not supplied to these spawn/death helpers.
- Wooma teleport and summoned spawn placement enforce free occupancy. Original placement can overlap. Whole-map rejection sampling has a scan fallback; no load benchmark is claimed.
- Stage summon targets currently prioritize players; full opposing summon/pet target parity remains open.
- Armadillo/Elder only have dig-out lifecycle in this checkpoint. Their attack/flee hooks and sleep/reactivation families are being implemented next.
- Other ledger families, attack plans and death hooks remain pending. No global parity percentage, new package, live gateway test or visual acceptance is claimed.

References: Crystal Server/MirObjects/Monsters at source revision 92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d. Test logs are stored beside this file.

## Second intermediate checkpoint

The focused suites now total 45 passing tests across Wooma 4, visibility 8, summons 3, revival 7, reactive/fox 7, sleep 5, Armadillo 2 and BombSpider 9. The latest combined log reran Armadillo/fox/sleep/spider (23) plus shared-zone 204; the remaining 22 tests passed in earlier logged batches. Zone library 58 and poison projection/legacy serialization 2 passed separately. These counts are not a claim of a final all-files run: work continues.

Added: Armadillo player attack/flee and owner push/AOI ordering; FrostTiger sitting/reactive aggro; DragonStatue non-death sleep and zero-health retained-cache exception; Red/WhiteFox retreat/teleport/slow; BombSpider delayed single explosion; authoritative PoisonResist equipment projection and replay compatibility. Stale native remove/harvest/AI metadata is rejected. The sleeping statue's cached death inference was fixed and late-join recovery tested.

Still open: incoming player damage retains one HP in the existing shared engine; green-poison periodic player damage; spider/pet and Armadillo/non-player target completeness; full movement side effects (trade/rental/concentration/ground spell); AI39 producer and AI41/42 nodes are being added. Do not interpret the mechanism tests as full monster-family or game completion.

## Third intermediate checkpoint — September 9

69 unique new integration tests have passed across intermediate source states: prior 45 plus RootSpider producer 4, nodes 2, deer 3, Thunder 3, TrapRock 1, Shinsu 4 and GreatFox/Guardian 7. The sixth batch verifies GreatFox 7, Shinsu 4 and shared-zone 204 together; subsequent TownArcher and lifecycle work requires another final-source run. Failed exploratory/fixture runs are retained as diagnostics and do not count as passing evidence.

Added real shared effects: RootSpider delayed child creation and inherited lifecycle, YinDevilNode stat effects, Deer timid/retaliation behavior, TrapRock ring/group collapse, Thunder ordinary-damage immunity and repulsion damage/rewards, Shinsu form locks and two-cell delayed breath, GreatFox stages/recall/delayed attacks and Guardian pulls. Accuracy export now reads original Stat10 for all 555 templates; Shinsu is 25. Independent player control-poison clocks and MagicResist equipment/replay projection are being verified. Source ordinary Paralysis is 32, LRParalysis is 256; new producers and CaveMaggot/IncarnatedZT/CharmedSnake no longer conflate them.

The new read-only `coverage-audit-20260909.json` maps all 47 personal file stems to actual source families and shared hooks. Its read snapshot predates Shinsu/GreatFox/TownArcher additions. Its file counts are neither monster counts nor completion percentages.

Open shared-engine boundaries remain substantive: full opposing monster/pet/Hero target semantics; player fatal damage and death custody; periodic player green-poison damage; complete teleport/push transaction, channeling and map-policy side effects; source dynamic respawn replenishment and custom drop policy. AI57 routes/GM metadata and several other dedicated families are still open. This is ongoing implementation, not an assertion of game completion or a Windows package/visual acceptance.

## Fourth intermediate checkpoint — September 9

The complete simulation run `seventh-intermediate-full-simulation.log` passed **1,975 tests: 1,503 library and 472 integration, zero failures/ignores**. This compiled source predates the last TownArcher missing-disposition gate, Shinsu expiry/status-lifecycle fixes and the Hell/Hugger/Mud/Boulder/periodic-poison integration now underway. It is an intermediate full baseline, not evidence that those later changes pass. Independent game-data tests passed 40/40; the read-only source check verified Accuracy Stat10 for all 555 templates.

Current working code adds unified player-life retirement for delayed actions, including authoritative vitals, PvP death, observer death/revival and reincarnation. New Hell97–99, Hugger69/70, MudZombie108/Boulder170 and counted player poison leases are being wired and verified. They remain unverified until the next result below is recorded. The existing shared nonfatal boundary and complete multiplayer target/death/transaction parity remain open.

## Fifth intermediate checkpoint — September 9

The next combined run passed **319 integration tests**: 107 dedicated shared-monster tests, 204 shared-zone tests and 8 deterministic replay tests. The Zone unit run passed 65. This closes the tested Hell97–99/Hugger69–70/Mud108/Boulder170 player mechanisms and earlier lifecycle/status fixes at that source state, including 11 Hell, 6 Hugger and 10 Mud/Boulder tests. Source replacement cannot extend an old poison; unrelated monster death broadcasts cannot retire the player's life; control-poison clocks do not resurrect an expired counted poison.

Subsequent working changes add Football68, Horned163–165, precise personal application of Zone damage, GM projection, fatal shared damage and ordered gateway vital deltas. Those require another verification pass. Horned169/171 are actively being implemented separately. The earlier inference that Commander copies its attack stats into BoulderSpirit was retracted: Crystal SpawnBoulder does not assign those stats; the imported zero DC/MC is genuine source data. No fallback damage is invented.

## Sixth checkpoint: ordered player settlement (intermediate)

`mir2-shared-gateway-regression.log` records 165 shared Gateway passes and
one existing PostgreSQL-environment ignore. The three separate Gateway logs
record passing cross-session heal/damage ordering, legacy integer queue
migration, and immediate exact-instance death-drop publication. The Simulation
settlement log records Football 4, personal damage/death settlement 6 and Zone
lethal damage 3 passing tests. These results precede the generation-stamped
receipt implementation and later AI hooks; they are not final-source coverage.

Active integration now includes life-generation/receipt deduplication, Horned
encounters, Kirin/Snow Wolf, Holy Deva, Tree Queen, Evil Mir, Yimoogi, Meow,
castle-gate occupancy and remaining pet-specific updates. Tests written for
those later changes must run before marking their leaves verified. Gate war
ownership and Evil Mir DragonSystem authority remain explicit integration
boundaries. No native package, interactive game acceptance, deployment or
whole-game completion is claimed by these server-side results.


## Seventh checkpoint: native pet damage and shared settlement (intermediate)

The archived `mir2-shared-stone-rental-resolved.log` verifies StoneTrap 5,
personal damage/death/rental 11 and shared Zone 204 tests on that intermediate
source. `mir2-shared-current-zone-unit.log` verifies 70 Zone library tests before
the subsequent incarnation work. `mir2-shared-stone-transfer-2.log` includes
4 passing transfer lifecycle tests and historical StoneTrap fixture failures;
it is not a wholly passing log. The later resolved log supersedes those failures.

Gateway vitals filter passed 3 tests; the four separate old-death, identity,
migration and exact-drop logs each passed one test using that built Gateway
executable. This does not certify the latest simulation source or a full Gateway
rerun. Vampire public integration then passed 3 tests before the new monster
incarnation and unified entity combat foundation.

Actual shared monster spawn/revival now receives a life incarnation, including
summoned SnakeTotem/TrapRock children; metadata resync must retain the life.
Counter checkpoint/fork preservation and new lifecycle tests are pending here.
Unified wild-monster attacks against real owned monsters, and fourteen special
families' non-player branches, remain active work. `globalComplete=false`;
no native visual acceptance, final-source full test claim or overall percentage.


### Life incarnation follow-up (before unified entity integration)

`mir2-shared-incarnation-lifecycle.log`: 8 monster lifecycle plus 4 transfer
lifecycle tests passed. `mir2-shared-incarnation-zone-unit-resolved.log`: 72 Zone
library tests passed. `mir2-shared-vampire-life-tests.log`: 3 vampire lifecycle
unit tests passed, including captured target revival and exactly-once rewards.
The adjacent log records security 20 and vampire integration 3 passing, with
shared Zone 203 passed / one old delayed-bite assertion failed. That assertion
has been changed to test Crystal's same-update bite; its rerun is still pending.
The ongoing entity combat integration supersedes this intermediate source;
these counts do not validate that unfinished integration.


## Eighth checkpoint: real entity targets and summon inheritance (intermediate)

The old “fourteen missing families” list is superseded by
`remaining-shared-update-20260909.md`; 47 counts personal modules, not game
completion or certified species. Current entity target references distinguish
player life and monster incarnation, including corpse completion versus actual
removal. Search, visible impact and impact have separate rules. Wild-monster
hits damage real owned monsters; no proxy player session or invented pet-owner
kill award is created. Green, Slow and Paralysis leases persist through
checkpoint/fork and clear independently on actual life retirement/purification.

Version-bounded passing runs archived here:

- `mir2-shared-entity-family-resolved.log`: Hugger 11, Meow 6.
- `mir2-shared-entity-base-resolved.log`: entity integration 3, shared Zone 204.
- `mir2-shared-entity-horned-tests.log`: Horned base 11.
- `mir2-shared-entity-encounter-kirin.log`: Horned encounter 11, Kirin/Snow 9.
- `mir2-shared-entity-status-zone-unit.log`: Zone library 87 (supersedes the
  separately archived 80/84 intermediate runs).
- `mir2-shared-entity-evil-tucson.log`: EvilMir 9 passed; Tucson had one invalid
  fixture (Shinsu cast beyond its range). This log is not wholly passing.
- `mir2-shared-tucson-hell-resolved.log`: corrected Tucson 7 and Hell 13 passed,
  including old-parent incarnation protection. Later mixed Hell changes are
  outside this passing source snapshot.
- `mir2-shared-inherited-targets-resolved.log`: stage summons 4, Meow 6,
  Kirin/Snow 9, SnakeTotem 2 passed. BoneLord's pet-only wave actually hits its
  inherited pet after the two-second lock, identically after checkpoint.

Earlier family/Zone logs include failing fixture/old timing assertions; later
resolved logs supersede those specific failures. The first inherited-target
compile failed on missing imports/reference type and was corrected before its
resolved run. `mir2-shared-fox-statue-armadillo.log` is an unfinished-integration
compile failure (Hell poison API being added), not failed runtime validation.
GreatFox, statue/centipede, Armadillo, spider, Hell, TreeQueen and Yimoogi remain
in the active combined integration round; final-source regression is pending.

This checkpoint does not certify complete Crystal targeting/LastHitter/EXPOwner,
pet PvP, Hero, DragonSystem or the whole game. No Windows package or visual
acceptance has been performed. `globalComplete=false`, `accepted=false`.


### Combined-family and target-assignment follow-up

The first combined-family run compiled and passed entity integration 4 and
Armadillo 6, but GreatFox had four fixture failures. Three old damage fixtures
used magic resistance 10 (which correctly prevents MAC hits); they now use
MR0 for damage and have a separate MR10 no-damage assertion. HolyDeva's summon
fixture was moved into the actual legal summon range. Corrected GreatFox 15,
TreeQueen 7 and Yimoogi 8 passed in `mir2-shared-families-corrected.log`.
That log still includes the SnakeTotem fixture's same-update action assertion
failure: stable actor ID order lets the monster run before the totem sets its
target. The resolved test verifies its next eligible movement decision.

`mir2-shared-entity-final-families-rest.log` separately passed entity 4,
Armadillo 6, Hell 17, spider 17 and statue/centipede 4. It contains the old
SnakeTotem, TreeQueen and Yimoogi fixture failures, so is not wholly passing.
TreeQueen's fixture had legally pushed its pet outside the later root area;
Yimoogi fixtures lacked ordinary attack admission and inspected ObjectItem
instead of the actual ObjectGold packet. Their substantive assertions remain.

`mir2-shared-forced-target-public.log` passes Meow 7 and SnakeTotem 3, including
real full-cap totem assignments that change subsequent movement and a species
range attack. `mir2-shared-entity-zone-unit-resolved-final.log` passes 101 Zone
library tests. This includes actual Dazed attack blocking while pursuit remains
possible, Frozen blocking both, and a hidden root poison lease that survives
Queen retirement but ends with the root's own lifetime. These runs precede the
last Mud/Hugger target-assignment additions and final full regression.


## Bounded shared update integration verified (2026-09-09)

2026-09-09 bounded shared update integration implemented for all 47 existing
personal dedicated monster modules: owned-monster combat, status leases,
targets, summons, death/retirement and checkpoint/fork are connected.
Full simulation baseline: 2193 passed; later HellKnight guard: Zone
102 + Hell/shared Zone 221 passed. Inventory restore fix: 98 passed.
Gateway full baseline: 785 passed / 7 failed / 1 ignored; all initial
failures resolved across persistence reruns (4 + Q1-Q4) and RPC 30 reruns.
Receipt namespace 4, drop regression 47, economy/replay 39 and Gate11 2 pass.
Later corpse expiry fix: full simulation 2197, Gateway expiry 2 pass.
These are version-bound, overlapping results, not a fresh final-source full Gateway run.
Evidence and remaining Crystal-only gaps:
`docs/generated/shared-monster-ai-20260908/shared-update-delivery-20260909.md`.
Next gaps include independent respawn groups/live IDs and legacy corpse migration.
No Windows package or visual acceptance; globalComplete=false, accepted=false.

See [delivery report](shared-update-delivery-20260909.md) and [verification](shared-update-verification-20260909.json). Historical checkpoints above retain original failures and version limits.
