# Protocol newcomer journey acceptance

> Current checkpoint, 2026-09-15: isolated Gateway R54 is active at
> localhost 17800/17810. Independent saved-state validation confirms Warrior
> Lv30, 55/55 mandatory + 4/4 milestones; Wizard Lv25, 45/55 + 3/4, q89 6/9;
> Taoist Lv23, 39/55 + 2/4, q99 3/6. This is 148/177 quest/milestone units
> (83.62%), with 26 mandatory quests and three milestones left; partial kill
> progress and visual gates are not counted as completed units.
> Full controller regression passes 610/610. The latest fixes recheck paralysis
> and death immediately after movement cadence waiting, preserve Wizard's
> legal nine-tile fireball range, buy ordinary Ruben Medium HP reserve only
> for q89 town departures, and require Taoist q98/q99 to leave town with
> 12 MP drugs while retaining the four-drug field trigger. q89 held Mediums
> are preferred with a bounded 2.5-second reuse; other play retains six seconds.
> Missing/unaffordable Medium rows fall back to held Small HP and normal retreat.
> Wizard R115 and Taoist R104 are now running the ordinary public journey on
> R54. R53 Taoist SoulFireBall XP193 persisted normally; natural R54 fireball
> progression and the remaining route/visual results are pending.
> The 375-minute budget is a design target, not measured full 0→30 playtime.
> Native 30-minute telemetry is stable but the observer gate remains FAIL for
> missing successful-reconnect evidence; it does not certify held-right input.
> See [checkpoint](generated/player-qa/protocol-journey-20260915/three-class-checkpoint-1910.md),
> [tests](generated/player-qa/protocol-journey-20260915/quest-agent-tests-r117-root.log)
> and [R54 practice](generated/player-qa/shared-fireball-practice-20260915/README.md).
> Full route, animation/UI and complete active-play timing remain unaccepted;
> formalCandidate=false, accepted=false, visualAccepted=false.

## Historical R52 Gateway / Warrior journey R88 completion

The current opt-in `newcomer-v1` route has 55 mandatory quests and four
separate level-15/20/25/30 milestone claims per class. Its design budget is
375 minutes (6 hours 15 minutes), with chapter budgets of 20, 25, 60, 75, 90
and 105 minutes. This is a calibration target; complete 0-to-30 active playtime
and native visual acceptance have not passed.

| Class | Current ordinary-player protocol checkpoint | Status |
| --- | --- | --- |
| Warrior | Journey R88, Lv30, 55/55 mandatory quests and 4/4 milestones persisted | PASS across saved checkpoints; normal LogOut completed |
| Wizard | Journey R110 resumed saved R109, Lv25, q89 at 3/9 | Active; complete route remains open |
| Taoist | Journey R98 resumed saved R97, Lv23, q98 at 5/6 | Active; complete route remains open |

Current Gateway R52 listens on `127.0.0.1:17800` and
`ws://127.0.0.1:17810/ws`, with executable SHA-256
`EBA81F35BB625C239580E6A7C898F728744EA33F538FA719ED79B7C1636E9413`.
The isolated launch script enables `MIR2_QUEST_CADENCE=newcomer-v1` while
retaining `MIR2_CONTENT_PROFILE=crystal_full` and the existing ordinary account
store. Current quest-agent code includes cadence-state serialization from
`b846f2a64610b70016bf305a24b3a1b0b3a70871`, paralysis-aware breakout and
authoritative retreat-state refresh. `1e8931561887b65632ddd5a493d6f942e0772d8f`
also budgets Taoist q98/q99 at 48 minimum / 64 departure Amulets and lets
multi-objective searches take an available unfinished required monster.
The complete suite passes 582/582, with
zero failures, cancellations or skips:
[quest-agent-tests-r110.log](generated/player-qa/protocol-journey-20260915/quest-agent-tests-r110.log).
These are current runner regression results; they do not relabel the earlier
R88 trace as a new run of later code.

The current route order and fixed quest-only budgets are:

| Chapter | Mandatory IDs | Minutes | Quests | Fixed XP | Kills | Quest items |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1–5 | 1,2,3,5,6 plus Warrior 7,8,9 / Wizard 10,11,12 / Taoist 13,14,15 | 20 | 8 | 1,600 | 15 | 1 |
| 6–10 | 22,23,24,25,26,27,29,30,35,36,37 | 25 | 11 | 49,558 | 3 | 6 |
| 11–15 | 33,49,39,40,41,42 | 60 | 6 | 65,742 | 18 | 0 |
| 16–20 | 50,51,52,53,54,61,65,60 | 75 | 8 | 480,000 | 14 | 2 |
| 21–25 | 83,86,87,88,97,98,99,102,103,110,111,112,89 | 90 | 13 | 1,800,000 | 24 | 1 |
| 26–30 | 113,114,117,118,119,121,124,122,123 | 105 | 9 | 4,900,000 | 36 | 1 |
| Total | Four milestone claims are counted separately | 375 | 55 | 7,296,900 | 110 | 11 |

q4 harvesting, q62's one KekTal plus one VioletKekTal challenge and q84 thread
collection are optional. Needed item objectives are capped at one per item;
NPC-given carry quantities retain their source values. Current configuration
and the recalculated level gates are documented in
[NEWCOMER-1-30-DESIGN.md](NEWCOMER-1-30-DESIGN.md).

Warrior R88 run `2026-09-15T01-00-52-845Z` resumed the naturally saved Lv29
character `JWa3693f66` and finished in 49 minutes 44.279 seconds. Its trace
`C:\mir2-protocol-journey-20260911\Warrior.2026-09-15T01-00-52-845Z.trace.jsonl`
contains 30,257 records. A BoneArcher death at sequence 7503 produced q122
1/6, the needed CleanSkull and q124 1/1 at sequences 7509–7512. NPC1358
was visible at WhiteVillage `(296,251)` with q124 at sequences 10552/10554,
and q124 completed at 10559/10560. q122 completed at 23079 with three
BoneArchers and three BoneSpearmen; q123 completed at 27829 with three
BoneBlademen and advanced the character to Lv30. The level-30 milestone
completed at 30252.

The long-run movement audit records 3,387 sends (691 Walk, 2,696 Run),
3,385 paired `UserLocation` responses and two requests interrupted by Death
(9476→9477 and 11886→11887). No live request remains unmatched. Response
latency is p50 400 ms, p95 663 ms, p99 695 ms and maximum 1,146 ms; none
exceeds 2.5 seconds, and `navigationMovementResponseTimeout` is zero. All
51 Runs sent 350–450 ms after a Walk response received their own response,
with p95 352 ms and maximum 480 ms. The trace has 23 `MapInformation`
packets including the initial map and 22 subsequent map transitions, and all
four Deaths received Revived within 1.167–1.403 seconds without a 60-second
death-interrupted movement timeout.

Final snapshot 30254 and account save revision 7427 agree on Lv30,
BichonProvince `0 (318,275)`, HP 419/419, MP 116/116, EXP
128,441/2,000,000, 135,652 gold and 21 HP drugs. The store contains all
55 mandatory quests and all four milestones completed, plus optional q62.
`logOut` sequence 30256 received `LogOutSuccess` at 30257. Full report,
trace, task progression, response-pairing method and persistence evidence:
[warrior-r88.md](generated/player-qa/protocol-journey-20260915/warrior-r88.md).

