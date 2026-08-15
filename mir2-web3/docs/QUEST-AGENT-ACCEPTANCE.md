# Quest Agent staged acceptance — 2026-08-15

## Decision requested

Accept the real-client Quest Agent foundation and the Warrior q1-q9 functional
slice. Do **not** accept this branch as a completed Warrior, Wizard, or Taoist
level-1-to-50 playthrough.

## Immutable anchors

- Source review base: PR #233 exact head
  `ef33e4764c1342620e30532fb5ffa4e784013dd2`.
- Main integration base: `f0e0bb6cdc7129fc3b183e7875603d198198bb75`
  (current `main`, including PR #234).
- Main-based acceptance code freeze:
  `4e1cdaffe52eb0a5ca6e74e9fe257a6d53ce1e55`.
- Patch-equivalent source freeze for the original transplant:
  `ccaba013515b0f1908e9c3aa6fca6a5c847db1f8`.
- Latest live-soak Quest Agent runtime revision:
  `85291de07e0758f16601468558acd4d3a1c7c0b2` (patch-equivalent to clean
  commit `4e1cdaffe`).
- Remote clean branch: `origin/codex/autonomous-quest-agent-main`.
- Code range: ten code/test commits plus three acceptance-documentation commits
  on top of the main integration base. The original eight transplanted source
  commits remain one-for-one matches under `git range-diff`; the later scene
  cache regression, grind-travel fix, and depleted-shelter recovery fix are
  clean-branch follow-ups.

## Acceptance matrix

| Surface | Status | Evidence boundary |
| --- | --- | --- |
| Physical input and read-only observation contract | PASS | CDP mouse/keyboard/text only; static and runtime shortcut audits report zero violations. |
| Resume, reconnect, death, potion, merchant, navigation, combat, harvest, and equipment framework | PASS for staged use | Exercised across the local development soak; failures remain explicit and resumable. |
| Warrior q1-q9 functional route | PASS | One finalized certificate reached all required authoritative stages with 684 inputs, 18 kills, no death, and zero shortcut violations. |
| Current extended Warrior chain | PARTIAL | q22-q24, q28, and q29 are complete; q25 is 6/20, q26/q27 are not yet unlocked in the current snapshot, and q30 is 0/1. |
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

The current `ab3582ed3` and `4e1cdaffe` follow-ups change only Quest Agent
JavaScript and its unit tests. The complete Quest Agent gate now passes
165/165, including the long-preparation travel and depleted-shelter recovery
regressions; Node syntax checks and `git diff --check` pass. The earlier full
Simulation/Gateway/TypeScript results remain the backend baseline rather than
being relabeled as a fresh run for these Agent-only changes.

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

The private evidence directory contains 45 finalized development reports
through `warrior-q30-r45-supervised`:

- 39,401,509 ms (10 h 56 m 41 s) browser-active runtime;
- 20,572 recorded physical inputs;
- 257 historical kill rows, including one r44 row now proven to repeat the
  same target object id rather than represent another kill;
- 10 deaths and 9 completed revives across intentionally interrupted and
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

## Sign-off wording

Use this wording for the current milestone:

> Accepted: auditable real-client Quest Agent foundation and Warrior q1-q9
> functional slice on the PR #233 baseline. Extended Warrior and three-class
> level-1-to-50 live completion remain open and must not be represented as
> accepted.
