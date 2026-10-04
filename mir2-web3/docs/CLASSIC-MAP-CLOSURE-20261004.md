# Classic map closure, phase 1: all 164 admitted maps

> 2026-10-04 phase2 is now integrated:44 additional reachable whitelist maps
> bring runtime scope to208; the original unreachable TacticalMaze D71653 is
> resource-only, making209 native maps. EvilSnake/WhiteBoar0 enter the profile
> and missing Monster049 is exported/verified. Actual coverage is67,993 drawable
> references with zero gaps; Node64/Rust12 worker checks and root integrated
> consumer checks pass. Full source and raw failures remain recorded.
> This does not grant the unbound NeedMove D10061 entry or claim ordinary
> Stone quest predicates/Boss loot/native acceptance. A bounded next route
> round handles named legacy NPC compatibility and preserves that source gap.
> [Phase2 report](CLASSIC-MAP-CLOSURE-PHASE2-20261004.md),
> [Root integration](generated/player-qa/classic-map-closure-20261004/phase2-208-209/README.md).

The Windows Candidate default now derives its native map list from the actual
`platinum_176` profile instead of the former 32-map merchant/Sabuk list. It
retains those 32 maps and the existing 15-map journey/supply minimum. Explicit
custom lists remain explicit: selecting the 15-map legacy set still selects
exactly 15 maps.

This is the resource stage for the already admitted world. It does not finish
the complete classic-gameplay Goal. The next separately owned stage still
needs the 45 excluded Stone Tomb/Zuma/Red Moon maps, Monster/049 and the original
entry-policy decision for D71653. No profile, bundle, actor asset, runtime or
live Gateway change is part of this phase.

## What changed

`candidate-release-profile.ps1` reads the profile identity, version and file
SHA, derives a safe, unique, ordinal-sorted map set, and requires every
previously shipped merchant and Sabuk map. Its scope receipt identifies either
`profile` or `explicit` selection and hashes the exact expected map list.

`audit-candidate-map-coverage.mjs` compares the native manifest against that
expected set before completing any floor atlas. An omitted classical branch
or unexpected map causes a failed receipt; checking only the old journey
minimum can no longer produce a full-profile success. The package script
passes its selected mode and expected maps through both audit stages.

The final PowerShell guard checks the scope against the current profile, both
resource manifest hashes, and the actual native manifest map list. Updating a
native manifest hash while retaining a false 164-name report also fails.
Existing PNG geometry/hash checks, one-pixel atlas gutters, pixel budgets,
source-missing budget, path/reparse/ADS guards and signing gates remain in
place. Original empty and out-of-range source slots retain their separate
classification.

Static topology reports conditional NPC/forced-move entries separately, keeps
the excluded 45-map list visible, and records D71653 as an original room with
no imported entry. `completeClassicWorld` and `runtimeGameplayAccepted` are
explicitly false. The profile stage rejects unreachable admitted maps; a
future 209-map stage must resolve the intentional-orphan policy rather than
silently inventing an entrance or claiming acceptance from a file count.

Old journey-only coverage receipts without a scope receipt are deliberately
rejected by the new guard. Preserve the signed R16/R17 packages with their
original verifier and evidence. A future package must regenerate and sign its
new scoped report; do not modify an already signed package in place.

## Focused verification

On the isolated `codex/classic-map-closure-20261004` worktree based on clean
`6032ef8b3e27dd97bad0b20c8676ef9f185db64b`:

- New Node scope/topology/CLI fixtures passed 8/8. They cover an omitted Red
  Moon side branch (`d10054`) despite retaining all 15 legacy maps; exact
  explicit selection; unsafe, duplicate and wrong-profile identities; early
  rejection before atlas completion; conditional entries; and the source
  orphan boundary without changing the real profile.
- PowerShell configuration/scope fixtures passed. They reject mismatched
  profile ID/version/SHA/count, wrong map-list SHA, missing scope receipts,
  string/false acceptance claims, omitted/unexpected maps, a scope-mode swap,
  and a report/native-manifest map-list mismatch. The explicit 15-map fixture
  remains unchanged.
- Native keyed-pack, map routing, atlas gutter/budget, quest route and new scope
  tests passed 60/60. The first run exposed an existing route-test expectation
  of profile version 25; the unchanged 6032 baseline failed identically because
  the real profile is already version 26. Only that obsolete assertion was
  updated to 26; no profile data was changed.
