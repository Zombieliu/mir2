# Fresh V2 blocking failures and bounded repairs

2026-09-19 14:23 UTC, independent cohort B Wizard died for the fourth time
at N19 near D022 (248,284), after completing N12 and N16. Its cumulative
three normal revivals were already spent. The packet trace showed six nearby
hostiles, including the initial V2 WoomaSoldier group of three at (250,282).
It received normal `LogOutSuccess`, retained the death and deadline evidence,
and cannot reach strict 78/78 under this cohort's rules. Warrior and Taoist
were still on their original runs when this was diagnosed.

The next V2-only spawn candidate reduces each training group to one monster
with zero spread and separates Dung (335,360), WoomaSoldier (320,345), and
WoomaFighter (300,335). All three cells have a static path from the ordinary
D022 entry, at 5/37/48 steps respectively, and the route/collision tests pass
57/57. Imported Crystal groups remain unchanged; ordinary respawn waits and
the unchanged 30-second search/20-attack/revival limits still apply. This is
a code/test candidate, not a live survival or completion receipt.

The 2026-09-19 14:39 UTC read-only strict verifier records Warrior 23/26
at level 28, Wizard 21/26 at level 26, and Taoist 22/26 at level 27,
total 66/78. All three public logouts succeeded and their authoritative
snapshot transforms matched the persisted characters. Raw verifier output:
`C:/mir2-newcomer-v2-cohort-b-20260919/strict-verification-20260919T1439.json`.
Warrior paused at N21 because the current broad WoomaFighter source was not
found in its bounded search. Taoist paused in N20 against WoomaSoldier
294000: normal SoulFireBall casts consumed its last Amulets, then the
controller fell back to low-damage melee and exhausted the existing
20-action cap with the target still alive. The V2 N20/N21 candidate now
visits the ordinary village shop for 100 real Amulets before D022; it
fails closed if the full stack cannot be purchased. Existing shop tests
exercise 100-Amulet purchasing, and V2 route tests pass 57/57. No live
success, two-hour pass, UI, animation or Crystal parity is inferred.

At 2026-09-19 14:50 UTC, the separately labelled functional recheck kept
the original cohort B clock and revival ledger. Warrior N21 armed HalfMoon
and sent its normal directional attack after walking to (319,366), but the
Gateway emitted `ObjectAttack` with spell 0 and no owned damage. The shared
Zone had a live WoomaFighter at (318,366); Gateway's directional-target
resolver consulted the stale personal monster mirror rather than its current
shared map, then fell back to personal combat. The candidate gateway repair
resolves the target from shared-map state and corrects unmatched Zone swings
without a phantom private attack. A focused gateway regression and live
N21 replay are pending; this does not claim HalfMoon animation parity.

The Taoist functional recheck reached Merchant Ruben through ordinary map
transfers. Its public snapshot at 15:02:35 UTC showed 100 real Amulets and
gold 40,190, down from 42,690 before the normal purchase. The character was
still travelling back toward D022; N20 survival and kills were not yet
proven. This purchase belongs to the separate recheck window, not the
expired strict 120-minute cohort B run.

The first fresh ordinary saved checkpoint was 34/78; the latest independent
cohort B strict saved checkpoint is 66/78. None of the repairs below
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

The next public runs verified Warrior 21/26 at level 26 after normal logout,
with snapshot/save quest rows and transform agreeing. N19 progressed Dung to
2/3, then a real 30-second search visited 5 of 677 valid wide-area points
without a third live target. Taoist completed N14 and reached 16/26 at level
22. N15 paused before practice because the poison equip helper looked only in
the bag. Its actual public logout snapshot holds GreenPoison x50 in belt slot
4 and RedPoison x50 in belt slot 5. The repair moves a held belt stack to a
verified empty bag slot with `moveItem`, then equips it with the ordinary
`equipItem` command. No poison was minted or inserted into storage.