Warrior's protocol chain passes across saved checkpoints and several repair
revisions. R88 alone proves the Lv29-to-30 closing segment, not an uninterrupted
0-to-30 timing certificate. Wizard/Taoist completion, native rendering and
animation, human feel and full current-version pacing remain open. Overall
`formalCandidate=false`, `accepted=false`, `visualAccepted=false`; this custom
profile evidence does not raise the Crystal parity score. Earlier checkpoints
below retain their contemporaneous versions, route counts and limitations.

## R47 six-hour route and Sabuk checkpoint

The opt-in 1-30 newcomer route now targets 375 minutes (6 hours 15 minutes),
with chapter budgets of 20, 25, 60, 75, 90 and 105 minutes. Its mandatory
three-class route contains 55 quests, 110 bounded kills and 11 bounded quest
items. q50 Cook Book Delivery replaces q62 Exterminate in the required
level-16-20 chapter. q62 remains available as a short optional challenge with
one KekTal and one VioletKekTal, so existing character progress is retained
without forcing a dense Insect Cave corpse run.

R47 runs on `127.0.0.1:17810` from executable SHA-256
`9E2670A8A7A529A4D5EDB43A9A31B47E9213612261E1399D9CF7F958D1AD96E7` and
keeps the existing account store. Warrior trace
`Warrior.2026-09-14T08-15-09-779Z.trace.jsonl` proves the resumed q110 route:
the character crossed D701 from `(28,22)` to its inner exit, entered the Sabuk
merchant quarter, completed q110 with zero deaths, and advanced the mandatory
count from 41/55 to 43/55 after also completing newly required q50. The same
run accepted q111 and completed all three physical pillar interactions.

Legacy migration is also live-proven. Wizard's already-complete q62 reconciled
to the new 1+1 optional objective, revived in town, skipped its optional
turn-in and completed q50 to reach level 21 and 33/55. Taoist left D2041
without advancing optional q62 and resumed the required route. Rust newcomer
journey checks pass 7/7 and the complete quest-agent suite passes 544/544.
Three-class completion and native UI/animation acceptance remain open.

## R40 paced emergency-escape checkpoint

R37 proved that funded cave departures now work through ordinary public
packets. Wizard and Taoist each acquired four Merchant Ruben
`RandomTeleport` items after the supply plan accounted for their full purchase
cost. The Taoist's cash recovery also exposed and closed a public-combat error:
safe Deer funding now uses a range-one physical attack instead of sending
`SoulFireBall` at a neutral animal. R39 then supplied live proof of that exact
physical attack path.

R39 Wizard reached the preferred D406 mine route and advanced q54 from 16/25
to 18/25 with normal `GreatFireBall` combat. Its first escape moved from
`(32,27)` to `(136,47)`; a second request 0.7 seconds later was deferred, and
the next successful use occurred about 18.5 seconds after the first. This
proved the shared cooldown conserved the reserve across independent navigation,
combat-retreat and recovery callers. The same run also found that a deferred
request disabled later attempts for the remainder of one long navigation, so
the Wizard eventually died while scrolls remained.

Deferred escape state now records a retry deadline rather than a permanent
per-navigation failure. Navigation, combat retreat and evasive recovery may
retry after that deadline, while a player below 35% HP uses the shorter
critical cooldown. Focused checks pass 189/189 and the complete quest-agent
suite passes 508/508. R40 three-class public-protocol runs are active; complete
q54/q62, all 55 mandatory tasks plus four milestones per class, and native
UI/animation acceptance remain open.

## R35 critical-navigation and funded escape checkpoint

R33 proved that the shared-Zone transform fix remains stable across two
consecutive public `UseItem` escapes. Warrior q62 moved inside D2042 from
`(58,199)` to `(146,203)`, then from `(158,191)` to `(155,104)`. Both uses
returned successful acknowledgements and persisted in following snapshots.
The player nevertheless died 16 seconds after the second escape because it
had only 19/224 HP and the recovery loop resumed movement near another group.
Wizard q54 exposed the adjacent case outside that loop: two zombies trapped
ordinary navigation at 17/68 HP and killed it before navigation returned an
error.

The navigator now checks authoritative HP and visible hostile distance before
each movement intent. During an active q54/q62 expedition it can consume a
normal Merchant Ruben RandomTeleport, requires a changed authoritative
position, clears its stale path, and replans from the new location. Expedition
stock is four scrolls. Cash-poor saved characters include the 400-gold scroll
reserve in their ordinary Deer/Venison funding target before returning to the
dungeon.

R35 Warrior live trace `Warrior.2026-09-13T17-19-39-951Z.trace.jsonl` proves
the new stock target through public shop traffic: one retained scroll plus a
fresh three-item Ruben purchase produced four authoritative RandomTeleport
items. The complete quest-agent suite passes 503/503. R35 three-class routes
remain active; full q54/q62 completion and native UI/animation acceptance are
still open.

## R32 authoritative emergency-teleport checkpoint

R32 runs the optimized Gateway on `127.0.0.1:17810` from executable SHA-256
`7CDBA4C8AAAB1C7FD1D83683942167A5259B1CC5EA7A63814C55AAFA3736DCEE` and keeps
the existing account store. RandomTeleport now samples the requested map-scale
radius before its exhaustive fallback instead of accepting the first adjacent
cell. A successful item use that emits `UserLocation` also commits the private
transform into the shared Zone before the tail snapshot, preventing the old
Zone position from pulling the player back.

Live Warrior trace `Warrior.2026-09-13T16-51-09-473Z.trace.jsonl` proves the
complete public-protocol chain on q62. At 119/224 HP and authoritative position
`D2041 (54,210)`, `criticalPackPressure` selected inventory item 717. The server
returned successful `UseItem`, `MapInformation` and `UserLocation (38,110)`;
the next two authoritative world snapshots retained `(38,110)` and reduced the
scroll reserve from two to one. The runner recorded `emergencyEscapeSuccess`
and resumed movement from the new position. This closes the previously observed
one-cell teleport and immediate Zone rollback defects.

The q54/q62 caster pressure trigger now treats three hostiles within three tiles
plus a proven recent attacker as a surround even if only one is adjacent. The
long expedition still targets 80 HP/MP drugs when affordable, but a saved q54 or
q62 caster may depart town at a 64-drug safety floor instead of repeatedly
hunting contested funding targets for the exact final units. Focused shared-Zone
teleport and simulation teleport tests pass, and the complete quest-agent suite
passes 500/500. R32 three-class live routes are still running; complete 1-30 and
native UI/animation acceptance remain open.

## R31 public emergency-scroll expedition checkpoint

The q54/q62 runner now buys an opt-in two-scroll `RandomTeleport` reserve from
Merchant Ruben through the ordinary `NPCGoods` and `buyItem` protocol. It
requires an exact item 717 shop row and proves the gold and inventory deltas.
Use goes through public `useItem` and is accepted only when a fresh
authoritative snapshot proves both a lower total scroll count and a changed
player coordinate. Normal quests retain the previous supply plan.

