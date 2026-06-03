use std::collections::{BTreeMap, BTreeSet};

use crate::config::{GroundDropLootSnapshot, GroundDropSnapshot};

use mir2_game_data::{crystal_magic_by_spell, crystal_monster_by_name};
use mir2_protocol::{
    ChatItem, ChatType, ClientBuff, MirDirection, MonsterInfo, ObjectAttackInfo, ObjectDiedInfo,
    ObjectEffectInfo, ObjectGoldInfo, ObjectHealthInfo, ObjectItemInfo, ObjectManaInfo,
    ObjectMovement, ObjectRangeAttackInfo, ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo,
    Point, ServerPacket, Spell, UserItem, UserItemStat,
};

use super::aoi::{players_visible, points_visible, AOI_X_RANGE, AOI_Y_RANGE};
use super::aoi_grid::AoiGrid;
use super::collision::ZoneCollision;
use super::ecs::ZoneEcs;
use super::movement::{
    movement_delay_ms, offset_point, ZONE_MOVE_READY_GRACE_MS, ZONE_RUN_GRACE_MS,
    ZONE_TURN_DELAY_MS,
};
use super::packets::{
    apply_observer_action_state, apply_retained_zone_object_packet, chat_packet,
    object_chat_packet, object_chat_packet_with_text, object_player_packets, object_run_packet,
    object_turn_packet, object_walk_packet, observer_action_packet, owner_action_transform,
    retained_zone_object_from_packet, retained_zone_object_is_drop, retained_zone_object_owned_by,
    retained_zone_object_remove_id, retained_zone_object_update_id, shared_object_action_packet,
    user_location_packet,
};
use super::types::{
    PlayerId, SessionId, ZoneCommand, ZoneGroundDrop, ZoneGroundDropClaim, ZoneJoin, ZoneKey,
    ZoneMonsterKillAward, ZoneMonsterSpawn, ZoneMovementAction, ZoneMovementActionKind,
    ZoneNativeMonster, ZoneObject, ZoneOutbound, ZonePlayer,
};

const SHOUT_COOLDOWN_MS: u64 = 10_000;
const ZONE_MOVEMENT_ACTION_QUEUE_LIMIT: usize = 8;
const ZONE_MOVEMENT_INPUT_BUFFER_MS: u64 = 300;
const ZONE_DROP_EXPIRE_MS: u64 = 30 * 60 * 300;
const ZONE_NATIVE_MONSTER_THINK_MS: u64 = 600;
const ZONE_NATIVE_MONSTER_ATTACK_MS: u64 = 1_200;
const ZONE_NATIVE_MONSTER_AGGRO_X: i32 = 8;
const ZONE_NATIVE_MONSTER_AGGRO_Y: i32 = 6;
const ZONE_NATIVE_MONSTER_RANGED_MAX: i32 = 8;
const ZONE_NATIVE_PLAYER_RANGE_ATTACK_MAX: i32 = 9;
const ZONE_NATIVE_PLAYER_MAGIC_MAX: i32 = 12;
const ZONE_NATIVE_PLAYER_ATTACK_ACTION_MS: u64 = 600;
const ZONE_NATIVE_PLAYER_SPELL_ACTION_MS: u64 = 300;
const ZONE_CRYSTAL_STATUS_TICK_MS: u64 = 1_000;
/// Crystal auto-closes doors 5000 ms after they are opened (`Map.Process`).
const ZONE_DOOR_OPEN_MS: u64 = 5_000;
const ZONE_CRYSTAL_GREEN_POISON_TICK_MS: u64 = 2_000;
const CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT: u8 = 16;
const CRYSTAL_ARROW_BUFF_POISON_SHOT: u8 = 17;
const CRYSTAL_POISON_GREEN: u16 = 1;
const CRYSTAL_POISON_SLOW: u16 = 4;
const CRYSTAL_POISON_FROZEN: u16 = 8;
const CRYSTAL_POISON_STUN: u16 = 16;
const CRYSTAL_POISON_PARALYSIS: u16 = 256;
const CRYSTAL_SPELL_EFFECT_ENTRAPMENT: u8 = 9;
const CRYSTAL_STAT_MIN_AC: u8 = 0;
const CRYSTAL_STAT_MAX_AC: u8 = 1;
const CRYSTAL_STAT_MIN_DC: u8 = 4;
const CRYSTAL_STAT_MAX_DC: u8 = 5;
const CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT: u8 = 124;
const CRYSTAL_PET_ENHANCER_BUFF_TYPE: u8 = 22;
const CRYSTAL_MAGIC_SHIELD_BUFF_TYPE: u8 = 24;
const CRYSTAL_SPELL_EFFECT_HEALING: u8 = 3;
const CRYSTAL_SPELL_EFFECT_MAGIC_SHIELD_UP: u8 = 6;
const CRYSTAL_SPELL_EFFECT_BLEEDING: u8 = 18;

// Note: ZoneRuntime is intentionally not `Clone`. It owns a `bevy_ecs::World`
// (see `ecs`), which is not cloneable, and nothing ever cloned a zone runtime
// (the derive was vestigial). See docs/L2-ECS-ZONE-DESIGN.md.
#[derive(Debug)]
pub struct ZoneRuntime {
    key: ZoneKey,
    collision: ZoneCollision,
    players: BTreeMap<SessionId, ZonePlayer>,
    objects: BTreeMap<u32, ZoneObject>,
    dead_object_ids: BTreeMap<u32, ZoneObjectDeadState>,
    revived_object_ids: BTreeSet<u32>,
    removed_object_ids: BTreeSet<u32>,
    harvested_object_ids: BTreeSet<u32>,
    native_monsters: BTreeMap<u32, ZoneNativeMonster>,
    pending_native_hits: Vec<PendingNativeMonsterHit>,
    pending_native_projectiles: Vec<PendingNativeProjectile>,
    pending_native_player_hits: Vec<PendingNativePlayerHit>,
    pending_native_player_heals: Vec<PendingNativePlayerHeal>,
    pending_native_summons: Vec<PendingNativeSummon>,
    pending_native_ground_spells: Vec<PendingNativeGroundSpellAction>,
    ground_drops: BTreeMap<u32, ZoneGroundDrop>,
    claimed_ground_drops: BTreeMap<u32, ZoneGroundDropClaim>,
    occupancy: BTreeMap<(i32, i32), SessionId>,
    /// Open doors → the tick (ms) at which they auto-close. Shared across all
    /// players on the map (Crystal `Map.Doors`).
    open_doors: BTreeMap<u8, u64>,
    /// Shared environmental hazard state for the map (Crystal `Map.Process`
    /// lightning/fire), authoritative for every player on the map.
    hazard: ZoneHazardState,
    // Spatial index of player positions for area-of-interest candidate lookup.
    // Kept in sync with `players` at every position change (join/leave/move/
    // sync) so visibility diffs scan a local neighborhood instead of all
    // players. See `aoi_grid` and `recompute_player_visibility`.
    player_grid: AoiGrid<SessionId>,
    // Spatial index of zone-object positions (NPCs, owner-generated objects).
    // Objects do not move after insert, so this is synced only at insert/remove.
    object_grid: AoiGrid<u32>,
    // ECS mirror of player entities (L2 step 1). Additive: the `players` map
    // above is still the source of truth; this World mirrors players at the
    // same join/leave/move sites so later L2 slices can flip systems to read
    // from it. Nothing in the tick reads it yet. See docs/L2-ECS-ZONE-DESIGN.md.
    ecs: ZoneEcs,
    next_object_id: u32,
}

#[derive(Debug, Clone, Default)]
struct ZoneHazardState {
    lightning: bool,
    fire: bool,
    lightning_damage: i32,
    fire_damage: i32,
    next_lightning_ms: u64,
    next_fire_ms: u64,
    lightning_strikes: u64,
    fire_strikes: u64,
}

#[derive(Debug, Clone)]
struct ZoneObjectDeadState {
    position: Option<Point>,
    direction: Option<MirDirection>,
}

#[derive(Debug, Clone)]
struct PendingNativeMonsterHit {
    ready_at_ms: u64,
    session_id: SessionId,
    attacker_object_id: u32,
    object_id: u32,
    damage: i32,
}

#[derive(Debug, Clone)]
struct PendingNativeProjectile {
    ready_at_ms: u64,
    session_id: SessionId,
    spell: Spell,
    source_id: u32,
    destination_id: u32,
}

#[derive(Debug, Clone)]
struct PendingNativePlayerHit {
    ready_at_ms: u64,
    attacker_object_id: u32,
    attacker_ai: u8,
    target_session_id: SessionId,
    target_object_id: u32,
    damage: i32,
    /// Whether the incoming hit is magical (mitigated by MAC) or physical (AC).
    magic: bool,
}

#[derive(Debug, Clone)]
struct PendingNativePlayerHeal {
    ready_at_ms: u64,
    session_id: SessionId,
    amount: i32,
}

#[derive(Debug, Clone)]
struct PendingNativeSummon {
    ready_at_ms: u64,
    owner_session_id: SessionId,
    owner_object_id: u32,
    monster_name: &'static str,
    max_summons: usize,
    limit_scope: NativeSummonLimitScope,
    expires_at_ms: Option<u64>,
    skill_level: u8,
    position: Point,
    direction: MirDirection,
}

