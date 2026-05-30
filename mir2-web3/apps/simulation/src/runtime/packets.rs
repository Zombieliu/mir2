use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_base_stats_info_packet_payload, crystal_game_shop_info_packet_payloads,
    crystal_guild_buff_list_packet_payload, crystal_item_by_index, crystal_magic_by_spell,
    crystal_map_respawns_by_file_name, crystal_map_respawns_by_index, crystal_monster_by_index,
    crystal_npc_info_manifest, crystal_recipe_bootstrap_packets, format_localized_text,
    localized_text_or_fallback, LanguageCode,
};
use mir2_protocol::{
    decode_server_packet, encode_frame, ChatItem, ChatType, ClientBuff, ClientFriend, ClientGtMap,
    ClientHeroInformation, ClientIntelligentCreature, ClientMail, ClientPacket, GuildMember,
    GuildRank, GuildStorageItem, HeroUserInformation, MirClass, MirDirection, MirGender,
    MirGridType, MonsterInfo, NpcInfo, ObjectDiedInfo, ObjectGoldInfo, ObjectHealthInfo,
    ObjectItemInfo, ObjectManaInfo, ObjectMovement, ObjectPlayerInfo, ObjectRevivedInfo,
    ObjectSpellInfo, ObjectStruckInfo, Point, RankCharacterInfo, ServerPacket, ServerPacketId,
    Spell, StruckInfo, UserInformation, UserItem, UserItemStat,
};

use crate::config::{
    CharacterRecord, CharacterSaveRecord, EquipmentSlot, GroundDropLootSnapshot,
    GroundDropSnapshot, ItemContainer, MapTransferSnapshot, NpcScriptDiagnosticSnapshot,
    QuestStage, SimulationConfig, Stage5AuctionListing, Stage5HeroState,
    Stage5ItemRentalRecordSnapshot, Stage5ItemRentalSnapshot, Stage5MailMessage,
    Stage5SystemsState, Stage5TradeState, WorldEntityDisposition, WorldEntityKind,
    WorldEntitySnapshot, WorldEntitySpriteSnapshot, WorldSnapshot,
};

use super::combat::{
    crystal_player_attack_blocked_by_status, crystal_player_magic_blocked_by_status,
    crystal_player_movement_blocked_by_status, crystal_player_slowed_by_status,
};
use super::components::{
    current_hero_object_id, current_player_is_dead, current_player_object_id, entity_facing,
    entity_object_id, entity_player_vitals, entity_position, hero_entity, player_entity,
    CharacterBody, DisplayName, DropOwnership, Facing, GeneralMeowMeowState, GroundDrop,
    HarvestMonsterState, Hero, Monster, MonsterAgent, MonsterAiState, MonsterPoisonState,
    MonsterVitals, Npc, NpcAgent, ObjectId, PlayerVitals, Position, RemotePlayer, SelfPlayer,
    SummonedMonster,
};
use super::crystal_compat::{
    BASE_STORAGE_SLOTS, BUFF_GENERAL_MEOW_MEOW_SHIELD, CRYSTAL_BIND_DONT_STORE,
    CRYSTAL_BIND_NO_HERO, CRYSTAL_ITEM_TYPE_ARMOUR, CRYSTAL_ITEM_TYPE_BELT,
    CRYSTAL_ITEM_TYPE_BOOTS, CRYSTAL_ITEM_TYPE_BRACELET, CRYSTAL_ITEM_TYPE_HELMET,
    CRYSTAL_ITEM_TYPE_NECKLACE, CRYSTAL_ITEM_TYPE_POTION, CRYSTAL_ITEM_TYPE_RING,
    CRYSTAL_ITEM_TYPE_WEAPON, CRYSTAL_POTION_SHAPE_NORMAL, CRYSTAL_POTION_SHAPE_SUN_POTION,
    CRYSTAL_STAT_HP, CRYSTAL_STAT_MP,
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
use super::hero_ai::{hero_inventory_crystal_stat_total, hero_inventory_equipment_slots};
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
    active_scene_view, crystal_movement_transfer_records_for_map, current_map_disallows_drug,
    current_map_disallows_hero, filter_decor_objects, filter_terrain_patches, is_safe_zone_point,
    normalize_map_file_name, point_visible, rebuild_world, spawn_stage5_hero,
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
use super::quests::{
    begin_quest, can_accept_quest, complete_quest_with_selection, completed_quest_ids,
    crystal_quest_task_list, ensure_runtime_quest, quest_definition_exists, quest_log_snapshots,
    quest_template_by_id,
};
use super::rental::{
    cancel_item_rental_impl, confirm_item_rental_impl, deposit_rental_item_impl,
    get_rented_items_impl, item_rental_fee_impl, item_rental_lock_fee_impl,
    item_rental_lock_item_impl, item_rental_period_impl, item_rental_request_impl,
    retrieve_rental_item_impl,
};
use super::resources::{
    crystal_packet_action_ready, crystal_packet_attack_delay_ticks,
    crystal_packet_move_delay_ticks, crystal_packet_spell_delay_ticks,
    intelligent_creature_default_rules, is_in_world, mark_crystal_packet_action,
    queue_crystal_movement_retry, BuffResource, HeroInventoryResource, InventoryResource,
    ItemRentalResource, MapRuntimeResource, NpcStateResource, PlayerActionKind,
    PlayerPermissionResource, PlayerRuntimeResource, PotionRecoveryResource, QuestResource,
    RuntimeConfigResource, RuntimeQueueResource, SessionResource, SkillResource,
    Stage5SystemsResource,
};
use super::save::*;
use super::session::SimulationSession;
use super::skills::{
    assign_magic_key, cast_skill_with_context, skill_key_for_crystal_spell, SkillCastContext,
};
use super::social_economy::{
    social_blocks_outgoing_mail, stage5_market_sale_price, stage5_market_settlement,
    stage5_trade_item_can_enter,
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

fn current_stage5_character_index(world: &World) -> i32 {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.index)
        .unwrap_or_default()
}

fn stage5_mail_cost(gold: u32, stamped: bool) -> u32 {
    if stamped {
        0
    } else {
        (gold / 1_000) * 100
    }
}

#[derive(Debug, Clone)]
struct Stage5MailTarget {
    account_id: String,
    character_index: i32,
    character_name: String,
}

fn stage5_mail_target_for_name(config: &SimulationConfig, name: &str) -> Option<Stage5MailTarget> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let store = config.account_store.lock().ok()?;
    for (account_id, account) in &store.accounts {
        for character in &account.characters {
            if character.name.eq_ignore_ascii_case(name) {
                return Some(Stage5MailTarget {
                    account_id: account_id.clone(),
                    character_index: character.index,
                    character_name: character.name.clone(),
                });
            }
        }
    }
    None
}

fn stage5_mail_target_is_current(world: &World, target: &Stage5MailTarget) -> bool {
    let session = world.resource::<SessionResource>();
    let account_id = session.account_id.as_deref().unwrap_or("demo");
    account_id == target.account_id
        && session
            .selected_character
            .as_ref()
            .is_some_and(|character| character.index == target.character_index)
}

