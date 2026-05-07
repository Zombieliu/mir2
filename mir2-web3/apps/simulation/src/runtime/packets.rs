use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_base_stats_info_packet_payload, crystal_game_shop_info_packet_payloads,
    crystal_guild_buff_list_packet_payload, crystal_item_by_index,
    crystal_map_respawns_by_file_name, crystal_map_respawns_by_index, crystal_monster_by_index,
    crystal_npc_info_manifest, crystal_recipe_bootstrap_packets, format_localized_text,
    localized_text_or_fallback, LanguageCode,
};
use mir2_protocol::{
    decode_server_packet, encode_frame, ClientFriend, ClientHeroInformation,
    ClientIntelligentCreature, ClientMail, ClientPacket, MirClass, MirDirection, MirGender,
    MirGridType, MonsterInfo, NpcInfo, ObjectDiedInfo, ObjectGoldInfo, ObjectHealthInfo,
    ObjectItemInfo, ObjectManaInfo, ObjectMovement, ObjectPlayerInfo, ObjectRevivedInfo,
    ObjectSpellInfo, ObjectStruckInfo, Point, ServerPacket, ServerPacketId, Spell, StruckInfo,
    UserInformation, UserItem,
};

use crate::config::{
    CharacterRecord, EquipmentSlot, GroundDropLootSnapshot, GroundDropSnapshot, ItemContainer,
    MapTransferSnapshot, QuestStage, SimulationConfig, Stage5AuctionListing, Stage5HeroState,
    Stage5MailMessage, Stage5TradeState, WorldEntityDisposition, WorldEntityKind,
    WorldEntitySnapshot, WorldEntitySpriteSnapshot, WorldSnapshot,
};

use super::components::{
    current_player_object_id, entity_facing, entity_object_id, entity_player_vitals,
    entity_position, player_entity, CharacterBody, DisplayName, Facing, GeneralMeowMeowState,
    GroundDrop, HarvestMonsterState, Monster, MonsterAgent, MonsterAiState, MonsterVitals, Npc,
    NpcAgent, ObjectId, PlayerVitals, Position, RemotePlayer, SelfPlayer, SummonedMonster,
};
use super::crystal_compat::{
    BASE_STORAGE_SLOTS, BUFF_GENERAL_MEOW_MEOW_SHIELD, CRYSTAL_ITEM_TYPE_ARMOUR,
    CRYSTAL_ITEM_TYPE_BELT, CRYSTAL_ITEM_TYPE_BOOTS, CRYSTAL_ITEM_TYPE_BRACELET,
    CRYSTAL_ITEM_TYPE_HELMET, CRYSTAL_ITEM_TYPE_NECKLACE, CRYSTAL_ITEM_TYPE_RING,
    CRYSTAL_ITEM_TYPE_WEAPON,
};
use super::drops::*;
use super::drops::{
    crystal_drop_name_colour_argb, crystal_item_grade_for_key,
    crystal_item_name_colour_argb_for_drop, DropLoot, DropPayload,
};
use super::equipment::*;
use super::equipment::{
    equipment_shape, equipment_slot_index, user_item_from_equipment_state, EquipmentState,
};
use super::fishing::{fishing_cast_impl, fishing_change_autocast_impl};
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
use super::quests::{complete_quest, ensure_runtime_quest, quest_template_by_id, set_quest_stage};
use super::rental::{
    cancel_item_rental_impl, confirm_item_rental_impl, deposit_rental_item_impl,
    get_rented_items_impl, item_rental_fee_impl, item_rental_lock_fee_impl,
    item_rental_lock_item_impl, item_rental_period_impl, item_rental_request_impl,
    retrieve_rental_item_impl,
};
use super::resources::{
    intelligent_creature_default_rules, is_in_world, BuffResource, InventoryResource,
    ItemRentalResource, MapRuntimeResource, NpcStateResource, PlayerPermissionResource,
    PlayerRuntimeResource, QuestResource, RuntimeConfigResource, RuntimeQueueResource,
    SessionResource, SkillResource, Stage5SystemsResource,
};
use super::save::*;
use super::session::SimulationSession;
use super::skills::{
    assign_magic_key, cast_skill_with_context, skill_key_for_crystal_spell, SkillCastContext,
};
use super::stage5::{push_unique, push_unique_u8, stage5_item_name, stage5_player_name};

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

fn current_stage5_character_name(world: &World) -> String {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "Scout".to_string())
}

fn stage5_mail_cost(gold: u32, stamped: bool) -> u32 {
    if stamped {
        0
    } else {
        (gold / 1_000) * 100
    }
}

fn stage5_mail_to_client_mail(mail: &Stage5MailMessage) -> ClientMail {
    let message = if mail.subject.is_empty() {
        mail.body.clone()
    } else if mail.body.is_empty() {
        mail.subject.clone()
    } else {
        format!("{}\n{}", mail.subject, mail.body)
    };
    ClientMail {
        mail_id: u64::from(mail.id),
        sender_name: mail.from.clone(),
        message,
        opened: false,
        locked: false,
        can_reply: true,
        collected: mail.claimed,
        date_sent_binary_datetime: current_binary_datetime(),
        gold: mail.gold,
        items: Vec::new(),
    }
}

fn stage5_receive_mail_packet(world: &World) -> ServerPacket {
    let mail = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .iter()
        .filter(|mail| !mail.deleted)
        .map(stage5_mail_to_client_mail)
        .collect();
    ServerPacket::ReceiveMail { mail }
}

fn stage5_send_mail_packet(
    world: &mut World,
    name: String,
    message: String,
    gold: u32,
    items_idx: [u64; 5],
    stamped: bool,
) -> Vec<ServerPacket> {
    if name.trim().is_empty() || items_idx.iter().any(|item_idx| *item_idx != 0) {
        return vec![ServerPacket::MailSent { result: -1 }];
    }
    let cost = stage5_mail_cost(gold, stamped);
    let total = gold.saturating_add(cost);
    if world.resource::<PlayerRuntimeResource>().gold < total {
        return vec![ServerPacket::MailSent { result: -1 }];
    }

    if total > 0 {
        world.resource_mut::<PlayerRuntimeResource>().gold -= total;
    }
    let from = current_stage5_character_name(world);
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let id = stage5
            .stage5_systems
            .mail
            .iter()
            .map(|mail| mail.id)
            .max()
            .unwrap_or(0)
            + 1;
        stage5.stage5_systems.mail.push(Stage5MailMessage {
            id,
            from,
            to: name,
            subject: String::new(),
            body: message,
            gold,
            items: Vec::new(),
            claimed: false,
            deleted: false,
        });
    }

    let mut packets = Vec::new();
    if total > 0 {
        packets.push(ServerPacket::LoseGold { gold: total });
    }
    packets.push(ServerPacket::MailSent { result: 0 });
    packets.push(stage5_receive_mail_packet(world));
    packets
}

fn stage5_collect_mail_packet(world: &mut World, mail_id: u64) -> Vec<ServerPacket> {
    let Some(mail_id) = u32::try_from(mail_id).ok() else {
        return vec![ServerPacket::ParcelCollected { result: -1 }];
    };
    let (mail_index, gold, items) = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        let Some(mail_index) = stage5
            .stage5_systems
            .mail
            .iter()
            .position(|mail| mail.id == mail_id && !mail.deleted)
        else {
            return vec![ServerPacket::ParcelCollected { result: -1 }];
        };
        let mail = &stage5.stage5_systems.mail[mail_index];
        if mail.claimed {
            return vec![ServerPacket::ParcelCollected { result: 0 }];
        }
        (mail_index, mail.gold, mail.items.clone())
    };
    {
        let inventory = world.resource::<InventoryResource>();
        for item_key in &items {
            if !can_gain_item_quantity(inventory, ItemContainer::Bag1, item_key, 1) {
                return vec![ServerPacket::ParcelCollected { result: -1 }];
            }
        }
    }

    if gold > 0 {
        world.resource_mut::<PlayerRuntimeResource>().gold = world
            .resource::<PlayerRuntimeResource>()
            .gold
            .saturating_add(gold);
    }
    for item_key in items {
        add_or_increment_item(
            world,
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Crystal mail attachment.",
            20,
            1,
            1,
        );
    }
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .mail[mail_index]
        .claimed = true;

    let mut packets = Vec::new();
    if gold > 0 {
        packets.push(ServerPacket::GainedGold { gold });
    }
    packets.push(ServerPacket::ParcelCollected { result: 0 });
    packets.push(stage5_receive_mail_packet(world));
    packets
}

