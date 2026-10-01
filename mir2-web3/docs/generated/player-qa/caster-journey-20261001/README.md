# Wizard / Taoist newcomer QA — 2026-10-01

This checks the user-requested caster journeys and medicine/supply UI. It does
not replace the user's accepted Warrior save or claim whole-game acceptance.

## Current completion and timing

The [final strict audit](caster-audit-r2-final.json) verifies **52/52** newcomer-v2
nodes from independent frozen full-route saves, ordinary completion receipts and
normal logout: **Wizard 26/26 and Taoist 26/26, both level 30, zero observed
deaths**. Identity, final transform and saved completion agree. The
[checkpoint before post-route shopping](caster-audit-r2-final-before-post-route.json)
retains the same route endpoints; later mutable saves do not certify the route.

| Class | Frozen save revision | Final route runId | Final snapshot / logout send / success |
| --- | --- | --- | --- |
| Wizard | 348 | `2026-10-01T13-34-42-695Z` | 1240 / 1241 / 1242 |
| Taoist | 326 | `2026-10-01T10-27-25-529Z` | 13537 / 13538 / 13539 |

This is saved functional completion, not fresh full-route timing acceptance.
The [original r2 ordinary audit](caster-audit-r2-ordinary.json) remains **48/52**:
Wizard 22/26 when its original 120-minute clock expired, Taoist 26/26 with normal
logout after 97 minutes 42 seconds. Wizard's
[separate fixed functional clock](cohort-r2-functional-clock.json) inherited
22 completed nodes and added N20, N21, N22 and graduation through ordinary play.
It completed before that separate deadline; neither original clock nor death
ledger was reset. `freshTimedCompletion=false`, `measuredTime=false` and
`visualAccepted=false` remain explicit. The earlier 33/52 cohort is preserved.

## Historical journey evidence

The first independent ordinary cohort ran source `b39cf9b38` with the Gateway
hash in [source.json](source.json). Normal NewAccount/Login/NewCharacter,
Walk/Run/Magic/Attack/NPC/item/quest packets were used; no grants, admin commands,
save edits, accelerated damage, reward rates, movement or respawn were used.
Its read-only [saved audit](caster-audit-first.json) verifies **33/52** nodes:
Wizard 21/26 at level 26, Taoist 12/26 at level 19. Both normal logout saves
match their final authoritative frames. Neither class completed the route.

The ordinary clocks and the separately labelled functional recheck clock
expired. The latter is retained in [functional-recheck-clock.json](functional-recheck-clock.json).
Neither clock nor death/revival ledger was reset. Complete raw traces and
private stores remain local and are deliberately excluded from Git.

A second independent cohort ran against a copied, hash-bound
[candidate Gateway](cohort-r2-source.json) after the corrections below, with
separate accounts, stores and ordinary clocks. Its original 48/52 and final
52/52 audits are distinct as described above. The first cohort is not
reclassified as successful.

## Ordinary purchases and supplies

[Selected public supply and class-practice receipts](cohort-r2-supply-receipts.json)
retain trace identity, original sequence numbers and raw-trace hashes. The
[read-only purchase verification](cohort-r2-purchases-verification.json) passes
seven cases using real BuyItem, LoseGold, GainedItem and carried-stock changes:

| Class / level | Ordinary purchased stock | Gold paid | Carried pool before / after |
| --- | --- | --- | --- |
| Wizard / 9 | Small HP potion x1 | 40 | 11 / 12 |
| Wizard / 15 | HP potions x22 | 880 | 2 / 24 |
| Wizard / 24 | TownTeleport x1 | 1000 | 1 / 2 |
| Taoist / 9 | Small HP potion x1 | 40 | 5 / 6 |
| Taoist / 19 | HP potions x24 | 2640 | 0 / 24 |
| Taoist / 22 | Amulet x98 | 2450 | 2 / 100 |
| Taoist / 22 | HP potions x24 | 2640 | 0 / 24 |

Quest rewards are not counted as purchases. The same curated evidence separately
covers real MP-potion consumption, Wizard random/town scroll transfers, Taoist
SoulFireBall mana/Amulet consumption, learned self-Healing, owned-pet damage and
Poisoning material/effect receipts. Each damage claim stays attached to its own
target and attacker; a completed composite practice is not one combined hit.

Separate level-30 post-route checks then used ordinary vendors with the full-route
saves already frozen. Both reports are `postRouteOnly=true` and
`ordinaryClockNotExtended=true`:

