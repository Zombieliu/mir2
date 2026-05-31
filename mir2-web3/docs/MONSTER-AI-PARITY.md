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

### AI 4 → SpittingSpider green-poison-on-hit ✅
Crystal `SpittingSpider.CompleteAttack` applies
`PoisonTarget(target, 8, 5, PoisonType.Green, 2000)` to every line target it
strikes. The Rust runtime already covered the 2-tile line geometry + 300ms
delay (`spitting_spider_line_branch` + `forward_line_opposing_monster_targets`),
but the matching green-poison status effect on the *player* was missing — only
the line geometry hit. Added arm `4 => GreenPoison { chance_denominator: 1,
duration_ticks: SPITTING_SPIDER_GREEN_POISON_DURATION_TICKS (5) }` to
`monster_player_status_effect`. `chance_denominator=1` matches Crystal's
deterministic `PoisonTarget` (no chance roll). Test:
`crystal_ai4_spitting_spider_poisons_player_on_hit` (asserts the player picks
up `TOXIC_GHOUL_GREEN_POISON_BUFF_KEY` within 10 ticks of an adjacent AI-4
spider).

### AI 29 → BoneSpearman line splash ✅
Crystal `BoneSpearman.Attack` is byte-identical to `SpittingSpider.Attack`
minus the poison: `Direction = DirectionFromPoint(...)`, `Broadcast(ObjectAttack
{...})`, then `LineAttack(damage, 2, 250)` — a 2-tile line attack along the
attack direction that splashes damage to any friendly-opposite target on the
line. The existing
`bone_spearman_ai_hits_from_two_tiles_like_line_attack` test covered the
*player* strike but the line-target splash was missing because AI 29 wasn't in
`spitting_spider_line_branch` (only 4 and 35 were). Extended the matcher to
`4 | 29 | 35`. AI 29 has no poison entry in `monster_player_status_effect`, so
the spider's deterministic poison does not leak into BoneSpearman. Test:
`crystal_ai29_bone_spearman_splashes_line_target` (asserts a friendly-opposite
secondary monster on the line tile loses HP).

### AI 44 → BlackFoxman line splash on range branch ✅
Crystal `BlackFoxman.Attack` is a hybrid: adjacent (range ≤ 1) AND
`Random.Next(3) > 0` (2/3) → base melee `Attack()` (Type 0). Otherwise (range
branch — distance > 1, or 1/3 of adjacent attacks) → `Broadcast(ObjectAttack
Type=1)` + `LineAttack(damage, 2, 250)` against a 2-tile line.

The Rust runtime already covered AI 44's attack range (2 tiles, x==y/parity)
and `monster_object_attack_type` emits Type=1 when distance > 1 (existing test
`black_foxman_uses_type_one_line_attack_at_two_tiles`), but the line *splash*
was missing — only the direct target took damage. Added
`black_foxman_line_branch = agent.ai == 44 && distance > 1` and folded it
into the `spider_line_targets` builder, mirroring the `LineAttack(damage, 2,
…)` shape. The adjacent path retains Type 0 + plain melee (Crystal's "close +
2/3 chance" branch falls back to base `Attack()`, so no line splash).

Test: `crystal_ai44_black_foxman_splashes_line_target_at_range` (spawns AI-44
fox at distance 2 + a friendly-opposite secondary monster on the line; asserts
the secondary's HP drops within 5 ticks).

### AI 26 → ShamanZombie 6-tile line splash ✅
Crystal `ShamanZombie.Attack` faces the target, broadcasts `ObjectRangeAttack`,
then runs `LineAttack(damage, 6, 300, MACAgility)` — a 6-tile line splash
against any friendly-opposite target on the line. The runtime already
mirrored AI 26's attack range (6 tiles, cardinal/diagonal only via
`x == 0 || y == 0 || x == y`), the ranged-attack packet, and the player
strike (existing `shaman_zombie_uses_six_tile_line_range_attack`), but the
6-tile line splash was missing — only the direct target took damage. Added
`shaman_zombie_line_branch = agent.ai == 26` to the `spider_line_targets`
builder with `line_distance = 6` (vs the 2-tile spider default and 3-tile
crystal-spider). Test:
`crystal_ai26_shaman_zombie_splashes_six_tile_line` (places a
friendly-opposite monster 3 tiles in front of the shaman on a 5-tile line and
asserts its HP drops within 6 ticks).

## Next candidates
Higher-effort: 28 `ToxicGhoul` (56 spawns — poison is already wired by AI
table; the AI needs special hooks), 12 `BugBagMaggot` (189 spawns), 7
`CaveMaggot` (poison already wired by name; the AI uses a paralysis-on-attack
override).