fn stage5_push_mail_to_local_world(world: &mut World, mut mail: Stage5MailMessage) {
    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    let id = stage5
        .stage5_systems
        .mail
        .iter()
        .map(|mail| mail.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    mail.id = id;
    stage5.stage5_systems.mail.push(mail);
}

fn stage5_push_mail_to_saved_character(
    config: &SimulationConfig,
    target: &Stage5MailTarget,
    mut mail: Stage5MailMessage,
) -> Result<(), String> {
    let mut store = config
        .account_store
        .lock()
        .map_err(|_| "account store mutex poisoned".to_string())?;
    let account = store
        .accounts
        .get_mut(&target.account_id)
        .ok_or_else(|| format!("mail target account {} not found", target.account_id))?;
    let character = account
        .characters
        .iter()
        .find(|character| character.index == target.character_index)
        .cloned()
        .ok_or_else(|| format!("mail target character {} not found", target.character_name))?;
    let save = account
        .saves
        .entry(character.index)
        .or_insert_with(|| CharacterSaveRecord::new(character.clone()));
    let mut systems = save
        .stage5_systems_json
        .as_deref()
        .and_then(|state| serde_json::from_str::<Stage5SystemsState>(state).ok())
        .unwrap_or_default();
    mail.id = systems
        .mail
        .iter()
        .map(|mail| mail.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    mail.to = character.name;
    systems.mail.push(mail);
    save.stage5_systems_json = Some(
        serde_json::to_string(&systems)
            .map_err(|error| format!("failed to encode stage5 mail: {error}"))?,
    );
    drop(store);
    config.save_account_store()
}

fn stage5_mail_attachment_ids(items_idx: &[u64; 5]) -> Option<Vec<u64>> {
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for unique_id in items_idx
        .iter()
        .copied()
        .filter(|unique_id| *unique_id != 0)
    {
        if !seen.insert(unique_id) {
            return None;
        }
        ids.push(unique_id);
    }
    Some(ids)
}

fn stage5_mail_attachment_states(world: &World, unique_ids: &[u64]) -> Option<Vec<ItemState>> {
    let inventory = world.resource::<InventoryResource>();
    let mut items = Vec::with_capacity(unique_ids.len());
    for unique_id in unique_ids {
        let item = inventory
            .inventory_items
            .iter()
            .find(|item| item_matches_inventory_unique_id(item, *unique_id))?
            .clone();
        items.push(item);
    }
    Some(items)
}

fn stage5_mail_attachment_user_items(mail: &Stage5MailMessage) -> Vec<UserItem> {
    let mut items = mail
        .item_states_json
        .iter()
        .filter_map(|state| serde_json::from_str::<ItemState>(state).ok())
        .map(|item| user_item_from_item_state(&item))
        .collect::<Vec<_>>();
    if !items.is_empty() {
        return items;
    }
    items.extend(mail.items.iter().enumerate().map(|(index, key)| {
        let unique_id = (u64::from(mail.id) << 32) | u64::try_from(index + 1).unwrap_or(1);
        stage5_guild_user_item_for_key(key, unique_id)
    }));
    items
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
        opened: mail.opened,
        locked: mail.locked,
        can_reply: true,
        collected: mail.claimed,
        date_sent_binary_datetime: current_binary_datetime(),
        gold: mail.gold,
        items: stage5_mail_attachment_user_items(mail),
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
    if name.trim().is_empty() {
        return vec![ServerPacket::MailSent { result: -1 }];
    }
    let Some(attachment_ids) = stage5_mail_attachment_ids(&items_idx) else {
        return vec![ServerPacket::MailSent { result: -1 }];
    };
    if social_blocks_outgoing_mail(
        &world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .social,
        &name,
    ) {
        return vec![system_message_key(
            world,
            "server.CannotMailPlayerOnBlacklist",
        )];
    }
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let Some(target) = stage5_mail_target_for_name(&config, &name) else {
        return vec![ServerPacket::MailSent { result: -1 }];
    };
    let Some(attachment_states) = stage5_mail_attachment_states(world, &attachment_ids) else {
        return vec![ServerPacket::MailSent { result: -1 }];
    };
    let cost = stage5_mail_cost(gold, stamped);
    let total = gold.saturating_add(cost);
    if world.resource::<PlayerRuntimeResource>().gold < total {
        return vec![ServerPacket::MailSent { result: -1 }];
    }

    let item_states_json = match attachment_states
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(states) => states,
        Err(_) => return vec![ServerPacket::MailSent { result: -1 }],
    };
    let mail = Stage5MailMessage {
        id: 0,
        from: current_stage5_character_name(world),
        to: target.character_name.clone(),
        subject: String::new(),
        body: message,
        gold,
        items: attachment_states
            .iter()
            .map(|item| item.key.clone())
            .collect(),
        item_states_json,
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    };
    if stage5_mail_target_is_current(world, &target) {
        stage5_push_mail_to_local_world(world, mail);
    } else if stage5_push_mail_to_saved_character(&config, &target, mail).is_err() {
        return vec![ServerPacket::MailSent { result: -1 }];
    }

    if total > 0 {
        world.resource_mut::<PlayerRuntimeResource>().gold -= total;
    }
    if !attachment_ids.is_empty() {
        let attachment_id_set = attachment_ids.iter().copied().collect::<BTreeSet<_>>();
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .retain(|item| !attachment_id_set.contains(&item_unique_id(item)));
    }

    let mut packets = Vec::new();
    if total > 0 {
        packets.push(ServerPacket::LoseGold { gold: total });
    }
    for item in attachment_states {
        packets.push(ServerPacket::DeleteItem {
            unique_id: item_unique_id(&item),
            count: item.quantity.min(u32::from(u16::MAX)) as u16,
        });
    }
    packets.push(ServerPacket::MailSent { result: 0 });
    packets.push(stage5_receive_mail_packet(world));
    packets
}

fn stage5_collect_mail_packet(world: &mut World, mail_id: u64) -> Vec<ServerPacket> {
    let Some(mail_id) = u32::try_from(mail_id).ok() else {
        return vec![ServerPacket::ParcelCollected { result: -1 }];
    };
    let (mail_index, gold, items, item_states_json) = {
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
        (
            mail_index,
            mail.gold,
            mail.items.clone(),
            mail.item_states_json.clone(),
        )
    };
    let item_states = item_states_json
        .iter()
        .filter_map(|state| serde_json::from_str::<ItemState>(state).ok())
        .collect::<Vec<_>>();
    let keyed_items = if item_states.is_empty() {
        items
    } else {
        Vec::new()
    };
    {
        let inventory = world.resource::<InventoryResource>();
        let free_slots =
            empty_slots_for_inventory_container(&inventory.inventory_items, ItemContainer::Bag1)
                .len();
        if item_states.len() > free_slots {
            return vec![ServerPacket::ParcelCollected { result: -1 }];
        }
        for item_key in &keyed_items {
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
    let mut gained_items = Vec::new();
    for mut item in item_states {
        let Some((container, slot)) = find_empty_inventory_item_slot(
            &world.resource::<InventoryResource>().inventory_items,
            ItemContainer::Bag1,
        ) else {
            return vec![ServerPacket::ParcelCollected { result: -1 }];
        };
        item.container = container;
        item.slot = slot;
        item.unique_id = allocate_item_unique_id(
            world.resource::<InventoryResource>(),
            item.container,
            item.slot,
        );
        gained_items.push(user_item_from_item_state(&item));
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
    }
    for item_key in keyed_items {
        let item = add_or_increment_item(
            world,
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Crystal mail attachment.",
            20,
            1,
            1,
        );
        gained_items.push(user_item_from_item_state(&item));
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
    for item in gained_items {
        packets.push(ServerPacket::GainedItem { item });
    }
    packets.push(ServerPacket::ParcelCollected { result: 0 });
    packets.push(stage5_receive_mail_packet(world));
    packets
}

fn stage5_read_mail_packet(world: &mut World, mail_id: u64) -> Vec<ServerPacket> {
    if let Ok(mail_id) = u32::try_from(mail_id) {
        if let Some(mail) = world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail
            .iter_mut()
            .find(|mail| mail.id == mail_id && !mail.deleted)
        {
            mail.opened = true;
        }
    }
    vec![stage5_receive_mail_packet(world)]
}

fn stage5_lock_mail_packet(world: &mut World, mail_id: u64, lock: bool) -> Vec<ServerPacket> {
    if let Ok(mail_id) = u32::try_from(mail_id) {
        if let Some(mail) = world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail
            .iter_mut()
            .find(|mail| mail.id == mail_id && !mail.deleted)
        {
            mail.locked = lock;
        }
    }
    vec![stage5_receive_mail_packet(world)]
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
            if !mail.locked {
                mail.deleted = true;
            }
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

fn stage5_hero_inventory_items(world: &World) -> Vec<Option<UserItem>> {
    let hero_inventory = world.resource::<HeroInventoryResource>();
    let mut items = vec![None; 40];
    for item in &hero_inventory.items {
        let Some(slot) = items.get_mut(usize::from(item.slot)) else {
            continue;
        };
        *slot = Some(user_item_from_item_state(item));
    }
    items
}

fn stage5_hero_information_packet(world: &World) -> Option<ServerPacket> {
    let hero = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()?;
    let object_id = current_hero_object_id(world).unwrap_or_else(|| {
        world
            .resource::<RuntimeConfigResource>()
            .config
            .object_id
            .saturating_add(1)
    });
    let level = hero.level.max(1);
    let hp = (60 + i32::from(level) * 6)
        .saturating_add(hero_inventory_crystal_stat_total(world, CRYSTAL_STAT_HP).max(0));
    let mp = (30 + i32::from(level) * 4)
        .saturating_add(hero_inventory_crystal_stat_total(world, CRYSTAL_STAT_MP).max(0));
    Some(ServerPacket::HeroInformation {
        info: HeroUserInformation {
            object_id,
            name: hero.name.clone(),
            class: hero.class,
            gender: hero.gender,
            level: hero.level,
            hair: 0,
            hp,
            mp,
            experience: i64::from(hero.experience),
            max_experience: i64::from(level) * 1_000,
            inventory: Some(stage5_hero_inventory_items(world)),
            equipment: Some(hero_inventory_equipment_slots(world)),
            magics: Vec::new(),
            auto_pot: hero.auto_pot,
            auto_hp_percent: hero.auto_hp_percent,
            auto_mp_percent: hero.auto_mp_percent,
            hp_item_index: hero.hp_item_index,
            mp_item_index: hero.mp_item_index,
        },
    })
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

fn stage5_hero_ready_for_inventory(world: &World) -> bool {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
        .is_some_and(|hero| hero.spawned)
        && current_hero_object_id(world).is_some()
}

fn player_item_unique_id_is_used(resources: &InventoryResource, unique_id: u64) -> bool {
    resources
        .belt_items
        .iter()
        .chain(resources.inventory_items.iter())
        .chain(resources.storage_items.iter())
        .any(|item| item_unique_id(item) == unique_id)
}

fn transfer_hero_item_packet(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::TransferHeroItem {
        from,
        to,
        success: false,
    };
    if !stage5_hero_ready_for_inventory(world) {
        return vec![failed_packet];
    }
    let Some(from_slot) = u8::try_from(from).ok() else {
        return vec![failed_packet];
    };
    let Some(to_slot) = u8::try_from(to).ok().filter(|slot| *slot < 40) else {
        return vec![failed_packet];
    };
    if !is_valid_inventory_slot(from_slot) {
        return vec![failed_packet];
    }
    if world
        .resource::<HeroInventoryResource>()
        .items
        .iter()
        .any(|item| item.slot == to_slot)
    {
        return vec![failed_packet];
    }

    let Some(source_index) = world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .position(|item| inventory_item_matches_index(item, from_slot))
    else {
        return vec![failed_packet];
    };
    let item_weight = {
        let inventory = world.resource::<InventoryResource>();
        inventory.inventory_items[source_index].total_weight()
    };
    if {
        let hero_inventory = world.resource::<HeroInventoryResource>();
        hero_inventory
            .items
            .iter()
            .map(ItemState::total_weight)
            .sum::<u32>()
            .saturating_add(item_weight)
            > CRYSTAL_BAG_WEIGHT_LIMIT
    } {
        return vec![
            system_message_key(world, "server.TooHeavyToTransfer"),
            failed_packet,
        ];
    }
    if {
        let inventory = world.resource::<InventoryResource>();
        let item = &inventory.inventory_items[source_index];
        item_has_crystal_or_rental_bind_flag(item, CRYSTAL_BIND_NO_HERO)
    } {
        return vec![failed_packet];
    }

    let mut item = {
        let mut inventory = world.resource_mut::<InventoryResource>();
        inventory.inventory_items.remove(source_index)
    };
    item.slot = to_slot;
    let mut hero_inventory = world.resource_mut::<HeroInventoryResource>();
    hero_inventory.items.push(item);
    vec![ServerPacket::TransferHeroItem {
        from,
        to,
        success: true,
    }]
}

fn take_back_hero_item_packet(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::TakeBackHeroItem {
        from,
        to,
        success: false,
    };
    if !stage5_hero_ready_for_inventory(world) {
        return vec![failed_packet];
    }
    let Some(from_slot) = u8::try_from(from).ok().filter(|slot| *slot < 40) else {
        return vec![failed_packet];
    };
    let Some(to_slot) = u8::try_from(to).ok() else {
        return vec![failed_packet];
    };
    let Some((to_container, to_inventory_slot)) = inventory_container_and_slot_for_index(to_slot)
    else {
        return vec![failed_packet];
    };
    if world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .any(|item| inventory_item_matches_index(item, to_slot))
    {
        return vec![failed_packet];
    }
    let Some(hero_index) = world
        .resource::<HeroInventoryResource>()
        .items
        .iter()
        .position(|item| item.slot == from_slot)
    else {
        return vec![failed_packet];
    };

    let mut item = {
        let mut hero_inventory = world.resource_mut::<HeroInventoryResource>();
        hero_inventory.items.remove(hero_index)
    };
    item.slot = to_inventory_slot;
    item.container = to_container;
    {
        let mut inventory = world.resource_mut::<InventoryResource>();
        if player_item_unique_id_is_used(&inventory, item_unique_id(&item)) {
            item.unique_id = allocate_item_unique_id(&inventory, item.container, item.slot);
        }
        inventory.inventory_items.push(item);
    }

    vec![ServerPacket::TakeBackHeroItem {
        from,
        to,
        success: true,
    }]
}

fn consume_hero_inventory_item_at_index(world: &mut World, item_index: usize) {
    let mut hero_inventory = world.resource_mut::<HeroInventoryResource>();
    let Some(item) = hero_inventory.items.get_mut(item_index) else {
        return;
    };
    if item.quantity > 1 {
        item.quantity -= 1;
    } else {
        hero_inventory.items.remove(item_index);
    }
}

fn hero_use_item_failed(unique_id: u64) -> Vec<ServerPacket> {
    vec![ServerPacket::UseItem {
        unique_id,
        success: false,
        grid: MirGridType::HeroInventory,
    }]
}

fn use_hero_inventory_item_packet(world: &mut World, unique_id: u64) -> Vec<ServerPacket> {
    if !stage5_hero_ready_for_inventory(world) {
        return hero_use_item_failed(unique_id);
    }
    if hero_entity(world)
        .and_then(|entity| world.entity(entity).get::<PlayerVitals>().copied())
        .is_some_and(|vitals| vitals.hp <= 0)
    {
        return hero_use_item_failed(unique_id);
    }
    let Some(hero_item_index) = world
        .resource::<HeroInventoryResource>()
        .items
        .iter()
        .position(|item| item_unique_id(item) == unique_id)
    else {
        return hero_use_item_failed(unique_id);
    };
    let item = world.resource::<HeroInventoryResource>().items[hero_item_index].clone();
    let ack = Some((unique_id, MirGridType::HeroInventory));
    let item_template = crystal_item_template_for_item_key(&item.key);
    let mut packets = Vec::new();

    if let Some(template) = item_template.as_ref() {
        if template.item_type == CRYSTAL_ITEM_TYPE_POTION && current_map_disallows_drug(world) {
            packets.push(system_message_key(world, "server.YouCannotUsePotionsHere"));
            return prepend_optional_packet(use_item_ack(ack, false), packets);
        }

        if template.item_type == CRYSTAL_ITEM_TYPE_POTION
            && template.shape == CRYSTAL_POTION_SHAPE_NORMAL
        {
            let hp = crystal_item_stat_value(template, CRYSTAL_STAT_HP).max(0);
            let mp = crystal_item_stat_value(template, CRYSTAL_STAT_MP).max(0);
            if !super::buffs::queue_crystal_normal_hero_potion_restore_amounts(world, hp, mp) {
                return prepend_optional_packet(use_item_ack(ack, false), packets);
            }
            consume_hero_inventory_item_at_index(world, hero_item_index);
            return prepend_optional_packet(use_item_ack(ack, true), packets);
        }

        if template.item_type == CRYSTAL_ITEM_TYPE_POTION
            && template.shape == CRYSTAL_POTION_SHAPE_SUN_POTION
        {
            let hp = crystal_item_stat_value(template, CRYSTAL_STAT_HP).max(0);
            let mp = crystal_item_stat_value(template, CRYSTAL_STAT_MP).max(0);
            if !super::buffs::restore_current_hero_vitals(world, hp, mp) {
                return prepend_optional_packet(use_item_ack(ack, false), packets);
            }
            consume_hero_inventory_item_at_index(world, hero_item_index);
            if let Some(hero) = hero_entity(world) {
                if let Some(info) = object_health_info_for_entity(world, hero, 0) {
                    packets.push(ServerPacket::ObjectHealth { info });
                }
                if mp > 0 {
                    if let Some(info) = object_mana_info_for_entity(world, hero) {
                        packets.push(ServerPacket::ObjectMana { info });
                    }
                }
            }
            return prepend_optional_packet(use_item_ack(ack, true), packets);
        }

        if let Some(magic) = crystal_learn_hero_book_magic(world, template) {
            packets.push(ServerPacket::NewMagic { magic, hero: true });
            consume_hero_inventory_item_at_index(world, hero_item_index);
            return prepend_optional_packet(use_item_ack(ack, true), packets);
        }
    }

    if item.heal_hp > 0 || item.heal_mp > 0 {
        if current_map_disallows_drug(world) {
            packets.push(system_message_key(world, "server.YouCannotUsePotionsHere"));
            return prepend_optional_packet(use_item_ack(ack, false), packets);
        }
        if !super::buffs::queue_crystal_normal_hero_potion_restore_amounts(
            world,
            item.heal_hp.max(0),
            item.heal_mp.max(0),
        ) {
            return prepend_optional_packet(use_item_ack(ack, false), packets);
        }
        consume_hero_inventory_item_at_index(world, hero_item_index);
        return prepend_optional_packet(use_item_ack(ack, true), packets);
    }

    prepend_optional_packet(use_item_ack(ack, false), packets)
}

fn stage5_hero_auto_pot_item_unique_id(world: &World, item_index: i32) -> Option<u64> {
    if item_index <= 0 {
        return None;
    }
    world
        .resource::<HeroInventoryResource>()
        .items
        .iter()
        .find(|item| crystal_item_index_for_item_state(item) == item_index)
        .map(item_unique_id)
}

fn stage5_hero_auto_pot_needs(current: i32, max: i32, threshold_percent: u8) -> bool {
    threshold_percent > 0
        && max > 0
        && i64::from(current.max(0)) * 100 < i64::from(max) * i64::from(threshold_percent)
}

pub(super) fn tick_stage5_hero_auto_pot(
    world: &mut World,
    _tick: u64,
    packets: &mut Vec<ServerPacket>,
) {
    if !stage5_hero_ready_for_inventory(world) {
        return;
    }
    let Some(vitals) =
        hero_entity(world).and_then(|entity| world.entity(entity).get::<PlayerVitals>().copied())
    else {
        return;
    };
    if vitals.hp <= 0 {
        return;
    }
    let (auto_pot, auto_hp_percent, auto_mp_percent, hp_item_index, mp_item_index) = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        let Some(hero) = stage5.stage5_systems.hero.as_ref() else {
            return;
        };
        (
            hero.auto_pot,
            hero.auto_hp_percent,
            hero.auto_mp_percent,
            hero.hp_item_index,
            hero.mp_item_index,
        )
    };
    if !auto_pot {
        return;
    }
    let (pending_hp, pending_mp) = {
        let recovery = world.resource::<PotionRecoveryResource>();
        (
            recovery.hero_pending_pot_health_amount,
            recovery.hero_pending_pot_mana_amount,
        )
    };

    if pending_hp <= 0 && stage5_hero_auto_pot_needs(vitals.hp, vitals.max_hp, auto_hp_percent) {
        if let Some(unique_id) = stage5_hero_auto_pot_item_unique_id(world, hp_item_index) {
            packets.extend(use_hero_inventory_item_packet(world, unique_id));
        }
    }
    if pending_mp <= 0 && stage5_hero_auto_pot_needs(vitals.mp, vitals.max_mp, auto_mp_percent) {
        if let Some(unique_id) = stage5_hero_auto_pot_item_unique_id(world, mp_item_index) {
            packets.extend(use_hero_inventory_item_packet(world, unique_id));
        }
    }
}

const HERO_SPAWN_STATE_NONE: u8 = 0;
const HERO_SPAWN_STATE_UNSUMMONED: u8 = 1;
const HERO_SPAWN_STATE_SUMMONED: u8 = 2;

fn stage5_hero_spawn_state(hero: Option<&Stage5HeroState>) -> u8 {
    match hero {
        Some(hero) if hero.spawned => HERO_SPAWN_STATE_SUMMONED,
        Some(_) => HERO_SPAWN_STATE_UNSUMMONED,
        None => HERO_SPAWN_STATE_NONE,
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
    let hero_can_spawn = !current_map_disallows_hero(world);
    let hero = Stage5HeroState {
        name,
        level: 1,
        class,
        gender,
        behaviour: 0,
        experience: 0,
        spawned: hero_can_spawn,
        auto_pot: true,
        auto_hp_percent: 0,
        auto_mp_percent: 0,
        hp_item_index: 0,
        mp_item_index: 0,
    };
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .hero = Some(hero.clone());
    if hero_can_spawn {
        let _ = spawn_stage5_hero(world);
    }
    let info = stage5_hero_info(0, &hero);
    let mut packets = vec![
        ServerPacket::NewHero { result: 0 },
        ServerPacket::NewHeroInfo {
            info,
            storage_index: -1,
        },
        stage5_hero_information_packet(world).expect("new hero should have information"),
        stage5_manage_heroes_packet(world),
        ServerPacket::UpdateHeroSpawnState {
            state: stage5_hero_spawn_state(Some(&hero)),
        },
    ];
    if !hero_can_spawn {
        packets.push(system_message_key(world, "server.CannotSummonHeroOnMap"));
    }
    packets
}

fn learned_spell_level(world: &World, spell: Spell) -> Option<u8> {
    let skill_key = skill_key_for_crystal_spell(spell)?;
    world
        .resource::<SkillResource>()
        .skills
        .iter()
        .find(|skill| skill.key == skill_key)
        .map(|skill| skill.level)
}

fn skill_toggle_state(world: &World, spell: Spell) -> bool {
    world
        .resource::<SkillResource>()
        .spell_toggles
        .iter()
        .find(|(candidate, _)| *candidate == spell)
        .map(|(_, enabled)| *enabled)
        .unwrap_or(false)
}

fn set_skill_toggle_state(world: &mut World, spell: Spell, enabled: bool) {
    let mut skills = world.resource_mut::<SkillResource>();
    if let Some((_, existing)) = skills
        .spell_toggles
        .iter_mut()
        .find(|(candidate, _)| *candidate == spell)
    {
        *existing = enabled;
    } else {
        skills.spell_toggles.push((spell, enabled));
    }
}

fn crystal_toggle_spell_is_stateful(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::Slaying
            | Spell::Thrusting
            | Spell::HalfMoon
            | Spell::CrossHalfMoon
            | Spell::DoubleSlash
    )
}

fn stage5_spell_toggle_packet(
    world: &mut World,
    spell: Spell,
    toggle_state: i8,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if toggle_state < 0 {
        let Some(hero_object_id) = current_hero_object_id(world) else {
            return Vec::new();
        };
        return vec![ServerPacket::SpellToggle {
            object_id: hero_object_id,
            spell,
            can_use: true,
        }];
    }
    if current_player_is_dead(world) || learned_spell_level(world, spell).is_none() {
        return Vec::new();
    }
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let can_use = toggle_state != 0;

    if crystal_toggle_spell_is_stateful(spell) {
        set_skill_toggle_state(world, spell, can_use);
        return vec![ServerPacket::SpellToggle {
            object_id,
            spell,
            can_use,
        }];
    }

    if spell == Spell::CounterAttack {
        if !can_use
            || world
                .resource::<BuffResource>()
                .buffs
                .iter()
                .any(|buff| buff.key == "counter-attack")
        {
            return Vec::new();
        }
        let level = learned_spell_level(world, spell).unwrap_or_default();
        let Some(magic) = crystal_magic_by_spell("CounterAttack") else {
            return Vec::new();
        };
        let mana_cost = i32::from(magic.base_cost) + i32::from(magic.level_cost) * i32::from(level);
        let Some(player) = player_entity(world) else {
            return Vec::new();
        };
        let current_mp = entity_player_vitals(world, player)
            .map(|vitals| vitals.mp)
            .unwrap_or_default();
        if current_mp <= mana_cost {
            return Vec::new();
        }
        {
            let mut entity = world.entity_mut(player);
            let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
            vitals.mp = (vitals.mp - mana_cost).max(0);
        }
        let stat_value = 11 + i32::from(level) * 3;
        let buff = super::buffs::BuffState {
            key: "counter-attack".to_string(),
            name: "Counter Attack".to_string(),
            description: "Crystal counter-attack stance is active.".to_string(),
            expires_at_tick: super::session::runtime_tick(world).saturating_add(7),
            attack_bonus: 0,
            defence_bonus: stat_value,
            stats: vec![
                UserItemStat {
                    stat: super::crystal_compat::CRYSTAL_STAT_MIN_AC,
                    value: stat_value,
                },
                UserItemStat {
                    stat: super::crystal_compat::CRYSTAL_STAT_MAX_AC,
                    value: stat_value,
                },
                UserItemStat {
                    stat: super::crystal_compat::CRYSTAL_STAT_MIN_MAC,
                    value: stat_value,
                },
                UserItemStat {
                    stat: super::crystal_compat::CRYSTAL_STAT_MAX_MAC,
                    value: stat_value,
                },
            ],
        };
        super::buffs::apply_or_refresh_buff(world, buff.clone());
        set_skill_toggle_state(world, spell, true);
        let mut packets = Vec::new();
        if let Some(packet) = super::buffs::client_buff_packet_for_state(world, &buff) {
            packets.push(packet);
        }
        if let Some(info) = object_mana_info_for_entity(world, player) {
            packets.push(ServerPacket::ObjectMana { info });
        }
        return packets;
    }

    if spell == Spell::MentalState {
        if !can_use {
            return Vec::new();
        }
        let next_state = {
            let mut skills = world.resource_mut::<SkillResource>();
            skills.mental_state = (skills.mental_state + 1) % 3;
            skills.mental_state
        };
        let buff = super::buffs::BuffState {
            key: "mental-state".to_string(),
            name: "Mental State".to_string(),
            description: "Crystal mental state is active.".to_string(),
            expires_at_tick: u64::MAX,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        };
        super::buffs::apply_or_refresh_buff(world, buff);
        return vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: super::buffs::crystal_buff_type_for_key("mental-state").unwrap_or(19),
                visible: false,
                object_id,
                expire_time: 0,
                infinite: true,
                paused: false,
                stats: Vec::new(),
                values: vec![i32::from(next_state)],
            },
        }];
    }

    if spell == Spell::FlamingSword {
        if skill_toggle_state(world, spell) || !can_use {
            return Vec::new();
        }
        let level = learned_spell_level(world, spell).unwrap_or_default();
        let Some(magic) = crystal_magic_by_spell("FlamingSword") else {
            return Vec::new();
        };
        let mana_cost = i32::from(magic.base_cost) + i32::from(magic.level_cost) * i32::from(level);
        let Some(player) = player_entity(world) else {
            return Vec::new();
        };
        let current_mp = entity_player_vitals(world, player)
            .map(|vitals| vitals.mp)
            .unwrap_or_default();
        if current_mp <= mana_cost {
            return Vec::new();
        }
        {
            let mut entity = world.entity_mut(player);
            let mut vitals = entity.get_mut::<PlayerVitals>().expect("player vitals");
            vitals.mp = (vitals.mp - mana_cost).max(0);
        }
        set_skill_toggle_state(world, spell, true);
        let mut packets = vec![ServerPacket::SpellToggle {
            object_id,
            spell,
            can_use: true,
        }];
        if let Some(info) = object_mana_info_for_entity(world, player) {
            packets.push(ServerPacket::ObjectMana { info });
        }
        return packets;
    }

    Vec::new()
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

fn stage5_guild_not_in_guild_chat(world: &World) -> ServerPacket {
    system_message(&localized_text_or_fallback(
        super::session::current_language(world),
        "server.NotPartOfGuild",
        "server.NotPartOfGuild",
    ))
}

const GUILD_STORAGE_SLOT_COUNT: usize = 112;
const GUILD_OPTION_CAN_CHANGE_RANK: u8 = 1;
const GUILD_OPTION_CAN_RECRUIT: u8 = 2;
const GUILD_OPTION_CAN_KICK: u8 = 4;
const GUILD_OPTION_CAN_STORE_ITEM: u8 = 8;
const GUILD_OPTION_CAN_RETRIEVE_ITEM: u8 = 16;
const GUILD_OPTION_CAN_ALTER_ALLIANCE: u8 = 32;
const GUILD_OPTION_CAN_CHANGE_NOTICE: u8 = 64;
const GUILD_OPTION_CAN_ACTIVATE_BUFF: u8 = 128;
const GUILD_OPTION_ALL: u8 = u8::MAX;
const STAGE5_GUILD_WAR_COST: u32 = 3_000;
const STAGE5_GUILD_WAR_TICK_INTERVAL: u64 = 60;
const STAGE5_GUILD_WAR_DURATION_TICKS: u64 = 180 * STAGE5_GUILD_WAR_TICK_INTERVAL;
const STAGE5_GUILD_WAR_SELF_COLOUR_ARGB: i32 = 0xFF00_00FFu32 as i32;
const STAGE5_GUILD_NORMAL_COLOUR_ARGB: i32 = -1;
const STAGE5_GUILD_TERRITORY_PAGE_SIZE: usize = 7;

fn stage5_guild_rank_is_leader(rank: &str) -> bool {
    let rank = rank.trim();
    rank.is_empty()
        || rank.eq_ignore_ascii_case("Guild Chief")
        || rank.eq_ignore_ascii_case("Leader")
}

fn stage5_current_guild_name(world: &World) -> String {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .trim()
        .to_string()
}

fn stage5_guild_permission_key(permission: &str) -> String {
    permission
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn stage5_guild_permissions_contain(permissions: &[String], candidates: &[&str]) -> bool {
    permissions.iter().any(|permission| {
        let permission = stage5_guild_permission_key(permission);
        candidates.iter().any(|candidate| permission == *candidate)
    })
}

fn stage5_guild_options_from_rank_and_permissions(rank: &str, permissions: &[String]) -> u8 {
    if stage5_guild_rank_is_leader(rank) {
        return GUILD_OPTION_ALL;
    }

    let mut options = 0;
    if stage5_guild_permissions_contain(permissions, &["changerank", "rank"]) {
        options |= GUILD_OPTION_CAN_CHANGE_RANK;
    }
    if stage5_guild_permissions_contain(permissions, &["recruit", "invite"]) {
        options |= GUILD_OPTION_CAN_RECRUIT;
    }
    if stage5_guild_permissions_contain(permissions, &["kick"]) {
        options |= GUILD_OPTION_CAN_KICK;
    }
    if stage5_guild_permissions_contain(permissions, &["storeitem", "store", "storage"]) {
        options |= GUILD_OPTION_CAN_STORE_ITEM;
    }
    if stage5_guild_permissions_contain(permissions, &["retrieveitem", "retrieve", "storage"]) {
        options |= GUILD_OPTION_CAN_RETRIEVE_ITEM;
    }
    if stage5_guild_permissions_contain(permissions, &["alteralliance", "alliance", "conquest"]) {
        options |= GUILD_OPTION_CAN_ALTER_ALLIANCE;
    }
    if stage5_guild_permissions_contain(permissions, &["changenotice", "notice"]) {
        options |= GUILD_OPTION_CAN_CHANGE_NOTICE;
    }
    if stage5_guild_permissions_contain(permissions, &["activatebuff", "buff"]) {
        options |= GUILD_OPTION_CAN_ACTIVATE_BUFF;
    }
    options
}

fn stage5_current_guild_rank_name(world: &World) -> String {
    let rank = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .rank
        .trim()
        .to_string();
    if rank.is_empty() {
        "Guild Chief".to_string()
    } else {
        rank
    }
}

fn stage5_current_guild_rank_index(world: &World) -> i32 {
    if stage5_guild_rank_is_leader(&stage5_current_guild_rank_name(world)) {
        0
    } else {
        1
    }
}

fn stage5_current_guild_options(world: &World) -> u8 {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let guild = &stage5.stage5_systems.guild;
    stage5_guild_options_from_rank_and_permissions(&guild.rank, &guild.permissions)
}

fn stage5_current_guild_has_option(world: &World, option: u8) -> bool {
    stage5_current_guild_options(world) & option != 0
}

fn stage5_guild_canonical_known_name(world: &World, name: &str) -> Option<String> {
    let target = name.trim();
    if target.is_empty() {
        return None;
    }
    let stage5 = world.resource::<Stage5SystemsResource>();
    let guild = &stage5.stage5_systems.guild;
    if guild.name.eq_ignore_ascii_case(target) {
        return Some(guild.name.clone());
    }
    if target.eq_ignore_ascii_case("NewbieGuild") {
        return Some("NewbieGuild".to_string());
    }
    guild
        .known_guilds
        .iter()
        .find(|known| known.eq_ignore_ascii_case(target))
        .cloned()
        .or_else(|| {
            let owner = stage5.stage5_systems.guild_territory.owner.trim();
            if owner.eq_ignore_ascii_case(target) {
                Some(stage5.stage5_systems.guild_territory.owner.clone())
            } else {
                None
            }
        })
}

fn stage5_guild_has_active_war(world: &World, name: &str) -> bool {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .active_wars
        .iter()
        .any(|war| war.eq_ignore_ascii_case(name))
}

pub(super) fn stage5_guild_is_at_war_with(world: &World, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    stage5_guild_has_active_war(world, name)
}

fn stage5_current_guild_is_at_war(world: &World) -> bool {
    !world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .active_wars
        .is_empty()
}

fn stage5_guild_current_colour_argb(world: &World) -> i32 {
    if stage5_current_guild_is_at_war(world) {
        STAGE5_GUILD_WAR_SELF_COLOUR_ARGB
    } else {
        STAGE5_GUILD_NORMAL_COLOUR_ARGB
    }
}

fn stage5_guild_end_war_packet(
    world: &mut World,
    enemy_name: String,
    packets: &mut Vec<ServerPacket>,
) {
    let guild_name = stage5_current_guild_name(world);
    let language = super::session::current_language(world);
    let current_message =
        format_localized_text(language, "server.WarEndedWithGuild", [enemy_name.clone()]);
    let enemy_message = format_localized_text(language, "server.WarEndedWithGuild", [guild_name]);
    let ended = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let guild = &mut stage5.stage5_systems.guild;
        let before = guild.active_wars.len();
        guild
            .active_wars
            .retain(|war| !war.eq_ignore_ascii_case(&enemy_name));
        guild
            .active_war_ticks_remaining
            .retain(|war, _| !war.eq_ignore_ascii_case(&enemy_name));
        let ended = guild.active_wars.len() != before;
        if ended {
            guild.war_broadcasts.push(current_message.clone());
            guild.war_broadcasts.push(enemy_message);
        }
        ended
    };
    if !ended {
        return;
    }
    packets.push(ServerPacket::Chat {
        message: current_message,
        chat_type: ChatType::Guild,
    });
    packets.push(ServerPacket::ColourChanged {
        name_colour_argb: stage5_guild_current_colour_argb(world),
    });
}

fn tick_stage5_guild_wars(world: &mut World, tick: u64, packets: &mut Vec<ServerPacket>) {
    if tick == 0 || tick % STAGE5_GUILD_WAR_TICK_INTERVAL != 0 {
        return;
    }
    let expired_wars = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let guild = &mut stage5.stage5_systems.guild;
        let mut expired = Vec::new();
        for enemy_name in guild.active_wars.clone() {
            let remaining = guild
                .active_war_ticks_remaining
                .entry(enemy_name.clone())
                .or_insert(STAGE5_GUILD_WAR_DURATION_TICKS);
            *remaining = remaining.saturating_sub(STAGE5_GUILD_WAR_TICK_INTERVAL);
            if *remaining == 0 {
                expired.push(enemy_name);
            }
        }
        expired
    };
    for enemy_name in expired_wars {
        stage5_guild_end_war_packet(world, enemy_name, packets);
    }
}

pub(super) fn stage5_guild_request_war_packet(world: &World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if stage5_current_guild_name(world).is_empty() {
        return vec![system_message_key(world, "server.NotInGuild")];
    }
    if stage5_current_guild_rank_index(world) != 0 {
        return vec![system_message_key(
            world,
            "server.YouMustBeLeaderToRequestWar",
        )];
    }
    vec![ServerPacket::GuildRequestWar]
}

fn stage5_guild_war_return_packet(world: &mut World, name: String) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }

    let target_name = name.trim().to_string();
    let guild_name = stage5_current_guild_name(world);
    if guild_name.is_empty() || stage5_current_guild_rank_index(world) != 0 {
        return Vec::new();
    }

    let Some(enemy_name) = stage5_guild_canonical_known_name(world, &target_name) else {
        return vec![system_message_key_args(
            world,
            "server.GuildNotFound",
            [target_name],
        )];
    };
    if guild_name.eq_ignore_ascii_case(&enemy_name) {
        return vec![system_message_key(world, "server.CannotWarOwnGuild")];
    }
    if enemy_name.eq_ignore_ascii_case("NewbieGuild") {
        return vec![system_message_key(world, "server.CannotWarNewPlayersGuild")];
    }
    if stage5_guild_is_at_war_with(world, &enemy_name) {
        return vec![system_message_key(world, "server.AlreadyAtWarWithGuild")];
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .storage_gold
        < STAGE5_GUILD_WAR_COST
    {
        return vec![system_message_key(
            world,
            "server.GuildBankFundsInsufficient",
        )];
    }

    let player_name = current_stage5_character_name(world);
    let language = super::session::current_language(world);
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let guild = &mut stage5.stage5_systems.guild;
        guild.storage_gold = guild.storage_gold.saturating_sub(STAGE5_GUILD_WAR_COST);
        push_unique(&mut guild.active_wars, enemy_name.clone());
        guild
            .active_war_ticks_remaining
            .insert(enemy_name.clone(), STAGE5_GUILD_WAR_DURATION_TICKS);
        guild.war_broadcasts.push(format_localized_text(
            language,
            "server.HasStartedWar",
            [guild_name.clone()],
        ));
    }

    vec![
        system_message_key_args(world, "server.YouStartedWarWith", [enemy_name]),
        ServerPacket::GuildStorageGoldChange {
            amount: STAGE5_GUILD_WAR_COST,
            change_type: 2,
            name: player_name,
        },
        ServerPacket::ColourChanged {
            name_colour_argb: stage5_guild_current_colour_argb(world),
        },
    ]
}