| Class / report | Purchases and actual receipt pairs | Normal logout report sequence |
| --- | --- | --- |
| [Wizard](Wizard.post-route-supplies.report.json), run `2026-10-01T13-42-59-010Z` | Samuel: large MP x1, 225 gold, 761/762; Bull: random scroll x1, 100 gold, 846/847 | 850 |
| [Taoist](Taoist.post-route-supplies.report.json), run `2026-10-01T11-01-08-039Z` | Samuel: large MP x1, 225 gold, 761/762; Bull: random scroll x1, 100 gold, 844/845; Travis: green poison x1, 40 gold, 955/956; red poison x1, 40 gold, 1050/1051 | 1094 |

Each pair is LoseGold followed by GainedItem; the reports also record before/after
gold and quantity. These bounded purchases establish ordinary availability and
settlement, not the amount needed for an extended expedition or native GUI layout.

## Corrections supported by the first run

- The native skills panel's F-key ground/directional/self-summon cast lost its
  map coordinate. Native input now retains cursor aim inside the map viewport
  through ordinary panels, including 1×/1.5×/2× stage transforms. Prompts/NPC
  capture, cast admission and out-of-stage guards remain enforced.
- Periodic shared snapshots exposed buried Zombie2/private plants before a
  real Crystal reveal, permitting a visible/selectable target the server
  correctly rejected. Gateway projection now consults authoritative Zone
  visibility before copying actors into a player's AOI. Corpses, stone Zuma,
  owned summons, removal ledgers and unknown-actor compatibility are retained.
- Old-map Struck/Poisoned/health notifications could reach a player after a
  TownTeleport bootstrap. The map boundary discards queued old viewport
  presentations while preserving personal settlement receipts. Online transfer
  carries finite control-poison deadlines for the same account/character/life
  and calibrates destination poison, including zero, without extending time.
- Healing's advertised cost was 6 while actual level-zero shared casting
  consumed 3 MP. Canonical BaseCost + LevelCost × Level now drives both UI
  affordability and personal preflight/debit. Non-Crystal generic fallback and
  unknown/negative-cost rejection remain explicit.

Controller-only corrections also retain all gameplay limits: refresh spell
readiness after authoritative movement; avoid buried targets until an actual
reveal; recognize all three authored training actors; retreat from unrelated
search aggressors; cap optional top-ups by real free weight after mandatory
minimum purchases; and use bounded, learned Taoist self-Healing after a safe
retreat. Two group neighbors are admissible for the existing three-actor
Temple training group, while adjacent-hostile, HP, 20-action, 48-engagement,
navigation, deadline and revival gates remain unchanged.

## Native UI and regression evidence

The [visual manifest](visual/caster-shop-layouts.json) checks seven caster supply
services at levels 9/28, Buy and Sell modes, zh-TW/en/pt-BR: **42 real offscreen
GPU captures**, with no layout failures. Service-open, imported item selection,
quantity, single purchase intent, tab switch, close/reopen and saved bag-layout
restoration run through production UI/ECS bindings. These are synthetic service
fixtures, not authenticated human gameplay acceptance. They did not cover the
later null-dialog snapshot that exposed the shop lifetime bug. The
[dynamic investigation](shop-lifecycle-20261001.md) preserves that failure,
the initial repair and final bounded FIFO Exit checks. The client-only r9
backport is clean source `715078944ac7a84a61b6d64901b80568a8e8d63b` based on
the deployed r8 `3f5e61533`, with no new server/runtime changes. Its own shared
suite passes [1197/10 ignored](checks/r9-shared-backport-full.log), native host
[803/5 ignored](checks/r9-native-backport-full-final.log), and
[six delayed-hide GPU captures](checks/r9-shop-lifecycle-gpu.log).
The first native run's 169 failures, a premature unchanged-environment rerun's
157 failures and one later pet-sound failure remain in separate logs. The clean
checkout initially lacked ignored map derivatives and local sound input; linking
the existing derivative/dependency inputs and adding
[15 absent WAV files](r9-missing-local-sound-inputs.json), without overwriting
tracked content, resolves those input failures. The release and installed human
GUI acceptance remain pending.