The Wizard N12 death trace also shows a critical 8/72 HP cave resume with
zero HP potions and two TownTeleport scrolls. V2 previously started practice
before its supply/survival path. A new V2 pre-objective readiness hook blocks
combat until an undersupplied character uses an ordinary TownTeleport,
receives its town arrival, restocks at the public shop, and has at least four
real HP potions and safe departure HP. If that path cannot be proven, it
pauses instead of casting. This protects a future fresh run; it does not
change the Wizard's exhausted 3/3 revival ledger or restart this character.
Focused V2 tests pass 48/48 and supplies tests 59/59. Taoist N15 recheck and
its subsequent normal logout/store verification remain open.

Taoist N15's first Poisoning cast had no server receipt despite a verified
GreenPoison equip. The chosen Zombie2 was AI 24: the personal snapshot listed
its HP, but the shared Zone kept the buried monster hidden until the player
approached within three tiles. V2 practice now approaches such a target and
waits for a public `ObjectShow` or visible `ObjectMonster` packet before
casting. The next ordinary run received `ObjectPoisoned`, completed N15, and
normally logged out. The read-only verifier confirmed 50/78 completed units:
Warrior 21/26 level 26, Wizard 12/26 level 19, Taoist 17/26 level 24, with
all three logout/save transforms matching. (The Taoist count includes the
earlier sixteenth completed unit and N15; quest IDs are not contiguous with
growth rewards.)

Taoist N16 then paused on a SummonSkeleton cast with no owned-pet receipt.
Its public snapshot showed GreenPoison equipped, zero Amulets in bag, belt,
or equipment, and adequate gold. The V2 pre-objective readiness gate now
uses an ordinary TownTeleport and public village shop if a required summon
or SoulFireBall practice lacks real Amulets, then checks the authoritative
stock before returning to the cave. Summon practice also pauses before
casting if no real Amulet can be equipped. Focused V2 tests pass 51/51,
including buried-target reveal and no-material summon cases. N16 live
recheck and the final 78/78, level-30, timing, UI, animation, and Crystal
comparison gates remain open.

The ordinary shop recheck did buy and equip six Amulets, but N16 still
paused. Its packet trace shows `Magic` and `ObjectMagic` for SummonSkeleton
with the monster object ID as `targetId`; no owned BoneFamiliar appeared.
Gateway's native summon route requires `targetId=0` while preserving the
cursor target location. V2 SummonSkeleton now sends that public summon
shape; a focused packet-shape regression passes (V2 suite 52/52), and the
same-clock live recheck is underway. The Warrior's later ordinary run
completed N19 and reached N20 at level 27, then failed its genuine bounded
30-second WoomaSoldier search (5/677 D022 waypoints). After both normal
logouts, the strict read-only verifier records 51/78: Warrior 22/26,
Wizard 12/26, Taoist 17/26; each saved transform matches the public logout
snapshot. Neither the Wizard revival limit nor the shared deadline changed.

The next N16 public attempt followed the corrected target-zero route and
spawned BoneFamiliar. Its `ObjectMonster` packet arrived before the personal
snapshot identified the owner, so V2 now probes one ordinary `clientVersion`
snapshot after the spawn and confirms `ownerName` matches the player. The
pet then struck a nearer Zombie2, not the selected Zombie3; `ObjectStruck`
from the owned pet and a matching positive `DamageIndicator` prove real
damage, while the old target-specific waiter falsely paused. N16 practice
now accepts positive owned-pet damage to any hostile, without extending the
12-second receipt window or adding attacks. The focused V2 suite passes
53/53, and the lawful same-character recheck is running.