fn stage5_guild_territory_listing(world: &World) -> Option<ClientGtMap> {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let territory = &stage5.stage5_systems.guild_territory;
    if territory.map_file_name.trim().is_empty() {
        return None;
    }
    Some(ClientGtMap {
        index: 0,
        name: territory.map_file_name.clone(),
        owner: territory.owner.clone(),
        leader: territory.leader.clone(),
        leader2: territory.leader2.clone(),
        price: territory.price,
        days: i32::try_from(territory.rental_days_left).unwrap_or(i32::MAX),
        begin: territory.begin,
    })
}

fn stage5_guild_territory_page_packet(world: &World, page: i32) -> Vec<ServerPacket> {
    if !is_in_world(world) || page < 0 {
        return Vec::new();
    }
    let listings = stage5_guild_territory_listing(world)
        .into_iter()
        .collect::<Vec<_>>();
    let start = usize::try_from(page)
        .unwrap_or_default()
        .saturating_mul(STAGE5_GUILD_TERRITORY_PAGE_SIZE);
    let page_listings = listings
        .iter()
        .skip(start)
        .take(STAGE5_GUILD_TERRITORY_PAGE_SIZE)
        .cloned()
        .collect::<Vec<_>>();
    vec![ServerPacket::GuildTerritoryPage {
        length: i32::try_from(listings.len()).unwrap_or(i32::MAX),
        listings: page_listings,
    }]
}