| Validation | Result and scope |
| --- | --- |
| Earlier shared client, complete serial run | 1187 passed / 9 ignored |
| Earlier native Windows client, complete serial run | 806 passed / 5 ignored |
| Native panel aim reproduction | 9 passed / 1 failed before; 10/10 after |
| Native buried-monster replay | 3/3; production adapter/input/render selection, CPU geometry |
| Caster shop behavior | 5/5, including seven imported service cases |
| Earlier controller and strict saved-evidence regressions | 386/386 after group/readiness changes |
| Latest affected controller/evidence suites | [355 passed](checks/lightning-controller-after.log); separate suite scope, not added to historical totals |
| Initial delayed-hide shop regression | 0/1 before; 8/8 after, with six three-class zh-TW GPU captures after 560 ms |
| Shared client at delayed-hide checkpoint | 1191 passed / 10 ignored; predates final FIFO Exit review |
| Final shared client including ordered service Exit | [1197 passed / 10 ignored](checks/shared-shop-lifecycle-fifo-final.log) |
| Final native Windows client | [808 passed / 5 ignored](checks/native-shop-lifecycle-fifo-final.log) |
| Final focused native FIFO checks | [3/3](checks/shop-lifecycle-fifo-after.log): two lifecycle tests plus one existing input guard; four batch cases cover all-success/all-failure, not mixed partial failure |
| Final GPU03 delayed-hide replay | [1/1](checks/shop-lifecycle-gpu-after-03.log), six three-class level-9 Buy/Sell captures after 560 ms; actual frames inspected, no human GUI operation |
| Private visibility | 0/4 before; 4/4 after |
| Gateway map-boundary status | 0/3 before; 3/3 after |
| Shared finite control handoff | 0/2 before; 5/5 after; counted-boundary 1/1 |
| Canonical mana | Metadata 2/3 and projection 1/5 before; 3/3 + 5/5 after |
| Final Gateway adjacent validation | 33/33, plus 7/7 isolated V2 bridge checks |
| Earlier complete Gateway run | 857 passed / 18 ignored; predates the late status/mana slice |
| Complete Simulation run | 1906 passed / 5 failed / 22 ignored; five old fixtures corrected below |
| Simulation fixture correction | Shinsu 2/2, line attack 1/1, town/bind 3/3 after |

The complete Simulation failures are preserved. Three old seeded Shinsu tests
provided only Scout's 35 MP for a level-three 40-MP summon; their fixture now
prepares canonical mana and verifies cast/MP debit. Two old TownTeleport tests
expected the spawn tile despite the imported safe-area binding center; they
now check that center, as TownRevive already did. Production mana/binding rules
were not rolled back. The clean full run is not relabelled as passing; these
focused results resolve its five failures.

## Second-cohort controller wire correction

The r2 ordinary traces exposed a QA-only gap: Wizard N8 and Taoist N11 sent
GreatFireBall/SoulFireBall 42–45 ms after the last movement, without a fresh
shared readiness frame. No Magic ACK or mana debit followed; both targets
remained stationary and unharmed. [Selected real receipts](practice-owner-wire-receipts.json)
retain the unextended pauses.

Crystal owner-only `UserLocation` serializes position and direction, without
an `objectId` (`Shared/ServerPackets.cs:870`). The earlier test invented that
field and therefore missed the live branch. The corrected real-wire test
fails before the controller fix; the 386-case suite passes afterward. Owner
receipts without an id now invalidate stale readiness; an explicitly foreign
id remains excluded. No server cooldown or projectile rule changed. Both
same accounts resumed through normal login within their original r2 clock.

## Limits

- The native buried replay binds original packet/snapshot sequence numbers.
  Its filtered frame and remove/map controls are explicitly synthetic; real
  reveal belongs to a different actor. It is not a GPU/human acceptance test.
- Only finite online control-poison handoff is corrected here. Counted or
  periodic poison leases remain a separate parity gap. Fresh Zone-login clock
  isolation is not a claim that Crystal logout clears saved poison.
- Canonical base/level mana is covered; original dynamic mana penalties and
  special teleport surcharges are not claimed complete.
- Native full-route visual play, laptop DPI and production soak remain open.
  Public playtest, installed r8, updater feed and human stores were not changed
  during this local caster round. Capacity work remains paused.
- Final FIFO Exit and combined client checks pass within the scopes above;
  a batch with mixed partial send failure was not measured. The separate
  client-only r9 build/release and installed mouse/GUI checks remain pending.
  Neither checkpoint nor final offscreen tests certify an installed user package.
- The original Wizard fresh timed route did not pass. Separate functional
  completion and later supply purchases do not replace the original 48/52 result.

## Ordinary combat readiness follow-up