Live traces `Warrior.2026-09-13T16-04-13-788Z.trace.jsonl` and
`Wizard.2026-09-13T16-04-13-796Z.trace.jsonl` each prove a two-scroll purchase;
`Taoist.2026-09-13T16-04-13-799Z.trace.jsonl` proves the one scroll affordable
after its HP, MP and Amulet purchases. Warrior advanced q62 to 4/16 before this
run; Wizard remains q54 15/25 and Taoist q54 8/25. The first escape policy was
too narrow: Wizard could briefly walk out from three adjacent attackers at
38/68 HP, then was surrounded again during recovery without reaching the
no-step-only trigger. A later Warrior replay narrowed the safe trigger further:
at 128/224 HP, two hostiles were adjacent but only one had a still-current hit
record; the pack caught the ordinary retreat and killed the player with both
scrolls unused. Low-health retreat now uses the configured scroll when at least
one recent attacker plus a second authoritative adjacent hostile prove the
surround, while preserving the no-step trigger and bounded combat fallback.

The first replay exposed a liquidation error before combat: Wizard and Taoist
attempted to sell their emergency scroll to Material Dealer Reece, which the
server correctly rejected. `RandomTeleport` is now always protected from
obsolete-material liquidation and both classes have restarted from unchanged
saved progress. Focused escape/supply checks pass 167/167 and the complete
quest-agent suite passes 499/499. A successful live emergency use, complete
three-class 1-30 routes and native UI/animation acceptance remain open.

## R31 dense-route control follow-up

The ordinary Blacksmith dialog exposes Crystal's combined `@BuySell` entry,
not a separate enabled `@Sell` link. The runner now accepts that exact menu
shape but still requires authoritative `NPCSell` before liquidating spare
equipment. Live Wizard then sold ordinary items, purchased HP/MP drugs, resumed
q54 and advanced from 4/25 to 8/25 through normal packets and town revivals.

Dangerous q54/q62 supply trips now end in town so the quest-aware traveler owns
the outbound route. Wizard and Taoist q54 funding targets the full 80 MP stock;
Taoist SoulFireBall uses the same bounded ranged-kiting policy as Wizard magic.
Caster q54 retreats use a 24-step profile and eight-tile clearance.

Two dense-corridor control leaks found by live Taoist replay are closed. An
unsafe pack discovered while clearing a doorway blocker now enters the normal
defer/retreat/recover loop. If live movement has already removed the proven
aggressor before fallback combat starts, the stale travel interruption is
treated as an evasion and the route is replanned. A partial cave escape that
improves exposure and leaves no proven pursuer now preserves its movement and
replans instead of aborting at a local dead end. A blocked cave corner with no
remaining proven attacker also defers and replans after bounded breakout.
q54 now finishes available D401 Zombie2/3 targets before crossing toward D406,
and q62 retreats toward the real D2041-to-D2042 transfer `(262,13)` instead of
erasing progress toward the old entrance. Focused combat passes 96/96,
survival passes 34/34, supplies passes 26/26, and the complete quest-agent suite
passes 490/490. Trace writes tolerate bounded multi-second Windows
`EBUSY`/`EACCES`/`EPERM` scanner locks instead of aborting a live route. Current
persisted checkpoints are Warrior level 20 q62 at 3/16,
Wizard level 18 q54 at 8/25, and Taoist level 18 q54 at 4/25. Complete 1-30 and
native UI/animation acceptance remain open.

## R31 isolated trace and class-aware q54 checkpoint

The public-protocol runner now writes each attempt to its own trace file and
loads only the newest 64 MiB tail of the prior class trace as navigation
memory. The previous shared traces had grown to roughly 430-925 MiB: every
restart rescanned the whole file, while a monitor could also contend with the
writer and cause `EBUSY`. Missing new trace files are treated as empty memory.
This removes that hidden restart cost and isolates live acceptance from
read-only monitoring.

The D2042 q62 trace proved that a full potion stack was not used while the
spawn-search loop enumerated unreachable cave waypoints. Every recoverable
navigation failure now runs the cadence-limited survival callback and converts
a proven adjacent attacker into `ThreatenedNavigation`, so the normal bounded
clear/retreat flow starts before the next waypoint.

The Taoist q54 trace exposed a separate policy error: Warrior's three-attacker,
55%-HP mine thresholds were applied to all classes. Warrior keeps those measured
values. Wizard and Taoist now interrupt healthy travel on the first proven
extra attacker, retreat at 75% HP under multiple pressure, and attempt at most
one breakout kill. A stocked q54 caster reconnects directly into this policy
instead of spending 90 seconds trying to recover in the old hostile field.

StartGame now accepts the exact named personal world snapshot when the Gateway
omits its optional `UserInformation` packet, avoiding a false 60-second failure.
q62 also has a 24-drug field trigger and an 80-drug departure refill for the
long D2041/D2042 traversal. Wizard and Taoist q54 departures now target 80 HP
and 80 MP drugs while retaining the lower in-field triggers.

Focused combat passes 92/92, survival passes 31/31 and the complete quest-agent
suite passes 481/481. Live Wizard has turned in q42, completed q51-q53, reached
level 18 q54 and proved four ordinary Zombie kills before a quick town revive.
Warrior level 20/q62 and Taoist level 18/q54 have resumed from saved checkpoints
under the new policies. Full 1-30 and native visual acceptance remain open.

## R31 canonical quest endpoints and q62 retreat checkpoint

R31 runs the optimized Gateway on `127.0.0.1:17810` from executable SHA-256
`141E565F4325DF6503A518F170396BCAD4A44975AA3290D63FC2EB71444334FF`. Before
the switch, all active clients disconnected and `accounts.json` was written at
21:00:12 local time. The new server opened both TCP and WebSocket ports, then
Warrior R18, Wizard R15 and Taoist R13 resumed their saved characters.

Crystal NPC identifiers can collide because quest packets may carry either a
loaded object identity or an NPC database index. The canonical resolver now
matches `loaded_object_id` first and uses the database index only when no loaded
object matches. q51/q52 therefore label, accept, finish and show quest icons at
CraftsLady object 33, while Merchant Bull object 22 is rejected. The focused
endpoint tests pass 2/2 and the profile-gated newcomer catalog test passes 1/1.

The quest-agent survival module was reconstructed after a full build cache
filled the E: volume and a failed write truncated the untracked file. Its full
behavior contract now passes, including HP/MP departure stock, passive recovery,
moving recovery and partial-path replanning. Ordinary focused combat marks a
retreated target immediately even when another same-name target has not entered
AOI; q30's unique Currish harvest remains retryable. q62 now biases D2041 escape
toward `(35,240)`, uses a 24-step retreat, permits three threat evasions on one
travel edge and up to three proven breakout kills. A new regression proves the
first two breakout kills before safe escape. Combat passes 91/91, survival
passes 26/26 and the complete quest-agent suite passes 471/471.

Live R31 proof is in progress. Wizard resumed q42 at 19/20 and reached 20/20,
ReadyToTurnIn. Taoist had already completed q52/q53 and resumed level-18 q54 at
2/25. Warrior resumed level-20 q62. Complete 1–30 and native visual acceptance
remain open.

## R30 release Gateway and dense-field recovery checkpoint

