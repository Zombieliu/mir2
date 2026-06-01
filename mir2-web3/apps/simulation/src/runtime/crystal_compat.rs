pub(super) const GUIDE_QUEST_ID: i32 = 1001;
pub(super) const GUIDE_NPC_ID: u32 = 4001;
pub(super) const FIELD_WASP_ID: u32 = 3002;
pub(super) const CRYSTAL_DATA_RANGE: i32 = 16;
pub(super) const CRYSTAL_RESPAWN_OBJECT_ID_BASE: u32 = 200_000;
pub(super) const CRYSTAL_PANEL_BUY: u8 = 0;
pub(super) const CRYSTAL_PANEL_BUY_SUB: u8 = 1;
pub(super) const CRYSTAL_PANEL_CRAFT: u8 = 2;
pub(super) const CRYSTAL_GOODS_HIDE_ADDED_STATS: bool = true;
pub(super) const CRYSTAL_GOODS_BUY_BACK_TIME_MINUTES: u64 = 60;
pub(super) const CRYSTAL_GOODS_BUY_BACK_MAX_STORED: usize = 20;
pub(super) const CRYSTAL_GOODS_MAX_STORED_PER_ITEM: usize = 20;
pub(super) const MONSTER_PLAYER_TARGET_RANGE: i32 = CRYSTAL_DATA_RANGE;
/// Crystal `MonsterObject.RegenDelay` is 10000ms; at 1 tick/second a damaged monster heals on a
/// 10-tick cadence. Object-id phasing reproduces the randomised `RegenTime` set on spawn.
/// Per-damage-class salts for the deterministic GetAttackPower roll, so a monster's DC, MC and SC
/// rolls on the same tick are decorrelated.
pub(super) const CRYSTAL_DAMAGE_ROLL_SALT_DC: u64 = 0xD0_0000;
pub(super) const CRYSTAL_DAMAGE_ROLL_SALT_MC: u64 = 0xC0_0000;
pub(super) const CRYSTAL_DAMAGE_ROLL_SALT_SC: u64 = 0x50_0000;
/// Decorrelates the monster armour roll (`GetDefencePower`) from the accuracy/agility hit roll, which
/// shares the `(tick, attacker_id, target_index)` seed.
pub(super) const CRYSTAL_MONSTER_ARMOUR_ROLL_SALT: usize = 0x47_0000;
pub(super) const MONSTER_REGEN_INTERVAL_TICKS: u64 = 10;
/// Crystal heals `(MaxHP * 0.022) + 1` each regen pulse (2.2 per-mille of maximum HP, plus one).
pub(super) const MONSTER_REGEN_PERMILLE: i32 = 22;
pub(super) const BUG_BAT_IMAGE: u16 = 42;
pub(super) const BUG_BAT_FALLBACK_VIEW_RANGE: u8 = 7;
pub(super) const BUG_BAT_FALLBACK_HP: i32 = 3;
pub(super) const BUG_BAT_FALLBACK_ATTACK_SPEED: u16 = 2_500;
pub(super) const BUG_BAT_FALLBACK_MOVE_SPEED: u16 = 1_200;
pub(super) const BOMB_SPIDER_IMAGE: u16 = 50;
pub(super) const BOMB_SPIDER_FALLBACK_VIEW_RANGE: u8 = 7;
pub(super) const BOMB_SPIDER_FALLBACK_HP: i32 = 10;
pub(super) const BOMB_SPIDER_FALLBACK_MOVE_SPEED: u16 = 800;
pub(super) const BOMB_SPIDER_EXPLOSION_LIFETIME_TICKS: u64 = 300;
pub(super) const BOMB_SPIDER_EXPLOSION_DELAY_TICKS: u64 = 1;
pub(super) const HELL_BOMB_EXPLOSION_LIFETIME_TICKS: u64 = 10;
pub(super) const HELL_BOMB_EXPLOSION_DELAY_TICKS: u64 = 1;
pub(super) const HELL_BOMB_EXPLOSION_RADIUS: i32 = 4;
pub(super) const HELL_BOMB_POISON_DURATION_TICKS: u64 = 5;
pub(super) const HELL_LORD_RAGE_DELAY_TICKS: u64 = 120;
/// HellLord SpawnQuakes: each pulse erupts up to this many MapQuake hazards within QUAKE_SPREAD
/// tiles of the player; each hazard ticks for QUAKE_LIFETIME ticks and hits whoever stands on it.
pub(super) const HELL_LORD_QUAKE_COUNT: usize = 5;
pub(super) const HELL_LORD_QUAKE_SPREAD: i32 = 4;
pub(super) const HELL_LORD_QUAKE_LIFETIME_TICKS: u64 = 5;
pub(super) const REVIVING_ZOMBIE_REVIVE_DELAY_TICKS: u64 = 4;
pub(super) const REVIVING_ZOMBIE_MAX_REVIVALS: u8 = 2;
pub(super) const REVIVING_ZOMBIE_LIFECOUNT_SALT: usize = 25_001;
pub(super) const REVIVING_ZOMBIE_DELAY_SALT: usize = 25_002;
// Crystal `PoisonType` bit flags (`Shared/Enums.cs`). `MonsterPoisonState.poison` stores these.
// GREEN/SLOW round out the documented bit set but are only referenced by the mask unit test.
#[allow(dead_code)]
pub(super) const CRYSTAL_POISON_GREEN: u16 = 1;
pub(super) const CRYSTAL_POISON_RED: u16 = 2;
#[allow(dead_code)]
pub(super) const CRYSTAL_POISON_SLOW: u16 = 4;
pub(super) const CRYSTAL_POISON_FROZEN: u16 = 8;
pub(super) const CRYSTAL_POISON_STUN: u16 = 16;
pub(super) const CRYSTAL_POISON_PARALYSIS: u16 = 32;
pub(super) const CRYSTAL_POISON_LR_PARALYSIS: u16 = 256;
pub(super) const CRYSTAL_POISON_DAZED: u16 = 1024;
/// Crystal `MonsterObject.CanMove`: a monster cannot move while Frozen, Stunned, Paralysed or
/// LR-Paralysed (the Stun-vs-`Info.Light` 10/5 exemption is omitted — no spawned family carries it).
pub(super) const CRYSTAL_POISON_BLOCKS_MOVE: u16 =
    CRYSTAL_POISON_FROZEN | CRYSTAL_POISON_STUN | CRYSTAL_POISON_PARALYSIS | CRYSTAL_POISON_LR_PARALYSIS;
