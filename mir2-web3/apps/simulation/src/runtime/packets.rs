use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_base_stats_info_packet_payload, crystal_game_shop_info_packet_payloads,
    crystal_guild_buff_list_packet_payload, crystal_item_by_index,
    crystal_map_respawns_by_file_name, crystal_npc_info_manifest, crystal_recipe_bootstrap_packets,
    format_localized_text, localized_text_or_fallback, LanguageCode,
};
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, MonsterInfo, NpcInfo,
    ObjectDiedInfo, ObjectGoldInfo, ObjectHealthInfo, ObjectItemInfo, ObjectManaInfo,
    ObjectMovement, ObjectPlayerInfo, ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo, Point,
    ServerPacket, ServerPacketId, Spell, StruckInfo, UserInformation, UserItem,
};

use crate::config::{
    CharacterRecord, EquipmentSlot, GroundDropLootSnapshot, GroundDropSnapshot, ItemContainer,
    MapTransferSnapshot, SimulationConfig, WorldEntityDisposition, WorldEntityKind,
    WorldEntitySnapshot, WorldEntitySpriteSnapshot, WorldSnapshot,
};

use super::components::{
    current_player_object_id, entity_facing, entity_object_id, entity_player_vitals,
    entity_position, player_entity, CharacterBody, DisplayName, Facing, GeneralMeowMeowState,
    GroundDrop, HarvestMonsterState, Monster, MonsterAgent, MonsterAiState, MonsterVitals, Npc,
    NpcAgent, ObjectId, PlayerVitals, Position, RemotePlayer, SelfPlayer, SummonedMonster,
};
use super::crystal_compat::{BASE_STORAGE_SLOTS, BUFF_GENERAL_MEOW_MEOW_SHIELD};
use super::drops::*;
use super::drops::{
    crystal_drop_name_colour_argb, crystal_item_grade_for_key,
    crystal_item_name_colour_argb_for_drop, DropLoot, DropPayload,
};
use super::equipment::*;
use super::equipment::{
    equipment_shape, equipment_slot_index, user_item_from_equipment_state, EquipmentState,
};
use super::inventory::*;
use super::inventory::{
    current_weight, expand_storage_rental_impl, free_bag_slots, storage_password_required,
};
use super::items::*;
use super::items::{
    crystal_item_template_for_item_key, item_info_from_crystal_template, user_item_from_item_state,
    ItemState,
};
use super::map::{
    active_scene_view, crystal_movement_transfer_records_for_map, filter_decor_objects,
    filter_terrain_patches, is_safe_zone_point, normalize_map_file_name, point_visible,
    rebuild_world,
};
use super::monster_ai::advance_world;
use super::monsters::{
    crystal_monster_effect_for_name, crystal_respawn_object_id,
    crystal_respawn_object_monster_packet, point_in_data_range, start_game_visible_respawn_spawns,
};
use super::movement::current_location;
use super::npc::{
    buy_item_impl, crystal_npc_visible_to_character, crystal_quest_ids_by_npc, dismiss_dialog,
    sell_item_impl,
};
use super::resources::{
    is_in_world, BuffResource, InventoryResource, MapRuntimeResource, NpcStateResource,
    PlayerPermissionResource, PlayerRuntimeResource, QuestResource, RuntimeConfigResource,
    RuntimeQueueResource, SessionResource, SkillResource, Stage5SystemsResource,
};
use super::save::*;
use super::session::SimulationSession;
use super::skills::{
    assign_magic_key, cast_skill_with_context, skill_key_for_crystal_spell, SkillCastContext,
};

pub(super) fn system_message(message: &str) -> ServerPacket {
    ServerPacket::Chat {
        message: message.to_string(),
        chat_type: mir2_protocol::ChatType::System,
    }
}

pub(super) fn system_message_key(world: &World, key: &str) -> ServerPacket {
    let message = localized_text_or_fallback(super::session::current_language(world), key, key);
    system_message(&message)
}

pub(super) fn hint_chat_key(world: &World, key: &str) -> ServerPacket {
    ServerPacket::Chat {
        message: localized_text_or_fallback(super::session::current_language(world), key, key),
        chat_type: mir2_protocol::ChatType::Hint,
    }
}

pub(super) fn system_message_key_args<I, S>(world: &World, key: &str, args: I) -> ServerPacket
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    let message = format_localized_text(super::session::current_language(world), key, args);
    system_message(&message)
}

pub(super) fn hint_chat_key_args<I, S>(world: &World, key: &str, args: I) -> ServerPacket
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    ServerPacket::Chat {
        message: format_localized_text(super::session::current_language(world), key, args),
        chat_type: mir2_protocol::ChatType::Hint,
    }
}

pub(super) fn localized_map_title(language: LanguageCode, fallback: &str) -> String {
    match fallback {
        "Starter Field" | "Starter Zone" => {
            localized_text_or_fallback(language, "content.scene.starterField.title", fallback)
        }
        _ => fallback.to_string(),
    }
}

pub(super) fn localized_visible_player_name_key(object_id: u32) -> Option<&'static str> {
    match object_id {
        2001 => Some("content.visiblePlayer.mentor.name"),
        _ => None,
    }
}

pub(super) fn localized_monster_name_key(object_id: u32) -> Option<&'static str> {
    match object_id {
        3001 => Some("content.monster.trainingDummy.name"),
        3002 => Some("content.monster.fieldWasp.name"),
        _ => None,
    }
}

pub(super) fn localized_npc_name_key(object_id: u32) -> Option<&'static str> {
    match object_id {
        super::crystal_compat::GUIDE_NPC_ID => Some("content.npc.villageGuide.name"),
        _ => None,
    }
}

pub(super) fn handle_chat_packet(world: &mut World, message: String) -> Vec<ServerPacket> {
    if !super::session::is_in_world(world) {
        return Vec::new();
    }

    if let Some(remaining_seconds) = active_chat_ban_remaining_seconds(world) {
        return vec![chat_ban_remaining_message(world, remaining_seconds)];
    }

    if message.trim().eq_ignore_ascii_case("@ADDSTORAGE") {
        return expand_storage_rental_impl(world);
    }

    let player_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "?????".to_string());

    vec![ServerPacket::ObjectChat {
        object_id: current_player_object_id(world).unwrap_or(0),
        text: format!("{player_name}: {message}"),
        chat_type: mir2_protocol::ChatType::Normal,
    }]
}

fn active_chat_ban_remaining_seconds(world: &mut World) -> Option<u64> {
    let now = unix_now_ms();
    let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
    if !player_runtime.chat_banned {
        return None;
    }
    match player_runtime.chat_ban_until_ms {
        Some(until_ms) if until_ms > now => Some((until_ms - now).div_ceil(1_000).max(1)),
        Some(_) => {
            player_runtime.chat_banned = false;
            player_runtime.chat_ban_until_ms = None;
            None
        }
        None => Some(1),
    }
}

fn chat_ban_remaining_message(world: &World, remaining_seconds: u64) -> ServerPacket {
    let days = remaining_seconds / 86_400;
    let hours = (remaining_seconds % 86_400) / 3_600;
    let minutes = (remaining_seconds % 3_600) / 60;
    let seconds = remaining_seconds % 60;
    if days > 0 {
        system_message_key_args(
            world,
            "server.ChatBanRemainingTimeByDay",
            [days, hours, minutes, seconds],
        )
    } else if hours > 0 {
        system_message_key_args(
            world,
            "server.ChatBanRemainingTimeByHour",
            [hours, minutes, seconds],
        )
    } else if minutes > 0 {
        system_message_key_args(
            world,
            "server.ChatBanRemainingTimeByMinutes",
            [minutes, seconds],
        )
    } else {
        system_message_key_args(world, "server.ChatBanRemainingTimeBySecond", [seconds])
    }
}