R30 runs the optimized Gateway on `127.0.0.1:17810` from executable SHA-256
`FF7DF42B82131DBEB4377F8FA981B444782B462716133428B98808900CE0B80F`. Initial
slow-stage samples are about 0.1-0.45 seconds, compared with the R29 debug
build's roughly one-second average and 19-second peak. All three public-protocol
runners reconnected and resumed persisted characters. The full quest-agent
suite passes 469/469.

Taoist q42 uses the measured 1-adjacent/3-nearby target limit. In R11, an extra
RedSnake's directed ObjectAttack woke the current RedViper response wait in 29
ms; `focusedTargetRetreat` was followed by a retreat command 1 ms later, before
the later ObjectStruck. The first authoritative displacement followed the hit
in 784 ms, the complete retreat reduced exposure from two adjacent/two within
three tiles to zero/zero, and the runner returned to kill the RedViper. q42
advanced from 6/20 to 8/20 without a death.

Warrior completed q54 at 25/25, received the reward, reached level 19 and
accepted q60 `Help the Farmers`. The first q60 attempts exposed two runner
defects: a 45-second follower recovery ended at 78/207 HP despite 68 HP drugs,
and an unreachable SpiderFrog chosen by emergency breakout leaked a raw
`UnreachableCombatTarget`. q60 now gets a 90-second, eight-tile moving recovery;
breakout quarantines an unreachable proven attacker and tries the next bounded
attacker. R17 recovered for about 65 seconds to more than 75% HP, completed all
eight SpiderFrogs, physically returned from D2041 to turn in q60, accepted q61
`Part Time Job`, travelled to MasterShok and completed q61. The apparent q60
pause was its real return journey, not a stuck runner.

Wizard q42 reached RedViper 4/10 but the old 12-MP departure target caused a
second long village trip after only two kills. The supply gate now retains the
four-drug field trigger and separately requires 32 MP drugs after restocking;
the q42 route also permits a healthy character with at most two kills remaining
to finish from a four-HP-drugs-per-kill reserve rather than immediately making
another full-map supply trip. The focused tests and complete 469-test
quest-agent suite pass. The running
Wizard will load this policy at its next safe town restart. Taoist q42, Wizard
q42 and later quests remain under live acceptance. Native visuals are still a
separate open gate.

The follow-up live runs closed both changes. Wizard R12 restored q42 at 7/20,
entered the village with two MP drugs, bought through the ordinary Ruben shop
flow to an authoritative stock of 32, and returned to map 2 with 31. Taoist
R12 completed RedViper 10/10 and TigerViper 10/10, crossed from map 2 to map 3,
turned q42 in to Merchant_Bruce and accepted q51. The Taoist route had no death,
process exit or navigation loop. The reduced-stock finish policy applies to
future runs because this R12 process had already begun its second town trip
before the code changed.

## R29 D401 arrival and evasive recovery checkpoint

R29 runs the development Gateway on `127.0.0.1:17810` from executable SHA-256
`07D9D24B86ED44F54064FC072A249A5F524088633499F0277B35F503F18EA269`.
The full quest-agent suite passes 463/463 and the respawn integration set passes
3/3. Gateway check and the installed debug build also pass.

Warrior resumed at level 18 with q54 at 22/25 and 80 HP drugs plus BronzeAxe.
At the R28 D401 entry `(24,181)`, three-to-four monsters occupied adjacent cells
immediately and killed the pinned player. R29 preserves each broad Crystal
respawn group's configured count while relocating slots out of an eight-tile
ring around every manifest arrival. The R29 entry at `(25,181)` observed no
initial hostile within eight tiles; the nearest were 10, 12 and 13 tiles away.
The Warrior left the entrance alive, traversed D401 to D411 and then D406, and
advanced q54 to 23/25 without a death or revival. Two Zombie1 kills remain.

Taoist resumed q42 at 4/20 from 42/89 HP with four HP drugs. The prior runner
stopped sustaining as soon as it reached a seven-tile safety gap and timed out
despite those drugs. R29's runner executes the same cadence-limited sustain once
per outer recovery cycle. Live evidence records remaining-drug use roughly six
to seven seconds apart, full HP recovery, a normal HP-threshold supply trip,
and an authoritative refill to 32 HP and 12 MP drugs. It did not repeat the
false evasive-recovery timeout. Wizard resumed q42 at RedViper 2/10 and
TigerViper 0/10; all three routes remain active. This is functional public
protocol evidence. Complete 1–30, one-day pacing and native visual acceptance
remain open.

Latest r25 checkpoint: compile/build passed at SHA-256
`5EFB51A48211897F8B1559E793AEDC9E28B24A68FEC89CC41F99FD7C84908A87`.
Five simulation tests, four Gateway cadence tests and the full 129/129 Node
protocol suite pass. The selected-character ECS position getter narrows pending
transform reads. Candidate travel now chooses the nearest graph route: map 2 is
one hop away while map 3 is two, with the existing fallback retained. Search and
approach interruption remains bounded to a proven aggressor.

The subsequent survival/kiting harness suite passes 143/143; the complete
quest-agent suite passes 344/344. After each `completeQuestObjectives` call, the
runner can make an ordinary return-to-village restock. The loop is limited to
32 attempts and stops when funds are insufficient; HP supply detection covers
every Drug item type. Wizard
retreat planning remains collision checked and limited to three cells per move
and eight moves per attack attempt. It is enabled only at affordable range with
at most two hostiles, and stops on death, map change or absence of a safe escape.

This corrects the r24 diagnosis: the route selected the first candidate on map 3
despite TigerSnake also existing on map 2, then crossed map 2 without combat;
Wizard and Taoist died during that transit. The proven-aggressor interruption
was useful but was not the root cause of that failure. r24 subsequently exited 0
with writer count 0.

The three r25 runs began at 23:47:04, 23:47:14 and 23:47:24 UTC. Warrior made
40 attacks and advanced q33 TigerSnake from 3 to 7; six sampled UseItem responses
succeeded in 384–726 ms, then the runner continued to the next single snake with
no HP potions and died. Wizard cast six FireBalls, reducing MP from 126 to 114;
two HP UseItem requests succeeded, the last returned false at death, and the
runner died while two nearby snakes could not be kited. Taoist used Healing four
times and attacked five times, then died with no HP potions while facing two
nearby snakes. These observations identify limits in the bot's between-engagement
restocking and kiting strategy; they do not establish a game-balance defect.

The first bounded-kiting live attempt halted the Wizard alive at 38/45 HP after
reaching the former 12-cell budget; only three FireBalls were proven, with MP
moving from 129 to 120. A follow-up tied the retreat budget to the existing 120
attack-attempt limit, allowing at most 360 retreat cells without changing the
three-cell/eight-move local bounds. Its r25-budget run began at 00:18:18 UTC and
ended at 00:18:51 after six FireBalls with
`No collision-safe Wizard retreat on 2 from 471,409`; the Wizard remained alive
at 45/45 HP. Server 26160 then exited 0 with writer lock 0. Read-only file
verification matches the latest persisted
revision 697 at map 2 position 471,409, HP 45 and MP 105.

All three characters are paused and saved. They remain level 12 at 15/53 core
quests and 0/4 growth milestones; Warrior q33 is 7/16 while Wizard and Taoist
remain 0/16. The live attempts prove that retreat movement occurred, but do not
accept the kiting strategy. Safe escape and target selection beyond the strict
local three-cell range remain open, together with live latency, one-day pacing,
complete routes and visual acceptance. The persisted state is verified, so the
flush-close log is not treated as evidence of a new backend defect.