The fixed functional-recheck deadline arrived at 2026-09-19 12:36:27 UTC.
Taoist N16's class practice and all three Zombie3 kills were authoritatively
complete, but the quest was only `readyToTurnIn` while the player was still
walking to its finish NPC. The runner stopped before turn-in, received
`LogOutSuccess` at the deadline, and saved without changing the original
clock or the cumulative revival ledger. The final strict verifier remains
51/78 completed units: Warrior 22/26 level 27, Wizard 12/26 level 19,
Taoist 17/26 level 24. All three normal logout snapshots match their saved
transforms and quest rows. `visualAccepted=false` and `measuredTime=false`.
Warrior N20's WoomaSoldier spawn search and Wizard N12's exhausted revival
allowance remain separate blockers. This window cannot be reported as a
78/78, three-level-30, or Crystal visual completion.

2026-09-19 independent cohort B uses new ordinary accounts and the isolated
`C:/mir2-newcomer-v2-cohort-b-20260919` store on ports 19900/19910. Each
class retains its original 2026-09-19 12:45:19 UTC start and 14:45:19 UTC
deadline; no account state, revival allowance, or 30-second search and
20-attack limits were changed. Warrior N16 exposed an underpowered level-20
MartialSabre against the required Zombie3. Bichon Blacksmith's live goods
do not sell the stronger PowerAxe, so the rejected purchase approach was
removed. V2 N16 now asks for Zombie2 x3 after N14 has already taught
Zombie3 x2; the class skill practice remains. The updated server projected
that objective for the already accepted quest, and the normal Warrior run
subsequently completed N16. Wizard N12 exposed a one-tile stale cursor race:
the server previously required exact target coordinates even when the same
monster object ID had moved one tile. The Zone now accepts at most one stale
tile, checks range against the live monster, and broadcasts the live target
position. A same-clock ordinary GreatFireBall cast received `ObjectMagic`
and positive `DamageIndicator`, then Wizard completed N12. These are
functional packet observations, not animation or Crystal visual acceptance.

Taoist reached N19 and normally paused after the unchanged 30-second
full-spread Dung search visited 5/677 waypoints. Imported D022 Dung,
WoomaSoldier, and WoomaFighter groups all use broad nominal (250,250)
spread-250 sources. V2-only bounded three-monster footholds were added at
collision-walkable (250,292), (250,282), and (270,270), while imported
Crystal groups and V1/default cadences remain unchanged. The Dung point
is 122 walk steps from the D022 entry under the static collision map.
The final gateway binary containing all three footholds is built, but its
cohort B live deployment and three-class N19–N22 recheck remain open as of
2026-09-19 14:02 UTC. No 78/78, three-level-30, two-hour, save, UI,
animation, or original-client visual claim follows from these repairs.

2026-09-19 latest bounded recheck: strict cohort B ended 66/78 (Warrior
23/26 level 28, Wizard 21/26 level 26, Taoist 22/26 level 27) with normal
logout and matching saved transforms. Wizard's fourth death exceeds its
original three-revival allowance, so this two-hour cohort cannot pass. A
separately labelled functional clock preserves every original start/death.
Taoist N20 bought 100 real Amulets through Merchant Ruben and hit the
unchanged 20-attack cap after 200 SoulFireBall damage against a roughly
285-HP WoomaSoldier; an owned BoneFamiliar assist is a candidate, not a
pass. Warrior N21 public HalfMoon exposed a stale personal-monster target
lookup; the shared-Zone directional fix has a passing release regression and
is deployed, while live damage is still pending. V2 quest searches now use
only their isolated D022 training sources and reject crowded N20/N21 targets.
Node route suite 57/57 and supply suite 59/59 pass. No 78/78, three level-30
saves, clean two-hour completion, or UI/animation/Crystal visual claim follows.

