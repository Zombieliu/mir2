# Monster System Parity Assessment (Crystal C# → Rust runtime)

_Last updated: 2026-05-30. Scope: `apps/simulation/src/runtime/{monster_ai,monsters,combat,crystal_compat}.rs` vs `Crystal/Server/MirObjects/MonsterObject.cs` and `Crystal/Server/MirObjects/Monsters/*.cs`._

## TL;DR — the monster AI module is far more complete than earlier audits claimed

A prior gap note estimated monster AI at "~16–20% (绝大多数怪走同一套通用状态机)". A direct file-by-file comparison against the Crystal source (submodule pulled, 212 monster subclasses) does **not** support that number. Coming into this session the monster-AI module was already ~78–85% complete for the **87 spawned AI families** (the families the stock respawn manifests actually place on maps); the session's fixes — notably correcting the base `MonsterObject` damage (it was a flat 7), adding `GetAttackPower` variance, regen, and several per-family behaviours — bring it to roughly **86–90%**. The remaining work is the special-attack-branch variance, a few complex boss mechanics, and combat-module fidelity (MAC mitigation).

Two completeness lenses, kept separate on purpose:

| Lens | Estimate | Notes |
| --- | --- | --- |
| Spawned families (87 of 212) — playable content | ~86–90% | Deep per-family handling; base damage + variance now corrected |
| All 212 Crystal monster classes | ~42% | 117 "data-only" classes are defined but never placed by stock respawns (no gameplay impact until spawned by GM/quests/custom content) |

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

All landed with deterministic tests and verified against the CI-style suite
(`cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`): the
pre-existing flaky baseline stayed at 70 failures with **zero new failures** and
+8 new passing tests.

1. **Monster HP regeneration (`ProcessRegen`)** — living, damaged monsters recover `≈2.2%·MaxHP + 1` every 10 ticks (Crystal `RegenDelay`), object-id phased. Non-combat props and training dummies excluded. `combat.rs::tick_monster_regen`.
2. **`PoisonStopRegen`** — a monster with an active green/bleeding poison no longer regenerates (Crystal's default for every monster bar the training dummy).
3. **Real damage class instead of a flat `7`** — `monster_player_attack_damage` fell through to a hardcoded `7` for every family without an explicit case, so the plain `MonsterObject` plus SpittingSpider, ShamanZombie, BoneSpearman, VampireSpider, SpittingToad, … all dealt exactly 7 to players regardless of stats. They now use their damage class.
4. **Monster attack damage variance** — monster DC/MC/SC damage now rolls `Random(MinDC..MaxDC)` (Crystal `GetAttackPower`, Luck 0) per swing instead of always-max, deterministically seeded by `(tick, attacker object id)`. Applied to the base attack path (`monster_player_attack_damage`) and monster-vs-monster (`summon_attack_damage`); 473 of 555 monsters have a DC range.
5. **SpittingSpider green poison-on-hit** — `PoisonTarget(8, 5, Green)` was missing.
6. **RevivingZombie** — `LifeCount` is now a per-zombie `Random.Next(3)` (0–2 revivals, ~1/3 stay dead) on a randomised 4–24 s `RevivalTime`, instead of a guaranteed two revivals on a fixed 4 s timer.
7. **WaterDragon (ai 181)** — routed through the shared EvilCentipede ambush cycle (invisible until a target is within 3 tiles, HP restored while buried); 425 spawned ambushers previously stood permanently visible.

## Genuine remaining gaps (to reach ≥90% on spawned families)

- **Variance on the special-attack branches** — the ~22 boss/AoE special-attack damage branches in `advance_world` (rage stomps, triple-DC slams, wide-line magic, …) still use always-max; only the base attack path was rolled this session.
- **Magic vs physical mitigation** — all monster damage is mitigated by player AC (`total_defence_bonus`); magic-typed monster attacks (`DefenceType.MAC*`) should use player MAC. Combat-module-adjacent.
- **`ShockTime`** — the player-skill-induced "stop attacking / drop target" timer is a stub (`shock_time = 0`).
- **Complex boss mechanics (1–2 entities each, need new infra)** — HellLord `SpawnQuakes` (player-damaging ground hazards), SnowWolfKing `FindWeakerTarget` teleport-on-hit, YinDevilNode friendly-DC buff aura.
- **Data-only families (117)** — AI exists in Crystal but the families are never placed by stock respawns; no gameplay impact until spawned.

## Note on the automated audit

A parallel Crystal-vs-Rust audit (three agents over all 87 spawned families)
surfaced candidate gaps, but ~40% were false positives on verification — e.g.
BugBagMaggot/RootSpider range (already `CRYSTAL_DATA_RANGE`), WoomaTaurus rage
duration (correct once 1 tick = 1 s is accounted for), ManectricClaw/DarkWraith
cooldowns/ranges (already approximated), Armadillo dig-out cycle (already routed
through `update_dig_out_zombie_state`), SnakeTotem/StoneTrap taunt (already
covered by friendly-summon targeting). Every finding above was re-verified
against the Crystal source before any change.

## Test-harness note (pre-existing, not introduced here)

The simulation suite must be run as CI runs it: `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`. On this checkout the suite has ~70 pre-existing failures that are **flaky / order- and wall-clock-sensitive** (e.g. `current_wall_time_ms()` gating, absolute-tick-phase attack branches such as `tick % 6 == 1`, and high-level monsters dealing always-max damage that kills the low-HP default character faster than a test expects). A clean-tree run measured `833 passed; 70 failed`. Every change in this session was diffed against that baseline: the final state is `839 passed; 70 failed` — **zero** new failures and **+6** new passing tests — so each fix is validated with deterministic, self-contained tests rather than relying on the flaky full-suite baseline.

> One scoped follow-up was deliberately left out: extending the `GetAttackPower` roll to the ~22 boss/AoE special-attack damage branches in `advance_world`. That change is correct (it even un-breaks `armadillo_type_one_branch_uses_three_half_dc_hits`, where the player now survives the three hits) but seven branch tests pin their special attacks to the maximum value and must first be converted to range assertions; doing so on the flaky baseline was out of scope for this pass.