fn stage5_delete_mail_packet(world: &mut World, mail_id: u64) -> Vec<ServerPacket> {
    if let Ok(mail_id) = u32::try_from(mail_id) {
        if let Some(mail) = world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail
            .iter_mut()
            .find(|mail| mail.id == mail_id)
        {
            mail.deleted = true;
        }
    }
    vec![stage5_receive_mail_packet(world)]
}

fn stage5_friend_entries(world: &World) -> Vec<ClientFriend> {
    let social = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .social;
    social
        .friends
        .iter()
        .map(|name| (name, false))
        .chain(social.blocked.iter().map(|name| (name, true)))
        .enumerate()
        .map(|(index, (name, blocked))| ClientFriend {
            index: index as i32,
            name: name.clone(),
            memo: social.memos.get(name).cloned().unwrap_or_default(),
            blocked,
            online: !blocked,
        })
        .collect()
}

fn stage5_friend_update_packet(world: &World) -> ServerPacket {
    ServerPacket::FriendUpdate {
        friends: stage5_friend_entries(world),
    }
}

fn stage5_add_friend_packet(world: &mut World, name: String, blocked: bool) -> Vec<ServerPacket> {
    if name.trim().is_empty() {
        return vec![stage5_friend_update_packet(world)];
    }
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        if blocked {
            push_unique(&mut stage5.stage5_systems.social.blocked, name.clone());
            stage5
                .stage5_systems
                .social
                .friends
                .retain(|friend| !friend.eq_ignore_ascii_case(&name));
        } else {
            push_unique(&mut stage5.stage5_systems.social.friends, name.clone());
            stage5
                .stage5_systems
                .social
                .blocked
                .retain(|blocked_name| !blocked_name.eq_ignore_ascii_case(&name));
        }
    }
    vec![stage5_friend_update_packet(world)]
}

fn stage5_remove_friend_packet(world: &mut World, character_index: i32) -> Vec<ServerPacket> {
    let name = stage5_friend_entries(world)
        .into_iter()
        .find(|friend| friend.index == character_index)
        .map(|friend| friend.name);
    if let Some(name) = name {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        stage5
            .stage5_systems
            .social
            .friends
            .retain(|friend| !friend.eq_ignore_ascii_case(&name));
        stage5
            .stage5_systems
            .social
            .blocked
            .retain(|blocked| !blocked.eq_ignore_ascii_case(&name));
        stage5.stage5_systems.social.memos.remove(&name);
    }
    vec![stage5_friend_update_packet(world)]
}

fn stage5_add_memo_packet(
    world: &mut World,
    character_index: i32,
    memo: String,
) -> Vec<ServerPacket> {
    let name = stage5_friend_entries(world)
        .into_iter()
        .find(|friend| friend.index == character_index)
        .map(|friend| friend.name);
    if let Some(name) = name {
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .social
            .memos
            .insert(name, memo);
    }
    vec![stage5_friend_update_packet(world)]
}

fn stage5_hero_info(index: i32, hero: &Stage5HeroState) -> ClientHeroInformation {
    ClientHeroInformation {
        index,
        name: hero.name.clone(),
        level: hero.level,
        class: hero.class,
        gender: hero.gender,
    }
}

fn stage5_current_hero_info(world: &World) -> Option<ClientHeroInformation> {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .map(|hero| stage5_hero_info(0, hero))
}

fn stage5_manage_heroes_packet(world: &World) -> ServerPacket {
    let current_hero = stage5_current_hero_info(world);
    ServerPacket::ManageHeroes {
        maximum_count: 1,
        current_hero: current_hero.clone(),
        heroes: Some(vec![current_hero]),
    }
}

fn stage5_new_hero_packet(
    world: &mut World,
    name: String,
    gender: MirGender,
    class: MirClass,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .is_some()
    {
        return vec![ServerPacket::NewHero { result: 1 }];
    }
    let hero = Stage5HeroState {
        name,
        level: 1,
        class,
        gender,
        behaviour: 0,
        experience: 0,
        spawned: true,
    };
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .hero = Some(hero.clone());
    let info = stage5_hero_info(0, &hero);
    vec![
        ServerPacket::NewHero { result: 0 },
        ServerPacket::NewHeroInfo {
            info,
            storage_index: -1,
        },
        stage5_manage_heroes_packet(world),
        ServerPacket::UpdateHeroSpawnState { state: 1 },
    ]
}

const STAGE5_TRADE_SLOT_COUNT: usize = 10;
const STAGE5_INTELLIGENT_CREATURE_FULLNESS_DECAY_TICKS: u64 = 10;
const STAGE5_INTELLIGENT_CREATURE_BLACKSTONE_TICK_MS: i64 = 1_000;
const STAGE5_INTELLIGENT_CREATURE_BLACKSTONE_CAP_MS: i64 = 24_000;

fn stage5_trade_items(world: &World) -> Vec<Option<UserItem>> {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let Some(trade) = stage5.stage5_systems.trade.as_ref() else {
        return vec![None; STAGE5_TRADE_SLOT_COUNT];
    };
    let inventory = world.resource::<InventoryResource>();
    let mut trade_items = vec![None; STAGE5_TRADE_SLOT_COUNT];
    for (trade_slot, inventory_index) in &trade.offered_slots {
        let Some(slot) = trade_items.get_mut(usize::from(*trade_slot)) else {
            continue;
        };
        *slot = inventory
            .inventory_items
            .iter()
            .find(|item| inventory_item_matches_index(item, *inventory_index))
            .map(user_item_from_item_state);
    }
    trade_items
}

fn stage5_trade_item_keys_for_slots(world: &World, slots: &BTreeMap<u8, u8>) -> Vec<String> {
    let inventory = world.resource::<InventoryResource>();
    let mut offered_items = Vec::new();
    for inventory_index in slots.values() {
        if let Some(item) = inventory
            .inventory_items
            .iter()
            .find(|item| inventory_item_matches_index(item, *inventory_index))
        {
            push_unique(&mut offered_items, item.key.clone());
        }
    }
    offered_items
}

fn current_group_map_name(world: &World) -> String {
    world
        .resource::<MapRuntimeResource>()
        .current_map
        .title
        .clone()
}

fn ensure_stage5_group_self_member(world: &mut World) -> String {
    let player_name = stage5_player_name(world);
    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    push_unique(
        &mut stage5.stage5_systems.group.members,
        player_name.clone(),
    );
    player_name
}

fn stage5_group_switch_packet(world: &mut World, allow_group: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let had_group = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let had_group = !stage5.stage5_systems.group.members.is_empty();
        stage5.stage5_systems.group.allow_group = allow_group;
        if !allow_group {
            stage5.stage5_systems.group.members.clear();
        }
        had_group
    };
    let mut packets = vec![ServerPacket::SwitchGroup { allow_group }];
    if !allow_group && had_group {
        packets.push(ServerPacket::DeleteGroup);
    }
    packets
}

fn stage5_group_add_member_packet(world: &mut World, name: String) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let target_name = name.trim().to_string();
    if target_name.is_empty() {
        return Vec::new();
    }
    let allow_group = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .group
        .allow_group;
    if !allow_group {
        return vec![ServerPacket::SwitchGroup { allow_group: false }];
    }
    let player_name = ensure_stage5_group_self_member(world);
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        push_unique(
            &mut stage5.stage5_systems.group.members,
            target_name.clone(),
        );
    }
    let member_map = current_group_map_name(world);
    let member_location = current_location(world).position;
    vec![
        ServerPacket::SwitchGroup { allow_group: true },
        ServerPacket::AddMember { name: player_name },
        ServerPacket::AddMember {
            name: target_name.clone(),
        },
        ServerPacket::GroupMembersMap {
            player_name: target_name.clone(),
            player_map: member_map,
        },
        ServerPacket::SendMemberLocation {
            member_name: target_name,
            member_location,
        },
    ]
}

