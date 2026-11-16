# Wizard q89 R128 Town-return diagnosis

Read-only trace: `C:/mir2-protocol-journey-20260911/Wizard.2026-09-15T20-59-07-113Z.trace.jsonl`, session 5352. No controller, process, save, or repository file changed.

## Fallback arm and Town ordering

q89 remained 8/9: CursedPriest 2/3, ShiZombie 3/3, CursedZombie 3/3. This trace does **not** evidence the R128 stalled-full-spread trigger. Its sole fallback arm is sequence 1059 (21:00:38.798Z), reason `focusedTargetUnsafe`, with the exact live key `crystal-move:d2031:198:34:117:184:267`.

- Seq 1050/1055: live CursedPriests 340704 and 340109 enter AOI.
- Seq 1058: the target 340704 is rejected as `unsafeTargetCluster` (adjacent 1, nearby 1).
- Seq 1059: the D2031 -> D2032 fallback arms.

The focused-unsafe branch arms then immediately calls `retreatAndRecover`; D2032 selection cannot happen until that recovery returns to the outer objective loop. It did not return before a critical escape:

- Seq 1629 (21:01:19.805Z): `navigationEmergencyEscapeAttempt`, HP ratio 0.19 at D2031 (261,231), one proven nearby attacker.
- Seq 1630: public `UseItem`, inventory unique ID 21.
- Seq 1664/1665: authoritative map 0 / UserLocation (288,616).
- Seq 1706: `navigationEmergencyEscapeSuccess` confirms the same D2031-to-Town relocation.

There is no D2032 chosen-destination diagnostic, keyed travel packet, D2032 MapInformation, or `objectiveMapFallbackEntered` before Town. This was a critical HP emergency escape, not an R128 fallback failure or Town supply trigger.

The item identity is proved independently of the generic `navigationEmergencyEscape` diagnostic. The pre-command seq 1627 full snapshot contains inventory unique ID 21 as `TownTeleport` x1 (and another TownTeleport unique ID 1 on the belt); its RandomTeleport total is eight across unique IDs 11, 12, 13, and 3. Seq 1630 sends `UseItem { grid: Inventory, uniqueId: 21 }`. The successful seq 1663 response names the same unique ID. The first post-arrival snapshot (seq 1705) retains one TownTeleport total and eight RandomTeleports. Thus the escape consumed the inventory TownTeleport, reducing TownTeleport 2 -> 1; it did not consume RandomTeleport.

## CursedShaman 340210 attribution

The actor is not inferred from an old corpse. It was newly rendered as `ObjectMonster` 340210 at seq 1036 (21:00:36.534Z), position (253,232). The first observed attack/hit arrives at seq 1078/1087, before its seq 1091 `ObjectHealth` 100% receipt, so that 100% receipt alone does not prove it respawned before the *first* damage.

It does prove a positive-HP current actor before the later sustained exchange: post-seq1091 it sends repeated range attacks and direct player strikes, including range attack seq 1145 and self strike seq 1165, continuing through range attack seq 1615 and self strike seq 1620. These packets, rather than stale AOI state, support the statement that a live 340210 contributed to the continuing unsafe recovery. The trace does not need a claim about the prior incarnation's death.

## Does the arm survive a resume?

Within one `completeQuestObjectives` invocation, the arm survives ordinary recovery. If that recovery returned on map 0, the next loop would first travel normally to D2031, refresh/revalidate the exact source/key, then take the D2032 hop.

It is intentionally not reconnect/new-call state: `objectiveMapFallbackState` is a local variable and the source says reconnects must prove the live transfer again. The trace has no subsequent arm after the Town transition, so it cannot show that the original in-memory arm reached the return-to-source plan.

## Fresh full-inventory reserve proof

The fresh map-0 world snapshot at seq 1796 (21:01:39Z), after the shop transaction, contains all `inventoryItems`, `beltItems`, and `equipmentItems`. Deduplicating by `uniqueId` across those containers yields:

| Item | Count |
|---|---:|
| (HP)DrugSmall | 59 |
| (MP)DrugSmall | 5 |
| RandomTeleport | 8 |
| TownTeleport | 1 |
| (HP)DrugMedium | 0 |

Gold is 9. This correctly preserves the observed seq 1784 one-Small purchase and seq 1790 Gold 49 -> 9. Ruben's same live `NPCGoods` packet (seq 1782) prices Small at 40, Medium at 110, and RandomTeleport at 100; the rendered TownTeleport item price is 1,000.

The generic 64-total-HP floor is short by five; MP (5 >= 4) and RandomTeleport (8 >= 8) already meet their targets. The q89 Wizard **Town-departure-only** policy separately asks for six Medium and two TownTeleports. Medium is not a field invariant: away from Town, missing or unaffordable Medium falls back to held Small plus normal retreat policy. At this Town snapshot, reaching the departure preference needs six Medium (660 Gold) and one TownTeleport (1,000 Gold). Those six Medium also raise total HP drugs to 65, so no additional Small is needed for the 64 floor. The proven departure top-up is therefore 1,660 Gold; Gold 9 is short by 1,651. This is not a claim that 1,660 is needed to continue safe field fallback with existing Small stock.

## Current normal funding receipts

The latest fresh trace snapshot at seq 3415 (21:06:59Z) is map 0, HP 100/100, MP 140/398, Gold 553; q89 is still 8/9. Funding is making verified positive progress rather than looping without receipts:

- Eight accepted public kills have `ObjectDied` receipts: 206302 (1808), 206515 (1894), 206324 (1995), 206530 (2199), 206524 (2487), 206320 (2719), 206545 (2883), and 206700 (3004).
- Fresh inventory snapshots show Venison rising 0 -> 8. Six `SellItem` receipts at 3263, 3269, 3274, 3281, 3286, and 3294 reduce it 8 -> 2 while Gold rises 9 -> 1,413: +1,404 accepted sale revenue. Two Venison remain at seq 3415.
- The current public shop spends are receipt-backed: seq 3405 buys five Small at the live 40 price (-200), and seq 3411 buys six Medium at the live 110 price (-660). Latest inventory is Small64, Medium6, MP Small5, Random8, Town1, Venison2, Gold553.

Net from the observed funding-sale/purchase segment is +544 Gold (9 -> 553). The six Medium and 64 total HP now meet those portions of the q89 Town departure preference; the remaining known departure shortage is one TownTeleport (1,000 Gold). No quest objective increment has occurred during this normal funding segment.

## Conclusion

The immediate blocker is unsafe-target recovery exposure: the arm occurs before recovery and the critical escape happens before the next D2032 planning iteration. Preserve the exact source/key and six-tile safety rules. Any correction must make the armed fallback reachable without re-entering this unsafe-pull recovery path; this trace does not support catalog-only D2032 claims or a weaker fallback guard.