fn stage5_purchase_guild_territory_packet(world: &mut World, owner: String) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let requested_owner = owner.trim().to_string();
    let Some(listing) = stage5_guild_territory_listing(world).filter(|listing| {
        !listing.owner.trim().is_empty() && listing.owner.eq_ignore_ascii_case(&requested_owner)
    }) else {
        return vec![system_message_key(world, "server.OwnerGuildNotFound")];
    };
    if listing.price == 0 {
        return vec![system_message_key(world, "server.TerritoryNoLongerForSale")];
    }

    let guild_name = stage5_current_guild_name(world);
    if guild_name.is_empty() || stage5_current_guild_rank_index(world) != 0 {
        return vec![system_message_key(
            world,
            "server.GuildLeaderToPurchaseTerritory",
        )];
    }
    if listing.owner.eq_ignore_ascii_case(&guild_name) {
        return vec![system_message_key(world, "server.AlreadyOwnTerritory")];
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild_territory
        .owned
    {
        return vec![system_message_key(world, "server.AlreadyOwnATerritory")];
    }
    let Ok(price) = u32::try_from(listing.price) else {
        return vec![system_message_key(world, "server.TerritoryNoLongerForSale")];
    };
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .storage_gold
        < price
    {
        return vec![system_message_key(world, "server.InsufficientFunds")];
    }

    let player_name = current_stage5_character_name(world);
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.guild.storage_gold -= price;
        let territory = &mut stage5.stage5_systems.guild_territory;
        territory.owned = true;
        territory.owner = guild_name;
        territory.leader = player_name;
        territory.price = 0;
    }

    vec![
        ServerPacket::GuildStorageGoldChange {
            amount: price,
            change_type: 2,
            name: String::new(),
        },
        system_message_key(world, "server.GuildTerritoryPurchaseProcess24Hrs"),
    ]
}