#[derive(Debug, Clone)]
struct PendingNativeGroundSpellAction {
    spell: Spell,
    caster_session_id: SessionId,
    caster_object_id: u32,
    locations: Vec<Point>,
    spell_object_ids: Vec<u32>,
    direction: MirDirection,
    spell_param: bool,
    damage: i32,
    spell_packets_at_ms: u64,
    spell_packets_sent: bool,
    next_damage_at_ms: u64,
    expires_at_ms: u64,
    tick_interval_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct ZoneNativeMonsterPlayerStatus {
    poison: u16,
    duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeSummonLimitScope {
    SameMonster,
    AllOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeSummonRecallMode {
    None,
    Player,
    Target,
}

#[derive(Debug, Clone, Copy)]
struct NativeSummonSpellProfile {
    monster_name: &'static str,
    base_delay_ms: u64,
    max_summons: usize,
    limit_scope: NativeSummonLimitScope,
    spawn_at_target: bool,
    projectile_delay: bool,
    recall_mode: NativeSummonRecallMode,
    expires_base_ms: Option<u64>,
    expires_per_level_ms: u64,
}

#[derive(Debug, Clone)]
struct NativeMonsterTarget {
    session_id: SessionId,
    object_id: u32,
    position: Point,
}

#[derive(Debug, Clone)]
struct NativeSummonTarget {
    object_id: u32,
    position: Point,
}

#[derive(Debug, Clone)]
struct NativeMonsterDecoyTarget {
    object_id: u32,
    position: Point,
}

impl ZoneRuntime {
    pub fn new(key: ZoneKey) -> Self {
        let collision = ZoneCollision::for_map(&key.map_file_name);
        Self::new_with_collision(key, collision)
    }

    /// L2 step-1 invariant: the ECS player mirror must exactly match the
    /// authoritative `players` map (same sessions, object ids, and positions).
    /// Returns true when they agree. Test-only; the mirror is not yet read by
    /// gameplay. See docs/L2-ECS-ZONE-DESIGN.md.
    #[cfg(test)]
    pub(crate) fn ecs_mirror_matches_players(&mut self) -> bool {
        let mut expected: Vec<(SessionId, u32, i32, i32)> = self
            .players
            .iter()
            .map(|(session_id, player)| {
                (
                    session_id.clone(),
                    player.object_id,
                    player.position.x,
                    player.position.y,
                )
            })
            .collect();
        expected.sort();
        self.ecs.mirror_snapshot() == expected
    }

    pub fn new_with_collision(key: ZoneKey, collision: ZoneCollision) -> Self {
        Self {
            key,
            collision,
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            dead_object_ids: BTreeMap::new(),
            revived_object_ids: BTreeSet::new(),
            removed_object_ids: BTreeSet::new(),
            harvested_object_ids: BTreeSet::new(),
            native_monsters: BTreeMap::new(),
            pending_native_hits: Vec::new(),
            pending_native_projectiles: Vec::new(),
            pending_native_player_hits: Vec::new(),
            pending_native_player_heals: Vec::new(),
            pending_native_summons: Vec::new(),
            pending_native_ground_spells: Vec::new(),
            ground_drops: BTreeMap::new(),
            claimed_ground_drops: BTreeMap::new(),
            occupancy: BTreeMap::new(),
            open_doors: BTreeMap::new(),
            hazard: ZoneHazardState::default(),
            // Cells are sized to the AOI range so the 3x3 neighborhood of any
            // point is a superset of everything within AOI range.
            player_grid: AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE),
            object_grid: AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE),
            ecs: ZoneEcs::new(),
            next_object_id: 1_000_000,
        }
    }

    pub fn key(&self) -> &ZoneKey {
        &self.key
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn ground_drop_count(&self) -> usize {
        self.ground_drops.len()
    }

    pub fn has_ground_drop(&self, object_id: u32) -> bool {
        self.ground_drops.contains_key(&object_id)
    }

    pub fn player_object_id(&self, session_id: &SessionId) -> Option<PlayerId> {
        self.players
            .get(session_id)
            .map(|player| PlayerId(player.object_id))
    }

    pub fn player_position(&self, session_id: &SessionId) -> Option<Point> {
        self.players
            .get(session_id)
            .map(|player| player.position.clone())
    }

    pub fn player_direction(&self, session_id: &SessionId) -> Option<MirDirection> {
        self.players.get(session_id).map(|player| player.direction)
    }

    pub fn player_identity(&self, session_id: &SessionId) -> Option<(SessionId, String, i32)> {
        self.players.get(session_id).map(|player| {
            (
                player.session_id.clone(),
                player.account_id.clone(),
                player.character_index,
            )
        })
    }

    pub fn handle(&mut self, command: ZoneCommand) -> Vec<ZoneOutbound> {
        match command {
            ZoneCommand::Join(join) => self.join(join),
            ZoneCommand::Leave { session_id } => self.leave(&session_id),
            ZoneCommand::Walk {
                session_id,
                direction,
                seq,
                now_ms,
            } => self.queue_movement_action(
                session_id,
                ZoneMovementAction {
                    kind: ZoneMovementActionKind::Walk,
                    direction,
                    seq: Some(seq),
                    received_at_ms: now_ms,
                },
            ),
            ZoneCommand::Run {
                session_id,
                direction,
                seq,
                now_ms,
            } => self.queue_movement_action(
                session_id,
                ZoneMovementAction {
                    kind: ZoneMovementActionKind::Run,
                    direction,
                    seq: Some(seq),
                    received_at_ms: now_ms,
                },
            ),
            ZoneCommand::Turn {
                session_id,
                direction,
                now_ms,
            } => self.queue_movement_action(
                session_id,
                ZoneMovementAction {
                    kind: ZoneMovementActionKind::Turn,
                    direction,
                    seq: None,
                    received_at_ms: now_ms,
                },
            ),
            ZoneCommand::UpdateChatProfile {
                session_id,
                profile,
            } => self.update_chat_profile(&session_id, profile),
            ZoneCommand::UpdatePlayerCombatStats { session_id, stats } => {
                self.update_player_combat_stats(&session_id, stats)
            }
            ZoneCommand::SyncPlayerTransform {
                session_id,
                position,
                direction,
            } => self.sync_player_transform(&session_id, position, direction),
            ZoneCommand::Chat {
                session_id,
                message,
                linked_items,
                linked_user_items,
                now_ms,
            } => self.chat(
                &session_id,
                &message,
                &linked_items,
                &linked_user_items,
                now_ms,
            ),
            ZoneCommand::BroadcastPackets {
                session_id,
                owner_local_object_id,
                packets,
                now_ms,
            } => self.broadcast_packets(&session_id, owner_local_object_id, &packets, now_ms),
            ZoneCommand::SyncSharedObjects {
                session_id,
                packets,
                now_ms,
            } => self.sync_shared_objects(&session_id, &packets, now_ms),
            ZoneCommand::BroadcastSharedObjectPackets {
                session_id,
                local_self_object_id,
                packets,
                now_ms,
            } => self.broadcast_shared_object_packets(
                &session_id,
                local_self_object_id,
                &packets,
                now_ms,
            ),
            ZoneCommand::SyncGroundDrops {
                session_id,
                drops,
                now_ms,
            } => self.sync_ground_drops(&session_id, &drops, now_ms),
            ZoneCommand::SpawnMonster {
                session_id,
                monster,
                now_ms,
            } => self.spawn_native_monster(&session_id, &monster, now_ms),
            ZoneCommand::PlayerAttackObject {
                session_id,
                object_id,
                direction,
                spell,
                level,
                attack_type,
                damage,
                now_ms,
            } => self.player_attack_native_object(
                &session_id,
                object_id,
                direction,
                spell,
                level,
                attack_type,
                damage,
                now_ms,
            ),
            ZoneCommand::PlayerRangeAttackObject {
                session_id,
                object_id,
                direction,
                target,
                spell,
                level,
                attack_type,
                damage,
                now_ms,
            } => self.player_range_attack_native_object(
                &session_id,
                object_id,
                direction,
                target,
                spell,
                level,
                attack_type,
                damage,
                now_ms,
            ),
            ZoneCommand::PlayerCastMagic {
                session_id,
                object_id,
                spell,
                direction,
                target,
                cast,
                level,
                damage,
                mp_cost,
                cooldown_ms,
                now_ms,
            } => self.player_cast_native_magic(
                &session_id,
                object_id,
                spell,
                direction,
                target,
                cast,
                level,
                damage,
                mp_cost,
                cooldown_ms,
                now_ms,
            ),
            ZoneCommand::ClaimGroundDrop {
                session_id,
                object_id,
                target,
                group_members,
                now_ms,
            } => self.claim_ground_drop(&session_id, object_id, &target, &group_members, now_ms),
            ZoneCommand::ClaimNearestGroundDrop {
                session_id,
                origin,
                max_range,
                allowed_object_ids,
                group_members,
                now_ms,
            } => self.claim_nearest_ground_drop(
                &session_id,
                &origin,
                max_range,
                &allowed_object_ids,
                &group_members,
                now_ms,
            ),
            ZoneCommand::CommitGroundDropClaim {
                session_id,
                object_id,
            } => self.commit_ground_drop_claim(&session_id, object_id),
            ZoneCommand::CancelGroundDropClaim {
                session_id,
                object_id,
                now_ms,
            } => self.cancel_ground_drop_claim(&session_id, object_id, now_ms),
            ZoneCommand::TickPlayerMovement { session_id, now_ms } => {
                self.tick_player_movement(&session_id, now_ms)
            }
            ZoneCommand::OpenDoor {
                session_id,
                door_index,
                now_ms,
            } => self.open_door(&session_id, door_index, now_ms),
            ZoneCommand::ConfigureHazards {
                lightning,
                fire,
                lightning_damage,
                fire_damage,
                ..
            } => {
                self.configure_hazards(lightning, fire, lightning_damage, fire_damage);
                Vec::new()
            }
            ZoneCommand::Tick { now_ms } => self.tick(now_ms),
        }
    }

    pub fn can_player_cast_magic(
        &self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: &Point,
        cast: bool,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        if cast && now_ms < player.next_spell_ready_at_ms {
            return false;
        }
        if object_id == 0 && zone_magic_targets_ground_point(spell) {
            return self.can_player_cast_native_ground_magic(
                session_id,
                spell,
                direction,
                target,
                cast,
                damage,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }
        if zone_magic_targets_summon(spell) {
            return self.can_player_cast_native_summon_magic(
                session_id,
                object_id,
                spell,
                direction,
                target,
                cast,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }
        if zone_magic_targets_pet_buff(spell) {
            return self.can_player_cast_native_pet_magic(
                session_id,
                object_id,
                spell,
                target,
                cast,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }
        if zone_magic_targets_self(spell) {
            return self.can_player_cast_native_self_magic(
                session_id,
                object_id,
                spell,
                target,
                cast,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }

        let Some(monster) = self.native_monsters.get(&object_id) else {
            return false;
        };
        if monster.dead || monster.hp <= 0 || !monster.hostile_to_player {
            return false;
        }
        if !points_within_action_range(&player.position, target, ZONE_NATIVE_PLAYER_MAGIC_MAX) {
            return false;
        }
        if cast {
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost.max(0) || now_ms < ready_at_ms {
                return false;
            }
        }
        if zone_magic_uses_ground_spell(spell) {
            return !self
                .native_ground_spell_locations(spell, target, &player.position, direction)
                .is_empty()
                && (damage > 0 || matches!(spell, Spell::Trap | Spell::TrapHexagon));
        }
        true
    }

    pub fn player_cast_magic_requires_item_consumption(
        &self,
        session_id: &SessionId,
        spell: Spell,
    ) -> bool {
        let Some(profile) = native_summon_spell_profile(spell) else {
            return true;
        };
        let Some(player) = self.players.get(session_id) else {
            return true;
        };
        profile.recall_mode == NativeSummonRecallMode::None
            || self
                .active_native_summon_object_id(player.object_id, profile.monster_name)
                .is_none()
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut outbounds = self.expire_zone_player_status_poisons(now_ms);
        let ready_sessions = self
            .players
            .iter()
            .filter_map(|(session_id, player)| {
                Self::movement_action_ready(player, now_ms).then(|| session_id.clone())
            })
            .collect::<Vec<_>>();

        for session_id in ready_sessions {
            outbounds.extend(self.tick_player_movement(&session_id, now_ms));
        }
        outbounds.extend(self.resolve_pending_native_projectiles(now_ms));
        outbounds.extend(self.resolve_pending_native_monster_hits(now_ms));
        outbounds.extend(self.resolve_pending_native_player_hits(now_ms));
        outbounds.extend(self.resolve_pending_native_player_heals(now_ms));
        outbounds.extend(self.resolve_pending_native_summons(now_ms));
        outbounds.extend(self.expire_native_monster_controls(now_ms));
        outbounds.extend(self.tick_native_monster_damage_poisons(now_ms));
        outbounds.extend(self.tick_native_ground_spells(now_ms));
        outbounds.extend(self.expire_native_snake_summons(now_ms));
        outbounds.extend(self.expire_native_vampire_spiders(now_ms));
        outbounds.extend(self.tick_native_monsters(now_ms));
        outbounds.extend(self.tick_doors(now_ms));
        outbounds.extend(self.tick_hazards(now_ms));
        outbounds.extend(self.expire_buffs(now_ms));
        self.expire_ground_drop_ownerships(now_ms);
        outbounds.extend(self.expire_zone_objects(now_ms));
        outbounds
    }

    /// Open a shared door (Crystal `Map.OpenDoor`): unblock its cells for every
    /// player on the map, schedule the 5 s auto-close, and broadcast the open.
    /// Re-opening refreshes the timer and re-broadcasts, matching Crystal.
    fn open_door(
        &mut self,
        _session_id: &SessionId,
        door_index: u8,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let door_index = door_index & 0x7F;
        if !self.collision.doors().contains_key(&door_index) {
            return Vec::new();
        }
        self.collision.open_door(door_index);
        self.open_doors
            .insert(door_index, now_ms.saturating_add(ZONE_DOOR_OPEN_MS));
        vec![ZoneOutbound::ToAll {
            packets: vec![ServerPacket::OpenDoor {
                door_index,
                close: false,
            }],
        }]
    }

    /// Auto-close shared doors whose timer elapsed (Crystal `Map.Process`).
    fn tick_doors(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let closing: Vec<u8> = self
            .open_doors
            .iter()
            .filter_map(|(index, close_at)| (now_ms >= *close_at).then_some(*index))
            .collect();
        if closing.is_empty() {
            return Vec::new();
        }
        let mut packets = Vec::new();
        for index in closing {
            self.collision.close_door(index);
            self.open_doors.remove(&index);
            packets.push(ServerPacket::OpenDoor {
                door_index: index,
                close: true,
            });
        }
        vec![ZoneOutbound::ToAll { packets }]
    }

    /// Set the map's shared hazard flags (Crystal `MapInfo.Lightning/Fire`).
    fn configure_hazards(
        &mut self,
        lightning: bool,
        fire: bool,
        lightning_damage: i32,
        fire_damage: i32,
    ) {
        self.hazard.lightning = lightning;
        self.hazard.fire = fire;
        self.hazard.lightning_damage = lightning_damage;
        self.hazard.fire_damage = fire_damage;
    }

    /// Drive shared lightning/fire strikes (Crystal `Map.Process`). Each hazard
    /// fires every 3–15 s; for every player on the map a 1-in-4 strike lands on
    /// them for authoritative damage, the rest hit a nearby cell (visual).
    fn tick_hazards(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut outbounds = Vec::new();
        if self.hazard.lightning && now_ms >= self.hazard.next_lightning_ms {
            let strike_index = self.hazard.lightning_strikes;
            self.hazard.lightning_strikes = strike_index.wrapping_add(1);
            self.hazard.next_lightning_ms =
                now_ms.saturating_add(zone_hazard_interval_ms(strike_index, 0xA1));
            let damage = self.hazard.lightning_damage;
            outbounds.extend(self.hazard_strike(Spell::MapLightning, damage, strike_index, 0x11));
        }
        if self.hazard.fire && now_ms >= self.hazard.next_fire_ms {
            let strike_index = self.hazard.fire_strikes;
            self.hazard.fire_strikes = strike_index.wrapping_add(1);
            self.hazard.next_fire_ms =
                now_ms.saturating_add(zone_hazard_interval_ms(strike_index, 0xF1));
            let damage = self.hazard.fire_damage;
            outbounds.extend(self.hazard_strike(Spell::MapLava, damage, strike_index, 0x22));
        }
        outbounds
    }

    fn hazard_strike(
        &mut self,
        spell: Spell,
        max_damage: i32,
        strike_index: u64,
        salt: u64,
    ) -> Vec<ZoneOutbound> {
        let targets: Vec<(SessionId, u32, Point)> = self
            .players
            .iter()
            .filter(|(_, player)| !player.dead)
            .map(|(session_id, player)| {
                (
                    session_id.clone(),
                    player.object_id,
                    player.position.clone(),
                )
            })
            .collect();

        let mut outbounds = Vec::new();
        for (player_index, (session_id, object_id, position)) in targets.into_iter().enumerate() {
            let on_player = strike_index.wrapping_add(player_index as u64) % 4 == 0;
            let location = if on_player {
                position.clone()
            } else {
                let dx =
                    (zone_hazard_hash(strike_index, salt ^ u64::from(object_id)) % 21) as i32 - 10;
                let dy =
                    (zone_hazard_hash(strike_index, salt.wrapping_mul(7) ^ u64::from(object_id))
                        % 21) as i32
                        - 10;
                Point {
                    x: position.x + dx,
                    y: position.y + dy,
                }
            };
            if self.collision.is_blocked(&location) {
                continue;
            }
            let spell_object_id = self.unique_object_id(0);
            outbounds.push(ZoneOutbound::ToAll {
                packets: vec![ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: spell_object_id,
                        location: location.clone(),
                        spell,
                        direction: MirDirection::Up,
                        param: false,
                    },
                }],
            });
            if location == position {
                outbounds.extend(self.apply_hazard_damage(
                    &session_id,
                    object_id,
                    max_damage,
                    strike_index,
                    salt,
                ));
            }
        }
        outbounds
    }

    fn apply_hazard_damage(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        max_damage: i32,
        strike_index: u64,
        salt: u64,
    ) -> Vec<ZoneOutbound> {
        let modulo = max_damage.max(1) as u64;
        let damage = (zone_hazard_hash(strike_index, salt) % modulo) as i32;
        let (position, health_percent) = {
            let Some(player) = self.players.get_mut(session_id) else {
                return Vec::new();
            };
            if player.dead {
                return Vec::new();
            }
            let damage = damage.min(player.hp.saturating_sub(1));
            if damage <= 0 {
                return Vec::new();
            }
            player.hp = player.hp.saturating_sub(damage).max(1);
            (
                player.position.clone(),
                native_player_health_percent(player.hp, player.max_hp),
            )
        };
        let recipients = self.native_monster_visible_recipients(object_id, &position);
        let mut outbounds = Vec::new();
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id,
                        percent: health_percent,
                        expire: 0,
                    },
                }],
            });
        }
        outbounds.push(ZoneOutbound::PlayerDamaged {
            session_id: session_id.clone(),
            damage,
        });
        outbounds
    }

    fn tick_player_movement(&mut self, session_id: &SessionId, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut outbounds = Vec::new();
        let mut remaining = ZONE_MOVEMENT_ACTION_QUEUE_LIMIT;
        while remaining > 0 && self.player_movement_action_ready(session_id, now_ms) {
            let before_len = self
                .players
                .get(session_id)
                .map(|player| player.movement_actions.len())
                .unwrap_or_default();
            outbounds.extend(self.consume_movement_action(session_id, now_ms));
            remaining -= 1;

            let Some(player) = self.players.get(session_id) else {
                break;
            };
            if player.movement_actions.len() >= before_len
                || !Self::movement_action_ready(player, now_ms)
            {
                break;
            }
        }
        outbounds
    }

    fn join(&mut self, mut join: ZoneJoin) -> Vec<ZoneOutbound> {
        let mut outbounds = Vec::new();
        if self.players.contains_key(&join.session_id) {
            outbounds.extend(self.leave(&join.session_id));
        }

        let object_id = self.unique_object_id(join.object_id);
        let requested_position = join.position.clone();
        let requested_direction = join.direction;
        join.position = self.first_available_position(join.position.clone(), None);
        let session_id = join.session_id.clone();
        let tile = tile_key(&join.position);
        let player = ZonePlayer::from_join(join, object_id);

        self.occupancy.insert(tile, session_id.clone());
        self.player_grid
            .insert(session_id.clone(), &player.position);
        self.ecs
            .upsert_player(&session_id, object_id, &player.position);
        self.players.insert(session_id.clone(), player);
        outbounds.extend(self.diff_visibility_for(&session_id));
        outbounds.extend(self.diff_zone_object_visibility_for(&session_id));
        let player = self
            .players
            .get(&session_id)
            .expect("joined player should exist");
        if player.position != requested_position || player.direction != requested_direction {
            outbounds.push(ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets: vec![user_location_packet(player)],
            });
        }
        outbounds.push(ZoneOutbound::SaveTransform {
            session_id,
            position: player.position.clone(),
            direction: player.direction,
        });
        outbounds
    }

    fn leave(&mut self, session_id: &SessionId) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.remove(session_id) else {
            return Vec::new();
        };
        self.pending_native_hits
            .retain(|hit| &hit.session_id != session_id);
        self.pending_native_player_hits
            .retain(|hit| &hit.target_session_id != session_id);
        self.occupancy.remove(&tile_key(&player.position));
        self.player_grid.remove(session_id);
        self.ecs.remove_player(session_id);

        let mut observers = Vec::new();
        for (other_session_id, other) in &mut self.players {
            if other.visible_object_ids.remove(&player.object_id) {
                observers.push(other_session_id.clone());
            }
            other.visible_object_ids.remove(&player.object_id);
        }

        let mut outbounds = Vec::new();
        if !observers.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![ServerPacket::ObjectRemove {
                    object_id: player.object_id,
                }],
            });
        }
        outbounds.extend(self.remove_owner_generated_zone_objects(player.object_id, &player.name));
        outbounds.push(ZoneOutbound::SaveTransform {
            session_id: session_id.clone(),
            position: player.position,
            direction: player.direction,
        });
        outbounds
    }

    fn remove_owner_generated_zone_objects(
        &mut self,
        owner_object_id: u32,
        owner_name: &str,
    ) -> Vec<ZoneOutbound> {
        let object_ids = self
            .objects
            .values()
            .filter(|object| retained_zone_object_owned_by(object, owner_object_id, owner_name))
            .map(|object| object.object_id)
            .collect::<Vec<_>>();
        let mut outbounds = Vec::new();
        for object_id in object_ids {
            self.objects.remove(&object_id);
            self.object_grid.remove(&object_id);
            self.native_monsters.remove(&object_id);
            self.dead_object_ids.remove(&object_id);
            self.revived_object_ids.remove(&object_id);
            self.harvested_object_ids.remove(&object_id);
            self.removed_object_ids.insert(object_id);
            let observers = self
                .players
                .iter_mut()
                .filter_map(|(session_id, player)| {
                    player
                        .visible_object_ids
                        .remove(&object_id)
                        .then(|| session_id.clone())
                })
                .collect::<Vec<_>>();
            if !observers.is_empty() {
                outbounds.push(ZoneOutbound::ToMany {
                    session_ids: observers,
                    packets: vec![ServerPacket::ObjectRemove { object_id }],
                });
            }
        }
        outbounds
    }

    fn movement_action_ready(player: &ZonePlayer, now_ms: u64) -> bool {
        !player.movement_actions.is_empty()
            && now_ms.saturating_add(ZONE_MOVE_READY_GRACE_MS) >= player.movement_ready_at_ms
    }

    fn player_movement_action_ready(&self, session_id: &SessionId, now_ms: u64) -> bool {
        self.players
            .get(session_id)
            .is_some_and(|player| Self::movement_action_ready(player, now_ms))
    }

    fn queue_movement_action(
        &mut self,
        session_id: SessionId,
        action: ZoneMovementAction,
    ) -> Vec<ZoneOutbound> {
        let replaces_pending_step =
            self.incoming_replaces_pending_movement_step(&session_id, &action);
        let mut outbounds = if !replaces_pending_step
            && self.player_movement_action_ready(&session_id, action.received_at_ms)
        {
            self.tick_player_movement(&session_id, action.received_at_ms)
        } else if !replaces_pending_step {
            if let Some(consume_at_ms) =
                self.buffered_movement_consume_at(&session_id, action.received_at_ms)
            {
                self.tick_player_movement(&session_id, consume_at_ms)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let Some(player) = self.players.get_mut(&session_id) else {
            return outbounds;
        };
        if let Some(seq) = action.seq {
            if seq <= player.last_seen_move_seq {
                return outbounds;
            }
            player.last_seen_move_seq = seq;
        }
        if matches!(
            action.kind,
            ZoneMovementActionKind::Walk | ZoneMovementActionKind::Run
        ) {
            player
                .movement_actions
                .retain(|pending| matches!(pending.kind, ZoneMovementActionKind::Turn));
        }
        while player.movement_actions.len() >= 2 {
            player.movement_actions.pop_back();
        }
        player.movement_actions.push_back(action);
        if let Some(consume_at_ms) =
            self.buffered_movement_consume_at(&session_id, action.received_at_ms)
        {
            outbounds.extend(self.tick_player_movement(&session_id, consume_at_ms));
        }
        outbounds
    }

    fn incoming_replaces_pending_movement_step(
        &self,
        session_id: &SessionId,
        action: &ZoneMovementAction,
    ) -> bool {
        if !matches!(
            action.kind,
            ZoneMovementActionKind::Walk | ZoneMovementActionKind::Run
        ) {
            return false;
        }
        let Some(incoming_seq) = action.seq else {
            return false;
        };
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        let Some(pending) = player.movement_actions.front() else {
            return false;
        };
        let Some(pending_seq) = pending.seq else {
            return false;
        };
        matches!(
            pending.kind,
            ZoneMovementActionKind::Walk | ZoneMovementActionKind::Run
        ) && incoming_seq > pending_seq
            && action.received_at_ms
                <= pending
                    .received_at_ms
                    .saturating_add(ZONE_MOVEMENT_INPUT_BUFFER_MS)
    }

    fn buffered_movement_consume_at(
        &self,
        session_id: &SessionId,
        received_at_ms: u64,
    ) -> Option<u64> {
        let player = self.players.get(session_id)?;
        let action = player.movement_actions.front()?;
        if !matches!(
            action.kind,
            ZoneMovementActionKind::Walk | ZoneMovementActionKind::Run
        ) {
            return None;
        }
        let ready_at_ms = player.movement_ready_at_ms;
        (ready_at_ms > received_at_ms
            && received_at_ms.saturating_add(ZONE_MOVEMENT_INPUT_BUFFER_MS) >= ready_at_ms)
            .then_some(ready_at_ms)
    }

    fn consume_movement_action(
        &mut self,
        session_id: &SessionId,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(action) = self
            .players
            .get_mut(session_id)
            .and_then(|player| player.movement_actions.pop_front())
        else {
            return Vec::new();
        };

        match action.kind {
            ZoneMovementActionKind::Turn => self.consume_turn_action(session_id, action, now_ms),
            ZoneMovementActionKind::Walk | ZoneMovementActionKind::Run => {
                self.consume_step_action(session_id, action, now_ms)
            }
        }
    }

    fn consume_step_action(
        &mut self,
        session_id: &SessionId,
        action: ZoneMovementAction,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let running = action.kind == ZoneMovementActionKind::Run;
        let (origin, can_move, can_run) = {
            let player = self
                .players
                .get(session_id)
                .expect("action owner should still exist");
            (
                player.position.clone(),
                !player.dead && !zone_player_status_blocks_movement(player, now_ms),
                !player.dead
                    && (player.run_step_until_ms == 0
                        || player.run_step_until_ms >= now_ms
                        || player.run_step_until_ms >= action.received_at_ms),
            )
        };
        if !can_move {
            return self.correct_player_location(session_id, now_ms);
        }

        let effective_running = running && can_run;
        let steps = if effective_running { 2 } else { 1 };
        let path = (1..=steps)
            .map(|amount| offset_point(&origin, action.direction, amount))
            .collect::<Vec<_>>();

        let blocked = path
            .iter()
            .any(|point| !self.can_player_movement_occupy(point, Some(session_id)));
        if blocked {
            let player = self
                .players
                .get_mut(session_id)
                .expect("action owner should still exist");
            if running {
                player.direction = action.direction;
            }
            player.movement_ready_at_ms = now_ms;
            return vec![ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets: vec![user_location_packet(player)],
            }];
        }

        let destination = path
            .last()
            .cloned()
            .expect("movement path should contain at least one point");
        {
            let player = self
                .players
                .get_mut(session_id)
                .expect("action owner should still exist");
            self.occupancy.remove(&tile_key(&player.position));
            player.position = destination;
            player.direction = action.direction;
            player.movement_ready_at_ms =
                now_ms.saturating_add(zone_player_move_delay_ms(player, effective_running, now_ms));
            player.run_step_until_ms = now_ms.saturating_add(ZONE_RUN_GRACE_MS);
            self.occupancy
                .insert(tile_key(&player.position), session_id.clone());
            let moved_position = player.position.clone();
            self.player_grid.moved(session_id, &moved_position);
            self.ecs.move_player(session_id, &moved_position);
        }

        let mut outbounds = Vec::new();
        outbounds.extend(self.diff_visibility_for(session_id));
        outbounds.extend(self.diff_zone_object_visibility_for(session_id));

        let player = self
            .players
            .get(session_id)
            .expect("action owner should still exist");
        outbounds.push(ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: vec![user_location_packet(player)],
        });

        let observers = self
            .players
            .iter()
            .filter_map(|(other_session_id, other)| {
                (other_session_id != session_id
                    && other.visible_object_ids.contains(&player.object_id))
                .then(|| other_session_id.clone())
            })
            .collect::<Vec<_>>();
        if !observers.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![if effective_running {
                    object_run_packet(player)
                } else {
                    object_walk_packet(player)
                }],
            });
        }
        outbounds.push(ZoneOutbound::SaveTransform {
            session_id: session_id.clone(),
            position: player.position.clone(),
            direction: player.direction,
        });
        outbounds
    }

    fn consume_turn_action(
        &mut self,
        session_id: &SessionId,
        action: ZoneMovementAction,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        if player.dead {
            return vec![ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets: vec![user_location_packet(player)],
            }];
        }
        player.direction = action.direction;
        player.movement_ready_at_ms = now_ms.saturating_add(ZONE_TURN_DELAY_MS);
        player.run_step_until_ms = 0;
        let player = self
            .players
            .get(session_id)
            .expect("turning player should still exist");
        let observers = self
            .players
            .iter()
            .filter_map(|(other_session_id, other)| {
                (other_session_id != session_id
                    && other.visible_object_ids.contains(&player.object_id))
                .then(|| other_session_id.clone())
            })
            .collect::<Vec<_>>();
        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: vec![user_location_packet(player)],
        }];
        if !observers.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![object_turn_packet(player)],
            });
        }
        outbounds.push(ZoneOutbound::SaveTransform {
            session_id: session_id.clone(),
            position: player.position.clone(),
            direction: player.direction,
        });
        outbounds
    }

    fn correct_player_location(
        &mut self,
        session_id: &SessionId,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        player.movement_ready_at_ms = now_ms;
        vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: vec![user_location_packet(player)],
        }]
    }

    fn expire_zone_player_status_poisons(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let expired_sessions = self
            .players
            .iter()
            .filter_map(|(session_id, player)| {
                (player.native_status_poison != 0
                    && player
                        .native_status_poison_expires_at_ms
                        .is_some_and(|expires_at_ms| now_ms >= expires_at_ms))
                .then(|| session_id.clone())
            })
            .collect::<Vec<_>>();
        let mut outbounds = Vec::new();
        for session_id in expired_sessions {
            let Some((object_id, position, poison)) =
                self.players.get_mut(&session_id).map(|player| {
                    player.poison &= !player.native_status_poison;
                    player.native_status_poison = 0;
                    player.native_status_poison_expires_at_ms = None;
                    (player.object_id, player.position.clone(), player.poison)
                })
            else {
                continue;
            };
            let recipients = self.player_status_recipients(object_id, &position);
            if recipients.is_empty() {
                continue;
            }
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![ServerPacket::ObjectPoisoned { object_id, poison }],
            });
        }
        outbounds
    }

    fn sync_player_transform(
        &mut self,
        session_id: &SessionId,
        position: Point,
        direction: MirDirection,
    ) -> Vec<ZoneOutbound> {
        if !self.players.contains_key(session_id) {
            return Vec::new();
        }

        let destination = self.first_available_position(position, Some(session_id));
        let already_synced = self
            .players
            .get(session_id)
            .is_some_and(|player| player.position == destination && player.direction == direction);
        if already_synced {
            return Vec::new();
        }

        {
            let player = self
                .players
                .get_mut(session_id)
                .expect("sync transform player should exist");
            self.occupancy.remove(&tile_key(&player.position));
            player.position = destination;
            player.direction = direction;
            player.movement_actions.clear();
            player.movement_ready_at_ms = 0;
            player.run_step_until_ms = 0;
            self.occupancy
                .insert(tile_key(&player.position), session_id.clone());
            let moved_position = player.position.clone();
            self.player_grid.moved(session_id, &moved_position);
            self.ecs.move_player(session_id, &moved_position);
        }

        let mut outbounds = self.diff_visibility_for(session_id);
        outbounds.extend(self.diff_zone_object_visibility_for(session_id));
        let player = self
            .players
            .get(session_id)
            .expect("sync transform player should still exist");
        outbounds.push(ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: vec![user_location_packet(player)],
        });
        outbounds.push(ZoneOutbound::SaveTransform {
            session_id: session_id.clone(),
            position: player.position.clone(),
            direction: player.direction,
        });
        outbounds
    }

    fn update_chat_profile(
        &mut self,
        session_id: &SessionId,
        profile: super::types::ZoneChatProfile,
    ) -> Vec<ZoneOutbound> {
        if let Some(player) = self.players.get_mut(session_id) {
            player.chat_profile = profile;
        }
        Vec::new()
    }

    fn update_player_combat_stats(
        &mut self,
        session_id: &SessionId,
        stats: super::types::ZonePlayerCombatStats,
    ) -> Vec<ZoneOutbound> {
        if let Some(player) = self.players.get_mut(session_id) {
            player.combat_stats = stats;
        }
        Vec::new()
    }

    fn chat(
        &mut self,
        session_id: &SessionId,
        message: &str,
        linked_items: &[ChatItem],
        linked_user_items: &[UserItem],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id).cloned() else {
            return Vec::new();
        };

        if message.is_empty() {
            return Vec::new();
        }

        let message = chat_text_with_linked_items(message, linked_items);
        let message = message.replace(
            "$pos",
            &format!("{}, {}", player.position.x, player.position.y),
        );

        if let Some(rest) = message.strip_prefix('/') {
            return self.whisper_chat(&player, rest, linked_user_items);
        }
        if let Some(rest) = message.strip_prefix("!!") {
            return self.group_chat(&player, rest, linked_user_items);
        }
        if let Some(rest) = message.strip_prefix("!~") {
            return self.guild_chat(&player, rest, linked_user_items);
        }
        if let Some(rest) = message.strip_prefix("!#") {
            return self.mentor_chat(&player, rest, linked_user_items);
        }
        if let Some(rest) = message
            .strip_prefix("!")
            .filter(|_| !message.starts_with("!!"))
        {
            return self.shout_chat(&player, rest, linked_user_items, now_ms);
        }
        if let Some(rest) = message.strip_prefix(":)") {
            return self.relationship_chat(&player, rest, linked_user_items);
        }
        if let Some(rest) = message.strip_prefix("@!") {
            return self.announcement_chat(&player, rest, linked_user_items);
        }
        if message.starts_with('@') {
            return Vec::new();
        }

        let recipients = self.visible_chat_recipients(&player);
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: packets_with_linked_items(
                linked_user_items,
                vec![object_chat_packet(&player, &message)],
            ),
        }]
    }

    fn visible_chat_recipients(&self, player: &ZonePlayer) -> Vec<SessionId> {
        let mut recipients = vec![player.session_id.clone()];
        recipients.extend(self.players.iter().filter_map(|(other_session_id, other)| {
            (other_session_id != &player.session_id
                && other.visible_object_ids.contains(&player.object_id))
            .then(|| other_session_id.clone())
        }));
        recipients
    }

    fn broadcast_packets(
        &mut self,
        session_id: &SessionId,
        owner_local_object_id: u32,
        packets: &[ServerPacket],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        for packet in packets {
            apply_observer_action_state(player, owner_local_object_id, packet, now_ms);
        }
        let (mut outbounds, rejected_action_transform) =
            self.apply_owner_action_transform(session_id, owner_local_object_id, packets);
        if rejected_action_transform {
            return outbounds;
        }
        let player = self
            .players
            .get(session_id)
            .expect("broadcasting player should still exist")
            .clone();
        let visible_objects_before = self.visible_zone_objects_by_session();
        let object_positions_before = self.zone_object_positions();
        let observer_packets = packets
            .iter()
            .filter_map(|packet| observer_action_packet(&player, owner_local_object_id, packet))
            .filter_map(|packet| self.canonical_observer_zone_object_packet(packet))
            .collect::<Vec<_>>();
        let has_retained_object_packets = observer_packets.iter().any(|packet| {
            self.packet_touches_retained_zone_object(packet, &visible_objects_before)
        });
        self.apply_zone_object_packets(&observer_packets, now_ms);
        if observer_packets.is_empty() {
            return outbounds;
        }
        outbounds.extend(self.dispatch_observer_packets(
            session_id,
            &player,
            &observer_packets,
            &visible_objects_before,
            &object_positions_before,
        ));
        if has_retained_object_packets {
            outbounds.extend(self.diff_all_zone_object_visibility_except(session_id));
        }
        outbounds
    }

    fn sync_shared_objects(
        &mut self,
        session_id: &SessionId,
        packets: &[ServerPacket],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.players.contains_key(session_id) {
            return Vec::new();
        }
        let packets = packets
            .iter()
            .filter(|packet| {
                retained_zone_object_from_packet(packet).is_some_and(|object| {
                    !self.objects.contains_key(&object.object_id)
                        && !self.removed_object_ids.contains(&object.object_id)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        if packets.is_empty() {
            return Vec::new();
        }
        self.apply_zone_object_packets(&packets, now_ms);
        self.diff_all_zone_object_visibility_except(session_id)
    }

    fn sync_ground_drops(
        &mut self,
        session_id: &SessionId,
        drops: &[GroundDropSnapshot],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.players.contains_key(session_id) {
            return Vec::new();
        }
        let mut spawn_packets = Vec::new();
        for drop in drops {
            if self.removed_object_ids.contains(&drop.object_id)
                || self.claimed_ground_drops.contains_key(&drop.object_id)
            {
                continue;
            }
            let owner_expires_at_ms = drop
                .ownership_remaining_ticks
                .filter(|ticks| *ticks > 0)
                .map(|ticks| now_ms.saturating_add(ticks.saturating_mul(300)));
            self.ground_drops
                .entry(drop.object_id)
                .and_modify(|stored| {
                    stored.drop = drop.clone();
                    stored.owner_expires_at_ms = owner_expires_at_ms;
                })
                .or_insert_with(|| ZoneGroundDrop {
                    drop: drop.clone(),
                    owner_expires_at_ms,
                });
            if !self.objects.contains_key(&drop.object_id) {
                spawn_packets.push(ground_drop_spawn_packet(drop));
            }
        }
        if spawn_packets.is_empty() {
            Vec::new()
        } else {
            self.apply_zone_object_packets(&spawn_packets, now_ms);
            self.diff_all_zone_object_visibility_except(session_id)
        }
    }

    fn spawn_native_monster(
        &mut self,
        session_id: &SessionId,
        spawn: &ZoneMonsterSpawn,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.players.contains_key(session_id) {
            return Vec::new();
        }

        let requested_object_id = spawn.object_id;
        if self.native_monsters.contains_key(&requested_object_id) {
            return Vec::new();
        }
        let object_id = if requested_object_id != 0
            && !self
                .players
                .values()
                .any(|player| player.object_id == requested_object_id)
            && !self.ground_drops.contains_key(&requested_object_id)
            && !self.claimed_ground_drops.contains_key(&requested_object_id)
        {
            self.next_object_id = self
                .next_object_id
                .max(requested_object_id.saturating_add(1));
            requested_object_id
        } else {
            self.unique_object_id(requested_object_id)
        };
        let monster = ZoneNativeMonster::from_spawn(spawn, object_id);
        let packet = native_monster_spawn_packet(spawn, object_id);
        self.native_monsters.insert(object_id, monster);
        self.apply_zone_object_packets(&[packet], now_ms);
        self.diff_all_zone_object_visibility()
    }

    fn player_attack_native_object(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        direction: MirDirection,
        spell: u8,
        level: u8,
        attack_type: u8,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id) else {
            return Vec::new();
        };
        if now_ms < player.next_attack_ready_at_ms {
            return self.correct_player_location(session_id, now_ms);
        }

        let Some(monster) = self
            .native_monsters
            .get(&object_id)
            .filter(|monster| !monster.dead && monster.hp > 0)
            .cloned()
        else {
            return Vec::new();
        };
        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        live_player.next_attack_ready_at_ms =
            now_ms.saturating_add(ZONE_NATIVE_PLAYER_ATTACK_ACTION_MS);
        let player = live_player.clone();

        let attack_packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: player.object_id,
                location: player.position.clone(),
                direction,
                spell,
                level,
                attack_type,
            },
        };
        if let Some(resolved_damage) =
            zone_resolve_player_physical_attack(&player, &monster, object_id, damage, now_ms)
        {
            self.pending_native_hits.push(PendingNativeMonsterHit {
                ready_at_ms: now_ms,
                session_id: session_id.clone(),
                attacker_object_id: player.object_id,
                object_id,
                damage: resolved_damage,
            });
        }
        self.apply_zone_object_packets(std::slice::from_ref(&attack_packet), now_ms);
        let recipients = self.native_monster_action_recipients(
            session_id,
            player.object_id,
            object_id,
            &monster.position,
        );
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![attack_packet],
        }]
    }

    fn player_range_attack_native_object(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        direction: MirDirection,
        target: Point,
        spell: Spell,
        level: u8,
        attack_type: u8,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id) else {
            return Vec::new();
        };
        if now_ms < player.next_attack_ready_at_ms {
            return self.correct_player_location(session_id, now_ms);
        }

        let Some(monster) = self
            .native_monsters
            .get(&object_id)
            .filter(|monster| !monster.dead && monster.hp > 0)
            .cloned()
        else {
            return Vec::new();
        };
        if target != monster.position
            || !points_within_action_range(
                &player.position,
                &monster.position,
                ZONE_NATIVE_PLAYER_RANGE_ATTACK_MAX,
            )
        {
            return self.correct_player_location(session_id, now_ms);
        }
        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        live_player.next_attack_ready_at_ms =
            now_ms.saturating_add(ZONE_NATIVE_PLAYER_ATTACK_ACTION_MS);
        let player = live_player.clone();

        let owner_packets = vec![
            user_location_packet(&player),
            ServerPacket::RangeAttack {
                target_id: object_id,
                target: target.clone(),
                spell,
            },
        ];
        let action_packets = vec![
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: player.object_id,
                    location: player.position.clone(),
                    direction,
                    spell: Spell::None as u8,
                    level: 0,
                    attack_type,
                },
            },
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: player.object_id,
                    location: player.position.clone(),
                    direction,
                    target_id: object_id,
                    target,
                    attack_type,
                    spell: spell as u8,
                    level,
                },
            },
        ];
        if let Some(resolved_damage) =
            zone_resolve_player_physical_attack(&player, &monster, object_id, damage, now_ms)
        {
            self.pending_native_hits.push(PendingNativeMonsterHit {
                ready_at_ms: now_ms,
                session_id: session_id.clone(),
                attacker_object_id: player.object_id,
                object_id,
                damage: resolved_damage,
            });
        }
        self.apply_zone_object_packets(&action_packets, now_ms);
        let recipients = self.native_monster_action_recipients(
            session_id,
            player.object_id,
            object_id,
            &monster.position,
        );

        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: owner_packets,
        }];
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: action_packets,
            });
        }
        outbounds
    }

    fn player_cast_native_magic(
        &mut self,
        session_id: &SessionId,
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
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id) else {
            return Vec::new();
        };
        if cast && now_ms < player.next_spell_ready_at_ms {
            return self.correct_player_location(session_id, now_ms);
        }
        // Authoritatively recompute the magic damage from the player's stat block
        // instead of trusting the gateway-supplied scalar. Value-identical to the
        // session formula, so it only relocates authority; PoisonCloud keeps the
        // supplied value because its bonus depends on inventory the zone lacks.
        let damage = if player.combat_stats.has_authoritative_damage()
            && !zone_magic_keeps_supplied_damage(spell)
        {
            zone_authoritative_magic_damage(spell, level, player.combat_stats.max_dc)
                .unwrap_or(damage)
        } else {
            damage
        };
        if object_id == 0 && zone_magic_targets_ground_point(spell) {
            return self.player_cast_native_ground_magic(
                session_id,
                spell,
                direction,
                target,
                cast,
                level,
                damage,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }
        if zone_magic_targets_summon(spell) {
            return self.player_cast_native_summon_magic(
                session_id,
                object_id,
                spell,
                direction,
                target,
                cast,
                level,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }
        if zone_magic_targets_pet_buff(spell) {
            return self.player_cast_native_pet_magic(
                session_id,
                object_id,
                spell,
                direction,
                target,
                cast,
                level,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }
        if zone_magic_targets_self(spell) {
            return self.player_cast_native_self_magic(
                session_id,
                object_id,
                spell,
                direction,
                target,
                cast,
                level,
                damage,
                mp_cost,
                cooldown_ms,
                now_ms,
            );
        }

        let Some(monster) = self
            .native_monsters
            .get(&object_id)
            .filter(|monster| !monster.dead && monster.hp > 0)
            .cloned()
        else {
            return Vec::new();
        };
        if target != monster.position
            || !points_within_action_range(
                &player.position,
                &monster.position,
                ZONE_NATIVE_PLAYER_MAGIC_MAX,
            )
        {
            return self.correct_player_location(session_id, now_ms);
        }
        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        let player = live_player.clone();
        if cast {
            let mp_cost = mp_cost.max(0);
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost || now_ms < ready_at_ms {
                return self.correct_player_location(session_id, now_ms);
            }
            if let Some(live_player) = self.players.get_mut(session_id) {
                live_player.mp = live_player.mp.saturating_sub(mp_cost).max(0);
                live_player.next_spell_ready_at_ms =
                    now_ms.saturating_add(ZONE_NATIVE_PLAYER_SPELL_ACTION_MS);
                live_player
                    .magic_ready_at_ms
                    .insert(spell_key, now_ms.saturating_add(cooldown_ms.max(1)));
            }
        }
        let player = self.players.get(session_id).cloned().unwrap_or(player);

        let target_point = target.clone();
        let secondary_target_ids =
            self.native_magic_secondary_targets(spell, object_id, &target_point);
        let owner_packets = vec![
            user_location_packet(&player),
            ServerPacket::Magic {
                spell,
                target_id: object_id,
                target: target.clone(),
                cast,
                level,
                secondary_target_ids: secondary_target_ids.clone(),
            },
        ];
        let mut action_packets = vec![ServerPacket::ObjectMagic {
            object_id: player.object_id,
            location: player.position.clone(),
            direction,
            spell,
            target_id: object_id,
            target,
            cast,
            level,
            self_broadcast: false,
            secondary_target_ids: secondary_target_ids.clone(),
        }];
        if cast {
            action_packets.push(ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: player.object_id,
                    percent: zone_mana_percent(player.mp),
                },
            });
        }
        if cast && zone_magic_uses_projectile(spell) {
            action_packets.push(ServerPacket::ObjectProjectile {
                spell,
                source_id: player.object_id,
                destination_id: object_id,
            });
        }
        if cast {
            action_packets
                .extend(self.apply_native_monster_magic_control(object_id, spell, level, now_ms));
            action_packets.extend(self.apply_native_monster_magic_damage_poison(
                object_id,
                session_id,
                player.object_id,
                spell,
                level,
                damage,
                now_ms,
            ));
            action_packets.extend(self.apply_native_player_ground_spell(
                session_id,
                object_id,
                player.object_id,
                &player.position,
                player.level,
                spell,
                direction,
                level,
                &target_point,
                zone_player_native_damage(&player, damage),
                now_ms,
            ));
            action_packets.extend(self.apply_native_player_arrow_cast_effects(
                session_id,
                spell,
                level,
                damage,
                &target_point,
                now_ms,
            ));
            self.apply_native_player_fire_bounce_chain(
                session_id,
                object_id,
                player.object_id,
                spell,
                level,
                zone_player_native_damage(&player, damage),
                now_ms,
            );
        }
        if cast && damage > 0 && !zone_magic_uses_ground_spell(spell) {
            let native_damage = zone_player_native_damage(&player, damage);
            for hit_object_id in std::iter::once(object_id).chain(secondary_target_ids.clone()) {
                let hit_damage =
                    zone_magic_hit_damage(spell, hit_object_id != object_id, native_damage);
                // Subtract the struck monster's magic armour, mirroring the AC
                // subtraction the physical path applies. Each secondary target
                // rolls against its own MAC.
                let hit_damage = match self.native_monsters.get(&hit_object_id) {
                    Some(target_monster) => zone_magic_damage_after_monster_armour(
                        target_monster,
                        hit_damage,
                        player.object_id,
                        now_ms,
                    ),
                    None => hit_damage,
                };
                self.pending_native_hits.push(PendingNativeMonsterHit {
                    ready_at_ms: now_ms,
                    session_id: session_id.clone(),
                    attacker_object_id: player.object_id,
                    object_id: hit_object_id,
                    damage: hit_damage,
                });
            }
            if spell == Spell::VampireShot {
                self.pending_native_player_heals
                    .push(PendingNativePlayerHeal {
                        ready_at_ms: now_ms,
                        session_id: session_id.clone(),
                        amount: zone_vampire_shot_heal_amount(native_damage, level),
                    });
            }
        }
        self.apply_zone_object_packets(&action_packets, now_ms);
        let recipients = self.native_monster_action_recipients(
            session_id,
            player.object_id,
            object_id,
            &monster.position,
        );

        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: owner_packets,
        }];
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: action_packets,
            });
        }
        outbounds
    }

    fn player_cast_native_ground_magic(
        &mut self,
        session_id: &SessionId,
        spell: Spell,
        direction: MirDirection,
        target: Point,
        cast: bool,
        level: u8,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id) else {
            return Vec::new();
        };
        if !points_within_action_range(&player.position, &target, ZONE_NATIVE_PLAYER_MAGIC_MAX) {
            return self.correct_player_location(session_id, now_ms);
        }
        if cast {
            let mp_cost = mp_cost.max(0);
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost || now_ms < ready_at_ms {
                return self.correct_player_location(session_id, now_ms);
            }
        }

        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        if cast {
            let mp_cost = mp_cost.max(0);
            let spell_key = spell as u8;
            live_player.mp = live_player.mp.saturating_sub(mp_cost).max(0);
            live_player.next_spell_ready_at_ms =
                now_ms.saturating_add(ZONE_NATIVE_PLAYER_SPELL_ACTION_MS);
            live_player
                .magic_ready_at_ms
                .insert(spell_key, now_ms.saturating_add(cooldown_ms.max(1)));
        }
        let player = live_player.clone();

        let owner_packets = vec![
            user_location_packet(&player),
            ServerPacket::Magic {
                spell,
                target_id: 0,
                target: target.clone(),
                cast,
                level,
                secondary_target_ids: Vec::new(),
            },
        ];
        let mut action_packets = vec![ServerPacket::ObjectMagic {
            object_id: player.object_id,
            location: player.position.clone(),
            direction,
            spell,
            target_id: 0,
            target: target.clone(),
            cast,
            level,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        }];
        if cast {
            action_packets.push(ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: player.object_id,
                    percent: zone_mana_percent(player.mp),
                },
            });
            action_packets.extend(self.apply_native_player_ground_spell(
                session_id,
                0,
                player.object_id,
                &player.position,
                player.level,
                spell,
                direction,
                level,
                &target,
                zone_player_native_damage(&player, damage),
                now_ms,
            ));
        }

        self.apply_zone_object_packets(&action_packets, now_ms);
        let recipients =
            self.ground_spell_visible_recipients(session_id, std::slice::from_ref(&target));
        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: owner_packets,
        }];
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: action_packets,
            });
        }
        outbounds
    }

    fn player_cast_native_summon_magic(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: Point,
        cast: bool,
        level: u8,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.can_player_cast_native_summon_magic(
            session_id,
            object_id,
            spell,
            direction,
            &target,
            cast,
            mp_cost,
            cooldown_ms,
            now_ms,
        ) {
            return self.correct_player_location(session_id, now_ms);
        }

        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        if cast {
            let mp_cost = mp_cost.max(0);
            let spell_key = spell as u8;
            live_player.mp = live_player.mp.saturating_sub(mp_cost).max(0);
            live_player.next_spell_ready_at_ms =
                now_ms.saturating_add(ZONE_NATIVE_PLAYER_SPELL_ACTION_MS);
            live_player
                .magic_ready_at_ms
                .insert(spell_key, now_ms.saturating_add(cooldown_ms.max(1)));
        }
        let player = live_player.clone();
        let Some(profile) = native_summon_spell_profile(spell) else {
            return Vec::new();
        };
        let summon_position =
            self.native_summon_spawn_position(&player.position, direction, &target, profile);
        let mut summon_outbounds = Vec::new();
        if cast {
            if let Some(summon_object_id) =
                self.active_native_summon_object_id(player.object_id, profile.monster_name)
            {
                match profile.recall_mode {
                    NativeSummonRecallMode::None => {}
                    NativeSummonRecallMode::Player => {
                        summon_outbounds.extend(self.recall_native_summon(
                            summon_object_id,
                            &player.position,
                            now_ms,
                        ));
                    }
                    NativeSummonRecallMode::Target => {
                        let recall_position = self.first_available_position(target.clone(), None);
                        summon_outbounds.extend(self.recall_native_summon(
                            summon_object_id,
                            &recall_position,
                            now_ms,
                        ));
                    }
                }
            } else {
                let delay_ms = native_summon_cast_delay_ms(profile, &player.position, &target);
                if self.active_native_summon_count_for_profile(player.object_id, profile)
                    < profile.max_summons
                {
                    let expires_at_ms = native_summon_expires_at_ms(
                        profile,
                        level,
                        now_ms.saturating_add(delay_ms),
                    );
                    self.pending_native_summons.push(PendingNativeSummon {
                        ready_at_ms: now_ms.saturating_add(delay_ms),
                        owner_session_id: session_id.clone(),
                        owner_object_id: player.object_id,
                        monster_name: profile.monster_name,
                        max_summons: profile.max_summons,
                        limit_scope: profile.limit_scope,
                        expires_at_ms,
                        skill_level: level,
                        position: summon_position,
                        direction,
                    });
                }
            }
        }

        let owner_packets = vec![
            user_location_packet(&player),
            ServerPacket::Magic {
                spell,
                target_id: object_id,
                target: target.clone(),
                cast,
                level,
                secondary_target_ids: Vec::new(),
            },
        ];
        let mut action_packets = vec![ServerPacket::ObjectMagic {
            object_id: player.object_id,
            location: player.position.clone(),
            direction,
            spell,
            target_id: object_id,
            target,
            cast,
            level,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        }];
        if cast {
            action_packets.push(ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: player.object_id,
                    percent: zone_mana_percent(player.mp),
                },
            });
        }

        let recipients = self.player_status_recipients(player.object_id, &player.position);
        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: owner_packets,
        }];
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: action_packets,
            });
        }
        outbounds.extend(summon_outbounds);
        outbounds
    }

    fn player_cast_native_pet_magic(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: Point,
        cast: bool,
        level: u8,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.can_player_cast_native_pet_magic(
            session_id,
            object_id,
            spell,
            &target,
            cast,
            mp_cost,
            cooldown_ms,
            now_ms,
        ) {
            return self.correct_player_location(session_id, now_ms);
        }

        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        if cast {
            let mp_cost = mp_cost.max(0);
            let spell_key = spell as u8;
            live_player.mp = live_player.mp.saturating_sub(mp_cost).max(0);
            live_player.next_spell_ready_at_ms =
                now_ms.saturating_add(ZONE_NATIVE_PLAYER_SPELL_ACTION_MS);
            live_player
                .magic_ready_at_ms
                .insert(spell_key, now_ms.saturating_add(cooldown_ms.max(1)));
        }
        let player = live_player.clone();

        let owner_packets = vec![
            user_location_packet(&player),
            ServerPacket::Magic {
                spell,
                target_id: object_id,
                target: target.clone(),
                cast,
                level,
                secondary_target_ids: Vec::new(),
            },
        ];
        let mut action_packets = vec![ServerPacket::ObjectMagic {
            object_id: player.object_id,
            location: player.position.clone(),
            direction,
            spell,
            target_id: object_id,
            target: target.clone(),
            cast,
            level,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        }];
        if cast {
            action_packets.push(ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: player.object_id,
                    percent: zone_mana_percent(player.mp),
                },
            });
            action_packets.extend(self.apply_native_pet_enhancer(object_id, level, now_ms));
        }

        self.apply_zone_object_packets(&action_packets, now_ms);
        let recipients =
            self.native_monster_action_recipients(session_id, player.object_id, object_id, &target);
        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: owner_packets,
        }];
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: action_packets,
            });
        }
        outbounds
    }

    fn player_cast_native_self_magic(
        &mut self,
        session_id: &SessionId,
        target_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: Point,
        cast: bool,
        level: u8,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.can_player_cast_native_self_magic(
            session_id,
            target_id,
            spell,
            &target,
            cast,
            mp_cost,
            cooldown_ms,
            now_ms,
        ) {
            return self.correct_player_location(session_id, now_ms);
        }

        let Some(live_player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        live_player.direction = direction;
        if cast {
            let mp_cost = mp_cost.max(0);
            let spell_key = spell as u8;
            live_player.mp = live_player.mp.saturating_sub(mp_cost).max(0);
            live_player.next_spell_ready_at_ms =
                now_ms.saturating_add(ZONE_NATIVE_PLAYER_SPELL_ACTION_MS);
            live_player
                .magic_ready_at_ms
                .insert(spell_key, now_ms.saturating_add(cooldown_ms.max(1)));
        }
        let player = live_player.clone();

        let owner_packets = vec![
            user_location_packet(&player),
            ServerPacket::Magic {
                spell,
                target_id,
                target: target.clone(),
                cast,
                level,
                secondary_target_ids: Vec::new(),
            },
        ];
        let mut action_packets = vec![ServerPacket::ObjectMagic {
            object_id: player.object_id,
            location: player.position.clone(),
            direction,
            spell,
            target_id,
            target: target.clone(),
            cast,
            level,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        }];
        if cast {
            action_packets.push(ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: player.object_id,
                    percent: zone_mana_percent(player.mp),
                },
            });
            action_packets.extend(self.apply_native_player_self_magic(
                session_id, spell, direction, level, damage, &target, now_ms,
            ));
        }

        let recipients = self.player_status_recipients(player.object_id, &player.position);
        let mut outbounds = vec![ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: owner_packets,
        }];
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: action_packets,
            });
        }
        outbounds
    }

    fn can_player_cast_native_summon_magic(
        &self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        _direction: MirDirection,
        target: &Point,
        cast: bool,
        mp_cost: i32,
        _cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        let Some(profile) = native_summon_spell_profile(spell) else {
            return false;
        };
        if !self.native_summon_target_point_allowed(object_id, profile, player, target) {
            return false;
        }
        if crystal_monster_by_name(profile.monster_name).is_none() {
            return false;
        }
        if (profile.recall_mode == NativeSummonRecallMode::None
            || self
                .active_native_summon_object_id(player.object_id, profile.monster_name)
                .is_none())
            && self.active_native_summon_count_for_profile(player.object_id, profile)
                >= profile.max_summons
        {
            return false;
        }
        if cast {
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost.max(0)
                || now_ms < ready_at_ms
                || now_ms < player.next_spell_ready_at_ms
            {
                return false;
            }
        }
        true
    }

    fn can_player_cast_native_pet_magic(
        &self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        target: &Point,
        cast: bool,
        mp_cost: i32,
        _cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        if spell != Spell::PetEnhancer {
            return false;
        }
        let Some(monster) = self.native_monsters.get(&object_id) else {
            return false;
        };
        if monster.dead
            || monster.hp <= 0
            || monster.master_object_id != player.object_id
            || monster.position != *target
            || monster.buffs.contains_key(&CRYSTAL_PET_ENHANCER_BUFF_TYPE)
        {
            return false;
        }
        if !points_within_action_range(
            &player.position,
            &monster.position,
            ZONE_NATIVE_PLAYER_MAGIC_MAX,
        ) {
            return false;
        }
        if cast {
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost.max(0)
                || now_ms < ready_at_ms
                || now_ms < player.next_spell_ready_at_ms
            {
                return false;
            }
        }
        true
    }

    fn can_player_cast_native_self_magic(
        &self,
        session_id: &SessionId,
        target_id: u32,
        spell: Spell,
        target: &Point,
        cast: bool,
        mp_cost: i32,
        _cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        if !zone_self_magic_target_point_allowed(spell, &player.position, target) {
            return false;
        }
        if target_id != 0 && target_id != player.object_id {
            return false;
        }
        if cast {
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost.max(0)
                || now_ms < ready_at_ms
                || now_ms < player.next_spell_ready_at_ms
            {
                return false;
            }
        }
        match spell {
            Spell::Healing => player.hp < player.max_hp,
            Spell::MassHealing | Spell::HealingCircle => {
                !self.native_area_heal_target_session_ids(target).is_empty()
            }
            Spell::MagicShield => !player.buffs.contains_key(&CRYSTAL_MAGIC_SHIELD_BUFF_TYPE),
            _ => false,
        }
    }

    fn can_player_cast_native_ground_magic(
        &self,
        session_id: &SessionId,
        spell: Spell,
        direction: MirDirection,
        target: &Point,
        cast: bool,
        damage: i32,
        mp_cost: i32,
        _cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        if !points_within_action_range(&player.position, target, ZONE_NATIVE_PLAYER_MAGIC_MAX) {
            return false;
        }
        if cast {
            let spell_key = spell as u8;
            let ready_at_ms = player
                .magic_ready_at_ms
                .get(&spell_key)
                .copied()
                .unwrap_or_default();
            if player.mp < mp_cost.max(0)
                || now_ms < ready_at_ms
                || now_ms < player.next_spell_ready_at_ms
            {
                return false;
            }
        }
        !self
            .native_ground_spell_locations(spell, target, &player.position, direction)
            .is_empty()
            && (damage > 0 || matches!(spell, Spell::Trap | Spell::TrapHexagon))
    }

    fn apply_native_monster_magic_control(
        &mut self,
        object_id: u32,
        spell: Spell,
        level: u8,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(control) = zone_native_magic_control(spell, level, object_id, now_ms) else {
            return Vec::new();
        };
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        if monster.dead || !monster.hostile_to_player {
            return Vec::new();
        }

        let control_until_ms = now_ms.saturating_add(control.duration_ms);
        monster.control_until_ms = monster.control_until_ms.max(control_until_ms);
        if control.blocks_ai {
            monster.next_ai_ready_at_ms = monster.next_ai_ready_at_ms.max(monster.control_until_ms);
            monster.next_attack_ready_at_ms = monster
                .next_attack_ready_at_ms
                .max(monster.control_until_ms);
        }

        let mut packets = Vec::new();
        if let Some(effect) = control.effect {
            packets.push(ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id,
                    effect,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            });
        }
        if let Some(poison) = control.poison {
            monster.control_poison = poison;
            packets.push(ServerPacket::ObjectPoisoned {
                object_id,
                poison: native_monster_current_poison(monster),
            });
        }
        packets
    }

    fn apply_native_monster_magic_damage_poison(
        &mut self,
        object_id: u32,
        owner_session_id: &SessionId,
        owner_object_id: u32,
        spell: Spell,
        level: u8,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        if spell != Spell::PoisonShot {
            return Vec::new();
        }
        self.apply_native_monster_green_damage_poison(
            object_id,
            owner_session_id,
            owner_object_id,
            level,
            damage,
            now_ms,
        )
    }

    fn apply_native_monster_green_damage_poison(
        &mut self,
        object_id: u32,
        owner_session_id: &SessionId,
        owner_object_id: u32,
        level: u8,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        if monster.dead || monster.hp <= 0 {
            return Vec::new();
        }

        let green_damage = zone_poison_shot_green_damage(damage, level);
        if monster.damage_poison & CRYSTAL_POISON_GREEN != 0
            && monster.damage_poison_value > green_damage
        {
            return Vec::new();
        }

        monster.damage_poison = CRYSTAL_POISON_GREEN;
        monster.damage_poison_value = green_damage;
        monster.damage_poison_next_damage_at_ms =
            now_ms.saturating_add(ZONE_CRYSTAL_GREEN_POISON_TICK_MS);
        monster.damage_poison_expires_at_ms =
            now_ms.saturating_add(zone_poison_shot_green_duration_ms(damage, level));
        monster.damage_poison_owner_session_id = Some(owner_session_id.clone());
        monster.damage_poison_owner_object_id = owner_object_id;

        vec![ServerPacket::ObjectPoisoned {
            object_id,
            poison: native_monster_current_poison(monster),
        }]
    }

    fn apply_native_player_ground_spell(
        &mut self,
        session_id: &SessionId,
        target_object_id: u32,
        player_object_id: u32,
        player_position: &Point,
        player_level: u16,
        spell: Spell,
        direction: MirDirection,
        level: u8,
        target: &Point,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        if !zone_magic_uses_ground_spell(spell) {
            return Vec::new();
        }
        if damage <= 0 && !matches!(spell, Spell::Trap | Spell::TrapHexagon) {
            return Vec::new();
        }

        let locations =
            self.native_ground_spell_locations(spell, target, player_position, direction);
        if locations.is_empty() {
            return Vec::new();
        }
        if spell == Spell::Trap
            && !self.apply_native_trap_control(target_object_id, player_level, now_ms)
        {
            return Vec::new();
        }
        if spell == Spell::TrapHexagon
            && !self.apply_native_trap_hexagon_control(target, level, now_ms)
        {
            return Vec::new();
        }

        let due_ms = now_ms.saturating_add(500);
        let (next_damage_at_ms, expires_at_ms, tick_interval_ms) =
            zone_ground_spell_timing(spell, due_ms, damage);
        let spell_object_ids = match spell {
            Spell::FireWall => (0..locations.len())
                .map(|index| {
                    player_object_id
                        .saturating_add(80_000)
                        .saturating_add(u32::try_from(index).unwrap_or_default())
                })
                .collect(),
            Spell::Blizzard | Spell::MeteorStrike => (0..locations.len())
                .map(|_| self.unique_object_id(0))
                .collect(),
            Spell::PoisonCloud => vec![self.unique_object_id(0)],
            Spell::Trap => vec![self.unique_object_id(0)],
            Spell::TrapHexagon => (0..locations.len())
                .map(|_| self.unique_object_id(0))
                .collect(),
            Spell::ExplosiveTrap => (0..locations.len())
                .map(|_| self.unique_object_id(0))
                .collect(),
            _ => Vec::new(),
        };
        self.pending_native_ground_spells
            .push(PendingNativeGroundSpellAction {
                spell,
                caster_session_id: session_id.clone(),
                caster_object_id: player_object_id,
                locations,
                spell_object_ids,
                direction,
                spell_param: spell == Spell::Trap && level > 0,
                damage,
                spell_packets_at_ms: due_ms,
                spell_packets_sent: false,
                next_damage_at_ms,
                expires_at_ms,
                tick_interval_ms,
            });
        Vec::new()
    }

    fn native_ground_spell_locations(
        &self,
        spell: Spell,
        target: &Point,
        player_position: &Point,
        direction: MirDirection,
    ) -> Vec<Point> {
        let locations = match spell {
            Spell::FireWall => vec![
                target.clone(),
                offset_point(target, MirDirection::Up, 1),
                offset_point(target, MirDirection::Right, 1),
                offset_point(target, MirDirection::Down, 1),
                offset_point(target, MirDirection::Left, 1),
            ],
            Spell::Blizzard | Spell::MeteorStrike => (-2..=2)
                .flat_map(|dy| {
                    (-2..=2).map(move |dx| Point {
                        x: target.x + dx,
                        y: target.y + dy,
                    })
                })
                .collect(),
            Spell::PoisonCloud => (-1..=1)
                .flat_map(|dy| {
                    (-1..=1).map(move |dx| Point {
                        x: target.x + dx,
                        y: target.y + dy,
                    })
                })
                .collect(),
            Spell::Trap => vec![target.clone()],
            Spell::TrapHexagon => trap_hexagon_spell_points(target),
            Spell::ExplosiveTrap => {
                let front = offset_point(player_position, direction, 1);
                if self.collision.is_blocked(&front) {
                    Vec::new()
                } else {
                    let side_direction = zone_rotated_direction(direction, 2);
                    std::iter::once(front.clone())
                        .chain([-1, 1].into_iter().filter_map(|side_offset| {
                            let location = offset_point(&front, side_direction, side_offset);
                            (!self.collision.is_blocked(&location)).then_some(location)
                        }))
                        .collect()
                }
            }
            _ => Vec::new(),
        };
        match spell {
            Spell::FireWall => locations
                .into_iter()
                .filter(|location| !self.collision.is_blocked(location))
                .collect(),
            _ => locations,
        }
    }

    fn apply_native_trap_control(
        &mut self,
        target_object_id: u32,
        player_level: u16,
        now_ms: u64,
    ) -> bool {
        let Some(monster) = self.native_monsters.get_mut(&target_object_id) else {
            return false;
        };
        if monster.dead
            || monster.hp <= 0
            || !monster.hostile_to_player
            || monster.level >= player_level.saturating_add(2)
        {
            return false;
        }

        let release_at_ms = now_ms.saturating_add(60_000);
        monster.control_until_ms = monster.control_until_ms.max(release_at_ms);
        monster.next_ai_ready_at_ms = monster.next_ai_ready_at_ms.max(release_at_ms);
        monster.next_attack_ready_at_ms = monster.next_attack_ready_at_ms.max(release_at_ms);
        true
    }

    fn apply_native_trap_hexagon_control(
        &mut self,
        target: &Point,
        level: u8,
        now_ms: u64,
    ) -> bool {
        let affected_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                (!monster.dead
                    && monster.hp > 0
                    && monster.hostile_to_player
                    && zone_tile_distance(&monster.position, target) <= 1)
                    .then_some(*object_id)
            })
            .collect::<Vec<_>>();
        if affected_object_ids.is_empty() {
            return false;
        }

        let release_at_ms = now_ms.saturating_add(
            (u64::from(level).saturating_mul(5).saturating_add(10)).saturating_mul(1_000),
        );
        for object_id in affected_object_ids {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.control_until_ms = monster.control_until_ms.max(release_at_ms);
                monster.next_ai_ready_at_ms = monster.next_ai_ready_at_ms.max(release_at_ms);
                monster.next_attack_ready_at_ms =
                    monster.next_attack_ready_at_ms.max(release_at_ms);
            }
        }
        true
    }

    fn apply_native_player_fire_bounce_chain(
        &mut self,
        session_id: &SessionId,
        primary_object_id: u32,
        player_object_id: u32,
        spell: Spell,
        level: u8,
        damage: i32,
        now_ms: u64,
    ) {
        if spell != Spell::FireBounce || damage <= 0 {
            return;
        }
        let Some(player_position) = self
            .players
            .get(session_id)
            .map(|player| player.position.clone())
        else {
            return;
        };
        let Some(primary_position) = self
            .native_monsters
            .get(&primary_object_id)
            .map(|monster| monster.position.clone())
        else {
            return;
        };

        let mut chain = vec![primary_object_id];
        let mut previous_position = primary_position;
        let max_bounces = usize::from(level).saturating_add(2);
        for _ in 0..max_bounces {
            let mut candidates = self
                .native_monsters
                .iter()
                .filter_map(|(object_id, monster)| {
                    (!chain.contains(object_id)
                        && !monster.dead
                        && monster.hp > 0
                        && monster.hostile_to_player
                        && zone_tile_distance(&previous_position, &monster.position) <= 4)
                        .then_some((
                            *object_id,
                            monster.position.clone(),
                            zone_tile_distance(&previous_position, &monster.position),
                        ))
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|(object_id, _, distance)| (*distance, *object_id));
            let Some((next_object_id, next_position, _)) = candidates.into_iter().next() else {
                break;
            };
            chain.push(next_object_id);
            previous_position = next_position;
        }

        let mut due_ms = now_ms;
        let mut source_object_id = player_object_id;
        let mut source_position = player_position;
        for (index, target_object_id) in chain.into_iter().enumerate() {
            let Some(target_position) = self
                .native_monsters
                .get(&target_object_id)
                .map(|monster| monster.position.clone())
            else {
                continue;
            };
            if index == 0 {
                source_object_id = target_object_id;
                source_position = target_position;
                continue;
            }

            let distance_ms = u64::try_from(zone_tile_distance(&source_position, &target_position))
                .unwrap_or_default()
                .saturating_mul(50)
                .max(1);
            due_ms = due_ms.saturating_add(distance_ms);
            self.pending_native_projectiles
                .push(PendingNativeProjectile {
                    ready_at_ms: due_ms,
                    session_id: session_id.clone(),
                    spell,
                    source_id: source_object_id,
                    destination_id: target_object_id,
                });
            // Each bounce target mitigates with its own magic armour, like the
            // primary attack-magic hit.
            let bounce_damage = match self.native_monsters.get(&target_object_id) {
                Some(target_monster) => zone_magic_damage_after_monster_armour(
                    target_monster,
                    damage,
                    player_object_id,
                    due_ms,
                ),
                None => damage,
            };
            self.pending_native_hits.push(PendingNativeMonsterHit {
                ready_at_ms: due_ms,
                session_id: session_id.clone(),
                attacker_object_id: player_object_id,
                object_id: target_object_id,
                damage: bounce_damage,
            });
            source_object_id = target_object_id;
            source_position = target_position;
        }
    }

    fn apply_native_player_self_magic(
        &mut self,
        session_id: &SessionId,
        spell: Spell,
        direction: MirDirection,
        level: u8,
        damage: i32,
        target: &Point,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        match spell {
            Spell::Healing => self.apply_native_player_healing(session_id, damage, now_ms),
            Spell::MassHealing => self.apply_native_player_area_healing(
                session_id, spell, direction, damage, target, now_ms,
            ),
            Spell::HealingCircle => self.apply_native_player_area_healing(
                session_id, spell, direction, damage, target, now_ms,
            ),
            Spell::MagicShield => {
                self.apply_native_player_magic_shield(session_id, level, damage, now_ms)
            }
            _ => Vec::new(),
        }
    }

    fn apply_native_player_healing(
        &mut self,
        session_id: &SessionId,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(player) = self.players.get(session_id) else {
            return Vec::new();
        };
        if player.dead || player.hp >= player.max_hp {
            return Vec::new();
        }
        let heal = damage.max(1).saturating_add(i32::from(player.level)).max(1);
        let object_id = player.object_id;
        self.pending_native_player_heals
            .push(PendingNativePlayerHeal {
                ready_at_ms: now_ms.saturating_add(500),
                session_id: session_id.clone(),
                amount: heal,
            });
        vec![ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id,
                effect: CRYSTAL_SPELL_EFFECT_HEALING,
                effect_type: 0,
                delay_time: 0,
                time: 0,
            },
        }]
    }

    fn apply_native_player_area_healing(
        &mut self,
        session_id: &SessionId,
        spell: Spell,
        direction: MirDirection,
        damage: i32,
        target: &Point,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(player) = self.players.get(session_id) else {
            return Vec::new();
        };
        if player.dead {
            return Vec::new();
        }
        let caster_object_id = player.object_id;

        let (delay_ms, heal) = match spell {
            Spell::MassHealing => (500, damage.max(1)),
            Spell::HealingCircle => (1_700, 25),
            _ => return Vec::new(),
        };
        let target_session_ids = self.native_area_heal_target_session_ids(target);
        if target_session_ids.is_empty() {
            return Vec::new();
        }
        let due_ms = now_ms.saturating_add(delay_ms);
        self.pending_native_player_heals
            .extend(target_session_ids.into_iter().map(|target_session_id| {
                PendingNativePlayerHeal {
                    ready_at_ms: due_ms,
                    session_id: target_session_id,
                    amount: heal,
                }
            }));

        if spell != Spell::HealingCircle {
            return Vec::new();
        }

        self.pending_native_ground_spells
            .push(PendingNativeGroundSpellAction {
                spell,
                caster_session_id: session_id.clone(),
                caster_object_id,
                locations: vec![target.clone()],
                spell_object_ids: vec![caster_object_id
                    .saturating_add(70_000)
                    .saturating_add(u32::try_from(now_ms % 1_000).unwrap_or_default())],
                direction,
                spell_param: false,
                damage: 0,
                spell_packets_at_ms: due_ms,
                spell_packets_sent: false,
                next_damage_at_ms: due_ms.saturating_add(1),
                expires_at_ms: due_ms.saturating_add(2),
                tick_interval_ms: 1,
            });
        Vec::new()
    }

    fn native_area_heal_target_session_ids(&self, target: &Point) -> Vec<SessionId> {
        self.players
            .iter()
            .filter_map(|(session_id, player)| {
                (!player.dead
                    && player.hp < player.max_hp
                    && zone_tile_distance(&player.position, target) <= 2)
                    .then(|| session_id.clone())
            })
            .collect()
    }

    fn apply_native_player_magic_shield(
        &mut self,
        session_id: &SessionId,
        level: u8,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        if player.buffs.contains_key(&CRYSTAL_MAGIC_SHIELD_BUFF_TYPE) {
            return Vec::new();
        }

        let duration_ms = u64::try_from(damage.max(0))
            .unwrap_or_default()
            .saturating_add(15)
            .max(1)
            .saturating_mul(1_000);
        let buff = ClientBuff {
            buff_type: CRYSTAL_MAGIC_SHIELD_BUFF_TYPE,
            visible: true,
            object_id: player.object_id,
            expire_time: i64::try_from(duration_ms).unwrap_or(i64::MAX),
            infinite: false,
            paused: false,
            stats: vec![UserItemStat {
                stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
                value: (i32::from(level) + 2).saturating_mul(10),
            }],
            values: Vec::new(),
        };
        player.buffs.insert(
            CRYSTAL_MAGIC_SHIELD_BUFF_TYPE,
            super::types::ZonePlayerBuff {
                buff: buff.clone(),
                expires_at_ms: Some(now_ms.saturating_add(duration_ms)),
            },
        );
        vec![
            ServerPacket::AddBuff { buff },
            ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: player.object_id,
                    effect: CRYSTAL_SPELL_EFFECT_MAGIC_SHIELD_UP,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            },
        ]
    }

    fn apply_native_pet_enhancer(
        &mut self,
        object_id: u32,
        level: u8,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        if monster.buffs.contains_key(&CRYSTAL_PET_ENHANCER_BUFF_TYPE) {
            return Vec::new();
        }

        let duration_ms = (10_u64 + u64::from(level).saturating_mul(5)).saturating_mul(1_000);
        let target_level = i32::from(monster.level).max(1);
        let dc_inc = 2 + target_level.saturating_mul(2);
        let ac_inc = 4 + target_level;
        let buff = ClientBuff {
            buff_type: CRYSTAL_PET_ENHANCER_BUFF_TYPE,
            visible: true,
            object_id,
            expire_time: i64::try_from(duration_ms).unwrap_or(i64::MAX),
            infinite: false,
            paused: false,
            stats: vec![
                UserItemStat {
                    stat: CRYSTAL_STAT_MIN_DC,
                    value: dc_inc,
                },
                UserItemStat {
                    stat: CRYSTAL_STAT_MAX_DC,
                    value: dc_inc,
                },
                UserItemStat {
                    stat: CRYSTAL_STAT_MIN_AC,
                    value: ac_inc,
                },
                UserItemStat {
                    stat: CRYSTAL_STAT_MAX_AC,
                    value: ac_inc,
                },
            ],
            values: Vec::new(),
        };
        monster.buffs.insert(
            CRYSTAL_PET_ENHANCER_BUFF_TYPE,
            super::types::ZonePlayerBuff {
                buff: buff.clone(),
                expires_at_ms: Some(now_ms.saturating_add(duration_ms)),
            },
        );
        vec![ServerPacket::AddBuff { buff }]
    }

    fn apply_native_player_arrow_cast_effects(
        &mut self,
        session_id: &SessionId,
        spell: Spell,
        level: u8,
        damage: i32,
        target: &Point,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        match spell {
            Spell::VampireShot => self.apply_native_player_arrow_marker_buff(
                session_id,
                CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT,
                level,
                now_ms,
            ),
            Spell::PoisonShot => self.apply_native_player_arrow_marker_buff(
                session_id,
                CRYSTAL_ARROW_BUFF_POISON_SHOT,
                level,
                now_ms,
            ),
            Spell::CrippleShot => {
                self.apply_native_player_cripple_shot(session_id, level, damage, target, now_ms)
            }
            _ => Vec::new(),
        }
    }

    fn apply_native_player_arrow_marker_buff(
        &mut self,
        session_id: &SessionId,
        buff_type: u8,
        level: u8,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        if player.buffs.contains_key(&CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT)
            || player.buffs.contains_key(&CRYSTAL_ARROW_BUFF_POISON_SHOT)
        {
            return Vec::new();
        }
        let roll = zone_deterministic_roll(
            now_ms / ZONE_CRYSTAL_STATUS_TICK_MS,
            usize::try_from(player.object_id).unwrap_or_default(),
            usize::from(level).saturating_add(usize::from(buff_type)),
            20,
        );
        if roll < 8 {
            return Vec::new();
        }

        let duration_ms = u64::from(level).saturating_mul(5_000).saturating_add(5_000);
        let buff = ClientBuff {
            buff_type,
            visible: true,
            object_id: player.object_id,
            expire_time: i64::try_from(duration_ms).unwrap_or(i64::MAX),
            infinite: false,
            paused: false,
            stats: Vec::new(),
            values: Vec::new(),
        };
        player.buffs.insert(
            buff_type,
            super::types::ZonePlayerBuff {
                buff: buff.clone(),
                expires_at_ms: Some(now_ms.saturating_add(duration_ms)),
            },
        );
        vec![ServerPacket::AddBuff { buff }]
    }

    fn apply_native_player_cripple_shot(
        &mut self,
        session_id: &SessionId,
        level: u8,
        damage: i32,
        target: &Point,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(player) = self.players.get_mut(session_id) else {
            return Vec::new();
        };
        let player_object_id = player.object_id;
        let consumed_buffs = [
            CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT,
            CRYSTAL_ARROW_BUFF_POISON_SHOT,
        ]
        .into_iter()
        .filter(|buff_type| player.buffs.remove(buff_type).is_some())
        .collect::<Vec<_>>();
        if consumed_buffs.is_empty() {
            return Vec::new();
        }

        let mut packets = consumed_buffs
            .iter()
            .map(|buff_type| ServerPacket::RemoveBuff {
                object_id: player_object_id,
                buff_type: *buff_type,
            })
            .collect::<Vec<_>>();
        if consumed_buffs.contains(&CRYSTAL_ARROW_BUFF_POISON_SHOT) {
            let affected_monster_ids = self
                .native_monsters
                .iter()
                .filter_map(|(object_id, monster)| {
                    (!monster.dead
                        && monster.hp > 0
                        && monster.hostile_to_player
                        && zone_tile_distance(&monster.position, target) <= 1)
                        .then_some(*object_id)
                })
                .collect::<Vec<_>>();
            for object_id in affected_monster_ids {
                packets.extend(self.apply_native_monster_green_damage_poison(
                    object_id,
                    session_id,
                    player_object_id,
                    level,
                    damage,
                    now_ms,
                ));
            }
        }
        if consumed_buffs.contains(&CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT) {
            self.pending_native_player_heals
                .push(PendingNativePlayerHeal {
                    ready_at_ms: now_ms,
                    session_id: session_id.clone(),
                    amount: zone_vampire_shot_heal_amount(damage, level),
                });
        }
        packets
    }

    fn native_magic_secondary_targets(
        &self,
        spell: Spell,
        primary_object_id: u32,
        target: &Point,
    ) -> Vec<u32> {
        let (radius, limit) = match spell {
            Spell::FireBang | Spell::IceStorm => (1, usize::MAX),
            Spell::MeteorShower => (4, 3),
            _ => return Vec::new(),
        };
        let mut targets = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                (*object_id != primary_object_id
                    && !monster.dead
                    && monster.hp > 0
                    && monster.hostile_to_player
                    && zone_tile_distance(&monster.position, target) <= radius)
                    .then_some((*object_id, zone_tile_distance(&monster.position, target)))
            })
            .collect::<Vec<_>>();
        targets.sort_by_key(|(object_id, distance)| (*distance, *object_id));
        targets
            .into_iter()
            .take(limit)
            .map(|(object_id, _)| object_id)
            .collect()
    }

    fn expire_native_monster_controls(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut clear_packets = Vec::new();
        for (object_id, monster) in self.native_monsters.iter_mut() {
            if monster.control_until_ms == 0 || now_ms < monster.control_until_ms {
                continue;
            }
            monster.control_until_ms = 0;
            if monster.control_poison == 0 {
                continue;
            }
            monster.control_poison = 0;
            let poison = native_monster_current_poison(monster);
            clear_packets.push((
                *object_id,
                monster.position.clone(),
                ServerPacket::ObjectPoisoned {
                    object_id: *object_id,
                    poison,
                },
            ));
        }
        if clear_packets.is_empty() {
            return Vec::new();
        }

        let packets = clear_packets
            .iter()
            .map(|(_, _, packet)| packet.clone())
            .collect::<Vec<_>>();
        self.apply_zone_object_packets(&packets, now_ms);

        clear_packets
            .into_iter()
            .filter_map(|(object_id, position, packet)| {
                let recipients = self.native_monster_visible_recipients(object_id, &position);
                (!recipients.is_empty()).then_some(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                })
            })
            .collect()
    }

    fn tick_native_monster_damage_poisons(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let monster_ids = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                (monster.damage_poison != 0
                    && !monster.dead
                    && (now_ms >= monster.damage_poison_expires_at_ms
                        || now_ms >= monster.damage_poison_next_damage_at_ms))
                    .then_some(*object_id)
            })
            .collect::<Vec<_>>();

        let mut outbounds = Vec::new();
        for object_id in monster_ids {
            outbounds.extend(self.tick_native_monster_damage_poison(object_id, now_ms));
        }
        outbounds
    }

    fn tick_native_monster_damage_poison(
        &mut self,
        object_id: u32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        if monster.damage_poison == 0 || monster.dead {
            return Vec::new();
        }

        if now_ms >= monster.damage_poison_expires_at_ms {
            monster.damage_poison = 0;
            monster.damage_poison_value = 0;
            monster.damage_poison_next_damage_at_ms = 0;
            monster.damage_poison_expires_at_ms = 0;
            monster.damage_poison_owner_session_id = None;
            monster.damage_poison_owner_object_id = 0;
            let position = monster.position.clone();
            let packet = ServerPacket::ObjectPoisoned {
                object_id,
                poison: native_monster_current_poison(monster),
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
            let recipients = self.native_monster_visible_recipients(object_id, &position);
            return (!recipients.is_empty())
                .then_some(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                })
                .into_iter()
                .collect();
        }

        if monster.damage_poison & CRYSTAL_POISON_GREEN == 0
            || monster.damage_poison_value <= 0
            || now_ms < monster.damage_poison_next_damage_at_ms
        {
            return Vec::new();
        }

        let damage = monster.damage_poison_value;
        let owner_session_id = monster.damage_poison_owner_session_id.clone();
        let owner_object_id = monster.damage_poison_owner_object_id;
        monster.damage_poison_next_damage_at_ms =
            now_ms.saturating_add(ZONE_CRYSTAL_GREEN_POISON_TICK_MS);

        let Some((
            applied_damage,
            health_percent,
            killed,
            monster_name,
            experience,
            position,
            monster_direction,
            drops,
        )) = self.apply_native_monster_damage(object_id, damage)
        else {
            return Vec::new();
        };

        let mut packets = vec![
            ServerPacket::DamageIndicator {
                damage: applied_damage,
                damage_type: 0,
                object_id,
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id,
                    percent: health_percent,
                    expire: 0,
                },
            },
        ];
        if killed {
            packets.push(ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id,
                    location: position.clone(),
                    direction: monster_direction,
                    kind: 0,
                },
            });
        }
        self.apply_zone_object_packets(&packets, now_ms);
        let recipients = self.native_monster_visible_recipients(object_id, &position);

        let mut outbounds = Vec::new();
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets,
            });
        }

        if killed {
            if let Some(owner_session_id) = owner_session_id {
                let spawned_drops = self.spawn_native_monster_drops(
                    &monster_name,
                    &position,
                    owner_object_id,
                    drops,
                    now_ms,
                );
                outbounds.extend(self.diff_all_zone_object_visibility());
                outbounds.push(ZoneOutbound::MonsterKillAward {
                    session_id: owner_session_id,
                    award: ZoneMonsterKillAward {
                        monster_object_id: object_id,
                        monster_name,
                        experience,
                        drops: spawned_drops,
                    },
                });
            }
        }

        outbounds
    }

    fn apply_native_poison_cloud_poison(
        &mut self,
        object_id: u32,
        owner_session_id: &SessionId,
        owner_object_id: u32,
        damage: i32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        if monster.dead || monster.hp <= 0 {
            return Vec::new();
        }

        monster.damage_poison = CRYSTAL_POISON_GREEN;
        monster.damage_poison_value = damage.saturating_div(3).max(1);
        monster.damage_poison_next_damage_at_ms =
            now_ms.saturating_add(ZONE_CRYSTAL_GREEN_POISON_TICK_MS);
        monster.damage_poison_expires_at_ms = now_ms.saturating_add(12_000);
        monster.damage_poison_owner_session_id = Some(owner_session_id.clone());
        monster.damage_poison_owner_object_id = owner_object_id;

        let position = monster.position.clone();
        let packet = ServerPacket::ObjectPoisoned {
            object_id,
            poison: native_monster_current_poison(monster),
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients = self.native_monster_visible_recipients(object_id, &position);
        (!recipients.is_empty())
            .then_some(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            })
            .into_iter()
            .collect()
    }

    fn tick_native_ground_spells(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let actions = std::mem::take(&mut self.pending_native_ground_spells);
        let mut retained = Vec::new();
        let mut outbounds = Vec::new();

        for mut action in actions {
            if now_ms >= action.expires_at_ms
                || !self.players.contains_key(&action.caster_session_id)
            {
                continue;
            }

            if !action.spell_packets_sent && now_ms >= action.spell_packets_at_ms {
                let packets = native_ground_spell_packets(&action);
                let recipients = self
                    .ground_spell_visible_recipients(&action.caster_session_id, &action.locations);
                if !recipients.is_empty() && !packets.is_empty() {
                    outbounds.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets,
                    });
                }
                action.spell_packets_sent = true;
            }

            if now_ms >= action.next_damage_at_ms {
                if matches!(
                    action.spell,
                    Spell::FireWall
                        | Spell::Blizzard
                        | Spell::MeteorStrike
                        | Spell::PoisonCloud
                        | Spell::ExplosiveTrap
                ) {
                    let mut detonated_trap = false;
                    let affected_object_ids = self
                        .native_monsters
                        .iter()
                        .filter_map(|(object_id, monster)| {
                            (!monster.dead
                                && monster.hp > 0
                                && monster.hostile_to_player
                                && action.locations.contains(&monster.position))
                            .then_some(*object_id)
                        })
                        .collect::<Vec<_>>();
                    for object_id in affected_object_ids {
                        // Ground/AoE attack spells mitigate their direct hit with
                        // the struck monster's magic armour, like the single-target
                        // cast. The PoisonCloud DoT below is applied separately and
                        // stays unmitigated (poison is not reduced by MAC).
                        let hit_damage = match self.native_monsters.get(&object_id) {
                            Some(monster) => zone_magic_damage_after_monster_armour(
                                monster,
                                action.damage,
                                action.caster_object_id,
                                now_ms,
                            ),
                            None => action.damage,
                        };
                        outbounds.extend(self.resolve_pending_native_monster_hit(
                            PendingNativeMonsterHit {
                                ready_at_ms: now_ms,
                                session_id: action.caster_session_id.clone(),
                                attacker_object_id: action.caster_object_id,
                                object_id,
                                damage: hit_damage,
                            },
                            now_ms,
                        ));
                        if action.spell == Spell::PoisonCloud {
                            outbounds.extend(self.apply_native_poison_cloud_poison(
                                object_id,
                                &action.caster_session_id,
                                action.caster_object_id,
                                action.damage,
                                now_ms,
                            ));
                        }
                        if action.spell == Spell::ExplosiveTrap {
                            detonated_trap = true;
                        }
                    }
                    if detonated_trap {
                        continue;
                    }
                }
                action.next_damage_at_ms = now_ms.saturating_add(action.tick_interval_ms.max(1));
            }

            if !action.spell_packets_sent || action.next_damage_at_ms < action.expires_at_ms {
                retained.push(action);
            }
        }

        self.pending_native_ground_spells = retained;
        outbounds
    }

    fn resolve_pending_native_projectiles(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let pending_projectiles = std::mem::take(&mut self.pending_native_projectiles);
        let mut remaining_projectiles = Vec::new();
        let mut outbounds = Vec::new();
        for projectile in pending_projectiles {
            if now_ms < projectile.ready_at_ms {
                remaining_projectiles.push(projectile);
                continue;
            }
            outbounds.extend(self.resolve_pending_native_projectile(projectile));
        }
        self.pending_native_projectiles = remaining_projectiles;
        outbounds
    }

    fn resolve_pending_native_projectile(
        &mut self,
        projectile: PendingNativeProjectile,
    ) -> Vec<ZoneOutbound> {
        if !self.players.contains_key(&projectile.session_id) {
            return Vec::new();
        }
        let Some(monster) = self
            .native_monsters
            .get(&projectile.destination_id)
            .filter(|monster| !monster.dead && monster.hp > 0)
        else {
            return Vec::new();
        };
        let recipients = self.native_monster_action_recipients(
            &projectile.session_id,
            projectile.source_id,
            projectile.destination_id,
            &monster.position,
        );
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![ServerPacket::ObjectProjectile {
                spell: projectile.spell,
                source_id: projectile.source_id,
                destination_id: projectile.destination_id,
            }],
        }]
    }

    fn resolve_pending_native_monster_hits(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let pending_hits = std::mem::take(&mut self.pending_native_hits);
        let mut remaining_hits = Vec::new();
        let mut outbounds = Vec::new();
        for hit in pending_hits {
            if now_ms < hit.ready_at_ms {
                remaining_hits.push(hit);
                continue;
            }
            outbounds.extend(self.resolve_pending_native_monster_hit(hit, now_ms));
        }
        self.pending_native_hits = remaining_hits;
        outbounds
    }

    fn resolve_pending_native_monster_hit(
        &mut self,
        hit: PendingNativeMonsterHit,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.players.contains_key(&hit.session_id) {
            return Vec::new();
        }

        let Some((
            damage,
            health_percent,
            killed,
            monster_name,
            experience,
            position,
            monster_direction,
            drops,
        )) = self.apply_native_monster_damage(hit.object_id, hit.damage)
        else {
            return Vec::new();
        };

        let mut packets = vec![
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: hit.object_id,
                    attacker_id: hit.attacker_object_id,
                    location: position.clone(),
                    direction: monster_direction,
                },
            },
            ServerPacket::DamageIndicator {
                damage,
                damage_type: 0,
                object_id: hit.object_id,
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: hit.object_id,
                    percent: health_percent,
                    expire: 0,
                },
            },
        ];
        if killed {
            packets.push(ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: hit.object_id,
                    location: position.clone(),
                    direction: monster_direction,
                    kind: 0,
                },
            });
        }
        let (should_bleed, mut outbounds) =
            self.apply_native_vampire_spider_master_vampire(hit.attacker_object_id, damage);
        if should_bleed {
            packets.push(ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: hit.object_id,
                    effect: CRYSTAL_SPELL_EFFECT_BLEEDING,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            });
        }
        if let Some(packet) = self.apply_native_charmed_snake_hit_paralysis(
            hit.attacker_object_id,
            hit.object_id,
            now_ms,
        ) {
            packets.push(packet);
        }

        self.apply_zone_object_packets(&packets, now_ms);
        let recipients = self.native_monster_action_recipients(
            &hit.session_id,
            hit.attacker_object_id,
            hit.object_id,
            &position,
        );
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets,
            });
        }

        if killed {
            let drop_owner_object_id = self
                .native_monsters
                .get(&hit.attacker_object_id)
                .filter(|monster| zone_native_summon_owner_player_object_id(monster) != 0)
                .map(zone_native_summon_owner_player_object_id)
                .unwrap_or(hit.attacker_object_id);
            let spawned_drops = self.spawn_native_monster_drops(
                &monster_name,
                &position,
                drop_owner_object_id,
                drops,
                now_ms,
            );
            outbounds.extend(self.diff_all_zone_object_visibility());
            outbounds.push(ZoneOutbound::MonsterKillAward {
                session_id: hit.session_id.clone(),
                award: ZoneMonsterKillAward {
                    monster_object_id: hit.object_id,
                    monster_name,
                    experience,
                    drops: spawned_drops,
                },
            });
        }
        outbounds
    }

    fn resolve_pending_native_player_hits(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let pending_hits = std::mem::take(&mut self.pending_native_player_hits);
        let mut remaining_hits = Vec::new();
        let mut outbounds = Vec::new();
        for hit in pending_hits {
            if now_ms < hit.ready_at_ms {
                remaining_hits.push(hit);
                continue;
            }
            outbounds.extend(self.resolve_pending_native_player_hit(hit, now_ms));
        }
        self.pending_native_player_hits = remaining_hits;
        outbounds
    }

    fn resolve_pending_native_player_hit(
        &mut self,
        hit: PendingNativePlayerHit,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let status =
            zone_native_monster_player_status(hit.attacker_ai, hit.attacker_object_id, now_ms);
        let (target_object_id, position, direction, damage, health_percent, poison) = {
            let Some(target) = self.players.get_mut(&hit.target_session_id) else {
                return Vec::new();
            };
            if target.object_id != hit.target_object_id || target.dead {
                return Vec::new();
            }
            let damage = zone_player_native_incoming_damage(target, hit.damage, hit.magic, now_ms)
                .min(target.hp.saturating_sub(1));
            if damage <= 0 {
                return Vec::new();
            }
            target.hp = target.hp.saturating_sub(damage).max(1);
            let poison = if let Some(status) = status {
                if target.native_status_poison != 0 {
                    target.poison &= !target.native_status_poison;
                }
                target.native_status_poison = status.poison;
                target.native_status_poison_expires_at_ms =
                    Some(now_ms.saturating_add(status.duration_ms));
                target.poison |= status.poison;
                Some(target.poison)
            } else {
                None
            };
            (
                target.object_id,
                target.position.clone(),
                target.direction,
                damage,
                native_player_health_percent(target.hp, target.max_hp),
                poison,
            )
        };
        let mut packets = vec![
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: target_object_id,
                    attacker_id: hit.attacker_object_id,
                    location: position.clone(),
                    direction,
                },
            },
            ServerPacket::DamageIndicator {
                damage,
                damage_type: 0,
                object_id: target_object_id,
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: target_object_id,
                    percent: health_percent,
                    expire: 0,
                },
            },
        ];
        if let Some(poison) = poison {
            packets.push(ServerPacket::ObjectPoisoned {
                object_id: target_object_id,
                poison,
            });
        }
        let recipients = self.native_monster_combat_recipients(
            hit.attacker_object_id,
            target_object_id,
            &position,
        );
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![
            ZoneOutbound::ToMany {
                session_ids: recipients,
                packets,
            },
            ZoneOutbound::PlayerDamaged {
                session_id: hit.target_session_id,
                damage,
            },
        ]
    }

    fn resolve_pending_native_player_heals(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let pending_heals = std::mem::take(&mut self.pending_native_player_heals);
        let mut remaining_heals = Vec::new();
        let mut outbounds = Vec::new();
        for heal in pending_heals {
            if now_ms < heal.ready_at_ms {
                remaining_heals.push(heal);
                continue;
            }
            outbounds.extend(self.resolve_pending_native_player_heal(heal));
        }
        self.pending_native_player_heals = remaining_heals;
        outbounds
    }

    fn resolve_pending_native_player_heal(
        &mut self,
        heal: PendingNativePlayerHeal,
    ) -> Vec<ZoneOutbound> {
        self.apply_native_player_heal(heal.session_id, heal.amount)
    }

    fn apply_native_player_heal(
        &mut self,
        session_id: SessionId,
        amount: i32,
    ) -> Vec<ZoneOutbound> {
        if amount <= 0 {
            return Vec::new();
        }
        let (object_id, position, health_percent, applied) = {
            let Some(player) = self.players.get_mut(&session_id) else {
                return Vec::new();
            };
            if player.dead || player.hp >= player.max_hp {
                return Vec::new();
            }
            let before = player.hp;
            player.hp = player.hp.saturating_add(amount).min(player.max_hp);
            (
                player.object_id,
                player.position.clone(),
                native_player_health_percent(player.hp, player.max_hp),
                player.hp.saturating_sub(before),
            )
        };
        if applied <= 0 {
            return Vec::new();
        }
        let healed_session_id = session_id.clone();
        let recipients = self
            .players
            .iter()
            .filter_map(|(candidate_session_id, player)| {
                (candidate_session_id == &healed_session_id
                    || player.visible_object_ids.contains(&object_id)
                    || points_visible(&player.position, &position))
                .then(|| candidate_session_id.clone())
            })
            .collect::<Vec<_>>();
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![
            ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id,
                        percent: health_percent,
                        expire: 0,
                    },
                }],
            },
            ZoneOutbound::PlayerHealed {
                session_id: healed_session_id,
                amount: applied,
            },
        ]
    }

    fn apply_native_vampire_spider_master_vampire(
        &mut self,
        attacker_object_id: u32,
        damage: i32,
    ) -> (bool, Vec<ZoneOutbound>) {
        if damage <= 0 {
            return (false, Vec::new());
        }
        let Some(monster) = self.native_monsters.get(&attacker_object_id) else {
            return (false, Vec::new());
        };
        if monster.name != "VampireSpider" {
            return (false, Vec::new());
        }
        let Some(owner_session_id) = monster.owner_session_id.clone() else {
            return (false, Vec::new());
        };
        let skill_level = monster.summon_skill_level;
        if !self.players.contains_key(&owner_session_id) {
            return (false, Vec::new());
        }
        let heal_amount = zone_vampire_shot_heal_amount(damage, skill_level);
        (
            true,
            self.apply_native_player_heal(owner_session_id, heal_amount),
        )
    }

    fn apply_native_charmed_snake_hit_paralysis(
        &mut self,
        attacker_object_id: u32,
        target_object_id: u32,
        now_ms: u64,
    ) -> Option<ServerPacket> {
        let attacker = self.native_monsters.get(&attacker_object_id)?;
        if attacker.name != "CharmedSnake" {
            return None;
        }
        let pet_level = attacker.summon_skill_level;
        let chance_denominator = 10_u64.saturating_sub(u64::from(pet_level)).max(1);
        let current_tick = now_ms.div_ceil(ZONE_CRYSTAL_STATUS_TICK_MS);
        if !zone_deterministic_chance_roll(
            current_tick,
            attacker_object_id,
            4_u64.saturating_add(u64::from(pet_level)),
            chance_denominator,
        ) {
            return None;
        }

        let target = self.native_monsters.get_mut(&target_object_id)?;
        if target.dead || target.hp <= 0 {
            return None;
        }
        if native_monster_control_active(target, now_ms)
            && target.control_poison
                & (CRYSTAL_POISON_SLOW
                    | CRYSTAL_POISON_FROZEN
                    | CRYSTAL_POISON_STUN
                    | CRYSTAL_POISON_PARALYSIS)
                != 0
        {
            return None;
        }

        let duration_ms = u64::from(pet_level)
            .saturating_add(4)
            .saturating_mul(ZONE_CRYSTAL_STATUS_TICK_MS);
        target.control_poison = CRYSTAL_POISON_PARALYSIS;
        target.control_until_ms = now_ms.saturating_add(duration_ms);
        target.next_ai_ready_at_ms = target.next_ai_ready_at_ms.max(target.control_until_ms);
        target.next_attack_ready_at_ms =
            target.next_attack_ready_at_ms.max(target.control_until_ms);
        Some(ServerPacket::ObjectPoisoned {
            object_id: target_object_id,
            poison: native_monster_current_poison(target),
        })
    }

    fn resolve_pending_native_summons(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let pending_summons = std::mem::take(&mut self.pending_native_summons);
        let mut remaining_summons = Vec::new();
        let mut outbounds = Vec::new();
        for summon in pending_summons {
            if now_ms < summon.ready_at_ms {
                remaining_summons.push(summon);
                continue;
            }
            outbounds.extend(self.resolve_pending_native_summon(summon, now_ms));
        }
        self.pending_native_summons = remaining_summons;
        outbounds
    }

    fn resolve_pending_native_summon(
        &mut self,
        summon: PendingNativeSummon,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let owner_present = self
            .players
            .get(&summon.owner_session_id)
            .is_some_and(|player| player.object_id == summon.owner_object_id && !player.dead);
        if !owner_present
            || self.active_native_summon_count_for_scope(
                summon.owner_object_id,
                summon.monster_name,
                summon.limit_scope,
            ) >= summon.max_summons
        {
            return Vec::new();
        }
        let Some(template) = crystal_monster_by_name(summon.monster_name) else {
            return Vec::new();
        };
        let object_id = self.unique_object_id(0);
        let position = self.first_available_position(summon.position, None);
        let spawn = ZoneMonsterSpawn {
            object_id,
            name: template.name.clone(),
            name_colour_argb: -1,
            image: template.image,
            ai: template.ai,
            level: template.level,
            max_hp: template.hp.max(1),
            hp: template.hp.max(1),
            experience: 0,
            defense: super::types::ZoneMonsterDefense::from_crystal_template(&template),
            position,
            direction: summon.direction,
            drops: Vec::new(),
        };
        let mut monster = ZoneNativeMonster::from_spawn(&spawn, object_id);
        monster.hostile_to_player = false;
        monster.owner_session_id = Some(summon.owner_session_id);
        monster.master_object_id = summon.owner_object_id;
        monster.summon_skill_level = summon.skill_level;
        monster.visible_extra = true;
        self.native_monsters.insert(object_id, monster);

        let packet = native_summon_spawn_packet(&spawn, object_id, summon.owner_object_id);
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        if let Some(expires_at_ms) = summon.expires_at_ms {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.expires_at_ms = Some(expires_at_ms);
            }
        }
        self.diff_all_zone_object_visibility()
    }

    fn active_native_summon_count_for_profile(
        &self,
        owner_object_id: u32,
        profile: NativeSummonSpellProfile,
    ) -> usize {
        self.active_native_summon_count_for_scope(
            owner_object_id,
            profile.monster_name,
            profile.limit_scope,
        )
    }

    fn active_native_summon_count_for_scope(
        &self,
        owner_object_id: u32,
        monster_name: &str,
        limit_scope: NativeSummonLimitScope,
    ) -> usize {
        self.native_monsters
            .values()
            .filter(|monster| {
                !monster.dead
                    && monster.master_object_id == owner_object_id
                    && match limit_scope {
                        NativeSummonLimitScope::SameMonster => {
                            monster.name.eq_ignore_ascii_case(monster_name)
                        }
                        NativeSummonLimitScope::AllOwned => true,
                    }
            })
            .count()
    }

    fn active_native_summon_object_id(
        &self,
        owner_object_id: u32,
        monster_name: &str,
    ) -> Option<u32> {
        self.native_monsters
            .iter()
            .find_map(|(object_id, monster)| {
                (!monster.dead
                    && monster.master_object_id == owner_object_id
                    && monster.name.eq_ignore_ascii_case(monster_name))
                .then_some(*object_id)
            })
    }

    fn active_native_totem_minion_count(&self, totem_object_id: u32) -> usize {
        self.native_monsters
            .values()
            .filter(|monster| {
                !monster.dead
                    && monster.master_object_id == totem_object_id
                    && monster.name == "CharmedSnake"
            })
            .count()
    }

    fn recall_native_summon(
        &mut self,
        object_id: u32,
        player_position: &Point,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        if monster.dead {
            return Vec::new();
        }
        monster.position = player_position.clone();
        let direction = monster.direction;
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: player_position.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let mut outbounds = self.diff_all_zone_object_visibility();
        let recipients = self.native_monster_visible_recipients(object_id, player_position);
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        outbounds
    }

    fn native_summon_spawn_position(
        &self,
        player_position: &Point,
        direction: MirDirection,
        target: &Point,
        profile: NativeSummonSpellProfile,
    ) -> Point {
        let requested = if profile.spawn_at_target {
            target.clone()
        } else if zone_tile_distance(player_position, target) <= 1 {
            target.clone()
        } else {
            offset_point(player_position, direction, 1)
        };
        self.first_available_position(requested, None)
    }

    fn native_summon_target_point_allowed(
        &self,
        object_id: u32,
        profile: NativeSummonSpellProfile,
        player: &ZonePlayer,
        target: &Point,
    ) -> bool {
        if !profile.spawn_at_target {
            return zone_summon_target_point_allowed(&player.position, target);
        }
        if !points_within_action_range(&player.position, target, ZONE_NATIVE_PLAYER_MAGIC_MAX) {
            return false;
        }
        if object_id == 0 {
            return !self.collision.is_blocked(target);
        }
        self.native_monsters.get(&object_id).is_some_and(|monster| {
            !monster.dead
                && monster.hp > 0
                && monster.hostile_to_player
                && monster.position == *target
        })
    }

    fn expire_native_snake_summons(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let totem_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                self.native_snake_totem_should_die(*object_id, monster, now_ms)
                    .then_some(*object_id)
            })
            .collect::<Vec<_>>();
        let minion_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                self.native_charmed_snake_should_die(*object_id, monster, now_ms)
                    .then_some(*object_id)
            })
            .collect::<Vec<_>>();

        let mut outbounds = Vec::new();
        for object_id in totem_object_ids {
            outbounds.extend(self.kill_native_snake_totem(object_id, now_ms));
        }
        for object_id in minion_object_ids {
            outbounds.extend(self.explode_native_charmed_snake(object_id, now_ms));
        }
        outbounds
    }

    fn native_snake_totem_should_die(
        &self,
        object_id: u32,
        monster: &ZoneNativeMonster,
        now_ms: u64,
    ) -> bool {
        if monster.dead || monster.name != "SnakeTotem" {
            return false;
        }
        let lifetime_expired = self
            .objects
            .get(&object_id)
            .and_then(|object| object.expires_at_ms)
            .is_some_and(|expires_at_ms| now_ms >= expires_at_ms);
        let master_missing_or_far = monster
            .owner_session_id
            .as_ref()
            .and_then(|session_id| self.players.get(session_id))
            .map(|player| {
                player.dead
                    || player.object_id != zone_native_summon_owner_player_object_id(monster)
                    || zone_tile_distance(&monster.position, &player.position) > 15
            })
            .unwrap_or(true);
        lifetime_expired || master_missing_or_far
    }

    fn native_charmed_snake_should_die(
        &self,
        object_id: u32,
        monster: &ZoneNativeMonster,
        now_ms: u64,
    ) -> bool {
        if monster.dead || monster.name != "CharmedSnake" {
            return false;
        }
        let lifetime_expired = self
            .objects
            .get(&object_id)
            .and_then(|object| object.expires_at_ms)
            .is_some_and(|expires_at_ms| now_ms >= expires_at_ms);
        let totem_missing_or_far = self
            .native_monsters
            .get(&monster.master_object_id)
            .map(|totem| {
                totem.dead
                    || zone_tile_distance(&monster.position, &totem.position) > 15
                    || totem.name != "SnakeTotem"
            })
            .unwrap_or(true);
        lifetime_expired || totem_missing_or_far
    }

    fn kill_native_snake_totem(&mut self, object_id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let Some(totem) = self.native_monsters.get(&object_id).cloned() else {
            return Vec::new();
        };
        if totem.dead || totem.name != "SnakeTotem" {
            return Vec::new();
        }

        if let Some(monster) = self.native_monsters.get_mut(&object_id) {
            monster.hp = 0;
            monster.dead = true;
        }
        self.pending_native_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        self.pending_native_player_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.expires_at_ms = Some(now_ms.saturating_add(3_000));
        }

        let position = totem.position.clone();
        let died_packet = ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id,
                location: position.clone(),
                direction: totem.direction,
                kind: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&died_packet), now_ms);
        let mut outbounds = Vec::new();
        let recipients = self.native_monster_visible_recipients(object_id, &position);
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![died_packet],
            });
        }

        let minion_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(minion_object_id, monster)| {
                (monster.name == "CharmedSnake"
                    && monster.master_object_id == object_id
                    && !monster.dead)
                    .then_some(*minion_object_id)
            })
            .collect::<Vec<_>>();
        for minion_object_id in minion_object_ids {
            outbounds.extend(self.explode_native_charmed_snake(minion_object_id, now_ms));
        }
        outbounds
    }

    fn explode_native_charmed_snake(&mut self, object_id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let Some(snake) = self.native_monsters.get(&object_id).cloned() else {
            return Vec::new();
        };
        if snake.dead || snake.name != "CharmedSnake" {
            return Vec::new();
        }

        if let Some(monster) = self.native_monsters.get_mut(&object_id) {
            monster.hp = 0;
            monster.dead = true;
        }
        self.pending_native_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        self.pending_native_player_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.expires_at_ms = Some(now_ms);
        }

        let position = snake.position.clone();
        let died_packet = ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id,
                location: position.clone(),
                direction: snake.direction,
                kind: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&died_packet), now_ms);
        let mut outbounds = Vec::new();
        let recipients = self.native_monster_visible_recipients(object_id, &position);
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![died_packet],
            });
        }

        let Some(owner_session_id) = snake.owner_session_id.clone() else {
            return outbounds;
        };
        let damage = zone_summoned_pet_explosion_damage(snake.summon_skill_level);
        let target_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(target_object_id, monster)| {
                (*target_object_id != object_id
                    && monster.hostile_to_player
                    && !monster.dead
                    && monster.hp > 0
                    && (monster.position.x - position.x).abs() <= 1
                    && (monster.position.y - position.y).abs() <= 1)
                    .then_some(*target_object_id)
            })
            .collect::<Vec<_>>();
        for target_object_id in target_object_ids {
            outbounds.extend(self.resolve_pending_native_monster_hit(
                PendingNativeMonsterHit {
                    ready_at_ms: now_ms,
                    session_id: owner_session_id.clone(),
                    attacker_object_id: object_id,
                    object_id: target_object_id,
                    damage,
                },
                now_ms,
            ));
        }
        outbounds
    }

    fn expire_native_vampire_spiders(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let expired_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                self.native_vampire_spider_should_self_destruct(*object_id, monster, now_ms)
                    .then_some(*object_id)
            })
            .collect::<Vec<_>>();
        let mut outbounds = Vec::new();
        for object_id in expired_object_ids {
            outbounds.extend(self.explode_native_vampire_spider(object_id, now_ms));
        }
        outbounds
    }

    fn native_vampire_spider_should_self_destruct(
        &self,
        object_id: u32,
        monster: &ZoneNativeMonster,
        now_ms: u64,
    ) -> bool {
        if monster.dead || monster.name != "VampireSpider" {
            return false;
        }
        let lifetime_expired = self
            .objects
            .get(&object_id)
            .and_then(|object| object.expires_at_ms)
            .is_some_and(|expires_at_ms| now_ms >= expires_at_ms);
        let master_missing_or_far = monster
            .owner_session_id
            .as_ref()
            .and_then(|session_id| self.players.get(session_id))
            .map(|player| {
                player.dead
                    || player.object_id != zone_native_summon_owner_player_object_id(monster)
                    || zone_tile_distance(&monster.position, &player.position) > 15
            })
            .unwrap_or(true);
        lifetime_expired || master_missing_or_far
    }

    fn explode_native_vampire_spider(&mut self, object_id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let Some(spider) = self.native_monsters.get(&object_id).cloned() else {
            return Vec::new();
        };
        if spider.dead || spider.name != "VampireSpider" {
            return Vec::new();
        }

        if let Some(monster) = self.native_monsters.get_mut(&object_id) {
            monster.hp = 0;
            monster.dead = true;
        }
        self.pending_native_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        self.pending_native_player_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.expires_at_ms = Some(now_ms);
        }

        let position = spider.position.clone();
        let died_packet = ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id,
                location: position.clone(),
                direction: spider.direction,
                kind: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&died_packet), now_ms);
        let mut outbounds = Vec::new();
        let recipients = self.native_monster_visible_recipients(object_id, &position);
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![died_packet],
            });
        }

        let Some(owner_session_id) = spider.owner_session_id.clone() else {
            return outbounds;
        };
        let damage = zone_summoned_pet_explosion_damage(spider.summon_skill_level);
        let target_object_ids = self
            .native_monsters
            .iter()
            .filter_map(|(target_object_id, monster)| {
                (*target_object_id != object_id
                    && monster.hostile_to_player
                    && !monster.dead
                    && monster.hp > 0
                    && (monster.position.x - position.x).abs() <= 1
                    && (monster.position.y - position.y).abs() <= 1)
                    .then_some(*target_object_id)
            })
            .collect::<Vec<_>>();
        for target_object_id in target_object_ids {
            outbounds.extend(self.resolve_pending_native_monster_hit(
                PendingNativeMonsterHit {
                    ready_at_ms: now_ms,
                    session_id: owner_session_id.clone(),
                    attacker_object_id: object_id,
                    object_id: target_object_id,
                    damage,
                },
                now_ms,
            ));
        }
        outbounds
    }

    fn tick_native_monsters(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let monster_ids = self
            .native_monsters
            .iter()
            .filter_map(|(object_id, monster)| {
                (!monster.dead && now_ms >= monster.next_ai_ready_at_ms).then_some(*object_id)
            })
            .collect::<Vec<_>>();
        let mut outbounds = Vec::new();
        for object_id in monster_ids {
            outbounds.extend(self.tick_native_monster(object_id, now_ms));
        }
        outbounds
    }

    fn tick_native_monster(&mut self, object_id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get(&object_id).cloned() else {
            return Vec::new();
        };
        if monster.dead {
            return Vec::new();
        }
        if native_monster_control_active(&monster, now_ms) {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms =
                    monster.next_ai_ready_at_ms.max(monster.control_until_ms);
                monster.next_attack_ready_at_ms = monster
                    .next_attack_ready_at_ms
                    .max(monster.control_until_ms);
            }
            return Vec::new();
        }
        if !monster.hostile_to_player {
            if monster.owner_session_id.is_some() && monster.master_object_id != 0 {
                return self.tick_native_summon_monster(object_id, monster, now_ms);
            }
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        }
        if let Some(decoy) = self.nearest_native_monster_decoy_target(object_id, &monster.position)
        {
            let Some(direction) = zone_direction_toward(&monster.position, &decoy.position) else {
                return Vec::new();
            };
            if native_monster_adjacent_to(&monster.position, &decoy.position) {
                return self
                    .launch_native_monster_decoy_attack(object_id, decoy, direction, now_ms);
            }

            let destination = offset_point(&monster.position, direction, 1);
            if !self.can_native_monster_occupy(object_id, &destination) {
                return self.turn_native_monster(object_id, direction, now_ms);
            }

            {
                let monster = self
                    .native_monsters
                    .get_mut(&object_id)
                    .expect("native monster id should still exist");
                monster.position = destination.clone();
                monster.direction = direction;
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id,
                    position: destination.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
            let mut outbounds = self.diff_all_zone_object_visibility();
            let recipients = self.native_monster_visible_recipients(object_id, &destination);
            if !recipients.is_empty() {
                outbounds.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                });
            }
            return outbounds;
        }
        let Some(target) = self.nearest_native_monster_target(&monster.position) else {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        };
        let Some(direction) = zone_direction_toward(&monster.position, &target.position) else {
            return Vec::new();
        };
        if zone_native_monster_prefers_ranged(&monster, &target.position)
            && points_within_action_range(
                &monster.position,
                &target.position,
                ZONE_NATIVE_MONSTER_RANGED_MAX,
            )
        {
            return self
                .launch_native_monster_player_ranged_attack(object_id, target, direction, now_ms);
        }
        if native_monster_adjacent_to(&monster.position, &target.position) {
            return self.launch_native_monster_player_attack(object_id, target, direction, now_ms);
        }

        let destination = offset_point(&monster.position, direction, 1);
        if !self.can_native_monster_occupy(object_id, &destination) {
            return self.turn_native_monster(object_id, direction, now_ms);
        }

        {
            let monster = self
                .native_monsters
                .get_mut(&object_id)
                .expect("native monster id should still exist");
            monster.position = destination.clone();
            monster.direction = direction;
            monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        }
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: destination.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let mut outbounds = self.diff_all_zone_object_visibility();
        let recipients = self.native_monster_visible_recipients(object_id, &destination);
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        outbounds
    }

    fn tick_native_summon_monster(
        &mut self,
        object_id: u32,
        summon: ZoneNativeMonster,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(owner_session_id) = summon.owner_session_id.clone() else {
            return Vec::new();
        };
        let owner_player_object_id = zone_native_summon_owner_player_object_id(&summon);
        let owner_present = self
            .players
            .get(&owner_session_id)
            .is_some_and(|player| player.object_id == owner_player_object_id && !player.dead);
        if !owner_present {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        }
        if summon.name == "SnakeTotem" {
            return self.tick_native_snake_totem(object_id, summon, now_ms);
        }
        let Some(target) = self.nearest_native_summon_target(object_id, &summon.position) else {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        };
        let Some(direction) = zone_direction_toward(&summon.position, &target.position) else {
            return Vec::new();
        };
        let attack_range = zone_native_summon_attack_range(&summon);
        if zone_native_monster_prefers_ranged(&summon, &target.position)
            && points_within_action_range(&summon.position, &target.position, attack_range)
        {
            return self.launch_native_summon_monster_ranged_attack(
                object_id,
                &owner_session_id,
                target,
                direction,
                now_ms,
            );
        }
        if attack_range > 0 && native_monster_adjacent_to(&summon.position, &target.position) {
            return self.launch_native_summon_monster_attack(
                object_id,
                &owner_session_id,
                target,
                direction,
                now_ms,
            );
        }
        if !zone_native_summon_can_move(&summon) {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        }

        let destination = offset_point(&summon.position, direction, 1);
        if !self.can_native_monster_occupy(object_id, &destination) {
            return self.turn_native_monster(object_id, direction, now_ms);
        }

        {
            let monster = self
                .native_monsters
                .get_mut(&object_id)
                .expect("native summon id should still exist");
            monster.position = destination.clone();
            monster.direction = direction;
            monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        }
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id,
                position: destination.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let mut outbounds = self.diff_all_zone_object_visibility();
        let recipients = self.native_monster_visible_recipients(object_id, &destination);
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        outbounds
    }

    fn tick_native_snake_totem(
        &mut self,
        object_id: u32,
        summon: ZoneNativeMonster,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(owner_session_id) = summon.owner_session_id.clone() else {
            return Vec::new();
        };
        let Some(target) = self.nearest_native_summon_target(object_id, &summon.position) else {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        };
        let minion_cap = usize::from(summon.summon_skill_level).saturating_add(1);
        if self.active_native_totem_minion_count(object_id) >= minion_cap {
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            }
            return Vec::new();
        }
        let Some(template) = crystal_monster_by_name("CharmedSnake") else {
            return Vec::new();
        };
        let Some(direction) = zone_direction_toward(&summon.position, &target.position) else {
            return Vec::new();
        };
        let totem_position = summon.position.clone();
        let spawn_position = self
            .first_available_position(offset_point(&totem_position, MirDirection::Down, 1), None);
        let minion_object_id = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            object_id: minion_object_id,
            name: template.name.clone(),
            name_colour_argb: -1,
            image: template.image,
            ai: template.ai,
            level: template.level,
            max_hp: template.hp.max(1),
            hp: template.hp.max(1),
            experience: 0,
            defense: super::types::ZoneMonsterDefense::from_crystal_template(&template),
            position: spawn_position,
            direction,
            drops: Vec::new(),
        };
        let mut minion = ZoneNativeMonster::from_spawn(&spawn, minion_object_id);
        minion.hostile_to_player = false;
        minion.owner_session_id = Some(owner_session_id.clone());
        minion.master_object_id = object_id;
        minion.owner_player_object_id = zone_native_summon_owner_player_object_id(&summon);
        minion.visible_extra = true;
        minion.summon_skill_level = summon.summon_skill_level;
        self.native_monsters.insert(minion_object_id, minion);

        if let Some(totem) = self.native_monsters.get_mut(&object_id) {
            totem.direction = direction;
            totem.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        }

        let attack_packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id,
                location: totem_position,
                direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        let spawn_packet = native_summon_spawn_packet(&spawn, minion_object_id, object_id);
        self.apply_zone_object_packets(&[attack_packet.clone(), spawn_packet], now_ms);
        if let Some(object) = self.objects.get_mut(&minion_object_id) {
            let lifetime_ms = 10_000_u64.saturating_add(
                u64::try_from(minion_cap.saturating_sub(1))
                    .unwrap_or_default()
                    .saturating_mul(2_000),
            );
            object.expires_at_ms = Some(now_ms.saturating_add(lifetime_ms));
        }
        let mut outbounds = self.diff_all_zone_object_visibility();
        let recipients = self.native_monster_action_recipients(
            &owner_session_id,
            object_id,
            target.object_id,
            &target.position,
        );
        if !recipients.is_empty() {
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![attack_packet],
            });
        }
        outbounds
    }

    fn turn_native_monster(
        &mut self,
        object_id: u32,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        monster.direction = direction;
        monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        let position = monster.position.clone();
        let packet = ServerPacket::ObjectTurn {
            movement: ObjectMovement {
                object_id,
                position: position.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients = self.native_monster_visible_recipients(object_id, &position);
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }]
    }

    fn launch_native_monster_player_attack(
        &mut self,
        object_id: u32,
        target: NativeMonsterTarget,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        monster.direction = direction;
        monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        if now_ms < monster.next_attack_ready_at_ms {
            return Vec::new();
        }
        monster.next_attack_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_ATTACK_MS);
        let monster_position = monster.position.clone();
        // Roll the monster's melee damage from its Crystal stats (matching the
        // ranged path) instead of a fixed placeholder of 1, so monster→player
        // damage is the zone's authoritative, data-driven value.
        let (damage, magic) = zone_native_monster_player_attack_damage(monster, &target.position);
        let attacker_ai = monster.ai;
        if damage > 0 {
            self.pending_native_player_hits
                .push(PendingNativePlayerHit {
                    ready_at_ms: now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS),
                    attacker_object_id: object_id,
                    attacker_ai,
                    target_session_id: target.session_id.clone(),
                    target_object_id: target.object_id,
                    damage,
                    magic,
                });
        }
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id,
                location: monster_position,
                direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients =
            self.native_monster_combat_recipients(object_id, target.object_id, &target.position);
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }]
    }

    fn launch_native_monster_player_ranged_attack(
        &mut self,
        object_id: u32,
        target: NativeMonsterTarget,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some((monster_position, attack_type, (damage, magic), attacker_ai)) = ({
            let Some(monster) = self.native_monsters.get_mut(&object_id) else {
                return Vec::new();
            };
            monster.direction = direction;
            monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            if now_ms < monster.next_attack_ready_at_ms {
                return Vec::new();
            }
            monster.next_attack_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_ATTACK_MS);
            Some((
                monster.position.clone(),
                zone_native_monster_range_attack_type(
                    monster.ai,
                    &monster.position,
                    &target.position,
                ),
                zone_native_monster_player_attack_damage(monster, &target.position),
                monster.ai,
            ))
        }) else {
            return Vec::new();
        };
        if damage > 0 {
            self.pending_native_player_hits
                .push(PendingNativePlayerHit {
                    ready_at_ms: now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS),
                    attacker_object_id: object_id,
                    attacker_ai,
                    target_session_id: target.session_id.clone(),
                    target_object_id: target.object_id,
                    damage,
                    magic,
                });
        }
        let packet = ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id,
                location: monster_position,
                direction,
                target_id: target.object_id,
                target: target.position.clone(),
                attack_type,
                spell: 0,
                level: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients =
            self.native_monster_combat_recipients(object_id, target.object_id, &target.position);
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }]
    }

    fn launch_native_monster_decoy_attack(
        &mut self,
        object_id: u32,
        target: NativeMonsterDecoyTarget,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        monster.direction = direction;
        monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        if now_ms < monster.next_attack_ready_at_ms {
            return Vec::new();
        }
        monster.next_attack_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_ATTACK_MS);
        let monster_position = monster.position.clone();
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id,
                location: monster_position,
                direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients =
            self.native_monster_combat_recipients(object_id, target.object_id, &target.position);
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }]
    }

    fn launch_native_summon_monster_attack(
        &mut self,
        object_id: u32,
        owner_session_id: &SessionId,
        target: NativeSummonTarget,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return Vec::new();
        };
        monster.direction = direction;
        monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        if now_ms < monster.next_attack_ready_at_ms {
            return Vec::new();
        }
        monster.next_attack_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_ATTACK_MS);
        let monster_position = monster.position.clone();
        let damage = zone_native_summon_monster_attack_damage(monster, &target.position);
        self.pending_native_hits.push(PendingNativeMonsterHit {
            ready_at_ms: now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS),
            session_id: owner_session_id.clone(),
            attacker_object_id: object_id,
            object_id: target.object_id,
            damage,
        });
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id,
                location: monster_position,
                direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients = self.native_monster_action_recipients(
            owner_session_id,
            object_id,
            target.object_id,
            &target.position,
        );
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }]
    }

    fn launch_native_summon_monster_ranged_attack(
        &mut self,
        object_id: u32,
        owner_session_id: &SessionId,
        target: NativeSummonTarget,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some((monster_position, attack_type, damage, hit_delay_ms)) = ({
            let Some(monster) = self.native_monsters.get_mut(&object_id) else {
                return Vec::new();
            };
            monster.direction = direction;
            monster.next_ai_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
            if now_ms < monster.next_attack_ready_at_ms {
                return Vec::new();
            }
            monster.next_attack_ready_at_ms = now_ms.saturating_add(ZONE_NATIVE_MONSTER_ATTACK_MS);
            Some((
                monster.position.clone(),
                zone_native_monster_range_attack_type(
                    monster.ai,
                    &monster.position,
                    &target.position,
                ),
                zone_native_summon_monster_attack_damage(monster, &target.position),
                zone_native_summon_hit_delay_ms(monster, &target.position),
            ))
        }) else {
            return Vec::new();
        };
        if damage > 0 {
            self.pending_native_hits.push(PendingNativeMonsterHit {
                ready_at_ms: now_ms.saturating_add(hit_delay_ms),
                session_id: owner_session_id.clone(),
                attacker_object_id: object_id,
                object_id: target.object_id,
                damage,
            });
        }
        let packet = ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id,
                location: monster_position,
                direction,
                target_id: target.object_id,
                target: target.position.clone(),
                attack_type,
                spell: 0,
                level: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let recipients = self.native_monster_action_recipients(
            owner_session_id,
            object_id,
            target.object_id,
            &target.position,
        );
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }]
    }

    fn nearest_native_monster_target(&self, position: &Point) -> Option<NativeMonsterTarget> {
        self.players
            .values()
            .filter(|player| !player.dead && !player.hidden)
            .filter(|player| {
                (player.position.x - position.x).abs() <= ZONE_NATIVE_MONSTER_AGGRO_X
                    && (player.position.y - position.y).abs() <= ZONE_NATIVE_MONSTER_AGGRO_Y
            })
            .min_by_key(|player| {
                (player.position.x - position.x)
                    .abs()
                    .saturating_add((player.position.y - position.y).abs())
            })
            .map(|player| NativeMonsterTarget {
                session_id: player.session_id.clone(),
                object_id: player.object_id,
                position: player.position.clone(),
            })
    }

    fn nearest_native_monster_decoy_target(
        &self,
        source_object_id: u32,
        position: &Point,
    ) -> Option<NativeMonsterDecoyTarget> {
        self.native_monsters
            .iter()
            .filter(|(object_id, monster)| {
                **object_id != source_object_id
                    && monster.name == "StoneTrap"
                    && !monster.hostile_to_player
                    && !monster.dead
                    && monster.hp > 0
            })
            .filter(|(_, monster)| {
                (monster.position.x - position.x).abs() <= ZONE_NATIVE_MONSTER_AGGRO_X
                    && (monster.position.y - position.y).abs() <= ZONE_NATIVE_MONSTER_AGGRO_Y
            })
            .min_by_key(|(object_id, monster)| {
                (zone_tile_distance(&monster.position, position), **object_id)
            })
            .map(|(object_id, monster)| NativeMonsterDecoyTarget {
                object_id: *object_id,
                position: monster.position.clone(),
            })
    }

    fn nearest_native_summon_target(
        &self,
        summon_object_id: u32,
        position: &Point,
    ) -> Option<NativeSummonTarget> {
        self.native_monsters
            .iter()
            .filter(|(object_id, monster)| {
                **object_id != summon_object_id
                    && monster.hostile_to_player
                    && !monster.dead
                    && monster.hp > 0
            })
            .filter(|(_, monster)| {
                (monster.position.x - position.x).abs() <= ZONE_NATIVE_MONSTER_AGGRO_X
                    && (monster.position.y - position.y).abs() <= ZONE_NATIVE_MONSTER_AGGRO_Y
            })
            .min_by_key(|(_, monster)| {
                (monster.position.x - position.x)
                    .abs()
                    .saturating_add((monster.position.y - position.y).abs())
            })
            .map(|(object_id, monster)| NativeSummonTarget {
                object_id: *object_id,
                position: monster.position.clone(),
            })
    }

    fn can_native_monster_occupy(&self, object_id: u32, point: &Point) -> bool {
        if self.collision.is_blocked(point) || self.occupancy.contains_key(&tile_key(point)) {
            return false;
        }
        !self.objects.values().any(|object| {
            object.object_id != object_id && retained_zone_object_blocks_tile(object, point)
        })
    }

    fn native_monster_visible_recipients(
        &self,
        object_id: u32,
        position: &Point,
    ) -> Vec<SessionId> {
        self.players
            .iter()
            .filter_map(|(session_id, player)| {
                (player.visible_object_ids.contains(&object_id)
                    || points_visible(&player.position, position))
                .then(|| session_id.clone())
            })
            .collect()
    }

    fn native_monster_combat_recipients(
        &self,
        monster_object_id: u32,
        target_object_id: u32,
        position: &Point,
    ) -> Vec<SessionId> {
        self.players
            .iter()
            .filter_map(|(session_id, player)| {
                (player.object_id == target_object_id
                    || player.visible_object_ids.contains(&monster_object_id)
                    || player.visible_object_ids.contains(&target_object_id)
                    || points_visible(&player.position, position))
                .then(|| session_id.clone())
            })
            .collect()
    }

    fn player_status_recipients(&self, object_id: u32, position: &Point) -> Vec<SessionId> {
        self.players
            .iter()
            .filter_map(|(session_id, player)| {
                (player.object_id == object_id
                    || player.visible_object_ids.contains(&object_id)
                    || points_visible(&player.position, position))
                .then(|| session_id.clone())
            })
            .collect()
    }

    fn ground_spell_visible_recipients(
        &self,
        caster_session_id: &SessionId,
        locations: &[Point],
    ) -> Vec<SessionId> {
        self.players
            .iter()
            .filter_map(|(session_id, player)| {
                (session_id == caster_session_id
                    || locations
                        .iter()
                        .any(|location| points_visible(&player.position, location)))
                .then(|| session_id.clone())
            })
            .collect()
    }

    fn apply_native_monster_damage(
        &mut self,
        object_id: u32,
        damage: i32,
    ) -> Option<(
        i32,
        u8,
        bool,
        String,
        u32,
        Point,
        MirDirection,
        Vec<GroundDropSnapshot>,
    )> {
        let monster = self.native_monsters.get_mut(&object_id)?;
        if monster.dead {
            return None;
        }

        let damage = damage.max(0).min(monster.hp);
        monster.hp = monster.hp.saturating_sub(damage);
        let killed = monster.hp <= 0;
        if killed {
            monster.hp = 0;
            monster.dead = true;
        }
        Some((
            damage,
            native_monster_health_percent(monster.hp, monster.max_hp),
            killed,
            monster.name.clone(),
            monster.experience,
            monster.position.clone(),
            monster.direction,
            if killed {
                monster.drops.clone()
            } else {
                Vec::new()
            },
        ))
    }

    fn native_monster_action_recipients(
        &self,
        actor_session_id: &SessionId,
        actor_object_id: u32,
        monster_object_id: u32,
        monster_position: &Point,
    ) -> Vec<SessionId> {
        self.players
            .iter()
            .filter_map(|(session_id, player)| {
                (session_id == actor_session_id
                    || player.visible_object_ids.contains(&actor_object_id)
                    || player.visible_object_ids.contains(&monster_object_id)
                    || points_visible(&player.position, monster_position))
                .then(|| session_id.clone())
            })
            .collect()
    }

    fn spawn_native_monster_drops(
        &mut self,
        monster_name: &str,
        position: &Point,
        owner_object_id: u32,
        drops: Vec<GroundDropSnapshot>,
        now_ms: u64,
    ) -> Vec<GroundDropSnapshot> {
        let mut spawned = Vec::new();
        let mut packets = Vec::new();
        for mut drop in drops {
            drop.object_id = self.unique_object_id(drop.object_id);
            if drop.x == 0 && drop.y == 0 {
                drop.x = position.x;
                drop.y = position.y;
            }
            if drop.source_monster.is_empty() {
                drop.source_monster = monster_name.to_string();
            }
            drop.owner_object_id = Some(owner_object_id);
            if drop.ownership_remaining_ticks.is_none() {
                drop.ownership_remaining_ticks = Some(100);
            }
            let owner_expires_at_ms = drop
                .ownership_remaining_ticks
                .filter(|ticks| *ticks > 0)
                .map(|ticks| now_ms.saturating_add(ticks.saturating_mul(300)));
            self.ground_drops.insert(
                drop.object_id,
                ZoneGroundDrop {
                    drop: drop.clone(),
                    owner_expires_at_ms,
                },
            );
            packets.push(ground_drop_spawn_packet(&drop));
            spawned.push(drop);
        }
        if !packets.is_empty() {
            self.apply_zone_object_packets(&packets, now_ms);
        }
        spawned
    }

    fn claim_ground_drop(
        &mut self,
        session_id: &SessionId,
        object_id: Option<u32>,
        target: &Point,
        group_members: &[String],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id).cloned() else {
            return Vec::new();
        };
        self.expire_ground_drop_ownerships(now_ms);
        let Some(object_id) = object_id.or_else(|| {
            self.ground_drops
                .values()
                .find(|drop| drop.drop.x == target.x && drop.drop.y == target.y)
                .map(|drop| drop.drop.object_id)
        }) else {
            return Vec::new();
        };
        let Some(stored) = self.ground_drops.get(&object_id) else {
            return Vec::new();
        };
        if stored.drop.x != target.x || stored.drop.y != target.y {
            return Vec::new();
        }
        if !self.ground_drop_ownership_allows(&stored.drop, player.object_id, group_members) {
            return vec![ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets: vec![ServerPacket::Chat {
                    message: "server.CannotPickupNotOwner".to_string(),
                    chat_type: ChatType::System,
                }],
            }];
        }
        let Some(stored) = self.ground_drops.remove(&object_id) else {
            return Vec::new();
        };
        self.claimed_ground_drops.insert(
            object_id,
            ZoneGroundDropClaim {
                session_id: session_id.clone(),
                drop: stored.drop.clone(),
            },
        );
        let mut outbounds = self.remove_ground_drop_object_for_claim(object_id);
        outbounds.push(ZoneOutbound::GroundDropClaimed {
            session_id: session_id.clone(),
            drop: stored.drop,
        });
        outbounds
    }

    fn claim_nearest_ground_drop(
        &mut self,
        session_id: &SessionId,
        origin: &Point,
        max_range: i32,
        allowed_object_ids: &BTreeSet<u32>,
        group_members: &[String],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id).cloned() else {
            return Vec::new();
        };
        if max_range < 0 || allowed_object_ids.is_empty() {
            return Vec::new();
        }
        self.expire_ground_drop_ownerships(now_ms);
        let Some((object_id, target)) = self
            .ground_drops
            .values()
            .filter(|stored| allowed_object_ids.contains(&stored.drop.object_id))
            .filter(|stored| {
                let distance = (stored.drop.x - origin.x)
                    .abs()
                    .max((stored.drop.y - origin.y).abs());
                distance <= max_range
                    && self.ground_drop_ownership_allows(
                        &stored.drop,
                        player.object_id,
                        group_members,
                    )
            })
            .map(|stored| {
                let distance = (stored.drop.x - origin.x)
                    .abs()
                    .max((stored.drop.y - origin.y).abs());
                (
                    distance,
                    stored.drop.object_id,
                    Point {
                        x: stored.drop.x,
                        y: stored.drop.y,
                    },
                )
            })
            .min_by_key(|(distance, object_id, _)| (*distance, *object_id))
            .map(|(_, object_id, target)| (object_id, target))
        else {
            return Vec::new();
        };
        self.claim_ground_drop(session_id, Some(object_id), &target, group_members, now_ms)
    }

    fn commit_ground_drop_claim(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
    ) -> Vec<ZoneOutbound> {
        if self
            .claimed_ground_drops
            .get(&object_id)
            .is_some_and(|claim| &claim.session_id == session_id)
        {
            self.claimed_ground_drops.remove(&object_id);
            self.removed_object_ids.insert(object_id);
        }
        Vec::new()
    }

    fn cancel_ground_drop_claim(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(claim) = self.claimed_ground_drops.remove(&object_id) else {
            return Vec::new();
        };
        if &claim.session_id != session_id {
            self.claimed_ground_drops.insert(object_id, claim);
            return Vec::new();
        }
        self.removed_object_ids.remove(&object_id);
        self.ground_drops.insert(
            object_id,
            ZoneGroundDrop {
                owner_expires_at_ms: claim
                    .drop
                    .ownership_remaining_ticks
                    .filter(|ticks| *ticks > 0)
                    .map(|ticks| now_ms.saturating_add(ticks.saturating_mul(300))),
                drop: claim.drop.clone(),
            },
        );
        self.apply_zone_object_packets(&[ground_drop_spawn_packet(&claim.drop)], now_ms);
        self.diff_all_zone_object_visibility()
    }

    fn broadcast_shared_object_packets(
        &mut self,
        session_id: &SessionId,
        local_self_object_id: Option<u32>,
        packets: &[ServerPacket],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id).cloned() else {
            return Vec::new();
        };
        let visible_objects_before = self.visible_zone_objects_by_session();
        let observer_packets = packets
            .iter()
            .filter_map(|packet| shared_object_action_packet(&player, local_self_object_id, packet))
            .filter_map(|packet| self.canonical_observer_zone_object_packet(packet))
            .collect::<Vec<_>>();
        if observer_packets.is_empty() {
            return Vec::new();
        }

        let delivery_anchors = self.shared_object_delivery_anchors(
            &player,
            &observer_packets,
            &visible_objects_before,
        );
        let anchored_packets = observer_packets
            .into_iter()
            .enumerate()
            .filter_map(|(index, packet)| {
                let anchors = delivery_anchors.get(index)?.clone();
                (!anchors.is_empty()).then_some((packet, anchors))
            })
            .collect::<Vec<_>>();
        let observer_packets = anchored_packets
            .iter()
            .map(|(packet, _)| packet.clone())
            .collect::<Vec<_>>();
        let delivery_anchors = anchored_packets
            .into_iter()
            .map(|(_, anchors)| anchors)
            .collect::<Vec<_>>();
        if observer_packets.is_empty() {
            return Vec::new();
        }

        let visible_objects_before = self.visible_zone_objects_by_session();
        let has_retained_object_packets = observer_packets.iter().any(|packet| {
            self.packet_touches_retained_zone_object(packet, &visible_objects_before)
        });
        self.apply_zone_object_packets(&observer_packets, now_ms);
        let mut outbounds = self.dispatch_shared_object_observer_packets(
            session_id,
            &observer_packets,
            &visible_objects_before,
            &delivery_anchors,
        );
        if has_retained_object_packets {
            outbounds.extend(self.diff_all_zone_object_visibility_except(session_id));
        }
        outbounds
    }

    fn canonical_observer_zone_object_packet(&self, packet: ServerPacket) -> Option<ServerPacket> {
        if let Some(mut object) = retained_zone_object_from_packet(&packet) {
            if self.removed_object_ids.contains(&object.object_id) {
                return None;
            }
            if let Some(dead_state) = self
                .dead_object_ids
                .get(&object.object_id)
                .or_else(|| self.harvested_dead_state(&object.object_id))
            {
                apply_retained_dead_state(&mut object, dead_state);
                return Some(object.packet);
            }
            if self.revived_object_ids.contains(&object.object_id) {
                apply_retained_revived_state(&mut object);
                return Some(object.packet);
            }
            return Some(packet);
        }
        if let Some(object_id) = retained_zone_object_update_id(&packet) {
            if self.removed_object_ids.contains(&object_id)
                && retained_zone_object_revive_id(&packet).is_none()
            {
                return None;
            }
            if self.dead_object_ids.contains_key(&object_id)
                && retained_zone_object_dead_stale_update(&packet)
            {
                return None;
            }
            if self.harvested_object_ids.contains(&object_id)
                && retained_zone_object_harvest_id(&packet).is_some()
            {
                return None;
            }
            if self.retained_zone_object_health_is_stale(object_id, &packet) {
                return None;
            }
        }
        Some(packet)
    }

    fn dispatch_observer_packets(
        &mut self,
        actor_session_id: &SessionId,
        actor: &ZonePlayer,
        packets: &[ServerPacket],
        visible_objects_before: &BTreeMap<SessionId, BTreeSet<u32>>,
        object_positions_before: &BTreeMap<u32, Point>,
    ) -> Vec<ZoneOutbound> {
        let packets_by_session = self
            .players
            .iter()
            .filter(|(other_session_id, _)| *other_session_id != actor_session_id)
            .filter_map(|(other_session_id, other)| {
                let packets = packets
                    .iter()
                    .filter(|packet| {
                        self.should_deliver_observer_packet(
                            other_session_id,
                            other,
                            actor,
                            packet,
                            visible_objects_before,
                            object_positions_before,
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                (!packets.is_empty()).then(|| (other_session_id.clone(), packets))
            })
            .collect::<Vec<_>>();

        let mut outbounds = Vec::new();
        for (session_id, packets) in packets_by_session {
            self.record_zone_object_visibility(std::slice::from_ref(&session_id), &packets);
            outbounds.push(ZoneOutbound::ToSession {
                session_id,
                packets,
            });
        }
        outbounds
    }

    fn dispatch_shared_object_observer_packets(
        &mut self,
        actor_session_id: &SessionId,
        packets: &[ServerPacket],
        visible_objects_before: &BTreeMap<SessionId, BTreeSet<u32>>,
        delivery_anchors: &[BTreeSet<u32>],
    ) -> Vec<ZoneOutbound> {
        let packets_by_session = self
            .players
            .keys()
            .filter(|other_session_id| *other_session_id != actor_session_id)
            .filter_map(|other_session_id| {
                let visible = visible_objects_before
                    .get(other_session_id)
                    .cloned()
                    .unwrap_or_default();
                let packets = packets
                    .iter()
                    .zip(delivery_anchors.iter())
                    .filter(|(_, anchors)| {
                        anchors.iter().any(|object_id| visible.contains(object_id))
                    })
                    .map(|(packet, _)| packet.clone())
                    .collect::<Vec<_>>();
                (!packets.is_empty()).then(|| (other_session_id.clone(), packets))
            })
            .collect::<Vec<_>>();

        let mut outbounds = Vec::new();
        for (session_id, packets) in packets_by_session {
            self.record_zone_object_visibility(std::slice::from_ref(&session_id), &packets);
            outbounds.push(ZoneOutbound::ToSession {
                session_id,
                packets,
            });
        }
        outbounds
    }

    fn shared_object_delivery_anchors(
        &self,
        player: &ZonePlayer,
        packets: &[ServerPacket],
        visible_objects_before: &BTreeMap<SessionId, BTreeSet<u32>>,
    ) -> Vec<BTreeSet<u32>> {
        let actor_ids = packets
            .iter()
            .filter_map(shared_object_actor_id)
            .filter(|object_id| {
                self.objects.contains_key(object_id)
                    || visible_objects_before
                        .values()
                        .any(|object_ids| object_ids.contains(object_id))
            })
            .collect::<BTreeSet<_>>();
        let local_self_result_anchor_ids = packets
            .iter()
            .filter_map(|packet| match packet {
                ServerPacket::ObjectStruck { info }
                    if info.object_id == player.object_id
                        && actor_ids.contains(&info.attacker_id) =>
                {
                    Some(info.attacker_id)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        packets
            .iter()
            .map(|packet| {
                let mut anchors = BTreeSet::new();
                if let Some(object_id) = shared_object_actor_id(packet) {
                    if actor_ids.contains(&object_id) {
                        anchors.insert(object_id);
                    }
                }
                if let Some(object_id) = shared_object_result_id(packet) {
                    if object_id == player.object_id {
                        anchors.extend(local_self_result_anchor_ids.iter().copied());
                    } else if self.objects.contains_key(&object_id)
                        || visible_objects_before
                            .values()
                            .any(|object_ids| object_ids.contains(&object_id))
                    {
                        anchors.insert(object_id);
                    }
                }
                anchors
            })
            .collect()
    }

    fn should_deliver_observer_packet(
        &self,
        observer_session_id: &SessionId,
        observer: &ZonePlayer,
        actor: &ZonePlayer,
        packet: &ServerPacket,
        visible_objects_before: &BTreeMap<SessionId, BTreeSet<u32>>,
        object_positions_before: &BTreeMap<u32, Point>,
    ) -> bool {
        let visible_before =
            visible_objects_before
                .get(observer_session_id)
                .is_some_and(|object_ids| {
                    retained_zone_object_from_packet(packet)
                        .map(|object| object_ids.contains(&object.object_id))
                        .or_else(|| {
                            retained_zone_object_remove_id(packet).map(|object_id| {
                                object_ids.contains(&object_id)
                                    || object_positions_before.get(&object_id).is_some_and(
                                        |position| points_visible(&observer.position, position),
                                    )
                            })
                        })
                        .or_else(|| {
                            retained_zone_object_update_id(packet)
                                .filter(|object_id| {
                                    self.objects.contains_key(object_id)
                                        || object_ids.contains(object_id)
                                })
                                .map(|object_id| object_ids.contains(&object_id))
                        })
                        .unwrap_or(false)
                });
        if visible_before {
            return true;
        }

        if retained_zone_object_from_packet(packet).is_some() {
            return false;
        }
        if retained_zone_object_remove_id(packet).is_some_and(|object_id| {
            self.objects.contains_key(&object_id)
                || object_positions_before.contains_key(&object_id)
                || visible_objects_before
                    .values()
                    .any(|object_ids| object_ids.contains(&object_id))
        }) {
            return false;
        }
        if retained_zone_object_update_id(packet).is_some_and(|object_id| {
            self.objects.contains_key(&object_id)
                || visible_objects_before
                    .values()
                    .any(|object_ids| object_ids.contains(&object_id))
        }) {
            return false;
        }

        observer.visible_object_ids.contains(&actor.object_id)
    }

    fn packet_touches_retained_zone_object(
        &self,
        packet: &ServerPacket,
        visible_objects_before: &BTreeMap<SessionId, BTreeSet<u32>>,
    ) -> bool {
        if retained_zone_object_from_packet(packet).is_some() {
            return true;
        }
        if let Some(object_id) = retained_zone_object_remove_id(packet)
            .or_else(|| retained_zone_object_update_id(packet))
        {
            return self.objects.contains_key(&object_id)
                || visible_objects_before
                    .values()
                    .any(|object_ids| object_ids.contains(&object_id));
        }
        false
    }

    fn visible_zone_objects_by_session(&self) -> BTreeMap<SessionId, BTreeSet<u32>> {
        self.players
            .iter()
            .map(|(session_id, player)| (session_id.clone(), player.visible_object_ids.clone()))
            .collect()
    }

    fn zone_object_positions(&self) -> BTreeMap<u32, Point> {
        self.objects
            .iter()
            .map(|(object_id, object)| (*object_id, object.position.clone()))
            .collect()
    }

    fn diff_all_zone_object_visibility_except(
        &mut self,
        skipped_session_id: &SessionId,
    ) -> Vec<ZoneOutbound> {
        self.players
            .keys()
            .filter(|session_id| *session_id != skipped_session_id)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|session_id| self.diff_zone_object_visibility_for(&session_id))
            .collect()
    }

    fn diff_all_zone_object_visibility(&mut self) -> Vec<ZoneOutbound> {
        self.players
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|session_id| self.diff_zone_object_visibility_for(&session_id))
            .collect()
    }

    fn record_zone_object_visibility(
        &mut self,
        session_ids: &[SessionId],
        packets: &[ServerPacket],
    ) {
        let object_ids = packets
            .iter()
            .filter_map(|packet| {
                retained_zone_object_from_packet(packet)
                    .map(|object| object.object_id)
                    .or_else(|| {
                        retained_zone_object_update_id(packet)
                            .filter(|object_id| self.objects.contains_key(object_id))
                    })
            })
            .collect::<Vec<_>>();
        if object_ids.is_empty() {
            return;
        }
        for session_id in session_ids {
            if let Some(player) = self.players.get_mut(session_id) {
                player.visible_object_ids.extend(object_ids.iter().copied());
            }
        }
    }

    fn apply_zone_object_packets(&mut self, packets: &[ServerPacket], now_ms: u64) {
        for packet in packets {
            if let Some(object_id) = retained_zone_object_remove_id(packet) {
                self.remove_retained_zone_object(object_id);
                continue;
            }
            if let Some(object_id) = retained_zone_object_revive_id(packet) {
                self.removed_object_ids.remove(&object_id);
                self.dead_object_ids.remove(&object_id);
                self.harvested_object_ids.remove(&object_id);
                self.revived_object_ids.insert(object_id);
            }
            if let Some((object_id, dead_state)) = retained_zone_object_harvested_state(packet) {
                if !self.object_id_belongs_to_player(object_id)
                    && !self.removed_object_ids.contains(&object_id)
                    && !self.revived_object_ids.contains(&object_id)
                {
                    self.harvested_object_ids.insert(object_id);
                    self.dead_object_ids.insert(object_id, dead_state);
                }
            }
            if let Some((object_id, dead_state)) = retained_zone_object_dead_state(packet) {
                if !self.removed_object_ids.contains(&object_id)
                    && !self.revived_object_ids.contains(&object_id)
                {
                    self.dead_object_ids.insert(object_id, dead_state);
                }
            }
            if let Some(mut object) = retained_zone_object_from_packet(packet) {
                if self.removed_object_ids.contains(&object.object_id) {
                    continue;
                }
                if let Some(dead_state) = self
                    .dead_object_ids
                    .get(&object.object_id)
                    .or_else(|| self.harvested_dead_state(&object.object_id))
                {
                    apply_retained_dead_state(&mut object, dead_state);
                } else if self.revived_object_ids.contains(&object.object_id) {
                    let arrived_dead = retained_zone_object_dead_state(&object.packet).is_some();
                    apply_retained_revived_state(&mut object);
                    if !arrived_dead {
                        self.revived_object_ids.remove(&object.object_id);
                    }
                } else if let Some((_, dead_state)) =
                    retained_zone_object_dead_state(&object.packet)
                {
                    self.dead_object_ids.insert(object.object_id, dead_state);
                }
                if retained_zone_object_is_drop(packet) {
                    object.expires_at_ms = Some(now_ms.saturating_add(ZONE_DROP_EXPIRE_MS));
                }
                self.object_grid.insert(object.object_id, &object.position);
                self.objects.insert(object.object_id, object);
                continue;
            }
            if let Some(object_id) = retained_zone_object_update_id(packet) {
                if self.removed_object_ids.contains(&object_id) {
                    continue;
                }
                if self.dead_object_ids.contains_key(&object_id)
                    && retained_zone_object_dead_stale_update(packet)
                {
                    continue;
                }
                if self.retained_zone_object_health_is_stale(object_id, packet) {
                    continue;
                }
                if let Some(object) = self.objects.get_mut(&object_id) {
                    apply_retained_zone_object_packet(object, packet);
                    track_zone_object_buff_expiry(object, packet, now_ms);
                }
            }
        }
    }

    fn remove_ground_drop_object_for_claim(&mut self, object_id: u32) -> Vec<ZoneOutbound> {
        self.objects.remove(&object_id);
        self.object_grid.remove(&object_id);
        self.dead_object_ids.remove(&object_id);
        self.revived_object_ids.remove(&object_id);
        self.harvested_object_ids.remove(&object_id);
        self.removed_object_ids.insert(object_id);
        let observers = self
            .players
            .iter_mut()
            .filter_map(|(session_id, player)| {
                player
                    .visible_object_ids
                    .remove(&object_id)
                    .then(|| session_id.clone())
            })
            .collect::<Vec<_>>();
        if observers.is_empty() {
            Vec::new()
        } else {
            vec![ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![ServerPacket::ObjectRemove { object_id }],
            }]
        }
    }

    fn expire_zone_objects(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let expired_buffs = self
            .objects
            .iter_mut()
            .flat_map(|(object_id, object)| {
                let buff_types = object
                    .buffs
                    .iter()
                    .filter_map(|(buff_type, state)| {
                        state
                            .expires_at_ms
                            .filter(|expires_at_ms| now_ms >= *expires_at_ms)
                            .map(|_| *buff_type)
                    })
                    .collect::<Vec<_>>();
                for buff_type in &buff_types {
                    object.buffs.remove(buff_type);
                    apply_retained_zone_object_packet(
                        object,
                        &ServerPacket::RemoveBuff {
                            object_id: *object_id,
                            buff_type: *buff_type,
                        },
                    );
                }
                buff_types
                    .into_iter()
                    .map(|buff_type| (*object_id, buff_type))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let expired_object_ids = self
            .objects
            .iter()
            .filter_map(|(object_id, object)| {
                object
                    .expires_at_ms
                    .filter(|expires_at_ms| now_ms >= *expires_at_ms)
                    .map(|_| *object_id)
            })
            .collect::<Vec<_>>();
        let mut outbounds = Vec::new();
        for (object_id, buff_type) in expired_buffs {
            let observers = self
                .players
                .iter()
                .filter_map(|(session_id, player)| {
                    player
                        .visible_object_ids
                        .contains(&object_id)
                        .then(|| session_id.clone())
                })
                .collect::<Vec<_>>();
            if !observers.is_empty() {
                outbounds.push(ZoneOutbound::ToMany {
                    session_ids: observers,
                    packets: vec![ServerPacket::RemoveBuff {
                        object_id,
                        buff_type,
                    }],
                });
            }
        }
        for object_id in expired_object_ids {
            self.objects.remove(&object_id);
            self.object_grid.remove(&object_id);
            self.native_monsters.remove(&object_id);
            self.ground_drops.remove(&object_id);
            self.claimed_ground_drops.remove(&object_id);
            self.dead_object_ids.remove(&object_id);
            self.revived_object_ids.remove(&object_id);
            self.harvested_object_ids.remove(&object_id);
            self.removed_object_ids.insert(object_id);
            let observers = self
                .players
                .iter_mut()
                .filter_map(|(session_id, player)| {
                    player
                        .visible_object_ids
                        .remove(&object_id)
                        .then(|| session_id.clone())
                })
                .collect::<Vec<_>>();
            if !observers.is_empty() {
                outbounds.push(ZoneOutbound::ToMany {
                    session_ids: observers,
                    packets: vec![ServerPacket::ObjectRemove { object_id }],
                });
            }
        }
        outbounds
    }

    fn expire_ground_drop_ownerships(&mut self, now_ms: u64) {
        for drop in self.ground_drops.values_mut() {
            if drop
                .owner_expires_at_ms
                .is_some_and(|expires_at_ms| now_ms >= expires_at_ms)
            {
                drop.owner_expires_at_ms = None;
                drop.drop.ownership_remaining_ticks = None;
            }
        }
    }

    fn remove_retained_zone_object(&mut self, object_id: u32) {
        self.objects.remove(&object_id);
        self.object_grid.remove(&object_id);
        self.native_monsters.remove(&object_id);
        self.pending_native_hits
            .retain(|hit| hit.object_id != object_id && hit.attacker_object_id != object_id);
        self.pending_native_player_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        self.dead_object_ids.remove(&object_id);
        self.revived_object_ids.remove(&object_id);
        self.harvested_object_ids.remove(&object_id);
        self.removed_object_ids.insert(object_id);
        for player in self.players.values_mut() {
            player.visible_object_ids.remove(&object_id);
        }
    }

    fn diff_zone_object_visibility_for(&mut self, session_id: &SessionId) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id).cloned() else {
            return Vec::new();
        };
        // Candidate object ids = AOI-grid neighbors (superset of everything in
        // range) UNION objects already visible to this player (so an object that
        // just left range still gets its ObjectRemove). Filter to live objects;
        // the exact points_visible test below preserves full-scan output.
        let mut candidate_object_ids: BTreeSet<u32> = self
            .object_grid
            .neighbors(&player.position)
            .into_iter()
            .collect();
        for object_id in &player.visible_object_ids {
            if self.objects.contains_key(object_id) {
                candidate_object_ids.insert(*object_id);
            }
        }
        let comparisons = candidate_object_ids
            .into_iter()
            .filter_map(|object_id| {
                self.objects.get(&object_id).map(|object| {
                    (
                        object.object_id,
                        points_visible(&player.position, &object.position),
                        player.visible_object_ids.contains(&object.object_id),
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut packets = Vec::new();
        for (object_id, visible_now, was_visible) in comparisons {
            match (visible_now, was_visible) {
                (true, false) => {
                    if let Some(player) = self.players.get_mut(session_id) {
                        player.visible_object_ids.insert(object_id);
                    }
                    if let Some(object) = self.objects.get(&object_id) {
                        packets.extend(retained_zone_object_visibility_packets(object));
                    }
                }
                (false, true) => {
                    if let Some(player) = self.players.get_mut(session_id) {
                        player.visible_object_ids.remove(&object_id);
                    }
                    packets.push(ServerPacket::ObjectRemove { object_id });
                }
                _ => {}
            }
        }
        if packets.is_empty() {
            Vec::new()
        } else {
            vec![ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets,
            }]
        }
    }

    fn apply_owner_action_transform(
        &mut self,
        session_id: &SessionId,
        owner_local_object_id: u32,
        packets: &[ServerPacket],
    ) -> (Vec<ZoneOutbound>, bool) {
        let Some((position, direction)) = packets
            .iter()
            .filter_map(|packet| owner_action_transform(owner_local_object_id, packet))
            .last()
        else {
            return (Vec::new(), false);
        };

        if !self.can_occupy(&position, Some(session_id)) {
            let Some(player) = self.players.get_mut(session_id) else {
                return (Vec::new(), true);
            };
            player.direction = direction;
            player.movement_actions.clear();
            let player = self
                .players
                .get(session_id)
                .expect("corrected player should still exist");
            return (
                vec![
                    ZoneOutbound::ToSession {
                        session_id: session_id.clone(),
                        packets: vec![user_location_packet(player)],
                    },
                    ZoneOutbound::SaveTransform {
                        session_id: session_id.clone(),
                        position: player.position.clone(),
                        direction: player.direction,
                    },
                ],
                true,
            );
        }

        {
            let player = self
                .players
                .get_mut(session_id)
                .expect("action transform owner should exist");
            self.occupancy.remove(&tile_key(&player.position));
            player.position = position;
            player.direction = direction;
            player.movement_actions.clear();
            self.occupancy
                .insert(tile_key(&player.position), session_id.clone());
            let moved_position = player.position.clone();
            self.player_grid.moved(session_id, &moved_position);
            self.ecs.move_player(session_id, &moved_position);
        }

        let mut outbounds = self.diff_visibility_for(session_id);
        outbounds.extend(self.diff_zone_object_visibility_for(session_id));
        let player = self
            .players
            .get(session_id)
            .expect("action transformed player should still exist");
        outbounds.push(ZoneOutbound::SaveTransform {
            session_id: session_id.clone(),
            position: player.position.clone(),
            direction: player.direction,
        });
        (outbounds, false)
    }

    fn expire_buffs(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let expired = self
            .players
            .iter_mut()
            .flat_map(|(session_id, player)| {
                let expired_buff_types = player
                    .buffs
                    .iter()
                    .filter_map(|(buff_type, state)| {
                        state
                            .expires_at_ms
                            .filter(|expires_at_ms| now_ms >= *expires_at_ms)
                            .map(|_| *buff_type)
                    })
                    .collect::<Vec<_>>();
                for buff_type in &expired_buff_types {
                    player.buffs.remove(buff_type);
                }
                expired_buff_types
                    .into_iter()
                    .map(|buff_type| (session_id.clone(), player.object_id, buff_type))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let expired_monster_buffs = self
            .native_monsters
            .iter_mut()
            .flat_map(|(object_id, monster)| {
                let expired_buff_types = monster
                    .buffs
                    .iter()
                    .filter_map(|(buff_type, state)| {
                        state
                            .expires_at_ms
                            .filter(|expires_at_ms| now_ms >= *expires_at_ms)
                            .map(|_| *buff_type)
                    })
                    .collect::<Vec<_>>();
                for buff_type in &expired_buff_types {
                    monster.buffs.remove(buff_type);
                }
                expired_buff_types
                    .into_iter()
                    .map(|buff_type| (*object_id, monster.position.clone(), buff_type))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut outbounds = Vec::new();
        for (session_id, object_id, buff_type) in expired {
            let observers = self
                .players
                .iter()
                .filter_map(|(other_session_id, other)| {
                    (other_session_id != &session_id
                        && other.visible_object_ids.contains(&object_id))
                    .then(|| other_session_id.clone())
                })
                .collect::<Vec<_>>();
            if observers.is_empty() {
                continue;
            }
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![ServerPacket::RemoveBuff {
                    buff_type,
                    object_id,
                }],
            });
        }
        for (object_id, position, buff_type) in expired_monster_buffs {
            let observers = self.native_monster_visible_recipients(object_id, &position);
            if observers.is_empty() {
                continue;
            }
            outbounds.push(ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![ServerPacket::RemoveBuff {
                    buff_type,
                    object_id,
                }],
            });
        }
        outbounds
    }

    fn whisper_chat(
        &self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
    ) -> Vec<ZoneOutbound> {
        let trimmed = rest.trim_start();
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let target_name = parts.next().unwrap_or_default().trim();
        let body = parts.next().unwrap_or_default();
        if target_name.is_empty() || body.trim().is_empty() {
            return Vec::new();
        }
        let Some(target) = self.player_by_name(target_name) else {
            return vec![ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: vec![chat_packet(
                    format!("Could not find {target_name}."),
                    ChatType::System,
                )],
            }];
        };
        if name_list_contains(&player.chat_profile.blocked_names, &target.name)
            || name_list_contains(&target.chat_profile.blocked_names, &player.name)
        {
            return vec![ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: vec![chat_packet(
                    format!("Could not find {target_name}."),
                    ChatType::System,
                )],
            }];
        }
        vec![
            ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: packets_with_linked_items(
                    linked_user_items,
                    vec![chat_packet(
                        format!("/{target_name} {body}"),
                        ChatType::WhisperOut,
                    )],
                ),
            },
            ZoneOutbound::ToSession {
                session_id: target.session_id.clone(),
                packets: packets_with_linked_items(
                    linked_user_items,
                    vec![chat_packet(
                        format!("{}=> {}", player.name, body),
                        ChatType::WhisperIn,
                    )],
                ),
            },
        ]
    }

    fn group_chat(
        &self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
    ) -> Vec<ZoneOutbound> {
        if player.chat_profile.group_members.is_empty() {
            return Vec::new();
        }
        let recipients = self.profile_named_recipients(
            player,
            player.chat_profile.group_members.iter().map(String::as_str),
            true,
        );
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: packets_with_linked_items(
                linked_user_items,
                vec![object_chat_packet_with_text(
                    player,
                    format!("{}: {}", player.name, rest.trim_start()),
                    ChatType::Group,
                )],
            ),
        }]
    }

    fn guild_chat(
        &self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
    ) -> Vec<ZoneOutbound> {
        let Some(guild_name) = player.chat_profile.guild_name.as_deref() else {
            return Vec::new();
        };
        let recipients = self
            .players
            .values()
            .filter(|candidate| {
                candidate
                    .chat_profile
                    .guild_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(guild_name))
            })
            .map(|candidate| candidate.session_id.clone())
            .collect::<Vec<_>>();
        if recipients.is_empty() {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: packets_with_linked_items(
                linked_user_items,
                vec![chat_packet(
                    format!("{}: {}", player.name, rest.trim_start()),
                    ChatType::Guild,
                )],
            ),
        }]
    }

    fn mentor_chat(
        &self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
    ) -> Vec<ZoneOutbound> {
        let Some(mentor_name) = player.chat_profile.mentor_name.as_deref() else {
            return Vec::new();
        };
        let Some(mentor) = self.player_by_name(mentor_name) else {
            return vec![ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: vec![chat_packet(
                    format!("{mentor_name} isn't online."),
                    ChatType::System,
                )],
            }];
        };
        vec![ZoneOutbound::ToMany {
            session_ids: vec![player.session_id.clone(), mentor.session_id.clone()],
            packets: packets_with_linked_items(
                linked_user_items,
                vec![chat_packet(
                    format!("{}: {}", player.name, rest.trim_start()),
                    ChatType::Mentor,
                )],
            ),
        }]
    }

    fn relationship_chat(
        &self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
    ) -> Vec<ZoneOutbound> {
        let Some(partner_name) = player.chat_profile.relationship_name.as_deref() else {
            return Vec::new();
        };
        let Some(partner) = self.player_by_name(partner_name) else {
            return vec![ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: vec![chat_packet(
                    format!("{partner_name} isn't online."),
                    ChatType::System,
                )],
            }];
        };
        vec![ZoneOutbound::ToMany {
            session_ids: vec![player.session_id.clone(), partner.session_id.clone()],
            packets: packets_with_linked_items(
                linked_user_items,
                vec![chat_packet(
                    format!("{}: {}", player.name, rest.trim_start()),
                    ChatType::Relationship,
                )],
            ),
        }]
    }

    fn shout_chat(
        &mut self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if now_ms < player.shout_ready_at_ms {
            let remaining_seconds = (player.shout_ready_at_ms - now_ms).div_ceil(1_000).max(1);
            return vec![ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: vec![chat_packet(
                    format!("You cannot shout for another {remaining_seconds} seconds."),
                    ChatType::System,
                )],
            }];
        }
        if player.level < 8
            && !player.chat_profile.free_map_shout
            && !player.chat_profile.free_server_shout
        {
            return vec![ZoneOutbound::ToSession {
                session_id: player.session_id.clone(),
                packets: vec![chat_packet(
                    "You need to be level 8 before you can shout.",
                    ChatType::System,
                )],
            }];
        }
        let used_map_shout = player.chat_profile.free_map_shout;
        let used_server_shout = player.chat_profile.free_server_shout;
        if let Some(current) = self.players.get_mut(&player.session_id) {
            current.shout_ready_at_ms = now_ms.saturating_add(SHOUT_COOLDOWN_MS);
            if used_map_shout {
                current.chat_profile.free_map_shout = false;
            }
            if used_server_shout {
                current.chat_profile.free_server_shout = false;
            }
        }
        let message = format!("(!){}:{}", player.name, rest);
        if player.chat_profile.free_server_shout {
            return vec![
                ZoneOutbound::ToAll {
                    packets: packets_with_linked_items(
                        linked_user_items,
                        vec![chat_packet(message, ChatType::Shout3)],
                    ),
                },
                ZoneOutbound::ConsumeShoutPermission {
                    session_id: player.session_id.clone(),
                    map_shout: false,
                    server_shout: true,
                },
            ];
        }
        if player.chat_profile.free_map_shout {
            return vec![
                ZoneOutbound::ToMany {
                    session_ids: self.players.keys().cloned().collect(),
                    packets: packets_with_linked_items(
                        linked_user_items,
                        vec![chat_packet(message, ChatType::Shout2)],
                    ),
                },
                ZoneOutbound::ConsumeShoutPermission {
                    session_id: player.session_id.clone(),
                    map_shout: true,
                    server_shout: false,
                },
            ];
        }
        let recipients = self
            .players
            .values()
            .filter(|candidate| players_visible_for_shout(player, candidate))
            .map(|candidate| candidate.session_id.clone())
            .collect::<Vec<_>>();
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: packets_with_linked_items(
                linked_user_items,
                vec![chat_packet(format!(" {message}"), ChatType::Shout)],
            ),
        }]
    }

    fn announcement_chat(
        &self,
        player: &ZonePlayer,
        rest: &str,
        linked_user_items: &[UserItem],
    ) -> Vec<ZoneOutbound> {
        if !player.chat_profile.is_gm {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: self.players.keys().cloned().collect(),
            packets: packets_with_linked_items(
                linked_user_items,
                vec![chat_packet(rest.trim_start(), ChatType::Announcement)],
            ),
        }]
    }

    fn player_by_name(&self, name: &str) -> Option<&ZonePlayer> {
        self.players
            .values()
            .find(|player| player.name.eq_ignore_ascii_case(name.trim()))
    }

    fn profile_named_recipients<'a>(
        &'a self,
        player: &ZonePlayer,
        names: impl Iterator<Item = &'a str>,
        include_self: bool,
    ) -> Vec<SessionId> {
        let mut recipients = Vec::new();
        if include_self {
            recipients.push(player.session_id.clone());
        }
        for name in names {
            if let Some(target) = self.player_by_name(name) {
                if !recipients.iter().any(|id| id == &target.session_id) {
                    recipients.push(target.session_id.clone());
                }
            }
        }
        recipients
    }

    fn diff_visibility_for(&mut self, session_id: &SessionId) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session_id).cloned() else {
            return Vec::new();
        };
        // Candidate set = players in the AOI grid neighborhood (a superset of
        // everyone newly/still visible) UNION the players already marked visible
        // (so someone who just left AOI range still gets an ObjectRemove). The
        // grid bounds this by local density; the exact players_visible test
        // below preserves identical output to a full scan.
        let mut candidate_sessions: BTreeSet<SessionId> = self
            .player_grid
            .neighbors(&player.position)
            .into_iter()
            .collect();
        for (other_session_id, other) in &self.players {
            if other_session_id != session_id
                && player.visible_object_ids.contains(&other.object_id)
            {
                candidate_sessions.insert(other_session_id.clone());
            }
        }
        candidate_sessions.remove(session_id);

        let comparisons = candidate_sessions
            .into_iter()
            .filter_map(|other_session_id| {
                self.players.get(&other_session_id).map(|other| {
                    (
                        other_session_id.clone(),
                        other.object_id,
                        players_visible(&player, other),
                        player.visible_object_ids.contains(&other.object_id),
                    )
                })
            })
            .collect::<Vec<_>>();

        let moved_player_packets = self
            .players
            .get(session_id)
            .map(object_player_packets)
            .expect("visibility player should still exist");
        let moved_object_id = player.object_id;
        let mut outbounds = Vec::new();

        for (other_session_id, other_object_id, visible_now, was_visible) in comparisons {
            match (visible_now, was_visible) {
                (true, false) => {
                    if let Some(player) = self.players.get_mut(session_id) {
                        player.visible_object_ids.insert(other_object_id);
                    }
                    if let Some(other) = self.players.get_mut(&other_session_id) {
                        other.visible_object_ids.insert(moved_object_id);
                    }
                    if let Some(other) = self.players.get(&other_session_id) {
                        outbounds.push(ZoneOutbound::ToSession {
                            session_id: session_id.clone(),
                            packets: object_player_packets(other),
                        });
                    }
                    outbounds.push(ZoneOutbound::ToSession {
                        session_id: other_session_id,
                        packets: moved_player_packets.clone(),
                    });
                }
                (false, true) => {
                    if let Some(player) = self.players.get_mut(session_id) {
                        player.visible_object_ids.remove(&other_object_id);
                    }
                    if let Some(other) = self.players.get_mut(&other_session_id) {
                        other.visible_object_ids.remove(&moved_object_id);
                    }
                    outbounds.push(ZoneOutbound::ToSession {
                        session_id: session_id.clone(),
                        packets: vec![ServerPacket::ObjectRemove {
                            object_id: other_object_id,
                        }],
                    });
                    outbounds.push(ZoneOutbound::ToSession {
                        session_id: other_session_id,
                        packets: vec![ServerPacket::ObjectRemove {
                            object_id: moved_object_id,
                        }],
                    });
                }
                _ => {}
            }
        }

        outbounds
    }

    fn unique_object_id(&mut self, requested: u32) -> u32 {
        let collides = requested == 0 || self.object_id_in_use(requested);
        if !collides {
            self.next_object_id = self.next_object_id.max(requested.saturating_add(1));
            return requested;
        }

        while self.object_id_in_use(self.next_object_id) {
            self.next_object_id = self.next_object_id.saturating_add(1);
        }
        let object_id = self.next_object_id;
        self.next_object_id = self.next_object_id.saturating_add(1);
        object_id
    }

    fn object_id_in_use(&self, object_id: u32) -> bool {
        self.players
            .values()
            .any(|player| player.object_id == object_id)
            || self.objects.contains_key(&object_id)
            || self.native_monsters.contains_key(&object_id)
            || self.ground_drops.contains_key(&object_id)
            || self.claimed_ground_drops.contains_key(&object_id)
    }

    fn first_available_position(
        &self,
        requested: Point,
        ignore_session: Option<&SessionId>,
    ) -> Point {
        if self.can_occupy(&requested, ignore_session) {
            return requested;
        }
        for radius in 1_i32..=8 {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let candidate = Point {
                        x: requested.x.saturating_add(dx),
                        y: requested.y.saturating_add(dy),
                    };
                    if self.can_occupy(&candidate, ignore_session) {
                        return candidate;
                    }
                }
            }
        }
        requested
    }

    fn can_occupy(&self, point: &Point, ignore_session: Option<&SessionId>) -> bool {
        if self.collision.is_blocked(point) {
            return false;
        }
        if self
            .objects
            .values()
            .any(|object| retained_zone_object_blocks_tile(object, point))
        {
            return false;
        }
        self.occupancy
            .get(&tile_key(point))
            .map(|owner| Some(owner) == ignore_session)
            .unwrap_or(true)
    }

    fn can_player_movement_occupy(
        &self,
        point: &Point,
        ignore_session: Option<&SessionId>,
    ) -> bool {
        if self.collision.is_player_movement_blocked(point) {
            return false;
        }
        if self
            .objects
            .values()
            .any(|object| retained_zone_object_blocks_tile(object, point))
        {
            return false;
        }
        self.occupancy
            .get(&tile_key(point))
            .map(|owner| Some(owner) == ignore_session)
            .unwrap_or(true)
    }

    fn object_id_belongs_to_player(&self, object_id: u32) -> bool {
        self.players
            .values()
            .any(|player| player.object_id == object_id)
    }

    fn harvested_dead_state(&self, object_id: &u32) -> Option<&ZoneObjectDeadState> {
        self.harvested_object_ids
            .contains(object_id)
            .then(|| self.dead_object_ids.get(object_id))
            .flatten()
    }

    fn retained_zone_object_health_is_stale(&self, object_id: u32, packet: &ServerPacket) -> bool {
        let Some(new_percent) = retained_zone_object_health_percent(packet) else {
            return false;
        };
        if new_percent == 0 {
            return false;
        }
        self.objects
            .get(&object_id)
            .and_then(|object| object.health.as_ref())
            .is_some_and(|current| new_percent > current.percent)
    }

    fn ground_drop_ownership_allows(
        &self,
        drop: &GroundDropSnapshot,
        picker_object_id: u32,
        group_members: &[String],
    ) -> bool {
        if !drop
            .ownership_remaining_ticks
            .is_some_and(|remaining_ticks| remaining_ticks > 0)
        {
            return true;
        }
        let Some(owner_object_id) = drop.owner_object_id else {
            return true;
        };
        if owner_object_id == picker_object_id {
            return true;
        }
        self.players
            .values()
            .find(|player| player.object_id == owner_object_id)
            .is_some_and(|owner| {
                group_members
                    .iter()
                    .any(|member| member.eq_ignore_ascii_case(&owner.name))
            })
    }
}

