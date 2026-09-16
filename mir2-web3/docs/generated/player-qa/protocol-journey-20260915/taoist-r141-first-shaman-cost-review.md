# Taoist R141 first-Shaman cost review

Bounded read-only trace: `C:/mir2-protocol-journey-20260911/Taoist.2026-09-16T00-28-55-830Z.trace.jsonl`. The first authoritative death receipt for Shaman object `340211` is seq1896 at `2026-09-16T00:31:13.201Z`; the provided run's external numbering refers to the same bounded fight. No credentials or private account data are included.

- First bounded owner snapshot seq707: HP129/180, MP94/184, MP drugs10. The first fresh snapshot at seq1889, immediately before ObjectDied, is HP103/180, MP64/184, MP drugs5. The observed item quantities in this trace show the five MP consumptions; Amulet item rows are split across stacks and should not be inferred from one row.
- SoulFireBall sent: 34. Healing sent: 28. ObjectDied for 340211: 1. The fight ran from the first target engagement around `00:29:27` to death at `00:31:13`.
- MP potion uses for UID172: seq1083, 1244, 1446, 1630, 1771; all five have successful UseItem ACKs. Fresh MP before each was respectively 55, 53, 48, 52, 54 out of 184 (26.1–29.9%), so these were not premature uses while MP was above the 30% threshold. The latest fresh MP after the final probe was 64/184 with five MP drugs remaining.
- SoulFireBall Magic/ObjectMagic acknowledgements are present. Direct target-health responses commonly follow in about 0.43–0.78 seconds; longer 4–7 second gaps occur when the next target-health packet is not the immediate response to that cast and should not be treated as cast latency. There is no evidence here of a missing ACK or a spell loop that failed to spend MP.
- HP fell as low as 80 during the fight and recovered through Healing; final pre-death snapshot was 103 HP. This is an expensive, sustained ordinary fight (34 SFB + 28 Healing, five true MP refills), but the bounded evidence does not establish a pacing or resource-accounting bug.

Trace cutoff is the active file at review time; this is a bounded fight result, not a full-run completion claim.
