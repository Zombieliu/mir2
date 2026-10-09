# Taoist q89 latest output and route review

Read-only bounded review of `Taoist.2026-09-15T22-33-13-539Z.trace.jsonl`.

## Observed D2031 segment

The sampled D2031 visit begins at sequence 7201 (`2026-09-15T22:48:25.070Z`) and exits to map11 at sequence 12662 (`22:55:32.977Z`). The objective remained q89 CursedPriest 2/3; ShiZombie and CursedZombie were already 3/3.

The direct cause of the low Priest output is unsafe-pack rejection plus navigation fallback, with target movement/loss. The loaded R131 Taoist retreat profile (`protocol-survival.mjs:236, 288-291`) is `maxTargetAdjacent=0`, `maxTargetNearby=2`; this is the profile used by the runner, rather than the generic `nearby=1` default.

| seq | diagnostic risk | owner snapshot | diagnostic object: real target | neighbors within 3 tiles | fresh attacker IDs |
|---:|---|---|---|---|---|
|7462|1 adjacent / 2 nearby|HP 121/180, `(237,222)`|340704 CursedPriest HP 173/205 `(242,227)`|340109 CursedPriest d1 HP189; 341010 HungryZombie d3 HP205|341103, 341108|
|7842|1 / 2|HP 136/180, `(234,221)`|340704 CursedPriest HP167/205 `(238,223)`|340109 CursedPriest d2 HP189; 341103 CursedShaman d2 HP102|341103, 341108|
|8359|1 / 3|HP 134/180, `(243,226)`|340704 CursedPriest HP149/205 `(238,227)`|340109 CursedPriest d1 HP189; 340807 CursedZombie d3 HP181; 341010 HungryZombie d3 HP205|341103|
|8672|1 / 5|HP 107/180, `(249,227)`|340704 CursedPriest HP149/205 `(244,225)`|340109 CursedPriest d2; 342800 CursedZombie0 d2; 340807 CursedZombie d3; 341010 HungryZombie d3|340210|
|9723|3 / 3|HP 124/180, `(219,112)`|339907 **HungryZombie** HP205/205 `(221,110)` (diagnostic label says CursedPriest, so the pending target identity was stale)|339110 CursedZombie d1; 339705 ShiZombie d1; 341700 ShiZombie0 d2|339907|
|10653|1 / 1|HP 48/180, `(259,36)`|339011 **HungryZombie** HP205/205 `(258,35)` (diagnostic label says CursedPriest, also stale)|339214 CursedPriest d1 HP205|339310, 339011|

All six records are rejected by the zero-adjacent limit. The two 1/2 cases were not full-health clean pulls: owner HP was 121/180 and 136/180, and each had fresh direct attacks from 341103/341108 immediately before the diagnostic. The later cases show increasing density or critical health. This supports the adjacent-zero guard and does not by itself justify relaxing the critical proven-attacker/HP guard.

Spawn/path evidence shows the same blockage: seq 7211 starts six CursedPriest candidates with 391 waypoints; seq 7212, 7235, 7236, 7636, 7677, 7689, and 8561 report `No walk path` and fall back from clearance 4 to 1. Escape fallbacks at seq 7510, 7916, 8456, 8892, and 10137 ignored 10–15 hostile trails (successful steps 4, 6, 10, 152, and 35). The trace then records `lostCombatTarget` CursedPriest object 338605 at seq 10641 (location 250,103), and `lowHealthTargetRetreat` object 339011 at seq 10652 (HP 48/180). The associated `unsafePackPartialRetreat` at seq 10720 says `noProvenAggressorAfterPartialEscape`.

Only six SoulFireBall sends occur in this bounded segment: seq 7798, 7818, 8095, and 8277 against Priest object 340704, then seq 9669 and 9704 against object 338609. There is no Priest ObjectDied or q89 increment in the segment. Thus the low output is explained by density/adjacency safety rejection and failed safe approach, followed by retreat/target loss; it is not evidence of an Amulet-only fuel shortage. The trace file continued beyond this segment and is still active, so the parent’s separate checkpoint of Amulet 100 to 97 should be treated as its own boundary, not merged with later inventory snapshots.

## D2032 route and policy check

The Taoist generated q89 definition (`docs/generated/quest-agent/taoist-1-25-newcomer-v1.json`, quest 89) does contain CursedPriest spawn candidates in D2032 at `(150,50)`, `(250,50)`, `(150,150)`, `(250,150)`, `(50,250)`, and `(250,250)`, as well as D2031 candidates. Therefore a D2032 Priest candidate is real catalog data, not an invented fallback. This review found no evidence that the active Taoist had entered D2032.

The repository’s guarded travel fixture records the existing D2031→D2032 entrance as `D2031 (198,34) -> D2032 (117,184)` with transfer key `crystal-move:d2031:198:34:117:184:267` (`apps/web/scripts/quest-agent/test-protocol-travel.mjs:300-308`; the same key is used in the combat travel tests). This supports investigating an ordinary collision-checked doorway route. It does not prove that the current live Taoist run has traversed it.

The Wizard-only q89 protected-shaman options are explicitly class-gated in `run-protocol-journey.mjs:941-964`; Taoist has no 7–9 tile protected-shaman band or stall-clearing option. Any follow-up should therefore use the existing normal movement/transfer path and live collision checks, without inheriting Wizard clearance behavior and without debug/admin teleport.

## Evidence boundary

Trace sampled as the current file at review time: 28,620,892 bytes, SHA-256 `03C3C7E2A56F93F4A74C238E2FAFC514AC3453DA80BF747BA5A9F9F5983ADECC`. This is an active-trace cutoff, not a final run result.