Earlier r19 recovery checkpoint: R18 froze its isolated File-only account store
after an interrupted publication. The exact reviewed source contained six
accounts and 11 characters at SHA-256
`34053E050535B8A3919DC7F304B95AF81110C7620FC54E7B2FB86DC5253F2F18`;
`recovery-backup-r19` preserves the pre-reconciliation evidence. The bounded
File-only reconciliation completed, startup consumed all three teardown
journals, and no new quarantine appeared. Clean r19 restarted at PID 179320,
server session 26519, from executable SHA-256
`BC97573EAA33CA7AC59546CDB3FA4E95AA28423E7A382E14B7A2268A1EC78572`.
The r19 live owner-HP object-ID correction is verified. Wizard subsequently
completed q26 and q27, so Warrior/Wizard/Taoist are now all level 10 at 14/53
core quests and 0/4 growth milestones. Protocol checks pass 121/121; the
profile and guidance checks each pass 6/6. Earlier runs confirmed uneven
UseItem latency: Warrior observations took 13.240 and 19.692 seconds, while
Wizard observations took about 2.2–2.5 seconds. Warrior's TownRevive wait also
timed out, although later authoritative state placed the character in map 89
town; that timeout is not counted as a proven successful revive. Those early
observations led to the thresholded r20 sampling summarized above. The newcomer
route now orders q27 -> q35 -> q33 while
preserving 53 core quests and the fixed 7,296,900 experience budget.

r20 was built at SHA-256
`021CC291B56A6F3AB541A9F97A68EC489D8EA1E6E45FE0A9FED0CC5838C2797E`.
After players disconnected, its opt-in console `shutdown` path logged
`gateway operator shutdown received; settling...` in
`gateway/stdin-shutdown-r20.log`, exited with code 0 and left the publication
marker empty. The Gateway restarted at PID 189400 in PTY session 84801; later
runs produced the current q35 checkpoint above. This proves the idle explicit-console path only; it
does not prove active-session draining. Full routes, one-day pacing and visual
acceptance remain open.

The local WebSocket runner validates the opt-in newcomer-v1 journey independently
of desktop input. Its results are protocol functional evidence, not Windows
visual acceptance, Crystal animation parity, or human time-to-30 measurements.

Run from the project root with Node 22 or later:

```powershell
$env:MIR2_JOURNEY_OUTPUT = 'C:/mir2-protocol-journey-20260911'
$env:MIR2_GATEWAY_WS_URL = 'ws://127.0.0.1:17810/ws'
$env:MIR2_JOURNEY_CLASS = 'Warrior' # Wizard or Taoist
$env:MIR2_JOURNEY_PLAY = '1'
node apps/web/scripts/quest-agent/run-protocol-journey.mjs
```

Use a local QA Gateway configured with `MIR2_QUEST_CADENCE=newcomer-v1`.
The runner creates its own ordinary account and character. Keep the generated
`*.private.json` files private and outside version control. Repeated runs reuse
these credentials and the character's normally saved progress. Reports and
redacted JSONL traces are written to the output directory.

The mandatory denominator is the shared journey configuration's 53 quests per
class, not every eligible quest in the source route manifest. Bootstrap alone
never sets `completed=true`. The runner verifies task operations against exact
request acknowledgements and authoritative quest stages. Walking uses local
collision data and normal directional commands; combat uses normal actions and
server observations. It does not seed progress, grant items, teleport, accelerate
server ticks, or send admin commands.

Implementation and acceptance are in progress. Ordinary scripted travel and
village potion purchasing now have protocol implementations and focused tests;
their complete route coverage remains unverified. Current limitations include
full class skill tactics and human findability/pacing. A failure must remain visible in the report and be diagnosed;
it must not be bypassed by modifying the character save.

Live checkpoint on 2026-09-11: all three independent characters registered,
were created and entered the existing local r10 Gateway. All completed q1–q3
through normal walking, Scarecrow combat and NPC interaction. Warrior and Taoist
also completed q5 (five Deer and five Scarecrows); their hand-ins verified
experience 180→380, gold 200→230, and WornIronBracelet 0→1. The other route tasks
remain in progress or unverified. Reports reside under the configured local
output folder; they are not visual acceptance evidence.

The first run exposed harness issues: a spawn search continued past visible
targets, and an unrelated monster death could terminate the current engagement.
Both are corrected in source, with target-specific death and health-percentage
regression coverage. Interrupted attempts remain in the trace; only authoritative
quest completion counts, never inferred kills. Character reconnects preserved
partial objective progress.

Source review also found the first Diary-only task q22 had no usable server
operation path. The newcomer-only server product fix passes five focused packet
tests and two adjacent NPC-dialog tests. Native Diary controls pass four focused
tests and Windows metadata passes its explicit-zero test. Native visual acceptance
remains pending. The supplied
Crystal client exposes Diary controls, but its server source likewise requires
an NPC. Therefore this change is not claimed as default Crystal server parity.

The isolated r12 Gateway runs at `127.0.0.1:17810`, using a copy of the normally
saved character store taken after stopping protocol players. No XP, quest state,
items or positions were edited in that copy. The earlier r10 Gateway at 17710
remains available to the desktop client. r12 executable SHA-256:
`866F651C9CC6469ADAAC638EB6C587CEA03647725C6FA8AE6DBEC8E173022603`.
Raw build/test/runtime logs and the executable are in the local output folder.

All three characters reached level 5 and completed the HookingCat target in q6.
Warrior has advanced to q8. Wizard and Taoist deaths are retained as failures,
followed by normal TownRevive, not silent resets. Temporary navigation failure
and sparse-spawn searching required harness corrections. Death count and bot
travel duration do not yet establish the human difficulty or one-day target.

The r12 Windows executable was built and copied beside r11 in the existing local
QA asset package, without launching it. SHA-256:
`8799CEA04C7746E78B1CEB28C66E300118A23ED66601DA9354B508BDC9CC806C`.
Its separate QA-VERSION manifest preserves `accepted=false`,
`visualAccepted=false`, and `formalCandidate=false`. Existing r11 UI screenshots
do not validate the new r12 Diary controls.

The protocol suite passes 72 tests after adding live transfer-source collision
handling. A fresh snapshot's exact selected entrance may override a static wall,
matching the server; dynamic occupancy still blocks it. The Wizard subsequently
entered map 0115 through normal movement. Its q10 instructor dialog still exposes
only Exit and leaves progress at 0/1, so q10 is not accepted as completed. Taoist
navigation to its distinct entrance remains under investigation. These are open
failures, not successful end-to-end route acceptance.

The q10 failure was a product persistence bug: an accepted dialogue-only quest
was ReadyToTurnIn before reconnect and incorrectly recomputed as 0/1 afterward.
Reconciliation now retains the same readiness semantics as begin_quest. Two
focused tests pass, including real save/login/StartGame and an unfinished kill
quest negative case. The isolated r13 Gateway was rebuilt (SHA-256
`C4D2595E3E4A6F38A6314B26CE8449CBFAEAA784C29E3F2D455301F57C6C89E6`)
and started on 17810 with the same normally saved account store. Wizard's normal
reconnect and q10 hand-in then passed; q11 was accepted. Returning from the house
exposed a separate harness issue when already standing on the exit source cell.
The current protocol suite passes 76 tests. No native visual acceptance occurred.

