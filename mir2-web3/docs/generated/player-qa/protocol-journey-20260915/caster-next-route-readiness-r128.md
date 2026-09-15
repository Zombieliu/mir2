# Caster q113/q114 route readiness (R128)

Scope: read-only comparison of the local newcomer route definitions, generated Wizard/Taoist profiles, map-travel graph, and current survival/controller policies. No live/store/process/source changes.

## Source-backed route facts

- `config/quest-guidance/newcomer-journey-v1.json:1463-1474` defines q113 **Insomnia**: level 26+, task `BlackMaggot 3` and `WedgeMoth 3`, diary accept and diary finish, with no start/finish NPC.
- The same config at `:1477-1495` defines q114 **Threats to Mongchon**: level 26+, task `BlackBoar 3` and `RedBoar 3`, start and finish at `MongchonDelegate_Michael` on map `3`, `(330,332)`.
- Generated profiles `docs/generated/quest-agent/wizard-1-30-newcomer-v1.json` and `taoist-1-30-newcomer-v1.json` contain the same class-independent route definitions. Their q113 objective candidates include both species on `D715` (and `D717` among other candidates). Their q114 candidates include both species on `D715`; RedBoar also has `D713`. q113 source is `MongchonProvince/StoneTomb/1`; q114 source is `MongchonProvince/StoneTomb/2`.
- `apps/web/scripts/quest-agent/route-manifest.mjs:490-519,701` builds ordinary map and NPC-script edges. The current graph exposes the ordinary chain `3 -> D710 -> D710A -> D711A -> D712 -> D713 -> D714 -> D715`, plus `D715 -> D714`; it does not expose a `D715 -> D717` edge. This is not a blocker for either route because `D715` is a common generated candidate for both q113 species and both q114 species.
- `apps/web/scripts/quest-agent/protocol-combat.mjs:143-168,855-897` chooses candidate maps through the route graph and `routeLength`, so unreachable D717 should be skipped in favor of reachable D715 when the graph is authoritative. No hard-coded q113/q114 destination override is present in `preferredObjectiveMapsForQuest` (`protocol-survival.mjs:418-425`).

## Existing caster policy

- `apps/web/scripts/quest-agent/protocol-survival.mjs:6` classifies q113/q114 as dangerous expeditions.
- `:228-251` gives Wizard and Taoist q113 the ranged-insect profile, including continued healthy travel and the wide/long recovery profile; q114 receives the generic dangerous-expedition profile with the same 90-second/8-tile recovery bands (`:212-236`).
- `:166` gives Wizard q113 a 65% escape floor; Taoist q113 remains at the generic 35% floor. `:193-202` gives q113 four RandomTeleport departure uses and q114 eight. `run-protocol-journey.mjs:92,313,475` includes both quests in dangerous-expedition and HP-target tables.
- The R127 Amulet exception is q89-only (`protocol-survival.mjs:63-90`); q113/q114 retain the ordinary Taoist Amulet policy. There is no source-backed q113/q114 evidence here requiring a new Amulet, MP, gold, or teleport budget.

## Readiness boundary

No concrete definition/policy contradiction is established. The immediate level gate is real: both quests require level 26, while the observed Taoist/Wizard saved states were level 25. q113’s diary lifecycle and q114’s Michael NPC lifecycle are represented in the route metadata and controller paths, but neither has a fresh level-26-to-30 live completion proof in the reviewed evidence.

The only topology unknown requiring observation during the next run is whether the live route graph and current spawn snapshots yield a reachable D715 candidate before the controller considers other generated candidates. If D715 is unavailable at runtime, the remaining candidate maps and their live route/landing state need to be recorded; this is an evidence gap, not a reason to change budgets now. q114’s generated D716xx candidates are deep alternatives and have no fresh caster run evidence in this review.

No broad balance or budget change is recommended from this bounded review.