fn tile_key(point: &Point) -> (i32, i32) {
    (point.x, point.y)
}

fn zone_direction_toward(source: &Point, target: &Point) -> Option<MirDirection> {
    let dx = (target.x - source.x).signum();
    let dy = (target.y - source.y).signum();

    match (dx, dy) {
        (0, 0) => None,
        (0, -1) => Some(MirDirection::Up),
        (1, -1) => Some(MirDirection::UpRight),
        (1, 0) => Some(MirDirection::Right),
        (1, 1) => Some(MirDirection::DownRight),
        (0, 1) => Some(MirDirection::Down),
        (-1, 1) => Some(MirDirection::DownLeft),
        (-1, 0) => Some(MirDirection::Left),
        (-1, -1) => Some(MirDirection::UpLeft),
        _ => None,
    }
}

fn native_monster_adjacent_to(source: &Point, target: &Point) -> bool {
    let dx = (target.x - source.x).abs();
    let dy = (target.y - source.y).abs();
    (dx != 0 || dy != 0) && dx <= 1 && dy <= 1
}

fn zone_tile_distance(source: &Point, target: &Point) -> i32 {
    (target.x - source.x).abs().max((target.y - source.y).abs())
}

fn points_within_action_range(source: &Point, target: &Point, max_range: i32) -> bool {
    zone_tile_distance(source, target) <= max_range
}