/// Crystal `MonsterObject.CanAttack`: additionally Dazed stops a monster attacking.
pub(super) const CRYSTAL_POISON_BLOCKS_ATTACK: u16 = CRYSTAL_POISON_BLOCKS_MOVE | CRYSTAL_POISON_DAZED;
pub(super) const SPITTING_SPIDER_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 8;
pub(super) const SPITTING_SPIDER_GREEN_POISON_DURATION_TICKS: u64 = 5;
pub(super) const CAVE_MAGGOT_PARALYSIS_CHANCE_DENOMINATOR: u64 = 20;
pub(super) const CAVE_MAGGOT_PARALYSIS_DURATION_TICKS: u64 = 5;
pub(super) const CAVE_MAGGOT_PARALYSIS_BUFF_KEY: &str = "crystal-paralysis";
pub(super) const TRAP_ROCK_PARALYSIS_DURATION_TICKS: u64 = 3;
pub(super) const TRAP_ROCK_ATTACK_PARALYSIS_CHANCE_DENOMINATOR: u64 = 8;
pub(super) const INCARNATED_ZT_PARALYSIS_CHANCE_DENOMINATOR: u64 = 12;
pub(super) const INCARNATED_ZT_PARALYSIS_DURATION_TICKS: u64 = 5;
pub(super) const TUCSON_GENERAL_PARALYSIS_CHANCE_DENOMINATOR: u64 = 3;
pub(super) const TUCSON_GENERAL_PARALYSIS_DURATION_TICKS: u64 = 5;
pub(super) const HELL_KEEPER_DAZED_DURATION_TICKS: u64 = 10;
pub(super) const HELL_KEEPER_DAZED_BUFF_KEY: &str = "crystal-dazed";
pub(super) const ICE_GUARD_SLOW_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const ICE_GUARD_SLOW_DURATION_TICKS: u64 = 5;
pub(super) const ICE_GUARD_SLOW_BUFF_KEY: &str = "crystal-slow";
pub(super) const ICE_GUARD_FROZEN_CHANCE_DENOMINATOR: u64 = 10;
pub(super) const ICE_GUARD_FROZEN_DURATION_TICKS: u64 = 3;
pub(super) const ICE_GUARD_FROZEN_BUFF_KEY: &str = "crystal-frozen";
pub(super) const SNOW_WOLF_SLOW_CHANCE_DENOMINATOR: u64 = 4;
pub(super) const SNOW_WOLF_SLOW_DURATION_TICKS: u64 = 5;
pub(super) const SNOW_WOLF_FROZEN_CHANCE_DENOMINATOR: u64 = 8;
pub(super) const SNOW_WOLF_FROZEN_DURATION_TICKS: u64 = 5;
pub(super) const SNOW_YETI_FROZEN_CHANCE_DENOMINATOR: u64 = 3;
pub(super) const SNOW_YETI_FROZEN_DURATION_TICKS: u64 = 5;
pub(super) const KIRIN_SLOW_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const KIRIN_SLOW_DURATION_TICKS: u64 = 4;
pub(super) const SEEDINGS_GENERAL_POISON_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const SEEDINGS_GENERAL_POISON_DURATION_TICKS: u64 = 5;
pub(super) const CANNIBAL_TENTACLES_HALFMOON_DAMAGE: i32 = 500;
pub(super) const CANNIBAL_TENTACLES_GREEN_POISON_DURATION_TICKS: u64 = 5;
pub(super) const JAR2_FROZEN_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const JAR2_FROZEN_DURATION_TICKS: u64 = 5;
pub(super) const SAND_SNAIL_GREEN_POISON_DURATION_TICKS: u64 = 5;
pub(super) const MAN_TREE_STUN_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const MAN_TREE_STUN_DURATION_TICKS: u64 = 5;
pub(super) const MAN_TREE_STUN_BUFF_KEY: &str = "crystal-stun";
pub(super) const TRAINER_DAMAGE_REPORT_IDLE_TICKS: u64 = 5;
/// YinDevilNode (UltimateEnhancer) buffs friendly monsters within 7 tiles with +MaxDC for 5s.
pub(super) const YIN_DEVIL_NODE_BUFF_RANGE: i32 = 7;
pub(super) const YIN_DEVIL_NODE_BUFF_DURATION_TICKS: u64 = 5;
pub(super) const GREAT_FOX_SPIRIT_SLOW_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const GREAT_FOX_SPIRIT_SLOW_DURATION_TICKS: u64 = 15;
pub(super) const GREAT_FOX_SPIRIT_PARALYSIS_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const GREAT_FOX_SPIRIT_PARALYSIS_DURATION_TICKS: u64 = 5;
pub(super) const GREAT_FOX_SPIRIT_RECALL_COOLDOWN_TICKS: u64 = 10;
pub(super) const GREAT_FOX_SPIRIT_RECALL_TELEPORT_EFFECT: u8 = 11;
pub(super) const BONE_LORD_STAGE_COUNT: u8 = 3;
pub(super) const BONE_LORD_SPAWN_BATCH_SIZE: usize = 8;
pub(super) const BONE_LORD_MAX_SLAVES: usize = 40;
pub(super) const BONE_LORD_SLAVE_NAMES: [&str; 4] =
    ["BoneSpearman", "BoneBlademan", "BoneArcher", "BoneCaptain"];
