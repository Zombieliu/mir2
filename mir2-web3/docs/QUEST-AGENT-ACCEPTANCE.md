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
  `d8264b5c1d80f150e29462dec47380995cc93185`.
- Patch-equivalent source freeze for the original transplant:
  `ccaba013515b0f1908e9c3aa6fca6a5c847db1f8`.
- Latest live-soak Quest Agent runtime revision:
  `acd95a4b40a04f1d0496d38146f83289f2513d7e` (patch-equivalent to clean
  commit `d8264b5c1`).
- Remote follow-up branch: `origin/codex/quest-agent-recovery-followup`.
- Integration lineage: PR #235's reviewed range is squash-merged as
  `aa928b99a`. This follow-up adds four code/test commits plus their acceptance
  documentation commits; source
  `0256c33a9`/`0afc30449`/`1c9ac3b4`/`acd95a4b4` and clean
  `d68674ce8`/`e50a8fce0`/`030cebe3`/`d8264b5c1` have matching stable patch
  ids respectively.

## Acceptance matrix

| Surface | Status | Evidence boundary |
| --- | --- | --- |
| Physical input and read-only observation contract | PASS | CDP mouse/keyboard/text only; static and runtime shortcut audits report zero violations. |
| Resume, reconnect, death, potion, merchant, navigation, combat, harvest, and equipment framework | PASS for staged use | Exercised across the local development soak; failures remain explicit and resumable. |
| Warrior q1-q9 functional route | PASS | One finalized certificate reached all required authoritative stages with 684 inputs, 18 kills, no death, and zero shortcut violations. |
| Current extended Warrior chain | PARTIAL | The resumed character has reached level 15. q22-q24, q28, and q29 are complete; q25 is 6/20, q26/q27 are not yet unlocked in the current snapshot, and q30 is 0/1. |
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

The later Quest Agent-only follow-ups through `d8264b5c1` change JavaScript and
its unit tests. The complete Quest Agent gate now passes 176/176, including the
long-preparation travel, depleted-shelter recovery, confirmed-corpse lifecycle,
cross-run supply recall, safe-room settlement, full-map route fallback, and
dense-shelter escape plus congested-portal rotation and physical-hit-target
selection, en-route reserve exhaustion, and optional hazard-waypoint
regressions. It also covers budget-disabled equipment-repair travel retaining
its explicit non-funding resource accounting while independently enabling
certified physical occupancy clearing; Node syntax checks and `git diff
--check` pass in both source and clean worktrees. The earlier full
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

The private evidence directory contains 70 finalized development reports
through `warrior-q30-r71-supervised` (r66 was an intentionally stopped live
trace and is excluded from these report aggregates):

- 61,864,668 ms (17 h 11 m 5 s) browser-active runtime;
- 32,510 recorded physical inputs;
- 330 historical kill rows, including one r44 row now proven to repeat the
  same target object id rather than represent another kill;
- 13 deaths and 12 completed revives across intentionally interrupted and
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