Further live checkpoint: Taoist q13 handed in after using the server's ordinary
NPC interaction range, then accepted q14. Warrior completed q22/q23/q24 and
reached level 9. q22 verified PrecisionPendant 0→1 and gold 353→403; q23 verified
BronzeShortSword 0→1 and gold 403→486; q24 verified the completed stage and gold
486→686. These are normal local protocol results, including Diary operations.
Wizard still cannot leave map 0115: an explicit step off/re-entry receives
UserLocation at the source but no MapChanged. Server diagnosis remains open.
Harness regressions now pass 78/78; full 1–30 and visual acceptance remain open.

The house exit is a shared-cadence product defect: a delayed admitted movement
emits UserLocation after the original request's transfer check has already run.
The pending authoritative transform must be consumed and checked for an exact
source-cell transfer. The first implementation's packet-presence pending flag
was rejected in independent review because one request can acknowledge an older
step while queuing a new one. The revised implementation compares the consumed
authoritative position with the personal mirror. Four focused tests pass; live
rollout remains pending final regression review/build.

At 15:48 UTC the live count was Warrior 11/53 (level 9, q25 2/4), Wizard 6/53
(level 5, q11 0/10), Taoist 6/53 (level 6, q14 5/10). Warrior's first plant was
harvested successfully; death interrupted the second. Both active runners then
spent roughly twenty minutes searching without attacking. Their runner processes
were stopped, relying on the Gateway disconnect-save path: sparse search rings
did not cover the full declared spawn spread. This is a harness coverage gap,
not evidence that collection drops failed or that human pacing meets one day.

2026-09-12 local-time checkpoint: final cadence fix passes four focused tests
(including the newer third step landing on the portal after the older step's
acknowledgement) and two adjacent existing map tests. r14 executable SHA-256 is
`4CEC66E9F03FD6660D144E00CEC8E96EF33868D6EBE2E7AC3D7AD00803D08839`.
It runs on the same isolated 17810 port and unchanged saved account store.
Wizard then actually exited: trace sequence 567 at 2026-09-11T16:06:28.780Z
contains MapInformation fileName=0 followed by UserLocation at (315,476).
The harness incorrectly timed out because it only recognized MapChanged;
support for actual MapInformation and a fresh post-transfer snapshot is pending.
The full-spread search and ten-minute diagnostic guard pass focused coverage;
the combined protocol/summary suite passes 84 tests. Three-class completion
and native visual acceptance remain open.

Actual MapInformation support is now implemented with stale-AOI invalidation,
subsequent UserLocation landing proof and fresh destination snapshots. Wizard
reconnected normally on map0. Full-spread searches exposed a second harness
inefficiency: interleaving distant candidate regions visited only 9/264 points
in 671,112 ms. Searches now use deterministic nearest-neighbor traversal, keep
observed-location revisits first, and enforce the ten-minute limit during
navigation as well as between waypoints. The combined suite passes 89/89.
All three runners restarted against r14; further task progress is still pending.

Subsequent protocol hardening passes 93 tests: sustain checks use only held
supplies while attacking/harvesting (two-second throttle), and cross-map monster
selection skips unreachable candidates via the normal travel graph. q60/q62
have reachable alternatives to their first generated spawn map. q53 carry items
are granted by begin_quest and become ready without a separate carry command.
Wizard's q11 advanced to 9/10 and Taoist q14 to 7/10 with the revised search;
Wizard suffered another ordinary death, retained in evidence. All three runners
were restarted with the sustain hook. Deaths do not reset completed objectives.

Later live evidence separates additional cases. Taoist's q22 no-credit death
was a contested ordinary kill: a remote Wizard delivered the final hit, received
36 EXP and q22 progress; no server award defect was inferred. The protocol now
retries only proven remote last blows without inventing credit. It also handles
one-second player-death settlement after target removal and clears only proven
adjacent hostile aggressors. The combined regression suite passes 100 tests.
Wizard completed q22–q24, reached level 9 and sent normal FireBall commands;
animation/UI remain unverified.

An actual shared visibility defect remains open: after TownRevive to (288,616),
Warrior received a live CannibalPlant 205839 projection for distant (125,513).
It later walked to (129,517) at 16:52:20.952Z, within ordinary AOI, without a fresh
projection for that object, while Wizard independently observed the same living
plant. Other nearby monsters did appear. Protocol runs were stopped for a
targeted server queue/visibility regression and fix; extending search duration
is not treated as a remedy. No progress save was edited.

The first proposed visibility filter is not accepted: its focused regression
failed at the initial spawn assertion (0 instead of 1), before exercising the
revival condition. Independent review additionally found that pending packets
are consumed before TownRevive changes the authoritative transform, whereas
the initial fixture queued its stale spawn after revival. Both the fixture and
transition ordering are under correction. No rebuilt runtime contains this
unverified change yet; isolated r14 remains the last running Gateway.

Source review narrows the visibility finding: CannibalPlant (AI5) starts hidden
and checks for a hostile player within Chebyshev distance 3 every two seconds.
Both recorded Warrior return points were distance 4, so absence there is not
evidence of a re-entry defect. The far-town projection remains invalid. Live
retest must approach within 3 and allow the normal reveal interval. The initial
regression's zero spawn was caused by this expected hidden AI; the generic queue
regression now uses an initially visible monster and queues before TownRevive.
Its corrected implementation defers those projections until the final revive
transform before authoritative visibility validation. Test execution is pending.

Final focused checks now pass: queued-revival projection 1, adjacent revival 1,
learned passive progression 2, adjacent magic progression 1, and next-attack
Fencing accuracy synchronization 1. The latter uses a thin trusted combat-stat
getter, avoiding a second complete world snapshot on each physical attack.
Initial stat-sync compilation failed on a missing wrapper method; it was fixed
and the final thin-getter regression passed. Gateway r15 build is underway.

The protocol suite remains 101/101 passing after loadout/reveal refinements.
Remembered CannibalPlant positions are approached within two tiles and observed
for at least 2100 ms before any attack; only actual server entities are usable.
Uniform zero-Luck weapon comparisons permit proven DC upgrades without losing
other relevant stats, including Wizard MC and Taoist SC. Taoist fallback applies
only without a learned offensive skill. This addresses script gear-selection
gaps without editing rewards or saves. Later spell access, equipment purchases,
supply pacing, full 53-task routes and all visual acceptance remain open.

Isolated Gateway r15 is now built and running on localhost:17810 with the same
unchanged character store. Executable SHA-256:
`75DA6B48EC3977A6E5F6B2CD75F91E7CD3F2AE7DBBE91C4DFC970428A450D4AE`.
The verified r14 process alone was stopped; the separate desktop server was
not touched. All three protocol players logged in normally and restored their
levels/completions (Warrior 9/11, Wizard 9/11, Taoist 6/8), then resumed q25/q25/q22.
Live verification of the new fixes is pending subsequent authoritative events.