fn stage5_group_del_member_packet(world: &mut World, name: String) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let target_name = name.trim().to_string();
    if target_name.is_empty() {
        return Vec::new();
    }
    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    let members = &mut stage5.stage5_systems.group.members;
    let before = members.len();
    members.retain(|member| !member.eq_ignore_ascii_case(&target_name));
    if members.len() == before {
        return Vec::new();
    }
    if members.len() <= 1 {
        members.clear();
        return vec![ServerPacket::DeleteGroup];
    }
    vec![ServerPacket::DeleteMember { name: target_name }]
}

fn stage5_group_invite_reply_packet(world: &mut World, accept_invite: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if !accept_invite {
        return vec![ServerPacket::DeleteGroup];
    }
    let player_name = ensure_stage5_group_self_member(world);
    vec![
        ServerPacket::SwitchGroup { allow_group: true },
        ServerPacket::AddMember { name: player_name },
    ]
}

const CRYSTAL_QUEST_STATE_ADD: u8 = 0;
const CRYSTAL_QUEST_STATE_UPDATE: u8 = 1;
const CRYSTAL_QUEST_STATE_REMOVE: u8 = 2;

fn stage5_quest_task_list(world: &World, quest_id: i32) -> Vec<String> {
    let language = world.resource::<SessionResource>().language;
    let Some(snapshot) = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)
        .map(|quest| quest.snapshot(language))
    else {
        return Vec::new();
    };
    let mut tasks = Vec::new();
    for task in [
        snapshot.objective,
        snapshot.progress_label,
        snapshot.tracker,
    ] {
        if !task.trim().is_empty() {
            push_unique(&mut tasks, task);
        }
    }
    tasks
}

fn stage5_quest_stage(world: &World, quest_id: i32) -> Option<QuestStage> {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)
        .map(|quest| quest.stage)
}

fn stage5_completed_quest_ids(world: &World) -> Vec<i32> {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .filter(|quest| quest.stage == QuestStage::Completed)
        .map(|quest| quest.quest_id)
        .collect()
}

fn stage5_quest_change_packet(
    world: &World,
    quest_id: i32,
    quest_state: u8,
    track_quest: bool,
    is_new: bool,
) -> ServerPacket {
    let stage = stage5_quest_stage(world, quest_id).unwrap_or(QuestStage::Available);
    ServerPacket::ChangeQuest {
        quest_id,
        task_list: stage5_quest_task_list(world, quest_id),
        taken: matches!(stage, QuestStage::InProgress | QuestStage::ReadyToTurnIn),
        completed: matches!(stage, QuestStage::ReadyToTurnIn | QuestStage::Completed),
        new: is_new,
        quest_state,
        track_quest,
    }
}

fn stage5_quest_remove_packet(quest_id: i32, completed: bool) -> ServerPacket {
    ServerPacket::ChangeQuest {
        quest_id,
        task_list: Vec::new(),
        taken: false,
        completed,
        new: false,
        quest_state: CRYSTAL_QUEST_STATE_REMOVE,
        track_quest: false,
    }
}

fn stage5_accept_quest_packet(world: &mut World, quest_id: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if quest_template_by_id(quest_id).is_none() {
        return vec![system_message_key(world, "server.CouldNotAcceptQuest")];
    }
    match ensure_runtime_quest(world, quest_id) {
        QuestStage::Available => {
            set_quest_stage(world, quest_id, QuestStage::InProgress);
            vec![stage5_quest_change_packet(
                world,
                quest_id,
                CRYSTAL_QUEST_STATE_ADD,
                true,
                true,
            )]
        }
        QuestStage::InProgress | QuestStage::ReadyToTurnIn => vec![stage5_quest_change_packet(
            world,
            quest_id,
            CRYSTAL_QUEST_STATE_UPDATE,
            true,
            false,
        )],
        QuestStage::Completed => vec![system_message_key(world, "server.QuestAlreadyCompleted")],
    }
}

fn stage5_finish_quest_packet(world: &mut World, quest_id: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if ensure_runtime_quest(world, quest_id) != QuestStage::ReadyToTurnIn {
        return Vec::new();
    }
    complete_quest(world, quest_id);
    if stage5_quest_stage(world, quest_id) != Some(QuestStage::Completed) {
        return vec![system_message_key(world, "server.CannotHandInQuestBagFull")];
    }
    vec![
        stage5_quest_remove_packet(quest_id, true),
        ServerPacket::CompleteQuest {
            completed_quests: stage5_completed_quest_ids(world),
        },
    ]
}

fn stage5_abandon_quest_packet(world: &mut World, quest_id: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let Some(stage) = stage5_quest_stage(world, quest_id) else {
        return Vec::new();
    };
    if matches!(stage, QuestStage::Available | QuestStage::Completed) {
        return Vec::new();
    }
    {
        let mut quests = world.resource_mut::<QuestResource>();
        if let Some(quest) = quests
            .quests
            .iter_mut()
            .find(|quest| quest.quest_id == quest_id)
        {
            quest.stage = QuestStage::Available;
            quest.current = 0;
        }
    }
    if let Some(template) = quest_template_by_id(quest_id) {
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .retain(|item| {
                item.container != ItemContainer::Quest || item.key != template.quest_item.key
            });
    }
    vec![stage5_quest_remove_packet(quest_id, false)]
}

fn stage5_share_quest_packet(world: &mut World, quest_id: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if quest_template_by_id(quest_id).is_none() {
        return vec![system_message_key(world, "server.CouldNotAcceptQuest")];
    }
    let sharer_name = stage5_player_name(world);
    let has_target = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .group
        .members
        .iter()
        .any(|member| !member.eq_ignore_ascii_case(&sharer_name));
    if !has_target {
        return vec![system_message_key(world, "server.QuestNotShared")];
    }
    vec![ServerPacket::ShareQuest {
        quest_index: quest_id,
        sharer_name,
    }]
}

fn stage5_market_success_key_args<I, S>(world: &World, key: &str, args: I) -> ServerPacket
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    ServerPacket::MarketSuccess {
        message: format_localized_text(super::session::current_language(world), key, args),
    }
}

fn stage5_market_listing_count_packet(world: &World, match_text: &str) -> Vec<ServerPacket> {
    let normalized = match_text.replace(' ', "").to_ascii_lowercase();
    let count = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .auction
        .iter()
        .filter(|listing| !listing.sold && !listing.cancelled && !listing.expired)
        .filter(|listing| {
            normalized.is_empty()
                || listing
                    .item_key
                    .replace('-', "")
                    .to_ascii_lowercase()
                    .contains(&normalized)
                || stage5_item_name(&listing.item_key)
                    .replace(' ', "")
                    .to_ascii_lowercase()
                    .contains(&normalized)
        })
        .count();
    vec![ServerPacket::MarketSuccess {
        message: format!("{count} market listings matched."),
    }]
}

fn stage5_consign_item_packet(
    world: &mut World,
    unique_id: u64,
    price: u32,
    _market_type: u8,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if price == 0 {
        return vec![
            ServerPacket::ConsignItem {
                unique_id,
                success: false,
            },
            ServerPacket::MarketFail { reason: 1 },
        ];
    }
    let Some(item) = world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|item| item_matches_inventory_unique_id(item, unique_id))
        .cloned()
    else {
        return vec![
            ServerPacket::ConsignItem {
                unique_id,
                success: false,
            },
            ServerPacket::MarketFail { reason: 1 },
        ];
    };
    let seller = stage5_player_name(world);
    let id = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        stage5
            .stage5_systems
            .auction
            .iter()
            .map(|listing| listing.id)
            .max()
            .unwrap_or(0)
            + 1
    };
    world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .retain(|candidate| !item_matches_inventory_unique_id(candidate, unique_id));
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .auction
        .push(Stage5AuctionListing {
            id,
            seller,
            item_key: item.key.clone(),
            price,
            sold: false,
            cancelled: false,
            expired: false,
        });
    vec![
        ServerPacket::ConsignItem {
            unique_id,
            success: true,
        },
        ServerPacket::MarketSuccess {
            message: format!("Listed {} for {price} Gold.", item.name),
        },
    ]
}

