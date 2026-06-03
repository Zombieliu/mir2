use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::config::GroundDropSnapshot;

use mir2_game_data::CrystalMonsterTemplate;
use mir2_protocol::{
    ChatItem, ClientBuff, MirClass, MirDirection, MirGender, ObjectHealthInfo, ObjectManaInfo,
    Point, ServerPacket, Spell, UserItem,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SessionId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SessionId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZoneKey {
    pub shard_id: String,
    pub map_file_name: String,
    pub channel_id: u16,
    pub instance_id: String,
}

impl ZoneKey {
    pub fn new(
        shard_id: impl Into<String>,
        map_file_name: impl Into<String>,
        channel_id: u16,
        instance_id: impl Into<String>,
    ) -> Self {
        Self {
            shard_id: shard_id.into(),
            map_file_name: map_file_name.into(),
            channel_id,
            instance_id: instance_id.into(),
        }
    }

    pub fn for_map(map_file_name: impl Into<String>) -> Self {
        Self::new("primary", map_file_name, 0, "main")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChatProfile {
    pub group_members: Vec<String>,
    pub guild_name: Option<String>,
    pub blocked_names: Vec<String>,
    pub mentor_name: Option<String>,
    pub relationship_name: Option<String>,
    pub is_gm: bool,
    pub free_map_shout: bool,
    pub free_server_shout: bool,
}

impl Default for ZoneChatProfile {
    fn default() -> Self {
        Self {
            group_members: Vec::new(),
            guild_name: None,
            blocked_names: Vec::new(),
            mentor_name: None,
            relationship_name: None,
            is_gm: false,
            free_map_shout: false,
            free_server_shout: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChatItem {
    pub metadata: ChatItem,
    pub item: Option<UserItem>,
}

/// Authoritative combat stat block for a player inside the shared zone.
///
/// Historically the gateway computed the final melee/range/magic damage inside
/// the attacker's *personal* `SimulationSession` and handed the zone a single
/// pre-rolled scalar (`ZoneCommand::PlayerAttackObject { damage, .. }`). That
/// made the per-player session — not the shared zone — the authority for combat
/// outcomes, so two players hitting the same monster were trusting two
/// independently computed numbers.
///
/// Carrying the attacker's stat block into the zone lets the zone itself roll
/// the Crystal-style `Random(MinDC..=MaxDC)` damage, run the accuracy-vs-agility
/// hit check, and subtract the target's armour — making the zone the single
/// source of truth for combat resolution.
///
/// When `has_authoritative_damage()` is `false` (the default), the zone falls
/// back to the legacy trusted scalar so existing callers keep working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZonePlayerCombatStats {
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_mc: i32,
    pub max_mc: i32,
    pub min_sc: i32,
    pub max_sc: i32,
    pub accuracy: i32,
    pub agility: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_mac: i32,
    pub max_mac: i32,
    /// Crystal `CriticalRate` (0..100 chance) and `CriticalDamage` (each point is
    /// +10% on a landed crit). Zero for a player with no crit gear.
    pub critical_rate: i32,
    pub critical_damage: i32,
    /// Crystal `Luck` stat. Biases the physical attack-power roll
    /// (`MapObject.GetAttackPower`): positive Luck can force the `MaxDC` end,
    /// negative Luck the `MinDC` end. Zero (the default) leaves the roll uniform,
    /// so a player with no Luck gear is unaffected.
    pub luck: i32,
}

impl ZonePlayerCombatStats {
    /// Whether the zone has enough information to roll authoritative physical
    /// (melee/range) damage instead of trusting the gateway-supplied scalar.
    pub fn has_authoritative_damage(&self) -> bool {
        self.max_dc > 0
    }

    /// Whether the zone has enough information to roll authoritative magic
    /// (wizardry) damage instead of trusting the gateway-supplied scalar.
    pub fn has_authoritative_magic(&self) -> bool {
        self.max_mc > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneJoin {
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub object_id: u32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub map_file_name: String,
    pub position: Point,
    pub direction: MirDirection,
    pub chat_profile: ZoneChatProfile,
    pub combat_stats: ZonePlayerCombatStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneMovementActionKind {
    Turn,
    Walk,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneMovementAction {
    pub kind: ZoneMovementActionKind,
    pub direction: MirDirection,
    pub seq: Option<u64>,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMonsterSpawn {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub image: u16,
    pub ai: u8,
    pub level: u16,
    pub max_hp: i32,
    pub hp: i32,
    pub experience: u32,
    pub position: Point,
    pub direction: MirDirection,
    /// Authoritative defensive stats so the zone can resolve incoming player
    /// damage (accuracy-vs-agility hit check + armour subtraction) itself. When
    /// `defense.is_zero()` the zone treats the monster as having no armour/dodge
    /// and applies trusted damage unchanged (legacy behaviour).
    pub defense: ZoneMonsterDefense,
    pub drops: Vec<GroundDropSnapshot>,
}

/// Authoritative defensive stats for a shared-zone monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZoneMonsterDefense {
    pub agility: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_mac: i32,
    pub max_mac: i32,
}

impl ZoneMonsterDefense {
    /// Project a Crystal monster template's authoritative defensive stats into a
    /// shared-zone defense block so the zone can run the accuracy-vs-agility hit
    /// check and subtract armour itself.
    pub fn from_crystal_template(template: &CrystalMonsterTemplate) -> Self {
        Self {
            agility: template.agility.max(0),
            min_ac: template.min_ac.max(0),
            max_ac: template.max_ac.max(0),
            min_mac: template.min_mac.max(0),
            max_mac: template.max_mac.max(0),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.agility == 0
            && self.min_ac == 0
            && self.max_ac == 0
            && self.min_mac == 0
            && self.max_mac == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMonsterKillAward {
    pub monster_object_id: u32,
    pub monster_name: String,
    pub experience: u32,
    pub drops: Vec<GroundDropSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ZoneCommand {
    Join(ZoneJoin),
    Leave {
        session_id: SessionId,
    },
    Walk {
        session_id: SessionId,
        direction: MirDirection,
        seq: u64,
        now_ms: u64,
    },
    Run {
        session_id: SessionId,
        direction: MirDirection,
        seq: u64,
        now_ms: u64,
    },
    Turn {
        session_id: SessionId,
        direction: MirDirection,
        now_ms: u64,
    },
    UpdateChatProfile {
        session_id: SessionId,
        profile: ZoneChatProfile,
    },
    /// Refresh the authoritative combat stat block for a player (e.g. after an
    /// equipment change or a buff recompute) so the zone keeps rolling damage
    /// from current stats.
    UpdatePlayerCombatStats {
        session_id: SessionId,
        stats: ZonePlayerCombatStats,
    },
    SyncPlayerTransform {
        session_id: SessionId,
        position: Point,
        direction: MirDirection,
    },
    Chat {
        session_id: SessionId,
        message: String,
        linked_items: Vec<ChatItem>,
        linked_user_items: Vec<UserItem>,
        now_ms: u64,
    },
    BroadcastPackets {
        session_id: SessionId,
        owner_local_object_id: u32,
        packets: Vec<ServerPacket>,
        now_ms: u64,
    },
    SyncSharedObjects {
        session_id: SessionId,
        packets: Vec<ServerPacket>,
        now_ms: u64,
    },
    BroadcastSharedObjectPackets {
        session_id: SessionId,
        local_self_object_id: Option<u32>,
        packets: Vec<ServerPacket>,
        now_ms: u64,
    },
    SyncGroundDrops {
        session_id: SessionId,
        drops: Vec<GroundDropSnapshot>,
        now_ms: u64,
    },
    SpawnMonster {
        session_id: SessionId,
        monster: ZoneMonsterSpawn,
        now_ms: u64,
    },
    PlayerAttackObject {
        session_id: SessionId,
        object_id: u32,
        direction: MirDirection,
        spell: u8,
        level: u8,
        attack_type: u8,
        damage: i32,
        now_ms: u64,
    },
    PlayerRangeAttackObject {
        session_id: SessionId,
        object_id: u32,
        direction: MirDirection,
        target: Point,
        spell: Spell,
        level: u8,
        attack_type: u8,
        damage: i32,
        now_ms: u64,
    },
    PlayerCastMagic {
        session_id: SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: Point,
        cast: bool,
        level: u8,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    },
    ClaimGroundDrop {
        session_id: SessionId,
        object_id: Option<u32>,
        target: Point,
        group_members: Vec<String>,
        now_ms: u64,
    },
    ClaimNearestGroundDrop {
        session_id: SessionId,
        origin: Point,
        max_range: i32,
        allowed_object_ids: BTreeSet<u32>,
        group_members: Vec<String>,
        now_ms: u64,
    },
    CommitGroundDropClaim {
        session_id: SessionId,
        object_id: u32,
    },
    CancelGroundDropClaim {
        session_id: SessionId,
        object_id: u32,
        now_ms: u64,
    },
    TickPlayerMovement {
        session_id: SessionId,
        now_ms: u64,
    },
    OpenDoor {
        session_id: SessionId,
        door_index: u8,
        now_ms: u64,
    },
    ConfigureHazards {
        session_id: SessionId,
        lightning: bool,
        fire: bool,
        lightning_damage: i32,
        fire_damage: i32,
    },
    Tick {
        now_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ZoneOutbound {
    ToSession {
        session_id: SessionId,
        packets: Vec<ServerPacket>,
    },
    ToMany {
        session_ids: Vec<SessionId>,
        packets: Vec<ServerPacket>,
    },
    ToAll {
        packets: Vec<ServerPacket>,
    },
    SaveTransform {
        session_id: SessionId,
        position: Point,
        direction: MirDirection,
    },
    ConsumeShoutPermission {
        session_id: SessionId,
        map_shout: bool,
        server_shout: bool,
    },
    GroundDropClaimed {
        session_id: SessionId,
        drop: GroundDropSnapshot,
    },
    MonsterKillAward {
        session_id: SessionId,
        award: ZoneMonsterKillAward,
    },
    PlayerDamaged {
        session_id: SessionId,
        damage: i32,
    },
    PlayerHealed {
        session_id: SessionId,
        amount: i32,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ZonePlayerBuff {
    pub buff: ClientBuff,
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ZoneObject {
    pub object_id: u32,
    pub position: Point,
    pub packet: ServerPacket,
    pub health: Option<ObjectHealthInfo>,
    pub mana: Option<ObjectManaInfo>,
    pub expires_at_ms: Option<u64>,
    pub buffs: BTreeMap<u8, ZonePlayerBuff>,
}

#[derive(Debug, Clone)]
pub(crate) struct ZoneGroundDrop {
    pub drop: GroundDropSnapshot,
    pub owner_expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ZoneGroundDropClaim {
    pub session_id: SessionId,
    pub drop: GroundDropSnapshot,
}

#[derive(Debug, Clone)]
pub(crate) struct ZoneNativeMonster {
    pub name: String,
    pub ai: u8,
    pub hostile_to_player: bool,
    pub owner_session_id: Option<SessionId>,
    pub master_object_id: u32,
    pub owner_player_object_id: u32,
    pub visible_extra: bool,
    pub summon_skill_level: u8,
    pub level: u16,
    pub max_hp: i32,
    pub hp: i32,
    pub experience: u32,
    pub defense: ZoneMonsterDefense,
    pub position: Point,
    pub direction: MirDirection,
    pub dead: bool,
    pub drops: Vec<GroundDropSnapshot>,
    pub next_ai_ready_at_ms: u64,
    pub next_attack_ready_at_ms: u64,
    pub control_until_ms: u64,
    pub control_poison: u16,
    pub damage_poison: u16,
    pub damage_poison_value: i32,
    pub damage_poison_next_damage_at_ms: u64,
    pub damage_poison_expires_at_ms: u64,
    pub damage_poison_owner_session_id: Option<SessionId>,
    pub damage_poison_owner_object_id: u32,
    pub buffs: BTreeMap<u8, ZonePlayerBuff>,
}

impl ZoneNativeMonster {
    pub fn from_spawn(spawn: &ZoneMonsterSpawn, _object_id: u32) -> Self {
        let max_hp = spawn.max_hp.max(1);
        let hp = spawn.hp.clamp(0, max_hp);
        Self {
            name: spawn.name.clone(),
            ai: spawn.ai,
            hostile_to_player: zone_native_monster_targets_players(spawn.ai),
            owner_session_id: None,
            master_object_id: 0,
            owner_player_object_id: 0,
            visible_extra: false,
            summon_skill_level: 0,
            level: spawn.level,
            max_hp,
            hp,
            experience: spawn.experience,
            defense: spawn.defense,
            position: spawn.position.clone(),
            direction: spawn.direction,
            dead: hp == 0,
            drops: spawn.drops.clone(),
            next_ai_ready_at_ms: 0,
            next_attack_ready_at_ms: 0,
            control_until_ms: 0,
            control_poison: 0,
            damage_poison: 0,
            damage_poison_value: 0,
            damage_poison_next_damage_at_ms: 0,
            damage_poison_expires_at_ms: 0,
            damage_poison_owner_session_id: None,
            damage_poison_owner_object_id: 0,
            buffs: BTreeMap::new(),
        }
    }
}

fn zone_native_monster_targets_players(ai: u8) -> bool {
    !matches!(ai, 1 | 2 | 3 | 6 | 34 | 56 | 57 | 58 | 113)
}

#[derive(Debug, Clone)]
pub(crate) struct ZonePlayer {
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub position: Point,
    pub direction: MirDirection,
    pub light: u8,
    pub weapon: i16,
    pub weapon_effect: i16,
    pub armour: i16,
    pub poison: u16,
    pub native_status_poison: u16,
    pub native_status_poison_expires_at_ms: Option<u64>,
    pub dead: bool,
    pub hidden: bool,
    pub effect: u8,
    pub wing_effect: u8,
    pub mount_type: i16,
    pub riding_mount: bool,
    pub fishing: bool,
    pub transform_type: i16,
    pub level_effects: u16,
    pub visible_object_ids: BTreeSet<u32>,
    pub movement_actions: VecDeque<ZoneMovementAction>,
    pub last_seen_move_seq: u64,
    pub movement_ready_at_ms: u64,
    pub run_step_until_ms: u64,
    pub shout_ready_at_ms: u64,
    pub next_attack_ready_at_ms: u64,
    pub next_spell_ready_at_ms: u64,
    pub magic_ready_at_ms: BTreeMap<u8, u64>,
    pub chat_profile: ZoneChatProfile,
    pub combat_stats: ZonePlayerCombatStats,
    pub buffs: BTreeMap<u8, ZonePlayerBuff>,
}

impl ZonePlayer {
    pub fn from_join(join: ZoneJoin, object_id: u32) -> Self {
        Self {
            session_id: join.session_id,
            account_id: join.account_id,
            character_index: join.character_index,
            object_id,
            name: join.name,
            name_colour_argb: -1,
            class: join.class,
            gender: join.gender,
            level: join.level,
            hp: join.hp.clamp(0, join.max_hp.max(1)),
            max_hp: join.max_hp.max(1),
            mp: join.mp.max(0),
            position: join.position,
            direction: join.direction,
            light: 0,
            weapon: -1,
            weapon_effect: 0,
            armour: -1,
            poison: 0,
            native_status_poison: 0,
            native_status_poison_expires_at_ms: None,
            dead: false,
            hidden: false,
            effect: 0,
            wing_effect: 0,
            mount_type: -1,
            riding_mount: false,
            fishing: false,
            transform_type: 0,
            level_effects: 0,
            visible_object_ids: BTreeSet::new(),
            movement_actions: VecDeque::new(),
            last_seen_move_seq: 0,
            movement_ready_at_ms: 0,
            run_step_until_ms: 0,
            shout_ready_at_ms: 0,
            next_attack_ready_at_ms: 0,
            next_spell_ready_at_ms: 0,
            magic_ready_at_ms: BTreeMap::new(),
            chat_profile: join.chat_profile,
            combat_stats: join.combat_stats,
            buffs: BTreeMap::new(),
        }
    }
}