#[derive(Debug, Clone)]
pub(super) struct VisibleObjectBundle {
    pub(super) spawn_packet: ServerPacket,
    pub(super) health_packet: Option<ServerPacket>,
}

pub(super) fn player_struck_packet(attacker_id: u32) -> ServerPacket {
    ServerPacket::Struck {
        info: StruckInfo { attacker_id },
    }
}

pub(super) fn object_struck_packet(
    world: &World,
    target_entity: Entity,
    attacker_id: u32,
) -> Option<ServerPacket> {
    Some(ServerPacket::ObjectStruck {
        info: ObjectStruckInfo {
            object_id: entity_object_id(world, target_entity)?,
            attacker_id,
            location: entity_position(world, target_entity)?,
            direction: entity_facing(world, target_entity)?,
        },
    })
}

pub(super) fn build_world_snapshot(world: &World) -> WorldSnapshot {
    let resources = world.resource::<InventoryResource>();
    let player_runtime = world.resource::<PlayerRuntimeResource>();
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    let session = world.resource::<SessionResource>();
    let quests = world.resource::<QuestResource>();
    let skills = world.resource::<SkillResource>();
    let buffs = world.resource::<BuffResource>();
    let npc_state = world.resource::<NpcStateResource>();
    let stage5 = world.resource::<Stage5SystemsResource>();
    let language = session.language;
    let tick = super::session::runtime_tick(world);
    let scene_view = active_scene_view(world);
    let mut entities = collect_world_entities(
        world,
        scene_view.as_ref(),
        language,
        &resources.equipment_items,
    );
    entities.sort_by_key(|entity| entity.object_id);

    let player_object_id = current_player_object_id(world);
    let player_vitals = player_entity(world).and_then(|entity| entity_player_vitals(world, entity));

    WorldSnapshot {
        tick,
        map_title: session
            .selected_character
            .as_ref()
            .map(|_| super::session::localized_map_title(language, &map.current_map.title)),
        map_file_name: session
            .selected_character
            .as_ref()
            .map(|_| map.current_map.file_name.clone()),
        in_safe_zone: player_entity(world)
            .and_then(|entity| entity_position(world, entity))
            .map(|position| is_safe_zone_point(config, map, &position))
            .unwrap_or(false),
        player_object_id,
        player_hp: player_vitals.map(|vitals| vitals.hp),
        player_max_hp: player_vitals.map(|vitals| vitals.max_hp),
        player_mp: player_vitals.map(|vitals| vitals.mp),
        player_experience: player_runtime.experience,
        player_max_experience: player_runtime.max_experience,
        gold: player_runtime.gold,
        credit: player_runtime.credit,
        current_weight: current_weight(resources),
        max_weight: super::drops::CRYSTAL_BAG_WEIGHT_LIMIT as u16,
        free_bag_slots: free_bag_slots(resources),
        max_bag_slots: 80,
        storage_size: resources.storage_size,
        has_expanded_storage: resources.has_expanded_storage,
        has_storage_password: resources.storage_has_password,
        require_storage_password: storage_password_required(
            config,
            resources.storage_has_password,
            resources.storage_unlocked,
        ),
        storage_password_last_set_binary_datetime: resources
            .storage_password_last_set_binary_datetime,
        expanded_storage_expiry_time_binary_datetime: resources
            .expanded_storage_expiry_time_binary_datetime,
        scene_view: scene_view.clone(),
        terrain_patches: filter_terrain_patches(world, scene_view.as_ref()),
        decor_objects: filter_decor_objects(world, scene_view.as_ref()),
        entities,
        ground_drops: collect_ground_drops(world, scene_view.as_ref(), language),
        belt_items: resources
            .belt_items
            .iter()
            .map(|item| item.snapshot(language))
            .collect(),
        inventory_items: resources
            .inventory_items
            .iter()
            .map(|item| item.snapshot(language))
            .collect(),
        storage_items: resources
            .storage_items
            .iter()
            .map(|item| item.snapshot(language))
            .collect(),
        equipment_items: resources
            .equipment_items
            .iter()
            .map(|item| item.snapshot(language))
            .collect(),
        quest_log: quests
            .quests
            .iter()
            .map(|quest| quest.snapshot(language))
            .collect(),
        active_npc_dialog: npc_state
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.snapshot(language)),
        known_skills: skills
            .skills
            .iter()
            .map(|skill| skill.snapshot(tick, language))
            .collect(),
        active_buffs: buffs
            .buffs
            .iter()
            .filter(|buff| buff.expires_at_tick > tick)
            .map(|buff| buff.snapshot(tick, language))
            .collect(),
        stage5_systems: stage5.stage5_systems.clone(),
        map_transfers: collect_map_transfer_snapshots(config, map),
        interaction_hints: build_interaction_hints(world, resources),
    }
}

pub(super) fn build_interaction_hints(
    world: &World,
    _resources: &InventoryResource,
) -> Vec<String> {
    let language = world.resource::<SessionResource>().language;
    let mut hints = vec![
        localized_text_or_fallback(
            language,
            "custom.interaction.primaryControls",
            "Left click walks, right click runs, and clicking a target focuses it.",
        ),
        localized_text_or_fallback(
            language,
            "custom.interaction.targetHelp",
            "Target monsters or NPCs in-scene, then attack or talk from range 1.",
        ),
    ];

    if let Some(quest) = world.resource::<QuestResource>().quests.first() {
        hints.push(format_localized_text(
            language,
            "custom.interaction.questHint",
            [quest.tracker(language)],
        ));
    }

    if world
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .is_some()
    {
        hints.push(localized_text_or_fallback(
            language,
            "custom.interaction.npcDialogLive",
            "NPC dialog is live; review it, then continue field progression.",
        ));
    }

    hints
}

pub(super) fn object_movement(world: &World, entity: Entity) -> Option<ObjectMovement> {
    let entry = world.entity(entity);
    Some(ObjectMovement {
        object_id: entry.get::<ObjectId>()?.0,
        position: entry.get::<Position>()?.0.clone(),
        direction: entry.get::<Facing>()?.0,
    })
}

pub(super) fn object_health_info_for_entity(
    world: &World,
    entity: Entity,
    expire: u8,
) -> Option<ObjectHealthInfo> {
    let entry = world.entity(entity);
    let object_id = entry.get::<ObjectId>()?.0;
    let (hp, max_hp) = if let Some(vitals) = entry.get::<MonsterVitals>() {
        (vitals.hp, vitals.max_hp)
    } else if let Some(vitals) = entry.get::<PlayerVitals>() {
        (vitals.hp, vitals.max_hp)
    } else {
        return None;
    };

    Some(ObjectHealthInfo {
        object_id,
        percent: health_percent(hp, max_hp),
        expire,
    })
}

pub(super) fn object_mana_info_for_entity(world: &World, entity: Entity) -> Option<ObjectManaInfo> {
    let entry = world.entity(entity);
    let object_id = entry.get::<ObjectId>()?.0;
    let vitals = entry.get::<PlayerVitals>()?;

    Some(ObjectManaInfo {
        object_id,
        percent: mana_percent(vitals.mp, 100),
    })
}