Later 2026-09-19 functional observations: Warrior N21's HalfMoon emitted a
spell-4 ObjectAttack and positive damage after the deployed shared-Zone fix.
The immediately following Thrusting was rejected during Zone's 600 ms melee
action lock; a 650 ms practice spacing regression passes. In the actual
objective, 20 ordinary attacks dealt 144 damage to a 285-HP imported
WoomaFighter. Taoist used an owned BoneFamiliar but died once and, after a
normal revive, paused on a D021→D022 monster-blocked transfer. Its pet's
one-damage output contradicted BoneFamiliar's imported 12–23 DC. Focused
Rust summon correction passes 1/1; new V2-only 120-HP training Wooma and
nine isolated actors retain full-strength imported groups and normal respawn
timing. V2 and combat controller suites pass 59/59 and 161/161. Rust spawn
integration passes 1/1; new Gateway deployment, live three-class 78/78 and visual
acceptance remain open.

2026-09-19 functional recheck after the nine-actor Gateway deployment:
Warrior finished N21 on a real training Wooma, then used a held TownTeleport
to return from D022 for N22. The normal scroll lands more than 350 walking
tiles from the Board, so report navigation now permits that actual city
walk. The independent `verify-newcomer-v2-evidence.mjs` check reports Warrior
**26/26, level 30**, `LogOutSuccess`, and matching saved level, transform,
gold, experience, and completed quest rows. Its original 12:45–14:45 UTC
strict clock had already expired; this is a separately labelled functional
recheck and cannot be called a clean two-hour pass. Taoist N20 resumed using
ordinary shop supplies; the D021→D022 path exposed a live CaveBat blocking
the cave corridor. V2 map travel now uses bounded public combat to clear a
proved blocker, with the existing 20-action target cap and deadline intact.
The V2 controller suite passes 59/59 and travel suite 55/55; Taoist remains
under live recheck. The earlier Wizard cohort exceeded its unchanged three
revivals, so a separately registered fresh ordinary Wizard cohort C is
running against the same shared Gateway. UI, animation, Crystal visual
comparison, clean two-hour timing, and three-class 78/78 remain unaccepted.

Later in the same functional recheck, Taoist completed N20's three
WoomaSoldiers and N21's three WoomaFighters plus the class-practice receipt,
then completed N22 and all four growth claims. The independent verifier now
reports both Warrior and Taoist at **26/26, level 30**, with normal
`LogOutSuccess` and matching persisted save state. Cohort B's old Wizard
remains 21/26 and exhausted its original revival allowance; this does not
become 78/78 by combining incompatible clocks. A fresh normal Wizard cohort C
reached N10 but used all three real revivals in N4, N7, and N10. Its timer
and death ledger stayed intact through two explicitly recorded runner
interruptions to load ordinary TownTeleport survival handling. That handling
produced a public `UseItem` and authoritative D001→Bichon transfer at low HP.
The next ordinary return-scroll purchase paused at Merchant Scott despite the
NPC appearing in the final public snapshot. A focused merchant observation
refresh test passes; live Scott interaction and the remaining Wizard route
are not yet accepted. These repair waits are not clean player time.

The Scott failure was a content-profile mismatch: `platinum_176` showed
object 42 but omitted its original `BichonProvince/BorderVillage/Pedlar`
script. Profile v26 restores the script; the same-profile shared Gateway
round-trip interaction regression passes 1/1 and the rebuilt live Gateway
serves public `Interact`, `@BuySell`, `NPCGoods` and a normal 1,000-gold
TownTeleport `BuyItem`. The persisted Wizard resumed N10 with the original
16:04–18:04 UTC deadline and all three actual revivals still recorded.
Completion, UI/animation and original-client visual comparison remain open.

One connection immediately after the shop probe was rejected by the existing
30-second route lease. Before V2 gameplay began, the runner's `finally`
handler overwrote the canonical report without its V2 ledger; a later resume
then incorrectly opened a fresh 17:23–19:23 clock with zero deaths. That run
was stopped as soon as the mismatch was detected. The incorrect report and
its public trace remain archived, and the canonical report was restored byte
for byte from the preceding valid 16:40 report. The ordinary 16:04–18:04
clock and all three real death/revive receipts now survive a tested bootstrap
failure; the resumed live report confirms them. The server character save was
not edited or reset. The intervening purchase and walking were real actions
and their repair wait is not clean player time.

