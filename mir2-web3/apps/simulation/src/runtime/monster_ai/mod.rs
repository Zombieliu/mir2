//! Monster AI: per-tick world advancement, the numeric Crystal AI-id
//! dispatch, and one submodule per monster behaviour.
//!
//! Split out of the former monolithic `monster_ai.rs` by a mechanical,
//! behavior-preserving refactor. Adding a new monster = a new `<name>.rs`
//! submodule (declared + re-exported here) plus one new arm in the
//! `update_special_monster_state` dispatch below.

mod armadillo;
mod axe_skeleton_fear;
mod bomb_spider;
mod bone_lord;
mod cannibal_plant;
mod common;
mod deer_run_away;
mod dig_out_zombie;
mod dragon_statue_sleep;
mod evil_centipede;
mod foxman_fear;
mod frost_tiger_sitting;
mod general_meow_meow;
mod great_fox_spirit;
mod hell_bomb;
mod hell_lord;
mod holy_deva_fear;
mod horned_archer;
mod horned_commander;
mod horned_mage;
mod horned_sorceror;
mod horned_warrior;
mod kirin_ice_thrust;
mod reviving_zombie;
mod shinsu;
mod snake_totem;
mod snow_wolf_king;
mod spitting_toad;
mod stone_trap;
mod summoned_monster;
mod thunder_element;
mod town_archer;
mod trap_rock;
mod tucson_general;
mod vampire_spider;
mod wooma_taurus;
mod yimoogi;
mod yin_devil_node;
mod zuma_monster;
mod zuma_taurus_stage;

pub(super) use self::armadillo::*;
pub(super) use self::axe_skeleton_fear::*;
pub(super) use self::bomb_spider::*;
pub(super) use self::bone_lord::*;
pub(super) use self::cannibal_plant::*;
pub(super) use self::common::*;
pub(super) use self::deer_run_away::*;
pub(super) use self::dig_out_zombie::*;
pub(super) use self::dragon_statue_sleep::*;
pub(super) use self::evil_centipede::*;
pub(super) use self::foxman_fear::*;
pub(super) use self::frost_tiger_sitting::*;
pub(super) use self::general_meow_meow::*;
pub(super) use self::great_fox_spirit::*;
pub(super) use self::hell_bomb::*;
pub(super) use self::hell_lord::*;
pub(super) use self::holy_deva_fear::*;
pub(super) use self::horned_archer::*;
pub(super) use self::horned_commander::*;
pub(super) use self::horned_mage::*;
pub(super) use self::horned_sorceror::*;
pub(super) use self::horned_warrior::*;
pub(super) use self::kirin_ice_thrust::*;
pub(super) use self::reviving_zombie::*;
pub(super) use self::shinsu::*;
pub(super) use self::snake_totem::*;
pub(super) use self::snow_wolf_king::*;
pub(super) use self::spitting_toad::*;
pub(super) use self::stone_trap::*;
pub(super) use self::summoned_monster::*;
pub(super) use self::thunder_element::*;
pub(super) use self::town_archer::*;
pub(super) use self::trap_rock::*;
pub(super) use self::tucson_general::*;
pub(super) use self::vampire_spider::*;
pub(super) use self::wooma_taurus::*;
pub(super) use self::yimoogi::*;
pub(super) use self::yin_devil_node::*;
pub(super) use self::zuma_monster::*;
pub(super) use self::zuma_taurus_stage::*;

use bevy_ecs::{entity::Entity, prelude::World};
use mir2_game_data::CrystalRespawnTemplate;
use mir2_protocol::{MirDirection, ObjectMovement, Point, ServerPacket};

use super::buffs::{
    tick_buffs, tick_crystal_normal_hero_potion_restore, tick_crystal_normal_potion_restore,
};
use super::combat::*;
use super::components::{
    current_player_is_dead, entity_object_id, entity_position, player_entity, DisplayName, Facing,
    Monster, MonsterAgent, MonsterAiState, MonsterVitals, Position, SummonedMonster,
};
use super::crystal_compat::*;
use super::drops::tick_ground_drop_expiry;
use super::fishing::tick_fishing;
use super::hero_ai::tick_stage5_hero_combat_ai;
use super::inventory::sync_expired_expanded_storage;
use super::monsters::*;
use super::movement::*;
use super::npc::process_crystal_npc_goods_expiry;
use super::packets::*;
use super::rental::{process_expired_rental_items, return_rented_items_on_player_death};
use super::resources::{
    advance_runtime_tick, crystal_packet_move_delay_ticks, current_language, is_in_world,
    mark_crystal_packet_action, take_crystal_movement_retry_if_ready, PlayerActionKind,
};
use super::session::SimulationSession;
use super::skills::tick_ground_spell_actions;

pub(super) fn update_special_monster_state(
    world: &mut World,
    entity: Entity,
    agent: &mut MonsterAgent,
    ai_state: &mut MonsterAiState,
    position: &Point,
    player_position: &Point,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    if update_summoned_monster_state(world, entity, agent, position, tick, packets) {
        return true;
    }

    if agent.ai == 2
        && update_deer_run_away_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 50
        && update_great_fox_spirit_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 17 {
        update_zuma_taurus_stage_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
        );
    }

    if agent.ai == 49
        && update_thunder_element_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 36
        && update_yimoogi_state(
            world,
            entity,
            agent,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 123 {
        update_general_meow_meow_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        );
    }

    if agent.ai == 54
        && update_dragon_statue_sleep_state(world, entity, agent, ai_state, tick, packets)
    {
        return true;
    }

    if agent.ai == 34
        && update_frost_tiger_sitting_state(world, entity, agent, ai_state, position, tick, packets)
    {
        return true;
    }

    if agent.ai == 8
        && update_axe_skeleton_fear_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if matches!(agent.ai, 45 | 46)
        && update_foxman_fear_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 38
        && update_holy_deva_fear_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    if agent.ai == 186
        && update_kirin_ice_thrust_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    // Crystal HornedMage (AI 163): AxeSkeleton fear/kite + MC splash / ranged DC / teleport.
    if agent.ai == 163
        && update_horned_mage_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    // Crystal HornedArcher (AI 164): ally-buff ProcessTarget (ranged DC is the default path).
    if agent.ai == 164
        && update_horned_archer_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    // Crystal HornedSorceror (AI 169): stomp/tornado/thrust ProcessAI state machine.
    if agent.ai == 169
        && update_horned_sorceror_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        )
    {
        return true;
    }

    match agent.ai {
        14 => update_evil_centipede_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        5 => update_cannibal_plant_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        11 => update_wooma_taurus_state(world, entity, agent, position, tick, packets),
        40 => update_bomb_spider_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        47 => update_trap_rock_state(
            world,
            entity,
            agent,
            ai_state,
            player_position,
            tick,
            packets,
        ),
        15 | 16 | 17 | 173 | 174 => update_zuma_monster_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        18 => update_shinsu_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        24 => update_dig_out_zombie_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        124 | 125 => update_armadillo_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        25 => update_reviving_zombie_state(world, entity, agent, ai_state, tick, packets),
        30 => update_bone_lord_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        // Crystal `MonsterObject.GetMonster` cases 41 + 42 both return
        // `new YinDevilNode(info)` — immobile passive node.
        41 | 42 => update_yin_devil_node_state(agent, tick),
        60 => update_vampire_spider_state(world, entity, agent, position, tick, packets),
        61 => update_spitting_toad_state(world, entity, agent, position, tick, packets),
        62 => update_snake_totem_state(world, entity, agent, position, tick, packets),
        131 => update_tucson_general_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        57 => update_town_archer_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        98 => update_hell_lord_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        99 => update_hell_bomb_state(world, entity, agent, ai_state, position, tick, packets),
        255 => update_stone_trap_state(world, entity, agent, position, tick),
        // Crystal HornedWarrior (AI 165): shield phase + DC melee / wide-line splash.
        165 => update_horned_warrior_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        // Crystal HornedCommander (AI 171): multi-phase boss (summons, shield, AoE).
        171 => update_horned_commander_state(
            world,
            entity,
            agent,
            ai_state,
            position,
            player_position,
            tick,
            packets,
        ),
        _ => false,
    }
}

