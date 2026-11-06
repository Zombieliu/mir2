use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::config::{GroundDropSnapshot, WorldEntityDisposition};

use mir2_game_data::{crystal_monster_by_name, CrystalMonsterTemplate};
use mir2_protocol::{
    ChatItem, ClientBuff, MirClass, MirDirection, MirGender, ObjectHealthInfo, ObjectManaInfo,
    Point, ServerPacket, Spell, UserItem,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct PlayerId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ZoneKey {
    pub shard_id: String,
    pub map_file_name: String,
    pub channel_id: u16,
    pub instance_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneMapMetadata {
    pub map_index: i32,
    pub file_name: String,
    pub title: String,
    pub mini_map: u16,
    pub big_map: u16,
    pub lights: u8,
    pub map_dark_light: u8,
    pub music: u16,
    pub weather: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneNpcTeleportDestination {
    pub map_file_name: String,
    pub object_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneNpcTeleportConfig {
    pub enabled: bool,
    pub cost: u32,
    pub maps: BTreeMap<String, ZoneMapMetadata>,
    pub destinations: Vec<ZoneNpcTeleportDestination>,
}

impl ZoneNpcTeleportConfig {
    pub fn disabled(cost: u32) -> Self {
        Self {
            enabled: false,
            cost,
            maps: BTreeMap::new(),
            destinations: Vec::new(),
        }
    }

    pub fn destination_enabled(&self, map_file_name: &str, object_id: u32) -> bool {
        self.enabled
            && self.destinations.iter().any(|destination| {
                destination.object_id == object_id
                    && destination
                        .map_file_name
                        .eq_ignore_ascii_case(map_file_name)
            })
    }

    pub fn map(&self, map_file_name: &str) -> Option<&ZoneMapMetadata> {
        self.maps
            .get(&map_file_name.to_ascii_lowercase())
            .or_else(|| {
                self.maps
                    .values()
                    .find(|map| map.file_name.eq_ignore_ascii_case(map_file_name))
            })
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneChatProfile {
    pub group_members: Vec<String>,
    pub guild_name: Option<String>,
    #[serde(default)]
    pub active_guild_wars: Vec<String>,
    pub blocked_names: Vec<String>,
    pub mentor_name: Option<String>,
    pub relationship_name: Option<String>,
    pub is_gm: bool,
    pub free_map_shout: bool,
    pub free_server_shout: bool,
    #[serde(default)]
    pub attack_mode: u8,
    #[serde(default)]
    pub pk_points: i32,
    #[serde(default)]
    pub in_safe_zone: bool,
}

impl Default for ZoneChatProfile {
    fn default() -> Self {
        Self {
            group_members: Vec::new(),
            guild_name: None,
            active_guild_wars: Vec::new(),
            blocked_names: Vec::new(),
            mentor_name: None,
            relationship_name: None,
            is_gm: false,
            free_map_shout: false,
            free_server_shout: false,
            attack_mode: 0,
            pk_points: 0,
            in_safe_zone: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

/// Trusted combat-admission state mirrored from the owning personal session.
///
/// This is deliberately separate from client packets and render snapshots.
/// A newly joined Zone player has no value and therefore cannot attack until
/// the trusted gateway/session integration explicitly synchronizes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZonePlayerCombatState {
    pub class: MirClass,
    pub has_class_weapon: bool,
    pub riding_mount: bool,
    #[serde(default)]
    pub mount_attack_allowed: bool,
    pub dead: bool,
    pub attack_blocked: bool,
    pub fishing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneMovementActionKind {
    Turn,
    Walk,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneMovementAction {
    pub kind: ZoneMovementActionKind,
    pub direction: MirDirection,
    pub seq: Option<u64>,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneMonsterRespawnPolicy {
    /// Smallest wall-clock delay before this spawn may return. This is a
    /// floor applied after the signed Crystal random window is evaluated.
    pub minimum_delay_ms: u64,
    /// Delay before applying the deterministic random window.
    pub base_delay_ms: u64,
    /// Size of one deterministic jitter step. Zero disables jitter.
    pub random_delay_step_ms: u64,
    /// Number of deterministic jitter outcomes, including the zero step.
    pub random_delay_steps: u64,
    /// Number of steps subtracted from the base before adding the roll.
    pub random_delay_subtract_steps: u64,
    /// Stable source coordinates keep the jitter independent of sessions.
    pub rule_index: u32,
    pub slot_index: u32,
}

impl ZoneMonsterRespawnPolicy {
    pub(crate) fn due_at_ms(self, died_at_ms: u64) -> u64 {
        let steps = self.random_delay_steps.max(1);
        let random_step = if self.random_delay_step_ms == 0 || steps == 1 {
            0
        } else {
            let salt = (died_at_ms / 1_000)
                .wrapping_mul(1_103_515_245)
                .wrapping_add(u64::from(self.rule_index).wrapping_mul(97_651))
                .wrapping_add(u64::from(self.slot_index).wrapping_mul(12_347))
                .wrapping_add(0x9E37_79B9);
            salt % steps
        };
        died_at_ms.saturating_add(self.delay_ms_for_roll(random_step))
    }

    fn delay_ms_for_roll(self, random_step: u64) -> u64 {
        let delay_ms = self
            .base_delay_ms
            .saturating_add(random_step.saturating_mul(self.random_delay_step_ms))
            .saturating_sub(
                self.random_delay_subtract_steps
                    .saturating_mul(self.random_delay_step_ms),
            )
            .max(self.minimum_delay_ms);
        delay_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneMonsterSpawn {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub image: u16,
    pub ai: u8,
    /// Explicit authoritative relationship to players. `None` represents an
    /// old or incomplete producer and is deliberately treated as non-hostile;
    /// AI controls behaviour, never combat authorization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<WorldEntityDisposition>,
    pub level: u16,
    pub max_hp: i32,
    pub hp: i32,
    pub experience: u32,
    /// Crystal movement cadence, kept in real milliseconds for the shared
    /// Zone clock. Zero preserves compatibility with old fixtures/checkpoints
    /// and falls back to template/default data in `ZoneNativeMonster`.
    pub move_speed_ms: u64,
    /// Crystal attack cadence in real milliseconds.
    pub attack_speed_ms: u64,
    /// Optional guild protected by this conquest guard. Ordinary monsters and
    /// non-conquest structures leave this unset.
    pub friendly_guild: Option<String>,
    pub position: Point,
    pub direction: MirDirection,
    /// Authoritative defensive stats so the zone can resolve incoming player
    /// damage (accuracy-vs-agility hit check + armour subtraction) itself. When
    /// `defense.is_zero()` the zone treats the monster as having no armour/dodge
    /// and applies trusted damage unchanged (legacy behaviour).
    pub defense: ZoneMonsterDefense,
    /// Server-authored wall-clock respawn policy. Dynamic/event monsters leave
    /// this unset and therefore do not silently become persistent map spawns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respawn: Option<ZoneMonsterRespawnPolicy>,
    pub drops: Vec<GroundDropSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneBossRewardAudit {
    pub reward_owner_session_id: SessionId,
    pub last_hit_session_id: SessionId,
    pub damage_contributions: BTreeMap<SessionId, u64>,
}

/// Authoritative defensive stats for a shared-zone monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneMonsterKillAward {
    pub monster_object_id: u32,
    /// Authoritative time of this monster incarnation's death. Crystal reuses
    /// a spawn's object id after respawn, so the object id alone cannot be an
    /// idempotency key for account/quest rewards.
    #[serde(default)]
    pub killed_at_ms: u64,
    pub monster_name: String,
    pub experience: u32,
    pub drops: Vec<GroundDropSnapshot>,
    #[serde(default)]
    pub boss_audit: Option<ZoneBossRewardAudit>,
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
    TeleportToNpc {
        session_id: SessionId,
        object_id: u32,
        available_gold: u32,
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
    /// Trusted server-to-zone admission update. This command is an internal
    /// API and must never be constructed from raw client JSON.
    SyncPlayerCombatState {
        session_id: SessionId,
        state: ZonePlayerCombatState,
    },
    SyncPlayerTransform {
        session_id: SessionId,
        position: Point,
        direction: MirDirection,
    },
    SyncPlayerVitals {
        session_id: SessionId,
        hp: i32,
        max_hp: i32,
        mp: i32,
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
        include_owner: bool,
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
    SyncNativeMonsters {
        session_id: SessionId,
        monsters: Vec<ZoneMonsterSpawn>,
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
    /// Trusted gateway transaction for a shared melee target that may not yet
    /// be retained by this Zone. Admission is checked before the optional
    /// monster is materialized, and the whole operation commits atomically.
    PlayerAttackMaterializedObject {
        session_id: SessionId,
        object_id: u32,
        monster: Option<ZoneMonsterSpawn>,
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
    /// Trusted gateway transaction equivalent of
    /// `PlayerAttackMaterializedObject` for Archer range attacks.
    PlayerRangeAttackMaterializedObject {
        session_id: SessionId,
        object_id: u32,
        monster: Option<ZoneMonsterSpawn>,
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
    /// Production gateway cast carrying the equipped reagent shape. The
    /// legacy variant remains for deterministic replays and existing callers.
    PlayerCastMagicWithItem {
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
        item_param: u8,
        now_ms: u64,
    },
    ResolveReincarnation {
        session_id: SessionId,
        accept: bool,
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
    /// Legacy object-id-only command. The authoritative runtime rejects it;
    /// callers must use `CommitGroundDropClaimWithTicket`.
    CommitGroundDropClaim {
        session_id: SessionId,
        object_id: u32,
    },
    CommitGroundDropClaimWithTicket {
        session_id: SessionId,
        ticket: GroundDropClaimTicket,
    },
    /// Legacy object-id-only command. The authoritative runtime rejects it;
    /// callers must use `CancelGroundDropClaimWithTicket`.
    CancelGroundDropClaim {
        session_id: SessionId,
        object_id: u32,
        now_ms: u64,
    },
    CancelGroundDropClaimWithTicket {
        session_id: SessionId,
        ticket: GroundDropClaimTicket,
        now_ms: u64,
    },
    /// Trusted server-side interaction boundary. Discards movement intents
    /// that were accepted before an NPC/dialog action became authoritative.
    CancelPendingMovement {
        session_id: SessionId,
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

impl ZoneCommand {
    /// Construct a trusted server-to-zone combat admission update without
    /// exposing the internal state as a client protocol shape.
    pub fn sync_player_combat_state(
        session_id: SessionId,
        class: MirClass,
        has_class_weapon: bool,
        riding_mount: bool,
        mount_attack_allowed: bool,
        dead: bool,
        attack_blocked: bool,
        fishing: bool,
    ) -> Self {
        Self::SyncPlayerCombatState {
            session_id,
            state: ZonePlayerCombatState {
                class,
                has_class_weapon,
                riding_mount,
                mount_attack_allowed,
                dead,
                attack_blocked,
                fishing,
            },
        }
    }
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
    NpcTeleportCommit {
        session_id: SessionId,
        gold_cost: u32,
        map: ZoneMapMetadata,
    },
    ConsumeShoutPermission {
        session_id: SessionId,
        map_shout: bool,
        server_shout: bool,
    },
    /// Legacy outbound retained for source compatibility. New claims use the
    /// ticket-bearing variant exclusively.
    GroundDropClaimed {
        session_id: SessionId,
        drop: GroundDropSnapshot,
    },
    GroundDropClaimedWithTicket {
        session_id: SessionId,
        ticket: GroundDropClaimTicket,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZonePlayerBuff {
    pub buff: ClientBuff,
    pub expires_at_ms: Option<u64>,
    /// Buffs mirrored from a personal session expire there; buffs created by
    /// the shared Zone need a removal packet sent back to their owner.
    #[serde(default)]
    pub notify_owner_on_expiry: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ZoneObject {
    pub object_id: u32,
    pub position: Point,
    pub packet: ServerPacket,
    pub health: Option<ObjectHealthInfo>,
    pub mana: Option<ObjectManaInfo>,
    pub expires_at_ms: Option<u64>,
    pub buffs: BTreeMap<u8, ZonePlayerBuff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZoneGroundDrop {
    pub drop: GroundDropSnapshot,
    pub owner_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub drop_generation: u64,
    #[serde(default)]
    pub payload_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZoneGroundDropClaim {
    pub session_id: SessionId,
    pub drop: GroundDropSnapshot,
    #[serde(default)]
    pub ticket: Option<GroundDropClaimTicket>,
}

/// Authoritative internal capability for a ground-drop claim. This type is
/// intentionally not exported through the client snapshot or packet layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundDropClaimTicket {
    pub claim_id: u64,
    pub object_id: u32,
    pub drop_generation: u64,
    pub payload_digest: String,
    pub idempotency_key: String,
    pub session_id: SessionId,
    pub owner_object_id: Option<u32>,
    pub drop: GroundDropSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZoneNativeMonster {
    pub name: String,
    pub ai: u8,
    /// Retained authoritative relationship supplied by the spawn producer.
    /// Missing legacy checkpoint data fails closed.
    #[serde(default)]
    pub disposition: Option<WorldEntityDisposition>,
    #[serde(default)]
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
    #[serde(default = "default_zone_monster_move_speed_ms")]
    pub move_speed_ms: u64,
    #[serde(default = "default_zone_monster_attack_speed_ms")]
    pub attack_speed_ms: u64,
    #[serde(default)]
    pub friendly_guild: Option<String>,
    pub defense: ZoneMonsterDefense,
    pub position: Point,
    pub direction: MirDirection,
    pub dead: bool,
    pub drops: Vec<GroundDropSnapshot>,
    pub next_ai_ready_at_ms: u64,
    pub next_attack_ready_at_ms: u64,
    pub control_until_ms: u64,
    pub control_poison: u16,
    /// Crystal Hallucination suppresses a monster's normal player targeting
    /// without freezing its movement or making it invulnerable.
    #[serde(default)]
    pub hallucination_until_ms: u64,
    /// Crystal Revelation temporarily exposes health to nearby observers.
    #[serde(default)]
    pub revelation_until_ms: u64,
    pub damage_poison: u16,
    pub damage_poison_value: i32,
    pub damage_poison_next_damage_at_ms: u64,
    pub damage_poison_expires_at_ms: u64,
    pub damage_poison_owner_session_id: Option<SessionId>,
    pub damage_poison_owner_object_id: u32,
    /// Authoritative damage credited to player sessions for Boss ownership.
    /// Summon and damage-over-time attacks use their owning player's session.
    #[serde(default)]
    pub damage_contributions: BTreeMap<SessionId, u64>,
    pub buffs: BTreeMap<u8, ZonePlayerBuff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZoneNativeMonsterRespawn {
    pub spawn: ZoneMonsterSpawn,
    pub due_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneNativeMonsterSnapshot {
    pub object_id: u32,
    pub name: String,
    pub position: Point,
    pub hp: i32,
    pub max_hp: i32,
    pub dead: bool,
    pub disposition: Option<WorldEntityDisposition>,
    pub hostile_to_player: bool,
}

impl ZoneMonsterSpawn {
    pub fn is_authoritatively_hostile_to_player(&self) -> bool {
        self.disposition == Some(WorldEntityDisposition::Hostile)
    }

    /// Crystal passive livestock can be struck by an adjacent physical melee
    /// attack even though it must never acquire or attack a player itself.
    /// Friendly entities and incomplete legacy records continue to fail closed.
    pub fn is_authoritatively_melee_attackable_by_player(&self) -> bool {
        self.is_authoritatively_hostile_to_player()
            || (self.disposition == Some(WorldEntityDisposition::Neutral)
                && zone_native_monster_requires_harvest(self.ai))
    }
}

pub(super) fn zone_native_monster_requires_harvest(ai: u8) -> bool {
    crate::runtime::monsters::monster_ai_requires_harvest(ai)
}

impl ZoneNativeMonster {
    pub fn from_spawn(spawn: &ZoneMonsterSpawn, _object_id: u32) -> Self {
        let max_hp = spawn.max_hp.max(1);
        let hp = spawn.hp.clamp(0, max_hp);
        let template = crystal_monster_by_name(&spawn.name);
        Self {
            name: spawn.name.clone(),
            ai: spawn.ai,
            disposition: spawn.disposition,
            hostile_to_player: spawn.is_authoritatively_hostile_to_player(),
            owner_session_id: None,
            master_object_id: 0,
            owner_player_object_id: 0,
            visible_extra: false,
            summon_skill_level: 0,
            level: spawn.level,
            max_hp,
            hp,
            experience: spawn.experience,
            move_speed_ms: normalize_zone_monster_move_speed_ms(
                spawn.move_speed_ms.max(
                    template
                        .as_ref()
                        .map(|value| u64::from(value.move_speed))
                        .unwrap_or_default(),
                ),
            ),
            attack_speed_ms: normalize_zone_monster_attack_speed_ms(
                spawn.attack_speed_ms.max(
                    template
                        .as_ref()
                        .map(|value| u64::from(value.attack_speed))
                        .unwrap_or_default(),
                ),
            ),
            friendly_guild: spawn.friendly_guild.clone(),
            defense: spawn.defense,
            position: spawn.position.clone(),
            direction: spawn.direction,
            dead: hp == 0,
            drops: spawn.drops.clone(),
            next_ai_ready_at_ms: 0,
            next_attack_ready_at_ms: 0,
            control_until_ms: 0,
            control_poison: 0,
            hallucination_until_ms: 0,
            revelation_until_ms: 0,
            damage_poison: 0,
            damage_poison_value: 0,
            damage_poison_next_damage_at_ms: 0,
            damage_poison_expires_at_ms: 0,
            damage_poison_owner_session_id: None,
            damage_poison_owner_object_id: 0,
            damage_contributions: BTreeMap::new(),
            buffs: BTreeMap::new(),
        }
    }
}

const fn default_zone_monster_move_speed_ms() -> u64 {
    600
}

const fn default_zone_monster_attack_speed_ms() -> u64 {
    1_200
}

fn normalize_zone_monster_move_speed_ms(value: u64) -> u64 {
    if value == 0 {
        default_zone_monster_move_speed_ms()
    } else {
        value.max(300)
    }
}

fn normalize_zone_monster_attack_speed_ms(value: u64) -> u64 {
    if value == 0 {
        default_zone_monster_attack_speed_ms()
    } else {
        value.max(300)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub sneaking: bool,
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
    #[serde(default)]
    pub last_damaged_at_ms: u64,
    #[serde(default)]
    pub last_regen_at_ms: u64,
    pub chat_profile: ZoneChatProfile,
    pub combat_stats: ZonePlayerCombatStats,
    /// `None` is the fail-closed default until the trusted session synchronizes
    /// all combat predicates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat_state: Option<ZonePlayerCombatState>,
    pub buffs: BTreeMap<u8, ZonePlayerBuff>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reincarnation_offer: Option<ZoneReincarnationOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZoneReincarnationOffer {
    pub caster_session_id: SessionId,
    pub caster_object_id: u32,
    pub ready_at_ms: u64,
    pub expires_at_ms: u64,
    pub effect_sent: bool,
    pub requested: bool,
    pub will_succeed: bool,
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
            sneaking: false,
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
            last_damaged_at_ms: 0,
            last_regen_at_ms: 0,
            chat_profile: join.chat_profile,
            combat_stats: join.combat_stats,
            combat_state: None,
            buffs: BTreeMap::new(),
            reincarnation_offer: None,
        }
    }
}

#[cfg(test)]
mod combat_state_tests {
    use super::*;

    #[test]
    fn old_host_defaults_missing_mount_attack_capability_to_denied() {
        let mut value = serde_json::to_value(ZonePlayerCombatState {
            class: MirClass::Warrior,
            has_class_weapon: false,
            riding_mount: true,
            mount_attack_allowed: true,
            dead: false,
            attack_blocked: false,
            fishing: false,
        })
        .expect("serialize combat state");
        value
            .as_object_mut()
            .expect("combat state object")
            .remove("mount_attack_allowed");
        let restored: ZonePlayerCombatState =
            serde_json::from_value(value).expect("deserialize old combat state");
        assert!(!restored.mount_attack_allowed);
    }

    #[test]
    fn crystal_minutes_delay_matches_private_formula_when_random_exceeds_delay() {
        let policy = ZoneMonsterRespawnPolicy {
            minimum_delay_ms: 60_000,
            base_delay_ms: 10 * 60_000,
            random_delay_step_ms: 60_000,
            random_delay_steps: 60,
            random_delay_subtract_steps: 30,
            rule_index: 0,
            slot_index: 0,
        };
        let delays = (0..60)
            .map(|roll| policy.delay_ms_for_roll(roll))
            .collect::<Vec<_>>();
        for (roll, delay_ms) in delays.iter().copied().enumerate() {
            let expected_minutes = (10_i64 - 30 + roll as i64).max(1) as u64;
            assert_eq!(delay_ms, expected_minutes * 60_000);
        }
        assert_eq!(delays.iter().filter(|delay| **delay == 60_000).count(), 22);
        assert_eq!(delays.last(), Some(&(39 * 60_000)));
    }

    #[test]
    fn fixed_tick_delay_matches_private_formula_for_every_roll() {
        let policy = ZoneMonsterRespawnPolicy {
            minimum_delay_ms: 0,
            base_delay_ms: 7_000,
            random_delay_step_ms: 1_000,
            random_delay_steps: 5,
            random_delay_subtract_steps: 0,
            rule_index: 0,
            slot_index: 0,
        };
        for roll in 0..5 {
            assert_eq!(policy.delay_ms_for_roll(roll), (7 + roll) * 1_000);
        }
    }

    #[test]
    fn crystal_harvest_ai_contract_is_shared_by_session_and_zone() {
        for ai in 0_u8..=u8::MAX {
            assert_eq!(
                crate::runtime::monsters::initial_harvest_monster_state(ai).is_some(),
                zone_native_monster_requires_harvest(ai),
                "private and Zone harvest semantics diverged for AI {ai}"
            );
        }
    }
}
