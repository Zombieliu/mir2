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
or search cap changed. This is a controller candidate, not an ordinary kill pass.

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

Functional completion, real level 30, final logout/store checks, human timing,
graduation equipment sources, UI, animations and original comparisons remain
separate open gates. `visualAccepted=false`, `measuredTime=false`,
`globalParityPercent=null`.