The restored strict Wizard clock expired exactly at 18:04:26 UTC: the public
snapshot showed level 21, 13/22 main nodes plus 2/4 growth claims, N14 at
3/5, and all three permitted revivals already consumed. This is a failed
two-hour result, not a timing pass. A separate functional recheck started at
18:05:32 UTC using the same ordinary character/account store, with the
original start/deadline and three revivals retained in the report. Its own
deadline is 20:05:32 UTC; no character level, item, gold or save was granted
or edited for this recheck.

2026-09-19 18:27 UTC: cohort C's separate Wizard functional recheck paused at
N15, level 22, with the same three recorded revivals. Its saved public
snapshot was at D401 (38,180), 24/85 HP, no HP medicine or TownTeleport,
beside four Zombie3 and a wounded Zombie2. This character is **not** a
three-class completion. The objective selector favored that wounded Zombie2
before considering its hostile pack, and the V2 Wizard policy cleared
non-objective aggressors; after returning to town inside one quest loop, it
could reenter the mine without repeating the departure supply check. The
candidate repair excludes adjacent/packed targets for Wizard N15/N16, keeps
the required Zombie2 in focus, and runs ordinary supply readiness before
each new mine transfer. It does not increase the 20-attack, 30-second search,
or three-revival limits. The combat/V2 Node suites pass 222/222, including
an isolated-versus-wounded target test and a blocked-unsupplied-transfer test.
An independently registered ordinary Wizard cohort D began at 18:31:56 UTC
against the existing shared Gateway to live-check the repair. Cohort C remains
preserved as failed evidence. D is a new run, not a reset of C's clock or
deaths; its outcome and normal saved logout are still pending. No UI,
animation, Crystal original-client visual, or clean two-hour pass is claimed.

At 19:24:23 UTC cohort D suffered one real N14 death at D401 (39,126) before
any N14 kill. The last public snapshots showed four Zombie3 converging at
melee range while the Wizard still held 32 small HP medicines and four normal
TownTeleport scrolls. A scoped candidate applies the same isolated-target
selection and pre-transfer real-supply check to N14 and requests the already
held public emergency scroll at 65% rather than 35% HP. It does not grant or
increase scrolls, damage, attack/search steps, revivals, or deadlines. The
affected Node suites remain 222/222. The running process had not loaded this
candidate, so it was stopped after a subsequent authoritative town snapshot
at 19:29 UTC for a normal same-clock reload; this repair interruption is
not clean player time. N14 live revalidation, the original two-hour gate,
and the three-class completion remain open.

The N14 reload retained 1/5 progress and its original clock/death. It safely
used real TownTeleport twice when the mine entrance reached 40–46/81 HP, but
did not gain another kill. A 19:39:45 UTC public snapshot at D401 (65,158)
showed nine live monsters within 12 tiles, including adjacent Zombie2 and
Zombie3; the chosen Zombie2 had taken 35 real damage before the safe retreat.
This is a survival-without-progress loop, not N14 success. The next candidate
uses the already bounded collision-planned Wizard retreat action, one-tile
hostile clearance during objective search/approach, and the same public
spell/escape commands. Its existing per-target retreat caps are unchanged;
combat, V2 and kiting suites pass 259/259. The earlier process was stopped
on map 0 near the mine entrance to load the candidate under the same clock.
Live kill progression and save parity remain pending.

At 19:47:20 UTC the kiting candidate stopped alive at D401 (47,129), 58/81
HP, N14 1/5, after an occupied corridor prevented a bounded retreat. The
character still held two TownTeleport scrolls, but the combat controller only
used them below its 65% emergency HP threshold. The next scoped repair permits
one already-held public escape if the bounded retreat actually fails, even
above that threshold. It also allows a target with two nonadjacent nearby
monsters and keeps the Wizard on the chosen target after its 12-cell kiting
budget, rather than abandoning an injured Zombie after one or two casts. The
combat/V2/kiting Node suites pass 260/260. Cohort D resumed from the same
18:31:56–20:31:56 UTC clock and one recorded death. At 20:03:31 UTC its live
server snapshot had N14 2/5 and 81/81 HP at D401 (40,119); this is progress,
not an N14 or three-class completion. The repair and run remain unverified for
full route, clean two-hour timing, UI/animation, or original Crystal visuals.