Live r15 follow-up: Warrior received actual Fencing MagicLeveled updates
(experience 1, 4, 7, 10 at 18:02 UTC), completed q25 harvesting and reached level
10 with 12/53 complete, then started q26. Taoist completed q22–q24 and reached
level 9 with 11/53 complete. Wizard reached q25 2/4 but stopped when plant 205839
left its snapshot before confirmed death; that target lifecycle is being checked
against the other player's trace. Other runners continue. This is functional
protocol evidence only; no native animation or UI acceptance occurred.

Taoist subsequently completed q25 and reached level 10 (12/53). Wizard's failure
was followed by authoritative self death about 1.8 seconds after target removal,
not a competing Warrior kill (the Warrior harvested a different plant). The
target-loss settlement is now bounded at three seconds so delayed death enters
the existing normal revival path; a living player's unexplained target loss
still fails. The early Remove versus later Hide ordering remains diagnostic.
Wizard also now keeps six-tile FireBall range while affordable, waits during
cooldown without sending a melee attack, and approaches a corpse only to harvest.
Insufficient MP uses the existing melee fallback. Combined harness tests pass
102/102; Wizard restarted normally with these changes, others continue q26.

The next runs are paused on two newly diagnosed defects. Warrior/Taoist q26
searches timed out after 600266/600154 ms (51/235 and 62/233 waypoints), despite
crossing manifest-backed SpittingSpider regions. Actual asynchronous movement
ingress bypasses the normal execute path's incremental monster activation; its
later pending-transform drain updates position but misses that activation. A
production-path regression and correction are being prepared.

Wizard's next failure sent zero attacks: its last authoritative snapshot had
FireBall cooldownRemainingTicks=3635 after relogin, inherited from a previous
session's absolute tick deadline. The harness consumed its 120-attempt allowance
waiting for that frozen snapshot. Waiting no longer consumes attack attempts;
it uses bounded normal snapshot refresh and a 30-second stall limit (combined
suite 104/104). Durable cooldown persistence must rebase remaining time while
preserving same-session rollback deadlines. An initial reset-only prototype is
not accepted: original player reconnect behavior retains remaining cooldown.
The first new-session fixture also failed its pre-save cast assertion; both
semantics and fixture are being corrected before any rebuilt runtime is used.

The revised durable clock design records versioned remaining milliseconds plus
save wall time inside existing skill JSON, subtracts elapsed offline time on
load, and caps against the configured cooldown. Legacy unmarked absolute times
expire; raw checkpoint timing is retained for same-session rollback. Four focused
tests passed before final deterministic-idle encoding coverage was added. Idle
skills now serialize zero timestamp/remaining values, preserving unchanged-save
equality. Final focused and adjacent tests are running.

Final cooldown checks pass 5/5, including deterministic inactive encoding;
the unchanged-save atomicity adjacent case passes 1/1. Hydration now passes its
real-ingress regression after correcting the fixture's default starter-slice
collision boundary. The test uses an explicit bounded collider (not a global
full-map toggle) with a real manifest-generated target; ordinary Walk must still
enter its AOI. Four cadence/map-transfer adjacent tests pass. These isolate
activation and ordering; full-map collision fidelity is not newly claimed.
Gateway r16 build is underway, with normal-character live verification next.

Gateway r16 is now running on localhost:17810 with the unchanged saved account
store. SHA-256:
`639870EA4CC9717F01B9D65EB54759A7AE59745FF19B3D18801DC27DFCDDBB34`.
Live normal sessions beginning 19:12–19:15 UTC observed SpittingSpider packets in
all three classes. Warrior collected q26 materials (2/2) and proceeded to q27.
Wizard sent 13 normal magic commands by 19:16 UTC, completed q25 and reached
level 10. Taoist is also level 10 on q26. This verifies ordinary distant monster
activation and usable post-relogin magic in the isolated protocol runtime.
Full 53-task routes, pacing and native visuals remain unverified.

The hydration fix runs after actual pending movement and after any map transfer,
using the final map's normal dormant-monster activation. Its first regression
incorrectly required an immediate Walk acknowledgement; it now permits the real
cadence worker to consume the queued ordinary intent before the serialized drain.
The passing hydration result and live r16 deployment are recorded above. Idle
isolated r15 was stopped after all runners ended; account storage was retained
unchanged.

The next checkpoint remains incomplete: Warrior has completed 14/53 core quests
and Wizard/Taoist 12/53 each, all level 10. Shared corpse contention is distinct
from the reproduced private-session live projection preceding the Zone respawn.
The harness now bounds unavailable-corpse retries without granting quest credit;
the Gateway suppresses retained unowned monster projections from personal output.
Its first regression fixture incorrectly tried to kill a Zone monster through
personal lifecycle packets; the revised fixture uses authoritative Zone combat.

Newcomer-only level 15/20/25 class rewards now include existing books/equipment;
level 30 retains its gold reward. Functional route completion requires 53 main
quests plus four ordinary Board reward claims. The initial milestone regression
miscounted Amulet rewards because they enter the Belt before the Bag. Revised
coverage checks Belt delivery and normal MoveItem/EquipItem use. Rust reruns are
completed: four focused and all 11 newcomer module tests pass. No live class
reward acceptance is claimed. Protocol tests now pass 118, including definition
cache survival after event eviction, real Board dialogue, Belt relocation,
paired accessory slots and damage-triggered authoritative vitals refresh.

Warrior's final r16 failure was not an empty potion supply: four HP potions were
held while eight attackers dealt 64 damage in about 9.5 seconds. Relative damage
indicators did not refresh the harness's absolute HP snapshot, so navigation's
potion condition never fired. The harness now requests ordinary clientVersion
snapshots on self damage, coalesced to at most once per 500 ms; it does not infer
absolute HP by subtracting display damage. Live survival proof remains pending.

The scheduled-spawn projection regression now proves no early live projection
or revival, but its final scheduled revival still fails. Investigation found
standalone ObjectHarvested has no shared delivery anchor and is filtered before
the Zone harvest gate is marked. This is under correction; it must pass retained
dead/harvestable validation and duplicate suppression before r17 live testing.

Final shared harvest checks pass 2/2 and Gateway projection checks pass 2/2.
Two omissions required correction: shared_object_action_packet first discarded
ObjectHarvested, then shared_object_result_id provided no delivery anchor. The
result now preserves the corpse ID, passes existing canonical dead/harvestable
guards, and commits its harvest marker. Tests cover duplicate idempotence,
scheduled revival and a second harvestable incarnation. The real manifest-backed
Gateway fixture proves private AOI reactivation produces neither an early live
projection nor a revival, while later scheduled Zone revival still passes.
Earlier failed fixture attempts and logs are retained, not counted as passes.
Concurrent personal inventory reward reservation is a separate unresolved gate;
this correction does not claim atomic cross-session harvest custody.

r17 built successfully and is running on localhost:17810, using the same saved
accounts. SHA-256:
`309834F529A64250552BB19891011B3E3FAF6C65E433AEF6E0A19259E9CDF7B7`.
Three ordinary protocol players resumed; live results are pending.
Final logs: shared-harvest-tests-r2.log and
personal-monster-projection-tests-r5.log under the protocol evidence directory.

r17 live: Taoist completed q26 PoisonSack and q27, reaching 14/53 core tasks;
Warrior is also 14/53 and Wizard remains 12/53, all level 10. Warrior/Taoist
equipped SteelBangle on the empty right wrist through ordinary equipItem. Runs
are paused and r17 is stopped for the next owner-wire correction.