- Node and PowerShell produce the same scope receipt and expected-map SHA.
- Physical dependency/PublicRoot cache copies used the locked R17 source.
  `git diff --exit-code -- apps/web/public` and the tracked-public status check
  remained empty after copying. There are no symlinks or hard links added by
  this work.

Commands, from the project root:

```powershell
node --test apps/game-client/platform-windows/scripts/test-candidate-map-coverage.mjs
powershell -NoProfile -File apps/game-client/platform-windows/scripts/test-candidate-map-scope.ps1 -EvidenceRoot <external-evidence-root>
node --test apps/web/scripts/test-native-keyed-map-pack.mjs apps/web/scripts/test-map-render-routing.mjs apps/web/scripts/test-map-atlas-gutters.mjs apps/web/scripts/test-map-atlas-budget.mjs apps/web/scripts/quest-agent/test-route-manifest.mjs apps/game-client/platform-windows/scripts/test-candidate-map-coverage.mjs
```

## Full 164-map generation and audit

All generated files and test evidence are outside the checkout, under
`C:/mir2-playtest-releases/20261004-native-r17/map-implementation/`. Independent
Node children used this approved directory as their process-local OS temp;
the existing dedicated-prefix, fresh-directory and reparse guards were not
changed. No Windows EXE was rebuilt, signed, installed, uploaded or published.

Using the locked original full pack with index SHA
`77a9050818df502f65a7b151976ee7d66bd01e94d87680cf0760b925dd10dbee`:

| Result | Measured value |
|---|---:|
| Expected and generated maps | 164 |
| Keyed/additive entries | 52,211 |
| Native PNG image bytes | 249,747,060 |
| Original-source fallback entries | 41,447 |
| Newly completed ordinary floor frames | 11,900 |
| Completed atlas pages / source frames | 535 / 16,623 |
| Completed atlas image bytes | 146,617,283 |
| Max atlas page pixels | 262,144 |
| Unique map references audited | 69,386 |
| Drawable references covered | 67,554 |
| Missing drawable / omitted / unexpected maps | 0 / 0 / 0 |
| Original no-draw / out-of-range references | 549 / 1,283 |
| Conditional-entry maps | 16 |
| Admitted maps unreachable in static topology | 0 |

The native generator retained its original `missingSourceCount=1177` and
`noDrawReferenceCount=530` counters; the independent complete-layer audit
classifies every referenced source slot and reports zero missing drawable
frames. Its larger no-draw/out-of-range totals include ordinary floors and
all Type100 animation phases, so these counters must not be treated as
equivalent or suppressed.

Generation took 312.5 seconds, floor completion 303.7 seconds, and the final
audit 6.0 seconds. The actual PowerShell native-closure and coverage guards
also passed against these generated files. These are resource generation
costs, not client load time or live gameplay measurements.

Receipt identities:

- Profile v26 SHA:
  `328ecec1474f9bde809b6a57508cb9dd9d3b0be82c136fb4dabcd8bba85b66cd`.
- Expected-map canonical SHA:
  `a547742ebf4894847f63c4e0259d82e23ab8e4392c0099c33587d7d7fac58413`.
- Native manifest SHA:
  `cabe0b8789be7b9fe8791c1abbe7361807fa7d4654e9b3d8dea6ad149072ce74`.
- Completed atlas manifest SHA:
  `4d21069b64cd6c6cbb67c9aa6fec1387944bd4cd73e7c96f9467ea6c0821d511`.

The 16 conditional maps include Great Tao Tomb (`needMove`), the Ancient Stone
Tomb chain (StoneHeart NPC), Prajna travel/dungeons and WhiteVillage (paid
Sailor). Original NPC quest/level/item/gold predicates still require real
Gateway checks; a static path does not prove those predicates or collision
are playable. D71653 remains excluded and is recorded with no imported entry.

## Remaining gates

Root owns integration, global progress documents and release. Before
publication, preserve the reviewed source and signing/provenance gates, stage
the exact 164-map resources, measure actual package/update deltas, and run
ordinary three-class Gateway travel with screenshots, combat, drops, saving
and reconnects. Test repeated map handoff and bounded image/layout retention;
do not treat all-content image bytes as resident GPU memory.

Then separately implement the 45-map/profile/Monster-049 stage and decide the
original orphan-room intent. No complete-world, later-Mir3, load-capacity or
human visual-acceptance claim is made by this phase.
