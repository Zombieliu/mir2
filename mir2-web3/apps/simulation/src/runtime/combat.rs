use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::{
    crystal_monster_by_name, crystal_monster_manifest, format_localized_text,
    localized_text_or_fallback, CrystalRespawnTemplate,
};
use mir2_protocol::{ChatType, MirDirection, Point, ServerPacket};

use super::buffs::{apply_or_refresh_buff, BuffState};
use super::components::{
    current_player_object_id, entity_by_object_id, entity_facing, entity_name, entity_object_id,
    entity_position, player_entity, Facing, GeneralMeowMeowState, Monster, MonsterAgent,
    MonsterAiState, MonsterVitals, ObjectId, PlayerVitals, Position, SpawnSlotRef, SummonedMonster,
    TrainerDamageState,
};
use super::crystal_compat::*;
use super::drops::{
    handle_monster_defeat, harvest_monster_entity, harvest_target_in_direction,
    HarvestTargetSelection,
};
use super::equipment::{damage_weapon_durability, damage_worn_durability, total_attack_bonus};
use super::monster_ai::{
    advance_world, schedule_snow_wolf_king_death_explosion, set_guardian_rocks_active_near,
};
use super::monsters::{
    crystal_dynamic_monster_template, crystal_monster_attack_damage,
    crystal_monster_effect_for_name, crystal_monster_raw_attack_damage,
    crystal_respawn_template_from_monster, deterministic_roll, ignores_monster_damage,
    is_hidden_or_sleeping_target, monster_ignores_damage, monster_is_damageable,
    monster_is_stoned_zuma, monster_locks_player_target_on_hit, monster_melee_attack_packet,
    queue_pending_monster_spawn, schedule_monster_respawn, PendingMonsterSpawnAction,
};
use super::movement::{
    current_location, current_movement, direction_toward, directional_destination, offset_point,
    push_player_in_direction, tile_distance,
};
use super::npc::dismiss_dialog;
use super::packets::{
    object_died_info_for_entity, object_health_info_for_entity, object_struck_packet,
    player_struck_packet, system_message_key, visible_object_bundle_for_entity,
};
use super::resources::{
    is_in_world, runtime_tick, BuffResource, InventoryResource, RuntimeClockResource,
    RuntimeQueueResource, SessionResource,
};
use super::session::SimulationSession;

#[allow(deprecated)]
pub(super) fn attack_target_in_direction(world: &World, direction: MirDirection) -> Option<u32> {
    let player = player_entity(world)?;
    let player_position = entity_position(world, player)?;
    let target_position = offset_point(&player_position, direction, 1);

    world
        .iter_entities()
        .filter_map(|entity| {
            let position = entity.get::<Position>()?;
            if position.0 != target_position {
                return None;
            }

            let monster = entity.get::<MonsterAgent>()?;
            let ai_state = entity.get::<MonsterAiState>().copied().unwrap_or_default();
            if monster.dead || ai_state.hidden || monster_is_stoned_zuma(monster, &ai_state) {
                return None;
            }

            Some(entity.get::<ObjectId>()?.0)
        })
        .min()
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingCombatTarget {
    Player,
    Monster(Entity),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingPlayerStatusEffect {
    Paralysis {
        chance_denominator: u64,
        duration_ticks: u64,
    },
    Dazed {
        duration_ticks: u64,
    },
    StunPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    SlowAndFrozen {
        slow_chance_denominator: u64,
        slow_duration_ticks: u64,
        slow_salt: u64,
        frozen_chance_denominator: u64,
        frozen_duration_ticks: u64,
        frozen_salt: u64,
    },
    SlowAndParalysis {
        slow_chance_denominator: u64,
        slow_duration_ticks: u64,
        slow_salt: u64,
        paralysis_chance_denominator: u64,
        paralysis_duration_ticks: u64,
        paralysis_salt: u64,
    },
    FrozenPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    SlowPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    RedPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    WhiteFoxmanSlow {
        duration_ticks: u64,
    },
    BleedingPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    BlindnessPoison {
        chance_denominator: u64,
        duration_ticks: u64,
        salt: u64,
    },
    GreenPoison {
        chance_denominator: u64,
        duration_ticks: u64,
    },
    GreenPoisonAndParalysis {
        green_chance_denominator: u64,
        green_duration_ticks: u64,
        green_salt: u64,
        paralysis_chance_denominator: u64,
        paralysis_duration_ticks: u64,
        paralysis_salt: u64,
    },
}

#[derive(Debug, Clone)]
pub(super) struct PendingMonsterDefeatAction {
    pub(super) object_id: u32,
    pub(super) name: String,
}

#[derive(Debug, Clone)]
pub(super) struct PendingPlayerMovement {
    pub(super) direction: MirDirection,
    pub(super) distance: i32,
}

#[derive(Debug, Clone)]
pub(super) struct PendingCombatAction {
    pub(super) due_tick: u64,
    pub(super) attacker_id: u32,
    pub(super) target: PendingCombatTarget,
    pub(super) damage: i32,
    pub(super) player_status_effect: Option<PendingPlayerStatusEffect>,
    pub(super) due_packet: Option<ServerPacket>,
    pub(super) player_movement: Option<PendingPlayerMovement>,
    pub(super) on_monster_defeat: Option<PendingMonsterDefeatAction>,
}

pub(super) fn queue_pending_combat_action(world: &mut World, action: PendingCombatAction) {
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_combat_actions
        .push(action);
}

pub(super) fn queue_due_packet(world: &mut World, due_tick: u64, packet: ServerPacket) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id: 0,
            target: PendingCombatTarget::Player,
            damage: 0,
            player_status_effect: None,
            due_packet: Some(packet),
            player_movement: None,
            on_monster_defeat: None,
        },
    );
}