At 20:07:10 UTC the same D Wizard returned alive to town after N14 2/5 and
paused at 26/81 HP despite seven real HP medicines and 9,580 gold. The
departure guard used a medicine but immediately compared HP before the
server's timed potion recovery could apply. It now waits at most 30 seconds
for an authoritative 35% HP receipt while in the village; a timeout still
pauses safely. Survival, supplies, and V2 Node suites pass 203/203. The
same-clock live resume reached 79/81 HP by 20:10:08 UTC and was traveling
back through Bichon, confirming the former false pause was removed. N14 and
the overall three-class gate are still open.

At 20:17:58 UTC D remained N14 2/5 and was back in Bichon alive after a
second mine excursion. A direct controller audit found that V2 kill objectives
supplied `combatAction` but omitted its `combatApproachRange`. The generic
objective default is one tile, so a Wizard with ready FireBall walked toward
melee range before casting and repeatedly pulled overlapping Zombies. V2 now
passes the class-aware authoritative approach distance (nine tiles for an
affordable Wizard spell, six for a usable Taoist ranged spell, one for melee).
The Wizard cooldown-refresh integration test now rejects a one-tile approach;
combat, V2, and kiting suites pass 260/260. The old runner was stopped only
after a town snapshot to load this repair. Its 18:31:56–20:31:56 UTC clock,
one actual revival and 2/5 N14 progress remain unchanged. Live-range and
three-class route validation are still pending; the interruption is repair
time, not clean player time.

The original D 120-minute clock expired at 20:31:56.928 UTC, and the runner
paused at 20:31:57.412 UTC with N14/N15 complete, N16 1/4, Wizard level 24,
two actual death/revive receipts, and an authoritative D401 (38,145) snapshot
at 92 HP. This is an explicit **failed two-hour Wizard result**; repair waits,
repeated town crossings and deaths remain in its elapsed clock. Warrior and
Taoist had previously passed their separate functional checks, but no honest
three-class timed pass exists. At 20:32:56.247 UTC a separately labelled
functional recheck began on the same ordinary account and save; its ledger
retains the original 18:31:56.928–20:31:56.928 UTC clock and two recorded
revivals. The recheck cannot make the failed two-hour run pass. N16–N22, the
Wizard level-30/save gate and all UI/animation/original-client visual checks
remain open.

The first separately labelled functional pass reached N16 2/4, level 24,
and paused alive at D401 (29,174), 83 HP, at 20:34:37 UTC. Its last trace
reported `navigationMovementControlBlocked` before `Navigation attempt budget
exceeded (8)`: the navigator counted 250 ms waits under an authoritative
action block as attempted moves. A scoped repair excludes such no-dispatch
waits from the existing movement-attempt cap and independently stops after
20 bounded control waits; it does not increase the eight actual movement
attempts. Navigator tests pass 52/52, including a ten-wait paralysis release
and a permanent block that sends no Walk/Run. A same-account, same-clock
recheck resumed with the original two deaths and N16 2/4. Live N16–N22 and
normal saved logout remain unverified.

The D Wizard subsequently completed N16, the level-25 growth claim, N17,
and N18, reaching level 26 with the real Lightning book learned. N19 had one
of three Dung kills confirmed before the second D022 pull. At 20:51:40 UTC
the third real death occurred at D022 (349,358): the trace shows a non-objective
WoomaWarrior (285 HP) intercepted the path to Dung, the Wizard spent repeated
FireBall casts on that warrior at one-tile range, and three adjacent monsters
landed attacks in the last seconds. The public revive returned the player to
Bichon; the runner was stopped alive on map 0 after its third recorded revival.
The three-death ceiling is exhausted. N19 remains 1/3, not complete.

