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

### AI 41 → YinDevilNode dispatch fix ✅
Crystal `MonsterObject.GetMonster` cases 41 AND 42 BOTH return
`new YinDevilNode(info)` — an immobile support node. The Rust dispatch in
`update_special_monster_state` only routed `42 => update_yin_devil_node_state`,
so AI 41 fell through to generic hostile chasing. Extended the arm to
`41 | 42 => update_yin_devil_node_state`. AI 41 monsters are now immobile and
do not emit attack packets at the player, matching AI 42. Test:
`crystal_ai41_yin_devil_node_is_immobile_like_ai42`. (The proactive
buff-emission Crystal does in `ProcessTarget`/`CompleteAttack` is a separate
gap noted for future work — both AIs only become immobile here.)

### AIs 24, 25 — DigOutZombie + RevivingZombie DC damage parity ✅
Crystal `DigOutZombie` and `RevivingZombie` both have *no `Attack()`
override* — once visible/revived they fall through to base
`MonsterObject.Attack()`, which uses `GetAttackPower(MinDC, MaxDC)`. The
runtime had bespoke hooks for the visibility/revival state machines, but
the damage path went through `monster_player_attack_damage` → default 7.
Added `24 | 25 => crystal_monster_attack_damage(monster_name)` to use
imported DC. (Visibility/revival hooks unchanged.)

### AI 97 HellKnight — imported DC damage ✅
Crystal `HellKnight` has no `Attack()` override — it uses base
`MonsterObject.Attack()` (DC damage). The runtime had AI 97 wired for the
`HellLord.KnightKilled` linkage via the death hook but no damage arm.
Added `97 => crystal_monster_attack_damage(monster_name)`.

### AI 116 BlackHammerCat — DC/MC damage + line splash ✅
Crystal `BlackHammerCat.Attack` is BlackFoxman-shaped: adjacent + 2/3 chance
→ Type 0 + `GetAttackPower(MinDC, MaxDC)` (DC). Otherwise → Type 1 +
`GetAttackPower(MinMC, MaxMC)` (MC) on the direct hit, then
`LineAttack(damage, 2, 300)` (DC) splashes a 2-tile line.

The runtime already had AI 116 in `monster_attack_range = 2`,
`monster_in_attack_range` (2-tile parity mask shared with 19/44/etc.), and
`monster_object_attack_type` (Type 1 when distance > 1), but no damage arm
(default 7) and no line-splash branch. Added:
- `116 if distance > 1 => crystal_monster_magic_damage(monster_name)` (range
  branch uses MC).
- `116 => crystal_monster_attack_damage(monster_name)` (adjacent branch uses
  DC).
- `black_hammer_cat_line_branch = agent.ai == 116 && distance > 1` folded
  into the `spider_line_targets` builder.

Test `crystal_ai116_black_hammer_cat_splashes_line_target_at_range` proves
the 2-tile line splash hits a friendly-opposite secondary monster.

### AIs 4, 8, 15, 26, 29, 44 — imported DC damage parity ✅
Audit of `monster_player_attack_damage` arms vs Crystal `Attack()` bodies
found six high-spawn AI numbers using `GetAttackPower(MinDC, MaxDC)` in
Crystal but falling through to the default `7` in the runtime:
- AI 4 `SpittingSpider` (48 spawns)
- AI 8 `AxeSkeleton` (306 spawns)
- AI 15 `ZumaMonster` (363 spawns — uses base `MonsterObject.Attack`)
- AI 26 `ShamanZombie` (70 spawns)
- AI 29 `BoneSpearman` (51 spawns)
- AI 44 `BlackFoxman` (65 spawns)

Added a single arm `4 | 8 | 15 | 26 | 29 | 44 =>
crystal_monster_attack_damage(monster_name)` to use the imported DC. Existing
literal-name tests for these AIs (which still resolve to the default 7
because Crystal monster lookup misses) stay green; the new
`crystal_ai26_shaman_zombie_uses_imported_dc_damage` test locks in the new
behavior with the manifest `ShamanZombie` (max_dc=17) — comfortably above 7
fallback.

### AI 31 RightGuard + AI 32 LeftGuard — imported DC damage ✅
Crystal `RightGuard.Attack` and `LeftGuard.Attack` both compute
`int damage = GetAttackPower(Stats[Stat.MinDC], Stats[Stat.MaxDC])` for both
the adjacent ObjectAttack branch and the ranged ObjectRangeAttack branch.
The Rust runtime had AI 31/32 fully wired for attack range (8), ranged
preference, ranged delay, and `monster_object_attack_type`, BUT the damage
arm fell through to the default `7` — so a real Crystal RightGuard
(min_dc=16, max_dc=39) hit the player for 7 instead of ~39. Added
`31 | 32 => crystal_monster_attack_damage(monster_name)` to
`monster_player_attack_damage` to use the imported DC. Test:
`crystal_ai31_right_guard_uses_imported_dc_damage` (spawns a Crystal
RightGuard and asserts damage dealt is well above the 7 fallback).