pub(super) fn schedule_damage_to_player(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    damage: i32,
) {
    schedule_damage_to_player_with_effect(
        world,
        due_tick,
        attacker_id,
        attacker_name,
        damage,
        None,
    );
}

pub(super) fn schedule_damage_to_player_with_effect(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    damage: i32,
    player_status_effect: Option<PendingPlayerStatusEffect>,
) {
    schedule_damage_to_player_with_effect_and_due_packet(
        world,
        due_tick,
        attacker_id,
        attacker_name,
        damage,
        player_status_effect,
        None,
    );
}

pub(super) fn schedule_damage_to_player_with_effect_and_due_packet(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    damage: i32,
    player_status_effect: Option<PendingPlayerStatusEffect>,
    due_packet: Option<ServerPacket>,
) {
    schedule_damage_to_player_with_effect_due_packet_and_movement(
        world,
        due_tick,
        attacker_id,
        attacker_name,
        damage,
        player_status_effect,
        due_packet,
        None,
    );
}

pub(super) fn schedule_damage_to_player_with_effect_due_packet_and_movement(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    _attacker_name: String,
    damage: i32,
    player_status_effect: Option<PendingPlayerStatusEffect>,
    due_packet: Option<ServerPacket>,
    player_movement: Option<PendingPlayerMovement>,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Player,
            damage,
            player_status_effect,
            due_packet,
            player_movement,
            on_monster_defeat: None,
        },
    );
}

pub(super) fn schedule_player_status_effect(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    player_status_effect: PendingPlayerStatusEffect,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Player,
            damage: 0,
            player_status_effect: Some(player_status_effect),
            due_packet: None,
            player_movement: None,
            on_monster_defeat: None,
        },
    );
}

pub(super) fn schedule_damage_to_monster(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    target_entity: Entity,
    damage: i32,
    target_name: Option<String>,
    defeat_action: Option<PendingMonsterDefeatAction>,
) {
    schedule_damage_to_monster_with_due_packet(
        world,
        due_tick,
        attacker_id,
        target_entity,
        damage,
        target_name,
        defeat_action,
        None,
    );
}

pub(super) fn schedule_damage_to_monster_with_due_packet(
    world: &mut World,
    due_tick: u64,
    attacker_id: u32,
    target_entity: Entity,
    damage: i32,
    _target_name: Option<String>,
    defeat_action: Option<PendingMonsterDefeatAction>,
    due_packet: Option<ServerPacket>,
) {
    queue_pending_combat_action(
        world,
        PendingCombatAction {
            due_tick,
            attacker_id,
            target: PendingCombatTarget::Monster(target_entity),
            damage,
            player_status_effect: None,
            due_packet,
            player_movement: None,
            on_monster_defeat: defeat_action,
        },
    );
}

pub(super) fn combat_delay_ticks(delay_ms: u64) -> u64 {
    delay_ms.max(1).div_ceil(1_000)
}

pub(super) fn melee_attack_delay_ticks() -> u64 {
    combat_delay_ticks(300)
}

pub(super) fn queued_before_world_tick_due_tick(current_tick: u64, delay_ticks: u64) -> u64 {
    current_tick + delay_ticks + 1
}

pub(super) fn ranged_attack_delay_ticks(source: &Point, target: &Point) -> u64 {
    let distance =
        u64::try_from(tile_distance(source, target).max(0)).expect("tile distance should fit u64");
    combat_delay_ticks(distance * 50 + 500)
}

pub(super) fn tucson_mage_wide_line_delay_ticks(
    source: &Point,
    target: &Point,
    direction: MirDirection,
) -> u64 {
    if *target == offset_point(source, direction, 1) {
        combat_delay_ticks(300)
    } else {
        ranged_attack_delay_ticks(source, target)
    }
}

pub(super) fn manectric_king_mass_attack_delay_ticks(source: &Point, target: &Point) -> u64 {
    let distance =
        u64::try_from(tile_distance(source, target).max(0)).expect("tile distance should fit u64");
    combat_delay_ticks(distance * 50 + 750)
}