pub(super) const ZUMA_TAURUS_STAGE_COUNT: u8 = 7;
pub(super) const ZUMA_TAURUS_SPAWN_BATCH_SIZE: usize = 8;
pub(super) const ZUMA_TAURUS_MAX_SLAVES: usize = 40;
pub(super) const ZUMA_TAURUS_SLAVE_NAMES: [&str; 7] = [
    "ZumaStatue",
    "ZumaGuardian",
    "ZumaArcher",
    "WedgeMoth",
    "ZumaArcher3",
    "ZumaStatue3",
    "ZumaGuardian3",
];
pub(super) const GENERAL_MEOW_MEOW_SHIELD_DURATION_TICKS: u64 = 30;
pub(super) const GENERAL_MEOW_MEOW_SLAVE_SPAWN_INTERVAL_TICKS: u64 = 60;
pub(super) const GENERAL_MEOW_MEOW_SLAVE_SPAWN_COUNT: usize = 3;
pub(super) const GENERAL_MEOW_MEOW_MAX_SLAVES: usize = 6;
pub(super) const GENERAL_MEOW_MEOW_SHIELD_ARMOUR: i32 = 100;
pub(super) const GENERAL_MEOW_MEOW_THUNDER_SPAWN_DELAY_TICKS: u64 = 2;
pub(super) const GENERAL_MEOW_MEOW_THUNDER_MIN_COOLDOWN_TICKS: u64 = 1;
pub(super) const GENERAL_MEOW_MEOW_THUNDER_RANDOM_COOLDOWN_TICKS: u64 = 4;
pub(super) const GENERAL_MEOW_MEOW_SLAVE_NAMES: [&str; 4] =
    ["StainHammerCat", "BlackHammerCat", "StrayCat", "CatShaman"];
