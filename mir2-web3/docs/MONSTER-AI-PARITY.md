# Monster AI per-species parity — Crystal `MonsterObject.GetMonster`

Crystal selects a monster subclass from the numeric `MonsterInfo.AI`
(`Server/MirObjects/MonsterObject.cs` switch). The Rust runtime mirrors this:
generic melee/track/wander applies to every monster (AI 0 baseline), and
distinctive behaviors are dispatched per AI id in
`update_special_monster_state` + special hooks in `advance_world`.

## Bespoke coverage (have)
`update_special_monster_state` arms + special hooks cover AI:
5, 6, 11, 14, 15, 16, 17, 18, 24, 25, 27, 30, 34, 36, 37, 38, 40, 42, 43,
45–50, 54, 60–62, 98, 99, 113, 117–127, 130, 131, 173, 174, 179–192, 255
(zuma, zombies, shinsu, armadillo, bone lord, guards, foxmen, traps, summons,
hell lord, great fox spirit, thunder element, …).

## Gap (top missing AIs by spawn count in the respawn manifest)
| AI | spawns | Crystal subclass | note |
|----|--------|------------------|------|
| 0  | 3588 | `MonsterObject` (base) | intentional generic baseline — not a gap |
| 3  | 365  | `Tree` | passive/harvestable |
| 7  | 233  | `CaveMaggot` | melee + paralysis poison (poison is config-driven) |
| 12 | 189  | `BugBagMaggot` | |
| 26 | 70   | `ShamanZombie` | |
| 44 | 65   | `BlackFoxman` | "2 attacks, 1 close + 1 line" |
| 57 | 62   | `TownArcher` | ranged guard |
| 28 | 56   | `ToxicGhoul` | |
| 29 | 51   | `BoneSpearman` | "1 line attack" |
| 4  | 48   | `SpittingSpider` | "1 line attack + poison" |
| **58** | **48** | **`Guard`** | **identical to AI 6 → done below** |
| 32/31 | 35/34 | Left/RightGuard | (RightGuard 31 already handled) |

## Implemented slices
### AI 58 → Guard ✅
Crystal `GetMonster` case 58 returns `new Guard(info)` — byte-identical to AI 6.
AI-58 monsters (Guard1, 48 spawns) were falling through to generic. Extended the
guard identification to `6 | 58 | 113` in `guard_can_target_monster`,
`monster_prefers_monster_target`, the guard aggro-range arm, and the guard
melee-attack/facing branches in `advance_world`. AI-58 already spawned
non-hostile-to-player (`monster_targets_players` excludes 58), so town guards now
attack hostile monsters and ignore players, matching AI 6. Test:
`crystal_ai58_town_guard_attacks_hostile_monsters` (attacks + strikes a hostile).

### AI 57 → TownArcher ✅
Crystal `TownArcher` overrides `FindTarget` to scan ONLY `ObjectType.Player`
within `ViewRange` and skip any player with `PKPoints < 200`; `Attack()` then
broadcasts `ObjectRangeAttack` and calls `ProjectileAttack(GetAttackPower(MinDC,
MaxDC))`. AI 57 was already configured as Neutral, `monster_uses_ranged_attack`,
attack range 10, but the runtime never *targeted* a player from AI 57 (Neutral
monsters skip the generic player-aggro path). Added bespoke hook
`update_town_archer_state` in `update_special_monster_state`: gates on
`PlayerRuntimeResource.pk_points >= CRYSTAL_RED_NAME_PK_POINTS` (200) and
`monster_in_attack_range` (≤10), then faces, emits `ObjectRangeAttack`,
schedules `schedule_damage_to_player(damage, monster_attack_delay_ticks)`.
Damage arm `57 => crystal_monster_attack_damage(monster_name)` (DC-based,
matches Crystal `GetAttackPower(MinDC, MaxDC)`). Inert against non-PK players
(PKPoints < 200) and against monsters (matches Crystal's `default: continue` in
FindTarget). Test: `crystal_ai57_town_archer_attacks_red_name_player` (no
attack when PKPoints=0; ObjectRangeAttack fires after `pk_points = 250`).

## Next candidates (line/AoE attackers — higher effort)
4 `SpittingSpider` (line+poison), 29 `BoneSpearman` (line),
44 `BlackFoxman` (close+line). These need a line/area attack pattern beyond the
generic ranged path.