fn zone_native_monster_prefers_ranged(monster: &ZoneNativeMonster, target: &Point) -> bool {
    let distance = zone_tile_distance(&monster.position, target);
    if monster.ai == 61 {
        return distance > 0 && distance <= 12;
    }
    if distance <= 1 {
        return false;
    }
    match monster.ai {
        36 | 50 | 123 | 131 => distance > 2,
        16 | 19 | 20 | 27 | 30 | 31 | 32 | 33 | 34 | 51 | 86 | 102 | 118 | 120 | 121 | 122
        | 130 | 181 | 182 | 189 | 190 => true,
        _ => matches!(
            monster.ai,
            8 | 26 | 38 | 45 | 46 | 47 | 48 | 54 | 57 | 61 | 113
        ),
    }
}

fn zone_native_monster_range_attack_type(ai: u8, source: &Point, target: &Point) -> u8 {
    match ai {
        131 if zone_tile_distance(source, target) > 2 => 1,
        _ => 0,
    }
}

/// Returns the monster's authoritative attack damage and whether it is a magic
/// attack (so the zone can mitigate with the target's MAC instead of AC).
fn zone_native_monster_player_attack_damage(
    monster: &ZoneNativeMonster,
    target: &Point,
) -> (i32, bool) {
    let distance = zone_tile_distance(&monster.position, target);
    match monster.ai {
        19 if distance > 1 => (zone_crystal_monster_magic_damage(&monster.name), true),
        20 if distance > 1 => (zone_crystal_monster_attack_damage(&monster.name) * 3, false),
        43 if distance > 2 => (zone_crystal_monster_magic_damage(&monster.name), true),
        120 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        121 if distance > 1 => (zone_crystal_monster_magic_damage(&monster.name), true),
        122 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        123 if distance > 2 => (zone_crystal_monster_magic_damage(&monster.name), true),
        102 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        86 if distance > 2 => (zone_crystal_monster_magic_damage(&monster.name), true),
        88 => (zone_crystal_monster_magic_damage(&monster.name), true),
        126 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        127 if distance > 1 => (zone_crystal_monster_magic_damage(&monster.name), true),
        188 if distance > 1 => (
            zone_crystal_monster_raw_attack_damage(&monster.name) * 2,
            false,
        ),
        189 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        192 if distance > 1 => (
            zone_crystal_monster_raw_attack_damage(&monster.name) * 3,
            false,
        ),
        130 if distance > 1 => (zone_crystal_monster_magic_damage(&monster.name), true),
        131 if distance > 2 => (zone_crystal_monster_spell_damage(&monster.name), true),
        118 | 181 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        182 if distance > 1 => (zone_crystal_monster_raw_magic_damage(&monster.name), true),
        _ => (zone_crystal_monster_attack_damage(&monster.name), false),
    }
}

