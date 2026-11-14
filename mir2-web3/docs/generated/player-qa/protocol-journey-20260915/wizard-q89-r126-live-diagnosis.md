# Wizard q89 R126 live diagnosis

Trace inspected: `Wizard.2026-09-15T20-39-33-006Z.trace.jsonl`, sequences 1–3222 (through 2026-09-15 20:43:02Z). This was read-only analysis; no controller, process, store, or repository file was changed.

## Result

q89 remains **8/9**: `CursedPriest 2/3`, `ShiZombie 3/3`, and `CursedZombie 3/3`. The run did not enter `D2032`, so it provides no live evidence that a remaining Priest exists there. The imported q89 route does contain seven `D2032` CursedPriest spawn groups, but that is catalog availability, not a live-AOI observation.

The current failure is not an R126 protected-transit guard. The controller ran four D2031 CursedPriest searches, repeatedly enumerated disconnected waypoint candidates without dispatching normal movement, waited for respawn three times, and then physically left through D2031's *other* live exit to map `11` (WoomyonWoods(N)). It never selected either verified D2031 -> D2032 transfer.

## Evidence

- Snapshot sequence 748 at 20:39:52Z: q89 is 8/9; Priest is 2/3. The same state is still present in snapshots 2667, 2707, and 2738.
- Successful normal Shaman clearance is proven by fresh `ObjectDied`: 341100 at seq 745, 341109 at 2031, and 340210 at 2124. The search diagnostics match the last two clears and the first clear; these are corridor clears, not quest progress.
- D2031 CursedPriest searches start at seqs 555, 763, 1250, and 1794. Each has 391 waypoints; 1,165 `spawnSearchClearanceFallback` diagnostics were emitted. Representative diagnostics say `No walk path on D2031 from 249,254` even after the clearance fallback is reduced from four to one tile. This accounts for the rapid static `spawnSearchProgress` output: failed path planning is being counted as inspected waypoints, not physical coverage.
- Three bounded respawn waits occurred at seqs 1213 (20:40:04Z), 1686 (20:40:42Z), and 2632 (20:41:57Z), all on D2031 for CursedPriest. The q89 profile has an eight-wait limit, so no full-search exhaustion fallback was reached before leaving the map.
- Two live CursedPriests appeared in AOI at seq 2075 (`340704`, 238,226) and seq 2115 (`340109`, 237,224). They had adjacent/nearby hostile companions: CursedZombie/CursedZombie0, ShiZombie, and a CursedShaman. There is no Priest magic/attack or Priest death packet; both Priests leave AOI at seqs 2416 and 2422. Their non-selection is consistent with the existing q89 0-adjacent/2-nearby safety gate, rather than a wrong-type attack.
- The only live D2031 transfers in snapshots 748/2667/2707 are `279,285` and `280,284` -> map `11`, plus `198,34`/`198,35` -> D2032. At seqs 2668–2726 the player runs from 254,266 to 279,285; `MapInformation` seq 2729 proves landing on map `11` at 39,314. No D2032 `MapInformation`, D2032 world snapshot, `chosenObjectiveDestination`, `objectiveMapFallback*`, or protected-transit diagnostic occurs.

## Code-path conclusion

`completeQuestObjectives` chooses D2031 whenever `selectTargetPlan` finds its current-map CursedPriest candidates. q89 Wizard has no unconditional preferred objective map: `shouldPreferObjectiveMapOverCurrent(89, ...)` is false and `preferredObjectiveMapsForQuest(89, ...)` is empty. The D2032 fallback is armed only by unsafe/lost focused-combat paths in `armObjectiveMapFallback`; a static no-target `SpawnSearchExhausted` and its respawn-wait branch do not arm it. Consequently this run can spend all normal D2031 respawn waits on collision-disconnected coverage without attempting the proven D2032 doorway.

## Minimal correction direction

Keep the six-tile Shaman halo, receipt proof, and R126 transit guard unchanged. For q89 Wizard only, after one completed no-physical-progress D2031 full-spread pass (or a clearly bounded equivalent summary of its disconnected-waypoint outcomes), arm the existing exact-live D2031 -> D2032 fallback before another respawn wait. Require the same current-map/source/key proof already implemented; if its 198,34/198,35 doorway is absent or clearance cannot safely resolve, use the existing defer/recovery path. Do not treat D2032 catalog groups as live targets until authoritative arrival and AOI confirm them.