fn stage5_guild_player_in_safe_zone(world: &World) -> bool {
    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(position) = entity_position(world, player) else {
        return false;
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    let map = world.resource::<MapRuntimeResource>();
    is_safe_zone_point(config, map, &position)
}

fn stage5_guild_storage_failure(change_type: u8, from: i32, to: i32) -> ServerPacket {
    ServerPacket::GuildStorageItemChange {
        change_type: 3u8.saturating_add(change_type),
        to,
        from,
        user: 0,
        item: None,
    }
}

fn stage5_guild_user_item_for_key(key: &str, unique_id: u64) -> UserItem {
    let item_index = crystal_item_template_for_item_key(key)
        .map(|template| template.item_index)
        .unwrap_or_else(|| i32::from(item_icon_for_key(key)));

    UserItem {
        unique_id,
        item_index,
        current_dura: 0,
        max_dura: 0,
        count: 1,
        soul_bound_id: -1,
        identified: true,
        cursed: false,
        slots: Vec::new(),
        gem_count: 0,
        added_stats: Vec::new(),
        awake_type: 0,
        awake_values: Vec::new(),
        refined_value: 0,
        refine_added: 0,
        refine_success_chance: 0,
        wedding_ring: -1,
        expire_info: None,
        rental_information: None,
        is_shop_item: false,
        sealed_info: None,
        gm_made: false,
    }
}

fn stage5_guild_storage_item_from_state(world: &World, key: &str, slot: u8) -> GuildStorageItem {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let guild = &stage5.stage5_systems.guild;
    let user_id = guild
        .storage_item_users
        .get(&slot)
        .copied()
        .map(i64::from)
        .unwrap_or_else(|| i64::from(current_stage5_character_index(world)));
    let item = guild
        .storage_item_states
        .get(&slot)
        .and_then(|json| serde_json::from_str::<ItemState>(json).ok())
        .map(|item| user_item_from_item_state(&item))
        .unwrap_or_else(|| stage5_guild_user_item_for_key(key, 90_000 + u64::from(slot)));
    GuildStorageItem { item, user_id }
}

fn stage5_swap_guild_storage_map<T>(map: &mut BTreeMap<u8, T>, from: u8, to: u8) {
    if from == to {
        return;
    }
    let from_value = map.remove(&from);
    let to_value = map.remove(&to);
    if let Some(value) = from_value {
        map.insert(to, value);
    }
    if let Some(value) = to_value {
        map.insert(from, value);
    }
}

fn stage5_guild_rank_snapshot(world: &World) -> Vec<GuildRank> {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let guild = &stage5.stage5_systems.guild;
    if guild.name.is_empty() {
        return Vec::new();
    }
    let rank_name = stage5_current_guild_rank_name(world);
    let current_name = current_stage5_character_name(world);
    let current_index = current_stage5_character_index(world);
    let mut members: Vec<GuildMember> = guild
        .members
        .iter()
        .enumerate()
        .map(|(index, name)| GuildMember {
            name: name.clone(),
            id: if name.eq_ignore_ascii_case(&current_name) {
                current_index
            } else {
                i32::try_from(index).unwrap_or_default()
            },
            last_login_binary_datetime: 0,
            has_voted: false,
            online: name.eq_ignore_ascii_case(&current_name),
        })
        .collect();
    if members.is_empty() {
        members.push(GuildMember {
            name: current_name,
            id: current_index,
            last_login_binary_datetime: 0,
            has_voted: false,
            online: true,
        });
    }
    vec![GuildRank {
        name: rank_name,
        options: stage5_current_guild_options(world),
        index: stage5_current_guild_rank_index(world),
        members,
    }]
}

fn stage5_guild_alliance_info_packets(world: &World) -> Vec<ServerPacket> {
    let (ally_count, allied_guilds, recent_broadcasts) = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        let guild = &stage5.stage5_systems.guild;
        if guild.name.trim().is_empty() {
            return Vec::new();
        }
        let allied_guilds = guild
            .allied_guilds
            .iter()
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>();
        let ally_count = u32::try_from(allied_guilds.len()).unwrap_or(u32::MAX);
        let recent_broadcasts = guild
            .alliance_broadcasts
            .iter()
            .rev()
            .filter(|message| !message.trim().is_empty())
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        (ally_count, allied_guilds, recent_broadcasts)
    };

    if ally_count == 0 && recent_broadcasts.is_empty() {
        return Vec::new();
    }

    let mut packets = vec![
        ServerPacket::Chat {
            message: format!("AllyCount: {ally_count}"),
            chat_type: ChatType::Guild,
        },
        ServerPacket::Chat {
            message: if allied_guilds.is_empty() {
                "AllyGuilds: None".to_string()
            } else {
                format!("AllyGuilds: {}", allied_guilds.join(", "))
            },
            chat_type: ChatType::Guild,
        },
    ];
    packets.extend(
        recent_broadcasts
            .into_iter()
            .rev()
            .map(|message| ServerPacket::Chat {
                message: format!("AllianceBroadcast: {message}"),
                chat_type: ChatType::Guild,
            }),
    );
    packets
}

fn stage5_request_guild_info_packet(world: &mut World, info_type: u8) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let guild_exists = !world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .is_empty();
    if !guild_exists {
        return Vec::new();
    }
    match info_type {
        0 => {
            let notice = world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .guild
                .notice
                .clone();
            let mut packets = vec![ServerPacket::GuildNoticeChange {
                update: i32::try_from(notice.len()).unwrap_or(i32::MAX),
                notice,
            }];
            packets.extend(stage5_guild_alliance_info_packets(world));
            packets
        }
        1 => {
            let mut packets = vec![ServerPacket::GuildMemberChange {
                name: String::new(),
                rank_index: 0,
                status: u8::MAX,
                ranks: stage5_guild_rank_snapshot(world),
            }];
            packets.extend(stage5_guild_alliance_info_packets(world));
            packets
        }
        _ => Vec::new(),
    }
}

fn stage5_edit_guild_notice_packet(world: &mut World, notice: Vec<String>) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .is_empty()
    {
        return vec![stage5_guild_not_in_guild_chat(world)];
    }
    if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_CHANGE_NOTICE) {
        return vec![system_message_key(
            world,
            "server.GuildNoticeChangeNotAllowed",
        )];
    }
    if notice.len() > 200 {
        return vec![system_message_key(world, "server.GuildNoticeMaxLines")];
    }
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .notice = notice;
    vec![ServerPacket::GuildNoticeChange {
        update: -1,
        notice: Vec::new(),
    }]
}

fn stage5_edit_guild_member_packet(
    world: &mut World,
    change_type: u8,
    rank_index: u8,
    name: String,
    rank_name: String,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let name = name.trim().to_string();
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .is_empty()
    {
        return vec![stage5_guild_not_in_guild_chat(world)];
    }
    match change_type {
        0 if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_RECRUIT) => {
            return vec![system_message_key(world, "server.NotAllowedRecruitMembers")];
        }
        1 if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_KICK) => {
            return vec![system_message_key(world, "server.CannotRemoveMembers")];
        }
        2 if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_CHANGE_RANK) => {
            return vec![system_message_key(
                world,
                "server.NotAllowedChangeOtherRank",
            )];
        }
        3..=5 if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_CHANGE_RANK) => {
            return vec![system_message_key(world, "server.NotAllowedChangeRank")];
        }
        _ => {}
    }
    let mut emit_member_change = false;
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let guild = &mut stage5.stage5_systems.guild;
        match change_type {
            0 => {
                if !name.is_empty() {
                    push_unique(&mut guild.members, name.clone());
                    emit_member_change = true;
                }
            }
            1 => {
                let before = guild.members.len();
                guild
                    .members
                    .retain(|member| !member.eq_ignore_ascii_case(&name));
                emit_member_change = guild.members.len() != before;
            }
            2 | 3 => {
                if !rank_name.trim().is_empty() {
                    guild.rank = rank_name.trim().to_string();
                    emit_member_change = true;
                }
            }
            4 => {
                emit_member_change = true;
            }
            5 => {
                if let Ok(option) = rank_name.parse::<u8>() {
                    if option <= 7 {
                        let permission = match option {
                            0 => "changeRank",
                            1 => "recruit",
                            2 => "kick",
                            3 => "storeItem",
                            4 => "retrieveItem",
                            5 => "alterAlliance",
                            6 => "changeNotice",
                            7 => "activateBuff",
                            _ => unreachable!("option is already bounded"),
                        }
                        .to_string();
                        if name.eq_ignore_ascii_case("true") {
                            push_unique(&mut guild.permissions, permission);
                        } else if name.eq_ignore_ascii_case("false") {
                            guild.permissions.retain(|existing| {
                                stage5_guild_permission_key(existing)
                                    != stage5_guild_permission_key(&permission)
                            });
                        }
                        emit_member_change = true;
                    }
                }
            }
            _ => {}
        }
    }
    if emit_member_change {
        vec![ServerPacket::GuildMemberChange {
            name,
            rank_index,
            status: change_type,
            ranks: stage5_guild_rank_snapshot(world),
        }]
    } else {
        Vec::new()
    }
}

fn stage5_guild_invite_reply_packet(world: &mut World, accept_invite: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if !accept_invite {
        return Vec::new();
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .is_empty()
    {
        return vec![system_message_key(world, "server.GuildNotInvited")];
    }
    let player_name = current_stage5_character_name(world);
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        push_unique(
            &mut stage5.stage5_systems.guild.members,
            player_name.clone(),
        );
    }
    vec![ServerPacket::GuildMemberChange {
        name: player_name,
        rank_index: 0,
        status: 0,
        ranks: stage5_guild_rank_snapshot(world),
    }]
}

fn stage5_guild_storage_gold_packet(
    world: &mut World,
    change_type: u8,
    amount: u32,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .is_empty()
    {
        return vec![stage5_guild_not_in_guild_chat(world)];
    }
    if !stage5_guild_player_in_safe_zone(world) {
        return vec![system_message_key(
            world,
            "server.CannotUseGuildStorageOutsideSafezones",
        )];
    }
    let player_name = current_stage5_character_name(world);
    match change_type {
        0 => {
            if world.resource::<PlayerRuntimeResource>().gold < amount {
                return vec![system_message_key(world, "server.InsufficientGold")];
            }
            if world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .guild
                .storage_gold
                > u32::MAX.saturating_sub(amount)
            {
                return vec![system_message_key(world, "server.GuildGoldLimitReached")];
            }
            world.resource_mut::<PlayerRuntimeResource>().gold -= amount;
            {
                let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
                stage5.stage5_systems.guild.storage_gold = stage5
                    .stage5_systems
                    .guild
                    .storage_gold
                    .saturating_add(amount);
            }
            vec![
                ServerPacket::LoseGold { gold: amount },
                ServerPacket::GuildStorageGoldChange {
                    amount,
                    change_type: 0,
                    name: player_name,
                },
            ]
        }
        _ => {
            let guild_gold = world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .guild
                .storage_gold;
            if guild_gold < amount {
                return vec![system_message_key(world, "server.InsufficientGold")];
            }
            if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), amount) {
                return vec![system_message_key(world, "server.GoldLimitReached")];
            }
            if stage5_current_guild_rank_index(world) != 0 {
                return vec![system_message_key(world, "server.InsufficientRank")];
            }
            world.resource_mut::<PlayerRuntimeResource>().gold += amount;
            world
                .resource_mut::<Stage5SystemsResource>()
                .stage5_systems
                .guild
                .storage_gold = guild_gold - amount;
            vec![
                ServerPacket::GainedGold { gold: amount },
                ServerPacket::GuildStorageGoldChange {
                    amount,
                    change_type: 1,
                    name: player_name,
                },
            ]
        }
    }
}

fn stage5_guild_storage_list_packet(world: &World) -> ServerPacket {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let mut items = vec![None; GUILD_STORAGE_SLOT_COUNT];
    for (slot, key) in &stage5.stage5_systems.guild.storage_items {
        if let Some(target) = items.get_mut(usize::from(*slot)) {
            *target = Some(stage5_guild_storage_item_from_state(world, key, *slot));
        }
    }
    ServerPacket::GuildStorageList { items }
}