fn zone_native_summon_attack_range(monster: &ZoneNativeMonster) -> i32 {
    match monster.ai {
        38 => 6,
        61 => 12,
        62 | 255 => 0,
        _ => 1,
    }
}

fn zone_native_summon_hit_delay_ms(monster: &ZoneNativeMonster, target: &Point) -> u64 {
    match monster.ai {
        38 if zone_tile_distance(&monster.position, target) > 1 => 500,
        61 if zone_tile_distance(&monster.position, target) > 1 => {
            u64::try_from(zone_tile_distance(&monster.position, target).max(0))
                .unwrap_or_default()
                .saturating_mul(50)
                .saturating_add(500)
        }
        _ => ZONE_NATIVE_MONSTER_THINK_MS,
    }
}

fn zone_native_summon_monster_attack_damage(monster: &ZoneNativeMonster, target: &Point) -> i32 {
    let dc_bonus = zone_native_monster_buff_stat_total(monster, CRYSTAL_STAT_MAX_DC).max(0);
    match monster.ai {
        38 | 60 | 61 | 63 if zone_tile_distance(&monster.position, target) > 0 => {
            zone_crystal_monster_attack_damage(&monster.name).saturating_add(dc_bonus)
        }
        _ => 1_i32.saturating_add(dc_bonus),
    }
}

