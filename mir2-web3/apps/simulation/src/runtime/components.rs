use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{localized_text_or_fallback, LanguageCode};
use mir2_protocol::{MirClass, MirDirection, MirGender, Point};

use crate::config::WorldEntityDisposition;

use super::resources::SessionResource;

#[derive(Component)]
pub(super) struct WorldObject;

#[derive(Component, Clone, Copy)]
pub(super) struct ObjectId(pub(super) u32);

#[derive(Component, Clone)]
pub(super) struct DisplayName {
    pub(super) value: String,
    pub(super) localization_key: Option<String>,
}

impl DisplayName {
    pub(super) fn literal(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            localization_key: None,
        }
    }

    pub(super) fn localized(
        localization_key: impl Into<String>,
        fallback: impl Into<String>,
    ) -> Self {
        Self {
            value: fallback.into(),
            localization_key: Some(localization_key.into()),
        }
    }

    pub(super) fn resolve(&self, language: LanguageCode) -> String {
        match self.localization_key.as_deref() {
            Some(key) => localized_text_or_fallback(language, key, &self.value),
            None => self.value.clone(),
        }
    }
}

#[derive(Component, Clone)]
pub(super) struct Position(pub(super) Point);

#[derive(Component, Clone, Copy)]
pub(super) struct Facing(pub(super) MirDirection);