fn stage5_market_buy_packet(
    world: &mut World,
    auction_id: u64,
    bid_price: u32,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let Ok(listing_id) = u32::try_from(auction_id) else {
        return vec![ServerPacket::MarketFail { reason: 7 }];
    };
    let buyer = stage5_player_name(world);
    let Some(index) = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .auction
        .iter()
        .position(|listing| listing.id == listing_id && !listing.cancelled)
    else {
        return vec![ServerPacket::MarketFail { reason: 7 }];
    };
    let listing = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .auction[index]
        .clone();
    if listing.sold {
        return vec![ServerPacket::MarketFail { reason: 2 }];
    }
    if listing.expired {
        return vec![ServerPacket::MarketFail { reason: 3 }];
    }
    if listing.seller.eq_ignore_ascii_case(&buyer) {
        return vec![ServerPacket::MarketFail { reason: 6 }];
    }
    let price = if bid_price > 0 {
        bid_price
    } else {
        listing.price
    };
    if world.resource::<PlayerRuntimeResource>().gold < price {
        return vec![ServerPacket::MarketFail { reason: 4 }];
    }
    {
        let inventory = world.resource::<InventoryResource>();
        if !can_gain_item_quantity(&inventory, ItemContainer::Bag1, &listing.item_key, 1) {
            return vec![ServerPacket::MarketFail { reason: 5 }];
        }
    }
    world.resource_mut::<PlayerRuntimeResource>().gold -= price;
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .auction[index]
        .sold = true;
    add_or_increment_item(
        world,
        ItemContainer::Bag1,
        &listing.item_key,
        &stage5_item_name(&listing.item_key),
        "Stage 5 market purchase.",
        21,
        1,
        1,
    );
    vec![
        ServerPacket::LoseGold { gold: price },
        stage5_market_success_key_args(
            world,
            "server.BoughtItemForGold",
            [stage5_item_name(&listing.item_key), price.to_string()],
        ),
    ]
}

fn stage5_market_get_back_packet(
    world: &mut World,
    _mode: u8,
    auction_id: u64,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let seller = stage5_player_name(world);
    let listing_index = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        if auction_id == 0 {
            stage5
                .stage5_systems
                .auction
                .iter()
                .position(|listing| listing.seller.eq_ignore_ascii_case(&seller))
        } else {
            let Ok(listing_id) = u32::try_from(auction_id) else {
                return vec![ServerPacket::MarketFail { reason: 7 }];
            };
            stage5.stage5_systems.auction.iter().position(|listing| {
                listing.id == listing_id && listing.seller.eq_ignore_ascii_case(&seller)
            })
        }
    };
    let Some(index) = listing_index else {
        return vec![ServerPacket::MarketFail { reason: 7 }];
    };
    let listing = world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .auction
        .remove(index);
    if listing.sold {
        world.resource_mut::<PlayerRuntimeResource>().gold += listing.price;
        return vec![
            ServerPacket::GainedGold {
                gold: listing.price,
            },
            stage5_market_success_key_args(
                world,
                "server.SoldItemEarningsCommission",
                [
                    stage5_item_name(&listing.item_key),
                    listing.price.to_string(),
                    listing.price.to_string(),
                    "0".to_string(),
                ],
            ),
        ];
    }
    {
        let inventory = world.resource::<InventoryResource>();
        if !can_gain_item_quantity(&inventory, ItemContainer::Bag1, &listing.item_key, 1) {
            world
                .resource_mut::<Stage5SystemsResource>()
                .stage5_systems
                .auction
                .push(listing);
            return vec![ServerPacket::MarketFail { reason: 5 }];
        }
    }
    add_or_increment_item(
        world,
        ItemContainer::Bag1,
        &listing.item_key,
        &stage5_item_name(&listing.item_key),
        "Stage 5 market return.",
        21,
        1,
        1,
    );
    vec![ServerPacket::MarketSuccess {
        message: format!(
            "Returned {} from market.",
            stage5_item_name(&listing.item_key)
        ),
    }]
}

fn stage5_market_sell_now_packet(world: &mut World, auction_id: u64) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let Ok(listing_id) = u32::try_from(auction_id) else {
        return vec![ServerPacket::MarketFail { reason: 7 }];
    };
    let seller = stage5_player_name(world);
    let has_listing = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .auction
        .iter()
        .any(|listing| listing.id == listing_id && listing.seller.eq_ignore_ascii_case(&seller));
    vec![ServerPacket::MarketFail {
        reason: if has_listing { 9 } else { 7 },
    }]
}

fn stage5_refine_slot(value: i32) -> Option<u8> {
    u8::try_from(value).ok().filter(|slot| *slot < 16)
}

fn stage5_deposit_refine_item_packet(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let Some(from_slot) = stage5_refine_slot(from) else {
        return vec![ServerPacket::DepositRefineItem {
            from,
            to,
            success: false,
        }];
    };
    let Some(to_slot) = stage5_refine_slot(to) else {
        return vec![ServerPacket::DepositRefineItem {
            from,
            to,
            success: false,
        }];
    };
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .refine
        .slots
        .contains_key(&to_slot)
    {
        return vec![ServerPacket::DepositRefineItem {
            from,
            to,
            success: false,
        }];
    }
    let Some(item) = ({
        let mut inventory = world.resource_mut::<InventoryResource>();
        inventory
            .inventory_items
            .iter()
            .position(|item| inventory_item_matches_index(item, from_slot))
            .map(|index| inventory.inventory_items.remove(index))
    }) else {
        return vec![ServerPacket::DepositRefineItem {
            from,
            to,
            success: false,
        }];
    };
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .refine
        .slots
        .insert(to_slot, item.key);
    vec![ServerPacket::DepositRefineItem {
        from,
        to,
        success: true,
    }]
}

fn stage5_retrieve_refine_item_packet(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let Some(from_slot) = stage5_refine_slot(from) else {
        return vec![ServerPacket::RetrieveRefineItem {
            from,
            to,
            success: false,
        }];
    };
    let Some(item_key) = world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .refine
        .slots
        .remove(&from_slot)
    else {
        return vec![ServerPacket::RetrieveRefineItem {
            from,
            to,
            success: false,
        }];
    };
    let preferred_slot = u8::try_from(to).unwrap_or(0);
    add_or_increment_item(
        world,
        ItemContainer::Bag1,
        &item_key,
        &stage5_item_name(&item_key),
        "Stage 5 refine return.",
        preferred_slot,
        1,
        1,
    );
    vec![ServerPacket::RetrieveRefineItem {
        from,
        to,
        success: true,
    }]
}

fn stage5_refine_cancel_packet(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let returned = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.refine.refining = false;
        stage5.stage5_systems.refine.ready = false;
        stage5.stage5_systems.refine.current_item = None;
        std::mem::take(&mut stage5.stage5_systems.refine.slots)
    };
    let mut packets = vec![ServerPacket::RefineCancel];
    for (slot, item_key) in returned {
        add_or_increment_item(
            world,
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Stage 5 refine cancel return.",
            slot,
            1,
            1,
        );
        packets.push(ServerPacket::RetrieveRefineItem {
            from: i32::from(slot),
            to: i32::from(slot),
            success: true,
        });
    }
    packets
}

fn stage5_refine_item_packet(world: &mut World, unique_id: u64) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let Some(item_key) = world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|item| item_matches_inventory_unique_id(item, unique_id))
        .map(|item| item.key.clone())
    else {
        return vec![ServerPacket::NPCCollectRefine { success: false }];
    };
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.refine.current_item = Some(item_key);
        stage5.stage5_systems.refine.refining = true;
        stage5.stage5_systems.refine.ready = true;
        stage5.stage5_systems.refine.slots.clear();
    }
    vec![ServerPacket::RefineItem { unique_id }]
}