The same r2 run found the practice correction did not cover ordinary combat.
N14 had 19 unacknowledged casts 0–52 ms after owner movement receipts.
[Selected original packets](ordinary-combat-owner-wire-receipts.json) show a
stationary public target, valid range and an expired independent spell delay;
the new movement ActionTime still correctly prevented the cast. The same
target later received accepted Magic and positive damage.

The controller now probes a fresh public frame when owner movement is newer
than its snapshot, respects its shared readiness and rereads the live target.
Owner-only receipts without an id are recognized; foreign ids and already
fresh frames do not create a redundant probe. The two real-wire class
regressions fail before the fix (50/52 custom loadout cases); final loadout
cases pass 54/54, including target movement/removal controls. The six affected
controller/evidence suites pass 292/292 TAP cases (the custom loadout script
is one TAP subprocess, so these totals must not be added).

This is a QA-controller correction. The copied candidate server, production
client rules, 20-action/48-engagement limits, ordinary clocks, character
state and first-cohort results are unchanged. Final r2 completion was pending
at this historical checkpoint; the frozen final audit above supersedes that
status without changing its clock or earlier failure evidence.

## Strict retreat controller follow-up

Wizard N20's captured pack left no collision-safe retreat. Strict kiting
correctly refused offense, but its generic error bypassed the combat loop's
existing held-scroll recovery. That branch now reports the existing typed
safety failure, preserving strict no-offense behavior, target identity and
all action, engagement and retreat limits. A synthetic blocked-map regression
fails before the change; four focused controls pass after. A separate combat
integration control reaches the held-scroll branch without any attack/cast.
The six affected controller/evidence suites pass 303/303 TAP cases, including
54 custom loadout cases represented by one TAP subprocess.

This is a QA-controller correction, not a production collision/monster change.
The original r2 ordinary clock expired with Wizard22/Taoist26 (48/52), zero
confirmed deaths and normal saved logouts. Remaining Wizard work uses a
separately labelled functional recheck; the original clock and result stay
unchanged. Full saved functional completion was pending at this checkpoint;
it is now verified separately by the final frozen audit above.

## Moving-target FireWall practice

The separate r2 Wizard functional run completed N20 normally, then paused N21
on an accepted FireWall miss. Fresh snapshot4165 showed readiness zero;
send4166/owner Magic4167 accepted target-zero ground aim (301,382), and the
next personal frame showed the real 30-MP debit. Selected actor1202102 walked
diagonally to (302,381) at4174, outside the five-cell cross later announced
at4175–4179. No positive target damage followed. This conforms to Crystal's
delayed ground effect; it is not a cooldown failure.

Practice now permits at most three accepted ground attempts in the same
original twelve-second receipt window. Before a retry it refreshes public
readiness, uses only owned supplies, and reaims the same living in-range
actor on the same map. No extra navigation/search budget is introduced.
Missing/wrong cast ACKs, target loss, three misses or elapsed time still pause;
positive target damage and final server-confirmed practice flags remain
required. Non-FireWall practice and production spell rules are unchanged.
New moving-target/budget controls fail 2/3 before; all seven FireWall controls
pass after. The seven affected controller/evidence suites pass317/317 TAP
cases, with the 54-case custom loadout script counted as one TAP subprocess.

The live follow-up produced real FireWall damage and the N21 practice 1/1.
It then reached the unchanged twelve-cell strict retreat limit during ordinary
combat. This limit now reports the same typed safety failure, enabling existing
deferral/recovery while still refusing an extra kite step or fallback cast.
The focused exhaustion check fails before; the final seven suites pass319/319.
An independent review also added fresh post-cooldown aim validation and
non-timeout error propagation. Both new boundaries fail before; all nine
FireWall controls pass after. Neither a moved-out-of-range actor nor a
connection/predicate failure authorizes another cast.

## Final Wizard combat evidence and remaining live gap

[Selected final ordinary combat and practice receipts](cohort-r2-wizard-final-combat.json)
bind completion, positive damage, kills, frozen save and normal logout to their
original run and sequence numbers. Final run `2026-10-01T13-34-42-695Z` used
thirteen GreatFireBall sends and completed the four remaining nodes.

The new ordinary Lightning chooser branch is **not live observed**. The recorded
Lightning send 612 / accepted Magic 613 in run `2026-10-01T12-14-14-284Z` belongs
to N21 class practice, with separate same-target damage receipts. That practice
evidence cannot accept the later ordinary branch. Controller regressions pass;
native GUI play and a fresh ordinary Lightning branch observation remain open.
