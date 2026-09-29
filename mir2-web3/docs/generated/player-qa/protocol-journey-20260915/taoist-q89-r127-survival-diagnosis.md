# Taoist R127 q89 survival diagnosis

Trace: `C:\mir2-protocol-journey-20260911\Taoist.2026-09-15T20-40-27-269Z.trace.jsonl` (bounded read-only parse; sanitized proof in `taoist-q89-r127-survival-proof.json`). The trace ends at sequence 15498 (`21:07:23.979Z`).

## Verified lifecycle

There is exactly one player `Death` packet in the trace, at seq 14092 (`21:04:30.628Z`, D2031 `(212,92)`), followed by the sent `townRevive` at seq 14098 (`21:04:30.777Z`), then received `Revived` at seq 14130 (`21:04:32.069Z`) with `ObjectRevived` and 100% `ObjectHealth` at seqs 14131-14132. `ObjectDied` seq 14096 is the same player death. Other `ObjectDied` packets (9881 and 14396) belong to monsters, not the player. Thus the trace does not show three player deaths/revivals.

The earlier seq 13973 snapshot was D2031 `(212,92)`, HP 51/180, MP 58/184, alive, with 20 HP-small, 2 MP-small, and 77 Amulet. At the death-state snapshot seq 14100, authoritative HP was 0 and the player was dead; MP was 74, with 19 HP-small, 1 MP-small, and 75 Amulet. This is an explicit packet-proven death, not an inference from a rounded 0% health update.

## Damage and actions

The player remained at `(212,92)` while the close pack hit repeatedly. The final damage burst was:

- seq 14015-14021, `21:04:27.459Z`: ShiZombie `338505` at `(211,92)` for 4, HungryZombie `339001` at `(212,93)` for 8, and HungryZombie `339906` at `(211,91)` for 7;
- seq 14028-14029, `21:04:27.707Z`: CursedZombie `339106` at `(212,91)` for 7;
- seq 14068-14074, `21:04:29.688Z`: the same ShiZombie/HungryZombie/HungryZombie cluster for 7, 11, and 7;
- seq 14093-14094, `21:04:30.628Z`: CursedZombie `339106` at `(212,91)` for the lethal 12.

Healing was sent at seqs 13976, 14010, and 14056; corresponding received `ObjectMagic Healing` acknowledgements were seqs 13991, 14026, and 14065. The snapshot after the second healing was HP 7/180 (seq 14027), and after the third it was HP 19/180 (seq 14066). SoulFireBall continued at seq 14033 while HP 7 and seq 14078 while HP 12, targeting HungryZombie `339001`; it did not create separation before the next incoming burst. No summoned pet/intelligent-creature entity was present in any inspected snapshot (`stage5Systems.intelligentCreatures` was empty and no pet/hero/summon entity was observed).

## Result and boundary

The immediate failure mechanism is close-range multi-attacker overlap at the D2031 north approach while the player was stationary at `(212,92)`. Healing acknowledgements landed, but the incoming 4+8+7+7 and then 7+11+7 bursts outpaced the observed HP recovery; the subsequent ranged casts still occurred inside the cluster. The trace does not support Amulet or MP exhaustion as the cause: Amulet remained 75 and MP 74 at death.

After revival, the player was back in map `0` at seq 14182 and ordinary restock completed: seq 14237 had 80 HP-small, seq 14241 had 12 MP-small, and seq 14248/14253 had 100 Amulet; seq 14253 showed HP 180/180, MP 184/184 and gold 14313. Navigation then resumed. This is evidence for one real death/recovery cycle and a stationary pack-overlap failure, not repeated revival-loop evidence in this trace.