fn stage5_check_refine_packet(world: &mut World, unique_id: u64) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let mut inventory = world.resource_mut::<InventoryResource>();
    let Some(item) = inventory
        .inventory_items
        .iter_mut()
        .find(|item| item_matches_inventory_unique_id(item, unique_id))
    else {
        return vec![ServerPacket::NPCCollectRefine { success: false }];
    };
    item.added_attack += 1;
    drop(inventory);
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .refine = Default::default();
    vec![
        ServerPacket::RefineItem { unique_id },
        system_message("Refine check succeeded."),
    ]
}

fn stage5_open_door_packet(world: &mut World, door_index: u8) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    push_unique_u8(
        &mut world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .conquest
            .open_gates,
        door_index,
    );
    vec![ServerPacket::OpenDoor {
        door_index,
        close: false,
    }]
}

fn request_map_info_packet(world: &World, map_index: i32) -> Vec<ServerPacket> {
    let mut info = world.resource::<MapRuntimeResource>().current_map.clone();
    if let Some(map) = crystal_map_respawns_by_index(map_index) {
        info.map_index = map.map_index;
        info.file_name = map.map_file_name;
        info.title = map.map_title;
        info.mini_map = map.mini_map;
        info.big_map = map.big_map;
        info.lights = map.light;
    }
    vec![ServerPacket::MapInformation { info }]
}

fn request_monster_info_packet(monster_index: i32) -> Vec<ServerPacket> {
    let Some(monster) = crystal_monster_by_index(monster_index) else {
        return Vec::new();
    };
    vec![ServerPacket::NewMonsterInfo {
        info: MonsterInfo {
            object_id: 0,
            name: monster.name,
            name_colour_argb: -1,
            location: Point { x: 0, y: 0 },
            image: monster.image,
            direction: MirDirection::Down,
            effect: monster.effect,
            ai: monster.ai,
            light: monster.light,
            dead: false,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id: 0,
            rarity: if monster.is_boss { 1 } else { 0 },
            buffs: Vec::new(),
        },
    }]
}

fn request_npc_info_packet(npc_index: i32) -> Vec<ServerPacket> {
    let Some(npc) = crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| npc.npc_index == npc_index)
    else {
        return Vec::new();
    };
    let mut quest_ids = npc.collect_quest_indexes;
    for quest_id in npc.finish_quest_indexes {
        if !quest_ids.contains(&quest_id) {
            quest_ids.push(quest_id);
        }
    }
    vec![ServerPacket::NewNpcInfo {
        info: NpcInfo {
            object_id: npc.loaded_object_id.unwrap_or(npc.npc_index.max(0) as u32),
            name: npc.name,
            name_colour_argb: -1,
            image: npc.image,
            colour_argb: -1,
            location: npc.location,
            direction: MirDirection::Down,
            quest_ids,
        },
    }]
}

pub(super) fn stage5_trade_request_packet(
    world: &mut World,
    partner_name: Option<String>,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let partner = partner_name.unwrap_or_else(|| "Trader".to_string());
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade = Some(Stage5TradeState {
        partner: partner.clone(),
        offered_items: Vec::new(),
        offered_slots: BTreeMap::new(),
        offered_gold: 0,
        accepted: false,
        locked: false,
        completed: false,
    });
    vec![ServerPacket::TradeRequest { name: partner }]
}

fn stage5_trade_reply_packet(world: &mut World, accept_invite: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if !accept_invite {
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = None;
        return vec![ServerPacket::TradeCancel { unlock: false }];
    }
    let partner = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let trade = stage5
            .stage5_systems
            .trade
            .get_or_insert_with(|| Stage5TradeState {
                partner: "Trader".to_string(),
                offered_items: Vec::new(),
                offered_slots: BTreeMap::new(),
                offered_gold: 0,
                accepted: false,
                locked: false,
                completed: false,
            });
        trade.partner.clone()
    };
    vec![ServerPacket::TradeAccept { name: partner }]
}

fn stage5_trade_gold_packet(world: &mut World, amount: u32) -> Vec<ServerPacket> {
    if world.resource::<PlayerRuntimeResource>().gold < amount {
        return Vec::new();
    }
    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
        return Vec::new();
    };
    if trade.completed {
        return Vec::new();
    }
    trade.offered_gold = amount;
    trade.accepted = false;
    trade.locked = false;
    vec![ServerPacket::TradeGold { amount }]
}

fn stage5_deposit_trade_item_packet(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let (Some(from_slot), Some(to_slot)) = (u8::try_from(from).ok(), u8::try_from(to).ok()) else {
        return vec![ServerPacket::DepositTradeItem {
            from,
            to,
            success: false,
        }];
    };
    let item_key = {
        let inventory = world.resource::<InventoryResource>();
        let Some(item) = inventory
            .inventory_items
            .iter()
            .find(|item| inventory_item_matches_index(item, from_slot))
        else {
            return vec![ServerPacket::DepositTradeItem {
                from,
                to,
                success: false,
            }];
        };
        item.key.clone()
    };
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
            return vec![ServerPacket::DepositTradeItem {
                from,
                to,
                success: false,
            }];
        };
        if trade.completed
            || trade.locked
            || usize::from(to_slot) >= STAGE5_TRADE_SLOT_COUNT
            || trade.offered_slots.contains_key(&to_slot)
            || trade.offered_slots.values().any(|slot| *slot == from_slot)
        {
            return vec![ServerPacket::DepositTradeItem {
                from,
                to,
                success: false,
            }];
        }
        trade.offered_slots.insert(to_slot, from_slot);
        push_unique(&mut trade.offered_items, item_key);
        trade.accepted = false;
    }
    vec![
        ServerPacket::DepositTradeItem {
            from,
            to,
            success: true,
        },
        ServerPacket::TradeItem {
            trade_items: stage5_trade_items(world),
        },
    ]
}

fn stage5_retrieve_trade_item_packet(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let Some(from_slot) = u8::try_from(from).ok() else {
        return vec![ServerPacket::RetrieveTradeItem {
            from,
            to,
            success: false,
        }];
    };
    let offered_slots = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
            return vec![ServerPacket::RetrieveTradeItem {
                from,
                to,
                success: false,
            }];
        };
        if trade.completed || trade.locked || trade.offered_slots.remove(&from_slot).is_none() {
            return vec![ServerPacket::RetrieveTradeItem {
                from,
                to,
                success: false,
            }];
        }
        trade.accepted = false;
        trade.offered_slots.clone()
    };
    let offered_items = stage5_trade_item_keys_for_slots(world, &offered_slots);
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        if let Some(trade) = stage5.stage5_systems.trade.as_mut() {
            trade.offered_items = offered_items;
        }
    }
    vec![
        ServerPacket::RetrieveTradeItem {
            from,
            to,
            success: true,
        },
        ServerPacket::TradeItem {
            trade_items: stage5_trade_items(world),
        },
    ]
}

pub(super) fn stage5_trade_confirm_packet(world: &mut World, locked: bool) -> Vec<ServerPacket> {
    if !locked {
        if let Some(trade) = world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .as_mut()
        {
            trade.accepted = false;
            trade.locked = false;
        }
        return vec![ServerPacket::TradeCancel { unlock: true }];
    }

    let (offered_gold, offered_slots) = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_ref() else {
            return Vec::new();
        };
        if trade.completed {
            return Vec::new();
        }
        (trade.offered_gold, trade.offered_slots.clone())
    };
    if world.resource::<PlayerRuntimeResource>().gold < offered_gold {
        return Vec::new();
    }
    {
        let inventory = world.resource::<InventoryResource>();
        if offered_slots.values().any(|inventory_index| {
            !inventory
                .inventory_items
                .iter()
                .any(|item| inventory_item_matches_index(item, *inventory_index))
        }) {
            return Vec::new();
        }
    }
    if offered_gold > 0 {
        world.resource_mut::<PlayerRuntimeResource>().gold -= offered_gold;
    }
    let offered_indices = offered_slots.values().copied().collect::<BTreeSet<_>>();
    world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .retain(|item| {
            inventory_index_for_item(item).is_none_or(|index| !offered_indices.contains(&index))
        });
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        if let Some(trade) = stage5.stage5_systems.trade.as_mut() {
            trade.accepted = true;
            trade.locked = true;
            trade.completed = true;
        }
    }

    let mut packets = Vec::new();
    if offered_gold > 0 {
        packets.push(ServerPacket::LoseGold { gold: offered_gold });
    }
    packets.push(ServerPacket::TradeConfirm);
    packets
}

