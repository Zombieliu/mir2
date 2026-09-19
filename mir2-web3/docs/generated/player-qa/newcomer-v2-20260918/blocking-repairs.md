# Fresh V2 blocking failures and bounded repairs

The fresh ordinary saved checkpoint remains 34/78. None of the repairs below
is a claim that the paused ordinary route, native UI or original Crystal visual
comparison has passed. The original clocks are expired and cumulative revivals
remain 0/3/1. No route has restarted with a replacement clock.

## Warrior weapon techniques

At N15 the Warrior learned Thrusting but the controller sent attackDirection
without first arming the Crystal stateful toggle. The target was adjacent;
public receipts showed no accepted owner Thrusting attack. The same missing
arming gate applies to the later HalfMoon practice.

V2 now sends the public spellToggle for the requested weapon technique and
requires a post-send SpellToggle acknowledgment for the exact owner and skill,
with canUse=true. Only then does it acquire a fresh target, align the legal ray
and attack. Missing, old, wrong-owner, wrong-skill or disabled acknowledgments
fail closed. No attack/navigation/revival cap or deadline changes.

The implementation worker's scoped V2 controller result is 43/43, including
both positive toggle-before-attack cases and negative receipt cases. These are
protocol contract tests; actual Gateway technique integration and ordinary N15
completion remain open.

The first functional recheck exposed a separate controller gate before the
packet reached Gateway: ProtocolClient's normal-command allowlist omitted the
public `spellToggle` packet. It now permits that exact ordinary command while
still rejecting QA commands; focused client tests pass 9/9. Warrior resumed on
the same persisted recheck start, with no additional attempt or revival budget.

That Warrior retry armed Thrusting and sent an accepted attack, but N15 still
paused before the authoritative damage/quest flag arrived. The Wizard N12
FireBall behaved the same way. Their positive ObjectStruck/DamageIndicator
packets appeared only when each runner issued normal LogOut after its 12-second
receipt wait. Gateway StartGame deferred the serialized runtime tick for 15
seconds; only movement woke it early, while KeepAlive merely acknowledged.
The shared Zone had resolved the hits; its combat outbounds were queued until
the serialized session drain. Gateway now treats normal attack, directional
attack, range attack, magic and CastSkill as active input with the existing
75 ms tick wake. Focused Gateway regression passes 1/1. This changes packet
delivery latency, not combat rules, attack count or the original clocks; live
revalidation on a rebuilt Gateway remains open.

Root's final affected-controller run passes 102/102: V2 43/43 plus ordinary
supplies 59/59. Raw local output is
`C:/mir2-newcomer-v2-clean-20260918/controller-blocking-repairs.tap.log`.
Syntax and scoped diff checks pass. The earlier 288-test checkpoint is retained
as history and is not presented as a rerun of this source version.

## Taoist early combat equipment

At N7 level 11, an intact WoodenSword (2–4 DC) and SpiritSword/Healing produced
mostly one-damage hits against a 36-HP ForestYeti. Accepted public attacks
reduced target 205909 from 36 to 18 HP before the unchanged 20-action cap.
Durability, rejection and missing early SoulFireBall readiness are not the cause:
SoulFireBall and Amulet require level 18 and cannot be used as a level-11 fix.

The ordinary character has 960 gold. The candidate preparation uses the existing
normal Blacksmith shop and public buy/equip receipts, preferring an affordable
IronSword and permitting a BronzeSword fallback. A narrowly scoped use of the
last gold reserve requires independently held HP/MP and retreat supplies.
Generic restocking and V1 policy keep their reserve. No reward is retroactively
granted and no character state is edited. BronzeSword balance, ordinary purchase
and actual ForestYeti completion within the cap still require live evidence;
human-facing preparation guidance remains open.

Supply regressions prove 960-gold BronzeSword purchase/equip through the live
stock contract, IronSword preference at 1200, failure with insufficient held
supplies, rechecking stock after travel, rejecting incorrect purchase debit,
and unchanged generic reserve behavior. Held supplies require at least six
HP and six MP potions plus one RandomTeleport and one TownTeleport. No attack
or search cap changed. The additional normal run completed N7 and three more
units through N9, with level16 and matching normal logout/save. This is N7
functional evidence, not a measured two-hour completion. Taoist N10 paused
against target 240470 at the unchanged 20-attack cap. Packet evidence identifies
that target as a 110-HP BoneFighter, not an objective Skeleton. The runner had
already damaged a Skeleton to 47% but diverted to clear the proven BoneFighter;
five-damage hits need at least 22 attacks. N10 now opts into the existing
focused-target safety policy so an unsafe pull retreats and reacquires a live
Skeleton instead of consuming the fixed budget on a nonobjective. The specific
Skeleton/BoneFighter regression and full combat suite pass 157/157; V2 tests
pass 44/44. No attack, search, retry, or revival caps changed. Ordinary Taoist
N10 completion on the saved character still requires live revalidation.

## Wizard contradictory monster lifecycle

At N12, Skeleton 240335 had a public dead projection at (166,317), followed by
a live projection at (154,309) without ObjectRevived. The later authoritative
snapshot still showed it dead at (166,317). The controller's final live-target
check correctly paused. Following moving coordinates alone cannot fix these
contradictory lifecycle receipts.

The pending unowned-monster filter previously validated visibility without
checking native Zone lifecycle. The repair validates queued monster and
positive-health projections against current authority, preserving death,
cleanup and genuine respawn ordering. A targeted Gateway run passes 2/2:
real Zone kill/dequeue rejects queued live/positive-health ghost packets;
real Zone respawn retains ObjectRevived and its fresh live projection while
rejecting a late corpse. The existing TownRevive/re-entry AOI regression also
passes. Raw output: `C:/mir2-newcomer-v2-clean-20260918/pending-zone-aoi-final.log`.
This closes the targeted packet regression, not Wizard N12 ordinary completion;
the repaired release and saved normal character still require live revalidation.

The latest normal logout/store verification is 38/78 overall: Warrior 16/26
level22, Wizard 12/26 level19, Taoist 10/26 level16. All three save/snapshot
transforms match. The separate functional recheck retains original expired
clocks and cumulative recoveries 0/3/1.

Functional completion, real level 30, final logout/store checks, human timing,
graduation equipment sources, UI, animations and original comparisons remain
separate open gates. `visualAccepted=false`, `measuredTime=false`,
`globalParityPercent=null`.