fn zone_native_summon_owner_player_object_id(monster: &ZoneNativeMonster) -> u32 {
    if monster.owner_player_object_id != 0 {
        monster.owner_player_object_id
    } else {
        monster.master_object_id
    }
}

fn zone_native_summon_can_move(monster: &ZoneNativeMonster) -> bool {
    !matches!(monster.ai, 61 | 62 | 255)
}

fn zone_native_monster_buff_stat_total(monster: &ZoneNativeMonster, stat: u8) -> i32 {
    monster
        .buffs
        .values()
        .filter(|state| !state.buff.paused)
        .flat_map(|state| state.buff.stats.iter())
        .filter(|item_stat| item_stat.stat == stat)
        .map(|item_stat| item_stat.value)
        .sum()
}

fn zone_crystal_monster_attack_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_dc.max(monster.min_dc).max(1))
        .unwrap_or(7)
}

fn zone_crystal_monster_magic_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_mc.max(monster.min_mc).max(1))
        .unwrap_or(7)
}

fn zone_crystal_monster_spell_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_sc.max(monster.min_sc).max(1))
        .unwrap_or(7)
}

fn zone_crystal_monster_raw_magic_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_mc.max(monster.min_mc))
        .unwrap_or(0)
}

fn zone_crystal_monster_raw_attack_damage(name: &str) -> i32 {
    crystal_monster_by_name(name)
        .map(|monster| monster.max_dc.max(monster.min_dc))
        .unwrap_or(0)
}