/// IcePillar (ai 89): a stationary, regen-less, poison-immune damage sponge that loses exactly 1 HP
/// per non-blocked hit, counterattacks ~1/3 of the time, and bursts a frozen AoE on death.
pub(super) const ICE_PILLAR_AI: u8 = 89;
/// Football (ai 68): an invulnerable soccer-ball monster — a player hit deals no damage and instead
/// rolls it up to this many tiles in the attacker's facing direction, bouncing off blocked tiles.
pub(super) const FOOTBALL_AI: u8 = 68;
pub(super) const FOOTBALL_KICK_DISTANCE: i32 = 4;
/// HoodedSummoner (ai 211): on a 1/6 attack roll (cases 4 and 5) it summons scroll-mob slaves —
/// case 4 from {WarriorScroll, TaoistScroll}, case 5 from {WizardScroll, AssassinScroll} — capped at
/// 4 live slaves on a 15-second throttle; every other roll is a ranged MC attack.
pub(super) const HOODED_SUMMONER_AI: u8 = 211;
pub(super) const HOODED_SUMMONER_MAX_SLAVES: u8 = 4;
pub(super) const HOODED_SUMMONER_SLAVE_THROTTLE_TICKS: u64 = 15;
pub(super) const HOODED_SUMMONER_SCROLLS_GROUP_A: [&str; 2] = ["WarriorScroll", "TaoistScroll"];
pub(super) const HOODED_SUMMONER_SCROLLS_GROUP_B: [&str; 2] = ["WizardScroll", "AssassinScroll"];
/// TreeQueen (ai 142): a stationary root-spawning boss. Each root pulse erupts a 7x7 MassRoots field
/// (MC damage) centred on the player; ground roots erupt a 5x5 DC patch. Root pulses are throttled,
/// and when the player is within 2 tiles it adds a 3-tile fire-bombardment (MACAgility) on its beat.
pub(super) const TREE_QUEEN_AI: u8 = 142;
pub(super) const TREE_QUEEN_ROOT_THROTTLE_TICKS: u64 = 3;
pub(super) const TREE_QUEEN_MASS_ROOT_RADIUS: i32 = 3;
pub(super) const TREE_QUEEN_ROOT_LIFETIME_TICKS: u64 = 2;
pub(super) const TREE_QUEEN_FIRE_BOMBARDMENT_RADIUS: i32 = 3;
pub(super) const SNOW_WOLF_KING_SLAVE_COUNT: usize = 3;
/// EvilMir (ai 52) `Attack`: each landed hit rolls `PoisonTarget(_, 5, 15, Green, 2000)` and
/// `PoisonTarget(_, 5, 5, Paralysis, 1000)` — both green DOT and a paralysis stun, 1/5 each. The
/// generated profile only carries the green half; this case adds the paralysis.
pub(super) const EVIL_MIR_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const EVIL_MIR_GREEN_POISON_DURATION_TICKS: u64 = 15;
pub(super) const EVIL_MIR_PARALYSIS_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const EVIL_MIR_PARALYSIS_DURATION_TICKS: u64 = 5;
/// SnowWolfKing FindWeakerTarget: when struck for more than its own DC it blinks (Teleport effect
/// 11) toward a fresh target, 50% of the time when a player lands the blow.
pub(super) const SNOW_WOLF_KING_TELEPORT_EFFECT: u8 = 11;
pub(super) const SNOW_WOLF_KING_TELEPORT_CHANCE_DENOMINATOR: u64 = 2;
/// GlacierWarrior (203) / MutatedManworm (65) share SnowWolfKing's FindWeakerTarget blink but with
/// teleport visual effect 4 (Crystal `TeleportEffect`).
pub(super) const GLACIER_WARRIOR_TELEPORT_EFFECT: u8 = 4;
pub(super) const DRAGON_STATUE_SLEEP_DURATION_TICKS: u64 = 15 * 60;
pub(super) const BUFF_GENERAL_MEOW_MEOW_SHIELD: u8 = 52;
pub(super) const SPELL_EFFECT_RED_MOON_EVIL: u8 = 4;
pub(super) const SPELL_EFFECT_GREAT_FOX_SPIRIT: u8 = 8;
pub(super) const CAT_SHAMAN_RED_POISON_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const CAT_SHAMAN_RED_POISON_DURATION_TICKS: u64 = 5;
pub(super) const WHITE_FOXMAN_SLOW_DURATION_TICKS: u64 = 5;
pub(super) const FOXMAN_FEAR_DURATION_TICKS: u64 = 5;
pub(super) const RED_FOXMAN_TELEPORT_COOLDOWN_TICKS: u64 = 10;
pub(super) const RED_FOXMAN_TELEPORT_RADIUS: i32 = 14;
pub(super) const RED_FOXMAN_TELEPORT_EFFECT: u8 = 2;
pub(super) const YIMOOGI_RED_POISON_DURATION_TICKS: u64 = 6;
pub(super) const YIMOOGI_RED_POISON_BUFF_KEY: &str = "crystal-red-poison";
pub(super) const YIMOOGI_CHILD_SPAWN_DELAY_TICKS: u64 = 4;
pub(super) const YIMOOGI_CHILD_ACTIVATION_DELAY_TICKS: u64 = 2;
pub(super) const YIMOOGI_FINAL_TELEPORT_ATTEMPTS: usize = 40;
pub(super) const YIMOOGI_FINAL_WHITE_SERPENT_COUNT: usize = 2;
pub(super) const YIMOOGI_WHITE_SERPENT_NAME: &str = "WhiteSerpent";
pub(super) const RESTLESS_JAR_BLINDNESS_CHANCE_DENOMINATOR: u64 = 4;
pub(super) const RESTLESS_JAR_BLINDNESS_DURATION_TICKS: u64 = 10;
pub(super) const RESTLESS_JAR_BLINDNESS_BUFF_KEY: &str = "crystal-blindness";
pub(super) const EVIL_CENTIPEDE_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 5;
pub(super) const EVIL_CENTIPEDE_GREEN_POISON_DURATION_TICKS: u64 = 15;
pub(super) const EVIL_CENTIPEDE_PARALYSIS_CHANCE_DENOMINATOR: u64 = 15;
pub(super) const EVIL_CENTIPEDE_PARALYSIS_DURATION_TICKS: u64 = 5;
pub(super) const WATER_DRAGON_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 7;
pub(super) const WATER_DRAGON_GREEN_POISON_DURATION_TICKS: u64 = 5;
pub(super) const FROST_TIGER_POISON_CHANCE_DENOMINATOR: u64 = 8;
pub(super) const FROST_TIGER_POISON_DURATION_TICKS: u64 = 5;
pub(super) const FROST_TIGER_BLEEDING_BUFF_KEY: &str = "crystal-bleeding";
pub(super) const FROST_TIGER_SIT_DOWN_MAX_DELAY_TICKS: u64 = 120;
pub(super) const TOXIC_GHOUL_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 8;
pub(super) const TOXIC_GHOUL_GREEN_POISON_DURATION_TICKS: u64 = 5;
pub(super) const TOXIC_GHOUL_GREEN_POISON_BUFF_KEY: &str = "crystal-green-poison";
pub(super) const DEER_SKIN_COUNT: u8 = 5;
pub(super) const HARVEST_MONSTER_SKIN_COUNT: u8 = 2;
pub(super) const JAR1_DEATH_SPAWN_DELAY_TICKS: u64 = 1;
pub(super) const TUCSON_EGG_DEATH_DELAY_TICKS: u64 = 1;
pub(super) const TUCSON_EGG_GREEN_POISON_CHANCE_DENOMINATOR: u64 = 3;
pub(super) const TUCSON_EGG_GREEN_POISON_DURATION_TICKS: u64 = 5;
pub(super) const TUCSON_GENERAL_RAGE_COOLDOWN_TICKS: u64 = 20;
pub(super) const TUCSON_GENERAL_RAGE_ATTACK_PAUSE_TICKS: u64 = 8;
pub(super) const TUCSON_GENERAL_ROCK_COUNT: usize = 15;
pub(super) const TUCSON_GENERAL_ROCK_TARGET_SALT_BASE: u64 = 131_000;
pub(super) const TUCSON_GENERAL_ROCK_TARGET_INDEX_SALT_BASE: usize = 131_500;
pub(super) const TUCSON_GENERAL_ROCK_SCATTER_X_SALT_BASE: usize = 132_000;
pub(super) const TUCSON_GENERAL_ROCK_SCATTER_Y_SALT_BASE: usize = 132_500;
pub(super) const TUCSON_GENERAL_ROCK_DELAY_SALT_BASE: usize = 133_000;
pub(super) const WOOMA_TAURUS_STAGE_COUNT: u8 = 7;
pub(super) const WOOMA_TAURUS_TELEPORT_DELAY_TICKS: u64 = 10;
pub(super) const WOOMA_TAURUS_MAD_DURATION_TICKS: u64 = 8;
pub(super) const WOOMA_TAURUS_MAD_MOVE_INTERVAL_TICKS: u64 = 1;
pub(super) const WOOMA_TAURUS_MAD_ATTACK_INTERVAL_TICKS: u64 = 1;
pub(super) const WOOMA_TAURUS_BLOCKED_NEIGHBOR_THRESHOLD: usize = 5;
pub(super) const WOOMA_TAURUS_TELEPORT_RADIUS: i32 = 8;
pub(super) const DOTNET_TICKS_AT_UNIX_EPOCH: i64 = 621_355_968_000_000_000;
pub(super) const DOTNET_DATETIME_KIND_LOCAL: i64 = i64::MIN;
pub(super) const BASE_STORAGE_SLOTS: u16 = 80;
pub(super) const EXPANDED_STORAGE_SLOTS: u16 = 160;
pub(super) const CRYSTAL_NPC_MAX_SECTION_HOPS: usize = 12;
pub(super) const DEFAULT_CRYSTAL_CLIENT_ROOT: &str = r"E:\mir2\Crystal\Build\Client\Debug";
pub(super) const CRYSTAL_BIND_DONT_DROP: i16 = 0x0002;
pub(super) const CRYSTAL_BIND_DONT_SELL: i16 = 0x0004;
pub(super) const CRYSTAL_BIND_DONT_STORE: i16 = 0x0008;
pub(super) const CRYSTAL_BIND_DONT_TRADE: i16 = 0x0010;
pub(super) const CRYSTAL_BIND_DONT_REPAIR: i16 = 0x0020;
pub(super) const CRYSTAL_BIND_DONT_UPGRADE: i16 = 0x0040;
pub(super) const CRYSTAL_BIND_DESTROY_ON_DROP: i16 = 0x0080;
pub(super) const CRYSTAL_BIND_ON_EQUIP: i16 = 0x0200;
pub(super) const CRYSTAL_BIND_NO_SREPAIR: i16 = 0x0400;
pub(super) const CRYSTAL_BIND_UNABLE_TO_RENT: i16 = 0x1000;
pub(super) const CRYSTAL_BIND_UNABLE_TO_DISASSEMBLE: i16 = 0x2000;
pub(super) const CRYSTAL_BIND_NO_HERO: i16 = i16::MIN;

