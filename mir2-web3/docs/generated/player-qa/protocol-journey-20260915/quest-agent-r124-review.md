# Quest-agent R124 read-only review

Reviewed corrected R124 source and post-fix retained logs. No source, live, process, store, commit, or UI mutations were made.

The prior review’s three root findings were real and are now addressed in the corrected delivery:

- The runner previously lacked `spawnStallProtectedBlocker` wiring. `run-protocol-journey.mjs:910-918` now passes the policy only for q89 Wizard: `CursedShaman`/`CursedShaman0`, approach range 7–9, clearance 6, and `maxBlockers: 2`; other quests/classes receive `null`.
- The earlier D2031 test previously teleported the owner with `Object.assign`. The corrected test now constructs `createNavigator` with `loadProtocolCollisionMap`, dispatches ordinary walk/run commands, records every travelled cell, and asserts every physical cell is more than six Chebyshev tiles from both Shamans. It also asserts the approach action is within 7–9 tiles of the selected blocker and beyond six tiles from the other Shaman; no direct owner-position assignment remains in this test block.
- The earlier blocker-clear path could accept an apparent completion without proving the optional blocker died. `protocol-combat.mjs:1414-1438` now records an attempt boundary, requires the player to remain on the original map, and accepts clearance only when the blocker is dead or HP <= 0 in the current snapshot, or a fresh `ObjectDied` receipt for that blocker arrives after the attempt boundary. Otherwise it throws `LostCombatTarget`; map changes remain fatal.

The corrected retained logs match the supplied hashes and counts:

- `q89-spawn-stall-r124-focused.log`: **190 tests, 190 passed, 0 failed**, SHA-256 `FC4FE71DF02A51B1008D90C68E39EAD573D1BE850DBB00088FB6373F7FB99999`.
- `q89-spawn-stall-r124-full.log`: **645 tests, 645 passed, 0 failed**, SHA-256 `83A3D334E02E10BAED267A05143A763E095C41578FDAD488145186BF49C258FF`.

The source and tests now provide the requested bounded max-two blocker behavior, named six-tile Shaman clearance, no target exemption, collision-checked travelled-path evidence, same-map/life safety, and strict death confirmation. These are deterministic focused/full test results; they do not constitute a live gateway journey or formal production PASS.

Status: **R124 corrected source/test review passes the requested static and retained-log checks.**