Observed UserInformation and worldSnapshot use owner ID 1000, while self
DamageIndicator/ObjectStruck/ObjectDied still carried the Zone ID 50000. The
owner normalization boundary now rebases only the exact owner target ID for
damage, struck, death, revive and poison; remote targets and attacker IDs remain
unchanged. Focused normalization 1/1 and death/potion/town-revive/save adjacent
1/1 pass. r18 build is running; live self-damage refresh is not yet verified.

Wizard's last r17 ClientVersion request received its authoritative snapshot
after 3.444 seconds, beyond the harness's 3-second limit. KeepAlive remained
responsive; this was delayed, not missing, output. The callback now allows six
seconds and still requires a post-request worldSnapshot. Its eventual HP0 result
will enter ordinary revive handling. Full routes, pacing and visuals remain open.

Gateway r44 is running on localhost:17810 from release executable
`mir2-gateway-snapshot-r44-release.exe` (SHA-256
`03B68E192DFEE9E4F5DB7DE26AB672A9A439A5B7CD079075DC6793C1777B037A`).
The shared-session potion bridge now commits personal health/mana tick output
back into Zone vitals before the next shared tick. Focused Gateway coverage
passes 1/1 for a normal MP potion tick and 2/2 for the related shared item
teleport module. Taoist live snapshots retained MP 40/102 across later Zone
ticks, then funded and bought 64 MP drugs, 12 equipped Amulets and four
RandomTeleport scrolls through ordinary shop, sale, move and equip commands.

All three r44 public-protocol resumptions remain alive. Warrior carried 80 HP
drugs and four RandomTeleport scrolls into D2041 for q62. Wizard carried 79 HP,
64 MP and four scrolls through D401 to D406, sent ordinary magic commands and
advanced q54 from 18/25 to 19/25. Taoist left town for the mine after completing
its funded expedition stock. These are live route checkpoints; q54/q62, later
quests, all three full level-30 routes and native visual acceptance remain open.

r45-r54 live follow-up on gateway r44: the expedition settlement window now
waits for the observed delayed shared kill award, q54 casters prefer the D406
Zombie source, and resumed q54/q62 players keep their field position while at
least one RandomTeleport remains. The 23-file Quest Agent suite passes after
these changes. Warrior resumed in D2041 with three scrolls without returning to
town, reached D2042, and advanced q62 from 5/16 to 6/16. After an ordinary death
it used TownRevive, harvested six Deer through the public multi-pass Harvest
flow, sold the Venison, restored HP stock to 80 and RandomTeleport to four, and
kept q62 progress. Wizard reached D406, completed and turned in q54, reached
level 19, and accepted q60. Taoist reached D406 without a debug transfer and
advanced q54 from 9/25 to 16/25 without dying. Current mandatory completion is
Warrior 31/55, Wizard 29/55, and Taoist 28/55. Later quests and native visual
acceptance remain open; these observations are functional public-protocol
evidence only.

r53/r56/r60 continuation on gateway r44: Warrior advanced q62 to 13/16,
retreated through the physical D2042 -> D2041 -> map1 -> map0 route while alive,
and retained its authoritative objective progress. It is currently using the
ordinary Deer harvest/sale funding loop because its 18 gold cannot buy another
RandomTeleport. Taoist's D406 run completed every q54 component (Zombie5,
Zombie2, Zombie3, Zombie4 and Zombie1 all 5/5) and received the authoritative
25/25 ready-to-turn-in state; it is physically returning through D411 to
Blacksmith_Bill. Wizard q60 exposed a D2041 one-cell corridor occupied by a
passive KekTal. The bounded q60 policy cleared that single nearby doorway
occupant with ordinary GreatFireBall casts, opened the collision route, killed
the exact SpiderFrog, and received ChangeQuest 2/8. Quest Agent coverage is
517/517 after the corridor regression. These remain functional protocol
checkpoints; the three level-30 routes and native visual acceptance are open.

r54/r55/r62/r58 continuation on gateway r44: the bounded q62 target-density
policy admitted the live 0/2 and 1/1 KekTal candidates and Warrior completed
q62 at 16/16. Its first completed-objective return exposed that the global
navigation escape filter stopped recognizing a cave quest after it changed to
ready-to-turn-in; Warrior died in D2041, used ordinary TownRevive, and retained
all objective progress. Emergency RandomTeleport protection now remains armed
until q54/q60/q62 is handed in. The new ready-return regression and the complete
Quest Agent suite pass at 518/518. A fresh r55 session then travelled normally
from Bichon to Master_Shok, handed in q62, advanced mandatory completion from
31/55 to 32/55, and entered q65 without another death. Wizard advanced q60 from
4/8 to 5/8 and Taoist from 2/8 to 4/8 in D2041 after both funded 80 HP drugs and
four RandomTeleport scrolls through public combat, harvest, sale and shop
packets. Their q60 completion and the corrected ready-return escape remain live
acceptance gates; later quests and native visual acceptance remain open.

r60/r67/r73/r74 continuation on gateway r44: Warrior completed q99's
FlamingWooma component at 10/10, restocked through ordinary harvest, sale and
shop packets after one TownRevive, returned to D022 and advanced WoomaFighter
to 4/10 while retaining q89 and q99 progress. The revised ranged q60 density
policy admitted the natural D2041 SpiderFrog packs: Wizard advanced 7/8 to 8/8,
returned through the physical cave route, handed q60 in and reached level 20;
Taoist advanced 5/8 to 6/8 without a death. Wizard's same-session level-up then
exposed an absent q2100020 definition cache entry even though the authoritative
Board dialog and quest snapshot offered the exact reward. Milestone validation
now accepts that narrowly bounded source only when the live Board operation link
and the complete generated reward preview exactly match the expected class
reward. Quest Agent coverage passes 528/528. A fresh ordinary-protocol session
then claimed q2100020, received exactly Gold 10000 and SpearWithHook x1, equipped
the weapon, and raised milestone completion from 1/4 to 2/4. Current mandatory
completion remains Warrior 39/55, Wizard 31/55 and Taoist 31/55; the remaining
level-30 routes and native visual acceptance remain open.

r61-r63/r69 continuation on gateway r44: the q99 Warrior repeatedly reached
the same final WoomaFighter as its live spawn group closed from one adjacent
and two nearby monsters to two adjacent and three nearby. The bounded q99
focus policy admitted that measured closing group, killed the remaining
53/150-HP WoomaFighter, completed the exact 10/10 FlamingWooma plus 10/10
WoomaFighter objective, and advanced Warrior mandatory completion to 40/55 at
level 24 without another death. Taoist's funded 32-Amulet departure completed
another D2041 SpiderFrog and advanced q60 from 6/8 to 7/8; one ordinary death
and TownRevive followed, and the saved objective progress remained intact.
Wizard's first q65 D421 crossing cast GreatFireBall/FireBall through a measured
two-monster passage and killed the blocking Zombie3, then exited only when a
later transient no-walk-path result escaped the quest loop. Living no-walk-path
failures are now retried only for q54/q60/q62/q65 while preserving the
authoritative field position. Quest Agent coverage passes 532/532. Taoist q60,
Wizard q65, the later routes and native visual acceptance remain open.