pub(super) fn advance_world(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }

    let tick = advance_runtime_tick(world);
    process_crystal_npc_goods_expiry(world);
    let mut packets = Vec::new();
    process_expired_rental_items(world, &mut packets);
    if current_player_is_dead(world) {
        return_rented_items_on_player_death(world, &mut packets);
    }
    tick_buffs(world, &mut packets);
    sync_expired_expanded_storage(world, &mut packets);
    tick_crystal_normal_potion_restore(world, &mut packets);
    tick_crystal_normal_hero_potion_restore(world, &mut packets);
    tick_stage5_hero_auto_pot(world, tick, &mut packets);
    tick_stage5_hero_combat_ai(world, tick, &mut packets);
    tick_ground_drop_expiry(world, tick);
    tick_stage5_intelligent_creatures(world, tick, &mut packets);
    tick_fishing(world, &mut packets);
    super::door::tick_doors(world, &mut packets);
    super::hazard::tick_map_hazards(world, tick, &mut packets);
    resolve_pending_combat_actions(world, tick, &mut packets);
    tick_player_status_effects(world, tick, &mut packets);
    tick_player_vital_regen(world, tick, &mut packets);
    tick_ground_spell_actions(world, tick, &mut packets);
    tick_monster_poisons(world, tick, &mut packets);
    emit_due_trainer_average_chats(world, tick, &mut packets);
    resolve_pending_monster_spawns(world, tick);
    // On-demand monster pool (CrystalWorld): materialise monsters as the player
    // approaches and despawn ones left far behind, so the AI pass below only
    // iterates the live set around the player rather than the whole map roster.
    super::map::reconcile_monster_activation(world);
    let revived_entities = tick_respawns(world);
    let player = player_entity(world).expect("player should exist");
    let player_position = entity_position(world, player).expect("player position");

    let monster_entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<Monster>>()
        .iter(world)
        .collect();

    for &entity in &monster_entities {
        let (original_agent, original_ai_state, monster_name, position, current_direction) = {
            let view = world.entity(entity);
            let agent = view.get::<MonsterAgent>().expect("monster agent").clone();
            (
                agent,
                view.get::<MonsterAiState>().copied().unwrap_or_default(),
                view.get::<DisplayName>()
                    .expect("monster name")
                    .resolve(current_language(world)),
                view.get::<Position>().expect("monster position").0.clone(),
                view.get::<Facing>().expect("monster facing").0,
            )
        };
        let mut agent = original_agent.clone();
        let mut ai_state = original_ai_state;

        if update_special_monster_state(
            world,
            entity,
            &mut agent,
            &mut ai_state,
            &position,
            &player_position,
            tick,
            &mut packets,
        ) {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        if agent.dead {
            continue;
        }

        let monster_target = if monster_prefers_monster_target(world, entity, &agent) {
            summoned_monster_entity_target(world, &monster_entities, entity, &position, &agent)
        } else {
            hostile_monster_summon_target(world, &monster_entities, entity, &position, &agent)
        };

        if let Some(target_entity) = monster_target.or_else(|| {
            find_guard_target_monster(world, &monster_entities, entity, &position, &agent)
        }) {
            let target_position =
                entity_position(world, target_entity).expect("guard target position");

            if monster_can_attack(&agent, &ai_state)
                && monster_in_attack_range(&agent, &position, &target_position)
            {
                if let Some(direction) = direction_toward(&position, &target_position) {
                    if tick >= agent.next_attack_tick {
                        agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
                        let attack_direction =
                            monster_attack_packet_direction(&agent, current_direction, direction);
                        if !matches!(agent.ai, 6 | 58) && current_direction != attack_direction {
                            world.entity_mut(entity).insert(Facing(attack_direction));
                        }
                        world.entity_mut(entity).insert(agent.clone());

                        if matches!(agent.ai, 6 | 58) {
                            if let Some(attack_packets) =
                                guard_melee_attack_packets(world, entity, target_entity)
                            {
                                packets.extend(attack_packets);
                            }
                        } else if !monster_prefers_ranged_when_not_adjacent(
                            &agent,
                            &position,
                            &target_position,
                        ) {
                            if let Some(packet) = monster_typed_attack_packet(
                                world,
                                entity,
                                &position,
                                attack_direction,
                                monster_object_attack_type(&agent, &position, &target_position),
                            ) {
                                packets.push(packet);
                            }
                        } else if let Some(packet) = monster_typed_ranged_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            target_entity,
                            &target_position,
                            monster_object_range_attack_type(&agent, &position, &target_position),
                        ) {
                            packets.push(packet);
                        }
                        if let Some(attacker_id) = entity_object_id(world, entity) {
                            let due_tick = tick
                                + monster_attack_delay_ticks(&agent, &position, &target_position);
                            let damage = summon_attack_damage(
                                &monster_name,
                                &agent,
                                tile_distance(&position, &target_position),
                            );
                            if damage > 0 {
                                schedule_damage_to_monster(
                                    world,
                                    due_tick,
                                    attacker_id,
                                    target_entity,
                                    damage,
                                    None,
                                    None,
                                );
                                if agent.ai == 18 {
                                    for line_target in shinsu_line_monster_targets(
                                        world,
                                        &monster_entities,
                                        entity,
                                        &position,
                                        direction,
                                        &agent,
                                    ) {
                                        if line_target == target_entity {
                                            continue;
                                        }
                                        schedule_damage_to_monster(
                                            world,
                                            due_tick,
                                            attacker_id,
                                            line_target,
                                            damage,
                                            None,
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    persist_monster_runtime_state(
                        world,
                        entity,
                        &original_agent,
                        &agent,
                        original_ai_state,
                        ai_state,
                    );

                    continue;
                }
            }

            if tick < agent.next_move_tick {
                persist_monster_runtime_state(
                    world,
                    entity,
                    &original_agent,
                    &agent,
                    original_ai_state,
                    ai_state,
                );
                continue;
            }

            let next = step_point_toward(&position, &target_position);
            if next != position && can_occupy(world, next.clone(), Some(entity)) {
                agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
                if let Some(direction) = direction_toward(&position, &next) {
                    world.entity_mut(entity).insert((
                        Position(next.clone()),
                        Facing(direction),
                        agent.clone(),
                    ));
                    packets.push(ServerPacket::ObjectWalk {
                        movement: ObjectMovement {
                            object_id: entity_object_id(world, entity)
                                .expect("moving monster object id"),
                            position: next,
                            direction,
                        },
                    });
                } else {
                    world
                        .entity_mut(entity)
                        .insert((Position(next), agent.clone()));
                }
                continue;
            }
        }

        let distance = tile_distance(&position, &player_position);
        if agent.tracking_player && distance > MONSTER_PLAYER_TARGET_RANGE {
            agent.tracking_player = false;
        }
        if agent.hostile_to_player && distance <= agent.view_range.max(1) {
            agent.tracking_player = true;
        }
        let player_in_aggro_range = agent.tracking_player;

        if monster_can_attack(&agent, &ai_state)
            && monster_in_attack_range(&agent, &position, &player_position)
            && player_in_aggro_range
            && !(agent.ai == 27 && distance > 1 && ai_state.next_state_tick > tick)
            && !(agent.ai == 20 && distance > 1 && ai_state.next_state_tick > tick)
            && !(agent.ai == 192 && distance > 1 && ai_state.next_state_tick > tick)
        {
            if tick >= agent.next_attack_tick {
                let Some(direction) = direction_toward(&position, &player_position) else {
                    persist_monster_runtime_state(
                        world,
                        entity,
                        &original_agent,
                        &agent,
                        original_ai_state,
                        ai_state,
                    );
                    continue;
                };

                let attack_direction =
                    monster_attack_packet_direction(&agent, current_direction, direction);
                if current_direction != attack_direction {
                    world.entity_mut(entity).insert(Facing(attack_direction));
                }

                let Some(attacker_id) = entity_object_id(world, entity) else {
                    persist_monster_runtime_state(
                        world,
                        entity,
                        &original_agent,
                        &agent,
                        original_ai_state,
                        ai_state,
                    );
                    continue;
                };

                if matches!(agent.ai, 12 | 39 | 62) {
                    let template = match agent.ai {
                        12 => bug_bat_template(),
                        39 => bomb_spider_template(),
                        62 => crystal_dynamic_monster_template("CharmedSnake"),
                        _ => None,
                    };
                    let Some(template) = template else {
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    };

                    let summon_cap = if agent.ai == 62 { 2 } else { 20 };
                    if active_summoned_monster_count(world, attacker_id) >= summon_cap {
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    }

                    agent.next_attack_tick = tick + agent.attack_interval_ticks.max(1);
                    world.entity_mut(entity).insert(agent.clone());
                    if let Some(packet) =
                        monster_melee_attack_packet(world, entity, &position, direction)
                    {
                        packets.push(packet);
                    }
                    let spawn_position = match agent.ai {
                        39 => match current_direction {
                            MirDirection::Up => offset_point(&position, MirDirection::Down, 1),
                            MirDirection::UpRight => {
                                offset_point(&position, MirDirection::DownRight, 1)
                            }
                            MirDirection::Right => {
                                offset_point(&position, MirDirection::DownLeft, 1)
                            }
                            _ => position.clone(),
                        },
                        62 => {
                            let fallback = offset_point(&position, MirDirection::Down, 1);
                            directional_destination(
                                world,
                                &position,
                                MirDirection::Down,
                                1,
                                Some(entity),
                            )
                            .unwrap_or(fallback)
                        }
                        _ => position.clone(),
                    };
                    let summon_metadata = if agent.ai == 62 {
                        Some(SummonedMonster {
                            summoner_object_id: attacker_id,
                            visible_extra: true,
                            expire_tick: Some(tick + combat_delay_ticks(10_000)),
                            require_summoner_within: Some(15),
                            despawn_tick_after_death: None,
                            totem_master_object_id: Some(attacker_id),
                            max_minions: Some(summon_cap),
                        })
                    } else {
                        None
                    };
                    queue_pending_monster_spawn(
                        world,
                        PendingMonsterSpawnAction {
                            due_tick: tick + combat_delay_ticks(500),
                            summoner_entity: entity,
                            template: CrystalRespawnTemplate {
                                location: spawn_position,
                                ..template
                            },
                            target_entity: Some(player),
                            summon_metadata,
                            hostile_to_player_override: Some(true),
                        },
                    );
                } else {
                    let armadillo_type_one_branch = matches!(agent.ai, 124 | 125) && tick % 6 == 1;
                    let armadillo_retreat_branch = matches!(agent.ai, 124 | 125) && tick % 6 == 0;
                    let hell_keeper_type_one_branch = agent.ai == 79 && tick % 3 == 1;
                    let snow_wolf_type_one_branch = agent.ai == 179 && tick % 5 == 1;
                    let frozen_miner_type_one_branch = agent.ai == 187 && tick % 8 == 1;
                    let frozen_magician_type_one_branch = agent.ai == 189
                        && tile_distance(&position, &player_position) > 1
                        && tick % 3 == 1;
                    let frozen_warewolf_hp_percent = if agent.ai == 180 {
                        world
                            .entity(entity)
                            .get::<MonsterVitals>()
                            .map(|vitals| vitals.hp * 100 / vitals.max_hp.max(1))
                            .unwrap_or(100)
                    } else {
                        100
                    };
                    let frozen_warewolf_variant_branch =
                        agent.ai == 180 && deterministic_chance_roll(tick, attacker_id, 1800, 3);
                    let frozen_warewolf_attack_type = if frozen_warewolf_variant_branch {
                        if frozen_warewolf_hp_percent >= 60 {
                            1
                        } else if frozen_warewolf_hp_percent >= 30 {
                            2
                        } else {
                            3
                        }
                    } else {
                        0
                    };
                    let lamia_kirin_type_one_branch = agent.ai == 186 && tick % 5 == 0;
                    let dark_beast_secondary_branch = agent.ai == 112 && tick % 5 == 0;
                    let seedings_general_stomp_branch =
                        agent.ai == 121 && distance > 1 && tick % 5 == 0;
                    let seedings_general_close_splash_branch =
                        agent.ai == 121 && distance == 1 && tick % 5 == 0;
                    let dark_devil_range_branch = agent.ai == 20 && distance > 1;
                    let minotaur_king_range_branch = agent.ai == 33 && distance > 1;
                    let general_meow_meow_range_branch = agent.ai == 123 && distance > 2;
                    let general_meow_meow_slam_branch =
                        agent.ai == 123 && distance <= 2 && tick % 9 == 0;
                    let manectric_claw_thrust_branch = agent.ai == 86 && distance > 1;
                    let guardian_rock_pull_movement = if agent.ai == 48 && distance > 1 {
                        direction_toward(&player_position, &position).map(|direction| {
                            PendingPlayerMovement {
                                direction,
                                distance: (distance - 1).clamp(1, 4),
                            }
                        })
                    } else {
                        None
                    };
                    let tucson_general_type_two_range_branch =
                        agent.ai == 131 && distance > 2 && tick % 4 == 0;
                    let tucson_general_stomp_branch =
                        agent.ai == 131 && distance <= 2 && tick % 4 == 0;
                    let cat_shaman_red_poison_branch =
                        agent.ai == 118 && distance > 1 && tick % 5 == 0;
                    let black_tortoise_halfmoon_branch =
                        agent.ai == 182 && distance == 1 && tick % 5 == 0;
                    let red_foxman_type_one_range_branch =
                        agent.ai == 45 && distance > 1 && tick % 2 == 0;
                    let white_foxman_slow_branch = agent.ai == 46 && distance > 1 && tick % 8 == 0;
                    let stray_cat_push_branch = agent.ai == 117 && distance == 1 && tick % 10 == 0;
                    let manectric_king_hp_percent = if agent.ai == 88 {
                        world
                            .entity(entity)
                            .get::<MonsterVitals>()
                            .map(|vitals| vitals.hp * 100 / vitals.max_hp.max(1))
                            .unwrap_or(100)
                    } else {
                        100
                    };
                    let manectric_king_mass_attack_branch = agent.ai == 88
                        && manectric_king_hp_percent < 20
                        && (ai_state.next_state_tick == 0 || tick >= ai_state.next_state_tick);
                    let manectric_king_push_line_branch = agent.ai == 88
                        && distance <= 2
                        && tick % 3 == 0
                        && !manectric_king_mass_attack_branch;
                    let great_fox_spirit_area_branch = agent.ai == 50;
                    let yimoogi_poison_branch = agent.ai == 36 && distance <= 4 && tick % 6 == 0;
                    let trap_rock_parent_paralysis_branch = agent.ai == 47
                        && world.entity(entity).get::<SummonedMonster>().is_none()
                        && deterministic_chance_roll(
                            tick,
                            attacker_id,
                            47,
                            TRAP_ROCK_ATTACK_PARALYSIS_CHANCE_DENOMINATOR,
                        );
                    let dark_wraith_line_branch = agent.ai == 192 && distance > 1;
                    // AI 4 SpittingSpider, 29 BoneSpearman, 35 unnamed line —
                    // Crystal `LineAttack(damage, 2, ...)`: 2-tile line that
                    // splashes damage along the attack direction.
                    let spitting_spider_line_branch = matches!(agent.ai, 4 | 29 | 35);
                    // AI 44 BlackFoxman branches: adjacent → base Attack() 2/3,
                    // otherwise `Broadcast(ObjectAttack Type=1)` +
                    // `LineAttack(damage, 2, 250)`. Splash only on the range
                    // branch (distance > 1) to match Crystal.
                    let black_foxman_line_branch = agent.ai == 44 && distance > 1;
                    // AI 116 BlackHammerCat is BlackFoxman-shaped: adjacent +
                    // 2/3 → DC melee (Type 0). Otherwise → Type 1 MC magic +
                    // `LineAttack(damage, 2, 300)` (DC). Splash on range only.
                    let black_hammer_cat_line_branch = agent.ai == 116 && distance > 1;
                    // AI 26 ShamanZombie: always emits `ObjectRangeAttack` +
                    // `LineAttack(damage, 6, 300, MACAgility)` — a 6-tile line.
                    let shaman_zombie_line_branch = agent.ai == 26;
                    let crystal_spider_line_branch = agent.ai == 37 && distance > 1;
                    let king_scorpion_line_targets = if agent.ai == 19 {
                        forward_line_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                            2,
                        )
                    } else {
                        Vec::new()
                    };
                    let king_scorpion_second_tile = offset_point(&position, attack_direction, 2);
                    let king_scorpion_forced_range_branch = agent.ai == 19
                        && king_scorpion_line_targets.iter().any(|target| {
                            entity_position(world, *target)
                                .map(|target_position| target_position == king_scorpion_second_tile)
                                .unwrap_or(false)
                        });
                    let king_scorpion_range_branch = agent.ai == 19
                        && (distance > 1 || king_scorpion_forced_range_branch || tick % 5 == 0);
                    let red_moon_evil_area_branch = agent.ai == 13;
                    let evil_centipede_area_branch = agent.ai == 14;
                    let oma_king_close_line_branch =
                        agent.ai == 43 && distance <= 2 && tick % 3 != 0;
                    let oma_king_type_one_magic_branch =
                        agent.ai == 43 && !oma_king_close_line_branch;
                    let ice_guard_fire_range_branch =
                        agent.ai == 102 && distance > 1 && tick % 3 == 0;
                    let ice_guard_ice_range_branch =
                        agent.ai == 102 && distance > 1 && !ice_guard_fire_range_branch;
                    let khazard_pull_branch = agent.ai == 27 && distance > 1;
                    let frozen_axeman_pull_branch = agent.ai == 188
                        && distance == 1
                        && tick % 3 != 0
                        && (ai_state.next_state_tick == 0 || tick >= ai_state.next_state_tick);
                    let dark_wraith_adjacent_area_targets = if agent.ai == 192 && distance == 1 {
                        nearby_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            &agent,
                            1,
                        )
                    } else {
                        Vec::new()
                    };
                    let dark_wraith_adjacent_area_branch =
                        !dark_wraith_adjacent_area_targets.is_empty() && tick % 2 == 1;
                    let snow_yeti_adjacent_double_hit_branch = agent.ai == 190 && distance == 1;
                    let turtle_grass_single_push_branch = agent.ai == 173 && tick % 4 == 0;
                    let cannibal_tentacles_halfmoon_branch =
                        agent.ai == 130 && distance == 1 && tick % 5 == 0;
                    let jar2_adjacent_melee_branch =
                        agent.ai == 120 && distance == 1 && tick % 3 == 0;
                    let jar2_adjacent_range_branch =
                        agent.ai == 120 && distance == 1 && !jar2_adjacent_melee_branch;
                    let sand_snail_halfmoon_branch = agent.ai == 115 && tick % 7 == 0;
                    let sand_snail_green_area_branch =
                        agent.ai == 115 && !sand_snail_halfmoon_branch && tick % 2 == 0;
                    let man_tree_boulder_branch = agent.ai == 174 && tick % 8 == 0;
                    let man_tree_halfmoon_branch =
                        agent.ai == 174 && !man_tree_boulder_branch && tick % 4 == 0;
                    let restless_jar_hp_percent = if agent.ai == 122 {
                        world
                            .entity(entity)
                            .get::<MonsterVitals>()
                            .map(|vitals| vitals.hp * 100 / vitals.max_hp.max(1))
                            .unwrap_or(100)
                    } else {
                        100
                    };
                    let restless_jar_special_adjacent_branch =
                        agent.ai == 122 && distance == 1 && tick % 3 == 2;
                    let restless_jar_tornado_branch =
                        restless_jar_special_adjacent_branch && restless_jar_hp_percent >= 50;
                    let restless_jar_stomp_branch =
                        restless_jar_special_adjacent_branch && restless_jar_hp_percent < 50;
                    let restless_jar_spin_branch = agent.ai == 122
                        && distance == 1
                        && !restless_jar_tornado_branch
                        && !restless_jar_stomp_branch;
                    let tucson_warrior_adjacent_smash_branch =
                        agent.ai == 127 && distance == 1 && tick % 5 == 0;
                    let tucson_warrior_adjacent_halfmoon_branch =
                        agent.ai == 127 && distance == 1 && !tucson_warrior_adjacent_smash_branch;
                    let tucson_warrior_smash_branch =
                        agent.ai == 127 && (distance > 1 || tucson_warrior_adjacent_smash_branch);
                    let tucson_mage_wide_line_branch =
                        agent.ai == 126 && (distance > 1 || tick % 3 == 0);
                    let cannibal_tentacles_halfmoon_targets = if cannibal_tentacles_halfmoon_branch
                    {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let sand_snail_halfmoon_targets = if sand_snail_halfmoon_branch {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let man_tree_halfmoon_targets = if man_tree_halfmoon_branch {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let tucson_warrior_halfmoon_targets = if tucson_warrior_adjacent_halfmoon_branch
                    {
                        halfmoon_opposing_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        )
                    } else {
                        Vec::new()
                    };
                    let manectric_claw_thrust_move_branch = manectric_claw_thrust_branch
                        && deterministic_chance_roll(tick, attacker_id, 862, 2);
                    if manectric_claw_thrust_move_branch {
                        agent.next_attack_tick = tick + combat_delay_ticks(300);
                        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
                        if let Some((destination, direction)) = monster_step_toward_with_fallback(
                            world,
                            entity,
                            &position,
                            &player_position,
                            tick,
                            86,
                        ) {
                            world.entity_mut(entity).insert((
                                Position(destination.clone()),
                                Facing(direction),
                                agent.clone(),
                            ));
                            packets.push(ServerPacket::ObjectWalk {
                                movement: ObjectMovement {
                                    object_id: attacker_id,
                                    position: destination,
                                    direction,
                                },
                            });
                        } else {
                            world.entity_mut(entity).insert(agent.clone());
                        }
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    }
                    let final_damage = if hell_keeper_type_one_branch {
                        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
                        if base_damage <= 0 {
                            0
                        } else {
                            let mitigation = crystal_player_rolled_armour(world);
                            (base_damage - mitigation).max(1)
                        }
                    } else if yimoogi_poison_branch {
                        0
                    } else if dark_beast_secondary_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if snow_wolf_type_one_branch {
                        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
                        if base_damage <= 0 {
                            0
                        } else {
                            let mitigation = crystal_player_rolled_armour(world);
                            (base_damage - mitigation).max(1)
                        }
                    } else if frozen_miner_type_one_branch {
                        let damage = crystal_monster_raw_attack_damage(&monster_name);
                        if damage > 0 {
                            (damage * 8 / 10).max(1)
                        } else {
                            0
                        }
                    } else if frozen_magician_type_one_branch {
                        crystal_monster_raw_magic_damage(&monster_name) * 3 / 2
                    } else if armadillo_type_one_branch && agent.ai == 124 {
                        (crystal_monster_attack_damage(&monster_name) / 2).max(1)
                    } else if armadillo_type_one_branch && agent.ai == 125 {
                        0
                    } else if cannibal_tentacles_halfmoon_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (CANNIBAL_TENTACLES_HALFMOON_DAMAGE - mitigation).max(1)
                    } else if king_scorpion_range_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else if jar2_adjacent_range_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if sand_snail_green_area_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else if seedings_general_close_splash_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (crystal_monster_magic_damage(&monster_name) - mitigation).max(1)
                    } else if man_tree_boulder_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if restless_jar_stomp_branch {
                        crystal_monster_raw_attack_damage(&monster_name)
                    } else if tucson_warrior_adjacent_smash_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else if general_meow_meow_slam_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (crystal_monster_attack_damage(&monster_name) * 3 - mitigation).max(1)
                    } else if tucson_general_type_two_range_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (crystal_monster_spell_damage(&monster_name) * 2 - mitigation).max(1)
                    } else if tucson_general_stomp_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (crystal_monster_magic_damage(&monster_name) - mitigation).max(1)
                    } else if white_foxman_slow_branch {
                        0
                    } else if stray_cat_push_branch {
                        crystal_monster_raw_magic_damage(&monster_name)
                    } else if tucson_mage_wide_line_branch {
                        let base_damage = crystal_monster_raw_magic_damage(&monster_name);
                        if base_damage <= 0 {
                            0
                        } else {
                            let mitigation = crystal_player_rolled_armour(world);
                            (base_damage - mitigation).max(1)
                        }
                    } else if manectric_king_mass_attack_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (crystal_monster_magic_damage(&monster_name) - mitigation).max(1)
                    } else if manectric_king_push_line_branch {
                        let mitigation = crystal_player_rolled_armour(world);
                        (crystal_monster_attack_damage(&monster_name) - mitigation).max(1)
                    } else if oma_king_type_one_magic_branch {
                        crystal_monster_magic_damage(&monster_name)
                    } else {
                        monster_player_attack_damage(
                            world,
                            &monster_name,
                            &agent,
                            &position,
                            &player_position,
                        )
                    };
                    let attack_type = if armadillo_type_one_branch
                        || hell_keeper_type_one_branch
                        || snow_wolf_type_one_branch
                        || frozen_miner_type_one_branch
                        || frozen_magician_type_one_branch
                        || lamia_kirin_type_one_branch
                        || dark_beast_secondary_branch
                        || yimoogi_poison_branch
                        || seedings_general_close_splash_branch
                        || turtle_grass_single_push_branch
                        || cannibal_tentacles_halfmoon_branch
                        || sand_snail_halfmoon_branch
                        || man_tree_halfmoon_branch
                        || restless_jar_tornado_branch
                        || tucson_warrior_adjacent_smash_branch
                        || general_meow_meow_slam_branch
                        || tucson_general_stomp_branch
                        || black_tortoise_halfmoon_branch
                        || stray_cat_push_branch
                        || tucson_mage_wide_line_branch
                        || manectric_king_push_line_branch
                        || oma_king_type_one_magic_branch
                    {
                        1
                    } else if dark_wraith_adjacent_area_branch {
                        1
                    } else if frozen_axeman_pull_branch
                        || sand_snail_green_area_branch
                        || man_tree_boulder_branch
                        || restless_jar_stomp_branch
                    {
                        2
                    } else if agent.ai == 180 {
                        frozen_warewolf_attack_type
                    } else {
                        monster_object_attack_type(&agent, &position, &player_position)
                    };
                    let yimoogi_ranged_attack_pause =
                        if agent.ai == 36 && (distance > 2 || yimoogi_poison_branch) {
                            combat_delay_ticks(500)
                        } else {
                            0
                        };
                    agent.next_attack_tick =
                        tick + agent.attack_interval_ticks.max(1) + yimoogi_ranged_attack_pause;
                    if frozen_axeman_pull_branch {
                        ai_state.next_state_tick = tick + combat_delay_ticks(10_000);
                    }
                    if khazard_pull_branch {
                        ai_state.next_state_tick = tick + combat_delay_ticks(5_000);
                    }
                    if dark_devil_range_branch {
                        ai_state.next_state_tick =
                            tick + combat_delay_ticks(2_000 + (tick % 3) * 1_000);
                    }
                    if dark_wraith_line_branch {
                        ai_state.next_state_tick =
                            tick + combat_delay_ticks(3_000 + (tick % 5) * 1_000);
                    }
                    if manectric_king_mass_attack_branch {
                        ai_state.next_state_tick =
                            tick + combat_delay_ticks(2_000 + (tick % 5) * 1_000);
                    }
                    if agent.ai == 180 && frozen_warewolf_hp_percent < 70 && !ai_state.mode {
                        ai_state.mode = true;
                        spawn_snow_wolf_king_slaves(
                            world,
                            attacker_id,
                            &position,
                            attack_direction,
                            player,
                            &player_position,
                            &agent,
                        );
                    }
                    if armadillo_retreat_branch {
                        let retreat_position = offset_point(&position, attack_direction, -2);
                        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
                        if agent.ai == 125 {
                            ai_state.mode = true;
                        }
                        world.entity_mut(entity).insert((
                            Position(retreat_position.clone()),
                            Facing(attack_direction),
                            agent.clone(),
                        ));
                        packets.push(ServerPacket::ObjectBackStep {
                            movement: ObjectMovement {
                                object_id: attacker_id,
                                position: retreat_position.clone(),
                                direction: attack_direction,
                            },
                            distance: 2,
                        });
                        if agent.ai == 124 {
                            let retreat_damage = crystal_monster_raw_attack_damage(&monster_name);
                            if retreat_damage > 0 {
                                let due_tick = tick + combat_delay_ticks(900);
                                if tile_distance(&retreat_position, &player_position) <= 2 {
                                    schedule_damage_to_player(
                                        world,
                                        due_tick,
                                        attacker_id,
                                        monster_name.clone(),
                                        retreat_damage,
                                    );
                                }
                                for area_target in nearby_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &retreat_position,
                                    &agent,
                                    2,
                                ) {
                                    if monster_ignores_damage(world, area_target) {
                                        ai_state.mode = true;
                                    }
                                    schedule_damage_to_monster(
                                        world,
                                        due_tick,
                                        attacker_id,
                                        area_target,
                                        retreat_damage,
                                        None,
                                        None,
                                    );
                                }
                            }
                        }
                        persist_monster_runtime_state(
                            world,
                            entity,
                            &original_agent,
                            &agent,
                            original_ai_state,
                            ai_state,
                        );
                        continue;
                    }
                    world.entity_mut(entity).insert(agent.clone());
                    let trap_rock_child =
                        agent.ai == 47 && world.entity(entity).get::<SummonedMonster>().is_some();
                    let attack_packet = if snow_yeti_adjacent_double_hit_branch {
                        if let Some(packet) = monster_typed_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            0,
                        ) {
                            packets.push(packet);
                        }
                        if let Some(packet) = monster_typed_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            1,
                        ) {
                            packets.push(packet);
                        }
                        None
                    } else if !trap_rock_child
                        && !yimoogi_poison_branch
                        && (king_scorpion_range_branch
                            || restless_jar_tornado_branch
                            || jar2_adjacent_range_branch
                            || manectric_king_mass_attack_branch
                            || monster_prefers_ranged_when_not_adjacent(
                                &agent,
                                &position,
                                &player_position,
                            ))
                    {
                        monster_typed_ranged_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            player,
                            &player_position,
                            if frozen_magician_type_one_branch
                                || ice_guard_fire_range_branch
                                || seedings_general_stomp_branch
                                || restless_jar_tornado_branch
                            {
                                1
                            } else if cat_shaman_red_poison_branch {
                                1
                            } else if tucson_general_type_two_range_branch {
                                2
                            } else if red_foxman_type_one_range_branch {
                                1
                            } else if white_foxman_slow_branch {
                                1
                            } else {
                                monster_object_range_attack_type(
                                    &agent,
                                    &position,
                                    &player_position,
                                )
                            },
                        )
                    } else {
                        monster_typed_attack_packet(
                            world,
                            entity,
                            &position,
                            attack_direction,
                            attack_type,
                        )
                    };
                    let due_packet = if monster_broadcasts_attack_on_damage_due(&agent) {
                        attack_packet
                    } else {
                        if let Some(packet) = attack_packet {
                            packets.push(packet);
                        }
                        None
                    };
                    if white_foxman_slow_branch {
                        schedule_player_status_effect(
                            world,
                            tick + combat_delay_ticks(300),
                            attacker_id,
                            PendingPlayerStatusEffect::WhiteFoxmanSlow {
                                duration_ticks: WHITE_FOXMAN_SLOW_DURATION_TICKS,
                            },
                        );
                    }
                    if stray_cat_push_branch {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            1,
                            &mut packets,
                        );
                    }
                    if manectric_king_push_line_branch {
                        let line_distance = (4 - distance).max(1);
                        let push_distance = (line_distance - 1).max(1);
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            push_distance,
                            &mut packets,
                        );
                    }
                    if khazard_pull_branch {
                        if let Some(pull_direction) = direction_toward(&player_position, &position)
                        {
                            let _ = push_player_in_direction(
                                world,
                                player,
                                pull_direction,
                                distance,
                                &mut packets,
                            );
                        }
                    }
                    if turtle_grass_single_push_branch {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            3,
                            &mut packets,
                        );
                    }
                    if armadillo_type_one_branch && agent.ai == 125 {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            2,
                            &mut packets,
                        );
                    }
                    if yimoogi_poison_branch {
                        apply_player_red_poison(world, tick, YIMOOGI_RED_POISON_DURATION_TICKS);
                    }
                    if trap_rock_parent_paralysis_branch {
                        apply_player_paralysis(world, tick, TRAP_ROCK_PARALYSIS_DURATION_TICKS);
                    }
                    if restless_jar_stomp_branch {
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            1,
                            &mut packets,
                        );
                    }
                    if snow_yeti_adjacent_double_hit_branch && final_damage > 0 {
                        for delay_ms in [500, 1_500] {
                            schedule_damage_to_player(
                                world,
                                tick + combat_delay_ticks(delay_ms),
                                attacker_id,
                                monster_name.clone(),
                                final_damage,
                            );
                        }
                    } else if dark_wraith_adjacent_area_branch && final_damage > 0 {
                        let due_tick = tick + combat_delay_ticks(600);
                        schedule_damage_to_player(
                            world,
                            due_tick,
                            attacker_id,
                            monster_name.clone(),
                            final_damage,
                        );
                        for area_target in dark_wraith_adjacent_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                    } else if frozen_axeman_pull_branch && final_damage > 0 {
                        let push_distance = 2 + i32::try_from(tick % 3).unwrap_or(0);
                        let _ = push_player_in_direction(
                            world,
                            player,
                            attack_direction,
                            push_distance,
                            &mut packets,
                        );
                        schedule_damage_to_player(
                            world,
                            tick + combat_delay_ticks(500),
                            attacker_id,
                            monster_name.clone(),
                            final_damage,
                        );
                    } else if armadillo_type_one_branch && agent.ai == 124 && final_damage > 0 {
                        for delay_ms in [400, 600, 800] {
                            schedule_damage_to_player(
                                world,
                                tick + combat_delay_ticks(delay_ms),
                                attacker_id,
                                monster_name.clone(),
                                final_damage,
                            );
                        }
                    } else if oma_king_close_line_branch && final_damage > 0 {
                        let pushed_player = if distance <= 1 {
                            push_player_in_direction(
                                world,
                                player,
                                attack_direction,
                                3 + i32::try_from(tick % 3).unwrap_or(0),
                                &mut packets,
                            )
                            .is_some()
                        } else {
                            false
                        };
                        if pushed_player && tick % 8 == 0 {
                            apply_player_paralysis(
                                world,
                                tick,
                                CAVE_MAGGOT_PARALYSIS_DURATION_TICKS,
                            );
                        }

                        let due_tick = tick + combat_delay_ticks(300);
                        let line_tiles = [
                            offset_point(&position, attack_direction, 1),
                            offset_point(&position, attack_direction, 2),
                        ];
                        if entity_position(world, player)
                            .map(|point| line_tiles.contains(&point))
                            .unwrap_or(false)
                        {
                            schedule_damage_to_player(
                                world,
                                due_tick,
                                attacker_id,
                                monster_name.clone(),
                                final_damage,
                            );
                        }
                        for line_target in shinsu_line_monster_targets(
                            world,
                            &monster_entities,
                            entity,
                            &position,
                            attack_direction,
                            &agent,
                        ) {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                    } else if agent.ai == 48 {
                        schedule_damage_to_player_with_effect_due_packet_and_movement(
                            world,
                            tick + monster_attack_delay_ticks(&agent, &position, &player_position),
                            attacker_id,
                            monster_name.clone(),
                            0,
                            None,
                            due_packet,
                            guardian_rock_pull_movement,
                        );
                    } else if final_damage > 0 {
                        let due_tick = if frozen_miner_type_one_branch {
                            tick + combat_delay_ticks(1_000)
                        } else if frozen_magician_type_one_branch {
                            let distance =
                                u64::try_from(tile_distance(&position, &player_position).max(0))
                                    .expect("tile distance should fit u64");
                            tick + combat_delay_ticks(distance * 50 + 750)
                        } else if lamia_kirin_type_one_branch {
                            tick + combat_delay_ticks(500)
                        } else if snow_wolf_type_one_branch {
                            tick + combat_delay_ticks(450)
                        } else if turtle_grass_single_push_branch {
                            tick + combat_delay_ticks(500)
                        } else if cannibal_tentacles_halfmoon_branch {
                            tick + combat_delay_ticks(500)
                        } else if jar2_adjacent_range_branch {
                            tick + combat_delay_ticks(500)
                        } else if oma_king_type_one_magic_branch {
                            tick + combat_delay_ticks(500)
                        } else if manectric_king_mass_attack_branch {
                            tick + manectric_king_mass_attack_delay_ticks(
                                &position,
                                &player_position,
                            )
                        } else if tucson_general_type_two_range_branch {
                            tick + combat_delay_ticks(500)
                        } else {
                            tick + monster_attack_delay_ticks(&agent, &position, &player_position)
                        };
                        let area_monster_targets = if frozen_miner_type_one_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let seedings_general_area_targets = if seedings_general_stomp_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let sand_snail_green_area_targets = if sand_snail_green_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let man_tree_boulder_area_targets = if man_tree_boulder_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let tucson_warrior_smash_targets = if tucson_warrior_smash_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let tucson_mage_wide_line_targets =
                            if tucson_mage_wide_line_branch && final_damage > 0 {
                                tucson_mage_wide_line_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &position,
                                    attack_direction,
                                    &agent,
                                )
                            } else {
                                Vec::new()
                            };
                        let hell_keeper_area_targets = if agent.ai == 79 && final_damage > 0 {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                agent.view_range.max(1),
                            )
                        } else {
                            Vec::new()
                        };
                        let snow_wolf_area_targets = if agent.ai == 179 && final_damage > 0 {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let tucson_general_stomp_targets = if tucson_general_stomp_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                3,
                            )
                        } else {
                            Vec::new()
                        };
                        let black_tortoise_halfmoon_targets = if black_tortoise_halfmoon_branch {
                            halfmoon_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                            )
                        } else {
                            Vec::new()
                        };
                        let restless_jar_area_targets =
                            if restless_jar_spin_branch || restless_jar_stomp_branch {
                                nearby_opposing_monster_targets(
                                    world,
                                    &monster_entities,
                                    entity,
                                    &position,
                                    &agent,
                                    1,
                                )
                            } else {
                                Vec::new()
                            };
                        let minotaur_king_area_targets = if minotaur_king_range_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                3,
                            )
                        } else {
                            Vec::new()
                        };
                        let general_meow_meow_area_targets = if general_meow_meow_range_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let mir_statue_area_targets = if agent.ai == 54 {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &player_position,
                                &agent,
                                2,
                            )
                        } else {
                            Vec::new()
                        };
                        let manectric_king_mass_targets = if manectric_king_mass_attack_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                7,
                            )
                        } else {
                            Vec::new()
                        };
                        let manectric_claw_cone_targets = if manectric_claw_thrust_branch {
                            manectric_claw_cone_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                            )
                        } else {
                            Vec::new()
                        };
                        let dark_devil_area_targets = if dark_devil_range_branch {
                            let area_center = offset_point(&position, attack_direction, 2);
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &area_center,
                                &agent,
                                1,
                            )
                        } else {
                            Vec::new()
                        };
                        let dark_wraith_line_targets = if dark_wraith_line_branch {
                            forward_line_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                                4,
                            )
                        } else {
                            Vec::new()
                        };
                        let spider_line_targets = if spitting_spider_line_branch
                            || crystal_spider_line_branch
                            || black_foxman_line_branch
                            || black_hammer_cat_line_branch
                            || shaman_zombie_line_branch
                        {
                            let line_distance = if shaman_zombie_line_branch {
                                6
                            } else if crystal_spider_line_branch {
                                3
                            } else {
                                2
                            };
                            forward_line_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                attack_direction,
                                &agent,
                                line_distance,
                            )
                        } else {
                            Vec::new()
                        };
                        let red_moon_evil_area_targets = if red_moon_evil_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                agent.view_range.max(1),
                            )
                        } else {
                            Vec::new()
                        };
                        let great_fox_spirit_area_targets = if great_fox_spirit_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                if distance > 2 { 7 } else { 2 },
                            )
                        } else {
                            Vec::new()
                        };
                        let evil_centipede_area_targets = if evil_centipede_area_branch {
                            nearby_opposing_monster_targets(
                                world,
                                &monster_entities,
                                entity,
                                &position,
                                &agent,
                                7,
                            )
                        } else {
                            Vec::new()
                        };
                        if red_moon_evil_area_branch {
                            if let Some(packet) =
                                object_effect_packet(world, player, SPELL_EFFECT_RED_MOON_EVIL)
                            {
                                packets.push(packet);
                            }
                            for area_target in &red_moon_evil_area_targets {
                                if let Some(packet) = object_effect_packet(
                                    world,
                                    *area_target,
                                    SPELL_EFFECT_RED_MOON_EVIL,
                                ) {
                                    packets.push(packet);
                                }
                            }
                        }
                        if great_fox_spirit_area_branch && distance > 2 {
                            if let Some(packet) =
                                object_effect_packet(world, player, SPELL_EFFECT_GREAT_FOX_SPIRIT)
                            {
                                packets.push(packet);
                            }
                            for area_target in &great_fox_spirit_area_targets {
                                if let Some(packet) = object_effect_packet(
                                    world,
                                    *area_target,
                                    SPELL_EFFECT_GREAT_FOX_SPIRIT,
                                ) {
                                    packets.push(packet);
                                }
                            }
                        }
                        schedule_damage_to_player_with_effect_due_packet_and_movement(
                            world,
                            due_tick,
                            attacker_id,
                            monster_name.clone(),
                            final_damage,
                            if hell_keeper_type_one_branch && final_damage > 0 {
                                Some(PendingPlayerStatusEffect::Dazed {
                                    duration_ticks: HELL_KEEPER_DAZED_DURATION_TICKS,
                                })
                            } else if dark_beast_secondary_branch
                                && crystal_monster_effect_for_name(&monster_name) == 1
                            {
                                Some(PendingPlayerStatusEffect::BleedingPoison {
                                    chance_denominator: 5,
                                    duration_ticks: 5,
                                    salt: 112,
                                })
                            } else if ice_guard_ice_range_branch {
                                Some(PendingPlayerStatusEffect::SlowAndFrozen {
                                    slow_chance_denominator: ICE_GUARD_SLOW_CHANCE_DENOMINATOR,
                                    slow_duration_ticks: ICE_GUARD_SLOW_DURATION_TICKS,
                                    slow_salt: 102,
                                    frozen_chance_denominator: ICE_GUARD_FROZEN_CHANCE_DENOMINATOR,
                                    frozen_duration_ticks: ICE_GUARD_FROZEN_DURATION_TICKS,
                                    frozen_salt: 103,
                                })
                            } else if agent.ai == 86 && distance > 1 {
                                Some(PendingPlayerStatusEffect::SlowAndFrozen {
                                    slow_chance_denominator: 5,
                                    slow_duration_ticks: 4,
                                    slow_salt: 860,
                                    frozen_chance_denominator: 5,
                                    frozen_duration_ticks: 2,
                                    frozen_salt: 861,
                                })
                            } else if snow_wolf_type_one_branch && final_damage > 0 {
                                Some(PendingPlayerStatusEffect::SlowAndFrozen {
                                    slow_chance_denominator: SNOW_WOLF_SLOW_CHANCE_DENOMINATOR,
                                    slow_duration_ticks: SNOW_WOLF_SLOW_DURATION_TICKS,
                                    slow_salt: 179,
                                    frozen_chance_denominator: SNOW_WOLF_FROZEN_CHANCE_DENOMINATOR,
                                    frozen_duration_ticks: SNOW_WOLF_FROZEN_DURATION_TICKS,
                                    frozen_salt: 179,
                                })
                            } else if agent.ai == 190 && distance > 1 {
                                Some(PendingPlayerStatusEffect::FrozenPoison {
                                    chance_denominator: SNOW_YETI_FROZEN_CHANCE_DENOMINATOR,
                                    duration_ticks: SNOW_YETI_FROZEN_DURATION_TICKS,
                                    salt: 190,
                                })
                            } else if seedings_general_stomp_branch {
                                Some(PendingPlayerStatusEffect::FrozenPoison {
                                    chance_denominator: SEEDINGS_GENERAL_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: SEEDINGS_GENERAL_POISON_DURATION_TICKS,
                                    salt: 1211,
                                })
                            } else if cannibal_tentacles_halfmoon_branch {
                                Some(PendingPlayerStatusEffect::GreenPoison {
                                    chance_denominator: 1,
                                    duration_ticks: CANNIBAL_TENTACLES_GREEN_POISON_DURATION_TICKS,
                                })
                            } else if agent.ai == 120
                                && (distance > 1 || jar2_adjacent_range_branch)
                            {
                                Some(PendingPlayerStatusEffect::FrozenPoison {
                                    chance_denominator: JAR2_FROZEN_CHANCE_DENOMINATOR,
                                    duration_ticks: JAR2_FROZEN_DURATION_TICKS,
                                    salt: 120,
                                })
                            } else if sand_snail_green_area_branch {
                                Some(PendingPlayerStatusEffect::GreenPoison {
                                    chance_denominator: 1,
                                    duration_ticks: SAND_SNAIL_GREEN_POISON_DURATION_TICKS,
                                })
                            } else if man_tree_boulder_branch {
                                Some(PendingPlayerStatusEffect::StunPoison {
                                    chance_denominator: MAN_TREE_STUN_CHANCE_DENOMINATOR,
                                    duration_ticks: MAN_TREE_STUN_DURATION_TICKS,
                                    salt: 174,
                                })
                            } else if agent.ai == 50 {
                                Some(PendingPlayerStatusEffect::SlowAndParalysis {
                                    slow_chance_denominator:
                                        GREAT_FOX_SPIRIT_SLOW_CHANCE_DENOMINATOR,
                                    slow_duration_ticks: GREAT_FOX_SPIRIT_SLOW_DURATION_TICKS,
                                    slow_salt: 500,
                                    paralysis_chance_denominator:
                                        GREAT_FOX_SPIRIT_PARALYSIS_CHANCE_DENOMINATOR,
                                    paralysis_duration_ticks:
                                        GREAT_FOX_SPIRIT_PARALYSIS_DURATION_TICKS,
                                    paralysis_salt: 501,
                                })
                            } else if restless_jar_tornado_branch {
                                Some(PendingPlayerStatusEffect::BlindnessPoison {
                                    chance_denominator: RESTLESS_JAR_BLINDNESS_CHANCE_DENOMINATOR,
                                    duration_ticks: RESTLESS_JAR_BLINDNESS_DURATION_TICKS,
                                    salt: 1221,
                                })
                            } else if tucson_general_stomp_branch {
                                Some(PendingPlayerStatusEffect::Paralysis {
                                    chance_denominator: TUCSON_GENERAL_PARALYSIS_CHANCE_DENOMINATOR,
                                    duration_ticks: TUCSON_GENERAL_PARALYSIS_DURATION_TICKS,
                                })
                            } else if cat_shaman_red_poison_branch {
                                Some(PendingPlayerStatusEffect::RedPoison {
                                    chance_denominator: CAT_SHAMAN_RED_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: CAT_SHAMAN_RED_POISON_DURATION_TICKS,
                                    salt: 118,
                                })
                            } else if matches!(agent.ai, 181 | 182) && distance > 1 {
                                Some(PendingPlayerStatusEffect::GreenPoison {
                                    chance_denominator:
                                        WATER_DRAGON_GREEN_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: WATER_DRAGON_GREEN_POISON_DURATION_TICKS,
                                })
                            } else if evil_centipede_area_branch {
                                Some(PendingPlayerStatusEffect::GreenPoisonAndParalysis {
                                    green_chance_denominator:
                                        EVIL_CENTIPEDE_GREEN_POISON_CHANCE_DENOMINATOR,
                                    green_duration_ticks:
                                        EVIL_CENTIPEDE_GREEN_POISON_DURATION_TICKS,
                                    green_salt: 140,
                                    paralysis_chance_denominator:
                                        EVIL_CENTIPEDE_PARALYSIS_CHANCE_DENOMINATOR,
                                    paralysis_duration_ticks:
                                        EVIL_CENTIPEDE_PARALYSIS_DURATION_TICKS,
                                    paralysis_salt: 141,
                                })
                            } else if agent.ai == 121 && distance > 1 {
                                Some(PendingPlayerStatusEffect::SlowPoison {
                                    chance_denominator: SEEDINGS_GENERAL_POISON_CHANCE_DENOMINATOR,
                                    duration_ticks: SEEDINGS_GENERAL_POISON_DURATION_TICKS,
                                    salt: 1210,
                                })
                            } else if agent.ai == 34 && distance > 1 {
                                if crystal_monster_effect_for_name(&monster_name) == 1 {
                                    Some(PendingPlayerStatusEffect::SlowPoison {
                                        chance_denominator: FROST_TIGER_POISON_CHANCE_DENOMINATOR,
                                        duration_ticks: FROST_TIGER_POISON_DURATION_TICKS,
                                        salt: 341,
                                    })
                                } else {
                                    Some(PendingPlayerStatusEffect::BleedingPoison {
                                        chance_denominator: FROST_TIGER_POISON_CHANCE_DENOMINATOR,
                                        duration_ticks: FROST_TIGER_POISON_DURATION_TICKS,
                                        salt: 340,
                                    })
                                }
                            } else {
                                monster_player_status_effect(&agent)
                            },
                            due_packet,
                            guardian_rock_pull_movement,
                        );
                        for area_target in area_monster_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in seedings_general_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in cannibal_tentacles_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in sand_snail_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in sand_snail_green_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in man_tree_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in man_tree_boulder_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in tucson_warrior_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in tucson_warrior_smash_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in tucson_mage_wide_line_targets {
                            if let Some(target_position) = entity_position(world, line_target) {
                                schedule_damage_to_monster(
                                    world,
                                    tick + tucson_mage_wide_line_delay_ticks(
                                        &position,
                                        &target_position,
                                        attack_direction,
                                    ),
                                    attacker_id,
                                    line_target,
                                    final_damage,
                                    None,
                                    None,
                                );
                            }
                        }
                        for area_target in hell_keeper_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in snow_wolf_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in tucson_general_stomp_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in black_tortoise_halfmoon_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in king_scorpion_line_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in restless_jar_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in minotaur_king_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in general_meow_meow_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in mir_statue_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in manectric_king_mass_targets {
                            if let Some(target_position) = entity_position(world, area_target) {
                                schedule_damage_to_monster(
                                    world,
                                    tick + manectric_king_mass_attack_delay_ticks(
                                        &position,
                                        &target_position,
                                    ),
                                    attacker_id,
                                    area_target,
                                    final_damage,
                                    None,
                                    None,
                                );
                            }
                        }
                        for area_target in manectric_claw_cone_targets {
                            if let Some(target_position) = entity_position(world, area_target) {
                                let base_damage = if tile_distance(&position, &target_position) > 2
                                {
                                    crystal_monster_magic_damage(&monster_name)
                                } else {
                                    crystal_monster_attack_damage(&monster_name)
                                };
                                let mitigation = crystal_player_rolled_armour(world);
                                schedule_damage_to_monster(
                                    world,
                                    due_tick,
                                    attacker_id,
                                    area_target,
                                    (base_damage - mitigation).max(1),
                                    None,
                                    None,
                                );
                            }
                        }
                        for area_target in dark_devil_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in dark_wraith_line_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for line_target in spider_line_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                line_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in red_moon_evil_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in great_fox_spirit_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                        for area_target in evil_centipede_area_targets {
                            schedule_damage_to_monster(
                                world,
                                due_tick,
                                attacker_id,
                                area_target,
                                final_damage,
                                None,
                                None,
                            );
                        }
                    }
                }
            }
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        let route_target = if monster_can_follow_route(&agent) {
            monster_route_target(&mut agent, &position, tick)
        } else {
            None
        };
        let desired_point = if player_in_aggro_range && monster_can_chase_player(&agent) {
            player_position.clone()
        } else if let Some(route_target) = route_target {
            route_target
        } else if monster_can_patrol_origin(&agent) {
            patrol_target(&agent.patrol_origin, tick, entity.index())
        } else {
            position.clone()
        };

        if desired_point == position {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        if tick < agent.next_move_tick {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        let next = step_point_toward(&position, &desired_point);
        if next == position {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        if !can_occupy(world, next.clone(), Some(entity)) {
            persist_monster_runtime_state(
                world,
                entity,
                &original_agent,
                &agent,
                original_ai_state,
                ai_state,
            );
            continue;
        }

        agent.next_move_tick = tick + agent.move_interval_ticks.max(1);
        if let Some(direction) = direction_toward(&position, &next) {
            world.entity_mut(entity).insert((
                Position(next.clone()),
                Facing(direction),
                agent.clone(),
            ));
        } else {
            world
                .entity_mut(entity)
                .insert((Position(next.clone()), agent.clone()));
        }

        packets.push(ServerPacket::ObjectWalk {
            movement: object_movement(world, entity).expect("monster movement"),
        });
    }

    for entity in revived_entities {
        if let Some(info) = object_revived_info_for_entity(world, entity, false) {
            packets.push(ServerPacket::ObjectRevived { info });
        }
        if let Some(info) = object_health_info_for_entity(world, entity, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }

    packets
}

impl SimulationSession {
    pub fn tick(&mut self) -> Vec<ServerPacket> {
        if let Some(command) = take_crystal_movement_retry_if_ready(self.app.world_mut()) {
            mark_crystal_packet_action(
                self.app.world_mut(),
                PlayerActionKind::Move,
                crystal_packet_move_delay_ticks(command.running),
            );
            let packets = self.move_player_by_direction(command.direction, command.running);
            return self.finalize_packets(packets);
        }
        let packets = advance_world(self.app.world_mut());
        self.finalize_packets(packets)
    }
}