pub(super) fn stage5_trade_cancel_packet(world: &mut World) -> Vec<ServerPacket> {
    let had_trade = world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade
        .take()
        .is_some();
    if had_trade {
        vec![ServerPacket::TradeCancel { unlock: false }]
    } else {
        Vec::new()
    }
}

pub(super) fn stage5_intelligent_creature_list_packet(world: &World) -> ServerPacket {
    let creatures = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creatures
        .clone();
    let summoned_creature_type = creatures
        .iter()
        .find(|creature| creature.pet_mode != 0)
        .map(|creature| creature.pet_type)
        .unwrap_or(0);
    ServerPacket::UpdateIntelligentCreatureList {
        creature_list: creatures,
        creature_summoned: summoned_creature_type != 0,
        summoned_creature_type,
        pearl_count: 0,
    }
}

fn stage5_update_intelligent_creature_packet(
    world: &mut World,
    mut creature: ClientIntelligentCreature,
    summon_me: bool,
    unsummon_me: bool,
    release_me: bool,
) -> Vec<ServerPacket> {
    if release_me {
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creatures
            .retain(|existing| existing.slot_index != creature.slot_index);
        return vec![stage5_intelligent_creature_list_packet(world)];
    }
    if summon_me {
        creature.pet_mode = creature.pet_mode.max(1);
    }
    if unsummon_me {
        creature.pet_mode = 0;
    }
    creature.creature_rules = intelligent_creature_default_rules(creature.pet_type);
    let was_new = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let creatures = &mut stage5.stage5_systems.intelligent_creatures;
        if let Some(existing) = creatures
            .iter_mut()
            .find(|existing| existing.slot_index == creature.slot_index)
        {
            *existing = creature.clone();
            false
        } else {
            creatures.push(creature.clone());
            creatures.sort_by_key(|creature| creature.slot_index);
            true
        }
    };
    let mut packets = Vec::new();
    if was_new {
        packets.push(ServerPacket::NewIntelligentCreature { creature });
    }
    packets.push(stage5_intelligent_creature_list_packet(world));
    packets
}

#[allow(deprecated)]
pub(super) fn stage5_intelligent_creature_pickup_packet(
    world: &mut World,
    location: Point,
    mouse_mode: Option<bool>,
) -> Vec<ServerPacket> {
    let Some(active_creature) = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creatures
        .iter()
        .find(|creature| creature.pet_mode != 0)
    else {
        return Vec::new();
    };
    if active_creature.fullness < active_creature.creature_rules.minimal_fullness.max(0) {
        return Vec::new();
    }
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let picker_object_id = entity_object_id(world, player).expect("player object id");
    let player_position = entity_position(world, player).unwrap_or(location.clone());
    let target_location = match mouse_mode {
        Some(true) => {
            if !active_creature.creature_rules.mouse_pickup_enabled {
                return Vec::new();
            }
            location
        }
        Some(false) => {
            if !active_creature.creature_rules.semi_auto_pickup_enabled
                || active_creature.pet_mode != 1
            {
                return Vec::new();
            }
            player_position
        }
        None => location,
    };
    let Some((object_id, drop_entity)) = world
        .iter_entities()
        .filter_map(|entity| {
            if entity.get::<GroundDrop>().is_none() {
                return None;
            }
            let object_id = entity.get::<ObjectId>()?.0;
            let position = entity.get::<Position>()?.0.clone();
            (position == target_location).then_some((object_id, entity.id()))
        })
        .min_by_key(|(object_id, _)| *object_id)
    else {
        return Vec::new();
    };
    if !drop_ownership_allows_pickup(world, drop_entity, picker_object_id) {
        return vec![system_message_key(world, "server.CannotPickupNotOwner")];
    }
    let payload = {
        let entity = world.entity(drop_entity);
        entity.get::<DropPayload>().expect("drop payload").clone()
    };
    if !stage5_intelligent_creature_filter_allows_drop(active_creature, &payload) {
        return Vec::new();
    }
    let mut packets = vec![ServerPacket::IntelligentCreaturePickup { object_id }];
    match payload.loot {
        DropLoot::Gold(amount) => {
            if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), amount) {
                return Vec::new();
            }
            world.resource_mut::<PlayerRuntimeResource>().gold += amount;
            let _ = world.despawn(drop_entity);
            packets.push(ServerPacket::GainedGold { gold: amount });
        }
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
            ..
        } => {
            {
                let resources = world.resource::<InventoryResource>();
                if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &key, payload.quantity)
                {
                    return Vec::new();
                }
            }
            let gained_item = add_or_increment_item_with_random_metadata(
                world,
                ItemContainer::Bag1,
                &key,
                &name,
                &description,
                8,
                payload.quantity,
                weight,
                durability_current,
                durability_max,
                added_attack,
                added_defence,
                added_stats,
                cursed,
                socket_slots,
            );
            let _ = world.despawn(drop_entity);
            packets.push(ServerPacket::GainedItem {
                item: user_item_from_item_state(&gained_item),
            });
        }
    }
    packets
}

pub(super) fn tick_stage5_intelligent_creatures(
    world: &mut World,
    tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if !is_in_world(world) {
        return;
    }
    let mut changed = false;
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        for creature in &mut stage5.stage5_systems.intelligent_creatures {
            if creature.pet_mode == 0 {
                continue;
            }
            if creature.creature_rules.can_produce_blackstone {
                let next_blackstone_time = creature
                    .blackstone_time
                    .saturating_add(STAGE5_INTELLIGENT_CREATURE_BLACKSTONE_TICK_MS)
                    .min(STAGE5_INTELLIGENT_CREATURE_BLACKSTONE_CAP_MS);
                if next_blackstone_time != creature.blackstone_time {
                    creature.blackstone_time = next_blackstone_time;
                    changed = true;
                }
            }
            if tick % STAGE5_INTELLIGENT_CREATURE_FULLNESS_DECAY_TICKS == 0 && creature.fullness > 0
            {
                creature.fullness -= 1;
                creature.maintain_food_time = creature
                    .maintain_food_time
                    .saturating_sub(STAGE5_INTELLIGENT_CREATURE_BLACKSTONE_TICK_MS);
                changed = true;
            }
        }
    }
    if changed {
        packets.push(stage5_intelligent_creature_list_packet(world));
    }
    if let Some(location) = stage5_intelligent_creature_auto_pickup_location(world) {
        packets.extend(stage5_intelligent_creature_pickup_packet(
            world, location, None,
        ));
    }
}

#[allow(deprecated)]
fn stage5_intelligent_creature_auto_pickup_location(world: &World) -> Option<Point> {
    let creature = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creatures
        .iter()
        .find(|creature| {
            creature.pet_mode != 0
                && creature.creature_rules.auto_pickup_enabled
                && creature.fullness >= creature.creature_rules.minimal_fullness.max(0)
                && creature.creature_rules.auto_pickup_range > 0
        })?
        .clone();
    let player = player_entity(world)?;
    let picker_object_id = entity_object_id(world, player)?;
    let player_position = entity_position(world, player)?;
    let range = creature.creature_rules.auto_pickup_range;

    world
        .iter_entities()
        .filter_map(|entity| {
            if entity.get::<GroundDrop>().is_none() {
                return None;
            }
            let object_id = entity.get::<ObjectId>()?.0;
            let position = entity.get::<Position>()?.0.clone();
            let distance = (position.x - player_position.x)
                .abs()
                .max((position.y - player_position.y).abs());
            if distance > range {
                return None;
            }
            let payload = entity.get::<DropPayload>()?;
            if !stage5_intelligent_creature_filter_allows_drop(&creature, payload) {
                return None;
            }
            if !drop_ownership_allows_pickup(world, entity.id(), picker_object_id) {
                return None;
            }
            Some((distance, object_id, position))
        })
        .min_by_key(|(distance, object_id, _)| (*distance, *object_id))
        .map(|(_, _, position)| position)
}