fn zone_mana_percent(mp: i32) -> u8 {
    ((mp.max(0) * 100) / 100).clamp(0, 100) as u8
}

fn zone_player_native_damage(player: &ZonePlayer, base_damage: i32) -> i32 {
    if base_damage <= 0 {
        return 0;
    }
    base_damage
        .saturating_add(zone_player_buff_stat_total(player, CRYSTAL_STAT_MAX_DC))
        .max(1)
}

/// Deterministically roll an inclusive `[min, max]` stat range using the shared
/// zone RNG so every observer of the zone derives the same value.
fn zone_roll_stat_range(min: i32, max: i32, tick: u64, actor_id: u32, salt: u64) -> i32 {
    let lo = min.min(max).max(0);
    let hi = max.max(min).max(0);
    if hi <= lo {
        return lo;
    }
    let span = u64::try_from(hi - lo + 1).unwrap_or(1);
    let roll = zone_deterministic_roll(
        tick,
        usize::try_from(actor_id).unwrap_or_default(),
        usize::try_from(salt).unwrap_or_default(),
        span,
    );
    lo.saturating_add(i32::try_from(roll).unwrap_or_default())
}

/// Crystal `MapObject.GetAttackPower(min, max)` — a Luck-biased roll used for the
/// physical attack-power channel (not for `GetDefencePower`/armour). Positive
/// Luck can force the `max` end, negative Luck the `min` end. With `luck == 0`
/// this is identical to [`zone_roll_stat_range`] (same tick/actor/salt), so a
/// player with no Luck gear rolls exactly as before. Mirrors the per-session
/// `crystal_attack_power_roll`.
fn zone_roll_attack_power(
    min: i32,
    max: i32,
    luck: i32,
    tick: u64,
    actor_id: u32,
    salt: u64,
) -> i32 {
    let lo = min.min(max).max(0);
    let hi = max.max(min).max(0);
    const MAX_LUCK: u64 = 10; // Crystal Settings.MaxLuck default.
    let actor = usize::try_from(actor_id).unwrap_or_default();
    if luck > 0 {
        let roll = zone_deterministic_roll(
            tick,
            actor,
            usize::try_from(salt).unwrap_or_default(),
            MAX_LUCK,
        ) as i32;
        if luck > roll {
            return hi;
        }
    } else if luck < 0 {
        let roll = zone_deterministic_roll(
            tick,
            actor,
            usize::try_from(salt.wrapping_add(0xABCD)).unwrap_or_default(),
            MAX_LUCK,
        ) as i32;
        if luck < -roll {
            return lo;
        }
    }
    zone_roll_stat_range(min, max, tick, actor_id, salt)
}

/// Authoritatively resolve a player's physical (melee/range) attack against a
/// shared-zone monster.
///
/// Historically the attacker's *personal* session pre-rolled the damage and the
/// zone trusted that scalar — there was no hit/miss check and no armour applied
/// inside the zone, so the shared world was not the combat authority. This rolls
/// `Random(MinDC..=MaxDC)` (+ buff DC), runs the Crystal accuracy-vs-agility hit
/// check, and subtracts `Random(MinAC..=MaxAC)` using the monster's own
/// authoritative defensive stats.
///
/// Returns `None` on a miss (only the already-broadcast swing animation is
/// shown), or `Some(damage)` on a hit. `damage` may be `0` when armour fully
/// absorbs the blow (a Crystal "block").
///
/// When the attacker has no authoritative stat block yet
/// (`combat_stats.has_authoritative_damage() == false`) this falls back to the
/// legacy trusted scalar so existing callers keep working unchanged.
fn zone_resolve_player_physical_attack(
    player: &ZonePlayer,
    monster: &ZoneNativeMonster,
    monster_object_id: u32,
    fallback_damage: i32,
    now_ms: u64,
) -> Option<i32> {
    let stats = player.combat_stats;
    if !stats.has_authoritative_damage() {
        return Some(zone_player_native_damage(player, fallback_damage));
    }

    let agility = monster.defense.agility.max(0);
    if agility > 0 {
        let roll = zone_deterministic_roll(
            now_ms,
            usize::try_from(player.object_id).unwrap_or_default(),
            usize::try_from(monster_object_id).unwrap_or_default(),
            u64::try_from(agility.saturating_add(1)).unwrap_or(1),
        );
        if roll > u64::try_from(stats.accuracy.max(0)).unwrap_or(0) {
            return None;
        }
    }

    let base = zone_roll_attack_power(
        stats.min_dc,
        stats.max_dc,
        stats.luck,
        now_ms,
        player.object_id,
        0x5DC,
    )
    .saturating_add(zone_player_buff_stat_total(player, CRYSTAL_STAT_MAX_DC));
    let base = zone_apply_player_critical(base, &stats, player.object_id, now_ms);
    let armour = zone_roll_stat_range(
        monster.defense.min_ac,
        monster.defense.max_ac,
        now_ms,
        player.object_id,
        0x0AC,
    );
    Some(base.saturating_sub(armour).max(0))
}

/// Crystal critical hit (HumanObject.cs:7156-7161, MonsterObject.cs:2594-2599):
/// the blow crits when `Random(100) < CriticalRate * Settings.CriticalRateWeight`
/// (weight 5) and is then amplified by
/// `Floor(damage * (CriticalDamage / Settings.CriticalDamageWeight) * 10)`
/// (weight 50 → +20% per `CriticalDamage` point). Resolved deterministically off
/// the tick (mirrors the per-session `crystal_apply_player_critical`). Inert when
/// the player has no crit gear.
fn zone_apply_player_critical(
    damage: i32,
    stats: &super::types::ZonePlayerCombatStats,
    object_id: u32,
    now_ms: u64,
) -> i32 {
    let rate = stats.critical_rate.clamp(0, 100);
    if rate <= 0 || damage <= 0 {
        return damage;
    }
    let roll = zone_deterministic_roll(
        now_ms,
        usize::try_from(object_id).unwrap_or_default(),
        0xC817,
        100,
    );
    let crit_chance =
        rate.saturating_mul(crate::runtime::crystal_compat::CRYSTAL_CRITICAL_RATE_WEIGHT);
    if roll >= u64::try_from(crit_chance).unwrap_or(0) {
        return damage;
    }
    crate::runtime::combat::crystal_critical_amplified_damage(damage, stats.critical_damage)
}

/// Subtract a target monster's authoritative magic armour `Random(MinMAC,MaxMAC)`
/// from an attack spell's damage, mirroring how `zone_resolve_player_physical_attack`
/// subtracts the monster's `Random(MinAC,MaxAC)` for melee/range. Crystal attack
/// spells mitigate via the target's MAC, so the zone — not the gateway — owns the
/// reduction. Floors at 0 (matching the physical path), and is a no-op for the many
/// monsters whose template MAC is 0.
fn zone_magic_damage_after_monster_armour(
    monster: &ZoneNativeMonster,
    damage: i32,
    attacker_object_id: u32,
    now_ms: u64,
) -> i32 {
    if damage <= 0 {
        return damage;
    }
    let armour = zone_roll_stat_range(
        monster.defense.min_mac,
        monster.defense.max_mac,
        now_ms,
        attacker_object_id,
        0x2AC,
    );
    damage.saturating_sub(armour).max(0)
}

/// Authoritatively recompute a spell's damage from the Crystal magic template,
/// so the zone — not the attacker's session — owns the magic damage number.
///
/// This mirrors the session's `zone_magic_attack_profile` formula exactly
/// (`crystal_magic_damage_from_base` over the player's melee base, with the two
/// pure-control spells dealing 0), so for every spell the recomputed value
/// matches what the gateway previously supplied. Returns `None` for spells with
/// no magic template so the caller falls back to the supplied value.
fn zone_authoritative_magic_damage(spell: Spell, level: u8, base: i32) -> Option<i32> {
    if matches!(spell, Spell::ElectricShock | Spell::Entrapment) {
        return Some(0);
    }
    let template = crystal_magic_by_spell(&format!("{spell:?}"))?;
    Some(
        crate::runtime::combat::crystal_magic_damage_from_base(&template, level, base.max(1))
            .max(1),
    )
}

/// Spells whose final damage the gateway adjusts with inventory-dependent item
/// bonuses (currently only PoisonCloud's amulet bonus), so the zone keeps the
/// supplied value for them rather than recomputing the base formula.
fn zone_magic_keeps_supplied_damage(spell: Spell) -> bool {
    matches!(spell, Spell::PoisonCloud)
}

fn zone_magic_hit_damage(spell: Spell, secondary: bool, damage: i32) -> i32 {
    if secondary && spell == Spell::MeteorShower {
        return damage.saturating_div(2).max(1);
    }
    damage
}

fn zone_player_native_incoming_damage(
    player: &ZonePlayer,
    base_damage: i32,
    magic: bool,
    now_ms: u64,
) -> i32 {
    let buff_ac = zone_player_buff_stat_total(player, CRYSTAL_STAT_MAX_AC).max(0);
    // Subtract the player's authoritative base armour, rolled from their stat
    // block: MAC for magical hits, AC for physical hits. With no stat block the
    // range is 0..0 and only buff AC + the reduction buff apply (legacy).
    let (min_armour, max_armour) = if magic {
        (player.combat_stats.min_mac, player.combat_stats.max_mac)
    } else {
        (player.combat_stats.min_ac, player.combat_stats.max_ac)
    };
    let base_armour = zone_roll_stat_range(
        min_armour,
        max_armour,
        now_ms,
        player.object_id,
        if magic { 0x3AC } else { 0x4AC },
    );
    let mitigated = base_damage
        .max(0)
        .saturating_sub(buff_ac)
        .saturating_sub(base_armour);
    let reduction_percent =
        zone_player_buff_stat_total(player, CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT).clamp(0, 100);
    mitigated
        .saturating_mul(100_i32.saturating_sub(reduction_percent))
        .saturating_div(100)
}

fn zone_player_buff_stat_total(player: &ZonePlayer, stat: u8) -> i32 {
    player
        .buffs
        .values()
        .filter(|state| !state.buff.paused)
        .flat_map(|state| state.buff.stats.iter())
        .filter(|item_stat| item_stat.stat == stat)
        .map(|item_stat| item_stat.value)
        .sum()
}

fn native_monster_current_poison(monster: &ZoneNativeMonster) -> u16 {
    monster.control_poison | monster.damage_poison
}

fn zone_poison_shot_green_damage(damage: i32, level: u8) -> i32 {
    damage
        .max(0)
        .saturating_div(25)
        .saturating_add(i32::from(level) + 1)
        .max(1)
}

fn zone_poison_shot_green_duration_ms(damage: i32, level: u8) -> u64 {
    let duration_ticks = damage
        .max(0)
        .saturating_mul(2)
        .saturating_add((i32::from(level) + 1).saturating_mul(7))
        .max(1);
    u64::try_from(duration_ticks)
        .unwrap_or_default()
        .saturating_mul(ZONE_CRYSTAL_GREEN_POISON_TICK_MS)
}

fn zone_vampire_shot_heal_amount(damage: i32, level: u8) -> i32 {
    damage
        .max(0)
        .saturating_mul(i32::from(level).saturating_add(1))
        .saturating_div(4)
}

fn zone_summoned_pet_explosion_damage(level: u8) -> i32 {
    i32::from(level.max(1)).saturating_mul(10)
}

#[derive(Debug, Clone, Copy)]
struct ZoneNativeMagicControl {
    duration_ms: u64,
    poison: Option<u16>,
    effect: Option<u8>,
    blocks_ai: bool,
}

fn zone_native_magic_control(
    spell: Spell,
    level: u8,
    object_id: u32,
    now_ms: u64,
) -> Option<ZoneNativeMagicControl> {
    match spell {
        Spell::ElectricShock => Some(ZoneNativeMagicControl {
            duration_ms: (u64::from(level).saturating_mul(5) + 10).saturating_mul(1_000),
            poison: None,
            effect: None,
            blocks_ai: true,
        }),
        Spell::Entrapment => Some(ZoneNativeMagicControl {
            duration_ms: u64::from(level).saturating_add(1).saturating_mul(1_000),
            poison: Some(CRYSTAL_POISON_PARALYSIS),
            effect: Some(CRYSTAL_SPELL_EFFECT_ENTRAPMENT),
            blocks_ai: true,
        }),
        Spell::CatTongue => {
            let poison = zone_cat_tongue_poison(object_id, now_ms, level);
            Some(ZoneNativeMagicControl {
                duration_ms: u64::from(level).saturating_add(1).saturating_mul(3_000),
                poison: Some(poison),
                effect: None,
                blocks_ai: poison != CRYSTAL_POISON_SLOW,
            })
        }
        _ => None,
    }
}

fn zone_cat_tongue_poison(object_id: u32, now_ms: u64, level: u8) -> u16 {
    let roll = zone_deterministic_roll(
        now_ms,
        usize::try_from(object_id).unwrap_or_default(),
        1_117 + usize::from(level),
        10,
    );
    match roll {
        0..=4 => CRYSTAL_POISON_STUN,
        5..=7 => CRYSTAL_POISON_SLOW,
        _ => CRYSTAL_POISON_FROZEN,
    }
}

fn zone_native_monster_player_status(
    ai: u8,
    attacker_object_id: u32,
    now_ms: u64,
) -> Option<ZoneNativeMonsterPlayerStatus> {
    let current_tick = now_ms.div_ceil(ZONE_CRYSTAL_STATUS_TICK_MS);
    let (poison, chance_denominator, duration_ticks, salt): (u16, u64, u64, u64) = match ai {
        7 => (CRYSTAL_POISON_PARALYSIS, 20, 5, 7),
        22 => (CRYSTAL_POISON_PARALYSIS, 12, 5, 7),
        28 | 37 => (CRYSTAL_POISON_GREEN, 8, 5, 28),
        _ => return None,
    };
    if !zone_deterministic_chance_roll(current_tick, attacker_object_id, salt, chance_denominator) {
        return None;
    }
    Some(ZoneNativeMonsterPlayerStatus {
        poison,
        duration_ms: duration_ticks.saturating_mul(ZONE_CRYSTAL_STATUS_TICK_MS),
    })
}

fn zone_deterministic_chance_roll(
    current_tick: u64,
    attacker_id: u32,
    salt: u64,
    denominator: u64,
) -> bool {
    if denominator <= 1 {
        return true;
    }

    let value = current_tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(attacker_id).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    value % denominator == 0
}

fn zone_deterministic_roll(
    current_tick: u64,
    rule_index: usize,
    slot_index: usize,
    modulo: u64,
) -> u64 {
    if modulo == 0 {
        return 0;
    }

    current_tick
        .wrapping_mul(1_103_515_245)
        .wrapping_add((rule_index as u64).wrapping_mul(97_651))
        .wrapping_add((slot_index as u64).wrapping_mul(12_347))
        .wrapping_add(0x9E37_79B9)
        % modulo
}

fn zone_player_status_blocks_movement(player: &ZonePlayer, now_ms: u64) -> bool {
    let status_poison = player.native_status_poison;
    status_poison & (CRYSTAL_POISON_PARALYSIS | CRYSTAL_POISON_STUN | CRYSTAL_POISON_FROZEN) != 0
        && player
            .native_status_poison_expires_at_ms
            .is_some_and(|expires_at_ms| now_ms < expires_at_ms)
}

fn zone_player_slowed(player: &ZonePlayer, now_ms: u64) -> bool {
    player.native_status_poison & CRYSTAL_POISON_SLOW != 0
        && player
            .native_status_poison_expires_at_ms
            .is_some_and(|expires_at_ms| now_ms < expires_at_ms)
}

/// Per-step movement delay for a zone player, accounting for being mounted
/// (faster) and the Slow poison (slower). Mirrors Crystal, where mounts speed
/// movement up and the Slow debuff roughly halves action speed.
fn zone_player_move_delay_ms(player: &ZonePlayer, running: bool, now_ms: u64) -> u64 {
    let mut delay = movement_delay_ms(running);
    if player.riding_mount && player.mount_type >= 0 {
        delay = delay.saturating_mul(2) / 3;
    }
    if zone_player_slowed(player, now_ms) {
        delay = delay.saturating_mul(2);
    }
    delay.max(1)
}

fn native_monster_control_active(monster: &ZoneNativeMonster, now_ms: u64) -> bool {
    monster.control_until_ms != 0 && now_ms < monster.control_until_ms
}

