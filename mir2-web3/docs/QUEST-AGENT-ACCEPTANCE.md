# Quest Agent staged acceptance — 2026-08-16

## Decision requested

Accept the real-client Quest Agent foundation and the Warrior q1-q9 functional
slice. Do **not** accept this branch as a completed Warrior, Wizard, or Taoist
level-1-to-50 playthrough.

## Immutable anchors

- Source review base: PR #233 exact head
  `ef33e4764c1342620e30532fb5ffa4e784013dd2`.
- Main integration base: `aa928b99ae7346eb6816ab8ca0b3f1063dae9394`
  (current `main`, including the squash merge of PR #235).
- Main-based follow-up code freezes:
  `d68674ce83f21680f4cc7546561471bd084ee216` and
  `e50a8fce04e8ae6cf9b0b5091c930fabd6a3375e`, followed by
  `030cebe31806e3fb7ef79f6014d12864593c3c34` and
  `d8264b5c1d80f150e29462dec47380995cc93185` and
  `40ffedcb55a31d5cc2da8f5007464dd21616d549` and
  `32f5b2f9d8ea7eaa6468007f97737b6ee781f2b6`.
- Patch-equivalent source freeze for the original transplant:
  `ccaba013515b0f1908e9c3aa6fca6a5c847db1f8`.
- Latest live-soak Quest Agent runtime revision:
  `e8065e599face64456dece9c9ca4a017433c1730` (patch-equivalent to clean
  commit `32f5b2f9d`).
- Remote follow-up branch: `origin/codex/quest-agent-recovery-followup`.
- Integration lineage: PR #235's reviewed range is squash-merged as
  `aa928b99a`. This follow-up adds six code/test commits plus their acceptance
  documentation commits; source
  `0256c33a9`/`0afc30449`/`1c9ac3b4`/`acd95a4b4`/`e533efbb3`/`e8065e599`
  and clean
  `d68674ce8`/`e50a8fce0`/`030cebe3`/`d8264b5c1`/`40ffedcb5`/`32f5b2f9d`
  have matching stable patch ids respectively.

## Acceptance matrix

| Surface | Status | Evidence boundary |
| --- | --- | --- |
| Physical input and read-only observation contract | PASS | CDP mouse/keyboard/text only; static and runtime shortcut audits report zero violations. |
| Resume, reconnect, death, potion, merchant, navigation, combat, harvest, and equipment framework | PASS for staged use | Exercised across the local development soak; failures remain explicit and resumable. |
| Warrior q1-q9 functional route | PASS | One finalized certificate reached all required authoritative stages with 684 inputs, 18 kills, no death, and zero shortcut violations. |
| Current extended Warrior chain | PARTIAL | The resumed character has reached level 15. q22-q24, q28, and q29 are complete; q25 is 8/20, q26/q27 are not yet unlocked in the current snapshot, and q30 is 0/1. |
| Three-class route generation | STATIC PASS | Warrior, Wizard, and Taoist manifests each contain 140 level-1-to-50 quests and report zero generated blockers. This is not live completion. |
| Clean visual-assets certificate | NOT ACCEPTED | The completed q1-q9 run contains missing-raster diagnostics. A later incomplete segment is diagnostics-clean, but cannot replace a complete clean run. |
| Contiguous Warrior level 1-50 | NOT ACCEPTED | No single/resumed evidence chain has completed it. |
| Wizard and Taoist level 1-50 | NOT ACCEPTED | Generated and unit-tested, not live-client completed. |
| Human feel, physical device, long production soak | NOT ACCEPTED | Requires separate human/device/production sign-off. |

## Automated gates at the code freeze

Run from `mir2-web3/apps/web`:

```bash
npm run test:quest-agent
npm run typecheck
node --check scripts/quest-agent/policy.mjs
node --check scripts/quest-agent/run-q1-q5.mjs
node --check scripts/quest-agent/test-policy.mjs
git diff --check
```

Run from the repository root:

```bash
cargo test -p mir2-simulation
cargo test -p mir2-gateway --lib
cargo fmt --all -- --check
```

Full-package gates passed on the original patch-equivalent source freeze:

- Quest Agent: 163 passed, 0 failed;
- TypeScript, Node syntax, Rust formatting, and whitespace checks: passed;
- `mir2-simulation`: full package exited 0, including `vertical_slice` 8/8
  and `zone_replay` 8/8;
- `mir2-gateway --lib`: 451 passed, 0 failed, 1 ignored in 242.55 seconds.

The main-based clean transplant was then verified independently:

- all eight source commits match one-for-one under `git range-diff`;
- Simulation, Gateway, and GameData source directories are byte-identical to
  the full-package-tested source freeze;
- Quest Agent 163/163, TypeScript, and Node syntax pass;
- the PR #234 asset-release integration passes contract 8/8 and safety 15/15;
- Simulation `security_lifecycle` passes 18/18 and `vertical_slice` passes 8/8;
- the Gateway paid-sailor round-trip passes 1/1;
- Rust formatting and whitespace checks pass.

The later Quest Agent-only follow-ups through `32f5b2f9d` change JavaScript and
its unit tests. The complete Quest Agent gate now passes 178/178, including the
long-preparation travel, depleted-shelter recovery, confirmed-corpse lifecycle,
cross-run supply recall, safe-room settlement, full-map route fallback, and
dense-shelter escape plus congested-portal rotation and physical-hit-target
selection, en-route reserve exhaustion, and optional hazard-waypoint
regressions. It also covers budget-disabled equipment-repair travel retaining
its explicit non-funding resource accounting while independently enabling
certified physical occupancy clearing; Node syntax checks and `git diff
--check` pass in both source and clean worktrees. The latest regression also
requires a harvest goal to handle at most two already attacking, certified
nearby threats before creating the source corpse; an unsafe or excess threat
forces physical disengagement instead. The newest regression keeps the
conservative 15-second/five-attack no-response budget for quest combat, but
lets incidental travel clearing rotate an unresponsive occupied tile after
four seconds and two real attacks. The earlier full
Simulation/Gateway/TypeScript results remain the backend baseline
rather than being relabeled as a fresh run for these Agent-only changes. PR
#235 at exact head `45192e947` finished 20 successful remote checks, two
conditional skips, and zero failures or pending checks before squash-merging as
`aa928b99a`; the follow-up head must be judged by its own remote check rollup.

The first full acceptance run exposed a deterministic saved-transform
regression introduced after the PR #233 base: a valid Bichon field position was
tested against the starter collision window and incorrectly recovered into the
town safe zone. PR #233 passed the same comparison test. Clean commit
`c96a7826d`
changes recovery to use authoritative full-map bounds and adds a regression
test that preserves valid field positions while retaining recovery for the
legacy mismatched-map case. The two previously failing FireBall vertical-slice
tests and the full Simulation package pass after the fix.

The Gateway paid-sailor test was also corrected in `8423d124b` to seed its
level/gold fixture through the isolated Platinum account store and use the
internal test transfer. It no longer depends on a production-profile-rejected
`qa.applyNativeState` command; the production boat path itself was not changed.

## Live evidence summary

The private evidence directory contains 101 finalized development reports
through `warrior-q30-r102-supervised` (r66 was an intentionally stopped live
trace and is excluded from these report aggregates):

- 89,984,193 ms (24 h 59 m 44 s) browser-active runtime;
- 47,510 recorded physical inputs;
- 470 historical kill rows, including one r44 row now proven to repeat the
  same target object id rather than represent another kill;
- 18 deaths and 17 completed revives across intentionally interrupted and
  diagnostic runs;
- zero shortcut violations.

The aggregate spans multiple code revisions and is endurance evidence, not a
single passing certificate. The r38 baseline and r39 A/B continuation add
1,183,782 ms, 604 inputs, and 5 kills with no death, shortcut violation, or
critical browser/network diagnostic. In r39, the same persisted level-14
character selected the already quest-certified `SpittingSpider`, walked from
the village edge to its real far-field spawn, completed two normal-client
kills, and advanced experience from 9,017 to 9,449. Its 902,380 ms budget then
expired during the third goal; this is an expected incomplete segment, not a
q25 or q30 completion certificate.

r40 then supplied the required counterexample to a sustained-throughput claim:
one further SpittingSpider goal advanced experience from 9,449 to 9,881, but a
dense-field follow-up exhausted health supplies and exposed a deterministic
recovery abort at 9/135 HP with zero potions. After the red/green fix, r41
continued ordinary collision-routed movement for 600,823 ms instead of taking
that fatal branch and recovered to 134/135 HP. r42 physically reached the
merchant district, entered the visible shelter while pursued, and returned,
but its runtime expired before purchase. The exact r42 resume in r43 completed
the missing closure through visible Ruben interaction: `(HP)DrugSmall` 0 -> 10
and gold 548 -> 148. r43 was then stopped cleanly during the next grind. The
four reports add 1,381,150 ms, 809 inputs, one kill, one shop purchase, no
death/revive, no shortcut violation, and no critical browser/network
diagnostic. They prove the depleted recovery and restock path, not q25/q30 or
level-15 completion.

r44 continued from that stocked field state for 900,323 ms and advanced EXP
from 9,881 to 11,177 with 443 physical inputs, no death, no potion use, and no
critical diagnostic. Audit review nevertheless rejected its raw 4-kill count:
goal rows 1 and 2 both named object id `202215`, and goal 2 completed in 1,846
ms only because the first death's EXP arrived after the stale corpse was
selected again. The honest r44 result is three distinct confirmed target ids,
not four independent kills.

Commit `d35415c16` adds a confirmed-dead object lifecycle across goals and
resume reports. A target remains ineligible while its corpse persists, even if
the renderer temporarily reports `dead=false` with no authoritative HP. It may
be selected again only after a complete AOI snapshot observes its absence and
a later snapshot observes definite positive HP; a bounded ten-minute hold is
the defensive fallback. Four new policy/executable contracts bring the full
Quest Agent gate to 169/169.

r45 is the live replay of the rejected r44 condition. Its first AOI still
contained the old `202213` and `202205` corpse ids, but the Agent physically
left them and completed six goals against six distinct ids: `202206`, `202203`,
`202304`, `202210`, `202302`, and `202308`. The 308,963 ms report records EXP
11,177 -> 13,553, 161 physical inputs, 0 deaths, 0 potion uses, 0 shortcut
violations, and 0 critical browser/network failures. The final screenshot
shows the selected 0/65 corpse and several separate `Down` spiders. This closes
the duplicate-corpse accounting regression, not level 15, q25, or q30.

r46-r49 then exercised the stocked far-field-to-supply loop. r46 consumed ten
potions during a long retreat and exposed that an idle GroceryStore client
could keep showing stale HP even though the next authoritative bootstrap was
fully healed. r47-r48 proved resumable town navigation and visible merchant
restock, while r49 returned to the far field, completed two distinct
SpittingSpider kills, and ended with a severe unresolved resource strain.

r50-r52 exposed and closed two cross-slice recovery gaps. An unresolved severe
strain now restores a one-shot supply recall only while stock is below ten, and
safe-room arrival holds a 20-second settlement window with bounded ordinary
walking instead of immediately re-entering the same attacker's chase window.
r50 visibly revived once, sold harvested supply, restocked to ten potions, and
completed three Oma goals. r51 completed four of five requested goals but
demonstrated the old immediate-exit shelter loop. The patched r52 entered the
GroceryStore twice, physically paced during both settlement windows, escaped
the pursuing Scarecrows, sold Venison for gold `76 -> 302`, and bought HP drugs
`5 -> 10` for gold `302 -> 102`. Its remaining budget was spent on ordinary
far-field travel; it recorded no death, shortcut violation, or critical
browser/network diagnostic.

r53 resumed that exact saved position, completed five of five SpittingSpider
goals, and advanced EXP from 15,329 to 17,273. A dense mixed field caused one
normal death and one visible Town Revive; the same recovery path restocked to
ten potions, settled in the safe room, and physically returned to a different
SpittingSpider field. The 1,770,718 ms segment recorded five kills, 934 inputs,
12 potion uses, one shop purchase, zero shortcut violations, and zero critical
browser/network diagnostics. It ended at level 14 with 115/135 HP and ten
potions. This proves resumable recovery and continued progression, not level
15 or q25/q30 completion.

r54-r57 then exercised the same recovery chain from two deterministic failure
states. r54 reached `(503,633)` but the old collision atlas searched only local
margins through 240 tiles and falsely declared both GroceryStore entrances
unreachable even though the full 700-by-700 map had a connected route around a
wall. The first fix adds cheap local searches followed by a true full-map
fallback on small Crystal maps and a bounded adaptive fallback on large maps.
r55 resumed the exact state and moved for its full 20-minute slice instead of
taking the former immediate fatal branch.

r56 then reached a dense beginner field and exposed a separate no-input retry:
the emergency shelter escape inherited `supplyFunding=true`, so mixed adjacent
attackers caused the funding safety guard to reject every bounded clearing
attack. The second fix classifies only that already-committed escape as normal
travel while retaining level certification, the four-attempt cap, target
quarantine, and ordinary combat input. r57 resumed the exact crowded state,
escaped the field, entered the GroceryStore, recovered passively, returned to a
visible Deer corpse, harvested Venison `1 -> 2`, sold it for gold `29 -> 279`,
and bought HP drugs `8 -> 10` for gold `279 -> 199`. It then continued ordinary
travel until the 1,200,000 ms slice limit. The report records 1,203,435 ms, 635
inputs, five kills, no death, no shortcut violation, and no critical
browser/network diagnostic; EXP advanced from 17,483 to 18,265 and the final
state remained level 14. This closes the two exact recovery regressions, not
level 15 or q25/q30 completion.

r58-r60 returned to ordinary progression and advanced EXP from 18,265 to
21,721 without a death. r58 reached a real SpittingSpider field, r59 survived
a one-HP/zero-potion retreat before a visible `0 -> 10` restock and completed
five Oma goals, and r60 recorded seven historical kill rows while clearing a
dense village-edge pack. The r60 aggregate includes incidental and delayed
combat settlement, so those rows remain audit rows rather than a unique-kill
claim.

r61 resumed the dense state and eventually moved from the pack to the merchant
approach, but spent most of its 624,288 ms budget switching among adjacent
actors whose rendered hit surfaces were not physically clickable. It gained
one historical kill row and EXP `21,721 -> 22,385`, then expired with zero
potions before opening the merchant. Clean commit `e50a8fce0` (source
`0afc30449`) restricts bounded occupancy clearing to physical hit targets,
prefers the already selected clickable actor, and rotates an ordinary recovery
portal only after 45 seconds without net distance improvement. The same portal
is cooled for 120 seconds; no direct movement, target, or map command is added.

r62 resumed the authoritative r61 character, escaped the same area through
ordinary movement, sold visible Venison for gold `178 -> 428`, and bought HP
drugs `0 -> 10` for gold `428 -> 28`. Its remaining ten-minute budget moved
from the merchant district toward the real SpittingSpider fields, ending at
131/135 HP with all ten potions. r63 then selected a physically clickable
SpittingSpider, completed one normal-client goal, and advanced EXP
`22,385 -> 22,601` in 187,583 ms with no potion use. Across r58-r63, the six
reports add 5,173,396 ms, 2,724 inputs, 17 historical kill rows, two shop
purchases, zero deaths/revives, zero shortcut violations, and zero critical
browser/network diagnostics. r62 did not need to emit the new portal-rotation
or occupancy-clear branch, so those exact branches remain unit-covered rather
than separately live-certified at that point. r65 later closes both branches.

r64 completed two of two SpittingSpider goals and advanced EXP
`22,601 -> 23,465`, but spent all ten potions during the physical retreat. The
journey had enabled resource enforcement while two potions remained; when the
same trip reached zero, the travel budget correctly fired but the recovery
layer failed to re-read the new depleted state and propagated a fatal at
80/135 HP. Source commit `1c9ac3b4` (clean `030cebe3`) treats that budget
crossing as a resumable outer-policy transition. The next authoritative frame
therefore chooses the existing depleted-escape rule; movement, death, revive,
health, stock, and map transfer remain client/server authoritative.

r65 resumed the exact 80-HP/zero-potion state and moved through the old fatal
point. It naturally exercised physically clickable occupancy clearing against
Oma, HookingCat, RakingCat, and Scarecrow targets, then emitted the bounded
recovery-transfer rotation after 117,841 ms without net progress. That closes
both branches that r62 had left unit-only. r65 also exposed a second error:
`navigateNear` may return either a typed collision error or the bounded text
form `navigation did not reach`; the optional hostile-corridor waypoint caught
only the typed form and therefore misclassified the optional point as a failed
portal. The same commit applies the existing retryable-navigation classifier
inside that optional waypoint and retains the direct physical portal route.

r66 was a live trace rather than a finalized report because it was stopped
after the acceptance closure. It entered GroceryStore through an ordinary
direction-key portal step, paced through the pursuit settlement window,
returned to map 0, killed and visibly harvested a Deer, sold Venison twice,
and bought HP drugs `0 -> 5 -> 10`. r67 then finalized the persisted result:
135/135 HP, two visible five-drug belt stacks, EXP 24,341/30,000, and a visible
Merchant Whitney equipment repair before ordinary SpittingSpider travel. The
three finalized r64/r65/r67 reports add 914,093 ms, 526 inputs, nine historical
kill rows, zero deaths/revives, zero shortcut violations, and zero critical
browser/network diagnostics. At the r67 boundary the character remained level
14; q25 was 6/20 and q30 remained open.

r68 resumed that exact persisted state at map 0 `(313/314,603)`, level 14 with
EXP 24,341/30,000, full HP, ten HP drugs, and a worn left bracelet. Equipment
repair selected Merchant Alice on map 0141, but the transfer remained pinned by
four adjacent low-level actors. Across 1,805,231 ms and 1,191 physical inputs it
attempted both known transfer entrances, emitted 960 turns, and issued zero
attacks: 0/0 goals, zero kills, zero shortcuts, and zero critical
browser/network diagnostics. The defect was exact rather than general map
disconnection: disabling the combat-resource budget also defaulted physical
occupancy clearing off and discarded the explicitly supplied non-funding
accounting goal.

Source commit `acd95a4b4` (clean `d8264b5c1`) separates those policies. Repair
travel still does not impose a funding gate, but it preserves the explicit
resource-accounting goal and explicitly permits bounded clearing of certified,
physically clickable trivial occupancy. A red/green regression locks this
case, and the full Quest Agent gate passes 176/176.

r69 then resumed the exact authoritative r68 save on the patched runtime and
left the former stuck position through ordinary input. It completed 10/10
bounded SpittingSpider goals and ten target-specific kill rows in 1,300,030 ms
with 639 physical inputs, advancing EXP `24,341 -> 28,661` (+4,320), with zero
death/revive, potion use, shortcut violation, target quarantine, or critical
browser/network diagnostic. Some object ids legitimately recur only after a
complete absence and positive-HP respawn lifecycle, so this is ten
target-specific successes rather than a claim of ten globally unique ids. On
bootstrap, recent nearby attackers caused the repair routine to defer safely;
therefore r69 proves the formerly stuck saved state is resumable and progressing
but does not relabel the exact repair-clear branch as live-covered. That branch
remains red/green unit-covered. At the finalized r69 boundary the character is
level 14 with EXP 28,661/30,000; q25 remains 6/20 and q30 remains 0/1.

r70 resumed that exact save and supplied live coverage for the shared
non-funding travel-policy separation without misrepresenting it as the
equipment-repair branch. Pending field combat consumed all ten HP drugs; the
depleted shelter journey then physically cleared one RakingCat and one
Scarecrow under the preserved non-funding accounting goal, recorded one
authoritative death/revive, entered map 0141, and recovered. The ordinary
client path returned to map 0, killed and visibly harvested a Deer, acquired a
second Venison, completed two visible sales, and bought HP drugs `0 -> 5 -> 10`
across a threat-driven safe-room settlement. It then resumed the real
SpittingSpider journey.

The finalized r70 report records 1,800,710 ms, 975 physical inputs, three kill
rows, one death/revive, ten potion uses, two visible purchases, zero shortcut
violations, zero target quarantines, and zero critical browser/network
diagnostics. Its budget expired during the long field walk at map 0
`(284,393)`, not during recovery: final state is 130/135 HP, ten HP drugs, 172
gold, and EXP `28,865/30,000`. The two sanitized start/final frames were
visually inspected and agree with the structured state. This closes a complete
live recovery/restock cycle and preserves a resumable position; it remains a
0/1 grind-goal segment and does not certify level 15, q25, or q30. Those quest
states remain 6/20 and 0/1 respectively.

r71 resumed the stocked r70 field position, physically reached the nearest
SpittingSpider band, and completed four of four grind goals. The fourth kill
crossed the authoritative threshold from level 14 and EXP 29,945/30,000 to
level 15 and EXP 377/40,000; the planner then immediately replaced the
preparation goal with q25 CannibalPlant hunting and harvesting. It killed six
CannibalPlants and two certified incidental threats. Four q25 harvest flows
completed, while two were explicitly marked retryable failures after an Oma or
SpittingSpider preempted the corpse and the corpse left the visible world.
None of those six plants produced the required random quest drops, so q25
honestly remains 6/20 rather than being advanced from kill count alone.

The r71 report records 564,969 ms, 262 physical inputs, 8/10 successful goals,
12 kill rows, two visible gold pickups, zero deaths/revives, zero potion use,
zero shortcuts, zero quarantines, and zero critical browser/network
diagnostics. It stopped at its explicit ten-goal bound, not at a runtime or
route failure. The final state is map 0 `(124,214)`, level 15 with EXP
2,009/40,000, 149/149 HP, ten HP drugs, and 421 gold. The level-up combat frame,
q25 corpse frame, and final frame were visually inspected and agree with the
structured report. This certifies the level-15 transition and live q25 route,
not q25 or q30 completion.

r72 ran the pre-fix in-memory runtime from the exact r71 state and supplied the
bounded counterexample for dense q25 harvest timing. It killed twelve
CannibalPlants and two incidental Omas, but only 3/11 goals completed: three
corpses were preempted after an adjacent attacker became active and five more
ended with an explicit incomplete harvest lifecycle. Two ordinary no-drop
harvests and one quest-drop harvest completed. That last result advanced the
authoritative CannibalStem objective `6 -> 7`; the normal inventory also held
nine CannibalLeaves, five CannibalFruits, and one CannibalPoison, while the
separate quest counter for CannibalLeaf remained 0/10.

The r72 report records 1,805,174 ms, 885 physical inputs, 14 kill rows, 28
harvest commands, three visible gold pickups, zero death/revive, zero potion
use, zero shortcuts, zero quarantines, and zero critical browser/network
diagnostics. It finished at map 0 `(164,238)`, level 15 with EXP 3,833/40,000,
149/149 HP, ten HP drugs, and 751 gold. Two long alternate-field walks consumed
most of the budget, but did not hide the exact ordering defect: the old policy
created the quest corpse before reacting to an already attacking nearby actor.

Source commit `e533efbb3` (clean `40ffedcb5`) adds a pre-combat guard for
harvest goals. It handles at most two already attacking, quest-certified
nearby threats through ordinary combat before creating the source corpse; an
unsafe or excess threat triggers the existing physical disengagement instead.
The new contract was first red, then passed with the full 177/177 Quest Agent
gate in both source and clean worktrees. This is a bounded ordering fix, not a
claim that post-kill attacks or harvest RNG disappear. The exact r72 resume was
therefore replayed on the patched runtime as recorded below.

r73 performed that exact-state replay on source `e533efbb3`. Seven of ten
bounded q25 goals completed, compared with 3/11 in the pre-fix r72 segment.
Spawn ordering was not controlled between runs, so this is marked live
improvement rather than a deterministic throughput benchmark. The replay
exercised all three new branches through ordinary client inputs: it cleared an
already attacking Oma before source combat, safely disengaged when the bounded
pre-harvest defence limit was reached, and twice switched to an already active
CannibalPlant before creating a new corpse. Thirteen CannibalPlants and one Oma
were killed; one successful quest drop advanced CannibalStem `7 -> 8`. The
three failed goals remained explicit and retryable: one bounded preflight
disengagement and two post-kill attacker/preemption cases outside this
pre-combat fix.

The finalized r73 report records 525,497 ms, 262 physical inputs, 14 kill rows,
24 harvest commands, two visible gold pickups, zero death/revive, zero potion
use, zero purchases, zero shortcuts, zero quarantines, and zero critical
browser/network diagnostics. Its final state is map 0 `(176,169)`, level 15
with EXP 5,681/40,000, 149/149 HP, ten HP drugs, and 1,045 gold; q25 is 8/20
and q30 remains 0/1. The quest-progress, safe-failure, and final frames were
visually inspected and agree with the structured report. This validates the
patched branches in live play, but does not certify q25 or q30 completion.

r74 resumed that exact state and supplied a separate zero-potion recovery
checkpoint. Its first q25 attempt hit the bounded pre-harvest defence limit,
physically disengaged from the active Oma, and then stopped the target attempt
when ten HP drugs had been consumed instead of sacrificing another quest
corpse. The outer recovery loop walked from the CannibalPlant field toward the
real map-0141 GroceryStore transfer. It confirmed three ordinary
travel-occupancy kills (HookingCat, RakingCat, and Scarecrow), explicitly
quarantined one different RakingCat after five attacks produced no
target-specific response, and continued collision-routed movement until the
run budget expired in a dense actor cluster at `(326,535)`.

The finalized r74 report records 904,508 ms, 513 physical inputs, 3 kill rows,
ten potion uses, zero death/revive, zero purchases, one target quarantine, zero
shortcuts, and zero critical browser/network diagnostics. Passive recovery
left the character at 149/149 HP with no HP drugs, level 15 and EXP
6,217/40,000; q25 remains 8/20 and q30 remains 0/1. Its combat and final
screenshots were visually inspected and match the report. This is a resumable
partial recovery trace, not a completed restock loop or quest certificate.

r75 resumed the exact r74 state on the same pre-fix in-memory runtime and
confirmed that the remaining recovery cost was not a one-run anomaly. It
advanced from `(326,535)` to `(304,567)` toward outdoor Merchant Ruben, but
seven different low-level occupants each consumed the old five-attack,
15-second no-response window before quarantine. Three other occupants produced
recorded combat completion. The normal-client merchant route was attempted but
the dialog could not be opened before the 900-second budget expired, so the
agent correctly held quest departure at zero HP drugs.

The finalized r75 report records 912,805 ms, 463 physical inputs, 266 attack
commands, 3 kill rows, seven target quarantines, zero death/revive, zero potion
use, zero purchases, zero shortcuts, and zero critical browser/network
diagnostics. Final state is map 0 `(304,567)`, 149/149 HP, level 15 with EXP
7,485/40,000 and 1,045 gold; q25 remains 8/20 and q30 remains 0/1. The final
frame shows the character inside the reported dense actor cluster. Together,
r74 and r75 are consecutive counterexamples to the old incidental-combat
no-response latency, not completed recovery or quest certificates.

Source commit `e8065e599` (clean `32f5b2f9d`) separates the two audit budgets:
ordinary quest combat still requires five real attacks over 15 seconds before
an unresponsive target is quarantined, while incidental travel clearing can
rotate after two real attacks over four seconds because it is only trying to
open one occupied movement tile. No-response is still a failed clear and is
never counted as a kill. The new unit contract was first red, then passed with
the full 178/178 Quest Agent gate in both source and clean worktrees. The exact
r75 patched replay is recorded below.

r76 loaded source `e8065e599` and resumed the exact r75 state. The character
died normally while trying to clear the initial dense cluster, two visible
town-revive attempts awaited authoritative acknowledgement, and the recovery
then completed through ordinary client actions: Merchant Ruben restocked HP
drugs `0 -> 10` for 400 gold. The agent entered map 0141 and used the visible
repair services to restore WornIronBracelet durability `0 -> 3739` through
Merchant Betty and OldCopperRing `778 -> 4699` through Merchant Alice. It then
left the shelter and crossed the former r74/r75 congestion band without any
target quarantine. Because no target reached the new four-second quarantine
branch, this is live closure for exact-state recovery and repair, not direct
live timing proof of that branch.

The finalized r76 report records 901,065 ms, 523 physical inputs, one
death/revive, one visible purchase, two visible repairs, zero kills, zero
target quarantines, zero potion use, zero shortcuts, and zero critical
browser/network diagnostics. Prior CannibalPlant resource strain correctly
changed the next preparation goal to level 16. The remaining budget expired
during the ordinary SpittingSpider walk at map 0 `(316,524)`, with 133/149 HP,
ten HP drugs, 233 gold, and EXP 7,569/40,000. q25 remains 8/20 and q30 remains
0/1. The exact start and final frames were visually inspected and match the
report; the grind goal and quests remain incomplete.

r77 resumed the safe r76 field state and completed the full physical journey
to the selected SpittingSpider band, reducing a 336-tile initial distance to a
visible, clickable target without a death, potion use, or supply return. Two of
four bounded grind goals completed with two target-specific confirmed kills.
One different spider was conservatively quarantined after the unchanged
quest-combat five-attack/15-second no-response window, and the final goal
expired while rotating to another real field. Authoritative EXP advanced
`7,569 -> 8,433`; the larger delta than the two recorded kill rows is retained
as state evidence but is not relabeled as additional confirmed kills.

The finalized r77 report records 903,536 ms, 456 physical inputs, two confirmed
kill rows, one target quarantine, zero death/revive, zero potion use, zero
purchases, zero shortcuts, and zero critical browser/network diagnostics. It
ends at map 0 `(506,168)`, 149/149 HP, ten HP drugs, and 233 gold. q25 remains
8/20 and q30 remains 0/1. The confirmed-combat and final frames were visually
inspected and agree with the report. This proves live level-16 preparation
progress, not level 16 or quest completion.

r78 resumed inside the reached SpittingSpider region and measured the
short-range grind throughput without paying the cross-map journey again. The
first selected spider left AOI without target-specific death evidence and was
correctly failed. Goals 2-10 then completed with nine distinct confirmed
target object ids. A visible gold pickup advanced gold `233 -> 341`, while
authoritative EXP advanced `8,433 -> 12,105`. As in r77, the EXP delta is
reported independently and is not converted into extra unrecorded kill claims.

The finalized r78 report records 509,352 ms, 257 physical inputs, 9/10
successful goals, nine confirmed kill rows, one visible gold pickup, zero
target quarantines, zero death/revive, zero potion use, zero shortcuts, and
zero critical browser/network diagnostics. It ends at map 0 `(646,128)`,
143/149 HP, ten HP drugs, and level 15. q25 remains 8/20 and q30 remains 0/1.
The first-success and final combat frames were visually inspected. The final
frame still showed the last target at 6/65 HP; target-specific death/EXP settled
in the following subsecond before report finalization, so that frame is treated
as combat-in-progress rather than a post-death visual certificate. This is
efficient preparation progress, not level 16 or quest completion.

r79 resumed the final r78 combat region with a larger 20-goal ceiling, but a
multi-spider overlap immediately converted the run into a recovery test. The
first grind goal completed against object `202218`; the character then consumed
all ten HP drugs while physically disengaging, died, revived, and returned to
Merchant Ruben. Available gold funded a visible partial restock `0 -> 5` for
200 gold rather than inventing the full ten-drug departure stock. The agent
entered map 0141 for safe settlement, returned through its visible exit, and
started toward a different lower-risk SpittingSpider field. A visible repair
interaction emitted an item-repaired chat but produced no recorded durability
or gold delta, so it is not counted as a completed repair.

The finalized r79 report records 903,077 ms, 536 physical inputs, 1/2
successful goals, one confirmed kill row, one death/revive, ten potion uses,
one visible purchase, zero target quarantines, zero shortcuts, and zero
critical browser/network diagnostics. It ends at map 0 `(284,481)`, 143/149
HP, five HP drugs, 141 gold, and EXP 12,567/40,000. q25 remains 8/20 and q30
remains 0/1. The high-risk combat and final resumed-travel frames were visually
inspected and match the report. This is a complete low-funds recovery cycle and
one preparation kill, not sustained grind throughput or quest completion.

r80 resumed that exact low-funds state and directly exercised the patched
incidental-travel no-response budget under live congestion. The character used
ordinary movement and attacks while trying to reach a visible Deer funding
source, confirmed six incidental low-level kills, and quarantined eleven
different occupied-tile targets. Every quarantine carries the exact reason
`2 real attacks over 4000ms produced no target-specific combat packet`; none is
counted as a kill. The recovery portal also rotated after 88,510 ms without a
distance improvement, proving that the outer route can replan independently of
the faster per-occupant rotation.

The finalized r80 report records 900,324 ms, 508 physical inputs, six confirmed
kill rows, eleven target quarantines, zero completed goals, zero death/revive,
zero potion use, zero purchase, zero shortcuts, and zero critical
browser/network diagnostics. Authoritative EXP advanced independently from
12,567 to 14,125, while q25 remains 8/20 and q30 remains 0/1. It ends on map 0
at `(305,576)` with 149/149 HP, five HP drugs, and 141 gold. The start and final
frames were visually inspected; the final frame shows the reported dense actor
cluster and a live game screen rather than a disconnect or modal stall. This is
direct live timing evidence for fast blocker rotation, not a funding closure,
level-16 milestone, or quest completion certificate.

r81 resumed the exact dense r80 endpoint and completed the recovery work that
r80 left open. Seven more incidental occupied-tile targets were quarantined;
all seven carry the same two-attack/four-second no-response reason and none is
counted as a kill. Four other low-level targets produced target-specific kill
evidence. Ordinary collision-routed movement escaped the cluster, one visible
gold pickup advanced gold `141 -> 251`, map 0141 provided safe passive
settlement, and the character returned to map 0 for a visible Merchant Ruben
purchase that restored HP drugs `5 -> 10` for 200 gold.

The finalized r81 report records 903,365 ms, 481 physical inputs, four
confirmed kill rows, seven target quarantines, one gold pickup, one purchase,
zero completed goals, zero death/revive, zero potion use, zero shortcuts, and
zero critical browser/network diagnostics. Authoritative EXP advanced
independently from 14,125 to 15,213. It ends on map 0 at `(295,609)` with
149/149 HP, ten HP drugs, and 51 gold; q25 remains 8/20 and q30 remains 0/1.
The start and final frames were visually inspected and show the congested
resume point followed by the live town/merchant area with the reported stock
and gold. This closes the low-funds restock continuation but does not certify
level 16, q25, or q30 completion.

r82 resumed the fully stocked r81 town state and selected the learned
SpittingSpider level-16 preparation objective. It used ordinary client movement
to reduce the field distance from roughly 257 tiles to 121 despite two dense
collision stalls. At the second stall, an adjacent Scarecrow without a usable
rendered hit surface continued attacking; one different Scarecrow produced a
confirmed kill and one no-response target was quarantined. The character then
died normally, revived in town, and the planner marked that source route as a
retryable lethal failure instead of recording the grind goal as successful.

The finalized r82 report records 900,346 ms, 528 physical inputs, 0/1
successful goals, one confirmed kill row, one death/revive, four potion-use
events, one target quarantine, zero purchases, zero shortcuts, and zero
critical browser/network diagnostics. It ends on map 0 at `(288,616)` with
149/149 HP, seven HP drugs, 51 gold, and EXP 15,303/40,000; q25 remains 8/20
and q30 remains 0/1. The start, failed-goal, and final frames were visually
inspected and show the ordinary town departure followed by the authoritative
post-revive town state. This is a bounded route-risk counterexample, not level
16 or quest completion.

r83 resumed the authoritative r82 post-revive state and first restored a
sustainable departure stock through normal-client economy actions. A visible
Deer was killed and harvested, Venison advanced `1 -> 2`, Butcher John bought
the supply for a visible gold change `51 -> 289`, and Merchant Ruben restored
HP drugs `7 -> 10` for 120 gold. The inherited lethal-route memory then chose a
different SpittingSpider field instead of repeating r82's approach. The agent
walked the alternate route from roughly 421 tiles away to 94 before the runtime
budget expired, preserving the intermediate transform for the next resume.

The finalized r83 report records 900,189 ms, 483 physical inputs, 0/1
successful goals, one confirmed Deer kill with completed harvest, one supply
pickup, one visible purchase, zero death/revive, zero potion use, zero target
quarantines, zero shortcuts, and zero critical browser/network diagnostics. It
ends on map 0 at `(295,275)` with 148/149 HP, ten HP drugs, 169 gold, and EXP
15,339/40,000; q25 remains 8/20 and q30 remains 0/1. The start and final frames
were visually inspected and show the town funding state followed by the live
alternate route. This proves cross-run risk-memory rerouting and economic
recovery, not arrival at the grind field or quest completion.

r84 resumed the r83 intermediate transform and re-evaluated the current map to
select a nearer, non-r82 SpittingSpider field. It completed the ordinary
184-tile walk, then finished all eleven bounded grind goals with eleven
target-specific SpittingSpider kill rows. The rows span eight server object ids;
three ids recur only after later live reappearance, and every row carries a
fresh positive authoritative EXP delta. They remain kill-row evidence rather
than a claim of eleven permanently unique spawn ids. After the eleventh goal,
the resource guard stopped combat and began a normal map-0141 withdrawal.

The finalized r84 report records 902,060 ms, 457 physical inputs, 11/11
successful goals, eleven confirmed kill rows, nine potion-use events, one
incidental travel quarantine, zero death/revive, zero purchases, zero
shortcuts, and zero critical browser/network diagnostics. Authoritative EXP
advanced independently from 15,339 to 19,983. It ends mid-withdrawal on map 0
at `(238,470)` with 142/149 HP, one HP drug, and 169 gold; q25 remains 8/20 and
q30 remains 0/1. The start, first-goal, and final frames were visually
inspected. The first-goal frame still shows 4/65 target HP before the later
target-specific death/EXP settlement, so it is retained as combat-in-progress
rather than relabeled as a post-death visual certificate. This proves the
alternate field's sustained throughput and bounded withdrawal, not safe-room,
level-16, or quest completion.

r85 resumed the exact r84 mid-withdrawal transform with the persisted resource
recall active. It crossed the remaining 138-tile town journey through the same
dense occupancy band, confirming four ordinary low-level kills and rotating
six different no-response blockers without counting those failures as kills.
Merchant Ruben then performed a visible partial restock `1 -> 5` for the
available 160 gold. Because an attacker was still present and the character
could not afford the ten-drug departure stock, the planner entered map 0141
instead of returning to combat.

The finalized r85 report records 900,454 ms, 511 physical inputs, four
confirmed kill rows, six target quarantines, one visible purchase, zero
completed goals, zero death/revive, zero potion use, zero shortcuts, and zero
critical browser/network diagnostics. Authoritative EXP advanced independently
from 19,983 to 21,205. It ends inside map 0141 at `(2,11)` with 149/149 HP, five
HP drugs, and 9 gold; q25 remains 8/20 and q30 remains 0/1. The safe-room pace
reached 17,416 ms of the 20-second settlement window before the run budget
expired, and two optional interior pace targets had no live collision path.
The start and final frames were visually inspected and match the field-to-store
transition. This closes the physical withdrawal and partial restock, not the
safe-room settlement, full departure stock, level-16, or quest milestone.

r86 resumed inside map 0141 and immediately confirmed that the cross-run safe
state could exit normally rather than repeating r85's nearly complete
settlement wait. The character sold the retained Venison for a visible gold
change `9 -> 214`, and Merchant Ruben restored HP drugs `5 -> 10` for 200
gold. It then completed the full 419-tile hostile-corridor leg toward a distant
SpittingSpider field and advanced the second leg from 242 to 140 tiles before
the runtime budget expired.

The finalized r86 report records 900,859 ms, 474 physical inputs, 0/1
successful goals, one incidental Scarecrow kill row, one no-response target
quarantine, one visible purchase, zero death/revive, zero potion use, zero
shortcuts, and zero critical browser/network diagnostics. The Scarecrow row
has target-specific death evidence but no immediate EXP delta, so it is not
used as an EXP claim; authoritative EXP independently advanced from 21,205 to
21,295. It ends on map 0 at `(395,166)` with 148/149 HP, ten HP drugs, and 14
gold; q25 remains 8/20 and q30 remains 0/1. The initial bootstrap and final
field frames were inspected; the latter matches the saved transform and full
stock. This closes the safe-room resume and restock continuation, not arrival
at the final grind field, level 16, or quest completion.

r87 resumed the second r86 travel leg and re-evaluated a nearer SpittingSpider
field from the persisted transform. After two empty spawn bands, it found a new
object-id range and completed four distinct target-specific kills. The fifth
goal failed explicitly when movement and combat reduced the sustainable stock
from ten HP drugs to five at roughly 83/149 HP. The resource guard cooled that
source, consumed the remaining stock while disengaging, and continued the
committed shelter escape with zero drugs rather than aborting or claiming the
fifth goal.

The finalized r87 report records 900,498 ms, 479 physical inputs, 4/5
successful goals, four distinct SpittingSpider kill rows, ten potion-use
events, zero target quarantines, zero death/revive, zero purchases, zero
shortcuts, and zero critical browser/network diagnostics. Authoritative EXP
advanced independently from 21,295 to 22,807. It ends on map 0 at `(259,370)`
with 149/149 HP, zero HP drugs, and 14 gold, 96 tiles from the current hostile-
corridor withdrawal waypoint; q25 remains 8/20 and q30 remains 0/1. The start,
first-goal, and final frames were visually inspected. The first-goal frame
shows 6/65 target HP before the later target-specific death/EXP settlement and
is therefore combat-in-progress, while the final frame matches the zero-stock
withdrawal. This proves the alternate source and depleted-escape continuation,
not safe arrival, restock, level 16, or quest completion.

r88 resumed the exact r87 zero-stock waypoint and completed the remaining
238-tile return to the town supply area. Because 14 gold could not buy an HP
drug and live attackers still occupied the merchant entrance, the character
entered map 0141 twice rather than pretending to restock. Both safe-room cycles
completed their 20-second settlement through visible two-tile pacing, directly
closing r85's 17,416 ms partial window. On the second exit, short bounded
disengagement steps moved the attacker cluster away from the transfer before
the runtime budget expired.

The finalized r88 report records 901,386 ms, 474 physical inputs, zero goals,
one incidental Scarecrow kill row with positive immediate EXP, two
no-response target quarantines, zero death/revive, zero potion use, zero
purchases, zero shortcuts, and zero critical browser/network diagnostics. It
ends on map 0 at `(291,612)` with 149/149 HP, zero HP drugs, 14 gold, and EXP
22,897/40,000; q25 remains 8/20 and q30 remains 0/1. The start and final frames
were visually inspected and show the zero-stock field resume followed by the
live Ruben area under one-point incoming damage. This proves safe settlement
and physical entrance disengagement, not funding, restock, level 16, or quest
completion.

r89 resumed beside the supply area and first attempted a visible Deer harvest;
active Scarecrow pressure preempted it without any Venison inventory increase,
so no new supply was claimed. After a completed map-0141 settlement, the agent
sold one retained Venison for a visible gold change `14 -> 213` and Merchant
Ruben performed the affordable partial restock `0 -> 5` for 200 gold. It then
used ordinary Scarecrow combat as a fallback funding source, but none of the
six target-specific kill rows produced a visible gold pickup before the run
returned through another completed safe-room settlement.

The finalized r89 report records 900,422 ms, 477 physical inputs, zero goals,
six Scarecrow kill rows across four server ids, one visible purchase, zero
gold/supply pickups, five target quarantines, zero death/revive, zero potion
use, zero shortcuts, and zero critical browser/network diagnostics. Three
quarantines use the incidental two-attack/four-second reason, while two funding
combat targets retain the conservative five-attack/15-second reason; this live
run exercises both policy branches without counting either failure as a kill.
It ends on map 0 at `(300,622)` with 149/149 HP, five HP drugs, 13 gold, and EXP
23,369/40,000; q25 remains 8/20 and q30 remains 0/1. The start and final frames
were visually inspected and match the partial-stock recovery. This proves a
truthful low-funds fallback and policy split, not full funding, level 16, or
quest completion.

r90 resumed the partial-stock state at the first safe transfer. Consecutive
outside attackers initially forced normal settlement cycles and bounded
eight-tile disengagement steps. The escape did not become a permanent orbit:
the recovery portal rotated to the second map-0141 entrance, settled there,
later returned to the first entrance, and continued moving through the town
funding band. Four distinct Scarecrow targets produced confirmed kill rows and
authoritative gold advanced `13 -> 39`, although no individual visible gold
pickup event was recorded.

The finalized r90 report records 904,248 ms, 447 physical inputs, zero goals,
four distinct Scarecrow kill rows, three incidental no-response quarantines,
zero death/revive, zero potion use, zero purchases, zero gold/supply pickups,
zero shortcuts, and zero critical browser/network diagnostics. It ends on map
0 at `(305,612)` with 149/149 HP, five HP drugs, 39 gold, and EXP
23,609/40,000; q25 remains 8/20 and q30 remains 0/1. The start and final frames
were visually inspected and show the first entrance followed by a live town
position with multiple visible Deer and the reported stock/gold. This proves
portal rotation prevents a deterministic shelter orbit and that funding can
make state progress, not full restock, level 16, or quest completion.

r91 resumed the r90 town position, quarantined one immediate no-response
attacker, and completed a normal map-0141 settlement before retrying the
visible Deer source. Six physical harvest inputs advanced Venison `1 -> 2`.
Butcher John then bought one unit for a visible gold change `39 -> 289`,
Merchant Ruben restored HP drugs `5 -> 10` for 200 gold, and one Venison was
retained for a later recovery. Only after the full departure stock was visible
did the agent restart the learned level-16 preparation walk.

The finalized r91 report records 900,819 ms, 476 physical inputs, 0/1
successful goals, one target-specific Deer kill with completed harvest, one
supply pickup, one visible purchase, one incidental target quarantine, zero
death/revive, zero potion use, zero shortcuts, and zero critical
browser/network diagnostics. It ends on map 0 at `(306,389)` with 148/149 HP,
ten HP drugs, one Venison, 89 gold, and EXP 23,645/40,000, roughly 40 tiles from
the current SpittingSpider corridor target; q25 remains 8/20 and q30 remains
0/1. The start and final frames were visually inspected and match the town
funding state followed by the fully stocked travel state. This closes the r87-
r90 low-funds recovery chain, not field arrival, level 16, or quest completion.

r92 resumed the exact r91 departure state and independently selected a nearer
SpittingSpider field around `(111,318)` instead of mechanically following the
old corridor target. The normal client walked from `(306,389)` into that live
field, rotated through visible targets, and cooled down one temporarily visible
but unreachable target rather than orbiting it. Four visible gold pickups
advanced gold `89 -> 199 -> 292 -> 380 -> 529`; the ten departure HP drugs were
not consumed.

The finalized r92 report records 889,363 ms, 422 physical inputs, 19/20
successful goals, 19 target-specific SpittingSpider kill rows across nine live
server object ids, four visible gold pickups, zero death/revive, zero potion
use, zero shortcuts, and zero critical browser/network diagnostics. It advances
authoritative EXP `23,645 -> 31,637/40,000` and ends on map 0 at `(63,334)`
with 145/149 HP, ten HP drugs, one Venison, and 529 gold. Its goal budget ends
during the twentieth target, so the expected nonzero process exit means the
full route remains incomplete; q25 is still 8/20, q30 is still 0/1, and level
16 has not yet been reached. The start, first combat, and final frames were
visually inspected and match the reported transform, live SpittingSpider
combat, drops, HP, EXP, and gold state. This is a high-throughput preparation
segment, not a q25/q30 or level-16 completion certificate.

r93 resumed the exact r92 field state and completed two more target-specific
SpittingSpider goals before a denser respawn consumed eight of the ten HP drugs
and triggered the ordinary resource guard. The agent abandoned the unsafe
fight, walked the low-exposure corridor toward map 0141, and recovered from a
temporary cluster north of the merchant entrance by clearing two responsive
incidental occupants, cooling down six nonresponsive occupants with the exact
fast travel-blocker policy, expiring stale collision corrections, and
replanning. It did not teleport, grant supplies, or claim the nonresponsive
targets as kills.

The finalized r93 report records 900,515 ms, 521 physical inputs, 2/2
successful goals, four kill rows (two SpittingSpiders plus one RakingCat and one
Scarecrow), eight potion uses, six target quarantines, zero death/revive, zero
purchase, zero shortcuts, and zero critical browser/network diagnostics. It
advances authoritative EXP `31,637 -> 33,249/40,000` and ends on map 0 at
`(281,609)` beside the merchant district with full 149/149 HP, two HP drugs,
one Venison, and 529 gold. The runtime limit lands during the final physical
approach to `(302,622)`, so safe-room entry and restock remain for the next
resume; q25 is still 8/20, q30 is still 0/1, and level 16 remains open. The
start, first combat, and final frames were visually inspected and match the
reported live field, HP-drug consumption, endpoint, HP, EXP, and gold state.

r94 completed the r93 merchant closure immediately: Ruben visibly restored HP
drugs `2 -> 10` for 320 gold. A dense departure then produced one normal
death/revive; the resumed recovery returned to Ruben and visibly restored the
post-combat stock `7 -> 10` for another 120 gold. The live-risk memory cooled
down that SpittingSpider source and selected a nearer Oma field instead of
repeating the lethal route. In the replacement field, one Oma produced a fresh
positive EXP delta while two different visible Oma ids exhausted the
conservative five-real-attack/15-second quest-combat window and were not
counted as kills.

The finalized r94 report records 908,257 ms, 504 physical inputs, 1/5
successful goals, one target-specific Oma kill, one death and one completed
revive, four potion uses, two visible shop purchases, two conservative target
quarantines, zero shortcuts, and zero critical browser/network diagnostics. It
advances authoritative EXP `33,249 -> 33,309/40,000` and ends on map 0 at
`(254,545)` in the Oma field with 86/149 HP, ten HP drugs, one Venison, and 89
gold. The budget expires during the fifth goal's physical search; q25 remains
8/20, q30 remains 0/1, and level 16 remains open. The initial resumed frame
contains transient vertical slice tearing, while the later merchant and Oma
frames render normally and match the reported state. Treat that visual artifact
as an open observation rather than a successful frontend certificate.

r95 resumed inside the exact r94 Oma field, immediately observed active damage,
and treated the save as an unsafe supply state rather than retrying the two old
object ids. All ten HP drugs were consumed during the normal 77-tile shelter
escape. The agent entered map 0141, recovered to full HP, sold the one retained
Venison for a visible gold change `89 -> 289`, and bought five HP drugs for 200
gold. A visible Deer corpse accepted three physical harvest inputs but produced
no new Venison, so the agent truthfully fell back to local Scarecrow funding.

The finalized r95 report records 917,051 ms, 540 physical inputs, no quest/grind
goals, three target-specific Scarecrow kill rows, ten potion uses, one visible
purchase, seven target quarantines, zero death/revive, zero shortcuts, and zero
critical browser/network diagnostics. The first three funding targets retain
the conservative five-real-attack/15-second window; later entrance occupants
use the separate two-real-attack/four-second travel-clearing window. The first
0141 entrance stalled at distance one and rotated to the second entrance after
45,544 ms without improvement, but continuing dynamic occupancy kept the agent
at the old entrance until the runtime cap. It ends on map 0 at `(302,622)` with
full 149/149 HP, five HP drugs, zero Venison, 89 gold, and authoritative EXP
33,525/40,000. q25 remains 8/20, q30 remains 0/1, and level 16 remains open.
The start and final frames were visually inspected and match the live Oma field
and crowded entrance state. Exact-state r96 replay is required before treating
the remaining entrance congestion as a repeatable recovery defect.

The exact-state r96 replay supplies that counterexample. It ran for 900,235 ms
and sent 403 ordinary keyboard/mouse inputs, but the authoritative player stayed
on map 0 at `(302,622)` for the whole report. The recovery loop repeatedly
waited at distance zero from the visible one-cell 0141 entrance, rotated to the
alternate entrance after its bounded stall window, then returned to the same
source without ever observing a map change. It records 0/0 completed goals,
zero kills, zero deaths/revives, zero potion uses or purchases, ten conservative
target quarantines, zero shortcuts, and zero critical browser/network
diagnostics. Final HP remains 149/149, EXP remains 33,525/40,000, inventory
remains five HP drugs and no Venison, gold remains 89, q25 remains 8/20, and
q30 remains 0/1. Both private frames were visually inspected and confirm the
same `(302,622)` endpoint; the fatal capture also contains transient light-column
and roof ghosting, so it is evidence of the navigation stall rather than a
frontend rendering certificate. This establishes a resumable transfer-source
defect; it does not establish q25/q30 or level-16 progress.

Source commit `2a3751901` and its main-based equivalent `dc03df754` close the
distance-zero input hole without adding a direct movement or map command. The
normal movement handlers first complete a Crystal source restored as the
authoritative player transform, while the browser agent sends one ordinary
cardinal key when there is no geometric direction toward the tile it already
occupies. The server regression was red before the change and green after it;
the complete source Simulation suite and the 178/178 Quest Agent gate pass.

The exact-state r97 replay starts from the same map-0 `(302,622)` transform and
does not repeat the 15-minute zero-distance loop. In 902,492 ms it sends 468
physical inputs, completes two visible map-0-to-0141 entries, two normal return
transfers, and both 20-second safe-room settlement cycles. Its action audit
contains two `enter-visible-map-transfer-diagonal-approach` inputs and four
`enter-visible-map-transfer` inputs. The policy first chose a nearby Deer
funding action and stepped off the saved source, so the new
`reactivate-visible-map-transfer` action itself remains red/green unit-covered
rather than being relabeled as live-covered.

r97 records nine kill rows (seven Scarecrows, one HookingCat, and one Deer),
one visible Venison supply pickup, zero deaths/revives, zero potion uses,
purchases, quarantines, shortcuts, or critical browser/network diagnostics.
Authoritative EXP advances `33,525 -> 34,019/40,000`; the final inventory has
five HP drugs and two Venison. The budget expires after visible harvest when
the agent cannot open Butcher John, so gold remains 89 and the final position is
map 0 `(286,638)` at full 149/149 HP. q25 remains 8/20, q30 remains 0/1, and
level 16 remains open. Both private frames were visually inspected: they match
the reported initial and final coordinates but retain vertical-slice/roof
ghosting, so r97 is a transfer/recovery certificate, not a frontend rendering
certificate or a sell/restock closure.

r98 closes that deferred economy chain through visible NPC input. Butcher John
sells one Venison for the observed gold change `89 -> 352`, while one Venison
remains in the bag; Merchant Ruben then restores HP drugs `5 -> 10` for 200
gold. Merchant Whitney emits `Item repaired.`, but the report has no confirmed
durability or gold delta, so this is not counted as a completed repair. With
full stock, the policy selects the inherited level-16 SpittingSpider
preparation goal and starts the real collision-routed field journey.

The finalized r98 report records 902,261 ms, 482 physical inputs, 0/1 completed
goals, one visible purchase, zero kills, deaths/revives, potion uses,
quarantines, shortcuts, or critical browser/network diagnostics. It moves from
map-0 `(286,638)` to `(279,322)` before the runtime limit expires during the
long field route. Final state is 146/149 HP, ten HP drugs, one Venison, 152
gold, and unchanged EXP `34,019/40,000`; q25 remains 8/20, q30 remains 0/1,
and level 16 remains open. The initial private frame again contains severe
vertical-slice tearing; the final frame renders coherently and matches the
reported coordinate, HP, gold, and ten-drug belt. Treat r98 as the visible
sell/restock closure and an intermediate travel checkpoint, not grind or
frontend-rendering acceptance.

r99 resumes that intermediate route, reaches a live SpittingSpider band, and
completes 6/6 grind goals against six distinct target ids. One additional
Scarecrow is killed during the subsequent ordinary withdrawal. The seven kill
rows advance authoritative EXP `34,019 -> 36,863/40,000`, while one visible
gold pickup changes gold `152 -> 266`. Dense spider combat consumes all ten HP
drugs without a death; the resource guard stops further combat and begins the
normal map-0141 shelter journey.

The finalized r99 report records 913,787 ms, 489 physical inputs, seven kill
rows, ten potion uses, one gold pickup, seven fast travel-blocker quarantines,
zero deaths/revives, purchases, shortcuts, or critical browser/network
diagnostics. Every quarantine keeps the exact two-real-attack/four-second
nonresponse reason and none is counted as a kill. The budget expires in the
dense return corridor at map-0 `(311,563)`, leaving a resumable full 148/149 HP
state with zero drugs, one Venison, 266 gold, and 3,137 EXP remaining to level
16. q25 remains 8/20 and q30 remains 0/1. The initial private frame contains
vertical-slice tearing; the grind frame cleanly shows a selected 0/65 spider
corpse, and the final clean frame shows the dense blocker field plus the
reported HP, gold, empty belt, coordinate, and 92.16% EXP bar. This is real
grind throughput and a safe withdrawal checkpoint, not level-16 or recovery
closure.

r100 resumes the exact dense zero-drug withdrawal. It rotates from the first
0141 entrance after ordinary movement stops at `(302,623)`, reaches the second
visible entrance at `(311,631)`, enters the safe room, and returns at full HP.
Merchant Ruben visibly restores HP drugs `0 -> 5` for the available 200 gold.
A Deer harvest supplies no immediate inventory acknowledgement; Butcher John
then visibly changes gold `66 -> 280`, and Ruben restores drugs `5 -> 10` for
another 200 gold. One Venison remains in the final bag.

The finalized r100 report records 902,161 ms, 480 physical inputs, 0/1
completed goals, one HookingCat kill row, two visible purchases, one
fast-timeout quarantine, zero deaths/revives, potion uses, shortcuts, or
critical browser/network diagnostics. Authoritative EXP advances
`36,863 -> 37,143/40,000`, but the larger delta is not relabeled as extra
confirmed kills. After restocking, the agent restarts the real SpittingSpider
journey and reaches map-0 `(299,579)` before the runtime limit. It ends at
146/149 HP with ten drugs, one Venison, 80 gold, and 2,857 EXP remaining to
level 16; q25 stays 8/20 and q30 stays 0/1. Both private frames render
coherently and match the dense starting corridor plus final route coordinate,
HP, ten-drug belt, gold, and 92.86% EXP bar. This closes the exact zero-stock
recovery/restock replay and leaves another safe travel checkpoint; it does not
complete the grind.

r101 first attempts a farther SpittingSpider source, but a dense town-edge
attack drops the player to 12/149 HP and consumes three HP drugs. The resource
guard cools that source instead of committing to the 391-tile journey. One
normal death/revive follows; the agent sells its retained Venison for the
visible gold change `80 -> 280`, restores drugs `7 -> 10` at Ruben for 120
gold, completes a safe-room cycle, and switches to a nearer Oma source under
the existing adaptive-risk policy.

The finalized r101 report records 902,625 ms, 511 physical inputs, 1/3
successful grind goals, one Oma and one incidental Scarecrow kill row, one
death and one completed revive, three potion uses, one visible purchase, zero
quarantines, shortcuts, or critical browser/network diagnostics. Authoritative
EXP advances `37,143 -> 37,323/40,000`. It ends during the third goal's
physical Oma route at map-0 `(291,616)`, full 149/149 HP, ten HP drugs, no
Venison, 160 gold, and 2,677 EXP remaining to level 16; q25 remains 8/20 and
q30 remains 0/1. All three private frames render coherently. The goal frame
documents the real critical 12/149 pursuit rather than a clean post-kill scene;
the final frame matches the full-HP town state, ten-drug belt, gold, coordinate,
and 93.31% EXP bar. This proves adaptive risk recovery and one nearer-field
goal, not level-16 completion.

r102 resumes that exact full-health town state and completes 2/2 adaptive Oma
grind goals through ordinary field travel and combat. Two target-specific Oma
rows and two incidental Scarecrow rows advance authoritative EXP
`37,323 -> 37,713/40,000`. Between the two goals, the agent performs one normal
map-0141 safe-room cycle; the report records two visible transfer milestones
and then returns to the live field without a shortcut.

The finalized r102 report records 900,794 ms, 490 physical inputs, four kill
rows, zero deaths, potion uses, purchases, quarantines, shortcut violations,
or critical browser/network diagnostics. Its budget expires during the
ordinary return toward the equipment-repair vendor, leaving a resumable map-0
state at `(289,611)`, full 149/149 HP, ten HP drugs, no Venison, 160 gold, and
2,287 EXP remaining to level 16. q25 remains 8/20 and q30 remains 0/1. The
three private frames render coherently and match the initial town state, first
completed Oma goal, and final return coordinate. This is another bounded
progression slice, not a level-16 or q25 completion certificate.

Reports contain local account and character identifiers so that a stopped run
can resume. Keep the evidence directory private and review only sanitized
summary fields; do not attach raw `report.json` files to a PR.

## Known boundary discovered during soak

Dense live-monster fields can still cause bounded approach failures and source
rotation. The code freeze fixes the distinct corpse bug where authoritative
`hp=0` actors could retain a lagging `dead=false` render flag and incorrectly
occupy collision and target-selection grids. Unit regression coverage and the
post-fix live smoke confirm that zero-health actors are filtered, but this does
not certify every crowded-field navigation layout.

The longer soak exposed a second corpse boundary: after a target-specific death
the retained sprite could temporarily carry neither `dead=true` nor a usable HP
value, and delayed EXP let the next goal appear to progress after selecting the
same object id. `d35415c16` makes the prior death itself authoritative instead
of trusting that stale render. r45 proves the exact old ids are skipped and six
new ids are selected; historical pre-fix aggregates remain kill-row counts, not
retrospectively certified unique kills.

The soak also exposed a long-preparation efficiency defect: the planner charged
the full one-time field walk to every prospective kill, so a character with
more than 20,000 EXP remaining repeatedly chose nearby low-yield cats. Commit
`ab3582ed3` amortizes that physical trip over at most 20 expected kills while
retaining the original locality preference for short grinds. A red/green unit
test locks the exact saved-state decision, and r39 proves the resulting far
field is physically reachable and yields authoritative EXP. q25 remains 6/20;
this fix improves honest progression throughput and does not skip its level-15
combat-preparation requirement.

That higher-yield field also exposed a separate recovery defect: an already
critical, zero-potion state was passed into the ordinary combat-resource guard
as both its before and after frame, so the shelter escape could abort before
moving. Commit `4e1cdaffe` keeps the budget for sustainable travel but lets an
already depleted escape proceed until visible shelter arrival or authoritative
death/revive. Unit coverage locks healthy-stocked, zero-stock, and critical-HP
cases. r41-r43 prove the physical escape, passive recovery, shelter transfer,
merchant return, and visible potion purchase across resumable runtime slices.

The subsequent soak exposed two more recovery-state boundaries. The severe
combat strain that requests resupply was process-local, and a full-HP player
could leave the safe room before the rendered attacker chase expired. Clean
commit `4d089b4bb` (source `a71cfce46`) persists the supply recall only below
the ten-potion departure stock, admits one level-certified adjacent blocker
during an active shelter escape, and keeps the player physically pacing inside
the safe room for 20 seconds. r52 proves both settlement cycles plus the
sell-and-restock closure; r53 proves a later death/revive can reuse that path
and return to successful combat. These fixes do not advance quest counters or
grant movement, health, items, gold, or experience directly.

r54 exposed a distinct static-routing boundary: the route endpoints and wall
detour belonged to the same full-map collision component, but the largest old
search window omitted the required northern passage. Clean commit `d68674ce8`
(source `0256c33a9`) keeps the inexpensive 72/240-tile searches, then permits a
full-map fallback only when the collision atlas is at most one million cells;
larger maps receive a bounded 384-to-700-tile fallback. The synthetic wall
regression fails at the old bound and passes at the fallback, and r55/r57 prove
the previously disconnected saved state can keep moving and reach the supply
area.

The same commit also closes the r56 mixed-occupancy retry loop. Emergency
shelter escape now supplies an explicit non-funding travel accounting goal to
the existing adjacent-occupancy clearing path. It does not relax ordinary
supply hunting or add direct movement/combat commands: the existing visible
target requirement, level gate, attempt bound, quarantine, and mouse/keyboard
inputs remain authoritative. r57 proves that exact crowded resume can escape,
harvest, sell, restock, and depart without a shortcut violation.

r60-r61 exposed a later dynamic-occupancy boundary rather than a static-atlas
disconnect: the route remained connected, but the clearer could repeatedly
choose an adjacent actor with no usable physical hit surface. Clean commit
`e50a8fce0` probes the actual rendered sprite/nameplate surfaces before bounded
clearing and gives the already selected clickable object priority. It also
tracks best distance to a visible recovery portal and permits rotation only
after a bounded 45-second stall. r62-r63 prove that the patched build resumes,
restocks, returns to a real spawn field, and completes combat without a
regression. r65 later supplies the naturally occurring live hits: several
physically clickable occupancy clears and an explicit two-portal rotation.

r64-r66 then closed two state-transition errors in that same ordinary travel
stack. A shelter journey can cross from reserved to depleted after it starts;
the recovery layer now yields and re-evaluates the authoritative state instead
of treating the budget signal as fatal. An optional hostile-corridor waypoint
also uses the same retryable-unreachable classification as the outer transfer,
so failure of a risk-reducing elbow no longer proves the visible portal itself
unreachable. r65 proves continuation past the old depletion fatal, while the
r66 live trace proves ordinary portal entry, safe-room settlement, visible
harvest/sale, and full restock. r67 confirms that state persisted across a new
client bootstrap.

## Sign-off wording

Use this wording for the current milestone:

> Accepted: auditable real-client Quest Agent foundation and Warrior q1-q9
> functional slice on the PR #233 baseline. Extended Warrior and three-class
> level-1-to-50 live completion remain open and must not be represented as
> accepted.