fn stage5_intelligent_creature_filter_allows_drop(
    creature: &ClientIntelligentCreature,
    payload: &DropPayload,
) -> bool {
    match &payload.loot {
        DropLoot::Gold(_) => creature.filter.pet_pickup_all || creature.filter.pet_pickup_gold,
        DropLoot::InventoryItem { key, .. } => {
            if creature.pickup_grade > 0 {
                let drop_grade = crystal_item_grade_for_key(key);
                if drop_grade < creature.pickup_grade {
                    return false;
                }
            }
            if creature.filter.pet_pickup_all {
                return true;
            }
            let Some(template) = crystal_item_template_for_item_key(key) else {
                return creature.filter.pet_pickup_others;
            };
            match template.item_type {
                CRYSTAL_ITEM_TYPE_WEAPON => creature.filter.pet_pickup_weapons,
                CRYSTAL_ITEM_TYPE_ARMOUR => creature.filter.pet_pickup_armours,
                CRYSTAL_ITEM_TYPE_HELMET => creature.filter.pet_pickup_helmets,
                CRYSTAL_ITEM_TYPE_BOOTS => creature.filter.pet_pickup_boots,
                CRYSTAL_ITEM_TYPE_BELT => creature.filter.pet_pickup_belts,
                CRYSTAL_ITEM_TYPE_NECKLACE
                | CRYSTAL_ITEM_TYPE_BRACELET
                | CRYSTAL_ITEM_TYPE_RING => creature.filter.pet_pickup_accessories,
                _ => creature.filter.pet_pickup_others,
            }
        }
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
        | ServerPacket::ObjectHero { .. }
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
        packets.push(decode_crystal_payload(
            ServerPacketId::NewRecipeInfo,
            recipe.payload,
        ));
    }
    packets
}

pub(super) fn start_game_account_social_and_shop_packets() -> Vec<ServerPacket> {
    let mut packets = vec![
        ServerPacket::CompleteQuest {
            completed_quests: Vec::new(),
        },
        ServerPacket::ReceiveMail { mail: Vec::new() },
        ServerPacket::FriendUpdate {
            friends: Vec::new(),
        },
        ServerPacket::LoverUpdate {
            name: String::new(),
            date_binary_datetime: 0,
            map_name: String::new(),
            married_days: 0,
        },
        ServerPacket::MentorUpdate {
            name: String::new(),
            level: 0,
            online: false,
            mentee_exp: 0,
        },
    ];
    packets.extend(
        crystal_game_shop_info_packet_payloads()
            .into_iter()
            .map(|payload| decode_crystal_payload(ServerPacketId::GameShopInfo, payload)),
    );
    packets
}

pub(super) fn start_game_base_stats_packet(class: MirClass) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    if let Some(payload) = crystal_base_stats_info_packet_payload(class) {
        packets.push(decode_crystal_payload(
            ServerPacketId::BaseStatsInfo,
            payload,
        ));
    }
    packets
}

pub(super) fn start_game_post_visible_crystal_bootstrap_packets() -> Vec<ServerPacket> {
    vec![
        ServerPacket::TimeOfDay { lights: 4 },
        ServerPacket::ChangeAMode { mode: 0 },
        ServerPacket::ChangePMode { mode: 0 },
        ServerPacket::SwitchGroup { allow_group: false },
        ServerPacket::DefaultNPC { object_id: 0 },
        decode_crystal_payload(
            ServerPacketId::GuildBuffList,
            crystal_guild_buff_list_packet_payload(),
        ),
        ServerPacket::NPCUpdate { npc_id: 0 },
        ServerPacket::NPCUpdate { npc_id: 0 },
        ServerPacket::NPCUpdate { npc_id: 0 },
        ServerPacket::NPCResponse { page: Vec::new() },
        ServerPacket::NPCResponse { page: Vec::new() },
        ServerPacket::NPCResponse { page: Vec::new() },
    ]
}

