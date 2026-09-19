# Fresh V2 blocking failures and bounded repairs

The first fresh ordinary saved checkpoint was 34/78; the latest is 40/78. None of the repairs below
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
revalidation on a rebuilt Gateway was required. The immutable release from
`3e5a29924` (SHA-256 `9ECC3826A3BDB531A59C9740049AB6455DA5BAA2482D55C68B8D20736C614BC8`)
was deployed to the isolated 19800/19810 service. Warrior N15 completed on
the ordinary account, with normal logout and matching save; the Wizard's
FireBall hit also arrived promptly instead of at logout.

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
pass 44/44. No attack, search, retry, or revival caps changed. The ordinary
Taoist completed N10, reached level18, logged out normally and matched its
save. N11 then paused on another nonobjective 110-HP BoneFighter: 12 accepted
SoulFireBall hits and eight accepted self-Heals consumed its fixed 20-action
window. The same focused-target policy now applies only to Taoist N10 and N11;
the 20-action cap remains unchanged. N11 live revalidation is still open.

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
The deployed release delivered the Wizard's FireBall hit promptly. N12 still
paused because the V2 practice driver sent GreatFireBall while its public
cooldown projection was one tick; Zone correctly rejected that cast. The
driver now waits for fresh ordinary `clientVersion` snapshots to confirm
readiness before a practice spell and uses a refreshed live target. Wizard
N12 on the saved character remains open until the next ordinary retry.

Warrior N16 separately proved a correct Thrusting toggle, legal two-tile ray
and accepted ObjectAttack, but the accuracy roll missed. The practice driver
now permits at most three ordinary swings inside its existing 12-second
receipt window, requiring a fresh toggle, live ray target and a positive owner
damage receipt. The objective combat cap remains 20. A missed-then-hit
regression passes; live N16 is open.

The latest normal logout/store verification is 40/78 overall: Warrior 17/26
level24, Wizard 12/26 level19, Taoist 11/26 level18. All three logouts and
save/snapshot transforms match. The separate functional recheck retains the
original expired clocks and cumulative recoveries 0/3/1. Current affected
tests pass V2 46/46, combat 157/157 and supplies 59/59; test logs are under
`C:/mir2-newcomer-v2-clean-20260918/` with the `v2-*-repairs-final` prefix.

Functional completion, real level 30, final logout/store checks, human timing,
graduation equipment sources, UI, animations and original comparisons remain
separate open gates. `visualAccepted=false`, `measuredTime=false`,
`globalParityPercent=null`.

## Subsequent ordinary stop and evidence repair

The next same-clock retry kept the saved tally at 40/78. Warrior N16's
Thrusting practice did receive its positive server flag, but its objective
loop cleared a fresh 155-HP Zombie3 that had attacked the player before a
visible 80-HP objective Zombie3; twenty accepted hits left the fresh monster
at 35%. Warrior N16 now uses the existing focused-target policy, keeping
the wounded required-species target ahead of that proven aggressor. The
target-selection regression passes, and the 20-action budget is unchanged.

Taoist N11 advanced to 2/3 Skeleton kills with class practice done, then
could not path out of four adjacent hostiles at 53/113 HP. The scoped cave
policy now permits its already-held ordinary RandomTeleport at 65% HP rather
than waiting for the general 35% threshold; the existing one-scroll usage
limit remains. Neither fix is yet a completed ordinary route unit.

Wizard N12 suffered a real Scorpion strike while at 8/72 HP and died. The
public death packet and saved 0 HP agree. Its historical three revivals are
already spent, so this character cannot lawfully continue under the current
three-revival rule. The new GreatFireBall readiness wait was entered but no
GreatFireBall was sent before that death. No extra revival or clock reset is
claimed.

On Taoist's previous normal LogOut, the Zone committed 280 experience after
the runner's pre-logout snapshot file and before LogOutSuccess; the saved
character included that experience. The runner now writes its final snapshot
after `close()`, and the read-only verifier prefers the actual public
worldSnapshot received during logout when one exists. Reverification of the
unaltered packets/store restores 40/78 with all three normal logout/save
transforms matching. This repairs evidence timing, not character state.
Current focused runs pass V2 46/46 and combat 158/158; syntax and diff checks
pass. The Wizard hard stop means 78/78 is impossible in this same-clock run.

## Same-clock continuation, 19 September

The fixed shared functional-recheck deadline remains 12:36:27 UTC. The
Wizard's real death and exhausted 3/3 revival allowance remain recorded; no
further Wizard run is attempted. Warrior and Taoist continue on their original
characters through public login and player commands only.

Taoist N14 exposed an accounting error: 17 accepted SoulFireBall hits plus
three accepted self-Healing casts exhausted the 20-attack limit while a
Zombie3 still had 4/155 HP. Self-Healing is now charged to the existing
bounded consecutive-wait time, not to the target's offensive-attempt count;
the 20-offense cap is unchanged. Warrior N19 exposed a separate stale hint:
a dead Dung in the current snapshot was classified as a live V2 search
waypoint. V2 now excludes dead current-snapshot monsters when historical
hints are disabled. Neither correction awards quest progress on its own.

The runner also retains the last complete public worldSnapshot around normal
logout, because Zone map cleanup may send a later empty snapshot. It probes
once before logout with the ordinary `clientVersion` command, then retains
the latest complete public projection for strict save comparison. It does not
alter the save or replace a failed route with a success. Focused combat tests
pass 160/160, including both new regression cases; syntax and diff checks
pass. The subsequent Warrior/Taoist live runs and logout/store comparison are
still in progress, so the last fully verified total remains 40/78.