fn zone_magic_uses_projectile(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::FireBall
            | Spell::GreatFireBall
            | Spell::ThunderBolt
            | Spell::SoulFireBall
            | Spell::FireBounce
            | Spell::CatTongue
    )
}

fn zone_magic_uses_ground_spell(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::FireWall
            | Spell::Blizzard
            | Spell::MeteorStrike
            | Spell::PoisonCloud
            | Spell::Trap
            | Spell::TrapHexagon
            | Spell::ExplosiveTrap
    )
}

fn zone_magic_targets_ground_point(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::FireWall
            | Spell::Blizzard
            | Spell::MeteorStrike
            | Spell::PoisonCloud
            | Spell::ExplosiveTrap
    )
}

fn zone_magic_targets_summon(spell: Spell) -> bool {
    native_summon_spell_profile(spell).is_some()
}

fn zone_magic_targets_pet_buff(spell: Spell) -> bool {
    matches!(spell, Spell::PetEnhancer)
}

fn native_summon_spell_profile(spell: Spell) -> Option<NativeSummonSpellProfile> {
    let taoist = |monster_name, base_delay_ms| NativeSummonSpellProfile {
        monster_name,
        base_delay_ms,
        max_summons: 2,
        limit_scope: NativeSummonLimitScope::SameMonster,
        spawn_at_target: false,
        projectile_delay: false,
        recall_mode: NativeSummonRecallMode::Player,
        expires_base_ms: None,
        expires_per_level_ms: 0,
    };
    let archer = |monster_name, expires_base_ms, expires_per_level_ms| NativeSummonSpellProfile {
        monster_name,
        base_delay_ms: 500,
        max_summons: 2,
        limit_scope: NativeSummonLimitScope::AllOwned,
        spawn_at_target: true,
        projectile_delay: true,
        recall_mode: NativeSummonRecallMode::Target,
        expires_base_ms: Some(expires_base_ms),
        expires_per_level_ms,
    };
    match spell {
        Spell::SummonSkeleton => Some(taoist("BoneFamiliar", 500)),
        Spell::SummonShinsu => Some(taoist("Shinsu", 500)),
        Spell::SummonHolyDeva => Some(taoist("HolyDeva", 1_500)),
        Spell::SummonVampire => Some(archer("VampireSpider", 15_000, 1_500)),
        Spell::SummonToad => Some(archer("SpittingToad", 25_000, 2_000)),
        Spell::SummonSnakes => Some(archer("SnakeTotem", 20_000, 1_500)),
        Spell::Stonetrap => Some(NativeSummonSpellProfile {
            monster_name: "StoneTrap",
            base_delay_ms: 500,
            max_summons: 1,
            limit_scope: NativeSummonLimitScope::SameMonster,
            spawn_at_target: true,
            projectile_delay: true,
            recall_mode: NativeSummonRecallMode::None,
            expires_base_ms: Some(10_000),
            expires_per_level_ms: 5_000,
        }),
        _ => None,
    }
}

fn native_summon_cast_delay_ms(
    profile: NativeSummonSpellProfile,
    player_position: &Point,
    target: &Point,
) -> u64 {
    if profile.projectile_delay {
        u64::try_from(zone_tile_distance(player_position, target).max(0))
            .unwrap_or_default()
            .saturating_mul(50)
            .saturating_add(profile.base_delay_ms)
            .saturating_add(500)
    } else {
        profile.base_delay_ms
    }
}

fn native_summon_expires_at_ms(
    profile: NativeSummonSpellProfile,
    level: u8,
    spawned_at_ms: u64,
) -> Option<u64> {
    profile.expires_base_ms.map(|base_ms| {
        spawned_at_ms.saturating_add(
            base_ms.saturating_add(u64::from(level).saturating_mul(profile.expires_per_level_ms)),
        )
    })
}

fn zone_summon_target_point_allowed(player_position: &Point, target: &Point) -> bool {
    zone_tile_distance(player_position, target) <= 1
}

fn zone_magic_targets_self(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::Healing | Spell::MassHealing | Spell::HealingCircle | Spell::MagicShield
    )
}

fn zone_self_magic_target_point_allowed(
    spell: Spell,
    player_position: &Point,
    target: &Point,
) -> bool {
    match spell {
        Spell::MassHealing | Spell::HealingCircle => {
            zone_tile_distance(player_position, target) <= 1
        }
        _ => player_position == target,
    }
}

fn zone_ground_spell_timing(spell: Spell, due_ms: u64, damage: i32) -> (u64, u64, u64) {
    match spell {
        Spell::FireWall => (
            due_ms,
            due_ms.saturating_add(
                10_000_u64.saturating_add(
                    u64::try_from(damage.max(0) / 2)
                        .unwrap_or_default()
                        .saturating_mul(1_000),
                ),
            ),
            2_000,
        ),
        Spell::Blizzard | Spell::MeteorStrike => (
            due_ms.saturating_add(800),
            due_ms.saturating_add(3_000),
            440,
        ),
        Spell::PoisonCloud => (due_ms, due_ms.saturating_add(6_000), 1_000),
        Spell::Trap => (due_ms, due_ms.saturating_add(1), 1),
        Spell::TrapHexagon => (due_ms, due_ms.saturating_add(1), 1),
        Spell::ExplosiveTrap => (due_ms, due_ms.saturating_add(10_000), 500),
        _ => (due_ms, due_ms, 1),
    }
}

fn native_ground_spell_packets(action: &PendingNativeGroundSpellAction) -> Vec<ServerPacket> {
    match action.spell {
        Spell::FireWall | Spell::Blizzard | Spell::MeteorStrike => {
            action
                .locations
                .iter()
                .enumerate()
                .map(|(index, location)| ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: action.spell_object_ids.get(index).copied().unwrap_or_else(
                            || {
                                action
                                    .caster_object_id
                                    .saturating_add(80_000)
                                    .saturating_add(u32::try_from(index).unwrap_or_default())
                            },
                        ),
                        location: location.clone(),
                        spell: action.spell,
                        direction: MirDirection::Up,
                        param: matches!(action.spell, Spell::Blizzard | Spell::MeteorStrike)
                            && index == action.locations.len() / 2,
                    },
                })
                .collect()
        }
        Spell::PoisonCloud => {
            let location = action
                .locations
                .get(action.locations.len() / 2)
                .cloned()
                .unwrap_or(Point { x: 0, y: 0 });
            vec![ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: action
                        .spell_object_ids
                        .first()
                        .copied()
                        .unwrap_or_else(|| action.caster_object_id.saturating_add(80_000)),
                    location,
                    spell: Spell::PoisonCloud,
                    direction: MirDirection::Up,
                    param: false,
                },
            }]
        }
        Spell::Trap => {
            let location = action
                .locations
                .first()
                .cloned()
                .unwrap_or(Point { x: 0, y: 0 });
            vec![ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: action
                        .spell_object_ids
                        .first()
                        .copied()
                        .unwrap_or_else(|| action.caster_object_id.saturating_add(80_000)),
                    location,
                    spell: Spell::Trap,
                    direction: action.direction,
                    param: action.spell_param,
                },
            }]
        }
        Spell::TrapHexagon => {
            action
                .locations
                .iter()
                .enumerate()
                .map(|(index, location)| ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: action.spell_object_ids.get(index).copied().unwrap_or_else(
                            || {
                                action
                                    .caster_object_id
                                    .saturating_add(80_000)
                                    .saturating_add(u32::try_from(index).unwrap_or_default())
                            },
                        ),
                        location: location.clone(),
                        spell: Spell::TrapHexagon,
                        direction: MirDirection::Up,
                        param: false,
                    },
                })
                .collect()
        }
        Spell::ExplosiveTrap => {
            action
                .locations
                .iter()
                .enumerate()
                .map(|(index, location)| ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: action.spell_object_ids.get(index).copied().unwrap_or_else(
                            || {
                                action
                                    .caster_object_id
                                    .saturating_add(80_000)
                                    .saturating_add(u32::try_from(index).unwrap_or_default())
                            },
                        ),
                        location: location.clone(),
                        spell: Spell::ExplosiveTrap,
                        direction: action.direction,
                        param: false,
                    },
                })
                .collect()
        }
        Spell::HealingCircle => {
            let location = action
                .locations
                .first()
                .cloned()
                .unwrap_or(Point { x: 0, y: 0 });
            vec![ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: action
                        .spell_object_ids
                        .first()
                        .copied()
                        .unwrap_or_else(|| action.caster_object_id.saturating_add(70_000)),
                    location,
                    spell: Spell::HealingCircle,
                    direction: action.direction,
                    param: false,
                },
            }]
        }
        _ => Vec::new(),
    }
}

fn zone_rotated_direction(direction: MirDirection, steps: i8) -> MirDirection {
    let directions = [
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::Right,
        MirDirection::DownRight,
        MirDirection::Down,
        MirDirection::DownLeft,
        MirDirection::Left,
        MirDirection::UpLeft,
    ];
    let index = directions
        .iter()
        .position(|candidate| *candidate == direction)
        .unwrap_or(0) as i8;
    let rotated = (index + steps).rem_euclid(directions.len() as i8) as usize;
    directions[rotated]
}

fn trap_hexagon_spell_points(center: &Point) -> Vec<Point> {
    [
        MirDirection::Up,
        MirDirection::Right,
        MirDirection::Down,
        MirDirection::Left,
    ]
    .into_iter()
    .flat_map(|direction| {
        let start = offset_point(center, direction, 2);
        let side_direction = if matches!(direction, MirDirection::Up | MirDirection::Down) {
            MirDirection::Right
        } else {
            MirDirection::Up
        };
        [
            offset_point(&start, side_direction, 1),
            offset_point(&start, side_direction, -1),
        ]
    })
    .filter(|point| point.x > 0 && point.y > 0)
    .collect()
}

fn native_monster_spawn_packet(spawn: &ZoneMonsterSpawn, object_id: u32) -> ServerPacket {
    let max_hp = spawn.max_hp.max(1);
    let hp = spawn.hp.clamp(0, max_hp);
    ServerPacket::ObjectMonster {
        info: MonsterInfo {
            object_id,
            name: spawn.name.clone(),
            name_colour_argb: spawn.name_colour_argb,
            location: spawn.position.clone(),
            image: spawn.image,
            direction: spawn.direction,
            effect: 0,
            ai: spawn.ai,
            light: 0,
            dead: hp == 0,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id: 0,
            rarity: 0,
            buffs: Vec::new(),
        },
    }
}

fn native_summon_spawn_packet(
    spawn: &ZoneMonsterSpawn,
    object_id: u32,
    master_object_id: u32,
) -> ServerPacket {
    let max_hp = spawn.max_hp.max(1);
    let hp = spawn.hp.clamp(0, max_hp);
    ServerPacket::ObjectMonster {
        info: MonsterInfo {
            object_id,
            name: spawn.name.clone(),
            name_colour_argb: spawn.name_colour_argb,
            location: spawn.position.clone(),
            image: spawn.image,
            direction: spawn.direction,
            effect: 0,
            ai: spawn.ai,
            light: 0,
            dead: hp == 0,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: true,
            extra_byte: 0,
            master_object_id,
            rarity: 0,
            buffs: Vec::new(),
        },
    }
}

fn native_monster_health_percent(hp: i32, max_hp: i32) -> u8 {
    if max_hp <= 0 || hp <= 0 {
        return 0;
    }
    ((hp.saturating_mul(100) + max_hp - 1) / max_hp).clamp(1, 100) as u8
}

fn native_player_health_percent(hp: i32, max_hp: i32) -> u8 {
    let max_hp = max_hp.max(1);
    ((hp.max(0) * 100) / max_hp).clamp(0, 100) as u8
}

fn ground_drop_spawn_packet(drop: &GroundDropSnapshot) -> ServerPacket {
    let location = Point {
        x: drop.x,
        y: drop.y,
    };
    match &drop.loot {
        GroundDropLootSnapshot::Gold { amount } => ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: drop.object_id,
                gold: *amount,
                location,
            },
        },
        GroundDropLootSnapshot::InventoryItem { .. } => ServerPacket::ObjectItem {
            info: ObjectItemInfo {
                object_id: drop.object_id,
                name: drop.name.clone(),
                name_colour_argb: drop.name_colour_argb,
                location,
                image: 0,
                grade: 0,
            },
        },
    }
}

fn retained_zone_object_visibility_packets(object: &ZoneObject) -> Vec<ServerPacket> {
    let mut packets = vec![object.packet.clone()];
    if let Some(info) = object.health.clone() {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    if let Some(info) = object.mana.clone() {
        packets.push(ServerPacket::ObjectMana { info });
    }
    packets.extend(
        object
            .buffs
            .values()
            .map(|state| state.buff.clone())
            .map(|buff| ServerPacket::AddBuff { buff }),
    );
    packets
}

fn retained_zone_object_blocks_tile(object: &ZoneObject, point: &Point) -> bool {
    if object.position != *point {
        return false;
    }
    match &object.packet {
        ServerPacket::ObjectNpc { .. } => true,
        ServerPacket::ObjectMonster { info } => !info.dead && !info.hidden,
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            !info.dead && !info.hidden
        }
        _ => false,
    }
}

fn retained_zone_object_revive_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectRevived { info } => Some(info.object_id),
        _ => None,
    }
}

fn retained_zone_object_harvest_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectHarvested { movement } => Some(movement.object_id),
        _ => None,
    }
}

fn retained_zone_object_harvested_state(
    packet: &ServerPacket,
) -> Option<(u32, ZoneObjectDeadState)> {
    match packet {
        ServerPacket::ObjectHarvested { movement } => Some((
            movement.object_id,
            ZoneObjectDeadState {
                position: Some(movement.position.clone()),
                direction: Some(movement.direction),
            },
        )),
        _ => None,
    }
}

fn retained_zone_object_health_percent(packet: &ServerPacket) -> Option<u8> {
    match packet {
        ServerPacket::ObjectHealth { info } => Some(info.percent),
        _ => None,
    }
}

fn retained_zone_object_dead_stale_update(packet: &ServerPacket) -> bool {
    match packet {
        ServerPacket::ObjectTurn { .. }
        | ServerPacket::ObjectWalk { .. }
        | ServerPacket::ObjectRun { .. }
        | ServerPacket::ObjectBackStep { .. }
        | ServerPacket::ObjectSitDown { .. }
        | ServerPacket::ObjectDash { .. }
        | ServerPacket::ObjectDashFail { .. }
        | ServerPacket::ObjectDashAttack { .. }
        | ServerPacket::ObjectPushed { .. }
        | ServerPacket::ObjectMana { .. } => true,
        ServerPacket::ObjectHealth { info } => info.percent > 0,
        _ => false,
    }
}

fn shared_object_actor_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement } => Some(movement.object_id),
        ServerPacket::ObjectAttack { info } => Some(info.object_id),
        ServerPacket::ObjectRangeAttack { info } => Some(info.object_id),
        ServerPacket::ObjectMagic { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectProjectile { source_id, .. } => Some(*source_id),
        ServerPacket::ObjectStruck { info } => Some(info.attacker_id),
        _ => None,
    }
}

fn shared_object_result_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectHealth { info } => Some(info.object_id),
        ServerPacket::ObjectMana { info } => Some(info.object_id),
        ServerPacket::DamageIndicator { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectDied { info } => Some(info.object_id),
        ServerPacket::ObjectPoisoned { object_id, .. } => Some(*object_id),
        ServerPacket::AddBuff { buff } => Some(buff.object_id),
        ServerPacket::RemoveBuff { object_id, .. } | ServerPacket::PauseBuff { object_id, .. } => {
            Some(*object_id)
        }
        _ => None,
    }
}

fn retained_zone_object_dead_state(packet: &ServerPacket) -> Option<(u32, ZoneObjectDeadState)> {
    match packet {
        ServerPacket::ObjectDied { info } => Some((
            info.object_id,
            ZoneObjectDeadState {
                position: Some(info.location.clone()),
                direction: Some(info.direction),
            },
        )),
        ServerPacket::ObjectHealth { info } if info.percent == 0 => Some((
            info.object_id,
            ZoneObjectDeadState {
                position: None,
                direction: None,
            },
        )),
        ServerPacket::ObjectMonster { info } if info.dead => Some((
            info.object_id,
            ZoneObjectDeadState {
                position: Some(info.location.clone()),
                direction: Some(info.direction),
            },
        )),
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info }
            if info.dead =>
        {
            Some((
                info.object_id,
                ZoneObjectDeadState {
                    position: Some(info.location.clone()),
                    direction: Some(info.direction),
                },
            ))
        }
        _ => None,
    }
}

fn apply_retained_dead_state(object: &mut ZoneObject, dead_state: &ZoneObjectDeadState) {
    if let (Some(position), Some(direction)) = (&dead_state.position, dead_state.direction) {
        apply_retained_zone_object_packet(
            object,
            &ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: object.object_id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            },
        );
    } else {
        apply_retained_zone_object_packet(
            object,
            &ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: object.object_id,
                    percent: 0,
                    expire: 0,
                },
            },
        );
    }
}

fn apply_retained_revived_state(object: &mut ZoneObject) {
    apply_retained_zone_object_packet(
        object,
        &ServerPacket::ObjectRevived {
            info: ObjectRevivedInfo {
                object_id: object.object_id,
                effect: true,
            },
        },
    );
}

fn track_zone_object_buff_expiry(object: &mut ZoneObject, packet: &ServerPacket, now_ms: u64) {
    match packet {
        ServerPacket::AddBuff { buff } if buff.object_id == object.object_id && buff.visible => {
            let expires_at_ms = if buff.infinite || buff.expire_time <= 0 {
                None
            } else {
                Some(now_ms.saturating_add(buff.expire_time as u64))
            };
            object.buffs.insert(
                buff.buff_type,
                super::types::ZonePlayerBuff {
                    buff: buff.clone(),
                    expires_at_ms,
                },
            );
        }
        ServerPacket::RemoveBuff {
            object_id,
            buff_type,
        } if *object_id == object.object_id => {
            object.buffs.remove(buff_type);
        }
        ServerPacket::PauseBuff {
            object_id,
            buff_type,
            paused,
        } if *object_id == object.object_id => {
            if let Some(state) = object.buffs.get_mut(buff_type) {
                state.buff.paused = *paused;
            }
        }
        _ => {}
    }
}

fn chat_text_with_linked_items(message: &str, linked_items: &[ChatItem]) -> String {
    let mut text = message.to_string();
    for item in linked_items {
        let needle = format!("<{}>", item.title);
        if let Some((start, end)) = find_chat_link(&text, &needle) {
            text.replace_range(start..end, &item.internal_name());
        }
    }
    text
}

fn find_chat_link(text: &str, needle: &str) -> Option<(usize, usize)> {
    for (start, _) in text.char_indices() {
        let end = start.saturating_add(needle.len());
        if end <= text.len()
            && text.is_char_boundary(end)
            && text[start..end].eq_ignore_ascii_case(needle)
        {
            return Some((start, end));
        }
    }
    None
}

fn name_list_contains(names: &[String], target: &str) -> bool {
    names.iter().any(|name| name.eq_ignore_ascii_case(target))
}

fn players_visible_for_shout(source: &ZonePlayer, target: &ZonePlayer) -> bool {
    let dx = (source.position.x - target.position.x).abs();
    let dy = (source.position.y - target.position.y).abs();
    dx <= 36 && dy <= 28
}

fn packets_with_linked_items(
    linked_user_items: &[UserItem],
    mut packets: Vec<ServerPacket>,
) -> Vec<ServerPacket> {
    if linked_user_items.is_empty() {
        return packets;
    }
    let mut output = linked_user_items
        .iter()
        .cloned()
        .map(|item| ServerPacket::NewChatItem { item })
        .collect::<Vec<_>>();
    output.append(&mut packets);
    output
}

/// Deterministic 64-bit mix for shared hazard rolls (splitmix-style).
fn zone_hazard_hash(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Crystal reschedules each hazard `Random(3000, 15000)` ms ahead.
fn zone_hazard_interval_ms(strike_index: u64, salt: u64) -> u64 {
    3_000 + zone_hazard_hash(strike_index, salt) % 12_000
}

#[cfg(test)]
mod door_tests {
    use super::*;
    use crate::runtime::zone::collision::ZoneBounds;

    fn door_cell() -> Point {
        Point { x: 10, y: 10 }
    }

    #[test]
    fn shared_door_opens_unblocks_and_auto_closes() {
        let collision = ZoneCollision::unbounded()
            .with_bounds(ZoneBounds::new(0, 100, 0, 100))
            .with_door(7, vec![(10, 10)]);
        let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);

        // A closed shared door blocks movement for everyone.
        assert!(zone.collision.is_player_movement_blocked(&door_cell()));

        // Opening unblocks the cell and broadcasts OpenDoor{close:false} to all.
        let opened = zone.handle(ZoneCommand::OpenDoor {
            session_id: SessionId::new("opener"),
            door_index: 7,
            now_ms: 1_000,
        });
        assert!(opened.iter().any(|outbound| matches!(outbound,
            ZoneOutbound::ToAll { packets }
                if packets.iter().any(|packet|
                    matches!(packet, ServerPacket::OpenDoor { door_index: 7, close: false })))));
        assert!(!zone.collision.is_player_movement_blocked(&door_cell()));

        // Before the 5 s timer elapses the door stays open.
        let early = zone.handle(ZoneCommand::Tick { now_ms: 5_000 });
        assert!(!early.iter().any(|outbound| matches!(outbound,
            ZoneOutbound::ToAll { packets }
                if packets.iter().any(|packet|
                    matches!(packet, ServerPacket::OpenDoor { close: true, .. })))));
        assert!(!zone.collision.is_player_movement_blocked(&door_cell()));

        // After 5 s it auto-closes: re-blocked and OpenDoor{close:true} to all.
        let closed = zone.handle(ZoneCommand::Tick { now_ms: 6_000 });
        assert!(closed.iter().any(|outbound| matches!(outbound,
            ZoneOutbound::ToAll { packets }
                if packets.iter().any(|packet|
                    matches!(packet, ServerPacket::OpenDoor { door_index: 7, close: true })))));
        assert!(zone.collision.is_player_movement_blocked(&door_cell()));
    }

    #[test]
    fn shared_open_door_unknown_index_is_noop() {
        let collision = ZoneCollision::unbounded().with_bounds(ZoneBounds::new(0, 100, 0, 100));
        let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
        let out = zone.handle(ZoneCommand::OpenDoor {
            session_id: SessionId::new("opener"),
            door_index: 9,
            now_ms: 1_000,
        });
        assert!(out.is_empty(), "opening a non-existent door is a no-op");
    }
}

#[cfg(test)]
mod hazard_tests {
    use super::*;
    use crate::runtime::zone::collision::ZoneBounds;
    use crate::runtime::zone::types::{ZoneChatProfile, ZonePlayerCombatStats};
    use mir2_protocol::{MirClass, MirGender};

    fn join_player(zone: &mut ZoneRuntime, id: &str, position: Point) -> SessionId {
        let session_id = SessionId::new(id);
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session_id.clone(),
            account_id: format!("acct-{id}"),
            character_index: 0,
            object_id: 0,
            name: id.to_string(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 10,
            hp: 500,
            max_hp: 500,
            mp: 100,
            map_file_name: "0".to_string(),
            position,
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }));
        session_id
    }

    #[test]
    fn shared_lightning_hazard_strikes_and_damages_players() {
        let collision = ZoneCollision::unbounded().with_bounds(ZoneBounds::new(0, 100, 0, 100));
        let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
        let session = join_player(&mut zone, "p1", Point { x: 50, y: 50 });
        zone.handle(ZoneCommand::ConfigureHazards {
            session_id: session.clone(),
            lightning: true,
            fire: false,
            lightning_damage: 80,
            fire_damage: 0,
        });

        let mut saw_strike = false;
        let mut saw_damage = false;
        for i in 0..16u64 {
            for outbound in zone.handle(ZoneCommand::Tick {
                now_ms: 1_000 + i * 20_000,
            }) {
                match outbound {
                    ZoneOutbound::ToAll { packets } => {
                        if packets.iter().any(|packet| {
                            matches!(packet,
                            ServerPacket::ObjectSpell { info } if info.spell == Spell::MapLightning)
                        }) {
                            saw_strike = true;
                        }
                    }
                    ZoneOutbound::PlayerDamaged { session_id, damage }
                        if session_id == session && damage > 0 =>
                    {
                        saw_damage = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(saw_strike, "lightning should strike on a hazard map");
        assert!(
            saw_damage,
            "a direct strike should authoritatively damage the player"
        );
    }

    #[test]
    fn no_shared_hazard_strikes_without_configuration() {
        let collision = ZoneCollision::unbounded().with_bounds(ZoneBounds::new(0, 100, 0, 100));
        let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
        let _session = join_player(&mut zone, "p1", Point { x: 50, y: 50 });
        for i in 0..8u64 {
            for outbound in zone.handle(ZoneCommand::Tick {
                now_ms: 1_000 + i * 20_000,
            }) {
                if let ZoneOutbound::ToAll { packets } = &outbound {
                    assert!(!packets.iter().any(|packet| matches!(packet,
                        ServerPacket::ObjectSpell { info }
                            if info.spell == Spell::MapLightning || info.spell == Spell::MapLava)));
                }
            }
        }
    }
}

#[cfg(test)]
mod step1_ecs_mirror_tests {
    //! L2 step-1 invariant tests: the additive ECS player mirror must stay
    //! exactly in sync with the authoritative `players` map across join / walk /
    //! leave. Unit-test layer (not the external `shared_zone` crate) so the
    //! crate-private `ecs_mirror_matches_players` helper is visible.
    use super::super::types::{ZoneChatProfile, ZoneJoin, ZonePlayerCombatStats};
    use super::*;
    use mir2_protocol::{MirClass, MirDirection, MirGender, Point};

    fn test_join(session: &str, object_id: u32, x: i32, y: i32) -> ZoneJoin {
        ZoneJoin {
            session_id: SessionId::new(session),
            account_id: format!("{session}-acct"),
            character_index: object_id as i32,
            object_id,
            name: format!("P{object_id}"),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            hp: 60,
            max_hp: 60,
            mp: 100,
            map_file_name: "0".to_string(),
            position: Point { x, y },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }
    }

    fn test_zone() -> ZoneRuntime {
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded())
    }

    #[test]
    fn mirror_matches_players_across_join_walk_leave() {
        let mut zone = test_zone();
        let first = SessionId::new("first");

        zone.handle(ZoneCommand::Join(test_join("first", 101, 330, 270)));
        zone.handle(ZoneCommand::Join(test_join("second", 102, 332, 270)));
        assert!(
            zone.ecs_mirror_matches_players(),
            "mirror must match players after joins"
        );

        zone.handle(ZoneCommand::Walk {
            session_id: first.clone(),
            direction: MirDirection::Right,
            seq: 1,
            now_ms: 0,
        });
        let _ = zone.tick(0);
        assert!(
            zone.ecs_mirror_matches_players(),
            "mirror must match players after a walk commits"
        );

        zone.handle(ZoneCommand::Leave { session_id: first });
        assert!(
            zone.ecs_mirror_matches_players(),
            "mirror must match players after a leave"
        );
        assert!(
            zone.ecs_mirror_matches_players(),
            "mirror must still match with one player remaining"
        );
    }

    #[test]
    fn mirror_matches_after_rejoin_same_session() {
        let mut zone = test_zone();
        zone.handle(ZoneCommand::Join(test_join("first", 101, 330, 270)));
        // Re-join the same session at a new spot (zone replaces it); mirror must
        // not leak a stale entity.
        zone.handle(ZoneCommand::Join(test_join("first", 101, 340, 280)));
        assert!(zone.ecs_mirror_matches_players());
    }
}

#[cfg(test)]
mod movement_status_tests {
    use super::*;
    use mir2_protocol::{MirClass, MirGender};

    fn test_player() -> ZonePlayer {
        ZonePlayer::from_join(
            ZoneJoin {
                session_id: SessionId::new("tester"),
                account_id: "tester-account".to_string(),
                character_index: 0,
                object_id: 1,
                name: "Tester".to_string(),
                class: MirClass::Warrior,
                gender: MirGender::Male,
                level: 7,
                hp: 60,
                max_hp: 60,
                mp: 100,
                map_file_name: "0".to_string(),
                position: Point { x: 10, y: 10 },
                direction: MirDirection::Down,
                chat_profile: Default::default(),
                combat_stats: Default::default(),
            },
            1,
        )
    }

    #[test]
    fn frozen_poison_blocks_movement_until_it_expires() {
        let mut player = test_player();
        player.native_status_poison = CRYSTAL_POISON_FROZEN;
        player.native_status_poison_expires_at_ms = Some(1_000);

        assert!(zone_player_status_blocks_movement(&player, 500));
        // Expired frozen no longer blocks.
        assert!(!zone_player_status_blocks_movement(&player, 1_000));
        assert!(!zone_player_status_blocks_movement(&player, 1_500));
    }

    #[test]
    fn green_poison_alone_does_not_block_movement() {
        let mut player = test_player();
        player.native_status_poison = CRYSTAL_POISON_GREEN;
        player.native_status_poison_expires_at_ms = Some(10_000);
        assert!(!zone_player_status_blocks_movement(&player, 0));
    }

    #[test]
    fn slow_poison_doubles_the_step_delay() {
        let mut player = test_player();
        let base_walk = movement_delay_ms(false);
        assert_eq!(zone_player_move_delay_ms(&player, false, 0), base_walk);

        player.native_status_poison = CRYSTAL_POISON_SLOW;
        player.native_status_poison_expires_at_ms = Some(5_000);
        assert_eq!(
            zone_player_move_delay_ms(&player, false, 1_000),
            base_walk.saturating_mul(2)
        );
        // Once the slow expires the player returns to base cadence.
        assert_eq!(zone_player_move_delay_ms(&player, false, 5_000), base_walk);
    }

    #[test]
    fn riding_a_mount_speeds_movement_up() {
        let mut player = test_player();
        let base_walk = movement_delay_ms(false);
        player.riding_mount = true;
        player.mount_type = 3;
        let mounted = zone_player_move_delay_ms(&player, false, 0);
        assert!(
            mounted < base_walk,
            "mounted delay {mounted} should be faster than base {base_walk}"
        );
        assert_eq!(mounted, base_walk.saturating_mul(2) / 3);
    }

    #[test]
    fn dismounted_flag_without_a_mount_type_does_not_speed_up() {
        let mut player = test_player();
        // riding flag set but no mount equipped (mount_type < 0) must not boost.
        player.riding_mount = true;
        player.mount_type = -1;
        assert_eq!(
            zone_player_move_delay_ms(&player, false, 0),
            movement_delay_ms(false)
        );
    }
}