fn stage5_guild_storage_item_packet(
    world: &mut World,
    change_type: u8,
    from: i32,
    to: i32,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild
        .name
        .is_empty()
    {
        return vec![
            stage5_guild_storage_failure(change_type, from, to),
            stage5_guild_not_in_guild_chat(world),
        ];
    }
    if !stage5_guild_player_in_safe_zone(world) && change_type != 3 {
        return vec![
            stage5_guild_storage_failure(change_type, from, to),
            system_message_key(world, "server.GuildStorageOutsideSafezone"),
        ];
    }

    let user = current_stage5_character_index(world);
    match change_type {
        0 => {
            if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_STORE_ITEM) {
                return vec![
                    stage5_guild_storage_failure(change_type, from, to),
                    system_message_key(world, "server.NoPermissionGuildStorage"),
                ];
            }
            let Ok(to_slot) = u8::try_from(to) else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            };
            if usize::from(to_slot) >= GUILD_STORAGE_SLOT_COUNT {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            }
            let Some(from_slot) = u8::try_from(from).ok() else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            };
            let item_index = {
                let inventory = world.resource::<InventoryResource>();
                inventory
                    .inventory_items
                    .iter()
                    .position(|item| inventory_item_matches_index(item, from_slot))
            };
            let Some(item_index) = item_index else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            };
            let item = world.resource::<InventoryResource>().inventory_items[item_index].clone();
            if item_has_crystal_or_rental_bind_flag(&item, CRYSTAL_BIND_DONT_STORE) {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            }
            if world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .guild
                .storage_items
                .contains_key(&to_slot)
            {
                return vec![
                    stage5_guild_storage_failure(change_type, from, to),
                    system_message_key(world, "server.TargetSlotNotEmpty"),
                ];
            }
            let item = world
                .resource_mut::<InventoryResource>()
                .inventory_items
                .remove(item_index);
            {
                let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
                let guild = &mut stage5.stage5_systems.guild;
                guild.storage_items.insert(to_slot, item.key.clone());
                if let Ok(encoded) = serde_json::to_string(&item) {
                    guild.storage_item_states.insert(to_slot, encoded);
                }
                guild.storage_item_users.insert(to_slot, user);
            }
            vec![ServerPacket::GuildStorageItemChange {
                change_type: 0,
                to,
                from,
                user,
                item: Some(GuildStorageItem {
                    item: user_item_from_item_state(&item),
                    user_id: i64::from(user),
                }),
            }]
        }
        1 => {
            if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_RETRIEVE_ITEM) {
                return vec![system_message_key(
                    world,
                    "server.NoPermissionGuildStorageRetrieve",
                )];
            }
            let Ok(from_slot) = u8::try_from(from) else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            };
            let Some(to_slot) = u8::try_from(to).ok() else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            };
            if !is_valid_inventory_slot(to_slot) {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            }
            let occupied = world
                .resource::<InventoryResource>()
                .inventory_items
                .iter()
                .any(|item| inventory_item_matches_index(item, to_slot));
            if occupied {
                return vec![
                    stage5_guild_storage_failure(change_type, from, to),
                    system_message_key(world, "server.TargetSlotNotEmpty"),
                ];
            }
            let (item_key, item_state_json) = {
                let stage5 = world.resource::<Stage5SystemsResource>();
                let guild = &stage5.stage5_systems.guild;
                let Some(item_key) = guild.storage_items.get(&from_slot).cloned() else {
                    return vec![stage5_guild_storage_failure(change_type, from, to)];
                };
                (item_key, guild.storage_item_states.get(&from_slot).cloned())
            };
            if let Some((container, slot)) = inventory_container_and_slot_for_index(to_slot) {
                if let Some(mut item) =
                    item_state_json.and_then(|json| serde_json::from_str::<ItemState>(&json).ok())
                {
                    item.container = container;
                    item.slot = slot;
                    if item.unique_id == 0
                        || world
                            .resource::<InventoryResource>()
                            .inventory_items
                            .iter()
                            .any(|candidate| item_unique_id(candidate) == item_unique_id(&item))
                    {
                        item.unique_id = allocate_item_unique_id(
                            world.resource::<InventoryResource>(),
                            container,
                            slot,
                        );
                    }
                    world
                        .resource_mut::<InventoryResource>()
                        .inventory_items
                        .push(item);
                } else {
                    add_or_increment_item(
                        world,
                        container,
                        &item_key,
                        &stage5_item_name(&item_key),
                        "Stage 5 guild storage retrieve.",
                        slot,
                        1,
                        1,
                    );
                }
            } else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            }
            {
                let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
                let guild = &mut stage5.stage5_systems.guild;
                guild.storage_items.remove(&from_slot);
                guild.storage_item_states.remove(&from_slot);
                guild.storage_item_users.remove(&from_slot);
            }
            vec![ServerPacket::GuildStorageItemChange {
                change_type: 1,
                to,
                from,
                user,
                item: None,
            }]
        }
        2 => {
            if !stage5_current_guild_has_option(world, GUILD_OPTION_CAN_STORE_ITEM) {
                return vec![
                    stage5_guild_storage_failure(change_type, from, to),
                    system_message_key(world, "server.NoGuildStorageMovePermission"),
                ];
            }
            let (Ok(from_slot), Ok(to_slot)) = (u8::try_from(from), u8::try_from(to)) else {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            };
            if usize::from(from_slot) >= GUILD_STORAGE_SLOT_COUNT
                || usize::from(to_slot) >= GUILD_STORAGE_SLOT_COUNT
            {
                return vec![stage5_guild_storage_failure(change_type, from, to)];
            }
            let moved_key = {
                let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
                let guild = &mut stage5.stage5_systems.guild;
                let Some(moved) = guild.storage_items.get(&from_slot).cloned() else {
                    return vec![stage5_guild_storage_failure(change_type, from, to)];
                };
                stage5_swap_guild_storage_map(&mut guild.storage_items, from_slot, to_slot);
                stage5_swap_guild_storage_map(&mut guild.storage_item_states, from_slot, to_slot);
                stage5_swap_guild_storage_map(&mut guild.storage_item_users, from_slot, to_slot);
                moved
            };
            vec![ServerPacket::GuildStorageItemChange {
                change_type: 2,
                to,
                from,
                user,
                item: Some(stage5_guild_storage_item_from_state(
                    world, &moved_key, to_slot,
                )),
            }]
        }
        3 => vec![stage5_guild_storage_list_packet(world)],
        _ => vec![stage5_guild_storage_failure(change_type, from, to)],
    }
}

const CRYSTAL_QUEST_STATE_ADD: u8 = 0;
const CRYSTAL_QUEST_STATE_UPDATE: u8 = 1;
const CRYSTAL_QUEST_STATE_REMOVE: u8 = 2;