The scoped N19–N21 candidate reuses the existing Wizard cave policy: retain
the required target through an aggressor interruption, reject adjacent packs,
kite on a one-tile threat, check real supplies before re-entering from town,
and permit an already-held public escape at the existing 65% cave threshold
or when bounded retreat is blocked. It adds no item, level, death allowance,
attack/search attempt, or extra time. Combat, V2, and kiting Node suites pass
260/260; live D022 and ordinary saved logout are still pending.

The same D account made a second ordinary D022 pass after buying 22 small HP
medicines with its own gold. It advanced N19 from 1/3 to 2/3, then returned
to town alive when the stock ran out. A further same-clock continuation reached
the third Dung, but at 21:15:18 UTC four Wooma/Dung actors occupied adjacent
tiles. At 82/105 HP the retreat controller found no authoritative escape
step, yet incorrectly tried breakout FireBall attacks because its above-65%
blocked-retreat escape exception was only checked *after* breakout failed.
The public RandomTeleport executed at 27 HP; already pending monster damage
still killed the player at its new D022 coordinate (254,198). This was a
fourth actual death, so the runner correctly stopped with `deathRecoveryLimit`
and did not revive again. D remains an explicit **failed** two-hour run and
failed capped functional recheck at N19 2/3, level 26. It must not be merged
into a passing three-class result.

The next candidate attempts an already-held public escape immediately when a
retreat has no legal first step and the caller opted into blocked-retreat
escape, before any breakout attack, even above the ordinary HP threshold. It
also forbids Wizard N19–N21 from converting a blocked kite into a spell trade
with high-HP Wooma. The 320 combat/V2/kiting/supplies Node tests pass. A fresh
ordinary Wizard account is required for a valid at-most-three-death recheck;
the failed D evidence and its original clock are preserved unchanged.

A fresh E Wizard started at 21:19:06.394 UTC with a new public account and an
unchanged 120-minute deadline. It reached N14 at level 20 with zero deaths,
completed all five Zombie2/Zombie3 kills, and reached N15 at level 22. At
21:44:48 UTC it paused alive at D401 (43,179), 85 HP, on the class-practice
reposition step. Its trace contains an accepted `UserLocation` from (41,179)
to (43,179). The runner compared the current mutable player entity with the
same object captured before navigation, so it incorrectly called this legal
move unconfirmed. The scoped fix copies the origin coordinates before moving
and also requires a post-dispatch `UserLocation` packet. A regression test
uses the real in-place update shape, plus a negative case with a local-only
coordinate mutation. This was a repair interruption, not a new clean clock or
a completed N15.

E resumed on the same clock, completed N15 at 21:57:02 UTC, and entered N16
at level 24 with zero deaths. Its first FireWall at (48,178) produced the
accepted `ObjectMagic` and five `ObjectSpell` cells, but no target damage over
12 seconds. The chosen Zombie2 had just respawned via `ObjectRevived` and had
no later `ObjectShow` or live `ObjectMonster` receipt. Crystal's AI 24 Zombie
is buried until a player comes within three tiles; the personal snapshot still
listed it while the shared Zone correctly rejected damage. The V2 spell
practice now approaches that reveal radius and requires the public visibility
receipt before casting, sharing the same rule previously used by Poisoning.
It does not count the failed cast as practice, alter the quest or save, or
extend E's original deadline. The targeted V2 Node suite passes 63/63,
including positive and negative buried FireWall cases. Live N16 completion
remains to be observed.