## Combat numerics: monster→player hit roll ✅ (forward-compatible)
Crystal `MapObject.GetArmour(ACAgility)` lets a target dodge an incoming hit:
`miss if Random.Next(targetAgility + 1) > attackerAccuracy`. The runtime
already had the **player→monster** direction (`crystal_player_hit_roll_succeeds`,
inert while monster Agility is absent from the manifest). Added the symmetric
**monster→player** roll: `crystal_monster_hit_roll_succeeds` reads the
attacker monster's `MonsterCombatStats.accuracy` and rolls against the
player's Agility (equipment + buffs via `crystal_player_agility`). On a miss
it broadcasts a Miss `DamageIndicator` (damage_type 1) for the player and
applies no damage — exactly mirroring the player-side path.

Forward-compatible / inert: monster `accuracy` is a new
`#[serde(default)]` field on `CrystalMonsterTemplate` /
`CrystalRespawnTemplate` / `MonsterSpawnRule` / `MonsterCombatStats`,
threaded through every spawn site. Until the manifest carries real Accuracy
values it defaults to 0, and the roll short-circuits to "always hit" when
attacker accuracy ≤ 0 — so current balance is unchanged (verified: an agile
player vs an accuracy-0 monster is still always hit). Once Accuracy lands in
the manifest the dodge activates 1:1 with Crystal. Test:
`crystal_monster_player_hit_roll_is_inert_at_zero_accuracy_and_dodges_with_accuracy`
(accuracy=0 → always hit; accuracy=1 + huge agility → dodge + Miss indicator).

**Data pipeline:** the manifest generator
(`generate-crystal-respawn-manifest.mjs`) now extracts monster Accuracy
(`Stat.Accuracy = 10`) alongside the already-extracted Agility
(`Stat.Agility = 11`) from `Server.MirDB`, and emits `monster_accuracy` on
each respawn. The only remaining step to activate BOTH hit rolls (player→
monster, which gates on monster Agility, and monster→player, which gates on
monster Accuracy) is to regenerate the committed manifest JSON with the
binary `Server.MirDB` present (not in-repo — same data constraint as the
MMap.Lib art). No further runtime code is required.

## Combat numerics: MagicShield / ElementalBarrier damage reduction ✅
Crystal `HumanObject.Attacked` reduces incoming damage by
`Stats[Stat.DamageReductionPercent]` (`damage -= damage * pct / 100`) before
subtracting armour. The player already gained a
`CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT` buff from casting MagicShield, but the
stat was never consumed — the buff was cosmetic. Added
`crystal_player_damage_reduction_percent` (sums the stat from equipment +
buffs) and applied it inside `monster_player_attack_damage` between the raw
DC/MC damage and the AC subtraction, matching Crystal's
`ChangeHP(armour - damage)` ordering. Data-active whenever the player holds a
DamageReductionPercent buff (e.g. MagicShield); no effect otherwise, so the
70-failure baseline is unchanged. Test:
`crystal_player_damage_reduction_percent_reduces_incoming_monster_damage`
(a RightGuard hit lands for less with a 50% buff than without).

## Verified-already-correct AIs
Audit of remaining high-spawn AIs found these already at parity in the
runtime:
- AI 7 `CaveMaggot` (233 spawns) — paralysis-on-hit wired via
  `monster_player_status_effect` arm `7 => Paralysis`.
- AI 12 `BugBagMaggot` (189 spawns) — stationary, summons bug-bats with cap
  20 and 500ms delay (existing `bug_bag_maggot_spawns_bug_bat_after_delay`).
- AI 28 `ToxicGhoul` (56 spawns) — green poison wired; the death AoE
  (`Info.Effect == 1`) is data-inactive because all manifest ToxicGhoul
  variants carry effect=0.
- AI 9 `HarvestMonster` (18 spawns) — passive harvest already wired via
  `HARVEST_MONSTER_SKIN_COUNT`.
- AI 10 `FlamingWooma` (25 spawns) — plain melee, damage/delay arms keyed.
- AI 56 `Trainer` (25 spawns) — non-attacking + Neutral.
- AI 112 `DarkBeast` (15 spawns) — `dark_beast_secondary_branch`.

## Remaining gaps (lower-impact / data-inactive)
- YinDevilNode (41/42) support-buff emission to friendly targets within 7
  (would emit `BlessedArmour`/`UltimateEnhancer` buffs onto other monsters,
  not the player).
- ToxicGhoul death-AoE branch (data-inactive in current manifests — all
  ToxicGhoul variants carry effect=0).
- Per-branch damage-source mixing (e.g. AI 88 ManectricKing push branch
  uses DC while range/mass uses MC; AI 116 BlackHammerCat Type-1 direct
  hit uses MC while the line splash uses DC). `monster_player_attack_damage`
  takes one (agent, source, target) tuple per call, so dispatching by
  internal random/HP branches isn't possible without restructuring; the
  current arms use the *common* damage source for the distance-keyed
  branch, which is correct for the dominant fraction of hits but not for
  the rare mixed sub-branches.
- AIs with 0 manifest spawns that have Crystal subclasses but no runtime
  bespoke logic (142 TreeQueen, 163-171 Horned*, 210 HoodedSummonerScrolls).
  None of these have active spawns in the imported respawn manifest.