pub(super) fn deterministic_chance_roll(
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

pub(super) fn apply_player_paralysis(world: &mut World, current_tick: u64, duration_ticks: u64) {
    apply_or_refresh_buff(
        world,
        BuffState {
            key: CAVE_MAGGOT_PARALYSIS_BUFF_KEY.to_string(),
            name: "Paralysis".to_string(),
            description: "Movement is stopped by paralysis poison.".to_string(),
            expires_at_tick: current_tick + duration_ticks,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        },
    );
}

pub(super) fn apply_player_red_poison(world: &mut World, current_tick: u64, duration_ticks: u64) {
    apply_or_refresh_buff(
        world,
        BuffState {
            key: YIMOOGI_RED_POISON_BUFF_KEY.to_string(),
            name: "Red Poison".to_string(),
            description: "Crystal red poison is active.".to_string(),
            expires_at_tick: current_tick + duration_ticks,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        },
    );
}

pub(super) fn apply_pending_player_status_effect(
    world: &mut World,
    current_tick: u64,
    attacker_id: u32,
    effect: PendingPlayerStatusEffect,
) {
    match effect {
        PendingPlayerStatusEffect::Paralysis {
            chance_denominator,
            duration_ticks,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, 7, chance_denominator) {
                return;
            }

            apply_player_paralysis(world, current_tick, duration_ticks);
        }
        PendingPlayerStatusEffect::Dazed { duration_ticks } => {
            apply_or_refresh_buff(
                world,
                BuffState {
                    key: HELL_KEEPER_DAZED_BUFF_KEY.to_string(),
                    name: "Dazed".to_string(),
                    description: "Crystal dazed poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::StunPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: MAN_TREE_STUN_BUFF_KEY.to_string(),
                    name: "Stun".to_string(),
                    description: "Crystal stun poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::SlowAndFrozen {
            slow_chance_denominator,
            slow_duration_ticks,
            slow_salt,
            frozen_chance_denominator,
            frozen_duration_ticks,
            frozen_salt,
        } => {
            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                slow_salt,
                slow_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                        name: "Slow".to_string(),
                        description: "Crystal slow poison is active.".to_string(),
                        expires_at_tick: current_tick + slow_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }

            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                frozen_salt,
                frozen_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: ICE_GUARD_FROZEN_BUFF_KEY.to_string(),
                        name: "Frozen".to_string(),
                        description: "Crystal frozen poison is active.".to_string(),
                        expires_at_tick: current_tick + frozen_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }
        }
        PendingPlayerStatusEffect::SlowAndParalysis {
            slow_chance_denominator,
            slow_duration_ticks,
            slow_salt,
            paralysis_chance_denominator,
            paralysis_duration_ticks,
            paralysis_salt,
        } => {
            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                slow_salt,
                slow_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                        name: "Slow".to_string(),
                        description: "Crystal slow poison is active.".to_string(),
                        expires_at_tick: current_tick + slow_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }

            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                paralysis_salt,
                paralysis_chance_denominator,
            ) {
                apply_player_paralysis(world, current_tick, paralysis_duration_ticks);
            }
        }
        PendingPlayerStatusEffect::FrozenPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: ICE_GUARD_FROZEN_BUFF_KEY.to_string(),
                    name: "Frozen".to_string(),
                    description: "Crystal frozen poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::SlowPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                    name: "Slow".to_string(),
                    description: "Crystal slow poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::RedPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_player_red_poison(world, current_tick, duration_ticks);
        }
        PendingPlayerStatusEffect::WhiteFoxmanSlow { duration_ticks } => {
            let level = world
                .resource::<SessionResource>()
                .selected_character
                .as_ref()
                .map(|character| i32::from(character.level))
                .unwrap_or(1);
            let threshold = (4 + (50 - level)).clamp(0, 20) as u64;
            if threshold == 0 {
                return;
            }
            if threshold < 20 {
                let value = current_tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ u64::from(attacker_id).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                    ^ 46_u64.wrapping_mul(0x94D0_49BB_1331_11EB);
                if value % 20 >= threshold {
                    return;
                }
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: ICE_GUARD_SLOW_BUFF_KEY.to_string(),
                    name: "Slow".to_string(),
                    description: "Crystal slow poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::BleedingPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: FROST_TIGER_BLEEDING_BUFF_KEY.to_string(),
                    name: "Bleeding".to_string(),
                    description: "Crystal bleeding poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::BlindnessPoison {
            chance_denominator,
            duration_ticks,
            salt,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, salt, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: RESTLESS_JAR_BLINDNESS_BUFF_KEY.to_string(),
                    name: "Blindness".to_string(),
                    description: "Crystal blindness poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::GreenPoison {
            chance_denominator,
            duration_ticks,
        } => {
            if !deterministic_chance_roll(current_tick, attacker_id, 28, chance_denominator) {
                return;
            }

            apply_or_refresh_buff(
                world,
                BuffState {
                    key: TOXIC_GHOUL_GREEN_POISON_BUFF_KEY.to_string(),
                    name: "Green Poison".to_string(),
                    description: "Crystal green poison is active.".to_string(),
                    expires_at_tick: current_tick + duration_ticks,
                    attack_bonus: 0,
                    defence_bonus: 0,
                    stats: Vec::new(),
                },
            );
        }
        PendingPlayerStatusEffect::GreenPoisonAndParalysis {
            green_chance_denominator,
            green_duration_ticks,
            green_salt,
            paralysis_chance_denominator,
            paralysis_duration_ticks,
            paralysis_salt,
        } => {
            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                green_salt,
                green_chance_denominator,
            ) {
                apply_or_refresh_buff(
                    world,
                    BuffState {
                        key: TOXIC_GHOUL_GREEN_POISON_BUFF_KEY.to_string(),
                        name: "Green Poison".to_string(),
                        description: "Crystal green poison is active.".to_string(),
                        expires_at_tick: current_tick + green_duration_ticks,
                        attack_bonus: 0,
                        defence_bonus: 0,
                        stats: Vec::new(),
                    },
                );
            }

            if deterministic_chance_roll(
                current_tick,
                attacker_id,
                paralysis_salt,
                paralysis_chance_denominator,
            ) {
                apply_player_paralysis(world, current_tick, paralysis_duration_ticks);
            }
        }
    }
}

