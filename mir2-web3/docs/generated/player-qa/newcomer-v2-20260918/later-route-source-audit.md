# Newcomer V2 later-route source audit

Read-only review of N13–N22 and G20/G25/G30 found no deterministic mismatch
between controller actions, server flags and imported manifests. This is source
review and does not establish ordinary route or visual completion.

- G25 armour is ordinary class/gender-compatible level-24 gear. It is legally
  equipped at N17; the level-26 weapons become legal at N18.
- FireWall 998 is a level-24 book. N16 learns it through the normal item route;
  N16/N21 use caster objectId, ground targetId zero and lock false, then require
  accepted cast, positive target damage and the final server flag separately.
- N18 is loadout-only. Learning SummonSkeleton does not force a pet-combat hunt.
- N22 uses the actual Bichon Wall Board dialog: database npc_index 35 maps to
  live objectId 24 at map 0 (334,259). Only committed owner dialog receipts
  satisfy the final expedition report.
- N13 normal Crystal NPC travel enters D401 at (25,181), matching the shared
  arrival condition. It does not require a debug teleport.
- G30 depends on N22. Its 1.8m EXP reward crosses the original level-29 to
  level-30 threshold; completion still requires all 26 server Completed rows.

N13/N17/N20 provide HP supplies; Wizard/Taoist receive appropriate MP supplies.
Taoist also receives amulets and poisons. Supplies must be held and consumed
through authoritative acknowledgements. Ordinary recovery remains bounded to
three deaths; no infinite replacement inventory or local completion is used.

D401 and D022 hostile density, dynamic occupancy, travel, survivability and
real timings remain ordinary-run gates. Graduation equipment acquisition is
still distinct and open; see graduation-acquisition.md.

visualAccepted=false; ordinaryLaterRouteAccepted=false; measuredTime=false.