#[derive(Component, Clone, Copy)]
pub(super) struct CharacterBody {
    pub(super) class: MirClass,
    pub(super) gender: MirGender,
    pub(super) level: u16,
    pub(super) armour_shape: Option<u16>,
    pub(super) weapon_shape: Option<u16>,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PlayerVitals {
    pub(super) hp: i32,
    pub(super) max_hp: i32,
    pub(super) mp: i32,
}

#[derive(Component, Clone, Copy)]
pub(super) struct MonsterVitals {
    pub(super) hp: i32,
    pub(super) max_hp: i32,
}

#[derive(Component, Clone, Copy, Default)]
pub(super) struct MonsterCombatStats {
    pub(super) agility: i32,
    /// Crystal `Stats[Stat.Accuracy]` for the monster — used for the
    /// monster→player hit roll (`Random.Next(playerAgility+1) > accuracy`).
    /// Sourced from the Crystal monster manifest (currently `#[serde(default)]`
    /// → 0 until accuracy is extracted, keeping the roll inert).
    pub(super) accuracy: i32,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct MonsterPoisonState {
    pub(super) poison: u16,
    pub(super) green_damage: i32,
    pub(super) next_damage_tick: u64,
    pub(super) expires_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RouteStep {
    pub(super) location: Point,
    pub(super) delay_ticks: u64,
}

#[derive(Component, Clone, PartialEq, Eq)]
pub(super) struct MonsterAgent {
    pub(super) image: u16,
    pub(super) dead: bool,
    pub(super) patrol_origin: Point,
    pub(super) ai: u8,
    pub(super) disposition: WorldEntityDisposition,
    pub(super) hostile_to_player: bool,
    pub(super) tracking_player: bool,
    pub(super) view_range: i32,
    pub(super) can_wander: bool,
    pub(super) move_interval_ticks: u64,
    pub(super) attack_interval_ticks: u64,
    pub(super) next_move_tick: u64,
    pub(super) next_attack_tick: u64,
    pub(super) route: Vec<RouteStep>,
    pub(super) route_index: usize,
    pub(super) route_waiting: bool,
    pub(super) next_route_tick: u64,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct MonsterAiState {
    pub(super) hidden: bool,
    pub(super) extra: bool,
    pub(super) extra_byte: u8,
    pub(super) mode: bool,
    pub(super) next_state_tick: u64,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct HarvestMonsterState {
    pub(super) remaining_skin_count: u8,
    pub(super) drops_ready: bool,
    pub(super) harvested: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) struct TrainerDamageState {
    pub(super) hit_count: u32,
    pub(super) total_damage: i32,
    pub(super) start_tick: u64,
    pub(super) last_attack_tick: u64,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct WoomaTaurusState {
    pub(super) stage: u8,
    pub(super) mad_until_tick: u64,
    pub(super) next_teleport_tick: u64,
    pub(super) base_move_interval_ticks: u64,
    pub(super) base_attack_interval_ticks: u64,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct GeneralMeowMeowState {
    pub(super) shield_until_tick: u64,
    pub(super) next_thunder_tick: u64,
    pub(super) next_slave_spawn_tick: u64,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct YimoogiState {
    pub(super) spawn_tick: u64,
    pub(super) child_spawned: bool,
    pub(super) is_child: bool,
    pub(super) final_teleport: bool,
    pub(super) sister_object_id: Option<u32>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SummonedMonster {
    pub(super) summoner_object_id: u32,
    pub(super) visible_extra: bool,
    pub(super) expire_tick: Option<u64>,
    pub(super) require_summoner_within: Option<i32>,
    pub(super) despawn_tick_after_death: Option<u64>,
    pub(super) totem_master_object_id: Option<u32>,
    pub(super) max_minions: Option<usize>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct NpcPetState {
    pub(super) pet_level: u8,
}

#[derive(Component, Clone, Copy)]
pub(super) struct SpawnSlotRef {
    pub(super) rule_index: usize,
    pub(super) slot_index: usize,
}

#[derive(Component, Clone)]
pub(super) struct NpcAgent {
    pub(super) image: u16,
    pub(super) colour_argb: i32,
    pub(super) quest_ids: Vec<i32>,
    pub(super) script_key: Option<String>,
}

#[derive(Component)]
pub(super) struct SelfPlayer;

#[derive(Component)]
pub(super) struct RemotePlayer;

#[derive(Component, Clone)]
pub(super) struct Hero {
    pub(super) owner_name: String,
    pub(super) next_attack_tick: u64,
    pub(super) next_move_tick: u64,
}

#[derive(Component)]
pub(super) struct Monster;

#[derive(Component)]
pub(super) struct Npc;

#[derive(Component)]
pub(super) struct GroundDrop;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DropExpiry {
    pub(super) expires_at_tick: u64,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DropOwnership {
    pub(super) owner_object_id: u32,
    pub(super) expires_at_tick: u64,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HarvestOwnership {
    pub(super) owner_object_id: u32,
}

#[allow(deprecated)]
pub(super) fn entity_by_object_id(world: &World, target: u32) -> Option<Entity> {
    world
        .iter_entities()
        .find_map(|entity| (entity.get::<ObjectId>()?.0 == target).then_some(entity.id()))
}

#[allow(deprecated)]
pub(super) fn player_entity(world: &World) -> Option<Entity> {
    world
        .iter_entities()
        .find_map(|entity| entity.contains::<SelfPlayer>().then_some(entity.id()))
}

#[allow(deprecated)]
pub(super) fn hero_entity(world: &World) -> Option<Entity> {
    world
        .iter_entities()
        .find_map(|entity| entity.contains::<Hero>().then_some(entity.id()))
}

pub(super) fn current_player_object_id(world: &World) -> Option<u32> {
    player_entity(world).and_then(|entity| entity_object_id(world, entity))
}

pub(super) fn current_hero_object_id(world: &World) -> Option<u32> {
    hero_entity(world).and_then(|entity| entity_object_id(world, entity))
}

pub(super) fn current_character_index(world: &World) -> Option<i32> {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.index)
}

pub(super) fn entity_object_id(world: &World, entity: Entity) -> Option<u32> {
    world.entity(entity).get::<ObjectId>().map(|value| value.0)
}

pub(super) fn entity_position(world: &World, entity: Entity) -> Option<Point> {
    world
        .entity(entity)
        .get::<Position>()
        .map(|value| value.0.clone())
}

pub(super) fn entity_name(world: &World, entity: Entity) -> Option<String> {
    let language = world.resource::<SessionResource>().language;
    world
        .entity(entity)
        .get::<DisplayName>()
        .map(|value| value.resolve(language))
}

pub(super) fn entity_facing(world: &World, entity: Entity) -> Option<MirDirection> {
    world.entity(entity).get::<Facing>().map(|value| value.0)
}

pub(super) fn entity_player_vitals(world: &World, entity: Entity) -> Option<PlayerVitals> {
    world.entity(entity).get::<PlayerVitals>().copied()
}

pub(super) fn current_player_is_dead(world: &World) -> bool {
    player_entity(world)
        .and_then(|entity| entity_player_vitals(world, entity))
        .is_some_and(|vitals| vitals.hp <= 0)
}
