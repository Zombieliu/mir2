# Wizard R124 q89 loop diagnosis

Read-only review of `Wizard.2026-09-15T19-37-28-605Z.trace.jsonl` through sequence 14976 (`2026-09-15T20:12:42.619Z`). q89 remained Priest 2/3, ShiZombie 3/3, CursedZombie 3/3.

## What caused the Town return

This was not an MP exhaustion, a `maxBlockers: 2` exhaustion, or an unconfirmed blocker kill.

- The first D2031 pass cleared CursedShamans 341105 and 341100; the later pass also cleared 341109. The clear snapshots show the targets at `dead:true, hp:0` (for example 341109 at sequence 8954), so the five recorded clears are not a missing-death loop.
- The one deferred blocker was 340210 at `(253,232)`. At sequence 9024 the Wizard was `(245,241)`, range 9, HP 100 and MP 130. It later moved to `(247,238)`, range 6, while the local zombie pack remained. The band guard correctly refused further action at sequence 9117, and sequence 9118 recorded: `target 340210 has no collision-safe Wizard ranged band on D2031`.
- The outstanding CursedPriest candidate 340800 was rejected at sequence 9600 as `adjacent:1, nearby:5`. The D2032 fallback was armed, but before it could transfer the player reached sequence 9721 at `(234,217)` with HP ratio `0.22`, `nearby:4`, and `provenNearby:2`. The authoritative emergency Town action succeeded at sequence 9803. This is the exact Town-return trigger.
- The following town delay was a supply-funding gate. At sequence 11323 the controller logged `needsFunds: unable to restock supplies to TownTeleport 2`; gold was 607. It hunted/sold until gold was 1098, bought item 719 (TownTeleport) for 1000 at sequences 11464/11468/11469, then selected D2031 again at 11471. MP was not the gate.

## Repeating entrance failure

The final re-entry shows the narrow defect clearly. The D2032 objective choice was made at sequence 14666 while the Wizard was at D2031 `(278,284)`, with protected CursedShamans 341105 `(275,270)` and 341100 `(263,273)`.

| Sequence | Position | Relationship to 341105 | Result |
| --- | --- | --- | --- |
| 14674 | `(278,279)` | Chebyshev distance 9; distance 15 from 341100 | legal ranged band, but transit continued without clearing |
| 14676 | `(278,277)` | distance 7 | still outside the hard six-tile halo |
| 14680 | `(276,275)` | distance 5 | ordinary transition path crossed the protected halo |
| 14762 | `(268,268)` | distance 7 from 341105 but distance 5 from 341100 | both breakout attempts correctly rejected as no all-Shaman-safe band |
| 14798/14799 | `(267,268)` to `(274,263)` | retreat after stalled exposure | no unsafe close-range cast recorded |

The live path therefore establishes that a safe band is geometrically encountered before the unsafe crossing. It does **not** establish an all-tile collision certificate for a combat approach beyond that point. The current D2031-to-D2032 transit does not invoke the protected-blocker resolver before crossing the first visible Shaman halo; it only tries breakout after exposure has already made both protected targets unavailable.

## Minimal corrective strategy

Keep the six-tile halo, the 7–9 firing band, critical two-proven-attacker Town escape, and the existing blocker bound. Do not increase budgets or weaken the density rule.

For q89 Wizard only, before an ordinary D2031 transition step would cross any visible CursedShaman/CursedShaman0 six-tile footprint:

1. Stop at the first collision-confirmed 7–9 band waypoint (the trace reaches `(278,279)` for 341105).
2. Invoke the existing protected-blocker action only when it can certify every approach step remains outside **both** visible Shaman halos. Its normal receipts, potions, death proof, and defer/recovery behavior remain authoritative.
3. If no certificate exists, defer/recover before the crossing. Do not continue the transition into the footprint and do not cast from inside it.
4. Replan the D2032 transfer only after that certified clearance or ordinary safe recovery.

This addresses the observed loop: it gives the resolver a reachable opportunity before `(276,275)` crosses the halo, while retaining the guard that correctly rejected 340210 and the later overlapping pair. The trace does not support killing 340210 or raising `maxBlockers` as a remedy for the Priest search.