pub(super) const CRYSTAL_ITEM_TYPE_MEAT: u8 = 15;
pub(super) const CRYSTAL_ITEM_TYPE_WEAPON: u8 = 1;
pub(super) const CRYSTAL_ITEM_TYPE_ARMOUR: u8 = 2;
pub(super) const CRYSTAL_ITEM_TYPE_HELMET: u8 = 4;
pub(super) const CRYSTAL_ITEM_TYPE_NECKLACE: u8 = 5;
pub(super) const CRYSTAL_ITEM_TYPE_BRACELET: u8 = 6;
pub(super) const CRYSTAL_ITEM_TYPE_RING: u8 = 7;
pub(super) const CRYSTAL_ITEM_TYPE_AMULET: u8 = 8;
pub(super) const CRYSTAL_ITEM_TYPE_BELT: u8 = 9;
pub(super) const CRYSTAL_ITEM_TYPE_BOOTS: u8 = 10;
pub(super) const CRYSTAL_ITEM_TYPE_STONE: u8 = 11;
pub(super) const CRYSTAL_ITEM_TYPE_TORCH: u8 = 12;
pub(super) const CRYSTAL_ITEM_TYPE_POTION: u8 = 13;
pub(super) const CRYSTAL_ITEM_TYPE_SCROLL: u8 = 17;
pub(super) const CRYSTAL_ITEM_TYPE_MOUNT: u8 = 19;
pub(super) const CRYSTAL_ITEM_TYPE_BOOK: u8 = 20;
pub(super) const CRYSTAL_ITEM_TYPE_GEM: u8 = 18;
pub(super) const CRYSTAL_ITEM_TYPE_FOOD: u8 = 27;
pub(super) const CRYSTAL_ITEM_TYPE_BAIT: u8 = 30;
pub(super) const CRYSTAL_FISHING_ROD_SHAPES: [i16; 2] = [49, 50];
pub(super) const CRYSTAL_POTION_SHAPE_NORMAL: i16 = 0;
pub(super) const CRYSTAL_POTION_SHAPE_SUN_POTION: i16 = 1;
pub(super) const CRYSTAL_POTION_SHAPE_MYSTERY_WATER: i16 = 2;
pub(super) const CRYSTAL_POTION_SHAPE_BUFF: i16 = 3;
pub(super) const CRYSTAL_POTION_SHAPE_EXP: i16 = 4;
pub(super) const CRYSTAL_POTION_SHAPE_DROP: i16 = 5;
pub(super) const CRYSTAL_SCROLL_SHAPE_DUNGEON_ESCAPE: i16 = 0;
pub(super) const CRYSTAL_SCROLL_SHAPE_TOWN_TELEPORT: i16 = 1;
pub(super) const CRYSTAL_SCROLL_SHAPE_RANDOM_TELEPORT: i16 = 2;
pub(super) const CRYSTAL_SCROLL_SHAPE_BENEDICTION_OIL: i16 = 3;
pub(super) const CRYSTAL_SCROLL_SHAPE_REPAIR_OIL: i16 = 4;
pub(super) const CRYSTAL_SCROLL_SHAPE_WAR_GOD_OIL: i16 = 5;
pub(super) const CRYSTAL_SCROLL_SHAPE_GT_INVITE: i16 = 26;
pub(super) const CRYSTAL_SCROLL_SHAPE_GT_TELEPORT: i16 = 27;
pub(super) const CRYSTAL_SCROLL_SHAPE_RESURRECTION: i16 = 6;
pub(super) const CRYSTAL_SCROLL_SHAPE_MAP_SHOUT: i16 = 8;
pub(super) const CRYSTAL_SCROLL_SHAPE_SERVER_SHOUT: i16 = 9;
pub(super) const CRYSTAL_REQUIRED_CLASS_WARRIOR: u8 = 1;
pub(super) const CRYSTAL_REQUIRED_CLASS_WIZARD: u8 = 2;
pub(super) const CRYSTAL_REQUIRED_CLASS_TAOIST: u8 = 4;
pub(super) const CRYSTAL_REQUIRED_CLASS_ASSASSIN: u8 = 8;
pub(super) const CRYSTAL_REQUIRED_CLASS_ARCHER: u8 = 16;
pub(super) const CRYSTAL_REQUIRED_GENDER_MALE: u8 = 1;
pub(super) const CRYSTAL_REQUIRED_GENDER_FEMALE: u8 = 2;
pub(super) const CRYSTAL_REQUIRED_TYPE_LEVEL: u8 = 0;
pub(super) const CRYSTAL_REQUIRED_TYPE_MAX_AC: u8 = 1;
pub(super) const CRYSTAL_REQUIRED_TYPE_MAX_MAC: u8 = 2;
pub(super) const CRYSTAL_REQUIRED_TYPE_MAX_DC: u8 = 3;
pub(super) const CRYSTAL_REQUIRED_TYPE_MAX_MC: u8 = 4;
pub(super) const CRYSTAL_REQUIRED_TYPE_MAX_SC: u8 = 5;
pub(super) const CRYSTAL_REQUIRED_TYPE_MAX_LEVEL: u8 = 6;
pub(super) const CRYSTAL_REQUIRED_TYPE_MIN_AC: u8 = 7;
pub(super) const CRYSTAL_REQUIRED_TYPE_MIN_MAC: u8 = 8;
pub(super) const CRYSTAL_REQUIRED_TYPE_MIN_DC: u8 = 9;
pub(super) const CRYSTAL_REQUIRED_TYPE_MIN_MC: u8 = 10;
pub(super) const CRYSTAL_REQUIRED_TYPE_MIN_SC: u8 = 11;
pub(super) const CRYSTAL_GEM_SHAPE_UPGRADE_GEM: i16 = 3;
pub(super) const CRYSTAL_GEM_SHAPE_UPGRADE_ORB: i16 = 4;
pub(super) const CRYSTAL_GEM_SHAPE_REPAIR_HAMMER: i16 = 1;
pub(super) const CRYSTAL_GEM_SHAPE_REPAIR_SEWING: i16 = 2;
pub(super) const CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_HAMMER: i16 = 5;
pub(super) const CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_SEWING: i16 = 6;
pub(super) const CRYSTAL_GEM_SHAPE_SOCKET: i16 = 7;
pub(super) const CRYSTAL_GEM_SHAPE_SEAL: i16 = 8;
pub(super) const CRYSTAL_SPECIAL_PARALYZE: i16 = 0x0001;
pub(super) const CRYSTAL_SPECIAL_TELEPORT: i16 = 0x0002;
pub(super) const CRYSTAL_SPECIAL_CLEAR_RING: i16 = 0x0004;
pub(super) const CRYSTAL_SPECIAL_PROTECTION: i16 = 0x0008;
pub(super) const CRYSTAL_SPECIAL_REVIVAL: i16 = 0x0010;
pub(super) const CRYSTAL_SPECIAL_MUSCLE: i16 = 0x0020;
pub(super) const CRYSTAL_SPECIAL_FLAME: i16 = 0x0040;
pub(super) const CRYSTAL_SPECIAL_HEALING: i16 = 0x0080;
pub(super) const CRYSTAL_SPECIAL_PROBE: i16 = 0x0100;
pub(super) const CRYSTAL_SPECIAL_SKILL: i16 = 0x0200;
pub(super) const CRYSTAL_SPECIAL_NO_DURA_LOSS: i16 = 0x0400;
pub(super) const CRYSTAL_ITEM_SEAL_DELAY_MINUTES: u64 = 60;
pub(super) const CRYSTAL_STAT_MIN_AC: u8 = 0;
pub(super) const CRYSTAL_STAT_MAX_AC: u8 = 1;
pub(super) const CRYSTAL_STAT_MIN_MAC: u8 = 2;
pub(super) const CRYSTAL_STAT_MAX_MAC: u8 = 3;
pub(super) const CRYSTAL_STAT_MIN_DC: u8 = 4;
pub(super) const CRYSTAL_STAT_MAX_DC: u8 = 5;
pub(super) const CRYSTAL_STAT_MIN_MC: u8 = 6;
pub(super) const CRYSTAL_STAT_MAX_MC: u8 = 7;
pub(super) const CRYSTAL_STAT_MIN_SC: u8 = 8;
pub(super) const CRYSTAL_STAT_MAX_SC: u8 = 9;
pub(super) const CRYSTAL_STAT_ACCURACY: u8 = 10;
pub(super) const CRYSTAL_STAT_AGILITY: u8 = 11;
pub(super) const CRYSTAL_STAT_HP: u8 = 12;
pub(super) const CRYSTAL_STAT_MP: u8 = 13;
pub(super) const CRYSTAL_STAT_ATTACK_SPEED: u8 = 14;
pub(super) const CRYSTAL_STAT_LUCK: u8 = 15;
pub(super) const CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT: u8 = 124;
pub(super) const CRYSTAL_STAT_ENERGY_SHIELD_PERCENT: u8 = 125;
pub(super) const CRYSTAL_STAT_ENERGY_SHIELD_HP_GAIN: u8 = 126;
pub(super) const CRYSTAL_STAT_MANA_PENALTY_PERCENT: u8 = 127;
pub(super) const CRYSTAL_STAT_TELEPORT_MANA_PENALTY_PERCENT: u8 = 128;
pub(super) const CRYSTAL_STAT_REFLECT: u8 = 19;
pub(super) const CRYSTAL_STAT_STRONG: u8 = 20;
pub(super) const CRYSTAL_STAT_FREEZING: u8 = 22;
pub(super) const CRYSTAL_STAT_POISON_ATTACK: u8 = 23;
pub(super) const CRYSTAL_STAT_MAGIC_RESIST: u8 = 30;
pub(super) const CRYSTAL_STAT_POISON_RESIST: u8 = 31;
pub(super) const CRYSTAL_STAT_HEALTH_RECOVERY: u8 = 32;
pub(super) const CRYSTAL_STAT_SPELL_RECOVERY: u8 = 33;
pub(super) const CRYSTAL_STAT_POISON_RECOVERY: u8 = 34;
pub(super) const CRYSTAL_STAT_CRITICAL_RATE: u8 = 35;
pub(super) const CRYSTAL_STAT_CRITICAL_DAMAGE: u8 = 36;
pub(super) const CRYSTAL_STAT_MAX_DC_RATE_PERCENT: u8 = 42;
pub(super) const CRYSTAL_STAT_MAX_MC_RATE_PERCENT: u8 = 43;
pub(super) const CRYSTAL_STAT_MAX_SC_RATE_PERCENT: u8 = 44;
pub(super) const CRYSTAL_STAT_ATTACK_SPEED_RATE_PERCENT: u8 = 45;
pub(super) const CRYSTAL_STAT_HP_DRAIN_RATE_PERCENT: u8 = 48;
pub(super) const CRYSTAL_STAT_GEM_RATE_PERCENT: u8 = 104;
pub(super) const CRYSTAL_STAT_SKILL_GAIN_MULTIPLIER: u8 = 107;
