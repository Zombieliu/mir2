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
  `8423d124bf6d52d02f0c129de38227b6cfe863ff`.
- Patch-equivalent source freeze:
  `ccaba013515b0f1908e9c3aa6fca6a5c847db1f8`.
- Latest live-soak runtime revision:
  `d8bca1792708baff979460fc1f98ca1dd4bd9ddf`.
- Remote clean branch: `origin/codex/autonomous-quest-agent-main`.
- Code range: seven code/test commits plus acceptance documentation on top of
  the main integration base. `git range-diff` matches all eight transplanted
  source commits exactly; documentation-only commits may follow the code freeze.

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

Full-package gates pass on the patch-equivalent source freeze:

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

The private evidence directory contains 37 finalized development reports
through `warrior-q30-r37-supervised`:

- 35,627,291 ms (9 h 53 m 47 s) browser-active runtime;
- 18,555 recorded physical inputs;
- 241 recorded kills;
- 10 deaths and 9 completed revives across intentionally interrupted and
  diagnostic runs;
- zero shortcut violations.

The aggregate spans multiple code revisions and is endurance evidence, not a
single passing certificate. The latest finalized r37 segment is the post-fix
clean resume/recovery smoke: 489,624 ms, 9 attempted goals, 5 successful goals,
5 kills, 283 inputs, experience advanced from 7,989 to 8,541, no death, no
shortcut violation, and no critical browser or network diagnostic.

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

## Sign-off wording

Use this wording for the current milestone:

> Accepted: auditable real-client Quest Agent foundation and Warrior q1-q9
> functional slice on the PR #233 baseline. Extended Warrior and three-class
> level-1-to-50 live completion remain open and must not be represented as
> accepted.