fn stage5_quest_task_list(world: &World, quest_id: i32) -> Vec<String> {
    if let Some(tasks) = crystal_quest_task_list(world, quest_id) {
        return tasks;
    }
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
    if !quest_definition_exists(quest_id) {
        return vec![system_message_key(world, "server.CouldNotAcceptQuest")];
    }
    if !can_accept_quest(world, quest_id)
        && !matches!(
            stage5_quest_stage(world, quest_id),
            Some(QuestStage::InProgress | QuestStage::ReadyToTurnIn | QuestStage::Completed)
        )
    {
        return vec![system_message_key(world, "server.CouldNotAcceptQuest")];
    }
    match ensure_runtime_quest(world, quest_id) {
        QuestStage::Available => {
            begin_quest(world, quest_id);
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

fn stage5_finish_quest_packet(
    world: &mut World,
    quest_id: i32,
    selected_item_index: Option<i32>,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if ensure_runtime_quest(world, quest_id) != QuestStage::ReadyToTurnIn {
        return Vec::new();
    }
    complete_quest_with_selection(world, quest_id, selected_item_index);
    if stage5_quest_stage(world, quest_id) != Some(QuestStage::Completed) {
        return vec![system_message_key(world, "server.CannotHandInQuestBagFull")];
    }
    vec![
        stage5_quest_remove_packet(quest_id, true),
        ServerPacket::CompleteQuest {
            completed_quests: completed_quest_ids(world),
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
    if !quest_definition_exists(quest_id) {
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
    let Some(price) = stage5_market_sale_price(listing.price, bid_price) else {
        return vec![ServerPacket::MarketFail { reason: 9 }];
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
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let listing = &mut stage5.stage5_systems.auction[index];
        listing.price = price;
        listing.sold = true;
    }
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
        let settlement = stage5_market_settlement(listing.price);
        world.resource_mut::<PlayerRuntimeResource>().gold += settlement.earnings;
        return vec![
            ServerPacket::GainedGold {
                gold: settlement.earnings,
            },
            stage5_market_success_key_args(
                world,
                "server.SoldItemEarningsCommission",
                [
                    stage5_item_name(&listing.item_key),
                    settlement.gross.to_string(),
                    settlement.earnings.to_string(),
                    settlement.commission.to_string(),
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

fn stage5_lover_update_packet(world: &World) -> ServerPacket {
    let relationship = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .relationship;
    ServerPacket::LoverUpdate {
        name: relationship.partner_name.clone(),
        date_binary_datetime: relationship.married_date_binary_datetime,
        map_name: relationship.map_name.clone(),
        married_days: relationship.married_days,
    }
}

fn stage5_mentor_update_packet(world: &World) -> ServerPacket {
    let mentor = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mentor;
    ServerPacket::MentorUpdate {
        name: mentor.name.clone(),
        level: mentor.level,
        online: mentor.online,
        mentee_exp: mentor.mentee_exp,
    }
}

fn stage5_marriage_request_packet(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if !world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .relationship
        .partner_name
        .is_empty()
    {
        return vec![system_message_key(world, "server.YouAlreadyMarried")];
    }
    vec![system_message_key(
        world,
        "server.FacePlayerForMarriageRequest",
    )]
}

fn stage5_marriage_reply_packet(world: &mut World, accept_invite: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let pending = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .relationship
        .pending_request_from
        .clone();
    let Some(partner_name) = pending else {
        return Vec::new();
    };
    if !accept_invite {
        world
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .relationship
            .pending_request_from = None;
        return Vec::new();
    }
    let map_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .title
        .clone();
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let relationship = &mut stage5.stage5_systems.relationship;
        relationship.partner_name = partner_name;
        relationship.map_name = map_name;
        relationship.married_date_binary_datetime = storage_password_binary_datetime();
        relationship.married_days = 0;
        relationship.pending_request_from = None;
    }
    vec![stage5_lover_update_packet(world)]
}

fn stage5_change_marriage_packet(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let allow = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let relationship = &mut stage5.stage5_systems.relationship;
        relationship.allow_marriage = !relationship.allow_marriage;
        relationship.allow_marriage
    };
    vec![system_message_key(
        world,
        if allow {
            "server.YouAllowMarriageRequests"
        } else {
            "server.YouBlockMarriageRequests"
        },
    )]
}

fn stage5_divorce_request_packet(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let partner_name = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .relationship
        .partner_name
        .clone();
    if partner_name.is_empty() {
        return vec![system_message_key(world, "server.YouNotMarried")];
    }
    vec![ServerPacket::DivorceRequest { name: partner_name }]
}

fn stage5_divorce_reply_packet(world: &mut World, accept_invite: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    if !accept_invite {
        return Vec::new();
    }
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let relationship = &mut stage5.stage5_systems.relationship;
        relationship.partner_name.clear();
        relationship.map_name.clear();
        relationship.married_days = 0;
        relationship.married_date_binary_datetime = storage_password_binary_datetime();
        relationship.pending_request_from = None;
        relationship.pending_divorce_from = None;
    }
    vec![stage5_lover_update_packet(world)]
}

fn stage5_add_mentor_packet(world: &mut World, name: String) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let player_name = stage5_player_name(world);
    if name.eq_ignore_ascii_case(&player_name) {
        return vec![system_message_key(world, "server.YouCantMentorYourself")];
    }
    if name.trim().is_empty() {
        return vec![system_message_key_args(
            world,
            "server.CannotFindPlayerByName",
            [name],
        )];
    }
    let current_mentor = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mentor
        .name
        .clone();
    if !current_mentor.is_empty() {
        return vec![system_message_key(world, "server.YouAlreadyHaveMentor")];
    }
    vec![system_message_key_args(
        world,
        "server.CannotFindPlayerByName",
        [name],
    )]
}

fn stage5_mentor_reply_packet(world: &mut World, accept_invite: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let pending = {
        let mentor = &world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .mentor;
        mentor
            .pending_request_from
            .clone()
            .map(|name| (name, mentor.pending_request_level))
    };
    let Some((name, level)) = pending else {
        return Vec::new();
    };
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let mentor = &mut stage5.stage5_systems.mentor;
        if accept_invite {
            mentor.name = name;
            mentor.level = level;
            mentor.online = true;
            mentor.mentee_exp = 0;
        }
        mentor.pending_request_from = None;
        mentor.pending_request_level = 0;
    }
    if accept_invite {
        vec![stage5_mentor_update_packet(world)]
    } else {
        Vec::new()
    }
}

fn stage5_allow_mentor_packet(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let allow = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let mentor = &mut stage5.stage5_systems.mentor;
        mentor.allow_mentor = !mentor.allow_mentor;
        mentor.allow_mentor
    };
    vec![hint_chat_key(
        world,
        if allow {
            "server.AllowingMentorRequests"
        } else {
            "server.BlockingMentorRequests"
        },
    )]
}

fn stage5_cancel_mentor_packet(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    let had_mentor = {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let mentor = &mut stage5.stage5_systems.mentor;
        let had_mentor = !mentor.name.is_empty();
        mentor.name.clear();
        mentor.level = 0;
        mentor.online = false;
        mentor.mentee_exp = 0;
        mentor.pending_request_from = None;
        mentor.pending_request_level = 0;
        had_mentor
    };
    if had_mentor {
        vec![
            system_message_key_args(world, "server.YouHaveMentorshipCooldown", ["7"]),
            stage5_mentor_update_packet(world),
        ]
    } else {
        vec![system_message_key(world, "server.NoMentorship")]
    }
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
    if trade.completed || trade.locked {
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
        if !stage5_trade_item_can_enter(item) {
            return vec![ServerPacket::DepositTradeItem {
                from,
                to,
                success: false,
            }];
        }
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
        for inventory_index in offered_slots.values() {
            let Some(item) = inventory
                .inventory_items
                .iter()
                .find(|item| inventory_item_matches_index(item, *inventory_index))
            else {
                return Vec::new();
            };
            if !stage5_trade_item_can_enter(item) {
                return Vec::new();
            }
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
    tick_stage5_guild_wars(world, tick, packets);
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

const CRYSTAL_CHAT_INTERVAL_MS: u64 = 2_000;
const CRYSTAL_CHAT_SPAM_TICKS_BEFORE_BAN: u8 = 5;
const CRYSTAL_CHAT_SPAM_BAN_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedChatPacket {
    pub message: String,
    pub linked_items: Vec<ChatItem>,
    pub linked_user_items: Vec<UserItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatPacketPreparation {
    Dispatch(PreparedChatPacket),
    Immediate(Vec<ServerPacket>),
}

pub(super) fn prepare_chat_packet(
    world: &mut World,
    message: String,
    linked_items: Vec<ChatItem>,
) -> ChatPacketPreparation {
    if !super::session::is_in_world(world) {
        return ChatPacketPreparation::Immediate(Vec::new());
    }

    if let Some(remaining_seconds) = active_chat_ban_remaining_seconds(world) {
        return ChatPacketPreparation::Immediate(vec![chat_ban_remaining_message(
            world,
            remaining_seconds,
        )]);
    }

    if message.is_empty() {
        return ChatPacketPreparation::Immediate(Vec::new());
    }

    if let Some(packet) = apply_crystal_chat_spam_guard(world) {
        return ChatPacketPreparation::Immediate(vec![packet]);
    }

    if message.trim().eq_ignore_ascii_case("@ADDSTORAGE") {
        return ChatPacketPreparation::Immediate(expand_storage_rental_impl(world));
    }

    ChatPacketPreparation::Dispatch(PreparedChatPacket {
        linked_user_items: chat_linked_user_items(world, &linked_items),
        message,
        linked_items,
    })
}

pub(super) fn handle_chat_packet(
    world: &mut World,
    message: String,
    linked_items: Vec<ChatItem>,
) -> Vec<ServerPacket> {
    let prepared = match prepare_chat_packet(world, message, linked_items) {
        ChatPacketPreparation::Dispatch(prepared) => prepared,
        ChatPacketPreparation::Immediate(packets) => return packets,
    };

    let player_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "?????".to_string());
    let message = chat_text_with_linked_items(&prepared.message, &prepared.linked_items);

    vec![ServerPacket::ObjectChat {
        object_id: current_player_object_id(world).unwrap_or(0),
        text: format!("{player_name}: {message}"),
        chat_type: mir2_protocol::ChatType::Normal,
    }]
}

fn apply_crystal_chat_spam_guard(world: &mut World) -> Option<ServerPacket> {
    let now = unix_now_ms();
    let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
    if now < player_runtime.chat_next_allowed_at_ms {
        player_runtime.chat_spam_tick = player_runtime.chat_spam_tick.saturating_add(1);
        if player_runtime.chat_spam_tick >= CRYSTAL_CHAT_SPAM_TICKS_BEFORE_BAN {
            player_runtime.chat_banned = true;
            player_runtime.chat_ban_until_ms = Some(now.saturating_add(CRYSTAL_CHAT_SPAM_BAN_MS));
            player_runtime.chat_spam_tick = 0;
            player_runtime.chat_next_allowed_at_ms = 0;
            drop(player_runtime);
            return Some(system_message_key(world, "server.ChatBanDuration5Minutes"));
        }
    } else {
        player_runtime.chat_spam_tick = 0;
    }
    player_runtime.chat_next_allowed_at_ms = now.saturating_add(CRYSTAL_CHAT_INTERVAL_MS);
    None
}

pub(super) fn chat_text_with_linked_items(message: &str, linked_items: &[ChatItem]) -> String {
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

fn chat_linked_user_items(world: &World, linked_items: &[ChatItem]) -> Vec<UserItem> {
    if linked_items.is_empty() {
        return Vec::new();
    }
    let inventory = world.resource::<InventoryResource>();
    let hero_inventory = world.resource::<HeroInventoryResource>();
    let mut items = Vec::new();
    for linked_item in linked_items {
        let item = match linked_item.grid {
            MirGridType::Equipment => inventory
                .equipment_items
                .iter()
                .filter_map(user_item_from_equipment_state)
                .find(|item| item.unique_id == linked_item.unique_id),
            MirGridType::HeroInventory => hero_inventory
                .items
                .iter()
                .find(|item| item_unique_id(item) == linked_item.unique_id)
                .map(user_item_from_item_state),
            _ => inventory
                .inventory_items
                .iter()
                .chain(inventory.belt_items.iter())
                .chain(inventory.storage_items.iter())
                .find(|item| {
                    item_matches_client_reference(item, linked_item.grid, linked_item.unique_id)
                })
                .map(user_item_from_item_state),
        };
        if let Some(item) = item {
            items.push(item);
        }
    }
    items
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
    let hero_inventory = world.resource::<HeroInventoryResource>();
    let player_runtime = world.resource::<PlayerRuntimeResource>();
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    let session = world.resource::<SessionResource>();
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
    let mut stage5_systems = stage5.stage5_systems.clone();
    stage5_systems.item_rental = item_rental_snapshot(world);

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
        hero_inventory_items: hero_inventory
            .items
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
        quest_log: quest_log_snapshots(world, language),
        active_npc_dialog: npc_state
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.snapshot(language)),
        npc_script_diagnostics: npc_state
            .npc_script_diagnostics
            .iter()
            .map(|diagnostic| NpcScriptDiagnosticSnapshot {
                script_key: diagnostic.script_key.clone(),
                label: diagnostic.label.clone(),
                line_number: diagnostic.line_number,
                command: diagnostic.command.clone(),
                message: diagnostic.message.clone(),
            })
            .collect(),
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
        stage5_systems,
        map_transfers: collect_map_transfer_snapshots(config, map),
        interaction_hints: build_interaction_hints(world, resources),
    }
}

fn item_rental_snapshot(world: &World) -> Stage5ItemRentalSnapshot {
    let rental = world.resource::<ItemRentalResource>();
    let active = rental.active.as_ref();
    Stage5ItemRentalSnapshot {
        partner_name: active.map(|active| active.partner_name.clone()),
        fee: active.map(|active| active.fee).unwrap_or(0),
        days: active.map(|active| active.days).unwrap_or(0),
        has_deposited_item: active
            .and_then(|active| active.deposited_item.as_ref())
            .is_some(),
        deposited_item_name: active
            .and_then(|active| active.deposited_item.as_ref())
            .map(|item| item.name.clone()),
        gold_locked: active.map(|active| active.gold_locked).unwrap_or(false),
        item_locked: active.map(|active| active.item_locked).unwrap_or(false),
        record_count: rental.rented_items.len(),
        rented_items: rental
            .rented_items
            .iter()
            .map(|item| Stage5ItemRentalRecordSnapshot {
                item_id: item.item_id,
                item_name: item.item_name.clone(),
                renting_player_name: item.renting_player_name.clone(),
                item_return_date_binary_datetime: item.item_return_date_binary_datetime,
            })
            .collect(),
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
        percent: mana_percent(vitals.mp, vitals.max_mp),
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
        let hero_marker = entity.get::<Hero>();
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
                player_vitals.map(|v| v.hp <= 0).unwrap_or(false),
                player_vitals.map(|v| v.hp),
                player_vitals.map(|v| v.max_hp),
                body.map(|v| v.level),
            )
        } else if hero_marker.is_some() {
            (
                WorldEntityKind::Player,
                WorldEntityDisposition::Friendly,
                player_vitals.map(|v| v.hp <= 0).unwrap_or(false),
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
            owner_name: hero_marker.map(|hero| hero.owner_name.clone()),
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
    let tick = super::session::runtime_tick(world);

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
        let ownership = entity.get::<DropOwnership>().and_then(|ownership| {
            (tick <= ownership.expires_at_tick).then(|| {
                (
                    ownership.owner_object_id,
                    ownership.expires_at_tick.saturating_sub(tick),
                )
            })
        });

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
            owner_object_id: ownership.map(|(owner_object_id, _)| owner_object_id),
            ownership_remaining_ticks: ownership.map(|(_, remaining_ticks)| remaining_ticks),
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

    if let Some(hero) = entry.get::<Hero>() {
        let name = entry.get::<DisplayName>()?.resolve(language);
        let position = entry.get::<Position>()?.0.clone();
        let facing = entry.get::<Facing>()?.0;
        let body = entry.get::<CharacterBody>()?;
        return Some((
            object_id,
            VisibleObjectBundle {
                spawn_packet: ServerPacket::ObjectHero {
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
                    owner_name: hero.owner_name.clone(),
                },
                health_packet: object_health_info_for_entity(world, entity, 0)
                    .map(|info| ServerPacket::ObjectHealth { info }),
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
                poison: entry
                    .get::<MonsterPoisonState>()
                    .map(|state| state.poison)
                    .unwrap_or(0),
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

#[derive(Debug, Clone)]
struct RankingCandidate {
    account_id: String,
    character_index: i32,
    player_id: i64,
    name: String,
    level: i32,
    class: MirClass,
    experience: i64,
    online: bool,
}

fn ranking_class_filter(rank_type: u8) -> Option<Option<MirClass>> {
    match rank_type {
        0 => Some(None),
        1 => Some(Some(MirClass::Warrior)),
        2 => Some(Some(MirClass::Wizard)),
        3 => Some(Some(MirClass::Taoist)),
        4 => Some(Some(MirClass::Assassin)),
        5 => Some(Some(MirClass::Archer)),
        _ => None,
    }
}

fn ranking_candidate_from_character(
    account_id: &str,
    character: &CharacterRecord,
    save: Option<&CharacterSaveRecord>,
    online: bool,
) -> RankingCandidate {
    let character = save.map(|save| &save.character).unwrap_or(character);
    RankingCandidate {
        account_id: account_id.to_string(),
        character_index: character.index,
        player_id: i64::from(character.index),
        name: character.name.clone(),
        level: i32::from(character.level),
        class: character.class,
        experience: save.map(|save| save.experience).unwrap_or_default(),
        online,
    }
}

fn stage5_get_ranking_packet(
    world: &World,
    rank_type: u8,
    rank_index: i32,
    online_only: bool,
) -> Vec<ServerPacket> {
    let Some(class_filter) = ranking_class_filter(rank_type) else {
        return Vec::new();
    };
    if rank_index < 0 {
        return Vec::new();
    }

    let session = world.resource::<SessionResource>();
    let Some(session_account_id) = session.account_id.clone() else {
        return Vec::new();
    };
    let active_character_index = session
        .selected_character
        .as_ref()
        .map(|character| character.index);
    let active_key = active_character_index.map(|index| (session_account_id.clone(), index));
    let active_save = snapshot_active_character_save(world);
    let config = world.resource::<RuntimeConfigResource>().config.clone();

    let mut candidates = Vec::new();
    let mut active_seen = false;
    {
        let Ok(store) = config.account_store.lock() else {
            return Vec::new();
        };
        for (account_id, account) in &store.accounts {
            for character in &account.characters {
                let online = active_key
                    .as_ref()
                    .is_some_and(|(active_account, active_index)| {
                        active_account == account_id && *active_index == character.index
                    });
                let save = if online {
                    active_seen = true;
                    active_save
                        .as_ref()
                        .or_else(|| account.saves.get(&character.index))
                } else {
                    account.saves.get(&character.index)
                };
                candidates.push(ranking_candidate_from_character(
                    account_id, character, save, online,
                ));
            }
        }
    }

    if !active_seen {
        if let (Some((account_id, character_index)), Some(save)) =
            (active_key.as_ref(), active_save.as_ref())
        {
            candidates.push(RankingCandidate {
                account_id: account_id.clone(),
                character_index: *character_index,
                player_id: i64::from(*character_index),
                name: save.character.name.clone(),
                level: i32::from(save.character.level),
                class: save.character.class,
                experience: save.experience,
                online: true,
            });
        }
    }

    candidates.retain(|candidate| {
        class_filter
            .map(|class| candidate.class == class)
            .unwrap_or(true)
    });
    candidates.sort_by(|left, right| {
        right
            .level
            .cmp(&left.level)
            .then_with(|| right.experience.cmp(&left.experience))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.player_id.cmp(&right.player_id))
    });

    let my_rank = active_key
        .as_ref()
        .and_then(|(account_id, character_index)| {
            candidates
                .iter()
                .position(|candidate| {
                    &candidate.account_id == account_id
                        && candidate.character_index == *character_index
                })
                .map(|index| index as i32 + 1)
        })
        .unwrap_or_default();

    let visible_candidates: Vec<&RankingCandidate> = candidates
        .iter()
        .filter(|candidate| !online_only || candidate.online)
        .collect();
    let count = visible_candidates.len() as i32;
    let start = rank_index as usize;
    if start >= visible_candidates.len() && !visible_candidates.is_empty() {
        return Vec::new();
    }
    let page: Vec<&RankingCandidate> = visible_candidates
        .into_iter()
        .skip(start)
        .take(20)
        .collect();
    let listing_details = page
        .iter()
        .map(|candidate| RankCharacterInfo {
            player_id: candidate.player_id,
            name: candidate.name.clone(),
            level: candidate.level,
            class: candidate.class,
        })
        .collect();
    let listings = page.iter().map(|candidate| candidate.player_id).collect();

    vec![ServerPacket::Rankings {
        rank_type,
        my_rank,
        listing_details,
        listings,
        count,
    }]
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
            ClientPacket::SetAutoPotValue { stat, value } => {
                let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
                let Some(hero) = resources.stage5_systems.hero.as_mut() else {
                    return Vec::new();
                };
                if !hero.spawned || !hero.auto_pot {
                    return Vec::new();
                }
                let percent = value.min(99) as u8;
                if stat == 0 {
                    hero.auto_hp_percent = percent;
                } else {
                    hero.auto_mp_percent = percent;
                }
                vec![ServerPacket::SetAutoPotValue { stat, value }]
            }
            ClientPacket::SetAutoPotItem { grid, item_index } => {
                let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
                let Some(hero) = resources.stage5_systems.hero.as_mut() else {
                    return Vec::new();
                };
                if !hero.spawned || !hero.auto_pot {
                    return Vec::new();
                }
                let item_index = if item_index > 0 && crystal_item_by_index(item_index).is_some() {
                    item_index
                } else {
                    0
                };
                if grid == MirGridType::HeroHpItem {
                    hero.hp_item_index = item_index;
                } else {
                    hero.mp_item_index = item_index;
                }
                vec![ServerPacket::SetAutoPotItem {
                    grid: grid as u8,
                    item_index,
                }]
            }
            ClientPacket::ReplaceWedRing { .. }
            | ClientPacket::TeleportToNpc { .. }
            | ClientPacket::SearchMap { .. }
            | ClientPacket::Inspect { .. }
            | ClientPacket::Observe { .. }
            | ClientPacket::ChangeAMode { .. }
            | ClientPacket::ChangePMode { .. }
            | ClientPacket::ChangeTrade { .. }
            | ClientPacket::BuyItemBack { .. }
            | ClientPacket::TownRevive
            | ClientPacket::RequestUserName { .. }
            | ClientPacket::RequestChatItem { .. }
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
            | ClientPacket::GameShopBuy { .. }
            | ClientPacket::ReportIssue { .. } => Vec::new(),
            ClientPacket::GetRanking {
                rank_type,
                rank_index,
                online_only,
            } => stage5_get_ranking_packet(self.app.world(), rank_type, rank_index, online_only),
            ClientPacket::GuildWarReturn { name } => {
                stage5_guild_war_return_packet(self.app.world_mut(), name)
            }
            ClientPacket::GuildTerritoryPage { page } => {
                stage5_guild_territory_page_packet(self.app.world(), page)
            }
            ClientPacket::PurchaseGuildTerritory { owner } => {
                stage5_purchase_guild_territory_packet(self.app.world_mut(), owner)
            }
            ClientPacket::EditGuildMember {
                change_type,
                rank_index,
                name,
                rank_name,
            } => stage5_edit_guild_member_packet(
                self.app.world_mut(),
                change_type,
                rank_index,
                name,
                rank_name,
            ),
            ClientPacket::EditGuildNotice { notice } => {
                stage5_edit_guild_notice_packet(self.app.world_mut(), notice)
            }
            ClientPacket::GuildInvite { accept_invite } => {
                stage5_guild_invite_reply_packet(self.app.world_mut(), accept_invite)
            }
            ClientPacket::GuildNameReturn { name } => {
                self.stage5_command("guild.create", vec![name])
            }
            ClientPacket::RequestGuildInfo { info_type } => {
                stage5_request_guild_info_packet(self.app.world_mut(), info_type)
            }
            ClientPacket::GuildStorageGoldChange {
                change_type,
                amount,
            } => stage5_guild_storage_gold_packet(self.app.world_mut(), change_type, amount),
            ClientPacket::GuildStorageItemChange {
                change_type,
                from,
                to,
            } => stage5_guild_storage_item_packet(self.app.world_mut(), change_type, from, to),
            ClientPacket::CraftItem { .. } => vec![ServerPacket::CraftItem { success: false }],
            ClientPacket::DepositTradeItem { from, to } => {
                stage5_deposit_trade_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::RetrieveTradeItem { from, to } => {
                stage5_retrieve_trade_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::TakeBackHeroItem { from, to } => {
                take_back_hero_item_packet(self.app.world_mut(), from, to)
            }
            ClientPacket::TransferHeroItem { from, to } => {
                transfer_hero_item_packet(self.app.world_mut(), from, to)
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
            ClientPacket::MarriageRequest => stage5_marriage_request_packet(self.app.world_mut()),
            ClientPacket::MarriageReply { accept_invite } => {
                stage5_marriage_reply_packet(self.app.world_mut(), accept_invite)
            }
            ClientPacket::ChangeMarriage => stage5_change_marriage_packet(self.app.world_mut()),
            ClientPacket::DivorceRequest => stage5_divorce_request_packet(self.app.world_mut()),
            ClientPacket::DivorceReply { accept_invite } => {
                stage5_divorce_reply_packet(self.app.world_mut(), accept_invite)
            }
            ClientPacket::AddMentor { name } => {
                stage5_add_mentor_packet(self.app.world_mut(), name)
            }
            ClientPacket::MentorReply { accept_invite } => {
                stage5_mentor_reply_packet(self.app.world_mut(), accept_invite)
            }
            ClientPacket::AllowMentor => stage5_allow_mentor_packet(self.app.world_mut()),
            ClientPacket::CancelMentor => stage5_cancel_mentor_packet(self.app.world_mut()),
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
            ClientPacket::FinishQuest {
                quest_index,
                selected_item_index,
            } => stage5_finish_quest_packet(
                self.app.world_mut(),
                quest_index,
                (selected_item_index >= 0).then_some(selected_item_index),
            ),
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
            ClientPacket::ReadMail { mail_id } => {
                stage5_read_mail_packet(self.app.world_mut(), mail_id)
            }
            ClientPacket::CollectParcel { mail_id } => {
                stage5_collect_mail_packet(self.app.world_mut(), mail_id)
            }
            ClientPacket::DeleteMail { mail_id } => {
                stage5_delete_mail_packet(self.app.world_mut(), mail_id)
            }
            ClientPacket::LockMail { mail_id, lock } => {
                stage5_lock_mail_packet(self.app.world_mut(), mail_id, lock)
            }
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
                    queue.pending_ground_spell_actions = Vec::new();
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
                if current_player_is_dead(self.app.world())
                    || crystal_player_movement_blocked_by_status(self.app.world())
                {
                    if is_in_world(self.app.world()) {
                        self.app
                            .world_mut()
                            .resource_mut::<RuntimeQueueResource>()
                            .pending_movement_command = None;
                    }
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                if is_in_world(self.app.world())
                    && !crystal_packet_action_ready(self.app.world(), PlayerActionKind::Move)
                {
                    queue_crystal_movement_retry(self.app.world_mut(), direction, false);
                    return Vec::new();
                }
                if is_in_world(self.app.world()) {
                    let delay_ticks = crystal_packet_move_delay_ticks(false)
                        + if crystal_player_slowed_by_status(self.app.world()) {
                            1
                        } else {
                            0
                        };
                    mark_crystal_packet_action(
                        self.app.world_mut(),
                        PlayerActionKind::Move,
                        delay_ticks,
                    );
                }
                self.move_player_by_direction(direction, false)
            }
            ClientPacket::Run { direction } => {
                dismiss_dialog(self.app.world_mut());
                if current_player_is_dead(self.app.world())
                    || crystal_player_movement_blocked_by_status(self.app.world())
                {
                    if is_in_world(self.app.world()) {
                        self.app
                            .world_mut()
                            .resource_mut::<RuntimeQueueResource>()
                            .pending_movement_command = None;
                    }
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                if is_in_world(self.app.world())
                    && !crystal_packet_action_ready(self.app.world(), PlayerActionKind::Move)
                {
                    queue_crystal_movement_retry(self.app.world_mut(), direction, true);
                    return Vec::new();
                }
                if is_in_world(self.app.world()) {
                    let delay_ticks = crystal_packet_move_delay_ticks(true)
                        + if crystal_player_slowed_by_status(self.app.world()) {
                            1
                        } else {
                            0
                        };
                    mark_crystal_packet_action(
                        self.app.world_mut(),
                        PlayerActionKind::Move,
                        delay_ticks,
                    );
                }
                self.move_player_by_direction(direction, true)
            }
            ClientPacket::Chat {
                message,
                linked_items,
            } => handle_chat_packet(self.app.world_mut(), message, linked_items),
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
                    return use_hero_inventory_item_packet(self.app.world_mut(), unique_id);
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
            ClientPacket::Attack { direction, spell } => {
                if current_player_is_dead(self.app.world())
                    || crystal_player_attack_blocked_by_status(self.app.world())
                {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                if is_in_world(self.app.world())
                    && !crystal_packet_action_ready(self.app.world(), PlayerActionKind::Attack)
                {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                if is_in_world(self.app.world()) {
                    let delay_ticks = crystal_packet_attack_delay_ticks()
                        + if crystal_player_slowed_by_status(self.app.world()) {
                            1
                        } else {
                            0
                        };
                    mark_crystal_packet_action(
                        self.app.world_mut(),
                        PlayerActionKind::Attack,
                        delay_ticks,
                    );
                }
                self.attack_in_direction_with_spell(direction, spell)
            }
            ClientPacket::RangeAttack {
                direction,
                target_id,
                target_location,
                ..
            } => {
                if current_player_is_dead(self.app.world())
                    || crystal_player_attack_blocked_by_status(self.app.world())
                {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                if is_in_world(self.app.world())
                    && !crystal_packet_action_ready(self.app.world(), PlayerActionKind::Attack)
                {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                if is_in_world(self.app.world()) {
                    let delay_ticks = crystal_packet_attack_delay_ticks()
                        + if crystal_player_slowed_by_status(self.app.world()) {
                            1
                        } else {
                            0
                        };
                    mark_crystal_packet_action(
                        self.app.world_mut(),
                        PlayerActionKind::Attack,
                        delay_ticks,
                    );
                }
                self.range_attack_impl(direction, target_id, target_location)
            }
            ClientPacket::Harvest { direction } => self.harvest_impl(direction),
            ClientPacket::CallNpc { object_id, key } => self.call_npc_impl(object_id, &key),
            ClientPacket::NpcConfirmInput {
                npc_id,
                page_name,
                value,
            } => self.confirm_npc_input_impl(npc_id, &page_name, &value),
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
                if current_player_is_dead(self.app.world())
                    || crystal_player_magic_blocked_by_status(self.app.world())
                {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
                let Some(skill_key) = skill_key_for_crystal_spell(spell) else {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                };
                if !crystal_packet_action_ready(self.app.world(), PlayerActionKind::Spell) {
                    return vec![ServerPacket::UserLocation {
                        location: current_location(self.app.world()),
                    }];
                }
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
                mark_crystal_packet_action(
                    self.app.world_mut(),
                    PlayerActionKind::Spell,
                    crystal_packet_spell_delay_ticks(),
                );
                let mut response = vec![ServerPacket::UserLocation {
                    location: current_location(self.app.world()),
                }];
                response.extend(packets);
                response
            }
            ClientPacket::SpellToggle {
                spell,
                toggle_state,
            } => stage5_spell_toggle_packet(self.app.world_mut(), spell, toggle_state),
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
                let hero_can_spawn = !current_map_disallows_hero(self.app.world());
                if let Some(hero) = self
                    .app
                    .world_mut()
                    .resource_mut::<Stage5SystemsResource>()
                    .stage5_systems
                    .hero
                    .as_mut()
                {
                    hero.spawned = hero_can_spawn;
                }
                if hero_can_spawn {
                    let _ = spawn_stage5_hero(self.app.world_mut());
                }
                let state = stage5_hero_spawn_state(
                    self.app
                        .world()
                        .resource::<Stage5SystemsResource>()
                        .stage5_systems
                        .hero
                        .as_ref(),
                );
                let mut packets = vec![
                    ServerPacket::ChangeHero {
                        from_index: list_index,
                    },
                    stage5_manage_heroes_packet(self.app.world()),
                    ServerPacket::UpdateHeroSpawnState { state },
                ];
                if let Some(info) = stage5_hero_information_packet(self.app.world()) {
                    packets.push(info);
                }
                if !hero_can_spawn {
                    packets.push(system_message_key(
                        self.app.world(),
                        "server.CannotSummonHeroOnMap",
                    ));
                }
                packets
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