pub(super) fn resolve_pending_combat_actions(
    world: &mut World,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let due_actions = {
        let mut resources = world.resource_mut::<RuntimeQueueResource>();
        let mut due = Vec::new();
        let mut pending = Vec::with_capacity(resources.pending_combat_actions.len());

        for action in resources.pending_combat_actions.drain(..) {
            if action.due_tick <= current_tick {
                due.push(action);
            } else {
                pending.push(action);
            }
        }

        resources.pending_combat_actions = pending;
        due
    };

    for action in due_actions {
        match action.target {
            PendingCombatTarget::Player => {
                let Some(player) = player_entity(world) else {
                    continue;
                };
                if action.damage > 0 {
                    if let Some(mut vitals) = world.entity_mut(player).get_mut::<PlayerVitals>() {
                        vitals.hp = (vitals.hp - action.damage).max(1);
                    }
                    packets.push(player_struck_packet(action.attacker_id));
                    packets.extend(damage_worn_durability(world, current_tick));
                }
                if let Some(effect) = action.player_status_effect {
                    apply_pending_player_status_effect(
                        world,
                        current_tick,
                        action.attacker_id,
                        effect,
                    );
                }
                if let Some(packet) = action.due_packet {
                    packets.push(packet);
                }
                if let Some(movement) = action.player_movement {
                    let _ = push_player_in_direction(
                        world,
                        player,
                        movement.direction,
                        movement.distance,
                        packets,
                    );
                }
            }
            PendingCombatTarget::Monster(target_entity) => {
                if !monster_is_damageable(world, target_entity) {
                    continue;
                }
                if let Some(packet) = action.due_packet {
                    packets.push(packet);
                }
                let ignores_damage = monster_ignores_damage(world, target_entity);
                if let Some(packet) = object_struck_packet(world, target_entity, action.attacker_id)
                {
                    packets.push(packet);
                }
                if ignores_damage {
                    continue;
                }
                let monster_dead = damage_monster_entity(
                    world,
                    target_entity,
                    action.damage,
                    current_tick,
                    packets,
                );
                if monster_dead {
                    if let Some(defeat_action) = action.on_monster_defeat {
                        handle_monster_defeat(
                            world,
                            defeat_action.object_id,
                            &defeat_action.name,
                            packets,
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn trainer_dps(total_damage: i32, start_tick: u64, end_tick: u64) -> f64 {
    let elapsed_ticks = end_tick.saturating_sub(start_tick).max(1);
    f64::from(total_damage) / elapsed_ticks as f64
}

pub(super) fn trainer_damage_chat(
    world: &World,
    damage: i32,
    state: &TrainerDamageState,
) -> ServerPacket {
    let dps = trainer_dps(state.total_damage, state.start_tick, state.last_attack_tick);
    let language = world.resource::<SessionResource>().language;
    let actor = localized_text_or_fallback(language, "server.You", "You");
    ServerPacket::Chat {
        message: format_localized_text(
            language,
            "server.PetInflictedDamageDps",
            [
                damage.to_string(),
                "Physical Agility".to_string(),
                format!("{dps:.2}"),
                actor,
            ],
        ),
        chat_type: ChatType::Trainer,
    }
}

pub(super) fn trainer_average_chat(
    world: &World,
    state: &TrainerDamageState,
) -> Option<ServerPacket> {
    if state.hit_count == 0 {
        return None;
    }

    let average_damage = state.total_damage / i32::try_from(state.hit_count).ok()?.max(1);
    let dps = trainer_dps(state.total_damage, state.start_tick, state.last_attack_tick);
    Some(ServerPacket::Chat {
        message: format_localized_text(
            world.resource::<SessionResource>().language,
            "server.AverageDamageOnTrainer",
            [average_damage.to_string(), format!("{dps:.2}")],
        ),
        chat_type: ChatType::Trainer,
    })
}

pub(super) fn record_trainer_damage(
    world: &mut World,
    monster_entity: Entity,
    damage: i32,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if damage <= 0 {
        return;
    }

    let mut state = world
        .entity(monster_entity)
        .get::<TrainerDamageState>()
        .copied()
        .unwrap_or_default();
    if state.hit_count == 0 {
        state.start_tick = current_tick;
        state.total_damage = 0;
    }
    state.hit_count = state.hit_count.saturating_add(1);
    state.total_damage = state.total_damage.saturating_add(damage);
    state.last_attack_tick = current_tick;

    world.entity_mut(monster_entity).insert(state);
    packets.push(trainer_damage_chat(world, damage, &state));
}

pub(super) fn emit_due_trainer_average_chats(
    world: &mut World,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let mut due = Vec::new();
    {
        let mut query =
            world.query_filtered::<(Entity, &TrainerDamageState), bevy_ecs::query::With<Monster>>();
        for (entity, state) in query.iter(world) {
            if state.hit_count > 0
                && current_tick > state.last_attack_tick + TRAINER_DAMAGE_REPORT_IDLE_TICKS
            {
                due.push((entity, *state));
            }
        }
    }

    for (entity, state) in due {
        if let Some(packet) = trainer_average_chat(world, &state) {
            packets.push(packet);
        }
        world
            .entity_mut(entity)
            .insert(TrainerDamageState::default());
    }
}

pub(super) fn damage_monster_entity(
    world: &mut World,
    monster_entity: Entity,
    damage: i32,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if damage <= 0 {
        return false;
    }

    let spawn_ref = world.entity(monster_entity).get::<SpawnSlotRef>().copied();
    let summoned_state = world
        .entity(monster_entity)
        .get::<SummonedMonster>()
        .copied();
    let position = entity_position(world, monster_entity);
    let name = entity_name(world, monster_entity);
    let (current_hp, max_hp, mut agent, ai_state) = {
        let monster = world.entity(monster_entity);
        let vitals = monster.get::<MonsterVitals>().expect("monster vitals");
        let agent = monster.get::<MonsterAgent>().expect("monster agent");
        let ai_state = monster.get::<MonsterAiState>().copied().unwrap_or_default();
        (vitals.hp, vitals.max_hp, agent.clone(), ai_state)
    };

    if agent.dead || current_hp <= 0 {
        return false;
    }

    if agent.ai == 56 {
        record_trainer_damage(world, monster_entity, damage, current_tick, packets);
        return false;
    }

    if ignores_monster_damage(&agent) || (agent.ai == 98 && ai_state.extra_byte < 4) {
        return false;
    }

    if agent.ai == 54 {
        if ai_state.mode {
            return false;
        }
        if damage >= current_hp {
            let mut sleep_state = ai_state;
            sleep_state.mode = true;
            sleep_state.next_state_tick = current_tick + DRAGON_STATUE_SLEEP_DURATION_TICKS;
            agent.tracking_player = false;
            agent.next_attack_tick = current_tick + 1;
            agent.next_move_tick = current_tick + 1;
            world.entity_mut(monster_entity).insert((
                MonsterVitals { hp: 0, max_hp },
                agent,
                sleep_state,
            ));
            if let Some(info) = object_health_info_for_entity(world, monster_entity, 0) {
                packets.push(ServerPacket::ObjectHealth { info });
            }
            return false;
        }
    }

    if agent.ai == 47 && summoned_state.is_some() {
        if let Some(summoned) = summoned_state {
            if let Some(parent) = entity_by_object_id(world, summoned.summoner_object_id) {
                if let Some(mut parent_ai_state) =
                    world.entity_mut(parent).get_mut::<MonsterAiState>()
                {
                    parent_ai_state.mode = false;
                }
            }
        }
        return false;
    }

    let trap_rock_first_hit_collapse = agent.ai == 47 && !ai_state.hidden && ai_state.mode;
    let mut next_ai_state = ai_state;
    if trap_rock_first_hit_collapse {
        next_ai_state.mode = false;
    }
    if matches!(agent.ai, 124 | 125) && next_ai_state.mode {
        if let Some(object_id) = entity_object_id(world, monster_entity) {
            if deterministic_chance_roll(current_tick, object_id, 1240, 4) {
                next_ai_state.mode = false;
            }
        }
    }

    let general_meow_meow_shield_armour = if agent.ai == 123
        && world
            .entity(monster_entity)
            .get::<GeneralMeowMeowState>()
            .map(|state| state.shield_until_tick > current_tick)
            .unwrap_or(false)
    {
        GENERAL_MEOW_MEOW_SHIELD_ARMOUR
    } else {
        0
    };

    let applied_damage = if trap_rock_first_hit_collapse {
        current_hp
    } else if matches!(agent.ai, 3 | 128) {
        1
    } else {
        damage.saturating_sub(general_meow_meow_shield_armour)
    };
    if applied_damage <= 0 {
        return false;
    }
    let next_hp = current_hp.saturating_sub(applied_damage).max(0);
    let monster_dead = next_hp == 0;
    let death_agent = agent.clone();
    agent.dead = monster_dead;

    let dead_ai = agent.ai;
    world.entity_mut(monster_entity).insert((
        MonsterVitals {
            hp: next_hp,
            max_hp,
        },
        agent,
        next_ai_state,
    ));

    let should_despawn_summoned = monster_dead
        && spawn_ref.is_none()
        && dead_ai != 60
        && summoned_state
            .map(|summoned| summoned.despawn_tick_after_death.is_none())
            .unwrap_or(false);
    if monster_dead {
        if let Some(spawn_ref) = spawn_ref {
            schedule_monster_respawn(world, spawn_ref, current_tick);
        }
        if dead_ai == 25 && ai_state.extra_byte < REVIVING_ZOMBIE_MAX_REVIVALS {
            let mut revival_state = ai_state;
            revival_state.mode = true;
            revival_state.next_state_tick = current_tick + REVIVING_ZOMBIE_REVIVE_DELAY_TICKS;
            world.entity_mut(monster_entity).insert(revival_state);
        }
    }

    if let Some(info) = object_health_info_for_entity(world, monster_entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    if monster_dead {
        if let Some(info) = object_died_info_for_entity(world, monster_entity, 0) {
            packets.push(ServerPacket::ObjectDied { info });
        }
    }

    if monster_dead && dead_ai == 60 {
        if let Some(position) = position.as_ref() {
            explode_vampire_spider(world, monster_entity, position, current_tick, packets);
        }
    }
    if monster_dead && dead_ai == 119 {
        if let Some(position) = position.as_ref() {
            complete_jar1_death(world, monster_entity, position, current_tick);
        }
    }
    if monster_dead && dead_ai == 128 {
        if let Some(position) = position.as_ref() {
            complete_tucson_egg_death(world, monster_entity, position, current_tick);
        }
    }
    if monster_dead && dead_ai == 97 {
        if let Some(summoned) = summoned_state {
            advance_hell_lord_stage(world, summoned.summoner_object_id, current_tick, packets);
        }
    }
    if monster_dead && dead_ai == 50 {
        if let Some(position) = position.as_ref() {
            set_guardian_rocks_active_near(world, position, false);
        }
    }
    if monster_dead && dead_ai == 180 {
        if let (Some(position), Some(name)) = (position.as_ref(), name.as_deref()) {
            schedule_snow_wolf_king_death_explosion(
                world,
                monster_entity,
                &death_agent,
                position,
                name,
                current_tick,
            );
        }
    }
    if monster_dead && dead_ai == 63 {
        if let Some(position) = position {
            explode_charmed_snake(world, monster_entity, &position, current_tick);
        }
    }
    if should_despawn_summoned {
        let _ = world.despawn(monster_entity);
    }

    let _ = name;
    monster_dead
}

pub(super) fn crystal_jar1_slave_template(
    monster_name: &str,
    monster_entity: Entity,
    current_tick: u64,
) -> Option<CrystalRespawnTemplate> {
    let parent = crystal_monster_by_name(monster_name)?;
    let minimum_level = parent.level.saturating_sub(10);
    let valid_monsters: Vec<_> = crystal_monster_manifest()
        .monsters
        .into_iter()
        .filter(|monster| monster.level <= parent.level && monster.level >= minimum_level)
        .filter(|monster| !monster.is_boss)
        .filter(|monster| !matches!(monster.ai, 72 | 73 | 80 | 81 | 82))
        .collect();

    let index = deterministic_roll(
        current_tick,
        monster_entity.index() as usize,
        usize::from(parent.ai),
        valid_monsters.len() as u64,
    ) as usize;
    valid_monsters
        .get(index)
        .cloned()
        .map(crystal_respawn_template_from_monster)
}

pub(super) fn complete_jar1_death(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
) {
    let name = entity_name(world, monster_entity).unwrap_or_else(|| "Jar1".to_string());
    let Some(template) = crystal_jar1_slave_template(&name, monster_entity, current_tick) else {
        return;
    };
    let target_entity = world
        .entity(monster_entity)
        .get::<MonsterAgent>()
        .filter(|agent| agent.tracking_player && agent.hostile_to_player)
        .and_then(|_| player_entity(world));

    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: current_tick + JAR1_DEATH_SPAWN_DELAY_TICKS,
            summoner_entity: monster_entity,
            template: CrystalRespawnTemplate {
                location: position.clone(),
                ..template
            },
            target_entity,
            summon_metadata: None,
            hostile_to_player_override: Some(true),
        },
    );
}

pub(super) fn complete_tucson_egg_death(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let name = entity_name(world, monster_entity).unwrap_or_else(|| "TucsonEgg".to_string());
    let damage = crystal_monster_raw_attack_damage(&name);
    if damage > 0 {
        if let Some(player) = player_entity(world) {
            if let Some(player_position) = entity_position(world, player) {
                if tile_distance(position, &player_position) <= 1 {
                    schedule_damage_to_player_with_effect(
                        world,
                        current_tick + TUCSON_EGG_DEATH_DELAY_TICKS,
                        attacker_id,
                        name.clone(),
                        damage,
                        Some(PendingPlayerStatusEffect::GreenPoison {
                            chance_denominator: TUCSON_EGG_GREEN_POISON_CHANCE_DENOMINATOR,
                            duration_ticks: TUCSON_EGG_GREEN_POISON_DURATION_TICKS,
                        }),
                    );
                }
            }
        }
    }

    if crystal_monster_effect_for_name(&name) != 1 {
        return;
    }
    let Some(template) = crystal_dynamic_monster_template("GeneralTucson")
        .or_else(|| crystal_dynamic_monster_template("TucsonGeneral"))
    else {
        return;
    };
    let direction = entity_facing(world, monster_entity).unwrap_or(MirDirection::Down);
    let spawn_position =
        directional_destination(world, position, direction, 1, Some(monster_entity))
            .unwrap_or_else(|| position.clone());
    queue_pending_monster_spawn(
        world,
        PendingMonsterSpawnAction {
            due_tick: current_tick + TUCSON_EGG_DEATH_DELAY_TICKS,
            summoner_entity: monster_entity,
            template: CrystalRespawnTemplate {
                location: spawn_position,
                ..template
            },
            target_entity: player_entity(world),
            summon_metadata: None,
            hostile_to_player_override: Some(true),
        },
    );
}

pub(super) fn advance_hell_lord_stage(
    world: &mut World,
    lord_object_id: u32,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(lord_entity) = entity_by_object_id(world, lord_object_id) else {
        return;
    };

    {
        let mut entry = world.entity_mut(lord_entity);
        let Some(agent) = entry.get::<MonsterAgent>() else {
            return;
        };
        if agent.ai != 98 || agent.dead {
            return;
        }
        let mut ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if ai_state.extra_byte >= 4 {
            return;
        }

        ai_state.extra_byte += 1;
        ai_state.mode = true;
        ai_state.next_state_tick = current_tick + HELL_LORD_RAGE_DELAY_TICKS;
        entry.insert(ai_state);
    }

    if let Some((_, bundle)) = visible_object_bundle_for_entity(
        world,
        lord_entity,
        world.resource::<SessionResource>().language,
    ) {
        if matches!(bundle.spawn_packet, ServerPacket::ObjectMonster { .. }) {
            packets.push(bundle.spawn_packet);
        }
    }
}

pub(super) fn explode_hell_bomb(
    world: &mut World,
    monster_entity: Entity,
    agent: &mut MonsterAgent,
    position: &Point,
    current_tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let attacker_name =
        entity_name(world, monster_entity).unwrap_or_else(|| "HellBomb".to_string());
    let attacker_hostile_to_player = agent.hostile_to_player;

    if let Some(packet) =
        monster_melee_attack_packet(world, monster_entity, position, MirDirection::Up)
    {
        packets.push(packet);
    }

    agent.dead = true;
    let max_hp = world
        .entity(monster_entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp)
        .unwrap_or(1);
    {
        let mut entry = world.entity_mut(monster_entity);
        entry.insert((
            MonsterVitals { hp: 0, max_hp },
            agent.clone(),
            Facing(MirDirection::Up),
        ));
        if let Some(mut summoned) = entry.get_mut::<SummonedMonster>() {
            summoned.despawn_tick_after_death =
                Some(current_tick + HELL_BOMB_EXPLOSION_DELAY_TICKS + 1);
        }
    }

    if let Some(info) = object_health_info_for_entity(world, monster_entity, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    if let Some(info) = object_died_info_for_entity(world, monster_entity, 0) {
        packets.push(ServerPacket::ObjectDied { info });
    }

    schedule_hell_bomb_explosion_damage(
        world,
        monster_entity,
        position,
        current_tick,
        attacker_id,
        attacker_name,
        attacker_hostile_to_player,
    );
}

pub(super) fn schedule_hell_bomb_explosion_damage(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
    attacker_id: u32,
    attacker_name: String,
    attacker_hostile_to_player: bool,
) {
    let damage = crystal_monster_attack_damage(&attacker_name);
    let due_tick = current_tick + HELL_BOMB_EXPLOSION_DELAY_TICKS;
    let player_status_effect = hell_bomb_player_status_effect(&attacker_name);

    if attacker_hostile_to_player {
        if let Some(player) = player_entity(world) {
            if let Some(player_position) = entity_position(world, player) {
                if tile_distance(position, &player_position) <= HELL_BOMB_EXPLOSION_RADIUS {
                    schedule_damage_to_player_with_effect(
                        world,
                        due_tick,
                        attacker_id,
                        attacker_name.clone(),
                        damage,
                        player_status_effect,
                    );
                }
            }
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == monster_entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > HELL_BOMB_EXPLOSION_RADIUS {
            continue;
        }

        schedule_damage_to_monster(
            world,
            due_tick,
            attacker_id,
            target_entity,
            damage,
            None,
            None,
        );
    }
}

pub(super) fn hell_bomb_player_status_effect(name: &str) -> Option<PendingPlayerStatusEffect> {
    match name {
        "HellBomb1" => Some(PendingPlayerStatusEffect::FrozenPoison {
            chance_denominator: 1,
            duration_ticks: HELL_BOMB_POISON_DURATION_TICKS,
            salt: 991,
        }),
        "HellBomb2" => Some(PendingPlayerStatusEffect::Dazed {
            duration_ticks: HELL_BOMB_POISON_DURATION_TICKS,
        }),
        "HellBomb3" => Some(PendingPlayerStatusEffect::BleedingPoison {
            chance_denominator: 1,
            duration_ticks: HELL_BOMB_POISON_DURATION_TICKS,
            salt: 993,
        }),
        _ => None,
    }
}

pub(super) fn explode_bomb_spider(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };

    let bomb_spider_damage = crystal_monster_by_name("BombSpider")
        .map(|monster| monster.max_dc.max(monster.min_dc).max(1))
        .unwrap_or(6);

    if tile_distance(
        position,
        &entity_position(world, player_entity(world).expect("player")).expect("player position"),
    ) <= 1
    {
        schedule_damage_to_player(
            world,
            world.resource::<RuntimeClockResource>().tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
            attacker_id,
            "BombSpider".to_string(),
            bomb_spider_damage,
        );
    }

    let current_tick = world.resource::<RuntimeClockResource>().tick;
    let _ = damage_monster_entity(world, monster_entity, i32::MAX, current_tick, packets);
}

pub(super) fn explode_vampire_spider(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
    _packets: &mut Vec<ServerPacket>,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let Some(attacker_agent) = world.entity(monster_entity).get::<MonsterAgent>().cloned() else {
        return;
    };

    let spider_damage = crystal_monster_by_name("VampireSpider")
        .map(|monster| (monster.max_dc.max(monster.min_dc) * 10).max(1))
        .unwrap_or(10);

    if let Some(player) = player_entity(world) {
        let Some(player_position) = entity_position(world, player) else {
            return;
        };
        if tile_distance(position, &player_position) <= 1 {
            schedule_damage_to_player(
                world,
                current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
                attacker_id,
                "VampireSpider".to_string(),
                spider_damage,
            );
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == monster_entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_agent.hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > 1 {
            continue;
        }

        schedule_damage_to_monster(
            world,
            current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
            attacker_id,
            target_entity,
            spider_damage,
            None,
            None,
        );
    }

    let Some(summoned) = world
        .entity(monster_entity)
        .get::<SummonedMonster>()
        .copied()
    else {
        return;
    };
    if let Some(mut summoned_mut) = world
        .entity_mut(monster_entity)
        .get_mut::<SummonedMonster>()
    {
        summoned_mut.despawn_tick_after_death = Some(
            summoned
                .despawn_tick_after_death
                .unwrap_or(current_tick + 1),
        );
    }
}

pub(super) fn explode_charmed_snake(
    world: &mut World,
    monster_entity: Entity,
    position: &Point,
    current_tick: u64,
) {
    let Some(attacker_id) = entity_object_id(world, monster_entity) else {
        return;
    };
    let Some(attacker_agent) = world.entity(monster_entity).get::<MonsterAgent>().cloned() else {
        return;
    };
    let damage = 30;

    if let Some(player) = player_entity(world) {
        let Some(player_position) = entity_position(world, player) else {
            return;
        };
        if tile_distance(position, &player_position) <= 1 && attacker_agent.hostile_to_player {
            schedule_damage_to_player(
                world,
                current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
                attacker_id,
                "CharmedSnake".to_string(),
                damage,
            );
        }
    }

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();
    for target_entity in monster_entities {
        if target_entity == monster_entity {
            continue;
        }
        let entry = world.entity(target_entity);
        let Some(target_agent) = entry.get::<MonsterAgent>() else {
            continue;
        };
        let target_ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        if target_agent.dead || is_hidden_or_sleeping_target(target_agent, &target_ai_state) {
            continue;
        }
        if attacker_agent.hostile_to_player == target_agent.hostile_to_player {
            continue;
        }
        let Some(target_position) = entry.get::<Position>().map(|value| value.0.clone()) else {
            continue;
        };
        if tile_distance(position, &target_position) > 1 {
            continue;
        }

        schedule_damage_to_monster(
            world,
            current_tick + BOMB_SPIDER_EXPLOSION_DELAY_TICKS,
            attacker_id,
            target_entity,
            damage,
            None,
            None,
        );
    }
}

impl SimulationSession {
    pub fn attack(&mut self, object_id: u32) -> Vec<ServerPacket> {
        let packets = self.attack_impl(object_id);
        self.finalize_packets(packets)
    }

    pub fn harvest(&mut self, direction: MirDirection) -> Vec<ServerPacket> {
        let packets = self.harvest_impl(direction);
        self.finalize_packets(packets)
    }

    pub(super) fn attack_impl(&mut self, object_id: u32) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        dismiss_dialog(self.app.world_mut());
        let Some(monster_entity) = entity_by_object_id(self.app.world(), object_id) else {
            return Vec::new();
        };

        let player_entity = player_entity(self.app.world()).expect("player should exist");
        let player_position =
            entity_position(self.app.world(), player_entity).expect("player position");
        let target_position =
            entity_position(self.app.world(), monster_entity).expect("monster position");
        let target_name =
            entity_name(self.app.world(), monster_entity).unwrap_or_else(|| "Target".to_string());

        let monster_entry = self.app.world().entity(monster_entity);
        let Some(monster_agent) = monster_entry.get::<MonsterAgent>() else {
            return Vec::new();
        };
        let monster_ai_state = monster_entry
            .get::<MonsterAiState>()
            .copied()
            .unwrap_or_default();
        if monster_agent.dead
            || monster_ai_state.hidden
            || monster_is_stoned_zuma(monster_agent, &monster_ai_state)
        {
            return Vec::new();
        }

        let Some(direction) = direction_toward(&player_position, &target_position) else {
            return Vec::new();
        };

        let mut packets = Vec::new();
        {
            let mut player = self.app.world_mut().entity_mut(player_entity);
            let mut facing = player.get_mut::<Facing>().expect("player facing");
            if facing.0 != direction {
                facing.0 = direction;
                packets.push(ServerPacket::UserLocation {
                    location: current_location(self.app.world()),
                });
            }
        }

        if tile_distance(&player_position, &target_position) > 1 {
            return packets;
        }

        let damage = {
            let resources = self.app.world().resource::<InventoryResource>();
            let session = self.app.world().resource::<SessionResource>();
            18 + i32::from(
                session
                    .selected_character
                    .as_ref()
                    .map(|c| c.level)
                    .unwrap_or(1)
                    / 2,
            ) + total_attack_bonus(&resources, self.app.world().resource::<BuffResource>())
        };
        let current_tick = runtime_tick(self.app.world());
        let player_object_id =
            current_player_object_id(self.app.world()).expect("player object id");
        let hit_due_tick =
            queued_before_world_tick_due_tick(current_tick, melee_attack_delay_ticks());

        if let Some(packet) = monster_melee_attack_packet(
            self.app.world(),
            player_entity,
            &player_position,
            direction,
        ) {
            packets.push(packet);
        }
        if let Some(packet) = damage_weapon_durability(self.app.world_mut(), current_tick) {
            packets.push(packet);
        }
        let mut agent = {
            let monster = self.app.world().entity(monster_entity);
            let agent = monster.get::<MonsterAgent>().expect("monster agent");
            agent.clone()
        };
        if monster_locks_player_target_on_hit(&agent) {
            agent.tracking_player = true;
        }
        self.app
            .world_mut()
            .entity_mut(monster_entity)
            .insert(agent);
        schedule_damage_to_monster(
            self.app.world_mut(),
            hit_due_tick,
            player_object_id,
            monster_entity,
            damage,
            Some(target_name.clone()),
            Some(PendingMonsterDefeatAction {
                object_id,
                name: target_name.clone(),
            }),
        );

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }

    pub(super) fn attack_in_direction(&mut self, direction: MirDirection) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        if let Some(player) = player_entity(self.app.world()) {
            self.app
                .world_mut()
                .entity_mut(player)
                .insert(Facing(direction));
        }

        let Some(object_id) = attack_target_in_direction(self.app.world(), direction) else {
            return vec![ServerPacket::UserLocation {
                location: current_location(self.app.world()),
            }];
        };

        self.attack_impl(object_id)
    }

    pub(super) fn harvest_impl(&mut self, direction: MirDirection) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        dismiss_dialog(self.app.world_mut());
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };

        self.app
            .world_mut()
            .entity_mut(player)
            .insert(Facing(direction));

        let mut packets = vec![ServerPacket::UserLocation {
            location: current_location(self.app.world()),
        }];

        if let Some(selection) = harvest_target_in_direction(self.app.world(), direction) {
            packets.push(ServerPacket::ObjectHarvest {
                movement: current_movement(self.app.world()),
            });
            match selection {
                HarvestTargetSelection::Target(target) => {
                    packets.extend(harvest_monster_entity(self.app.world_mut(), target));
                }
                HarvestTargetSelection::OwnerBlocked => {
                    packets.push(system_message_key(
                        self.app.world(),
                        "server.NoNearbyOwnedCarcasses",
                    ));
                }
            }
        }

        packets.extend(advance_world(self.app.world_mut()));
        packets
    }
}
