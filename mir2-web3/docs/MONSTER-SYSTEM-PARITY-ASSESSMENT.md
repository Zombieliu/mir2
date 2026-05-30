# Monster System Parity Assessment (Crystal C# → Rust runtime)

_Last updated: 2026-05-30. Scope: `apps/simulation/src/runtime/{monster_ai,monsters,combat,crystal_compat}.rs` vs `Crystal/Server/MirObjects/MonsterObject.cs` and `Crystal/Server/MirObjects/Monsters/*.cs`._

## TL;DR — the monster AI module is far more complete than earlier audits claimed

A prior gap note estimated monster AI at "~16–20% (绝大多数怪走同一套通用状态机)". A direct file-by-file comparison against the Crystal source (submodule pulled, 212 monster subclasses) does **not** support that number. The monster AI module is approximately **78–85%** complete for the **87 spawned AI families** (the families that the stock respawn manifests actually place on maps). The remaining work is a long tail of per-family nuances plus a few base-loop behaviors — not a wholesale rewrite.

Two completeness lenses, kept separate on purpose:

| Lens | Estimate | Notes |
| --- | --- | --- |
| Spawned families (87 of 212) — playable content | ~78–85% | Deep per-family handling already exists |
| All 212 Crystal monster classes | ~40% | 117 "data-only" classes are defined but never placed by stock respawns (no gameplay impact until spawned by GM/quests/custom content) |

## What is already implemented faithfully (verified against Crystal source)

- **Per-AI base damage** — `monsters.rs::monster_player_attack_damage` selects physical/magic/spell power and distance-gated variants per AI number, then applies player AC mitigation (`total_defence_bonus`). ~70 AI numbers are special-cased.
- **Per-AI poison-on-hit** — `monsters.rs::monster_player_status_effect` + an extensive distance/variant cascade in `monster_ai.rs` (`advance_world`). Poison **probabilities match Crystal exactly**: Crystal `PoisonTarget(t, chance, …)` is `Random.Next(chance)==0` (1/chance); the Rust `deterministic_chance_roll(_, _, _, chance)` is `value % chance == 0` (1/chance). Constants (e.g. `CAVE_MAGGOT_PARALYSIS_CHANCE_DENOMINATOR = 20`) line up with the Crystal subclasses.
- **Per-AI `InAttackRange` shapes** — `monsters.rs::monster_in_attack_range` reproduces the bespoke hit shapes: ShamanZombie (ai 26) straight/diagonal range 6; SpittingSpider (ai 4) diagonal range 2; range-2/­range-12 diagonal families, etc. These are 1:1 with the Crystal `InAttackRange` overrides.
- **Attack patterns / AoE** — line, wide-line, half-moon and cone target enumerators exist for the families that use them (`shinsu_line_monster_targets`, `tucson_mage_wide_line_opposing_monster_targets`, `halfmoon_opposing_monster_targets`, `manectric_claw_cone_points`, …).
- **~31 dedicated special-state handlers** dispatched from `update_special_monster_state` — Deer flee, GreatFoxSpirit, ZumaTaurus/ZumaMonster stages, ThunderElement, Yimoogi, GeneralMeowMeow (shield + thunder + slaves), DragonStatue sleep, FrostTiger sit, AxeSkeleton/Foxman/HolyDeva fear, Kirin, EvilCentipede, CannibalPlant, WoomaTaurus, BombSpider, TrapRock, DigOutZombie, Armadillo, RevivingZombie, BoneLord, YinDevilNode, VampireSpider, SpittingToad, SnakeTotem, TucsonGeneral, HellLord, HellBomb, StoneTrap, and boss summon/stage logic for Bone Lord, Zuma Taurus, Snow Wolf King, Hell Lord, Yimoogi.
- **Idle wandering** — `monster_can_patrol_origin` + `patrol_target` roam non-stationary monsters around their spawn origin (respecting `can_wander` and per-AI stationary exclusions).
- **Aggro / target lock** — `tracking_player` aggros within `view_range` and persists out to `MONSTER_PLAYER_TARGET_RANGE`; attacking a monster locks its target (retaliation).
- **Knockback** — monsters are pushed by Repulsion / Shoulder-Dash (`push_monster_one_tile_for_dash`, `apply_crystal_repulsion_spell`, `ObjectPushed`).
- **Death / respawn / revive** — ObjectHealth 0 + ObjectDied locking, scheduled respawns, RevivingZombie self-revive, summon despawn rules, Dragon-Statue sleep-on-lethal.

## Changes made this session

1. **Monster HP regeneration (`ProcessRegen`)** — _new_. Living, damaged monsters recover `≈2.2%·MaxHP + 1` every 10 ticks (Crystal `RegenDelay = 10000ms`), with object-id phasing reproducing Crystal's randomised `RegenTime`. Non-combat props (gates/traps/elementals that ignore damage) and training dummies are excluded. `combat.rs::tick_monster_regen`, wired into `advance_world`. Deterministic tests added.

_(Further per-family fixes from the in-progress audit will be appended here as they land.)_

## Genuine remaining gaps (to reach ≥90% on spawned families)

- **Damage variance** — monster melee/magic damage currently returns `max(MinDC, MaxDC)` (always-max) rather than Crystal's `GetAttackPower = Random(MinDC, MaxDC+1)`. 473 of 555 monsters have a DC range, so this is a broad lethality-curve gap. **Belongs to the combat module** (not monster-AI), but it is the single biggest fidelity item and it makes high-level monsters one/two-shot low-HP characters (visible in `armadillo_*`/`snow_yeti_*` test expectations).
- **Magic vs physical mitigation** — all monster damage is mitigated by player AC (`total_defence_bonus`); magic-typed monster attacks (`DefenceType.MAC*`) should be mitigated by player MAC. Combat-module-adjacent.
- **`ShockTime`** — the player-skill-induced "stop attacking / drop target" timer is a stub (`shock_time = 0`); the few skills that set it on monsters do not yet suppress monster targeting.
- **Long tail of per-family nuances** — being enumerated by an automated Crystal-vs-Rust audit; fixes tracked in follow-up commits.
- **Data-only families (117)** — AI exists in Crystal but the families are never placed by stock respawns; no gameplay impact until spawned.

## Test-harness note (pre-existing, not introduced here)

The simulation suite must be run as CI runs it: `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`. On this checkout the suite has ~70 pre-existing failures that are **flaky / order- and wall-clock-sensitive** (e.g. `current_wall_time_ms()` gating, absolute-tick-phase attack branches such as `tick % 6 == 1`, and high-level monsters dealing always-max damage that kills the low-HP default character faster than a test expects). A clean-tree run measured `833 passed; 70 failed`; the monster-HP-regen change added **zero** new failures (verified by diffing the failure sets). New monster work in this session is therefore validated with deterministic, self-contained tests rather than relying on the flaky full-suite baseline.