pub(super) fn decode_crystal_payload(packet_id: ServerPacketId, payload: Vec<u8>) -> ServerPacket {
    let frame = encode_frame(packet_id as i16, &payload)
        .expect("generated Crystal packet payload should fit into a protocol frame");
    decode_server_packet(&frame).expect("generated Crystal packet payload should decode")
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
    hero: Option<&Stage5HeroState>,
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
        has_hero: hero.is_some(),
        hero_behaviour: hero.map(|hero| hero.behaviour).unwrap_or(0),
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
            ClientPacket::GetRentedItems => get_rented_items_impl(self.app.world()),
            ClientPacket::DepositRefineItem { from, to } => {
                stage5_deposit_refine_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::RetrieveRefineItem { from, to } => {
                stage5_retrieve_refine_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::RefineCancel => stage5_refine_cancel_packet(self.app.world_mut()),
            ClientPacket::RefineItem { unique_id } => {
                stage5_refine_item_packet(self.app.world_mut(), unique_id)
            }
            ClientPacket::CheckRefine { unique_id } => {
                stage5_check_refine_packet(self.app.world_mut(), unique_id)
            }
            ClientPacket::ReplaceWedRing { .. }
            | ClientPacket::TeleportToNpc { .. }
            | ClientPacket::SearchMap { .. }
            | ClientPacket::Inspect { .. }
            | ClientPacket::Observe { .. }
            | ClientPacket::ChangeAMode { .. }
            | ClientPacket::ChangePMode { .. }
            | ClientPacket::ChangeTrade { .. }
            | ClientPacket::CallNpc { .. }
            | ClientPacket::BuyItemBack { .. }
            | ClientPacket::SetAutoPotValue { .. }
            | ClientPacket::SetAutoPotItem { .. }
            | ClientPacket::TownRevive
            | ClientPacket::RequestUserName { .. }
            | ClientPacket::RequestChatItem { .. }
            | ClientPacket::EditGuildMember { .. }
            | ClientPacket::EditGuildNotice { .. }
            | ClientPacket::GuildInvite { .. }
            | ClientPacket::GuildNameReturn { .. }
            | ClientPacket::RequestGuildInfo { .. }
            | ClientPacket::GuildStorageGoldChange { .. }
            | ClientPacket::GuildStorageItemChange { .. }
            | ClientPacket::GuildWarReturn { .. }
            | ClientPacket::EquipSlotItem { .. }
            | ClientPacket::AcceptReincarnation
            | ClientPacket::CancelReincarnation
            | ClientPacket::AwakeningNeedMaterials { .. }
            | ClientPacket::AwakeningLockedItem { .. }
            | ClientPacket::Awakening { .. }
            | ClientPacket::DisassembleItem { .. }
            | ClientPacket::DowngradeAwakening { .. }
            | ClientPacket::ResetAddedItem { .. }
            | ClientPacket::GuildBuffUpdate { .. }
            | ClientPacket::NpcConfirmInput { .. }
            | ClientPacket::GameShopBuy { .. }
            | ClientPacket::ReportIssue { .. }
            | ClientPacket::GetRanking { .. }
            | ClientPacket::GuildTerritoryPage { .. }
            | ClientPacket::PurchaseGuildTerritory { .. } => Vec::new(),
            ClientPacket::CraftItem { .. } => vec![ServerPacket::CraftItem { success: false }],
            ClientPacket::DepositTradeItem { from, to } => {
                stage5_deposit_trade_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::RetrieveTradeItem { from, to } => {
                stage5_retrieve_trade_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::TakeBackHeroItem { from, to } => {
                vec![ServerPacket::TakeBackHeroItem {
                    from,
                    to,
                    success: false,
                }]
            }
            ClientPacket::TransferHeroItem { from, to } => {
                vec![ServerPacket::TransferHeroItem {
                    from,
                    to,
                    success: false,
                }]
            }
            ClientPacket::ConsignItem {
                unique_id,
                price,
                market_type,
            } => stage5_consign_item_packet(self.app.world_mut(), unique_id, price, market_type),
            ClientPacket::MarketSearch { match_text, .. } => {
                stage5_market_listing_count_packet(self.app.world(), &match_text)
            }
            ClientPacket::MarketRefresh => stage5_market_listing_count_packet(self.app.world(), ""),
            ClientPacket::MarketPage { .. } => {
                stage5_market_listing_count_packet(self.app.world(), "")
            }
            ClientPacket::MarketBuy {
                auction_id,
                bid_price,
            } => stage5_market_buy_packet(self.app.world_mut(), auction_id, bid_price),
            ClientPacket::MarketGetBack { mode, auction_id } => {
                stage5_market_get_back_packet(self.app.world_mut(), mode, auction_id)
            }
            ClientPacket::MarketSellNow { auction_id } => {
                stage5_market_sell_now_packet(self.app.world_mut(), auction_id)
            }
            ClientPacket::OpenDoor { door_index } => {
                stage5_open_door_packet(self.app.world_mut(), door_index)
            }
            ClientPacket::RequestMapInfo { map_index } => {
                request_map_info_packet(self.app.world(), map_index)
            }
            ClientPacket::RequestMonsterInfo { monster_index } => {
                request_monster_info_packet(monster_index)
            }
            ClientPacket::RequestNpcInfo { npc_index } => request_npc_info_packet(npc_index),
            ClientPacket::MarriageRequest
            | ClientPacket::MarriageReply { .. }
            | ClientPacket::ChangeMarriage
            | ClientPacket::DivorceRequest
            | ClientPacket::DivorceReply { .. }
            | ClientPacket::AddMentor { .. }
            | ClientPacket::MentorReply { .. }
            | ClientPacket::AllowMentor
            | ClientPacket::CancelMentor => Vec::new(),
            ClientPacket::TradeRequest => stage5_trade_request_packet(self.app.world_mut(), None),
            ClientPacket::TradeReply { accept_invite } => {
                stage5_trade_reply_packet(self.app.world_mut(), accept_invite)
            }
            ClientPacket::TradeGold { amount } => {
                stage5_trade_gold_packet(self.app.world_mut(), amount)
            }
            ClientPacket::TradeConfirm { locked } => {
                stage5_trade_confirm_packet(self.app.world_mut(), locked)
            }
            ClientPacket::TradeCancel => stage5_trade_cancel_packet(self.app.world_mut()),
            ClientPacket::SwitchGroup { allow_group } => {
                stage5_group_switch_packet(self.app.world_mut(), allow_group)
            }
            ClientPacket::AddMember { name } => {
                stage5_group_add_member_packet(self.app.world_mut(), name)
            }
            ClientPacket::DelMember { name } => {
                stage5_group_del_member_packet(self.app.world_mut(), name)
            }
            ClientPacket::GroupInvite { accept_invite } => {
                stage5_group_invite_reply_packet(self.app.world_mut(), accept_invite)
            }
            ClientPacket::AcceptQuest { quest_index, .. } => {
                stage5_accept_quest_packet(self.app.world_mut(), quest_index)
            }
            ClientPacket::FinishQuest { quest_index, .. } => {
                stage5_finish_quest_packet(self.app.world_mut(), quest_index)
            }
            ClientPacket::AbandonQuest { quest_index } => {
                stage5_abandon_quest_packet(self.app.world_mut(), quest_index)
            }
            ClientPacket::ShareQuest { quest_index } => {
                stage5_share_quest_packet(self.app.world_mut(), quest_index)
            }
            ClientPacket::FishingCast { cast_out } => {
                fishing_cast_impl(self.app.world_mut(), cast_out)
            }
            ClientPacket::FishingChangeAutocast { auto_cast } => {
                fishing_change_autocast_impl(self.app.world_mut(), auto_cast)
            }
            ClientPacket::SendMail {
                name,
                message,
                gold,
                items_idx,
                stamped,
            } => stage5_send_mail_packet(
                self.app.world_mut(),
                name,
                message,
                gold,
                items_idx,
                stamped,
            ),
            ClientPacket::ReadMail { .. } => vec![stage5_receive_mail_packet(self.app.world())],
            ClientPacket::CollectParcel { mail_id } => {
                stage5_collect_mail_packet(self.app.world_mut(), mail_id)
            }
            ClientPacket::DeleteMail { mail_id } => {
                stage5_delete_mail_packet(self.app.world_mut(), mail_id)
            }
            ClientPacket::LockMail { .. } => vec![stage5_receive_mail_packet(self.app.world())],
            ClientPacket::MailLockedItem { unique_id, locked } => {
                vec![ServerPacket::MailLockedItem { unique_id, locked }]
            }
            ClientPacket::MailCost { gold, stamped, .. } => {
                vec![ServerPacket::MailCost {
                    cost: stage5_mail_cost(gold, stamped),
                }]
            }
            ClientPacket::RequestIntelligentCreatureUpdates { .. } => {
                vec![stage5_intelligent_creature_list_packet(self.app.world())]
            }
            ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            } => stage5_update_intelligent_creature_packet(
                self.app.world_mut(),
                creature,
                summon_me,
                unsummon_me,
                release_me,
            ),
            ClientPacket::IntelligentCreaturePickup {
                mouse_mode,
                location,
            } => stage5_intelligent_creature_pickup_packet(
                self.app.world_mut(),
                location,
                Some(mouse_mode),
            ),
            ClientPacket::AddFriend { name, blocked } => {
                stage5_add_friend_packet(self.app.world_mut(), name, blocked)
            }
            ClientPacket::RemoveFriend { character_index } => {
                stage5_remove_friend_packet(self.app.world_mut(), character_index)
            }
            ClientPacket::RefreshFriends => vec![stage5_friend_update_packet(self.app.world())],
            ClientPacket::AddMemo {
                character_index,
                memo,
            } => stage5_add_memo_packet(self.app.world_mut(), character_index, memo),
            ClientPacket::ItemRentalRequest => {
                item_rental_request_impl(self.app.world_mut(), None, false)
            }
            ClientPacket::ItemRentalFee { amount } => {
                item_rental_fee_impl(self.app.world_mut(), amount)
            }
            ClientPacket::ItemRentalPeriod { days } => {
                item_rental_period_impl(self.app.world_mut(), days)
            }
            ClientPacket::DepositRentalItem { from, to } => {
                deposit_rental_item_impl(self.app.world_mut(), from, to)
            }
            ClientPacket::RetrieveRentalItem { from, to } => {
                retrieve_rental_item_impl(self.app.world_mut(), from, to)
            }
            ClientPacket::CancelItemRental => cancel_item_rental_impl(self.app.world_mut()),
            ClientPacket::ItemRentalLockFee => item_rental_lock_fee_impl(self.app.world_mut()),
            ClientPacket::ItemRentalLockItem => item_rental_lock_item_impl(self.app.world_mut()),
            ClientPacket::ConfirmItemRental => confirm_item_rental_impl(self.app.world_mut()),
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
            ClientPacket::NewHero {
                name,
                gender,
                class,
            } => stage5_new_hero_packet(self.app.world_mut(), name, gender, class),
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
                    self.app
                        .world_mut()
                        .resource_mut::<ItemRentalResource>()
                        .active = None;
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
                if grid == MirGridType::Equipment {
                    return use_item(self.app.world_mut(), "", Some((unique_id, grid)));
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
            ClientPacket::SetHeroBehaviour { behaviour } => {
                let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
                let Some(hero) = resources.stage5_systems.hero.as_mut() else {
                    return Vec::new();
                };
                if hero.behaviour == behaviour {
                    return Vec::new();
                }
                hero.behaviour = behaviour;
                vec![ServerPacket::SetHeroBehaviour { behaviour }]
            }
            ClientPacket::ChangeHero { list_index } => {
                if self
                    .app
                    .world()
                    .resource::<Stage5SystemsResource>()
                    .stage5_systems
                    .hero
                    .is_none()
                {
                    return Vec::new();
                }
                if let Some(hero) = self
                    .app
                    .world_mut()
                    .resource_mut::<Stage5SystemsResource>()
                    .stage5_systems
                    .hero
                    .as_mut()
                {
                    hero.spawned = true;
                }
                vec![
                    ServerPacket::ChangeHero {
                        from_index: list_index,
                    },
                    stage5_manage_heroes_packet(self.app.world()),
                    ServerPacket::UpdateHeroSpawnState { state: 1 },
                ]
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