E subsequently completed N16 and its growth claim, reached N19 at level 26,
and retained one actual death from a seven-monster D401 surround. The D022
entry at (339,355) placed the authored Dung foothold at (340,355) amid at
least four imported 285-HP Wooma and another imported enemy within five tiles
in the 22:23:46 UTC snapshot. The Wizard used real escape items twice and
returned alive, but N19 stayed 0/3. This disproves the earlier "isolated"
label. The runner was stopped while alive on map 1 at 22:28 UTC for repair;
the original clock and one death remain in the evidence. Candidate repair
retains imported D022 groups, stats, spread and respawn timing in the opt-in
V2 mode, but caps each imported group to one live actor. All other cadences
and maps keep their original counts. Targeted test and resumed live result
must still confirm the new behavior.

The reduced-density E continuation reached an actual N19 training Dung in
D022. Its HP fell from 155 to 39 while the Wizard remained at 100/105 HP;
no other monster was within 12 tiles in the final snapshot. The Dung followed
each retreat until the existing 12-cell per-target kiting budget was spent,
then the generic no-trade rule paused before an otherwise legal finishing
spell. N19 alone now permits that bounded direct spell fallback; N20–N21
retain the strict no-trade rule for Wooma. No retreat, attack, death, or time
budget was raised. The combined V2/kiting Node suites pass 100/100. The E
run remains at N19 0/3 until live damage and quest receipts prove progress.

Live E then cleared N19 and N20, reaching level 28 and N21 with one death.
Its first Lightning hit was accepted, but the planned q21 reposition tried
(308,392) from (306,391), a statically blocked D022 cell; the live
WoomaFighter was at (304,389). The runner paused alive before any unconfirmed
movement could count. The scoped candidate tries the original endpoint, then
an adjacent vertical and horizontal step, sharing the original 12 successful
steps and 20 attempts across all three. It still requires post-send
`UserLocation` before counting the reposition. The V2 Node suite passes
64/64, including the blocked-endpoint and missing-receipt cases; live N21
remains pending.

## 2026-09-20 mixed-cohort functional closure

The legal-fallback candidate cleared E Wizard N21 on live server receipts,
then N22 and the level-30 growth claim. Its ordinary runner reported completion
at 2026-09-19 22:50:48.754 UTC. The unextended original E deadline was
23:19:06.394 UTC. This run retained one actual death and revival. Its
91m42s wall interval includes several development repair waits, so it is not
a clean uninterrupted player-time measurement.

The read-only evidence verifier was run against the original B Warrior and
Taoist evidence, original E Wizard evidence, and the real B Gateway account
store after normal logout. A staging directory used directory symlinks only;
the source reports, traces, snapshots and account store were not copied or
edited for verification. The sanitized verifier output is
[`ordinary-evidence-mixed-b-e.json`](ordinary-evidence-mixed-b-e.json),
SHA-256 `67B9A71B0329A896733FE3D71243F0D9C196BA118628C654ADC550E3A6C67DB7`.

| Class and source | Saved level | Persisted V2 units | Normal logout | Final snapshot vs save |
|---|---:|---:|---|---|
| Warrior, B (`C:/mir2-newcomer-v2-cohort-b-20260919/warrior`) | 30 | 26/26 | success | match, revision 285 |
| Wizard, E (`C:/mir2-newcomer-v2-cohort-e-20260920/wizard`) | 30 | 26/26 | success | match, revision 301 |
| Taoist, B (`C:/mir2-newcomer-v2-cohort-b-20260919/taoist`) | 30 | 26/26 | success | match, revision 404 |

The V2 **functional route and save gate is 78/78** across these explicitly
mixed cohorts. B Warrior and Taoist completed after their original strict
120-minute deadlines in separately labelled functional rechecks. The failed
B/C/D Wizard runs and their death/deadline ledgers remain failed evidence;
combining B and E results does not turn any of those cohorts into a clean
three-class two-hour pass. No fresh same-build three-class time trial, human
player timing, native visual/animation acceptance, Crystal 1:1 comparison,
or level-30 graduation-equipment source audit is claimed here.