pub(super) fn health_percent(hp: i32, max_hp: i32) -> u8 {
    let max_hp = max_hp.max(1);
    let percent = ((hp.max(0) * 100) / max_hp).clamp(0, 100);
    u8::try_from(percent).expect("health percent should fit")
}

pub(super) fn mana_percent(mp: i32, max_mp: i32) -> u8 {
    let max_mp = max_mp.max(1);
    let percent = ((mp.max(0) * 100) / max_mp).clamp(0, 100);
    u8::try_from(percent).expect("mana percent should fit")
}

pub(super) fn object_died_info_for_entity(
    world: &World,
    entity: Entity,
    kind: u8,
) -> Option<ObjectDiedInfo> {
    let movement = object_movement(world, entity)?;
    Some(ObjectDiedInfo {
        object_id: movement.object_id,
        location: movement.position,
        direction: movement.direction,
        kind,
    })
}

pub(super) fn object_revived_info_for_entity(
    world: &World,
    entity: Entity,
    effect: bool,
) -> Option<ObjectRevivedInfo> {
    Some(ObjectRevivedInfo {
        object_id: world.entity(entity).get::<ObjectId>()?.0,
        effect,
    })
}

pub(super) fn should_emit_object_packet(
    packet: &ServerPacket,
    previous_visible: &BTreeSet<u32>,
    next_visible: &BTreeSet<u32>,
    self_object_id: Option<u32>,
) -> bool {
    match packet {
        ServerPacket::ObjectPlayer { .. }
        | ServerPacket::ObjectRemove { .. }
        | ServerPacket::ObjectItem { .. }
        | ServerPacket::ObjectGold { .. }
        | ServerPacket::ObjectNpc { .. } => false,
        ServerPacket::ObjectMonster { info } => previous_visible.contains(&info.object_id),
        ServerPacket::ObjectTeleportOut { object_id, .. }
        | ServerPacket::ObjectTeleportIn { object_id, .. } => {
            should_emit_tracked_object(*object_id, previous_visible, next_visible, self_object_id)
        }
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement }
        | ServerPacket::ObjectBackStep { movement, .. }
        | ServerPacket::ObjectSitDown { movement, .. }
        | ServerPacket::ObjectHarvest { movement }
        | ServerPacket::ObjectHarvested { movement } => should_emit_tracked_object(
            movement.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectAttack { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectStruck { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectRangeAttack { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectSpell { .. } => true,
        ServerPacket::ObjectEffect { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectChat { object_id, .. }
        | ServerPacket::ObjectHide { object_id }
        | ServerPacket::ObjectShow { object_id } => {
            should_emit_tracked_object(*object_id, previous_visible, next_visible, self_object_id)
        }
        ServerPacket::ObjectDied { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectRevived { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectHealth { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::ObjectMana { info } => should_emit_tracked_object(
            info.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::AddBuff { buff } => should_emit_tracked_object(
            buff.object_id,
            previous_visible,
            next_visible,
            self_object_id,
        ),
        ServerPacket::RemoveBuff { object_id, .. } => {
            should_emit_tracked_object(*object_id, previous_visible, next_visible, self_object_id)
        }
        _ => true,
    }
}

pub(super) fn should_emit_tracked_object(
    object_id: u32,
    previous_visible: &BTreeSet<u32>,
    next_visible: &BTreeSet<u32>,
    self_object_id: Option<u32>,
) -> bool {
    Some(object_id) == self_object_id
        || previous_visible.contains(&object_id)
        || next_visible.contains(&object_id)
}

pub(super) fn request_item_info_impl(item_index: i32) -> Vec<ServerPacket> {
    match crystal_item_by_index(item_index) {
        Some(template) => vec![ServerPacket::NewItemInfo {
            info: item_info_from_crystal_template(template),
        }],
        None => vec![super::session::system_message(&localized_text_or_fallback(
            LanguageCode::English,
            "server.NotFound",
            "server.NotFound",
        ))],
    }
}

pub(super) fn start_game_item_info_packets(
    resources: &InventoryResource,
    sent_item_info_indices: &mut BTreeSet<i32>,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for item_key in resources
        .inventory_items
        .iter()
        .map(|item| item.key.as_str())
        .chain(
            resources
                .equipment_items
                .iter()
                .map(|item| item.key.as_str()),
        )
    {
        let Some(template) = crystal_item_template_for_item_key(item_key) else {
            continue;
        };
        if sent_item_info_indices.insert(template.item_index) {
            packets.push(ServerPacket::NewItemInfo {
                info: item_info_from_crystal_template(template),
            });
        }
    }
    packets
}

pub(super) fn start_game_recipe_info_packets(
    sent_item_info_indices: &mut BTreeSet<i32>,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for recipe in crystal_recipe_bootstrap_packets() {
        for item_index in recipe.item_info_indices {
            if !sent_item_info_indices.insert(item_index) {
                continue;
            }
            let Some(template) = crystal_item_by_index(item_index) else {
                continue;
            };
            packets.push(ServerPacket::NewItemInfo {
                info: item_info_from_crystal_template(template),
            });
        }
        packets.push(ServerPacket::NewRecipeInfo {
            payload: recipe.payload,
        });
    }
    packets
}

pub(super) fn start_game_account_social_and_shop_packets() -> Vec<ServerPacket> {
    let mut packets = vec![
        raw_server_packet(ServerPacketId::CompleteQuest, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::ReceiveMail, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::FriendUpdate, &[0, 0, 0, 0]),
        raw_server_packet(
            ServerPacketId::LoverUpdate,
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        raw_server_packet(
            ServerPacketId::MentorUpdate,
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
    ];
    packets.extend(
        crystal_game_shop_info_packet_payloads()
            .into_iter()
            .map(|payload| ServerPacket::Raw {
                packet_id: ServerPacketId::GameShopInfo,
                payload,
            }),
    );
    packets
}

pub(super) fn start_game_base_stats_packet(class: MirClass) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    if let Some(payload) = crystal_base_stats_info_packet_payload(class) {
        packets.push(ServerPacket::Raw {
            packet_id: ServerPacketId::BaseStatsInfo,
            payload,
        });
    }
    packets
}

pub(super) fn start_game_post_visible_crystal_bootstrap_packets() -> Vec<ServerPacket> {
    vec![
        raw_server_packet(ServerPacketId::TimeOfDay, &[4]),
        raw_server_packet(ServerPacketId::ChangeAMode, &[0]),
        raw_server_packet(ServerPacketId::ChangePMode, &[0]),
        raw_server_packet(ServerPacketId::SwitchGroup, &[0]),
        raw_server_packet(ServerPacketId::DefaultNPC, &[0, 0, 0, 0]),
        ServerPacket::Raw {
            packet_id: ServerPacketId::GuildBuffList,
            payload: crystal_guild_buff_list_packet_payload(),
        },
        raw_server_packet(ServerPacketId::NPCUpdate, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::NPCUpdate, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::NPCUpdate, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::NPCResponse, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::NPCResponse, &[0, 0, 0, 0]),
        raw_server_packet(ServerPacketId::NPCResponse, &[0, 0, 0, 0]),
    ]
}

pub(super) fn start_game_static_visible_object_packets(
    map_file_name: &str,
    player_position: &Point,
    character: &CharacterRecord,
) -> Vec<ServerPacket> {
    let normalized_map = normalize_map_file_name(map_file_name);
    let quest_ids_by_npc = crystal_quest_ids_by_npc();
    let mut objects = Vec::<(i32, i32, u32, ServerPacket)>::new();

    for npc in crystal_npc_info_manifest().npcs {
        if npc
            .map_file_name
            .as_deref()
            .map(normalize_map_file_name)
            .as_deref()
            != Some(normalized_map.as_str())
        {
            continue;
        }
        if !point_in_data_range(&npc.location, player_position) {
            continue;
        }
        if !crystal_npc_visible_to_character(&npc, character) {
            continue;
        }
        let Some(object_id) = npc.loaded_object_id else {
            continue;
        };
        let quest_ids = quest_ids_by_npc
            .get(&object_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default();
        objects.push((
            npc.location.y,
            npc.location.x,
            object_id,
            ServerPacket::ObjectNpc {
                info: NpcInfo {
                    object_id,
                    name: npc.name,
                    name_colour_argb: -16_711_936,
                    image: npc.image,
                    colour_argb: 0,
                    location: npc.location,
                    direction: MirDirection::Up,
                    quest_ids,
                },
            },
        ));
    }

    if let Some(map) = crystal_map_respawns_by_file_name(map_file_name) {
        for respawn in &map.respawns {
            let visible_spawns =
                start_game_visible_respawn_spawns(map_file_name, respawn, player_position);
            for (slot_index, location, direction) in visible_spawns {
                let object_id = crystal_respawn_object_id(respawn, slot_index);
                objects.push((
                    location.y,
                    location.x,
                    object_id,
                    crystal_respawn_object_monster_packet(respawn, object_id, location, direction),
                ));
            }
        }
        for spell in map.safe_zone_spells {
            if !point_in_data_range(&spell.location, player_position) {
                continue;
            }
            let spell_kind = match spell.spell.as_str() {
                "TrapHexagon" => Spell::TrapHexagon,
                "Healing" => Spell::Healing,
                _ => Spell::None,
            };
            objects.push((
                spell.location.y,
                spell.location.x,
                spell.object_id,
                ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: spell.object_id,
                        location: spell.location,
                        spell: spell_kind,
                        direction: MirDirection::Up,
                        param: false,
                    },
                },
            ));
        }
    }

    objects.sort_by_key(|(y, x, object_id, _)| (*y, *x, *object_id));
    objects
        .into_iter()
        .map(|(_, _, _, packet)| packet)
        .collect()
}

pub(super) fn build_user_information(
    config: &SimulationConfig,
    character: &CharacterRecord,
    position: &Point,
    direction: MirDirection,
    vitals: PlayerVitals,
    experience: i64,
    max_experience: i64,
    gold: u32,
    credit: u32,
    storage_size: u16,
    has_expanded_storage: bool,
    storage_has_password: bool,
    require_storage_password: bool,
    storage_password_last_set_binary_datetime: i64,
    expanded_storage_expiry_time_binary_datetime: i64,
    hair: u8,
    inventory_items: &[ItemState],
    equipment_items: &[EquipmentState],
) -> UserInformation {
    UserInformation {
        object_id: config.object_id,
        real_id: u32::try_from(character.index).unwrap_or(config.real_id),
        name: character.name.clone(),
        guild_name: String::new(),
        guild_rank: String::new(),
        name_colour_argb: -1,
        class: character.class,
        gender: character.gender,
        level: character.level,
        location: position.clone(),
        direction,
        hair,
        hp: vitals.hp,
        mp: vitals.mp,
        experience,
        max_experience,
        level_effects: 0,
        has_hero: false,
        hero_behaviour: 0,
        inventory_section_present: true,
        inventory: Some(user_inventory_slots(inventory_items)),
        equipment_section_present: true,
        equipment: Some(user_equipment_slots(equipment_items)),
        quest_inventory_section_present: true,
        quest_inventory: Some(user_quest_inventory_slots(inventory_items)),
        gold,
        credit,
        has_expanded_storage,
        has_storage_password: storage_has_password,
        require_storage_password,
        storage_password_last_set_binary_datetime,
        expanded_storage_expiry_time_binary_datetime: if has_expanded_storage
            || expanded_storage_expiry_time_binary_datetime != 0
            || storage_size != BASE_STORAGE_SLOTS
        {
            expanded_storage_expiry_time_binary_datetime
        } else {
            0
        },
        magic_count: 0,
        intelligent_creature_count: 0,
        summoned_creature_type: 99,
        creature_summoned: false,
        allow_observe: false,
        observer: false,
    }
}

pub(super) fn user_inventory_slots(items: &[ItemState]) -> Vec<Option<UserItem>> {
    let mut slots = vec![None; 46];
    for item in items {
        let index = match item.container {
            ItemContainer::Bag1 => usize::from(item.slot),
            ItemContainer::Bag2 => 40 + usize::from(item.slot),
            _ => continue,
        };
        if let Some(slot) = slots.get_mut(index) {
            *slot = Some(user_item_from_item_state(item));
        }
    }
    slots
}

pub(super) fn user_equipment_slots(items: &[EquipmentState]) -> Vec<Option<UserItem>> {
    let mut slots = vec![None; 14];
    for item in items {
        let Some(index) = equipment_slot_index(item.slot) else {
            continue;
        };
        if let Some(slot) = slots.get_mut(index) {
            *slot = user_item_from_equipment_state(item);
        }
    }
    slots
}

pub(super) fn user_quest_inventory_slots(items: &[ItemState]) -> Vec<Option<UserItem>> {
    let mut slots = vec![None; 40];
    for item in items
        .iter()
        .filter(|item| item.container == ItemContainer::Quest)
    {
        if let Some(slot) = slots.get_mut(usize::from(item.slot)) {
            *slot = Some(user_item_from_item_state(item));
        }
    }
    slots
}

pub(super) fn collect_map_transfer_snapshots(
    config: &SimulationConfig,
    map: &MapRuntimeResource,
) -> Vec<MapTransferSnapshot> {
    let current_map = normalize_map_file_name(&map.current_map.file_name);
    let mut transfers: Vec<MapTransferSnapshot> = config
        .map_transfers
        .iter()
        .filter(|transfer| normalize_map_file_name(&transfer.from_map_file_name) == current_map)
        .map(|transfer| MapTransferSnapshot {
            key: transfer.key.clone(),
            map_file_name: transfer.from_map_file_name.clone(),
            bounds: transfer.from_bounds,
            to_map_file_name: transfer.to_map_file_name.clone(),
            to_map_title: transfer.to_map_title.clone(),
            to_position: transfer.to_position.clone(),
            to_direction: transfer.to_direction,
        })
        .collect();

    transfers.extend(
        crystal_movement_transfer_records_for_map(&map.current_map.file_name)
            .into_iter()
            .map(|transfer| MapTransferSnapshot {
                key: transfer.key,
                map_file_name: transfer.from_map_file_name,
                bounds: transfer.from_bounds,
                to_map_file_name: transfer.to_map_file_name,
                to_map_title: transfer.to_map_title,
                to_position: transfer.to_position,
                to_direction: transfer.to_direction,
            }),
    );

    transfers
}

#[allow(deprecated)]
pub(super) fn collect_world_entities(
    world: &World,
    scene_view: Option<&mir2_game_data::SceneView>,
    language: LanguageCode,
    self_equipment_items: &[EquipmentState],
) -> Vec<WorldEntitySnapshot> {
    let mut result = Vec::new();
    for entity in world.iter_entities() {
        let Some(object_id) = entity.get::<ObjectId>() else {
            continue;
        };
        let Some(name) = entity.get::<DisplayName>() else {
            continue;
        };
        let Some(position) = entity.get::<Position>() else {
            continue;
        };
        let Some(facing) = entity.get::<Facing>() else {
            continue;
        };

        let body = entity.get::<CharacterBody>();
        let player_vitals = entity.get::<PlayerVitals>();
        let monster_agent = entity.get::<MonsterAgent>();
        let monster_vitals = entity.get::<MonsterVitals>();
        let self_marker = entity.get::<SelfPlayer>();
        let remote_marker = entity.get::<RemotePlayer>();
        let npc_marker = entity.get::<Npc>();
        let npc_agent = entity.get::<NpcAgent>();

        if self_marker.is_none() && !point_visible(scene_view, &position.0) {
            continue;
        }

        let (kind, disposition, dead, hp, max_hp, level) = if self_marker.is_some() {
            (
                WorldEntityKind::SelfPlayer,
                WorldEntityDisposition::Friendly,
                false,
                player_vitals.map(|v| v.hp),
                player_vitals.map(|v| v.max_hp),
                body.map(|v| v.level),
            )
        } else if remote_marker.is_some() {
            (
                WorldEntityKind::Player,
                WorldEntityDisposition::Friendly,
                false,
                None,
                None,
                body.map(|v| v.level),
            )
        } else if let Some(agent) = monster_agent {
            (
                WorldEntityKind::Monster,
                agent.disposition,
                agent.dead,
                monster_vitals.map(|v| v.hp),
                monster_vitals.map(|v| v.max_hp),
                None,
            )
        } else if npc_marker.is_some() {
            (
                WorldEntityKind::Npc,
                WorldEntityDisposition::Neutral,
                false,
                None,
                None,
                None,
            )
        } else {
            continue;
        };

        let sprite = entity_sprite_snapshot(
            body,
            monster_agent,
            npc_agent,
            if self_marker.is_some() {
                Some(self_equipment_items)
            } else {
                None
            },
        );
        let quest_ids = npc_agent
            .map(|agent| agent.quest_ids.clone())
            .unwrap_or_default();
        let name_colour_argb = match kind {
            WorldEntityKind::Npc => -16_711_936,
            _ => -1,
        };

        result.push(WorldEntitySnapshot {
            object_id: object_id.0,
            kind,
            name: name.resolve(language),
            x: position.0.x,
            y: position.0.y,
            direction: facing.0,
            class: body.map(|body| body.class),
            gender: body.map(|body| body.gender),
            level,
            hp,
            max_hp,
            name_colour_argb,
            dead,
            disposition,
            sprite,
            quest_ids,
        });
    }

    result
}

pub(super) fn entity_sprite_snapshot(
    body: Option<&CharacterBody>,
    monster_agent: Option<&MonsterAgent>,
    npc_agent: Option<&NpcAgent>,
    equipment_items: Option<&[EquipmentState]>,
) -> Option<WorldEntitySpriteSnapshot> {
    if let Some(body) = body {
        let armour_shape = equipment_shape(equipment_items, EquipmentSlot::Armour)
            .or(body.armour_shape)
            .unwrap_or(0);
        let hair_shape = 0;
        let weapon_shape =
            equipment_shape(equipment_items, EquipmentSlot::Weapon).or(body.weapon_shape);
        let uses_assassin_weapon =
            matches!(weapon_shape, Some(shape) if (100..200).contains(&shape));
        let uses_archer_weapon = matches!(weapon_shape, Some(shape) if shape >= 200);
        let body_library = format!("CArmour/{armour_shape:02}");
        let hair_library = Some(format!("CHair/{hair_shape:02}"));
        let (alt_body_library, alt_hair_library) = match body.class {
            MirClass::Assassin if uses_assassin_weapon => (
                Some(format!("AArmour/{armour_shape:02}")),
                Some(format!("AHair/{hair_shape:02}")),
            ),
            MirClass::Archer if uses_archer_weapon => (
                Some(format!("ARArmour/{armour_shape:02}")),
                Some(format!("ARHair/{hair_shape:02}")),
            ),
            _ => (None, None),
        };
        let (
            weapon_library,
            weapon_library_secondary,
            alt_weapon_library,
            alt_weapon_library_secondary,
        ) = match weapon_shape {
            Some(shape) if (100..200).contains(&shape) => {
                let index = shape - 100;
                (
                    None,
                    None,
                    Some(format!("AWeapon/{index:02} R")),
                    Some(format!("AWeapon/{index:02} L")),
                )
            }
            Some(shape) if shape >= 200 => {
                let index = shape - 200;
                (
                    Some(format!("ARWeapon/{index:02}")),
                    None,
                    Some(format!("ARWeapon/{index:02} S")),
                    None,
                )
            }
            Some(shape) => (Some(format!("CWeapon/{shape:02}")), None, None, None),
            None => (None, None, None, None),
        };
        let frame_base_offset = match body.gender {
            MirGender::Male => 0,
            MirGender::Female => 808,
        };
        let weapon_frame_offset = weapon_library.as_ref().map(|_| match body.gender {
            MirGender::Male => 0,
            MirGender::Female => 416,
        });
        let alt_frame_base_offset = match body.class {
            MirClass::Archer if uses_archer_weapon => Some(match body.gender {
                MirGender::Male => 0,
                MirGender::Female => 352,
            }),
            MirClass::Assassin if uses_assassin_weapon => Some(match body.gender {
                MirGender::Male => 0,
                MirGender::Female => 512,
            }),
            _ => None,
        };
        let alt_weapon_frame_offset = alt_frame_base_offset;

        return Some(WorldEntitySpriteSnapshot {
            body_library,
            hair_library,
            weapon_library,
            weapon_library_secondary,
            frame_base_offset,
            weapon_frame_offset,
            alt_body_library,
            alt_hair_library,
            alt_weapon_library,
            alt_weapon_library_secondary,
            alt_frame_base_offset,
            alt_weapon_frame_offset,
            frame_count: 4,
            direction_stride: 4,
        });
    }

    if let Some(monster) = monster_agent {
        return Some(WorldEntitySpriteSnapshot {
            body_library: format!("Monster/{:03}", monster.image),
            hair_library: None,
            weapon_library: None,
            weapon_library_secondary: None,
            frame_base_offset: 0,
            weapon_frame_offset: None,
            alt_body_library: None,
            alt_hair_library: None,
            alt_weapon_library: None,
            alt_weapon_library_secondary: None,
            alt_frame_base_offset: None,
            alt_weapon_frame_offset: None,
            frame_count: 4,
            direction_stride: 4,
        });
    }

    npc_agent.map(|npc| WorldEntitySpriteSnapshot {
        body_library: format!("NPC/{:02}", npc.image),
        hair_library: None,
        weapon_library: None,
        weapon_library_secondary: None,
        frame_base_offset: 0,
        weapon_frame_offset: None,
        alt_body_library: None,
        alt_hair_library: None,
        alt_weapon_library: None,
        alt_weapon_library_secondary: None,
        alt_frame_base_offset: None,
        alt_weapon_frame_offset: None,
        frame_count: 4,
        direction_stride: 4,
    })
}

#[allow(deprecated)]
pub(super) fn collect_ground_drops(
    world: &World,
    scene_view: Option<&mir2_game_data::SceneView>,
    language: LanguageCode,
) -> Vec<GroundDropSnapshot> {
    let mut drops = Vec::new();

    for entity in world.iter_entities() {
        if entity.get::<GroundDrop>().is_none() {
            continue;
        }

        let object_id = entity.get::<ObjectId>().expect("drop object id").0;
        let name = entity
            .get::<DisplayName>()
            .expect("drop name")
            .resolve(language);
        let position = entity.get::<Position>().expect("drop position").0.clone();
        let payload = entity.get::<DropPayload>().expect("drop payload");
        if !point_visible(scene_view, &position) {
            continue;
        }

        drops.push(GroundDropSnapshot {
            object_id,
            name,
            name_colour_argb: crystal_drop_name_colour_argb(&payload.loot),
            x: position.x,
            y: position.y,
            quantity: payload.quantity,
            source_monster: match payload.source_monster_key.as_deref() {
                Some(key) => localized_text_or_fallback(language, key, &payload.source_monster),
                None => payload.source_monster.clone(),
            },
            loot: ground_drop_loot_snapshot(&payload.loot),
        });
    }

    drops.sort_by_key(|drop| drop.object_id);
    drops
}

fn ground_drop_loot_snapshot(loot: &DropLoot) -> GroundDropLootSnapshot {
    match loot {
        DropLoot::Gold(amount) => GroundDropLootSnapshot::Gold { amount: *amount },
        DropLoot::InventoryItem {
            key,
            name,
            description,
            weight,
            durability_current,
            durability_max,
            added_attack,
            added_defence,
            added_stats,
            cursed,
            socket_slots,
            show_group_pickup,
        } => GroundDropLootSnapshot::InventoryItem {
            key: key.clone(),
            name: name.clone(),
            description: description.clone(),
            weight: *weight,
            durability_current: *durability_current,
            durability_max: *durability_max,
            added_attack: *added_attack,
            added_defence: *added_defence,
            added_stats: added_stats.clone(),
            cursed: *cursed,
            socket_slots: *socket_slots,
            show_group_pickup: *show_group_pickup,
        },
    }
}

#[allow(deprecated)]
pub(super) fn collect_visible_objects(world: &World) -> BTreeMap<u32, VisibleObjectBundle> {
    let mut objects = BTreeMap::new();
    let scene_view = active_scene_view(world);
    let language = super::session::current_language(world);

    for entity in world.iter_entities() {
        let position = entity.get::<Position>().map(|value| value.0.clone());
        if let Some(position) = position.as_ref() {
            if !point_visible(scene_view.as_ref(), position) {
                continue;
            }
        }

        if entity.get::<SelfPlayer>().is_some() {
            continue;
        }

        if let Some((object_id, bundle)) =
            visible_object_bundle_for_entity(world, entity.id(), language)
        {
            objects.insert(object_id, bundle);
        }
    }

    objects
}

pub(super) fn visible_object_bundle_for_entity(
    world: &World,
    entity: Entity,
    language: LanguageCode,
) -> Option<(u32, VisibleObjectBundle)> {
    let entry = world.entity(entity);
    let object_id = entry.get::<ObjectId>()?.0;

    if entry.get::<RemotePlayer>().is_some() {
        let name = entry.get::<DisplayName>()?.resolve(language);
        let position = entry.get::<Position>()?.0.clone();
        let facing = entry.get::<Facing>()?.0;
        let body = entry.get::<CharacterBody>()?;
        return Some((
            object_id,
            VisibleObjectBundle {
                spawn_packet: ServerPacket::ObjectPlayer {
                    info: ObjectPlayerInfo {
                        object_id,
                        name,
                        guild_name: String::new(),
                        guild_rank_name: String::new(),
                        name_colour_argb: -1,
                        class: body.class,
                        gender: body.gender,
                        level: body.level,
                        location: position,
                        direction: facing,
                        hair: 0,
                        light: 0,
                        weapon: 0,
                        weapon_effect: 0,
                        armour: 0,
                        poison: 0,
                        dead: false,
                        hidden: false,
                        effect: 0,
                        wing_effect: 0,
                        extra: false,
                        mount_type: 0,
                        riding_mount: false,
                        fishing: false,
                        transform_type: 0,
                        element_orb_effect: 0,
                        element_orb_level: 0,
                        element_orb_max: 0,
                        buffs: Vec::new(),
                        level_effects: 0,
                    },
                },
                health_packet: None,
            },
        ));
    }

    if entry.get::<Monster>().is_some() {
        let name = entry.get::<DisplayName>()?.resolve(language);
        let effect = crystal_monster_effect_for_name(&name);
        let position = entry.get::<Position>()?.0.clone();
        let facing = entry.get::<Facing>()?.0;
        let monster = entry.get::<MonsterAgent>()?;
        let ai_state = entry.get::<MonsterAiState>().copied().unwrap_or_default();
        let harvested = entry
            .get::<HarvestMonsterState>()
            .map(|state| state.harvested)
            .unwrap_or(false);
        if ai_state.hidden {
            return None;
        }
        let summoned = entry.get::<SummonedMonster>().copied();
        let tick = super::session::runtime_tick(world);
        let buffs = if monster.ai == 123
            && entry
                .get::<GeneralMeowMeowState>()
                .map(|state| state.shield_until_tick > tick)
                .unwrap_or(false)
        {
            vec![BUFF_GENERAL_MEOW_MEOW_SHIELD]
        } else {
            Vec::new()
        };
        let spawn_packet = ServerPacket::ObjectMonster {
            info: MonsterInfo {
                object_id,
                name,
                name_colour_argb: -1,
                location: position,
                image: monster.image,
                direction: facing,
                effect,
                ai: monster.ai,
                light: 0,
                dead: monster.dead,
                skeleton: harvested,
                poison: 0,
                hidden: false,
                shock_time: 0,
                binding_shot_center: false,
                extra: ai_state.extra || summoned.map(|value| value.visible_extra).unwrap_or(false),
                extra_byte: ai_state.extra_byte,
                master_object_id: summoned
                    .map(|summoned| summoned.summoner_object_id)
                    .unwrap_or(0),
                rarity: 0,
                buffs,
            },
        };

        return Some((
            object_id,
            VisibleObjectBundle {
                spawn_packet,
                health_packet: object_health_info_for_entity(world, entity, 0)
                    .map(|info| ServerPacket::ObjectHealth { info }),
            },
        ));
    }

    if entry.get::<Npc>().is_some() {
        let name = entry.get::<DisplayName>()?.resolve(language);
        let position = entry.get::<Position>()?.0.clone();
        let facing = entry.get::<Facing>()?.0;
        let npc = entry.get::<NpcAgent>()?;
        return Some((
            object_id,
            VisibleObjectBundle {
                spawn_packet: ServerPacket::ObjectNpc {
                    info: NpcInfo {
                        object_id,
                        name,
                        name_colour_argb: -1,
                        image: npc.image,
                        colour_argb: npc.colour_argb,
                        location: position,
                        direction: facing,
                        quest_ids: npc.quest_ids.clone(),
                    },
                },
                health_packet: None,
            },
        ));
    }

    if entry.get::<GroundDrop>().is_some() {
        let position = entry.get::<Position>()?.0.clone();
        let payload = entry.get::<DropPayload>()?;
        let spawn_packet = match &payload.loot {
            DropLoot::Gold(amount) => ServerPacket::ObjectGold {
                info: ObjectGoldInfo {
                    object_id,
                    gold: *amount,
                    location: position,
                },
            },
            DropLoot::InventoryItem {
                key,
                added_attack,
                added_defence,
                added_stats,
                socket_slots,
                ..
            } => ServerPacket::ObjectItem {
                info: ObjectItemInfo {
                    object_id,
                    name: entry.get::<DisplayName>()?.resolve(language),
                    name_colour_argb: crystal_item_name_colour_argb_for_drop(
                        key,
                        *added_attack,
                        *added_defence,
                        added_stats,
                        *socket_slots,
                    ),
                    location: position,
                    image: super::items::item_icon_for_key(key),
                    grade: crystal_item_grade_for_key(key),
                },
            },
        };

        return Some((
            object_id,
            VisibleObjectBundle {
                spawn_packet,
                health_packet: None,
            },
        ));
    }

    None
}

pub(super) fn raw_server_packet(packet_id: ServerPacketId, payload: &[u8]) -> ServerPacket {
    ServerPacket::Raw {
        packet_id,
        payload: payload.to_vec(),
    }
}

pub(super) fn prepend_packet(
    packet: ServerPacket,
    mut packets: Vec<ServerPacket>,
) -> Vec<ServerPacket> {
    let mut result = Vec::with_capacity(packets.len() + 1);
    result.push(packet);
    result.append(&mut packets);
    result
}

pub(super) fn prepend_optional_packet(
    packet: Option<ServerPacket>,
    packets: Vec<ServerPacket>,
) -> Vec<ServerPacket> {
    match packet {
        Some(packet) => prepend_packet(packet, packets),
        None => packets,
    }
}

pub(super) fn use_item_ack(
    packet_ack: Option<(u64, MirGridType)>,
    success: bool,
) -> Option<ServerPacket> {
    packet_ack.map(|(unique_id, grid)| ServerPacket::UseItem {
        unique_id,
        success,
        grid,
    })
}

impl SimulationSession {
    pub fn handle_packet(&mut self, packet: ClientPacket) -> Vec<ServerPacket> {
        if let ClientPacket::StartGame { character_index } = packet {
            return self.start_game(character_index);
        }

        let packets = self.handle_packet_impl(packet);
        self.finalize_packets(packets)
    }

    fn handle_packet_impl(&mut self, packet: ClientPacket) -> Vec<ServerPacket> {
        match packet {
            ClientPacket::ClientVersion { .. } => {
                self.app
                    .world_mut()
                    .resource_mut::<SessionResource>()
                    .version_verified = true;
                vec![ServerPacket::ClientVersion { result: 1 }]
            }
            ClientPacket::Disconnect => {
                persist_active_character_save(self.app.world());
                vec![ServerPacket::Disconnect { reason: 0 }]
            }
            ClientPacket::KeepAlive { time } => vec![ServerPacket::KeepAlive { time }],
            ClientPacket::NewAccount {
                account_id,
                password,
                ..
            } => {
                let config = self
                    .app
                    .world()
                    .resource::<RuntimeConfigResource>()
                    .config
                    .clone();
                let result = create_account_with_password(&config, &account_id, &password);
                self.app
                    .world_mut()
                    .resource_mut::<SessionResource>()
                    .characters
                    .clear();
                vec![ServerPacket::NewAccount { result }]
            }
            ClientPacket::ChangePassword {
                account_id,
                current_password,
                new_password,
            } => {
                let result = change_account_password(
                    self.app
                        .world()
                        .resource::<RuntimeConfigResource>()
                        .config
                        .clone(),
                    &account_id,
                    &current_password,
                    &new_password,
                );
                vec![ServerPacket::ChangePassword { result }]
            }
            ClientPacket::UnlockStorage { password } => {
                unlock_storage_impl(self.app.world_mut(), &password)
            }
            ClientPacket::SetStoragePassword {
                current_password,
                new_password,
            } => set_storage_password_impl(self.app.world_mut(), &current_password, &new_password),
            ClientPacket::RemoveStoragePassword { current_password } => {
                remove_storage_password_impl(self.app.world_mut(), &current_password)
            }
            ClientPacket::Login {
                account_id,
                password,
            } => {
                let config = self
                    .app
                    .world()
                    .resource::<RuntimeConfigResource>()
                    .config
                    .clone();
                let characters = match login_account(&config, &account_id, &password) {
                    AccountLoginResult::Success(characters) => characters,
                    AccountLoginResult::Banned(ban) => {
                        return vec![ServerPacket::LoginBanned {
                            reason: ban.reason,
                            expiry_binary_datetime: ban.ban_until_ms.unwrap_or_default() as i64,
                        }];
                    }
                    AccountLoginResult::InvalidCredentials => {
                        return vec![ServerPacket::Login { result: 4 }];
                    }
                };
                let mut session = self.app.world_mut().resource_mut::<SessionResource>();
                session.account_id = Some(account_id);
                session.characters = characters;
                vec![ServerPacket::LoginSuccess {
                    characters: session
                        .characters
                        .iter()
                        .map(CharacterRecord::to_select_info)
                        .collect(),
                }]
            }
            ClientPacket::NewCharacter {
                name,
                gender,
                class,
            } => {
                let config = self
                    .app
                    .world()
                    .resource::<RuntimeConfigResource>()
                    .config
                    .clone();
                let account_id = self
                    .app
                    .world()
                    .resource::<SessionResource>()
                    .account_id
                    .clone()
                    .unwrap_or_else(|| "demo".to_string());
                let character = add_character_to_account(
                    &config,
                    &account_id,
                    CharacterRecord {
                        index: 0,
                        name,
                        level: 1,
                        class,
                        gender,
                    },
                );
                self.app
                    .world_mut()
                    .resource_mut::<SessionResource>()
                    .characters = account_characters(&config, &account_id);
                vec![ServerPacket::NewCharacterSuccess {
                    char_info: character.to_new_character_select_info(),
                }]
            }
            ClientPacket::DeleteCharacter { character_index } => {
                self.delete_character_impl(character_index)
            }
            ClientPacket::StartGame { character_index } => self.start_game(character_index),
            ClientPacket::LogOut => {
                persist_active_character_save(self.app.world());
                {
                    let config = self
                        .app
                        .world()
                        .resource::<RuntimeConfigResource>()
                        .config
                        .clone();
                    {
                        let mut session = self.app.world_mut().resource_mut::<SessionResource>();
                        session.selected_character = None;
                        if let Some(account_id) = session.account_id.clone() {
                            session.characters = account_characters(&config, &account_id);
                        }
                    }
                    self.app
                        .world_mut()
                        .resource_mut::<PlayerPermissionResource>()
                        .unlock_curse = false;
                    self.app
                        .world_mut()
                        .resource_mut::<NpcStateResource>()
                        .active_npc_dialog = None;
                    let mut queue = self.app.world_mut().resource_mut::<RuntimeQueueResource>();
                    queue.pending_combat_actions = Vec::new();
                    queue.pending_monster_spawns = Vec::new();
                    drop(queue);
                    let mut inventory = self.app.world_mut().resource_mut::<InventoryResource>();
                    inventory.storage_unlocked =
                        !inventory.storage_has_password || !config.require_storage_password;
                }
                rebuild_world(self.app.world_mut());
                let session = self.app.world().resource::<SessionResource>();
                vec![ServerPacket::LogOutSuccess {
                    characters: session
                        .characters
                        .iter()
                        .map(CharacterRecord::to_select_info)
                        .collect(),
                }]
            }
            ClientPacket::Turn { direction } => {
                dismiss_dialog(self.app.world_mut());
                if let Some(player) = player_entity(self.app.world()) {
                    self.app
                        .world_mut()
                        .entity_mut(player)
                        .insert(Facing(direction));
                    let mut packets = vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                    packets.extend(advance_world(self.app.world_mut()));
                    packets
                } else {
                    Vec::new()
                }
            }
            ClientPacket::Walk { direction } => {
                dismiss_dialog(self.app.world_mut());
                self.move_player_by_direction(direction, false)
            }
            ClientPacket::Run { direction } => {
                dismiss_dialog(self.app.world_mut());
                self.move_player_by_direction(direction, true)
            }
            ClientPacket::Chat { message } => handle_chat_packet(self.app.world_mut(), message),
            ClientPacket::MoveItem { grid, from, to } => {
                move_item_impl(self.app.world_mut(), grid, from, to)
            }
            ClientPacket::StoreItem { from, to } => store_item_impl(self.app.world_mut(), from, to),
            ClientPacket::TakeBackItem { from, to } => {
                take_back_item_impl(self.app.world_mut(), from, to)
            }
            ClientPacket::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            } => merge_item_impl(self.app.world_mut(), grid_from, grid_to, id_from, id_to),
            ClientPacket::EquipItem {
                grid,
                unique_id,
                to,
            } => self.equip_item_packet_impl(grid, unique_id, to),
            ClientPacket::RemoveItem {
                grid,
                unique_id,
                to,
            } => remove_equipped_item_impl(self.app.world_mut(), grid, unique_id, to),
            ClientPacket::RemoveSlotItem {
                grid,
                grid_to,
                unique_id,
                to,
                from_unique_id,
            } => remove_equipped_slot_item_impl(
                self.app.world_mut(),
                grid,
                grid_to,
                unique_id,
                to,
                from_unique_id,
            ),
            ClientPacket::SplitItem {
                grid,
                unique_id,
                count,
            } => split_item_impl(self.app.world_mut(), grid, unique_id, count),
            ClientPacket::UseItem { unique_id, grid } => {
                if grid == MirGridType::HeroInventory {
                    // Current runtime does not model hero inventory; do not fall back into player bag items.
                    return vec![ServerPacket::UseItem {
                        unique_id,
                        success: false,
                        grid,
                    }];
                }
                let key = item_key_for_client_reference(self.app.world(), unique_id, grid);
                match key {
                    Some(key) => use_item(self.app.world_mut(), &key, Some((unique_id, grid))),
                    None => vec![ServerPacket::UseItem {
                        unique_id,
                        success: false,
                        grid,
                    }],
                }
            }
            ClientPacket::DropItem {
                unique_id,
                count,
                hero_inventory,
            } => drop_item_packet(self.app.world_mut(), unique_id, count, hero_inventory),
            ClientPacket::DeleteItem {
                unique_id,
                count,
                hero_inventory,
            } => delete_item_impl(self.app.world_mut(), unique_id, count, hero_inventory),
            ClientPacket::DropGold { amount } => drop_gold_impl(self.app.world_mut(), amount),
            ClientPacket::PickUp => pick_up_current_cell_ground_drop(self.app.world_mut()),
            ClientPacket::RequestItemInfo { item_index } => request_item_info_impl(item_index),
            ClientPacket::Attack { direction, .. } => self.attack_in_direction(direction),
            ClientPacket::RangeAttack { target_id, .. } => self.attack_impl(target_id),
            ClientPacket::Harvest { direction } => self.harvest_impl(direction),
            ClientPacket::BuyItem {
                item_index,
                count,
                panel_type,
            } => buy_item_impl(self.app.world_mut(), item_index, count, panel_type),
            ClientPacket::SellItem { unique_id, count } => {
                sell_item_impl(self.app.world_mut(), unique_id, count)
            }
            ClientPacket::RepairItem { unique_id } => prepend_packet(
                ServerPacket::RepairItem { unique_id },
                repair_item_impl(self.app.world_mut(), unique_id, false),
            ),
            ClientPacket::SRepairItem { unique_id } => prepend_packet(
                ServerPacket::RepairItem { unique_id },
                repair_item_impl(self.app.world_mut(), unique_id, true),
            ),
            ClientPacket::MagicKey {
                spell,
                key,
                old_key,
            } => {
                if is_in_world(self.app.world()) {
                    assign_magic_key(self.app.world_mut(), spell, key, old_key);
                }
                Vec::new()
            }
            ClientPacket::Magic {
                spell,
                direction,
                target_id,
                location,
                ..
            } => {
                if !is_in_world(self.app.world()) {
                    return Vec::new();
                }
                let Some(skill_key) = skill_key_for_crystal_spell(spell) else {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                };
                if let Some(player) = player_entity(self.app.world()) {
                    self.app
                        .world_mut()
                        .entity_mut(player)
                        .insert(Facing(direction));
                }
                let packets = cast_skill_with_context(
                    self.app.world_mut(),
                    &skill_key,
                    Some(SkillCastContext {
                        direction,
                        target_id,
                        target: location,
                    }),
                );
                if packets.is_empty() {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                let mut response = vec![ServerPacket::UserLocation {
                    location: current_location(self.app.world()),
                }];
                response.extend(packets);
                response
            }
            ClientPacket::SpellToggle {
                spell,
                toggle_state,
            } => {
                if !is_in_world(self.app.world()) {
                    return Vec::new();
                }
                vec![ServerPacket::SpellToggle {
                    object_id: current_player_object_id(self.app.world()).unwrap_or_default(),
                    spell,
                    can_use: toggle_state > 0,
                }]
            }
            ClientPacket::CombineItem {
                grid,
                id_from,
                id_to,
            } => combine_item_impl(self.app.world_mut(), grid, id_from, id_to),
        }
    }

    pub(super) fn finalize_packets(&mut self, packets: Vec<ServerPacket>) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            self.visible_objects.clear();
            return packets;
        }

        let next_visible = collect_visible_objects(self.app.world());
        let next_object_ids = next_visible.keys().copied().collect::<BTreeSet<_>>();
        let previous_visible = self.visible_objects.clone();
        let self_object_id = current_player_object_id(self.app.world());
        let mut final_packets = Vec::new();

        for object_id in next_object_ids.difference(&previous_visible) {
            let bundle = next_visible
                .get(object_id)
                .expect("visible object bundle should exist");
            final_packets.push(bundle.spawn_packet.clone());
            if let Some(health_packet) = &bundle.health_packet {
                final_packets.push(health_packet.clone());
            }
        }

        final_packets.extend(packets.into_iter().filter(|packet| {
            should_emit_object_packet(packet, &previous_visible, &next_object_ids, self_object_id)
        }));

        for object_id in previous_visible.difference(&next_object_ids) {
            final_packets.push(ServerPacket::ObjectRemove {
                object_id: *object_id,
            });
        }

        self.visible_objects = next_object_ids;
        final_packets
    }
}
