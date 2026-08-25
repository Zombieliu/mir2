use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_game_shop_info_packet_payloads, crystal_item_by_index, format_localized_text,
    localized_text_or_fallback,
};
use mir2_protocol::{
    ChatType, GameShopItem, MirClass, MirDirection, MirGender, Point, ServerPacket, ServerPacketId,
};
use serde::{Deserialize, Serialize};

use crate::config::{
    crystal_base_vitals, new_stage5_mail_delivery_nonce, AccountStore, CharacterRecord,
    CurrencyKind, EquipmentSlot, ItemContainer, ItemGrade, Stage5AuctionListing, Stage5GuildState,
    Stage5HeroState, Stage5MailMessage, Stage5SystemsState, Stage5TradeState,
    WorldEntityDisposition,
};
use crate::{NativeGameShopPurchaseRequest, NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2};

use super::components::{
    entity_by_object_id, entity_position, player_entity, DisplayName, Facing, Npc, NpcAgent,
    ObjectId, PlayerVitals, Position, WorldObject,
};
use super::crystal_compat::{
    CRYSTAL_ITEM_SEAL_DELAY_MINUTES, CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_DC,
};
use super::equipment::{
    damage_equipment_item, equipment_slot_from_stage5_arg, equipment_slot_unique_id,
    equipment_uses_durability, EquipmentState,
};
use super::inventory::{
    add_minutes_to_binary_datetime, add_or_increment_item,
    additional_slots_needed_for_item_quantity, allocate_item_unique_id, binary_datetime_ticks,
    can_gain_item_quantity, crystal_duration_label_from_seconds, crystal_npc_storage_open_packets,
    current_binary_datetime, find_empty_inventory_item_slot, free_bag_slots,
    future_binary_datetime_minutes, item_heal_values_for_key, normalize_fresh_item_tree_unique_ids,
};
use super::items::{
    crystal_equipment_slot_for_item_key, crystal_item_key_for_template, crystal_item_stat_value,
    crystal_item_template_for_item_key, crystal_seal_minutes_for_source_item,
    crystal_socket_slot_limit_for_item_key, crystal_socket_source_valid_for_item,
    item_icon_for_key, validate_committed_item_state_carrier, ItemState,
};
use super::map::spawn_stage5_hero;
use super::monsters::{
    crystal_dynamic_monster_template, crystal_spawn_candidates_on_map, spawn_runtime_monster,
};
use super::npc::ActiveNpcServiceState;
use super::packets::{
    decode_crystal_payload, object_health_info_for_entity, stage5_append_mail_to_save,
    stage5_guild_request_war_packet,
};
use super::resources::{
    is_in_world, InventoryResource, MapRuntimeResource, NpcStateResource, PlayerRuntimeResource,
    RuntimeConfigResource, SessionResource, Stage5SystemsResource,
};
use super::save::{
    apply_character_save, decode_state_vec, merge_persisted_mail_into_character_save,
    snapshot_active_character_save, validate_character_save_record,
};
use super::session::{current_language, system_message, SimulationSession};
use super::social_economy::{
    stage5_mail_exact_item_slots, stage5_social_add_friend_entry, stage5_trade_item_can_enter,
    Stage5SocialAddResult,
};

pub(super) fn stage5_player_name(world: &World) -> String {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "Scout".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage5MailClaimError {
    NotFound,
    AlreadyClaimed,
    InvalidExactItemState,
    Capacity,
    BalanceOverflow,
}

#[derive(Debug, Clone)]
pub(super) struct Stage5MailClaimOutcome {
    pub(super) gold: u32,
    pub(super) items: Vec<ItemState>,
}

const STAGE5_GUILD_STORAGE_SLOT_COUNT: usize = 112;

fn validate_exact_stage5_item_state(item: &ItemState) -> Result<(), String> {
    validate_committed_item_state_carrier(item)
        .map_err(|error| format!("invalid exact stage5 item carrier: {error}"))?;
    if item.key.trim().is_empty() || item.name.trim().is_empty() {
        return Err("exact stage5 item identity is empty".to_string());
    }
    if crystal_item_template_for_item_key(&item.key).is_none() {
        return Err(format!("unknown exact stage5 item key: {}", item.key));
    }
    for child in &item.socketed {
        validate_exact_stage5_item_state(child)?;
    }
    Ok(())
}

pub(super) fn exact_mail_item_state_is_valid(item: &ItemState) -> bool {
    validate_exact_stage5_item_state(item).is_ok()
}

pub(super) fn validate_stage5_mail_item_carriers(mail: &[Stage5MailMessage]) -> Result<(), String> {
    for (mail_index, message) in mail.iter().enumerate() {
        for (attachment_index, encoded) in message.item_states_json.iter().enumerate() {
            let item = serde_json::from_str::<ItemState>(encoded).map_err(|error| {
                format!(
                    "failed to decode stage5 mail {mail_index} attachment {attachment_index}: {error}"
                )
            })?;
            validate_exact_stage5_item_state(&item).map_err(|error| {
                format!("invalid stage5 mail {mail_index} attachment {attachment_index}: {error}")
            })?;
        }
    }
    Ok(())
}

pub(super) fn validate_stage5_guild_storage_item_carriers(
    guild: &Stage5GuildState,
) -> Result<(), String> {
    let item_slots = guild.storage_items.keys().copied().collect::<Vec<_>>();
    let state_slots = guild
        .storage_item_states
        .keys()
        .copied()
        .collect::<Vec<_>>();
    if item_slots != state_slots {
        return Err(format!(
            "stage5 guild storage key/state slot-set mismatch: keys {item_slots:?}, states {state_slots:?}"
        ));
    }
    let owner_slots = guild.storage_item_users.keys().copied().collect::<Vec<_>>();
    if item_slots != owner_slots {
        return Err(format!(
            "stage5 guild storage key/owner slot-set mismatch: keys {item_slots:?}, owners {owner_slots:?}"
        ));
    }

    for (slot, key) in &guild.storage_items {
        if usize::from(*slot) >= STAGE5_GUILD_STORAGE_SLOT_COUNT {
            return Err(format!("stage5 guild storage slot {slot} is out of range"));
        }
        if key.trim().is_empty() || crystal_item_template_for_item_key(key).is_none() {
            return Err(format!(
                "stage5 guild storage slot {slot} has unknown item key: {key}"
            ));
        }
        let encoded = guild.storage_item_states.get(slot).ok_or_else(|| {
            format!("stage5 guild storage slot {slot} is missing exact item state")
        })?;
        let item = serde_json::from_str::<ItemState>(encoded).map_err(|error| {
            format!("failed to decode stage5 guild storage slot {slot}: {error}")
        })?;
        if item.key != *key {
            return Err(format!(
                "stage5 guild storage slot {slot} key/state mismatch: {key} != {}",
                item.key
            ));
        }
        validate_exact_stage5_item_state(&item).map_err(|error| {
            format!("invalid stage5 guild storage slot {slot} item carrier: {error}")
        })?;
    }

    Ok(())
}
pub(super) fn validate_stage5_systems_item_carriers(
    systems: &Stage5SystemsState,
) -> Result<(), String> {
    validate_stage5_mail_item_carriers(&systems.mail)?;
    validate_stage5_guild_storage_item_carriers(&systems.guild)
}

#[cfg(test)]
mod stage5_item_carrier_validation_tests {
    use super::*;
    use mir2_game_data::LanguageCode;
    use mir2_protocol::ClientPacket;

    fn mail_with_attachment(encoded: String) -> Stage5MailMessage {
        Stage5MailMessage {
            id: 1,
            delivery_nonce: "stage5-carrier-test".to_string(),
            from: "System".to_string(),
            to: "Demo".to_string(),
            subject: "Carrier".to_string(),
            body: "Carrier validation".to_string(),
            gold: 0,
            items: Vec::new(),
            item_states_json: vec![encoded],
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        }
    }

    fn valid_game_shop_attachment() -> ItemState {
        let (product, _) =
            authoritative_game_shop_product(31).expect("fixture game-shop product should exist");
        let template =
            crystal_item_by_index(product.item_index).expect("fixture item template should exist");
        let key = crystal_item_key_for_template(&template);
        let encoded = game_shop_attachment_states_json(&template, &key, u32::from(product.count))
            .expect("fixture attachment should be valid");
        serde_json::from_str(&encoded[0]).expect("fixture attachment should decode")
    }

    fn over_budget_attachment() -> ItemState {
        let mut attachment = valid_game_shop_attachment();
        for _ in 0..=9 {
            let mut parent = attachment.clone();
            parent.socket_slots = 1;
            parent.gem_count = 1;
            parent.socketed = vec![attachment];
            parent.user_item_metadata = None;
            attachment = parent;
        }
        attachment
    }

    fn unknown_index_attachment_json() -> String {
        let mut attachment = serde_json::to_value(valid_game_shop_attachment()).unwrap();
        attachment["user_item_metadata"] = serde_json::json!({
            "item_index": i32::MAX,
        });
        serde_json::to_string(&attachment).unwrap()
    }

    fn metadata_only_attachment_with_child_count(count: u16) -> ItemState {
        let child_template =
            mir2_game_data::crystal_item_by_name("BronzeHelmet").expect("BronzeHelmet must exist");
        let child_key = crystal_item_key_for_template(&child_template);
        let child_json = game_shop_attachment_states_json(&child_template, &child_key, 1)
            .expect("BronzeHelmet child fixture should be valid");
        let child_state: ItemState =
            serde_json::from_str(&child_json[0]).expect("BronzeHelmet child should decode");
        let mut child = super::super::items::try_user_item_from_item_state(&child_state)
            .expect("BronzeHelmet child should convert to UserItem");
        child.unique_id = 9_801;
        child.count = count;

        let mut attachment = valid_game_shop_attachment();
        attachment.unique_id = 9_800;
        attachment.socket_slots = 1;
        let root_template = crystal_item_template_for_item_key(&attachment.key)
            .expect("root attachment template should exist");
        attachment.user_item_metadata = Some(
            serde_json::from_value(serde_json::json!({
                "item_index": root_template.item_index,
                "slots": [child],
            }))
            .expect("metadata-only sidecar should decode"),
        );
        assert!(attachment.socketed.is_empty());
        attachment
    }

    fn guild_systems_with_state(encoded: String) -> Stage5SystemsState {
        let key = valid_game_shop_attachment().key;
        let mut systems = Stage5SystemsState::default();
        systems.guild.name = "Carrier Test Guild".to_string();
        systems.guild.storage_items.insert(0, key);
        systems.guild.storage_item_states.insert(0, encoded);
        systems.guild.storage_item_users.insert(0, 1);
        systems
    }

    #[test]
    fn stage5_mail_attachment_reuses_complete_carrier_budget() {
        let attachment = over_budget_attachment();
        let systems = Stage5SystemsState {
            mail: vec![mail_with_attachment(
                serde_json::to_string(&attachment).unwrap(),
            )],
            ..Stage5SystemsState::default()
        };

        let error = validate_stage5_systems_item_carriers(&systems).unwrap_err();
        assert!(error.contains("attachment 0"));
        assert!(error.contains("depth") || error.contains("Depth"));
    }

    #[test]
    fn stage5_guild_storage_rejects_malformed_unknown_and_over_budget_carriers_without_mutation() {
        let cases = [
            "{corrupt guild storage JSON".to_string(),
            unknown_index_attachment_json(),
            serde_json::to_string(&over_budget_attachment()).unwrap(),
        ];

        for encoded in cases {
            let systems = guild_systems_with_state(encoded);
            let before = systems.clone();
            let error = validate_stage5_systems_item_carriers(&systems).unwrap_err();
            assert!(
                error.contains("decode")
                    || error.contains("Unknown")
                    || error.contains("unknown")
                    || error.contains("depth")
                    || error.contains("Depth")
                    || error.contains("no Crystal template"),
                "unexpected strict validation error: {error}"
            );
            assert_eq!(systems, before);
        }
    }

    #[test]
    fn stage5_guild_storage_rejects_metadata_only_zero_and_overstack_children_without_mutation() {
        let bronze_helmet =
            mir2_game_data::crystal_item_by_name("BronzeHelmet").expect("BronzeHelmet must exist");
        assert_eq!(bronze_helmet.stack_size, 1);

        for count in [0, bronze_helmet.stack_size.saturating_add(1)] {
            let attachment = metadata_only_attachment_with_child_count(count);
            super::super::items::validate_item_state_carrier(&attachment)
                .expect("generic carrier should preserve transient metadata-only child counts");
            let systems = guild_systems_with_state(
                serde_json::to_string(&attachment).expect("attachment should encode"),
            );
            let before = systems.clone();

            let error = validate_stage5_systems_item_carriers(&systems).unwrap_err();
            assert!(
                error.contains("committed UserItem")
                    && error.contains(&format!("quantity {count}"))
                    && error.contains("outside Crystal stack range"),
                "unexpected committed quantity error for count {count}: {error}"
            );
            assert_eq!(systems, before);
        }
    }

    #[test]
    fn stage5_guild_storage_accepts_complete_exact_slot_sets_without_mutation() {
        let item = valid_game_shop_attachment();
        let systems = guild_systems_with_state(serde_json::to_string(&item).unwrap());
        let before = systems.clone();

        validate_stage5_systems_item_carriers(&systems)
            .expect("complete key/state/owner slot sets should validate");
        assert_eq!(systems, before);
    }

    #[test]
    fn stage5_guild_storage_rejects_missing_owner_without_mutation() {
        let item = valid_game_shop_attachment();
        let mut systems = guild_systems_with_state(serde_json::to_string(&item).unwrap());
        systems.guild.storage_item_users.remove(&0);
        let before = systems.clone();

        assert!(validate_stage5_systems_item_carriers(&systems)
            .unwrap_err()
            .contains("key/owner slot-set mismatch"));
        assert_eq!(systems, before);
    }

    #[test]
    fn stage5_guild_storage_rejects_equal_length_orphan_owner_set_without_mutation() {
        let item = valid_game_shop_attachment();
        let mut systems = guild_systems_with_state(serde_json::to_string(&item).unwrap());
        let owner = systems.guild.storage_item_users.remove(&0).unwrap();
        systems.guild.storage_item_users.insert(1, owner);
        let before = systems.clone();

        assert!(validate_stage5_systems_item_carriers(&systems)
            .unwrap_err()
            .contains("key/owner slot-set mismatch"));
        assert_eq!(systems, before);
    }

    #[test]
    fn stage5_guild_storage_rejects_equal_length_missing_state_set_without_mutation() {
        let item = valid_game_shop_attachment();
        let mut systems = guild_systems_with_state(serde_json::to_string(&item).unwrap());
        let encoded = systems.guild.storage_item_states.remove(&0).unwrap();
        systems.guild.storage_item_states.insert(1, encoded);
        let before = systems.clone();

        assert!(validate_stage5_systems_item_carriers(&systems)
            .unwrap_err()
            .contains("key/state slot-set mismatch"));
        assert_eq!(systems, before);
    }
    #[test]
    fn qa_apply_native_state_rejects_corrupt_stage5_without_world_or_session_mutation() {
        let config = crate::config::SimulationConfig::default();
        let mut session = SimulationSession::new(config.clone());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let active_save = super::super::save::default_save_for_character(
            &config,
            config.default_character.clone(),
        );
        apply_character_save(session.app.world_mut(), &active_save)
            .expect("qa atomicity fixture should enter a valid active state");
        super::super::map::rebuild_world(session.app.world_mut());

        let save = snapshot_active_character_save(session.app.world()).unwrap();
        let payload = serde_json::json!({
            "character": {
                "name": save.character.name,
                "level": save.character.level,
                "class": save.character.class,
                "gender": save.character.gender,
            },
            "mapFileName": save.map_file_name,
            "mapTitle": save.map_title,
            "position": save.position,
            "direction": save.direction,
            "hp": save.hp,
            "maxHp": save.max_hp,
            "mp": save.mp,
            "maxMp": save.max_mp,
            "experience": save.experience,
            "maxExperience": save.max_experience,
            "gold": save.gold,
            "credit": save.credit,
            "cityCurrencies": save.city_currencies,
            "inventoryItemsJson": save.inventory_items_json,
            "beltItemsJson": save.belt_items_json,
            "storageItemsJson": save.storage_items_json,
            "equipmentItemsJson": save.equipment_items_json,
        })
        .to_string();

        let mut invalid_carrier_payload =
            serde_json::from_str::<serde_json::Value>(&payload).unwrap();
        invalid_carrier_payload["inventoryItemsJson"] =
            serde_json::json!(["{corrupt inventory item JSON"]);
        let carrier_world_before = session.world_snapshot();
        let carrier_active_before =
            serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                .unwrap();
        let carrier_session_before = {
            let state = session.app.world().resource::<SessionResource>();
            (
                state.selected_character.clone(),
                state.active_save_revision(),
            )
        };
        let carrier_packets =
            session.stage5_qa_apply_native_state(vec![invalid_carrier_payload.to_string()]);
        let expected = format_localized_text(
            LanguageCode::English,
            "server.InvalidPacketReceived",
            ["qa.applyNativeState item state".to_string()],
        );
        assert!(carrier_packets.iter().any(
            |packet| matches!(packet, ServerPacket::Chat { message, .. } if message == &expected)
        ));
        assert_eq!(session.world_snapshot(), carrier_world_before);
        assert_eq!(
            serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                .unwrap(),
            carrier_active_before
        );
        let state = session.app.world().resource::<SessionResource>();
        assert_eq!(
            (
                state.selected_character.clone(),
                state.active_save_revision(),
            ),
            carrier_session_before
        );

        session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail
            .push(mail_with_attachment("{corrupt exact item JSON".to_string()));

        let world_before = session.world_snapshot();
        let active_before =
            serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                .unwrap();
        let session_before = {
            let state = session.app.world().resource::<SessionResource>();
            (
                state.selected_character.clone(),
                state.active_save_revision(),
            )
        };

        let packets = session.stage5_qa_apply_native_state(vec![payload.clone()]);
        assert!(packets.iter().any(
            |packet| matches!(packet, ServerPacket::Chat { message, .. } if message == &expected)
        ));
        assert_eq!(session.world_snapshot(), world_before);
        assert_eq!(
            serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                .unwrap(),
            active_before
        );
        let state = session.app.world().resource::<SessionResource>();
        assert_eq!(
            (
                state.selected_character.clone(),
                state.active_save_revision(),
            ),
            session_before
        );

        session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail
            .clear();
        let guild_cases = [
            "{corrupt guild storage JSON".to_string(),
            unknown_index_attachment_json(),
            serde_json::to_string(&over_budget_attachment()).unwrap(),
        ];
        for encoded in guild_cases {
            let replacement = guild_systems_with_state(encoded).guild;
            session
                .app
                .world_mut()
                .resource_mut::<Stage5SystemsResource>()
                .stage5_systems
                .guild = replacement;

            let world_before = session.world_snapshot();
            let guild_before = session
                .app
                .world()
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .guild
                .clone();
            let active_before =
                serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                    .unwrap();
            let session_before = {
                let state = session.app.world().resource::<SessionResource>();
                (
                    state.selected_character.clone(),
                    state.active_save_revision(),
                )
            };

            let packets = session.stage5_qa_apply_native_state(vec![payload.clone()]);
            assert!(packets.iter().any(
                |packet| matches!(packet, ServerPacket::Chat { message, .. } if message == &expected)
            ));
            assert_eq!(session.world_snapshot(), world_before);
            assert_eq!(
                session
                    .app
                    .world()
                    .resource::<Stage5SystemsResource>()
                    .stage5_systems
                    .guild,
                guild_before
            );
            assert_eq!(
                serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                    .unwrap(),
                active_before
            );
            let state = session.app.world().resource::<SessionResource>();
            assert_eq!(
                (
                    state.selected_character.clone(),
                    state.active_save_revision(),
                ),
                session_before
            );
        }
    }
}

pub(super) fn stage5_claim_mail_authoritative(
    world: &mut World,
    mail_id: u32,
) -> Result<Stage5MailClaimOutcome, Stage5MailClaimError> {
    let (mail_index, gold, keyed_items, item_states_json) = {
        let stage5 = world.resource::<Stage5SystemsResource>();
        let Some(mail_index) = stage5
            .stage5_systems
            .mail
            .iter()
            .position(|mail| mail.id == mail_id && !mail.deleted)
        else {
            return Err(Stage5MailClaimError::NotFound);
        };
        let mail = &stage5.stage5_systems.mail[mail_index];
        if mail.claimed {
            return Err(Stage5MailClaimError::AlreadyClaimed);
        }
        (
            mail_index,
            mail.gold,
            mail.items.clone(),
            mail.item_states_json.clone(),
        )
    };

    // Exact attachment state is authoritative. Once present, it must parse and
    // validate as one complete batch; corruption must never fall back to the
    // lossy legacy key list.
    let has_exact_item_states = !item_states_json.is_empty();
    let item_states = if has_exact_item_states {
        let parsed = item_states_json
            .iter()
            .map(|state| serde_json::from_str::<ItemState>(state))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
        if parsed
            .iter()
            .any(|item| validate_exact_stage5_item_state(item).is_err())
        {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
        parsed
    } else {
        Vec::new()
    };
    let keyed_items = if has_exact_item_states {
        Vec::new()
    } else {
        keyed_items
    };

    let exact_item_slots = {
        let resources = world.resource::<InventoryResource>();
        let slots = stage5_mail_exact_item_slots(&resources.inventory_items, &item_states)
            .ok_or(Stage5MailClaimError::Capacity)?;
        let mut keyed_quantities = BTreeMap::<&str, u32>::new();
        for key in &keyed_items {
            *keyed_quantities.entry(key.as_str()).or_default() += 1;
        }
        let needed_legacy_slots = keyed_quantities
            .into_iter()
            .try_fold(0u32, |needed, (key, quantity)| {
                needed.checked_add(additional_slots_needed_for_item_quantity(
                    resources,
                    ItemContainer::Bag1,
                    key,
                    quantity,
                ))
            })
            .ok_or(Stage5MailClaimError::Capacity)?;
        if needed_legacy_slots > u32::from(free_bag_slots(resources)) {
            return Err(Stage5MailClaimError::Capacity);
        }
        slots
    };

    let next_gold = world
        .resource::<PlayerRuntimeResource>()
        .gold
        .checked_add(gold)
        .ok_or(Stage5MailClaimError::BalanceOverflow)?;

    // No fallible validation remains after this point. Commit currency,
    // inventory, and mailbox state as one authoritative transaction.
    if gold > 0 {
        world.resource_mut::<PlayerRuntimeResource>().gold = next_gold;
    }
    let mut gained_items = Vec::with_capacity(item_states.len() + keyed_items.len());
    for (mut item, (container, slot)) in item_states.into_iter().zip(exact_item_slots) {
        item.container = container;
        item.slot = slot;
        normalize_fresh_item_tree_unique_ids(world.resource::<InventoryResource>(), &mut item, &[]);
        gained_items.push(item.clone());
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
    }
    for key in keyed_items {
        gained_items.push(add_or_increment_item(
            world,
            ItemContainer::Bag1,
            &key,
            &stage5_item_name(&key),
            "Stage 5 mail attachment.",
            20,
            1,
            1,
        ));
    }
    {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        let mail = &mut stage5.stage5_systems.mail[mail_index];
        mail.claimed = true;
        mail.gold = 0;
        mail.items.clear();
        mail.item_states_json.clear();
    }

    Ok(Stage5MailClaimOutcome {
        gold,
        items: gained_items,
    })
}

pub(super) fn push_unique(values: &mut Vec<String>, value: String) {
    if !values
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&value))
    {
        values.push(value);
    }
}

pub(super) fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        push_unique(&mut result, value);
    }
    result
}

fn stage5_guild_rank_is_leader(rank: &str) -> bool {
    let rank = rank.trim();
    rank.is_empty()
        || rank.eq_ignore_ascii_case("Guild Chief")
        || rank.eq_ignore_ascii_case("Leader")
}

fn stage5_guild_permission_key(permission: &str) -> String {
    permission
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn stage5_guild_can_alter_alliance(guild: &Stage5GuildState) -> bool {
    stage5_guild_rank_is_leader(&guild.rank)
        || guild.permissions.iter().any(|permission| {
            matches!(
                stage5_guild_permission_key(permission).as_str(),
                "alteralliance" | "alliance" | "conquest"
            )
        })
}

fn stage5_guild_canonical_alliance_target(
    guild: &Stage5GuildState,
    territory_owner: &str,
    name: &str,
) -> Option<String> {
    let target = name.trim();
    if target.is_empty() {
        return None;
    }
    if guild.name.eq_ignore_ascii_case(target) {
        return Some(guild.name.clone());
    }
    guild
        .known_guilds
        .iter()
        .find(|known| known.eq_ignore_ascii_case(target))
        .cloned()
        .or_else(|| {
            guild
                .allied_guilds
                .iter()
                .find(|ally| ally.eq_ignore_ascii_case(target))
                .cloned()
        })
        .or_else(|| {
            let territory_owner = territory_owner.trim();
            (!territory_owner.is_empty() && territory_owner.eq_ignore_ascii_case(target))
                .then(|| territory_owner.to_string())
        })
}

fn stage5_guild_is_allied(guild: &Stage5GuildState, target: &str) -> bool {
    guild
        .allied_guilds
        .iter()
        .any(|ally| ally.eq_ignore_ascii_case(target))
}

pub(super) fn parse_u32_arg(args: &[String], index: usize) -> Option<u32> {
    args.get(index).and_then(|value| value.parse::<u32>().ok())
}

const CRYSTAL_GAME_SHOP_MAX_QUANTITY: u8 = 99;
const CRYSTAL_GAME_SHOP_MAX_ATTACHMENT_STACKS: u32 = 5;
const CRYSTAL_MAIL_CAPACITY: usize = 100;
const GAME_SHOP_STOCK_UNAVAILABLE_AT_COMMIT: &str = "game-shop stock unavailable at commit:";
const STALE_POSTGRES_GAME_SHOP_GLOBAL_STOCK: &str = "stale postgres game-shop global-stock write";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameShopPriceType {
    Credit,
    Gold,
}

impl TryFrom<i32> for GameShopPriceType {
    type Error = GameShopPurchaseFailure;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            // Crystal `PlayerObject.GameshopBuy`: 0 = Credit, 1 = Gold.
            0 => Ok(Self::Credit),
            1 => Ok(Self::Gold),
            _ => Err(GameShopPurchaseFailure::InvalidPriceType),
        }
    }
}

impl GameShopPriceType {
    const fn raw(self) -> i32 {
        match self {
            Self::Credit => 0,
            Self::Gold => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameShopPurchaseFailure {
    NotInGame,
    InvalidPriceType,
    InvalidQuantity,
    UnknownProduct,
    ClassUnavailable,
    PaymentUnavailable,
    StockUnavailable,
    InsufficientCurrency,
    MailFull,
    CommitFailed,
}

impl GameShopPurchaseFailure {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotInGame => "notInGame",
            Self::InvalidPriceType => "invalidPriceType",
            Self::InvalidQuantity => "invalidQuantity",
            Self::UnknownProduct => "unknownProduct",
            Self::ClassUnavailable => "classUnavailable",
            Self::PaymentUnavailable => "paymentUnavailable",
            Self::StockUnavailable => "stockUnavailable",
            Self::InsufficientCurrency => "insufficientCurrency",
            Self::MailFull => "mailFull",
            Self::CommitFailed => "commitFailed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "GameShopPurchaseOutcomeWire")]
pub struct GameShopPurchaseOutcome {
    pub success: bool,
    pub g_index: i32,
    pub quantity: u8,
    pub price_type: i32,
    pub new_stock_level: Option<i32>,
    pub mail_id: Option<u64>,
    pub failure: Option<GameShopPurchaseFailure>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GameShopPurchaseOutcomeWire {
    success: bool,
    g_index: i32,
    quantity: u8,
    price_type: i32,
    new_stock_level: Option<i32>,
    mail_id: Option<u64>,
    failure: Option<GameShopPurchaseFailure>,
}

impl TryFrom<GameShopPurchaseOutcomeWire> for GameShopPurchaseOutcome {
    type Error = &'static str;

    fn try_from(wire: GameShopPurchaseOutcomeWire) -> Result<Self, Self::Error> {
        if wire.success {
            if wire.mail_id.is_none() || wire.failure.is_some() {
                return Err("successful game-shop outcome requires mailId and forbids failure");
            }
        } else {
            let Some(failure) = wire.failure else {
                return Err("failed game-shop outcome requires failure");
            };
            if wire.mail_id.is_some() {
                return Err("failed game-shop outcome forbids mailId");
            }
            if wire.new_stock_level.is_some()
                && failure != GameShopPurchaseFailure::StockUnavailable
            {
                return Err("only stockUnavailable may carry newStockLevel");
            }
        }
        Ok(Self {
            success: wire.success,
            g_index: wire.g_index,
            quantity: wire.quantity,
            price_type: wire.price_type,
            new_stock_level: wire.new_stock_level,
            mail_id: wire.mail_id,
            failure: wire.failure,
        })
    }
}

impl GameShopPurchaseOutcome {
    fn success(
        g_index: i32,
        quantity: u8,
        price_type: i32,
        new_stock_level: Option<i32>,
        mail_id: u64,
    ) -> Self {
        Self {
            success: true,
            g_index,
            quantity,
            price_type,
            new_stock_level,
            mail_id: Some(mail_id),
            failure: None,
        }
    }

    fn failure(
        g_index: i32,
        quantity: u8,
        price_type: i32,
        failure: GameShopPurchaseFailure,
        new_stock_level: Option<i32>,
    ) -> Self {
        debug_assert!(
            new_stock_level.is_none() || failure == GameShopPurchaseFailure::StockUnavailable
        );
        Self {
            success: false,
            g_index,
            quantity,
            price_type,
            new_stock_level,
            mail_id: None,
            failure: Some(failure),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameShopPurchaseExecution {
    pub packets: Vec<ServerPacket>,
    pub outcome: GameShopPurchaseOutcome,
}

const NATIVE_GAME_SHOP_LEDGER_FROM: &str = "Mir2.Internal";
const NATIVE_GAME_SHOP_LEDGER_SUBJECT: &str = "NativeGameShopLedgerV2";
const NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE: &str = "native-gameshop-ledger-v2";
const NATIVE_GAME_SHOP_LEDGER_MAX_ENTRIES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeGameShopLedgerEntry {
    server_idempotency_key: String,
    gateway_session_id: String,
    client_request_id: String,
    g_index: i32,
    quantity: u8,
    price_type: i32,
    outcome: GameShopPurchaseOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeGameShopDurableLedger {
    protocol_version: u16,
    account_id: String,
    character_index: i32,
    entries: Vec<NativeGameShopLedgerEntry>,
}

fn is_native_game_shop_ledger_mail(mail: &Stage5MailMessage) -> bool {
    mail.deleted
        && mail.from == NATIVE_GAME_SHOP_LEDGER_FROM
        && mail.subject == NATIVE_GAME_SHOP_LEDGER_SUBJECT
}

fn decode_native_game_shop_ledger(
    mail: &Stage5MailMessage,
) -> Result<NativeGameShopDurableLedger, String> {
    serde_json::from_str(&mail.body)
        .map_err(|error| format!("failed to decode native GameShop ledger: {error}"))
}

fn validate_native_game_shop_ledger_mail_metadata(mail: &Stage5MailMessage) -> Result<(), String> {
    if !is_native_game_shop_ledger_mail(mail)
        || mail.delivery_nonce != NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE
        || !mail.locked
        || !mail.claimed
        || mail.gold != 0
        || !mail.items.is_empty()
        || !mail.item_states_json.is_empty()
    {
        return Err("native GameShop ledger mail metadata is invalid".to_string());
    }
    Ok(())
}

fn native_game_shop_ledger_mail_index(
    systems: &Stage5SystemsState,
) -> Result<Option<usize>, String> {
    let mut indexes = systems
        .mail
        .iter()
        .enumerate()
        .filter(|(_, mail)| {
            is_native_game_shop_ledger_mail(mail)
                || mail.delivery_nonce == NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE
        })
        .map(|(index, _)| index);
    let Some(index) = indexes.next() else {
        return Ok(None);
    };
    if indexes.next().is_some() {
        return Err("native GameShop durable save contains multiple ledgers".to_string());
    }
    validate_native_game_shop_ledger_mail_metadata(&systems.mail[index])?;
    Ok(Some(index))
}

/// Merge the one mutable hidden ledger by idempotency key. Ordinary Crystal
/// mail bodies are immutable, so the generic mailbox merge deliberately keeps
/// the live body; this internal row is the sole exception and must converge by
/// union or fail closed on any conflicting key/binding.
pub(super) fn merge_native_game_shop_ledger_mail(
    local: &mut Stage5MailMessage,
    external: &Stage5MailMessage,
) -> Result<Option<bool>, String> {
    let local_candidate = is_native_game_shop_ledger_mail(local)
        || local.delivery_nonce == NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE;
    let external_candidate = is_native_game_shop_ledger_mail(external)
        || external.delivery_nonce == NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE;
    if !local_candidate && !external_candidate {
        return Ok(None);
    }

    validate_native_game_shop_ledger_mail_metadata(local)?;
    validate_native_game_shop_ledger_mail_metadata(external)?;
    if local.to != external.to {
        return Err("native GameShop ledger mail recipient mismatch".to_string());
    }

    let mut local_ledger = decode_native_game_shop_ledger(local)?;
    let external_ledger = decode_native_game_shop_ledger(external)?;
    if local_ledger.protocol_version != NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2
        || external_ledger.protocol_version != NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2
        || local_ledger.account_id != external_ledger.account_id
        || local_ledger.character_index != external_ledger.character_index
    {
        return Err("native GameShop ledger merge binding mismatch".to_string());
    }

    let mut local_indexes = BTreeMap::new();
    for (index, entry) in local_ledger.entries.iter().enumerate() {
        if local_indexes
            .insert(entry.server_idempotency_key.clone(), index)
            .is_some()
        {
            return Err("native GameShop local ledger contains duplicate keys".to_string());
        }
    }
    let mut external_keys = BTreeMap::new();
    let mut changed = false;
    for entry in external_ledger.entries {
        if external_keys
            .insert(entry.server_idempotency_key.clone(), ())
            .is_some()
        {
            return Err("native GameShop external ledger contains duplicate keys".to_string());
        }
        if let Some(index) = local_indexes.get(&entry.server_idempotency_key).copied() {
            if local_ledger.entries[index] != entry {
                return Err(
                    "native GameShop ledger key has conflicting request or outcome".to_string(),
                );
            }
            continue;
        }
        local_indexes.insert(
            entry.server_idempotency_key.clone(),
            local_ledger.entries.len(),
        );
        local_ledger.entries.push(entry);
        changed = true;
    }
    if local_ledger.entries.len() > NATIVE_GAME_SHOP_LEDGER_MAX_ENTRIES {
        return Err("native GameShop merged ledger exceeds character capacity".to_string());
    }

    // Canonical ordering makes A∪B and B∪A byte-identical and prevents
    // stale sessions from oscillating the persisted JSON representation.
    local_ledger.entries.sort_by(|left, right| {
        left.server_idempotency_key
            .cmp(&right.server_idempotency_key)
    });
    let merged_body = serde_json::to_string(&local_ledger)
        .map_err(|error| format!("failed to encode merged native GameShop ledger: {error}"))?;
    if local.body != merged_body {
        local.body = merged_body;
        changed = true;
    }
    Ok(Some(changed))
}

fn validate_native_game_shop_purchase_request(
    request: &NativeGameShopPurchaseRequest,
) -> Result<(), String> {
    if request.protocol_version != NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2 {
        return Err("native GameShop purchase protocol version is unsupported".to_string());
    }
    let decoded_key = URL_SAFE_NO_PAD
        .decode(request.server_idempotency_key.as_bytes())
        .map_err(|_| "native GameShop server idempotency key is invalid".to_string())?;
    if decoded_key.len() != 32 {
        return Err("native GameShop server idempotency key must contain 256 bits".to_string());
    }
    if request.gateway_session_id.is_empty()
        || request.gateway_session_id.len() > 512
        || request.gateway_session_id.chars().any(char::is_control)
        || request.account_id.trim().is_empty()
        || request.account_id.len() > 256
        || request.account_id.chars().any(char::is_control)
        || request.client_request_id.is_empty()
        || request.client_request_id.len() > 64
        || !request.client_request_id.is_ascii()
        || request
            .client_request_id
            .bytes()
            .any(|byte| !(0x20..=0x7e).contains(&byte))
    {
        return Err("native GameShop purchase identity tuple is invalid".to_string());
    }
    Ok(())
}

fn native_game_shop_ledger_outcome(
    systems: &Stage5SystemsState,
    request: &NativeGameShopPurchaseRequest,
) -> Result<Option<GameShopPurchaseOutcome>, String> {
    let Some(ledger_index) = native_game_shop_ledger_mail_index(systems)? else {
        return Ok(None);
    };
    let mail = &systems.mail[ledger_index];
    let ledger = decode_native_game_shop_ledger(mail)?;
    if ledger.protocol_version != NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2
        || ledger.account_id != request.account_id
        || ledger.character_index != request.character_index
    {
        return Err("native GameShop ledger binding mismatch".to_string());
    }
    let Some(entry) = ledger
        .entries
        .iter()
        .find(|entry| entry.server_idempotency_key == request.server_idempotency_key)
    else {
        return Ok(None);
    };
    if entry.gateway_session_id != request.gateway_session_id
        || entry.client_request_id != request.client_request_id
        || entry.g_index != request.g_index
        || entry.quantity != request.quantity
        || entry.price_type != request.price_type
    {
        return Err("native GameShop idempotency key was reused with another request".to_string());
    }
    Ok(Some(entry.outcome.clone()))
}

/// Insert an outcome into the same durable mailbox document used by the
/// purchase transaction. The row is permanently deleted/locked and therefore
/// never consumes Crystal mailbox capacity or appears in ReceiveMail.
fn record_native_game_shop_ledger_outcome(
    systems: &mut Stage5SystemsState,
    player_name: &str,
    request: &NativeGameShopPurchaseRequest,
    outcome: &GameShopPurchaseOutcome,
) -> Result<Option<GameShopPurchaseOutcome>, String> {
    if let Some(existing) = native_game_shop_ledger_outcome(systems, request)? {
        return Ok(Some(existing));
    }

    let delivery_nonce = NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE.to_string();
    let ledger_index = native_game_shop_ledger_mail_index(systems)?;
    let mut ledger = match ledger_index {
        Some(index) => decode_native_game_shop_ledger(&systems.mail[index])?,
        None => NativeGameShopDurableLedger {
            protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
            account_id: request.account_id.clone(),
            character_index: request.character_index,
            entries: Vec::new(),
        },
    };
    if ledger.entries.len() >= NATIVE_GAME_SHOP_LEDGER_MAX_ENTRIES {
        return Err("native GameShop durable ledger is full for this character".to_string());
    }
    ledger.entries.push(NativeGameShopLedgerEntry {
        server_idempotency_key: request.server_idempotency_key.clone(),
        gateway_session_id: request.gateway_session_id.clone(),
        client_request_id: request.client_request_id.clone(),
        g_index: request.g_index,
        quantity: request.quantity,
        price_type: request.price_type,
        outcome: outcome.clone(),
    });
    let body = serde_json::to_string(&ledger)
        .map_err(|error| format!("failed to encode native GameShop ledger: {error}"))?;
    match ledger_index {
        Some(index) => systems.mail[index].body = body,
        None => {
            // Never evict a completed key merely because a newer Gateway
            // session appears. A delayed RPC from the older session must still
            // resolve to its original outcome. At the hard cap, purchases fail
            // closed rather than forgetting a possibly replayable mutation.
            let id = systems
                .mail
                .iter()
                .map(|mail| mail.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| "native GameShop ledger mail id exhausted".to_string())?;
            systems.mail.push(Stage5MailMessage {
                id,
                delivery_nonce,
                from: NATIVE_GAME_SHOP_LEDGER_FROM.to_string(),
                to: player_name.to_string(),
                subject: NATIVE_GAME_SHOP_LEDGER_SUBJECT.to_string(),
                body,
                gold: 0,
                items: Vec::new(),
                item_states_json: Vec::new(),
                opened: false,
                locked: true,
                claimed: true,
                deleted: true,
            });
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameShopPurchaseDetails {
    game_shop_index: i32,
    stock: i32,
    individual_stock: bool,
    purchase_quantity: u8,
    item_key: String,
    item_name: String,
    item_count: u32,
    attachment_states_json: Vec<String>,
    total_price: u32,
    price_type: GameShopPriceType,
}

fn game_shop_class_matches(product_class: &str, player_class: MirClass) -> bool {
    match product_class.trim().to_ascii_lowercase().as_str() {
        "" | "all" | "show all" => true,
        "warrior" => player_class == MirClass::Warrior,
        "wizard" => player_class == MirClass::Wizard,
        "taoist" => player_class == MirClass::Taoist,
        "assassin" => player_class == MirClass::Assassin,
        "archer" => player_class == MirClass::Archer,
        _ => false,
    }
}

fn authoritative_game_shop_product(game_shop_index: i32) -> Option<(GameShopItem, i32)> {
    crystal_game_shop_info_packet_payloads()
        .into_iter()
        .filter_map(
            |payload| match decode_crystal_payload(ServerPacketId::GameShopInfo, payload) {
                ServerPacket::GameShopInfo { item, stock_level } => Some((item, stock_level)),
                _ => None,
            },
        )
        .find(|(item, _)| item.g_index == game_shop_index)
}

fn game_shop_payment_allowed(
    can_buy_credit: bool,
    can_buy_gold: bool,
    price_type: GameShopPriceType,
) -> bool {
    match price_type {
        GameShopPriceType::Credit => can_buy_credit,
        GameShopPriceType::Gold => can_buy_gold,
    }
}

fn game_shop_stock_available(stock: i32, purchases: u64, quantity: u8) -> bool {
    if stock == 0 {
        return true;
    }
    let Ok(stock) = u64::try_from(stock) else {
        return false;
    };
    purchases
        .checked_add(u64::from(quantity))
        .is_some_and(|next| next <= stock)
}

pub(super) fn game_shop_stock_level(stock: i32, purchases: u64) -> i32 {
    if stock <= 0 {
        return stock.max(0);
    }
    i32::try_from((stock as u64).saturating_sub(purchases)).unwrap_or(0)
}

fn game_shop_attachment_states_json(
    template: &mir2_game_data::CrystalItemTemplate,
    item_key: &str,
    item_count: u32,
) -> Result<Vec<String>, GameShopPurchaseFailure> {
    let stack_size = u32::from(template.stack_size.max(1));
    let mut remaining = item_count;
    let mut attachments = Vec::new();
    while remaining > 0 {
        let quantity = remaining.min(stack_size);
        let grade = match template.grade {
            1 => ItemGrade::Common,
            2 => ItemGrade::Rare,
            3 => ItemGrade::Legendary,
            4 => ItemGrade::Mythical,
            5 => ItemGrade::Heroic,
            _ => ItemGrade::None,
        };
        let (heal_hp, heal_mp) = item_heal_values_for_key(item_key);
        let state = ItemState {
            key: item_key.to_string(),
            name: template.name.clone(),
            icon: template.image,
            slot: 0,
            unique_id: 0,
            container: ItemContainer::Bag1,
            quantity,
            description: template
                .tooltip
                .clone()
                .unwrap_or_else(|| "Crystal game shop purchase.".to_string()),
            durability_current: (template.durability > 0).then_some(template.durability),
            durability_max: (template.durability > 0).then_some(template.durability),
            weight: u16::from(template.weight),
            equip_slot: crystal_equipment_slot_for_item_key(item_key),
            grade,
            added_attack: 0,
            added_defence: 0,
            added_stats: Vec::new(),
            socketed: Vec::new(),
            user_item_metadata: None,
            cursed: false,
            socket_slots: template.slots,
            gem_count: 0,
            identified: None,
            soul_bound_id: None,
            sealed_expiry_time_binary_datetime: 0,
            sealed_next_time_binary_datetime: 0,
            rental_binding_flags: 0,
            rental_owner_name: String::new(),
            rental_expiry_binary_datetime: 0,
            rental_locked: false,
            attack: crystal_item_stat_value(template, CRYSTAL_STAT_MAX_DC),
            defence: crystal_item_stat_value(template, CRYSTAL_STAT_MAX_AC),
            heal_hp,
            heal_mp,
        };
        validate_exact_stage5_item_state(&state)
            .map_err(|_| GameShopPurchaseFailure::InvalidQuantity)?;
        attachments.push(
            serde_json::to_string(&state).map_err(|_| GameShopPurchaseFailure::InvalidQuantity)?,
        );
        remaining -= quantity;
    }
    Ok(attachments)
}

#[cfg(test)]
mod game_shop_validation_tests {
    use super::{
        authoritative_game_shop_product, game_shop_class_matches, game_shop_payment_allowed,
        game_shop_stock_available, game_shop_stock_level, GameShopPriceType,
        GameShopPurchaseExecution, GameShopPurchaseFailure, GameShopPurchaseOutcome,
        PlayerRuntimeResource, SimulationSession, Stage5SystemsResource,
    };
    use crate::config::{
        deliver_stage5_system_mail, AccountRecord, AccountStore, AccountStoreTransactionFault,
        AccountStoreTransactionScopeObservation, CharacterRecord, SimulationConfig,
        Stage5MailDelivery, Stage5MailMessage, Stage5MailTargetKind, Stage5SystemsState,
    };
    use crate::{NativeGameShopPurchaseRequest, NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use mir2_game_data::crystal_game_shop_packet_manifest;
    use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_store_path(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir()
            .join(format!(
                "mir2-game-shop-{label}-{}-{unique}",
                std::process::id()
            ))
            .join("accounts.json")
    }

    fn add_account(config: &SimulationConfig, account_id: &str, name: &str) {
        let character = CharacterRecord {
            index: 0,
            name: name.to_string(),
            level: 1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        };
        config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned")
            .accounts
            .insert(account_id.to_string(), AccountRecord::new(character));
    }

    fn started_session(
        config: SimulationConfig,
        account_id: &str,
        gold: u32,
        credit: u32,
    ) -> SimulationSession {
        let mut session = SimulationSession::new(config);
        session.handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let mut player = session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>();
        player.gold = gold;
        player.credit = credit;
        drop(player);
        session
    }

    fn loaded_session(config: SimulationConfig, account_id: &str) -> SimulationSession {
        let mut session = SimulationSession::new(config);
        session.handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session
    }

    fn durable_systems(config: &SimulationConfig, account_id: &str) -> Stage5SystemsState {
        let store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        store
            .accounts
            .get(account_id)
            .and_then(|account| account.saves.get(&0))
            .and_then(|save| save.stage5_systems_json.as_deref())
            .map(serde_json::from_str)
            .transpose()
            .expect("durable stage5 systems should decode")
            .unwrap_or_default()
    }

    fn finite_product(stock: i32, individual_stock: bool) -> mir2_protocol::GameShopItem {
        let (mut product, _) = authoritative_game_shop_product(1)
            .expect("checked-in game-shop product 1 should exist");
        product.stock = stock;
        product.i_stock = individual_stock;
        product.gold_price = 10;
        product.credit_price = 10;
        product.can_buy_gold = true;
        product.can_buy_credit = true;
        product
    }

    fn native_request(
        account_id: &str,
        gateway_session_id: &str,
        server_idempotency_key: &str,
        client_request_id: &str,
    ) -> NativeGameShopPurchaseRequest {
        NativeGameShopPurchaseRequest {
            protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
            server_idempotency_key: server_idempotency_key.to_string(),
            gateway_session_id: gateway_session_id.to_string(),
            account_id: account_id.to_string(),
            character_index: 0,
            client_request_id: client_request_id.to_string(),
            g_index: 31,
            quantity: 1,
            price_type: 1,
        }
    }

    fn native_key(seed: u8) -> String {
        URL_SAFE_NO_PAD.encode([seed; 32])
    }

    fn native_key_u32(seed: u32) -> String {
        let mut bytes = [0_u8; 32];
        bytes[..4].copy_from_slice(&seed.to_le_bytes());
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn native_ledger_at_capacity(
        account_id: &str,
        player_name: &str,
    ) -> (
        Stage5MailMessage,
        NativeGameShopPurchaseRequest,
        GameShopPurchaseOutcome,
    ) {
        let outcome = GameShopPurchaseOutcome::success(31, 1, 1, None, 77);
        let entries = (0..super::NATIVE_GAME_SHOP_LEDGER_MAX_ENTRIES)
            .map(|index| super::NativeGameShopLedgerEntry {
                server_idempotency_key: native_key_u32(index as u32),
                gateway_session_id: format!("gateway-session-{index}"),
                client_request_id: format!("gs-{index:016x}"),
                g_index: 31,
                quantity: 1,
                price_type: 1,
                outcome: outcome.clone(),
            })
            .collect::<Vec<_>>();
        let ledger = super::NativeGameShopDurableLedger {
            protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
            account_id: account_id.to_string(),
            character_index: 0,
            entries,
        };
        let mail = Stage5MailMessage {
            id: 1,
            delivery_nonce: super::NATIVE_GAME_SHOP_LEDGER_DELIVERY_NONCE.to_string(),
            from: super::NATIVE_GAME_SHOP_LEDGER_FROM.to_string(),
            to: player_name.to_string(),
            subject: super::NATIVE_GAME_SHOP_LEDGER_SUBJECT.to_string(),
            body: serde_json::to_string(&ledger).unwrap(),
            gold: 0,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: true,
            claimed: true,
            deleted: true,
        };
        let oldest_request = native_request(
            account_id,
            "gateway-session-0",
            &native_key_u32(0),
            "gs-0000000000000000",
        );
        (mail, oldest_request, outcome)
    }

    fn install_native_ledger_at_capacity(
        config: &SimulationConfig,
        account_id: &str,
        player_name: &str,
    ) -> (NativeGameShopPurchaseRequest, GameShopPurchaseOutcome) {
        let (mail, oldest_request, outcome) = native_ledger_at_capacity(account_id, player_name);
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let save = store
            .accounts
            .get_mut(account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("capacity fixture save should exist");
        save.stage5_systems_json = Some(
            serde_json::to_string(&Stage5SystemsState {
                mail: vec![mail],
                ..Stage5SystemsState::default()
            })
            .unwrap(),
        );
        (oldest_request, outcome)
    }

    fn assert_typed_failure(
        execution: &GameShopPurchaseExecution,
        failure: GameShopPurchaseFailure,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    ) {
        assert!(!execution.outcome.success);
        assert_eq!(execution.outcome.g_index, g_index);
        assert_eq!(execution.outcome.quantity, quantity);
        assert_eq!(execution.outcome.price_type, price_type);
        assert_eq!(execution.outcome.mail_id, None);
        assert_eq!(execution.outcome.failure, Some(failure));
        if failure != GameShopPurchaseFailure::StockUnavailable {
            assert_eq!(execution.outcome.new_stock_level, None);
        }
    }

    fn test_mail(id: u32) -> Stage5MailMessage {
        Stage5MailMessage {
            id,
            delivery_nonce: format!("game-shop-outcome-test-{id}"),
            from: "System".to_string(),
            to: "Archer".to_string(),
            subject: "Test".to_string(),
            body: "Test".to_string(),
            gold: 0,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        }
    }

    #[test]
    fn finite_stock_uses_purchase_quantity_and_handles_overflow() {
        assert!(game_shop_stock_available(0, u64::MAX, 99));
        assert!(game_shop_stock_available(10, 8, 2));
        assert!(!game_shop_stock_available(10, 9, 2));
        assert!(!game_shop_stock_available(10, u64::MAX, 1));
        assert!(!game_shop_stock_available(-1, 0, 1));
        assert_eq!(game_shop_stock_level(10, 3), 7);
        assert_eq!(game_shop_stock_level(10, 30), 0);
        assert_eq!(game_shop_stock_level(0, 30), 0);
    }

    #[test]
    fn game_shop_class_filter_is_explicit_and_unknown_values_fail_closed() {
        assert!(game_shop_class_matches("All", MirClass::Warrior));
        assert!(game_shop_class_matches("Wizard", MirClass::Wizard));
        assert!(!game_shop_class_matches("Wizard", MirClass::Warrior));
        assert!(!game_shop_class_matches("future-class", MirClass::Warrior));
    }

    #[test]
    fn game_shop_payment_flags_are_enforced_for_the_selected_currency() {
        assert!(game_shop_payment_allowed(
            true,
            false,
            GameShopPriceType::Credit
        ));
        assert!(!game_shop_payment_allowed(
            true,
            false,
            GameShopPriceType::Gold
        ));
        assert!(!game_shop_payment_allowed(
            false,
            true,
            GameShopPriceType::Credit
        ));
        assert!(game_shop_payment_allowed(
            false,
            true,
            GameShopPriceType::Gold
        ));
    }

    #[test]
    fn generated_game_shop_manifest_matches_all_authoritative_payloads() {
        let manifest = crystal_game_shop_packet_manifest();
        assert_eq!(manifest.total_items, 105);
        assert_eq!(manifest.items.len(), manifest.total_items);
        assert!(
            manifest.items.iter().all(|item| item.stock == 0),
            "the checked-in 105-row catalog must remain unlimited"
        );
        let mut credit_enabled = 0usize;
        let mut credit_disabled = 0usize;
        let mut gold_enabled = 0usize;
        let mut gold_disabled = 0usize;
        for template in manifest.items {
            let (item, stock_level) = authoritative_game_shop_product(template.game_shop_index)
                .expect("every generated shop row must decode from its authoritative payload");
            assert_eq!(item.item_index, template.item_index);
            assert_eq!(item.g_index, template.game_shop_index);
            assert_eq!(item.gold_price, template.gold_price);
            assert_eq!(item.credit_price, template.credit_price);
            assert_eq!(item.count, template.count);
            assert_eq!(item.class, template.class);
            assert_eq!(item.category, template.category);
            assert_eq!(item.stock, template.stock);
            assert_eq!(stock_level, template.stock_level);
            if item.can_buy_credit {
                credit_enabled += 1;
            } else {
                credit_disabled += 1;
            }
            if item.can_buy_gold {
                gold_enabled += 1;
            } else {
                gold_disabled += 1;
            }
        }
        assert!(credit_enabled > 0 && credit_disabled > 0);
        assert!(gold_enabled > 0 && gold_disabled > 0);
    }

    #[test]
    fn production_game_shop_path_selects_exact_transaction_scope_and_preserves_behavior() {
        use AccountStoreTransactionScopeObservation::{AccountOnly, WithGlobal};

        let cases: [(
            &str,
            i32,
            bool,
            AccountStoreTransactionScopeObservation,
            Option<i32>,
            Option<u64>,
            Option<u64>,
        ); 3] = [
            ("unlimited", 0, false, AccountOnly, None, None, None),
            (
                "finite-individual",
                3,
                true,
                AccountOnly,
                Some(2),
                None,
                Some(1),
            ),
            (
                "finite-global",
                3,
                false,
                WithGlobal,
                Some(2),
                Some(1),
                None,
            ),
        ];

        for (
            label,
            stock,
            individual_stock,
            expected_scope,
            expected_stock_level,
            expected_global_purchases,
            expected_individual_purchases,
        ) in cases
        {
            let config = SimulationConfig::default();
            let mut session = started_session(config.clone(), "demo", 1_000, 0);
            config.clear_account_store_transaction_scope_observations();

            let packets = session.game_shop_buy_product(
                finite_product(stock, individual_stock),
                1,
                GameShopPriceType::Gold,
            );

            assert!(
                packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 10 })),
                "{label} must emit the unchanged successful currency outcome"
            );
            let packet_stock_level = packets.iter().find_map(|packet| match packet {
                ServerPacket::GameShopStock {
                    g_index: 1,
                    stock_level,
                } => Some(*stock_level),
                _ => None,
            });
            assert_eq!(packet_stock_level, expected_stock_level, "{label}");
            assert_eq!(session.world_snapshot().gold, 990, "{label}");
            assert_eq!(
                config.account_store_transaction_scope_observations(),
                vec![expected_scope],
                "{label} must perform exactly one commit with the least privilege"
            );

            let global_purchases = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned")
                .game_shop_global_purchases
                .get(&1)
                .copied();
            let systems = durable_systems(&config, "demo");
            assert_eq!(global_purchases, expected_global_purchases, "{label}");
            assert_eq!(
                systems.game_shop_individual_purchases.get(&1).copied(),
                expected_individual_purchases,
                "{label}"
            );
            assert_eq!(systems.mail.len(), 1, "{label}");
            assert!(
                packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::Chat {
                        chat_type: mir2_protocol::ChatType::Hint,
                        ..
                    }
                )),
                "{label} must expose the successful mailbox-delivery outcome"
            );
        }
    }

    #[test]
    fn unlimited_catalog_purchase_does_not_create_stock_counters_or_patch() {
        let config = SimulationConfig::default();
        let mut session = started_session(config.clone(), "demo", 1_000_000, 10_000);
        let (product, _) = authoritative_game_shop_product(1)
            .expect("checked-in game-shop product 1 should exist");
        assert_eq!(product.stock, 0);
        let price_type = if product.can_buy_gold && product.gold_price > 0 {
            GameShopPriceType::Gold
        } else {
            GameShopPriceType::Credit
        };
        let packets = session.game_shop_buy_product(product, 1, price_type);
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::LoseGold { .. } | ServerPacket::LoseCredit { .. }
        )));
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GameShopStock { .. })));
        let store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        assert!(store.game_shop_global_purchases.is_empty());
        drop(store);
        assert!(durable_systems(&config, "demo")
            .game_shop_individual_purchases
            .is_empty());
    }

    #[test]
    fn individual_finite_stock_consumes_purchase_quantity_and_survives_reload() {
        let path = temp_store_path("individual-reload");
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        config
            .save_account_store()
            .expect("initial account store should persist");
        let mut session = started_session(config.clone(), "demo", 1_000, 0);
        let mut product = finite_product(3, true);
        product.count = 2;

        let packets = session.game_shop_buy_product(product.clone(), 2, GameShopPriceType::Gold);
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 20 })));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GameShopStock {
                g_index: 1,
                stock_level: 1
            }
        )));
        let systems = durable_systems(&config, "demo");
        assert_eq!(systems.game_shop_individual_purchases.get(&1), Some(&2));
        assert_eq!(systems.mail.len(), 1);
        assert!(systems.mail[0].body.contains("x 4"));

        let before = session.world_snapshot();
        let rejected = session.game_shop_buy_product(product, 2, GameShopPriceType::Gold);
        assert!(!rejected
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
        assert!(rejected.iter().any(|packet| matches!(
            packet,
            ServerPacket::GameShopStock {
                g_index: 1,
                stock_level: 1
            }
        )));
        assert_eq!(session.world_snapshot(), before);

        let reloaded = SimulationConfig::default().with_account_store_path(path.clone());
        let reloaded_systems = durable_systems(&reloaded, "demo");
        assert_eq!(
            reloaded_systems.game_shop_individual_purchases.get(&1),
            Some(&2)
        );
        assert_eq!(reloaded_systems.mail.len(), 1);
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn concurrent_same_character_sessions_do_not_oversell_individual_stock() {
        let config = SimulationConfig::default();
        let first = started_session(config.clone(), "demo", 1_000, 0);
        let second = started_session(config.clone(), "demo", 1_000, 0);
        let barrier = Arc::new(Barrier::new(2));
        let product = finite_product(1, true);

        let run = |mut session: SimulationSession,
                   product: mir2_protocol::GameShopItem,
                   barrier: Arc<Barrier>| {
            std::thread::spawn(move || {
                barrier.wait();
                session.game_shop_buy_product(product, 1, GameShopPriceType::Gold)
            })
        };
        let first = run(first, product.clone(), Arc::clone(&barrier));
        let second = run(second, product, barrier);
        let results = [
            first.join().expect("first purchase thread should finish"),
            second.join().expect("second purchase thread should finish"),
        ];
        assert_eq!(
            results
                .iter()
                .filter(|packets| packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })))
                .count(),
            1
        );
        let systems = durable_systems(&config, "demo");
        assert_eq!(systems.game_shop_individual_purchases.get(&1), Some(&1));
        assert_eq!(systems.mail.len(), 1);
    }

    #[test]
    fn concurrent_accounts_do_not_oversell_global_stock() {
        let config = SimulationConfig::default();
        add_account(&config, "second", "SecondBuyer");
        let first = started_session(config.clone(), "demo", 1_000, 0);
        let second = started_session(config.clone(), "second", 1_000, 0);
        let barrier = Arc::new(Barrier::new(2));
        let product = finite_product(1, false);

        let run = |mut session: SimulationSession,
                   product: mir2_protocol::GameShopItem,
                   barrier: Arc<Barrier>| {
            std::thread::spawn(move || {
                barrier.wait();
                session.game_shop_buy_product(product, 1, GameShopPriceType::Gold)
            })
        };
        let first = run(first, product.clone(), Arc::clone(&barrier));
        let second = run(second, product, barrier);
        let results = [
            first.join().expect("first purchase thread should finish"),
            second.join().expect("second purchase thread should finish"),
        ];
        assert_eq!(
            results
                .iter()
                .filter(|packets| packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })))
                .count(),
            1
        );
        let store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        assert_eq!(store.game_shop_global_purchases.get(&1), Some(&1));
        let mail_count = ["demo", "second"]
            .iter()
            .map(|account_id| {
                store.accounts[*account_id].saves[&0]
                    .stage5_systems_json
                    .as_deref()
                    .map(serde_json::from_str::<Stage5SystemsState>)
                    .transpose()
                    .expect("durable systems should decode")
                    .unwrap_or_default()
                    .mail
                    .len()
            })
            .sum::<usize>();
        assert_eq!(mail_count, 1);
    }

    #[test]
    fn finite_stock_persist_failure_and_counter_overflow_are_atomic() {
        let path = temp_store_path("atomic-failure");
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        config
            .save_account_store()
            .expect("initial account store should persist");
        let mut session = started_session(config.clone(), "demo", 1_000, 0);
        let before_world = session.world_snapshot();
        let before_file = std::fs::read(&path).expect("initial store should be readable");
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
        let packets =
            session.game_shop_buy_product(finite_product(2, false), 1, GameShopPriceType::Gold);
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
        assert_eq!(session.world_snapshot(), before_world);
        assert!(config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned")
            .game_shop_global_purchases
            .is_empty());
        assert_eq!(
            std::fs::read(&path).expect("store should remain readable"),
            before_file
        );

        config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned")
            .game_shop_global_purchases
            .insert(1, u64::MAX);
        let before_overflow = session.world_snapshot();
        let packets = session.game_shop_buy_product(
            finite_product(i32::MAX, false),
            1,
            GameShopPriceType::Gold,
        );
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
        assert_eq!(session.world_snapshot(), before_overflow);
        assert_eq!(
            config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned")
                .game_shop_global_purchases
                .get(&1),
            Some(&u64::MAX)
        );
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn typed_success_uses_the_committed_mail_id_for_gold_and_credit() {
        for (label, price_type) in [
            ("gold", GameShopPriceType::Gold),
            ("credit", GameShopPriceType::Credit),
        ] {
            let config = SimulationConfig::default();
            let mut session = started_session(config.clone(), "demo", 1_000, 1_000);
            let execution =
                session.game_shop_buy_product_with_outcome(finite_product(0, false), 1, price_type);
            let systems = durable_systems(&config, "demo");
            let committed_mail = systems
                .mail
                .last()
                .unwrap_or_else(|| panic!("{label} purchase should durably append mail"));

            assert!(execution.outcome.success, "{label} should succeed");
            assert_eq!(execution.outcome.g_index, 1);
            assert_eq!(execution.outcome.quantity, 1);
            assert_eq!(execution.outcome.price_type, price_type.raw());
            assert_eq!(execution.outcome.new_stock_level, None);
            assert_eq!(
                execution.outcome.mail_id,
                Some(u64::from(committed_mail.id))
            );
            assert_eq!(execution.outcome.failure, None);
            assert!(execution.packets.iter().any(|packet| match price_type {
                GameShopPriceType::Gold => matches!(packet, ServerPacket::LoseGold { gold: 10 }),
                GameShopPriceType::Credit => {
                    matches!(packet, ServerPacket::LoseCredit { credit: 10 })
                }
            }));
        }
    }

    #[test]
    fn typed_failures_cover_request_and_authoritative_product_validation() {
        let mut not_in_game = SimulationSession::new(SimulationConfig::default());
        let execution = not_in_game.game_shop_buy_packet_with_outcome(1, 1, 1);
        assert_typed_failure(&execution, GameShopPurchaseFailure::NotInGame, 1, 1, 1);

        let mut session = started_session(SimulationConfig::default(), "demo", 1_000, 1_000);
        let execution = session.game_shop_buy_packet_with_outcome(1, 1, 77);
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::InvalidPriceType,
            1,
            1,
            77,
        );
        let execution = session.game_shop_buy_packet_with_outcome(1, 0, 1);
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::InvalidQuantity,
            1,
            0,
            1,
        );
        let execution = session.game_shop_buy_packet_with_outcome(i32::MAX, 1, 1);
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::UnknownProduct,
            i32::MAX,
            1,
            1,
        );

        let mut product = finite_product(0, false);
        product.class = "Wizard".to_string();
        let execution =
            session.game_shop_buy_product_with_outcome(product, 1, GameShopPriceType::Gold);
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::ClassUnavailable,
            1,
            1,
            1,
        );

        let mut product = finite_product(0, false);
        product.can_buy_gold = false;
        let execution =
            session.game_shop_buy_product_with_outcome(product, 1, GameShopPriceType::Gold);
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::PaymentUnavailable,
            1,
            1,
            1,
        );

        let execution = session.game_shop_buy_product_with_outcome(
            finite_product(-1, false),
            1,
            GameShopPriceType::Gold,
        );
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::StockUnavailable,
            1,
            1,
            1,
        );
        assert_eq!(execution.outcome.new_stock_level, None);
    }

    #[test]
    fn typed_failures_cover_currency_mail_capacity_and_persist_failure() {
        let mut poor = started_session(SimulationConfig::default(), "demo", 0, 0);
        let execution = poor.game_shop_buy_product_with_outcome(
            finite_product(0, false),
            1,
            GameShopPriceType::Gold,
        );
        assert_typed_failure(
            &execution,
            GameShopPurchaseFailure::InsufficientCurrency,
            1,
            1,
            1,
        );

        let mut full = started_session(SimulationConfig::default(), "demo", 1_000, 0);
        full.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail = (1..=100).map(test_mail).collect();
        let execution = full.game_shop_buy_product_with_outcome(
            finite_product(0, false),
            1,
            GameShopPriceType::Gold,
        );
        assert_typed_failure(&execution, GameShopPurchaseFailure::MailFull, 1, 1, 1);

        let path = temp_store_path("typed-persist-failure");
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        config
            .save_account_store()
            .expect("initial account store should persist");
        let mut failed = started_session(config.clone(), "demo", 1_000, 0);
        let before_world = failed.world_snapshot();
        let before_file = std::fs::read(&path).expect("initial store should be readable");
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
        let execution = failed.game_shop_buy_product_with_outcome(
            finite_product(2, false),
            1,
            GameShopPriceType::Gold,
        );
        assert_typed_failure(&execution, GameShopPurchaseFailure::CommitFailed, 1, 1, 1);
        assert_eq!(failed.world_snapshot(), before_world);
        assert_eq!(
            std::fs::read(&path).expect("store should remain readable"),
            before_file
        );
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn typed_stock_outcomes_report_post_commit_and_current_levels() {
        let config = SimulationConfig::default();
        let mut session = started_session(config.clone(), "demo", 1_000, 0);
        let product = finite_product(2, true);

        let success =
            session.game_shop_buy_product_with_outcome(product.clone(), 2, GameShopPriceType::Gold);
        assert!(success.outcome.success);
        assert_eq!(success.outcome.new_stock_level, Some(0));
        assert!(success.outcome.mail_id.is_some());

        let unavailable =
            session.game_shop_buy_product_with_outcome(product, 1, GameShopPriceType::Gold);
        assert_typed_failure(
            &unavailable,
            GameShopPurchaseFailure::StockUnavailable,
            1,
            1,
            1,
        );
        assert_eq!(unavailable.outcome.new_stock_level, Some(0));
        assert!(matches!(
            unavailable.packets.last(),
            Some(ServerPacket::GameShopStock {
                g_index: 1,
                stock_level: 0
            })
        ));
        assert_eq!(durable_systems(&config, "demo").mail.len(), 1);
    }

    #[test]
    fn legacy_packet_wrapper_matches_typed_execution_packets() {
        let mut legacy = started_session(SimulationConfig::default(), "demo", 1_000_000, 0);
        let mut typed = started_session(SimulationConfig::default(), "demo", 1_000_000, 0);
        let game_shop_index = (1..=105)
            .find(|game_shop_index| {
                authoritative_game_shop_product(*game_shop_index).is_some_and(|(product, _)| {
                    product.can_buy_gold
                        && product.gold_price > 0
                        && game_shop_class_matches(&product.class, MirClass::Warrior)
                })
            })
            .expect("the authoritative catalog should contain a Gold product");
        let legacy_packets = legacy.game_shop_buy_packet(game_shop_index, 1, 1);
        let typed_execution = typed.game_shop_buy_packet_with_outcome(game_shop_index, 1, 1);
        assert_eq!(legacy_packets, typed_execution.packets);
        assert!(typed_execution.outcome.success);
    }

    #[test]
    fn typed_outcome_constructors_enforce_success_failure_shape_and_stable_codes() {
        let success = GameShopPurchaseOutcome::success(9, 2, 1, Some(3), 44);
        assert!(success.success);
        assert_eq!(success.mail_id, Some(44));
        assert_eq!(success.failure, None);

        let failures = [
            (GameShopPurchaseFailure::NotInGame, "notInGame"),
            (
                GameShopPurchaseFailure::InvalidPriceType,
                "invalidPriceType",
            ),
            (GameShopPurchaseFailure::InvalidQuantity, "invalidQuantity"),
            (GameShopPurchaseFailure::UnknownProduct, "unknownProduct"),
            (
                GameShopPurchaseFailure::ClassUnavailable,
                "classUnavailable",
            ),
            (
                GameShopPurchaseFailure::PaymentUnavailable,
                "paymentUnavailable",
            ),
            (
                GameShopPurchaseFailure::StockUnavailable,
                "stockUnavailable",
            ),
            (
                GameShopPurchaseFailure::InsufficientCurrency,
                "insufficientCurrency",
            ),
            (GameShopPurchaseFailure::MailFull, "mailFull"),
            (GameShopPurchaseFailure::CommitFailed, "commitFailed"),
        ];
        for (failure, code) in failures {
            let stock = (failure == GameShopPurchaseFailure::StockUnavailable).then_some(7);
            let outcome = GameShopPurchaseOutcome::failure(9, 2, 1, failure, stock);
            assert!(!outcome.success);
            assert_eq!(outcome.mail_id, None);
            assert_eq!(outcome.failure, Some(failure));
            assert_eq!(outcome.new_stock_level, stock);
            assert_eq!(failure.code(), code);

            let failure_json = serde_json::to_string(&failure)
                .expect("game-shop failure should serialize to a stable enum string");
            assert_eq!(failure_json, format!("\"{code}\""));
            assert_eq!(
                serde_json::from_str::<GameShopPurchaseFailure>(&failure_json)
                    .expect("game-shop failure should deserialize"),
                failure
            );

            let outcome_json = serde_json::to_string(&outcome)
                .expect("game-shop failure outcome should serialize");
            assert!(outcome_json.contains("\"gIndex\":9"));
            assert!(outcome_json.contains("\"priceType\":1"));
            assert!(outcome_json.contains("\"newStockLevel\""));
            assert!(outcome_json.contains("\"mailId\":null"));
            assert_eq!(
                serde_json::from_str::<GameShopPurchaseOutcome>(&outcome_json)
                    .expect("game-shop failure outcome should deserialize"),
                outcome
            );
        }

        let success_json =
            serde_json::to_string(&success).expect("game-shop success outcome should serialize");
        assert!(success_json.contains("\"gIndex\":9"));
        assert!(success_json.contains("\"newStockLevel\":3"));
        assert!(success_json.contains("\"mailId\":44"));
        assert_eq!(
            serde_json::from_str::<GameShopPurchaseOutcome>(&success_json)
                .expect("game-shop success outcome should deserialize"),
            success
        );

        for invalid_json in [
            r#"{"success":true,"gIndex":9,"quantity":2,"priceType":1,"newStockLevel":null,"mailId":null,"failure":null}"#,
            r#"{"success":true,"gIndex":9,"quantity":2,"priceType":1,"newStockLevel":null,"mailId":44,"failure":"commitFailed"}"#,
            r#"{"success":false,"gIndex":9,"quantity":2,"priceType":1,"newStockLevel":null,"mailId":null,"failure":null}"#,
            r#"{"success":false,"gIndex":9,"quantity":2,"priceType":1,"newStockLevel":null,"mailId":44,"failure":"commitFailed"}"#,
            r#"{"success":false,"gIndex":9,"quantity":2,"priceType":1,"newStockLevel":7,"mailId":null,"failure":"mailFull"}"#,
        ] {
            assert!(
                serde_json::from_str::<GameShopPurchaseOutcome>(invalid_json).is_err(),
                "invalid outcome shape must be rejected: {invalid_json}"
            );
        }
    }

    #[test]
    fn native_game_shop_duplicate_key_returns_original_outcome_without_second_mutation() {
        let config = SimulationConfig::default();
        let account_id = "native-idempotent-success";
        add_account(&config, account_id, "IdempotentBuyer");
        let mut session = started_session(config.clone(), account_id, 1_000_000, 0);
        let request = native_request(
            account_id,
            "gateway-session-a",
            &native_key(1),
            "gs-0000000000000001",
        );

        let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let first = session
            .game_shop_buy_packet_idempotent(request.clone())
            .expect("first native purchase should commit");
        assert!(first.outcome.success);
        assert!(!first.packets.is_empty());
        let after_first_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        assert!(after_first_gold < before_gold);

        let duplicate = session
            .game_shop_buy_packet_idempotent(request.clone())
            .expect("duplicate must return the durable outcome");
        assert_eq!(duplicate.outcome, first.outcome);
        assert!(duplicate.packets.is_empty());
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            after_first_gold
        );
        let systems = durable_systems(&config, account_id);
        assert_eq!(systems.mail.iter().filter(|mail| !mail.deleted).count(), 1);
        assert_eq!(
            systems
                .mail
                .iter()
                .filter(|mail| super::is_native_game_shop_ledger_mail(mail))
                .count(),
            1
        );

        let mut reloaded = started_session(config.clone(), account_id, after_first_gold, 0);
        let after_reload = reloaded
            .game_shop_buy_packet_idempotent(request)
            .expect("durable duplicate must survive runtime reconstruction");
        assert_eq!(after_reload.outcome, first.outcome);
        assert!(after_reload.packets.is_empty());
        assert_eq!(
            durable_systems(&config, account_id)
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count(),
            1
        );
    }

    #[test]
    fn native_game_shop_key_binding_and_new_connection_scope_are_exact() {
        let config = SimulationConfig::default();
        let account_id = "native-idempotent-binding";
        add_account(&config, account_id, "BindingBuyer");
        let mut session = started_session(config.clone(), account_id, 1_000_000, 0);
        let first_request = native_request(
            account_id,
            "gateway-session-a",
            &native_key(1),
            "gs-0000000000000001",
        );
        let first = session
            .game_shop_buy_packet_idempotent(first_request.clone())
            .expect("first request should commit");
        let gold_after_first = session.app.world().resource::<PlayerRuntimeResource>().gold;

        deliver_stage5_system_mail(
            &config,
            Stage5MailDelivery {
                target_kind: Stage5MailTargetKind::Account,
                target_id: account_id.to_string(),
                from: "MergeProbe".to_string(),
                subject: "External delivery between sessions".to_string(),
                body: "The durable ledger must survive mailbox merge.".to_string(),
                gold: 0,
                items: Vec::new(),
            },
        )
        .expect("external mailbox delivery should commit");

        let mut conflicting = first_request.clone();
        conflicting.quantity = 2;
        let conflicting = session
            .game_shop_buy_packet_idempotent(conflicting)
            .expect("idempotency binding conflict should be a typed ambiguous failure");
        assert_eq!(
            conflicting.outcome.failure,
            Some(GameShopPurchaseFailure::CommitFailed)
        );
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            gold_after_first
        );

        let second_request = native_request(
            account_id,
            "gateway-session-b",
            &native_key(2),
            "gs-0000000000000001",
        );
        let second = session
            .game_shop_buy_packet_idempotent(second_request.clone())
            .expect("same client correlation id on a new trusted scope is a new purchase");
        assert!(second.outcome.success);
        assert_ne!(second.outcome.mail_id, first.outcome.mail_id);
        assert!(session.app.world().resource::<PlayerRuntimeResource>().gold < gold_after_first);
        let gold_after_second = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let systems_after_second = durable_systems(&config, account_id);
        assert_eq!(
            systems_after_second
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count(),
            3,
            "the two purchase mails and externally merged mail must all survive"
        );

        let delayed_first = session
            .game_shop_buy_packet_idempotent(first_request)
            .expect("session A duplicate delayed until after session B must still replay");
        assert_eq!(delayed_first.outcome, first.outcome);
        assert!(delayed_first.packets.is_empty());
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            gold_after_second,
            "delayed session A replay must not debit a third time"
        );
        let systems_after_delayed_replay = durable_systems(&config, account_id);
        assert_eq!(
            systems_after_delayed_replay
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count(),
            3,
            "delayed session A replay must not create another purchase mail"
        );
        assert_eq!(
            systems_after_delayed_replay
                .mail
                .iter()
                .filter(|mail| super::is_native_game_shop_ledger_mail(mail))
                .count(),
            1,
            "mail merge and session rollover must preserve one character ledger"
        );
    }

    #[test]
    fn native_game_shop_business_failure_is_durable_and_not_re_evaluated() {
        let config = SimulationConfig::default();
        let account_id = "native-idempotent-failure";
        add_account(&config, account_id, "FailureBuyer");
        let mut session = started_session(config.clone(), account_id, 0, 0);
        let request = native_request(
            account_id,
            "gateway-session-failure",
            &native_key(1),
            "gs-0000000000000001",
        );
        let first = session
            .game_shop_buy_packet_idempotent(request.clone())
            .expect("business failure should be durably classified");
        assert_eq!(
            first.outcome.failure,
            Some(GameShopPurchaseFailure::InsufficientCurrency)
        );
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 1_000_000;
        let duplicate = session
            .game_shop_buy_packet_idempotent(request)
            .expect("duplicate failure should return the original outcome");
        assert_eq!(duplicate.outcome, first.outcome);
        assert!(duplicate.packets.is_empty());
        assert_eq!(
            durable_systems(&config, account_id)
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count(),
            0
        );
    }

    #[test]
    fn native_game_shop_stale_session_mail_merge_preserves_all_replay_keys() {
        let config = SimulationConfig::default();
        let account_id = "native-idempotent-stale-merge";
        add_account(&config, account_id, "StaleMergeBuyer");
        let mut session_a = started_session(config.clone(), account_id, 1_000_000, 0);
        let request_a = native_request(
            account_id,
            "gateway-session-a",
            &native_key(1),
            "gs-0000000000000001",
        );
        let outcome_a = session_a
            .game_shop_buy_packet_idempotent(request_a.clone())
            .expect("session A purchase should commit")
            .outcome;

        let mut session_b = loaded_session(config.clone(), account_id);
        let request_b = native_request(
            account_id,
            "gateway-session-b",
            &native_key(2),
            "gs-0000000000000001",
        );
        let outcome_b = session_b
            .game_shop_buy_packet_idempotent(request_b.clone())
            .expect("session B purchase should commit over A ledger")
            .outcome;
        let gold_after_b = session_b
            .app
            .world()
            .resource::<PlayerRuntimeResource>()
            .gold;

        assert!(
            session_a.refresh_active_external_mail(),
            "stale session A must import B mail and ledger entry"
        );
        let refreshed_a = session_a.world_snapshot().stage5_systems;
        assert_eq!(
            super::native_game_shop_ledger_outcome(&refreshed_a, &request_a).unwrap(),
            Some(outcome_a.clone())
        );
        assert_eq!(
            super::native_game_shop_ledger_outcome(&refreshed_a, &request_b).unwrap(),
            Some(outcome_b.clone()),
            "mutable hidden ledger body must union B into stale A"
        );

        session_a.save_active_character();
        let mut reloaded = loaded_session(config.clone(), account_id);
        let replay_a = reloaded
            .game_shop_buy_packet_idempotent(request_a)
            .expect("reloaded session must replay A");
        let replay_b = reloaded
            .game_shop_buy_packet_idempotent(request_b)
            .expect("reloaded session must replay B");
        assert_eq!(replay_a.outcome, outcome_a);
        assert_eq!(replay_b.outcome, outcome_b);
        assert!(replay_a.packets.is_empty());
        assert!(replay_b.packets.is_empty());
        assert_eq!(
            reloaded
                .app
                .world()
                .resource::<PlayerRuntimeResource>()
                .gold,
            gold_after_b,
            "replaying both stale-merged keys must not debit again"
        );
        assert_eq!(
            durable_systems(&config, account_id)
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count(),
            2,
            "replaying both stale-merged keys must not create more mail"
        );
    }

    #[test]
    fn native_game_shop_ledger_merge_conflict_fails_closed() {
        let account_id = "native-idempotent-merge-conflict";
        let request = native_request(
            account_id,
            "gateway-session-conflict",
            &native_key(9),
            "gs-0000000000000009",
        );
        let mut local_systems = Stage5SystemsState::default();
        let original = GameShopPurchaseOutcome::success(31, 1, 1, None, 10);
        super::record_native_game_shop_ledger_outcome(
            &mut local_systems,
            "ConflictBuyer",
            &request,
            &original,
        )
        .unwrap();
        let mut external_systems = local_systems.clone();
        let external_mail = external_systems
            .mail
            .iter_mut()
            .find(|mail| super::is_native_game_shop_ledger_mail(mail))
            .unwrap();
        let mut conflicting_ledger = super::decode_native_game_shop_ledger(external_mail).unwrap();
        conflicting_ledger.entries[0].outcome =
            GameShopPurchaseOutcome::success(31, 1, 1, None, 11);
        external_mail.body = serde_json::to_string(&conflicting_ledger).unwrap();

        let original_local = local_systems.mail.clone();
        let error = super::super::save::merge_external_stage5_mail(
            &mut local_systems.mail,
            external_systems.mail,
        )
        .expect_err("same key with a different outcome must fail closed");
        assert!(error.contains("conflicting request or outcome"));
        assert_eq!(local_systems.mail, original_local);
    }

    #[test]
    fn native_game_shop_ledger_capacity_fails_closed_without_evicting_old_keys() {
        let account_id = "native-idempotent-capacity";
        let (mail, oldest_request, outcome) =
            native_ledger_at_capacity(account_id, "CapacityBuyer");
        let mut systems = Stage5SystemsState::default();
        systems.mail.push(mail);
        assert_eq!(
            super::native_game_shop_ledger_outcome(&systems, &oldest_request).unwrap(),
            Some(outcome.clone())
        );

        let overflow_request = native_request(
            account_id,
            "gateway-session-overflow",
            &native_key_u32(u32::MAX),
            "gs-ffffffffffffffff",
        );
        let error = super::record_native_game_shop_ledger_outcome(
            &mut systems,
            "CapacityBuyer",
            &overflow_request,
            &outcome,
        )
        .expect_err("ledger capacity must reject a new purchase instead of evicting history");
        assert!(error.contains("full for this character"));
        assert_eq!(
            super::native_game_shop_ledger_outcome(&systems, &oldest_request).unwrap(),
            Some(outcome),
            "the oldest key remains replay-safe after capacity rejection"
        );
    }

    #[test]
    fn native_game_shop_ledger_capacity_blocks_complete_purchase_transaction() {
        for (case, individual_stock, price_type) in [
            ("global-gold", false, GameShopPriceType::Gold),
            ("individual-credit", true, GameShopPriceType::Credit),
        ] {
            let account_id = format!("native-capacity-{case}");
            let player_name = format!("Capacity{case}");
            let config = SimulationConfig::default();
            add_account(&config, &account_id, &player_name);
            let (oldest_request, oldest_outcome) =
                install_native_ledger_at_capacity(&config, &account_id, &player_name);
            let mut session = started_session(config.clone(), &account_id, 1_000, 1_000);
            let product = finite_product(20, individual_stock);
            let mut overflow_request = native_request(
                &account_id,
                &format!("gateway-session-overflow-{case}"),
                &native_key_u32(u32::MAX),
                &format!("gs-overflow-{case}"),
            );
            overflow_request.g_index = product.g_index;
            overflow_request.price_type = price_type.raw();

            let before_world = session.world_snapshot();
            let before_store = serde_json::to_value(
                &*config
                    .account_store
                    .lock()
                    .expect("account store mutex should not be poisoned"),
            )
            .unwrap();
            let before_visible_mail = before_world
                .stage5_systems
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count();
            let before_individual_stock = before_world
                .stage5_systems
                .game_shop_individual_purchases
                .clone();
            let before_global_stock = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned")
                .game_shop_global_purchases
                .clone();

            let rejected = session.game_shop_buy_product_attempt(
                product,
                1,
                price_type,
                true,
                Some(overflow_request),
            );
            assert_typed_failure(
                &rejected,
                GameShopPurchaseFailure::CommitFailed,
                1,
                1,
                price_type.raw(),
            );
            assert!(
                rejected.packets.is_empty(),
                "ambiguous capacity failure must expose no ordinary mutation packets"
            );
            assert_eq!(session.world_snapshot(), before_world);
            let after_store = serde_json::to_value(
                &*config
                    .account_store
                    .lock()
                    .expect("account store mutex should not be poisoned"),
            )
            .unwrap();
            assert_eq!(after_store, before_store);
            assert_eq!(
                session
                    .world_snapshot()
                    .stage5_systems
                    .mail
                    .iter()
                    .filter(|mail| !mail.deleted)
                    .count(),
                before_visible_mail
            );
            assert_eq!(
                session
                    .world_snapshot()
                    .stage5_systems
                    .game_shop_individual_purchases,
                before_individual_stock
            );
            assert_eq!(
                config
                    .account_store
                    .lock()
                    .expect("account store mutex should not be poisoned")
                    .game_shop_global_purchases,
                before_global_stock
            );

            let replay = session
                .game_shop_buy_packet_idempotent(oldest_request)
                .expect("oldest key must remain replayable after entry 4,097 is rejected");
            assert_eq!(replay.outcome, oldest_outcome);
            assert!(replay.packets.is_empty());
            let after_replay = session.world_snapshot();
            assert_eq!(after_replay.gold, before_world.gold);
            assert_eq!(after_replay.credit, before_world.credit);
            assert_eq!(
                after_replay
                    .stage5_systems
                    .mail
                    .iter()
                    .filter(|mail| !mail.deleted)
                    .count(),
                before_visible_mail
            );
            assert_eq!(
                after_replay.stage5_systems.game_shop_individual_purchases,
                before_individual_stock
            );
            assert_eq!(
                config
                    .account_store
                    .lock()
                    .expect("account store mutex should not be poisoned")
                    .game_shop_global_purchases,
                before_global_stock
            );
            let after_replay_store = serde_json::to_value(
                &*config
                    .account_store
                    .lock()
                    .expect("account store mutex should not be poisoned"),
            )
            .unwrap();
            assert_eq!(after_replay_store, before_store);
        }
    }

    #[test]
    fn native_game_shop_hidden_ledger_is_not_player_mail_or_mutable() {
        let config = SimulationConfig::default();
        let account_id = "native-hidden-ledger";
        add_account(&config, account_id, "HiddenLedgerBuyer");
        let mut session = started_session(config.clone(), account_id, 1_000_000, 0);
        let request = native_request(
            account_id,
            "gateway-session-hidden",
            &native_key(7),
            "gs-hidden-ledger",
        );
        let purchase = session
            .game_shop_buy_packet_idempotent(request.clone())
            .expect("fixture purchase should commit");
        assert!(purchase.outcome.success);
        let hidden = session
            .world_snapshot()
            .stage5_systems
            .mail
            .into_iter()
            .find(super::is_native_game_shop_ledger_mail)
            .expect("purchase should create one hidden ledger");
        assert!(hidden.deleted && hidden.locked && hidden.claimed);
        let before = session.world_snapshot();

        for packets in [
            session.handle_packet(ClientPacket::ReadMail {
                mail_id: u64::from(hidden.id),
            }),
            session.handle_packet(ClientPacket::CollectParcel {
                mail_id: u64::from(hidden.id),
            }),
            session.handle_packet(ClientPacket::DeleteMail {
                mail_id: u64::from(hidden.id),
            }),
        ] {
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
            )));
            assert!(packets.iter().all(|packet| !matches!(
                packet,
                ServerPacket::ReceiveMail { mail }
                    if mail.iter().any(|mail| mail.mail_id == u64::from(hidden.id))
            )));
        }
        assert_eq!(session.world_snapshot(), before);
        let replay = session
            .game_shop_buy_packet_idempotent(request)
            .expect("player mail operations must not damage the ledger");
        assert_eq!(replay.outcome, purchase.outcome);
        assert!(replay.packets.is_empty());
    }

    #[test]
    fn legacy_store_and_character_systems_default_stock_counters_to_empty() {
        let mut store_json = serde_json::to_value(AccountStore::new(
            SimulationConfig::default().default_character,
        ))
        .expect("account store should encode");
        store_json
            .as_object_mut()
            .expect("account store should be an object")
            .remove("gameShopGlobalPurchases");
        let decoded: AccountStore =
            serde_json::from_value(store_json).expect("legacy account store should decode");
        assert!(decoded.game_shop_global_purchases.is_empty());

        let mut systems_json = serde_json::to_value(Stage5SystemsState::default())
            .expect("stage5 systems should encode");
        systems_json
            .as_object_mut()
            .expect("stage5 systems should be an object")
            .remove("gameShopIndividualPurchases");
        let decoded: Stage5SystemsState =
            serde_json::from_value(systems_json).expect("legacy systems should decode");
        assert!(decoded.game_shop_individual_purchases.is_empty());
    }
}

fn stage5_game_shop_request(
    args: &[String],
    price_type: GameShopPriceType,
) -> Result<(i32, u8, GameShopPriceType), GameShopPurchaseFailure> {
    let game_shop_index = args
        .first()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or(GameShopPurchaseFailure::UnknownProduct)?;
    let quantity = match args.get(1) {
        Some(value) => value
            .parse::<u8>()
            .map_err(|_| GameShopPurchaseFailure::InvalidQuantity)?,
        None => 1,
    };
    Ok((game_shop_index, quantity, price_type))
}

fn game_shop_purchase_details(
    world: &World,
    product: GameShopItem,
    quantity: u8,
    price_type: GameShopPriceType,
) -> Result<GameShopPurchaseDetails, GameShopPurchaseFailure> {
    if !(1..=CRYSTAL_GAME_SHOP_MAX_QUANTITY).contains(&quantity) {
        return Err(GameShopPurchaseFailure::InvalidQuantity);
    }

    if !is_in_world(world) {
        return Err(GameShopPurchaseFailure::NotInGame);
    }
    let session = world.resource::<SessionResource>();
    let character = session
        .selected_character
        .as_ref()
        .ok_or(GameShopPurchaseFailure::NotInGame)?;
    if !game_shop_class_matches(&product.class, character.class) {
        return Err(GameShopPurchaseFailure::ClassUnavailable);
    }
    if !game_shop_payment_allowed(product.can_buy_credit, product.can_buy_gold, price_type) {
        return Err(GameShopPurchaseFailure::PaymentUnavailable);
    }
    if product.stock < 0 {
        return Err(GameShopPurchaseFailure::StockUnavailable);
    }

    let template =
        crystal_item_by_index(product.item_index).ok_or(GameShopPurchaseFailure::UnknownProduct)?;
    let item_count = u32::from(product.count)
        .checked_mul(u32::from(quantity))
        .ok_or(GameShopPurchaseFailure::InvalidQuantity)?;
    let attachment_count = item_count.div_ceil(u32::from(template.stack_size.max(1)));
    if item_count == 0 || attachment_count > CRYSTAL_GAME_SHOP_MAX_ATTACHMENT_STACKS {
        return Err(GameShopPurchaseFailure::InvalidQuantity);
    }
    let item_key = crystal_item_key_for_template(&template);
    let attachment_states_json =
        game_shop_attachment_states_json(&template, &item_key, item_count)?;
    let unit_price = match price_type {
        GameShopPriceType::Credit => product.credit_price,
        GameShopPriceType::Gold => product.gold_price,
    };
    let total_price = unit_price
        .checked_mul(u32::from(quantity))
        .filter(|price| *price > 0)
        .ok_or(GameShopPurchaseFailure::InvalidPriceType)?;
    let player = world.resource::<PlayerRuntimeResource>();
    let can_afford = match price_type {
        GameShopPriceType::Credit => player.credit >= total_price,
        GameShopPriceType::Gold => player.gold >= total_price,
    };
    if !can_afford {
        return Err(GameShopPurchaseFailure::InsufficientCurrency);
    }
    let active_mail = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .iter()
        .filter(|mail| !mail.deleted)
        .count();
    if active_mail >= CRYSTAL_MAIL_CAPACITY {
        return Err(GameShopPurchaseFailure::MailFull);
    }

    Ok(GameShopPurchaseDetails {
        game_shop_index: product.g_index,
        stock: product.stock,
        individual_stock: product.i_stock,
        purchase_quantity: quantity,
        item_key,
        item_name: template.name,
        item_count,
        attachment_states_json,
        total_price,
        price_type,
    })
}

pub(super) fn stage5_item_name(key: &str) -> String {
    key.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn crystal_npc_asset_id(parts: &[&str]) -> Option<u8> {
    parts
        .get(1)
        .or_else(|| parts.first())
        .and_then(|value| value.parse::<u8>().ok())
}

pub(super) fn normalize_stage5_key(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub(super) fn push_unique_u8(values: &mut Vec<u8>, value: u8) {
    if !values.contains(&value) {
        values.push(value);
    }
}

pub(super) fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QaApplyNativeCharacterState {
    character: QaApplyNativeCharacterRecord,
    #[serde(alias = "map_file_name")]
    map_file_name: String,
    #[serde(default, alias = "map_title")]
    map_title: String,
    position: Point,
    direction: MirDirection,
    hp: i32,
    #[serde(default, alias = "max_hp")]
    max_hp: Option<i32>,
    mp: i32,
    #[serde(default, alias = "max_mp")]
    max_mp: Option<i32>,
    #[serde(default)]
    experience: Option<i64>,
    #[serde(default, alias = "max_experience")]
    max_experience: Option<i64>,
    #[serde(default)]
    gold: Option<u32>,
    #[serde(default)]
    credit: Option<u32>,
    #[serde(default, alias = "city_currencies")]
    city_currencies: Option<BTreeMap<String, u32>>,
    #[serde(default, alias = "inventory_items_json")]
    inventory_items_json: Vec<String>,
    #[serde(default, alias = "belt_items_json")]
    belt_items_json: Vec<String>,
    #[serde(default, alias = "storage_items_json")]
    storage_items_json: Vec<String>,
    #[serde(default, alias = "equipment_items_json")]
    equipment_items_json: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QaApplyNativeCharacterRecord {
    name: String,
    level: u16,
    class: MirClass,
    gender: MirGender,
}

impl SimulationSession {
    pub fn stage5_command(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        let packets = self.stage5_command_impl(action, args);
        self.finalize_packets(packets)
    }

    fn stage5_command_impl(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        if !self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .stage5_action_is_allowed(action)
        {
            return vec![system_message(
                "This feature is unavailable in the active content profile.",
            )];
        }
        match action {
            "group.create" => self.stage5_group_create(args),
            "group.loot" => self.stage5_group_loot(args),
            "group.leave" => self.stage5_group_leave(),
            "guild.create" => self.stage5_guild_create(args),
            "guild.rank" => self.stage5_guild_rank(args),
            "guild.requestWar" => stage5_guild_request_war_packet(self.app.world()),
            "guild.ally" | "guild.alliance" => self.stage5_guild_ally(args),
            "guild.unally" | "guild.endAlliance" => self.stage5_guild_unally(args),
            "guild.chat" => self.stage5_guild_chat(args),
            "social.friend" => self.stage5_social_friend(args),
            "social.unfriend" => self.stage5_social_unfriend(args),
            "social.block" => self.stage5_social_block(args),
            "social.unblock" => self.stage5_social_unblock(args),
            "mail.send" => self.stage5_mail_send(args),
            "mail.claim" => self.stage5_mail_claim(args),
            "mail.delete" => self.stage5_mail_delete(args),
            "trade.start" => self.stage5_trade_start(args),
            "trade.offerGold" => self.stage5_trade_offer_gold(args),
            "trade.offerItem" => self.stage5_trade_offer_item(args),
            "trade.accept" => self.stage5_trade_accept(),
            "trade.cancel" => self.stage5_trade_cancel(),
            "shop.buy" => self.stage5_shop_buy(args),
            "shop.buyCredit" => self.stage5_shop_buy_credit(args),
            "gameShop.buyCredit" => self.game_shop_buy_credit(args),
            "gameShop.buyGold" => self.game_shop_buy_gold(args),
            "auction.list" => self.stage5_auction_list(args),
            "auction.buy" => self.stage5_auction_buy(args),
            "auction.cancel" => self.stage5_auction_cancel(args),
            "conquest.start" => self.stage5_conquest_start(args),
            "conquest.owner" => self.stage5_conquest_owner(args),
            "conquest.end" => self.stage5_conquest_end(args),
            "event.spawn" => self.stage5_event_spawn(args),
            "hero.recruit" => self.stage5_hero_recruit(args),
            "hero.behaviour" => self.stage5_hero_behaviour(args),
            "mine" => self.stage5_mine(args),
            "craft" => self.stage5_craft(args),
            "item.addSocket" => self.stage5_item_add_socket(args),
            "item.seal" => self.stage5_item_seal(args),
            "qa.damageEquipment" => self.stage5_qa_damage_equipment(args),
            "qa.damagePlayer" => self.stage5_qa_damage_player(args),
            "qa.giveItem" => self.stage5_qa_give_item(args),
            "qa.applyNativeState" => self.stage5_qa_apply_native_state(args),
            "qa.openNpcDialog" => self.stage5_qa_open_npc_dialog(),
            "qa.openStorage" => self.stage5_qa_open_storage(),
            other => {
                let language = current_language(self.app.world());
                vec![system_message(&format_localized_text(
                    language,
                    "server.InvalidPacketReceived",
                    [other.to_string()],
                ))]
            }
        }
    }

    fn stage5_group_create(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let member = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Companion".to_string());
        let player_name = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.group.members = unique_strings([player_name, member]);
        Vec::new()
    }

    fn stage5_group_loot(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let mode = args.first().cloned().unwrap_or_else(|| "free".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .group
            .loot_mode = mode.clone();
        Vec::new()
    }

    fn stage5_group_leave(&mut self) -> Vec<ServerPacket> {
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .group = Default::default();
        Vec::new()
    }

    fn stage5_guild_create(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Bichon".to_string());
        let language = current_language(self.app.world());
        let player_name = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let guild = &mut stage5.stage5_systems.guild;
        guild.name = name.clone();
        guild.members = unique_strings([player_name]);
        guild.rank = "Guild Chief".to_string();
        push_unique(&mut guild.known_guilds, name.clone());
        guild.active_wars.clear();
        guild.active_war_ticks_remaining.clear();
        guild.allied_guilds.clear();
        guild.ally_count = 0;
        guild.alliance_broadcasts.clear();
        guild.war_broadcasts.clear();
        guild.permissions = vec![
            "changeRank".to_string(),
            "recruit".to_string(),
            "kick".to_string(),
            "storeItem".to_string(),
            "retrieveItem".to_string(),
            "alterAlliance".to_string(),
            "changeNotice".to_string(),
            "activateBuff".to_string(),
        ];
        vec![system_message(&format_localized_text(
            language,
            "server.SuccessfullyCreatedGuild",
            [name],
        ))]
    }

    fn stage5_guild_rank(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let rank = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Member".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .guild
            .rank = rank.clone();
        Vec::new()
    }

    fn stage5_guild_ally(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        self.stage5_guild_alliance_change(args, true)
    }

    fn stage5_guild_unally(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        self.stage5_guild_alliance_change(args, false)
    }

    fn stage5_guild_alliance_change(
        &mut self,
        args: Vec<String>,
        create: bool,
    ) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let target_name = args.first().map(|name| name.trim()).unwrap_or_default();
        let (guild_name, can_alter, canonical_target, already_allied, at_war) = {
            let resources = self.app.world().resource::<Stage5SystemsResource>();
            let guild = &resources.stage5_systems.guild;
            let guild_name = guild.name.trim().to_string();
            let canonical_target = stage5_guild_canonical_alliance_target(
                guild,
                &resources.stage5_systems.guild_territory.owner,
                target_name,
            );
            let already_allied = canonical_target
                .as_deref()
                .is_some_and(|target| stage5_guild_is_allied(guild, target));
            let at_war = canonical_target.as_deref().is_some_and(|target| {
                guild
                    .active_wars
                    .iter()
                    .any(|war| war.eq_ignore_ascii_case(target))
            });
            (
                guild_name,
                stage5_guild_can_alter_alliance(guild),
                canonical_target,
                already_allied,
                at_war,
            )
        };

        if guild_name.is_empty() {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotInGuild",
                "server.NotInGuild",
            ))];
        }
        if !can_alter {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NoCorrectGuildRank",
                "server.NoCorrectGuildRank",
            ))];
        }
        let Some(target_name) = canonical_target else {
            return vec![system_message(&format_localized_text(
                language,
                "server.GuildNotFound",
                [target_name.to_string()],
            ))];
        };
        if guild_name.eq_ignore_ascii_case(&target_name) {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.CannotWarOwnGuild",
                "server.CannotWarOwnGuild",
            ))];
        }
        if create && at_war {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.AlreadyAtWarWithGuild",
                "server.AlreadyAtWarWithGuild",
            ))];
        }
        if create && already_allied {
            return Vec::new();
        }
        if !create && !already_allied {
            return Vec::new();
        }

        let message = if create {
            format!("Alliance formed with {target_name}.")
        } else {
            format!("Alliance ended with {target_name}.")
        };
        {
            let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            let guild = &mut resources.stage5_systems.guild;
            if create {
                push_unique(&mut guild.allied_guilds, target_name);
            } else {
                guild
                    .allied_guilds
                    .retain(|ally| !ally.eq_ignore_ascii_case(&target_name));
            }
            guild.ally_count = u32::try_from(guild.allied_guilds.len()).unwrap_or(u32::MAX);
            guild.alliance_broadcasts.push(message.clone());
        }

        vec![ServerPacket::Chat {
            message,
            chat_type: ChatType::Guild,
        }]
    }

    fn stage5_guild_chat(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let message = args.join(" ");
        let text = if message.is_empty() {
            "Guild message".to_string()
        } else {
            message
        };
        let sender = stage5_player_name(self.app.world());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .guild
            .chat_log
            .push(format!("{sender}: {text}"));
        vec![ServerPacket::Chat {
            message: text,
            chat_type: ChatType::Guild,
        }]
    }

    fn stage5_social_friend(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Friend".to_string());
        let player_name = stage5_player_name(self.app.world());
        let result = {
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            stage5_social_add_friend_entry(
                &mut stage5.stage5_systems.social,
                &player_name,
                &name,
                false,
            )
        };
        self.stage5_social_add_result(result)
    }

    fn stage5_social_add_result(&self, result: Stage5SocialAddResult) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        match result {
            Stage5SocialAddResult::Added => Vec::new(),
            Stage5SocialAddResult::EmptyTarget => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.PlayerDoesNotExist",
                    "server.PlayerDoesNotExist",
                ))]
            }
            Stage5SocialAddResult::SelfTarget => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.CannotAddYourself",
                    "server.CannotAddYourself",
                ))]
            }
            Stage5SocialAddResult::AlreadyAdded => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.PlayerAlreadyAdded",
                    "server.PlayerAlreadyAdded",
                ))]
            }
        }
    }

    fn stage5_social_unfriend(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Friend".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .social
            .friends
            .retain(|friend| !friend.eq_ignore_ascii_case(&name));
        Vec::new()
    }

    fn stage5_social_block(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Blocked".to_string());
        let player_name = stage5_player_name(self.app.world());
        let result = {
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            stage5_social_add_friend_entry(
                &mut stage5.stage5_systems.social,
                &player_name,
                &name,
                true,
            )
        };
        self.stage5_social_add_result(result)
    }

    fn stage5_social_unblock(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Blocked".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .social
            .blocked
            .retain(|blocked| !blocked.eq_ignore_ascii_case(&name));
        Vec::new()
    }

    fn stage5_mail_send(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let to = args
            .first()
            .cloned()
            .unwrap_or_else(|| stage5_player_name(self.app.world()));
        let subject = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "Crystal Mail".to_string());
        let body = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "Message".to_string());
        let gold = args
            .get(3)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let from = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
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
            delivery_nonce: new_stage5_mail_delivery_nonce(),
            from,
            to,
            subject,
            body,
            gold,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
        Vec::new()
    }

    fn stage5_mail_claim(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["mail.claim".to_string()],
            ))];
        };
        let language = current_language(self.app.world());
        match stage5_claim_mail_authoritative(self.app.world_mut(), id) {
            Ok(_) | Err(Stage5MailClaimError::AlreadyClaimed) => Vec::new(),
            Err(Stage5MailClaimError::NotFound) => vec![system_message(
                &localized_text_or_fallback(language, "server.NotFound", "server.NotFound"),
            )],
            Err(Stage5MailClaimError::InvalidExactItemState) => {
                vec![system_message(&format_localized_text(
                    language,
                    "server.InvalidPacketReceived",
                    ["mail.claim".to_string()],
                ))]
            }
            Err(Stage5MailClaimError::Capacity) => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))]
            }
            Err(Stage5MailClaimError::BalanceOverflow) => {
                vec![system_message(&format_localized_text(
                    language,
                    "server.InvalidPacketReceived",
                    ["mail.claim".to_string()],
                ))]
            }
        }
    }

    fn stage5_mail_delete(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["mail.delete".to_string()],
            ))];
        };
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        if let Some(mail) = stage5
            .stage5_systems
            .mail
            .iter_mut()
            .find(|mail| mail.id == id)
        {
            mail.deleted = true;
        }
        Vec::new()
    }

    fn stage5_trade_start(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let partner = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Trader".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = Some(Stage5TradeState {
            partner: partner.clone(),
            offered_items: Vec::new(),
            offered_slots: BTreeMap::new(),
            offered_unique_ids: BTreeMap::new(),
            offered_gold: 0,
            offered_currency: CurrencyKind::Gold,
            accepted: false,
            locked: false,
            completed: false,
        });
        Vec::new()
    }

    fn stage5_trade_offer_gold(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let Some(amount) = parse_u32_arg(&args, 0) else {
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["trade.offerGold"],
            ))];
        };
        // Optional second arg selects the currency (gold by default), so the
        // legacy `trade.offerGold <amount>` form keeps offering gold unchanged.
        let currency = args
            .get(1)
            .map(|value| CurrencyKind::from_arg(value))
            .unwrap_or(CurrencyKind::Gold);
        let language = current_language(self.app.world());
        let player = self.app.world().resource::<PlayerRuntimeResource>();
        let sufficient = match currency.city_key() {
            None => player.gold >= amount,
            Some(key) => player.city_currency_balance(key) >= amount,
        };
        if !sufficient {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.LowGold",
                "server.LowGold",
            ))];
        }
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if trade.completed || trade.locked {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        }
        trade.offered_gold = amount;
        trade.offered_currency = currency;
        trade.accepted = false;
        trade.locked = false;
        Vec::new()
    }

    fn stage5_trade_offer_item(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let language = current_language(self.app.world());
        let resources = self.app.world_mut().resource_mut::<InventoryResource>();
        let Some(item) = resources
            .inventory_items
            .iter()
            .find(|item| item.key == key)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if !stage5_trade_item_can_enter(item) {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "client.CantTrade",
                "client.CantTrade",
            ))];
        }
        drop(resources);
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if trade.completed || trade.locked {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        }
        push_unique(&mut trade.offered_items, key.clone());
        trade.accepted = false;
        trade.locked = false;
        Vec::new()
    }

    fn stage5_trade_accept(&mut self) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let Some((offered_gold, offered_currency, offered_items)) = self
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .as_ref()
            .map(|trade| {
                (
                    trade.offered_gold,
                    trade.offered_currency,
                    trade.offered_items.clone(),
                )
            })
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        {
            let inventory = self.app.world().resource::<InventoryResource>();
            for offered_item in offered_items {
                let Some(item) = inventory
                    .inventory_items
                    .iter()
                    .find(|item| item.key == offered_item)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.NotFound",
                        "server.NotFound",
                    ))];
                };
                if !stage5_trade_item_can_enter(item) {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "client.CantTrade",
                        "client.CantTrade",
                    ))];
                }
            }
        }
        let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
        match offered_currency.city_key() {
            None => {
                if player.gold < offered_gold {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.LowGold",
                        "server.LowGold",
                    ))];
                }
                player.gold -= offered_gold;
            }
            Some(key) => {
                if !player.spend_city_currency(key, offered_gold) {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.LowGold",
                        "server.LowGold",
                    ))];
                }
            }
        }
        drop(player);
        if let Some(trade) = self
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .as_mut()
        {
            trade.accepted = true;
            trade.completed = true;
        }
        vec![system_message(&localized_text_or_fallback(
            language,
            "server.TradeSuccessful",
            "server.TradeSuccessful",
        ))]
    }

    fn stage5_trade_cancel(&mut self) -> Vec<ServerPacket> {
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = None;
        Vec::new()
    }

    fn stage5_shop_buy(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let price = args
            .get(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(25);
        let language = current_language(self.app.world());
        {
            if self.app.world().resource::<PlayerRuntimeResource>().gold < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.LowGold",
                    "server.LowGold",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &key, 1) {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold -= price;
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &key,
            &stage5_item_name(&key),
            "Stage 5 shop purchase.",
            20,
            1,
            1,
        );
        vec![system_message(&format_localized_text(
            language,
            "server.BoughtItemForGold",
            [key, price.to_string()],
        ))]
    }

    fn stage5_shop_buy_credit(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let price = args
            .get(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        let language = current_language(self.app.world());
        let player_name = stage5_player_name(self.app.world());
        let mail_id;
        {
            let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            if player.credit < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouDontHaveEnoughCurrency",
                    "server.YouDontHaveEnoughCurrency",
                ))];
            }
            player.credit -= price;
            drop(player);
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            mail_id = stage5
                .stage5_systems
                .mail
                .iter()
                .map(|mail| mail.id)
                .max()
                .unwrap_or(0)
                + 1;
            stage5.stage5_systems.mail.push(Stage5MailMessage {
                id: mail_id,
                delivery_nonce: new_stage5_mail_delivery_nonce(),
                from: "Gameshop".to_string(),
                to: player_name,
                subject: "Game shop purchase".to_string(),
                body: format!("{key} was sent from the game shop."),
                gold: 0,
                items: vec![key.clone()],
                item_states_json: Vec::new(),
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });
        }
        vec![
            ServerPacket::LoseCredit { credit: price },
            system_message(&format_localized_text(
                language,
                "server.BoughtItemForCredit",
                [key, price.to_string()],
            )),
        ]
    }

    fn game_shop_buy_credit(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        match stage5_game_shop_request(&args, GameShopPriceType::Credit) {
            Ok((game_shop_index, quantity, price_type)) => {
                self.game_shop_buy(game_shop_index, quantity, price_type)
            }
            Err(error) => self.game_shop_rejection_packet(error),
        }
    }

    fn game_shop_buy_gold(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        match stage5_game_shop_request(&args, GameShopPriceType::Gold) {
            Ok((game_shop_index, quantity, price_type)) => {
                self.game_shop_buy(game_shop_index, quantity, price_type)
            }
            Err(error) => self.game_shop_rejection_packet(error),
        }
    }

    pub(super) fn game_shop_buy_packet(
        &mut self,
        game_shop_index: i32,
        quantity: u8,
        raw_price_type: i32,
    ) -> Vec<ServerPacket> {
        self.game_shop_buy_packet_with_outcome(game_shop_index, quantity, raw_price_type)
            .packets
    }

    pub(crate) fn game_shop_buy_packet_with_outcome(
        &mut self,
        game_shop_index: i32,
        quantity: u8,
        raw_price_type: i32,
    ) -> GameShopPurchaseExecution {
        match GameShopPriceType::try_from(raw_price_type) {
            Ok(price_type) => {
                self.game_shop_buy_with_outcome(game_shop_index, quantity, price_type)
            }
            Err(failure) => self.game_shop_rejection_execution(
                game_shop_index,
                quantity,
                raw_price_type,
                failure,
                None,
            ),
        }
    }

    pub(crate) fn game_shop_buy_packet_idempotent(
        &mut self,
        request: NativeGameShopPurchaseRequest,
    ) -> Result<GameShopPurchaseExecution, String> {
        validate_native_game_shop_purchase_request(&request)?;
        let identity = self
            .active_identity()
            .ok_or_else(|| "native GameShop purchase requires an active character".to_string())?;
        if identity.account_id != request.account_id
            || identity.character_index != request.character_index
        {
            return Err(
                "native GameShop purchase identity does not match the active character".to_string(),
            );
        }

        let price_type = match GameShopPriceType::try_from(request.price_type) {
            Ok(price_type) => price_type,
            Err(failure) => {
                return self.persist_native_game_shop_rejection(&request, failure, None)
            }
        };
        let Some((product, _)) = authoritative_game_shop_product(request.g_index) else {
            return self.persist_native_game_shop_rejection(
                &request,
                GameShopPurchaseFailure::UnknownProduct,
                None,
            );
        };
        Ok(self.game_shop_buy_product_attempt(
            product,
            request.quantity,
            price_type,
            true,
            Some(request),
        ))
    }

    fn persist_native_game_shop_rejection(
        &mut self,
        request: &NativeGameShopPurchaseRequest,
        failure: GameShopPurchaseFailure,
        new_stock_level: Option<i32>,
    ) -> Result<GameShopPurchaseExecution, String> {
        debug_assert_ne!(failure, GameShopPurchaseFailure::CommitFailed);
        let outcome = GameShopPurchaseOutcome::failure(
            request.g_index,
            request.quantity,
            request.price_type,
            failure,
            new_stock_level,
        );
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let active_save = snapshot_active_character_save(self.app.world())
            .ok_or_else(|| "native GameShop rejection has no active save".to_string())?;
        let expected_revision = active_save.revision;
        let active_character = active_save.character.clone();
        let account_id = request.account_id.clone();
        let request_for_commit = request.clone();
        let player_name = active_character.name.clone();
        let touched_accounts = vec![account_id.clone()];
        let committed =
            config.commit_account_store_transaction(&touched_accounts, move |store| {
                let persisted_save = store
                    .accounts
                    .get(&account_id)
                    .and_then(|account| account.saves.get(&active_character.index))
                    .cloned()
                    .ok_or_else(|| "native GameShop durable save is unavailable".to_string())?;
                if persisted_save.character.name != active_character.name {
                    return Err("native GameShop durable character identity changed".to_string());
                }
                let baseline_revision = persisted_save.revision;
                let mut staged_save = if baseline_revision == expected_revision {
                    let mut current = active_save;
                    merge_persisted_mail_into_character_save(&mut current, &persisted_save)?;
                    current
                } else {
                    persisted_save
                };
                let mut systems = staged_save
                    .stage5_systems_json
                    .as_deref()
                    .map(serde_json::from_str::<Stage5SystemsState>)
                    .transpose()
                    .map_err(|error| format!("failed to decode native GameShop ledger: {error}"))?
                    .unwrap_or_default();
                validate_stage5_systems_item_carriers(&systems)?;
                if let Some(existing) = record_native_game_shop_ledger_outcome(
                    &mut systems,
                    &player_name,
                    &request_for_commit,
                    &outcome,
                )? {
                    return Ok((
                        existing,
                        systems,
                        baseline_revision,
                        baseline_revision,
                        true,
                    ));
                }
                staged_save.stage5_systems_json =
                    Some(serde_json::to_string(&systems).map_err(|error| {
                        format!("failed to encode native GameShop ledger: {error}")
                    })?);
                validate_character_save_record(&staged_save)?;
                let committed_revision = baseline_revision
                    .checked_add(1)
                    .ok_or_else(|| "native GameShop character revision exhausted".to_string())?;
                staged_save.revision = committed_revision;
                store
                    .accounts
                    .get_mut(&account_id)
                    .expect("validated native GameShop account should exist")
                    .saves
                    .insert(active_character.index, staged_save);
                Ok((
                    outcome,
                    systems,
                    baseline_revision,
                    committed_revision,
                    false,
                ))
            })?;
        let (committed_outcome, committed_systems, baseline_revision, committed_revision, replayed) =
            committed;
        if !replayed && baseline_revision == expected_revision {
            self.app
                .world()
                .resource::<SessionResource>()
                .advance_active_save_revision(expected_revision, committed_revision);
        }
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail = committed_systems.mail;
        if replayed {
            return Ok(GameShopPurchaseExecution {
                packets: Vec::new(),
                outcome: committed_outcome,
            });
        }
        Ok(self.game_shop_rejection_execution(
            request.g_index,
            request.quantity,
            request.price_type,
            failure,
            new_stock_level,
        ))
    }

    fn game_shop_buy(
        &mut self,
        game_shop_index: i32,
        quantity: u8,
        price_type: GameShopPriceType,
    ) -> Vec<ServerPacket> {
        self.game_shop_buy_with_outcome(game_shop_index, quantity, price_type)
            .packets
    }

    fn game_shop_buy_with_outcome(
        &mut self,
        game_shop_index: i32,
        quantity: u8,
        price_type: GameShopPriceType,
    ) -> GameShopPurchaseExecution {
        let Some((product, _)) = authoritative_game_shop_product(game_shop_index) else {
            return self.game_shop_rejection_execution(
                game_shop_index,
                quantity,
                price_type.raw(),
                GameShopPurchaseFailure::UnknownProduct,
                None,
            );
        };
        self.game_shop_buy_product_with_outcome(product, quantity, price_type)
    }

    fn game_shop_buy_product(
        &mut self,
        product: GameShopItem,
        quantity: u8,
        price_type: GameShopPriceType,
    ) -> Vec<ServerPacket> {
        self.game_shop_buy_product_with_outcome(product, quantity, price_type)
            .packets
    }

    fn game_shop_buy_product_with_outcome(
        &mut self,
        product: GameShopItem,
        quantity: u8,
        price_type: GameShopPriceType,
    ) -> GameShopPurchaseExecution {
        self.game_shop_buy_product_attempt(product, quantity, price_type, true, None)
    }

    fn game_shop_buy_product_attempt(
        &mut self,
        product: GameShopItem,
        quantity: u8,
        price_type: GameShopPriceType,
        allow_global_refresh_retry: bool,
        idempotent_request: Option<NativeGameShopPurchaseRequest>,
    ) -> GameShopPurchaseExecution {
        let requested_g_index = product.g_index;
        let raw_price_type = price_type.raw();
        let retry_product = product.clone();
        let details =
            match game_shop_purchase_details(self.app.world(), product, quantity, price_type) {
                Ok(details) => details,
                Err(failure) => {
                    if let Some(request) = idempotent_request.as_ref() {
                        return self
                            .persist_native_game_shop_rejection(request, failure, None)
                            .unwrap_or_else(|error| {
                                eprintln!("native GameShop rejection ledger failed: {error}");
                                self.game_shop_rejection_execution(
                                    requested_g_index,
                                    quantity,
                                    raw_price_type,
                                    GameShopPurchaseFailure::CommitFailed,
                                    None,
                                )
                            });
                    }
                    return self.game_shop_rejection_execution(
                        requested_g_index,
                        quantity,
                        raw_price_type,
                        failure,
                        None,
                    );
                }
            };
        let language = current_language(self.app.world());
        let player_name = stage5_player_name(self.app.world());
        let price_type = details.price_type;
        let total_price = details.total_price;
        let game_shop_index = details.game_shop_index;
        let stock = details.stock;
        let individual_stock = details.individual_stock;
        let uses_global_stock = stock > 0 && !individual_stock;
        let purchase_quantity = details.purchase_quantity;
        let attachment_count = details.attachment_states_json.len();
        let pending_mail = Stage5MailMessage {
            id: 0,
            delivery_nonce: new_stage5_mail_delivery_nonce(),
            from: "Gameshop".to_string(),
            to: player_name.clone(),
            subject: "Game shop purchase".to_string(),
            body: format!(
                "{} x {} was sent from the game shop.",
                details.item_name, details.item_count
            ),
            gold: 0,
            // Keep one display key per exact attachment stack. Claim uses the
            // `item_states_json` path and preserves each stack's quantity.
            items: vec![details.item_key; attachment_count],
            item_states_json: details.attachment_states_json,
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        };

        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let session = self.app.world().resource::<SessionResource>();
        let account_id = match session
            .account_id
            .as_deref()
            .map(str::trim)
            .filter(|account_id| !account_id.is_empty())
        {
            Some(account_id) => account_id.to_string(),
            None => {
                return self.game_shop_rejection_execution(
                    game_shop_index,
                    quantity,
                    raw_price_type,
                    GameShopPurchaseFailure::NotInGame,
                    None,
                )
            }
        };
        let active_character = match session.selected_character.as_ref() {
            Some(character) => character.clone(),
            None => {
                return self.game_shop_rejection_execution(
                    game_shop_index,
                    quantity,
                    raw_price_type,
                    GameShopPurchaseFailure::NotInGame,
                    None,
                )
            }
        };
        let active_save = match snapshot_active_character_save(self.app.world()) {
            Some(save) => save,
            None => {
                return self.game_shop_rejection_execution(
                    game_shop_index,
                    quantity,
                    raw_price_type,
                    GameShopPurchaseFailure::NotInGame,
                    None,
                )
            }
        };
        let expected_revision = active_save.revision;
        let touched_accounts = vec![account_id.clone()];
        let idempotent_request_for_commit = idempotent_request.clone();

        let purchase_transaction = move |store: &mut AccountStore| {
            let persisted_save = {
                let account = store
                    .accounts
                    .get(&account_id)
                    .ok_or_else(|| "game-shop account changed before commit".to_string())?;
                let persisted_character = account
                    .characters
                    .iter()
                    .find(|character| character.index == active_character.index)
                    .ok_or_else(|| "game-shop character changed before commit".to_string())?;
                if persisted_character.name != active_character.name
                    || active_save.character.index != active_character.index
                    || active_save.character.name != active_character.name
                {
                    return Err("game-shop character identity mismatch".to_string());
                }
                account
                    .saves
                    .get(&active_character.index)
                    .cloned()
                    .ok_or_else(|| "game-shop save changed before commit".to_string())?
            };
            let baseline_revision = persisted_save.revision;
            let mut staged_save = if baseline_revision == expected_revision {
                let mut current = active_save;
                merge_persisted_mail_into_character_save(&mut current, &persisted_save)?;
                current
            } else {
                persisted_save
            };
            let mut systems = staged_save
                .stage5_systems_json
                .as_deref()
                .map(serde_json::from_str::<Stage5SystemsState>)
                .transpose()
                .map_err(|error| format!("failed to decode committed game-shop systems: {error}"))?
                .unwrap_or_default();
            validate_stage5_systems_item_carriers(&systems)?;

            if let Some(request) = idempotent_request_for_commit.as_ref() {
                if let Some(existing) = native_game_shop_ledger_outcome(&systems, request)? {
                    let inventory_items =
                        decode_state_vec::<ItemState>(&staged_save.inventory_items_json)
                            .ok_or_else(|| {
                                "failed to decode idempotent game-shop inventory".to_string()
                            })?;
                    return Ok((
                        None,
                        staged_save.gold,
                        staged_save.credit,
                        inventory_items,
                        systems,
                        existing.new_stock_level,
                        baseline_revision,
                        baseline_revision,
                        existing,
                        true,
                    ));
                }
            }

            let post_stock_level = if stock == 0 {
                None
            } else {
                let purchases = if individual_stock {
                    systems
                        .game_shop_individual_purchases
                        .get(&game_shop_index)
                        .copied()
                        .unwrap_or_default()
                } else {
                    store
                        .game_shop_global_purchases
                        .get(&game_shop_index)
                        .copied()
                        .unwrap_or_default()
                };
                if !game_shop_stock_available(stock, purchases, purchase_quantity) {
                    return Err(format!(
                        "{GAME_SHOP_STOCK_UNAVAILABLE_AT_COMMIT}{}",
                        game_shop_stock_level(stock, purchases)
                    ));
                }
                let next_purchases = purchases
                    .checked_add(u64::from(purchase_quantity))
                    .ok_or_else(|| "game-shop stock counter overflow".to_string())?;
                if individual_stock {
                    systems
                        .game_shop_individual_purchases
                        .insert(game_shop_index, next_purchases);
                } else {
                    store
                        .game_shop_global_purchases
                        .insert(game_shop_index, next_purchases);
                }
                Some(game_shop_stock_level(stock, next_purchases))
            };

            match price_type {
                GameShopPriceType::Credit => {
                    staged_save.credit = staged_save
                        .credit
                        .checked_sub(total_price)
                        .ok_or_else(|| "insufficient game-shop credit at commit".to_string())?;
                }
                GameShopPriceType::Gold => {
                    staged_save.gold = staged_save
                        .gold
                        .checked_sub(total_price)
                        .ok_or_else(|| "insufficient game-shop gold at commit".to_string())?;
                }
            }
            staged_save.stage5_systems_json = Some(
                serde_json::to_string(&systems)
                    .map_err(|error| format!("failed to encode game-shop systems: {error}"))?,
            );
            let mail = stage5_append_mail_to_save(&mut staged_save, &player_name, pending_mail)?;
            let inventory_items = decode_state_vec::<ItemState>(&staged_save.inventory_items_json)
                .ok_or_else(|| "failed to decode committed game-shop inventory".to_string())?;
            let mut committed_systems = staged_save
                .stage5_systems_json
                .as_deref()
                .map(serde_json::from_str::<Stage5SystemsState>)
                .transpose()
                .map_err(|error| format!("failed to decode committed game-shop mailbox: {error}"))?
                .unwrap_or_default();
            let committed_outcome = GameShopPurchaseOutcome::success(
                game_shop_index,
                quantity,
                raw_price_type,
                post_stock_level,
                u64::from(mail.id),
            );
            if let Some(request) = idempotent_request_for_commit.as_ref() {
                let existing = record_native_game_shop_ledger_outcome(
                    &mut committed_systems,
                    &player_name,
                    request,
                    &committed_outcome,
                )?;
                debug_assert!(existing.is_none());
                staged_save.stage5_systems_json =
                    Some(serde_json::to_string(&committed_systems).map_err(|error| {
                        format!("failed to encode committed native GameShop ledger: {error}")
                    })?);
            }
            validate_character_save_record(&staged_save)?;
            let committed_revision = baseline_revision
                .checked_add(1)
                .ok_or_else(|| "game-shop character revision exhausted".to_string())?;
            staged_save.revision = committed_revision;
            store
                .accounts
                .get_mut(&account_id)
                .expect("validated game-shop account should exist")
                .saves
                .insert(active_character.index, staged_save.clone());
            Ok((
                Some(mail),
                staged_save.gold,
                staged_save.credit,
                inventory_items,
                committed_systems,
                post_stock_level,
                baseline_revision,
                committed_revision,
                committed_outcome,
                false,
            ))
        };
        let committed = if uses_global_stock {
            config.commit_account_store_transaction_with_global(
                &touched_accounts,
                purchase_transaction,
            )
        } else {
            config.commit_account_store_transaction(&touched_accounts, purchase_transaction)
        };
        let (
            committed_mail,
            committed_gold,
            committed_credit,
            committed_inventory,
            committed_systems,
            post_stock_level,
            baseline_revision,
            committed_revision,
            committed_outcome,
            replayed,
        ) = match committed {
            Ok(committed) => committed,
            Err(error) => {
                if allow_global_refresh_retry
                    && stock > 0
                    && !individual_stock
                    && error.starts_with(STALE_POSTGRES_GAME_SHOP_GLOBAL_STOCK)
                {
                    match config.refresh_game_shop_global_stock() {
                        Ok(true) => {
                            return self.game_shop_buy_product_attempt(
                                retry_product,
                                quantity,
                                price_type,
                                false,
                                idempotent_request,
                            );
                        }
                        Ok(false) => {}
                        Err(refresh_error) => {
                            eprintln!(
                                "game-shop global-stock refresh failed after stale CAS: {refresh_error}"
                            );
                        }
                    }
                }
                eprintln!("game-shop transaction failed: {error}");
                let rejection = if error == "mailbox is full" {
                    GameShopPurchaseFailure::MailFull
                } else if error.starts_with("insufficient game-shop") {
                    GameShopPurchaseFailure::InsufficientCurrency
                } else if let Some(stock_level) =
                    error.strip_prefix(GAME_SHOP_STOCK_UNAVAILABLE_AT_COMMIT)
                {
                    if let Some(request) = idempotent_request.as_ref() {
                        return self
                            .persist_native_game_shop_rejection(
                                request,
                                GameShopPurchaseFailure::StockUnavailable,
                                stock_level.parse::<i32>().ok(),
                            )
                            .unwrap_or_else(|persist_error| {
                                eprintln!(
                                    "native GameShop stock rejection ledger failed: {persist_error}"
                                );
                                self.game_shop_rejection_execution(
                                    game_shop_index,
                                    quantity,
                                    raw_price_type,
                                    GameShopPurchaseFailure::CommitFailed,
                                    None,
                                )
                            });
                    }
                    return self.game_shop_rejection_execution(
                        game_shop_index,
                        quantity,
                        raw_price_type,
                        GameShopPurchaseFailure::StockUnavailable,
                        stock_level.parse::<i32>().ok(),
                    );
                } else {
                    GameShopPurchaseFailure::CommitFailed
                };
                if rejection == GameShopPurchaseFailure::CommitFailed
                    && idempotent_request.is_some()
                {
                    return self.native_game_shop_unknown_execution(
                        game_shop_index,
                        quantity,
                        raw_price_type,
                    );
                }
                if rejection != GameShopPurchaseFailure::CommitFailed {
                    if let Some(request) = idempotent_request.as_ref() {
                        return self
                            .persist_native_game_shop_rejection(request, rejection, None)
                            .unwrap_or_else(|persist_error| {
                                eprintln!(
                                    "native GameShop rejection ledger failed: {persist_error}"
                                );
                                self.game_shop_rejection_execution(
                                    game_shop_index,
                                    quantity,
                                    raw_price_type,
                                    GameShopPurchaseFailure::CommitFailed,
                                    None,
                                )
                            });
                    }
                }
                return self.game_shop_rejection_execution(
                    game_shop_index,
                    quantity,
                    raw_price_type,
                    rejection,
                    None,
                );
            }
        };

        if replayed {
            return GameShopPurchaseExecution {
                packets: Vec::new(),
                outcome: committed_outcome,
            };
        }

        if baseline_revision == expected_revision {
            self.app
                .world()
                .resource::<SessionResource>()
                .advance_active_save_revision(expected_revision, committed_revision);
        }

        let currency_packet = match price_type {
            GameShopPriceType::Credit => {
                self.app
                    .world_mut()
                    .resource_mut::<PlayerRuntimeResource>()
                    .credit = committed_credit;
                ServerPacket::LoseCredit {
                    credit: total_price,
                }
            }
            GameShopPriceType::Gold => {
                self.app
                    .world_mut()
                    .resource_mut::<PlayerRuntimeResource>()
                    .gold = committed_gold;
                ServerPacket::LoseGold { gold: total_price }
            }
        };
        self.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items = committed_inventory;
        {
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            stage5.stage5_systems.mail = committed_systems.mail;
            stage5.stage5_systems.game_shop_individual_purchases =
                committed_systems.game_shop_individual_purchases;
        }
        let mut packets = vec![currency_packet];
        if let Some(stock_level) = post_stock_level {
            packets.push(ServerPacket::GameShopStock {
                g_index: game_shop_index,
                stock_level,
            });
        }
        packets.push(ServerPacket::Chat {
            message: localized_text_or_fallback(
                language,
                "server.PurchasesSentMailbox",
                "server.PurchasesSentMailbox",
            ),
            chat_type: ChatType::Hint,
        });
        GameShopPurchaseExecution {
            packets,
            outcome: GameShopPurchaseOutcome::success(
                game_shop_index,
                quantity,
                raw_price_type,
                post_stock_level,
                u64::from(
                    committed_mail
                        .expect("new GameShop commit must create a delivery mail")
                        .id,
                ),
            ),
        }
    }

    fn game_shop_rejection_execution(
        &self,
        game_shop_index: i32,
        quantity: u8,
        raw_price_type: i32,
        failure: GameShopPurchaseFailure,
        new_stock_level: Option<i32>,
    ) -> GameShopPurchaseExecution {
        let mut packets = self.game_shop_rejection_packet(failure);
        if failure == GameShopPurchaseFailure::StockUnavailable {
            if let Some(stock_level) = new_stock_level {
                packets.push(ServerPacket::GameShopStock {
                    g_index: game_shop_index,
                    stock_level,
                });
            }
        }
        GameShopPurchaseExecution {
            packets,
            outcome: GameShopPurchaseOutcome::failure(
                game_shop_index,
                quantity,
                raw_price_type,
                failure,
                new_stock_level,
            ),
        }
    }

    fn native_game_shop_unknown_execution(
        &self,
        game_shop_index: i32,
        quantity: u8,
        raw_price_type: i32,
    ) -> GameShopPurchaseExecution {
        GameShopPurchaseExecution {
            packets: Vec::new(),
            outcome: GameShopPurchaseOutcome::failure(
                game_shop_index,
                quantity,
                raw_price_type,
                GameShopPurchaseFailure::CommitFailed,
                None,
            ),
        }
    }

    fn game_shop_rejection_packet(&self, error: GameShopPurchaseFailure) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let message = match error {
            GameShopPurchaseFailure::UnknownProduct
            | GameShopPurchaseFailure::ClassUnavailable
            | GameShopPurchaseFailure::PaymentUnavailable => localized_text_or_fallback(
                language,
                "server.YouBuyItemNotInShop",
                "server.YouBuyItemNotInShop",
            ),
            GameShopPurchaseFailure::StockUnavailable => localized_text_or_fallback(
                language,
                "server.YouBuyMoreThanAvailable",
                "server.YouBuyMoreThanAvailable",
            ),
            GameShopPurchaseFailure::InsufficientCurrency => localized_text_or_fallback(
                language,
                "server.YouDontHaveEnoughCurrency",
                "server.YouDontHaveEnoughCurrency",
            ),
            GameShopPurchaseFailure::MailFull => localized_text_or_fallback(
                language,
                "server.MailOverflowing",
                "server.MailOverflowing",
            ),
            GameShopPurchaseFailure::InvalidQuantity => return Vec::new(),
            GameShopPurchaseFailure::NotInGame
            | GameShopPurchaseFailure::InvalidPriceType
            | GameShopPurchaseFailure::CommitFailed => format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["gameShopBuy".to_string()],
            ),
        };
        vec![system_message(&message)]
    }

    fn stage5_auction_list(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let item_key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let price = args
            .get(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(50);
        // Optional third arg selects the listing currency (gold by default).
        let currency = args
            .get(2)
            .map(|value| CurrencyKind::from_arg(value))
            .unwrap_or(CurrencyKind::Gold);
        let seller = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let id = stage5
            .stage5_systems
            .auction
            .iter()
            .map(|listing| listing.id)
            .max()
            .unwrap_or(0)
            + 1;
        stage5.stage5_systems.auction.push(Stage5AuctionListing {
            id,
            seller,
            item_key,
            price,
            currency,
            sold: false,
            cancelled: false,
            expired: false,
        });
        Vec::new()
    }

    fn stage5_auction_buy(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["auction.buy".to_string()],
            ))];
        };
        let language = current_language(self.app.world());
        let (index, price, currency, item_key, seller) = {
            let stage5 = self.app.world().resource::<Stage5SystemsResource>();
            let Some(index) = stage5.stage5_systems.auction.iter().position(|listing| {
                listing.id == id && !listing.sold && !listing.cancelled && !listing.expired
            }) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let listing = &stage5.stage5_systems.auction[index];
            (
                index,
                listing.price,
                listing.currency,
                listing.item_key.clone(),
                listing.seller.clone(),
            )
        };
        {
            let player = self.app.world().resource::<PlayerRuntimeResource>();
            let affordable = match currency.city_key() {
                None => player.gold >= price,
                Some(key) => player.city_currency_balance(key) >= price,
            };
            if !affordable {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.LowGold",
                    "server.LowGold",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &item_key, 1) {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        // Deduct the buyer in the listing's currency.
        {
            let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            match currency.city_key() {
                None => player.gold -= price,
                Some(key) => {
                    player.spend_city_currency(key, price);
                }
            }
        }
        // Settle the seller. In this single-session model the only account loaded
        // is the buyer's, so proceeds can be paid out directly only when the
        // player is buying back their own listing. Listings owned by another
        // character (e.g. seeded marketplace entries) have no in-session wallet
        // to credit, so they are left unsettled (cross-account settlement would
        // need a global marketplace service, which is out of scope here).
        let buyer_name = stage5_player_name(self.app.world());
        if seller == buyer_name {
            let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            match currency.city_key() {
                None => player.gold = player.gold.saturating_add(price),
                Some(key) => {
                    player.gain_city_currency(key, price);
                }
            }
        }
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .auction[index]
            .sold = true;
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Stage 5 auction purchase.",
            21,
            1,
            1,
        );
        Vec::new()
    }

    fn stage5_auction_cancel(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["auction.cancel".to_string()],
            ))];
        };
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        if let Some(listing) = stage5
            .stage5_systems
            .auction
            .iter_mut()
            .find(|listing| listing.id == id && !listing.sold && !listing.expired)
        {
            listing.cancelled = true;
        }
        Vec::new()
    }

    fn stage5_conquest_start(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let castle = args.first().cloned().unwrap_or_else(|| "Sabuk".to_string());
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        push_unique(
            &mut resources.stage5_systems.conquest.active_wars,
            castle.clone(),
        );
        resources
            .stage5_systems
            .conquest
            .event_log
            .push(format!("War started: {castle}"));
        Vec::new()
    }

    fn stage5_conquest_owner(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let owner = args
            .first()
            .cloned()
            .or_else(|| {
                let resources = self.app.world().resource::<Stage5SystemsResource>();
                (!resources.stage5_systems.guild.name.is_empty())
                    .then(|| resources.stage5_systems.guild.name.clone())
            })
            .unwrap_or_else(|| "Independent".to_string());
        // Optional second arg: the conquest index this owner now controls, used
        // to gate conquest movements (Crystal `MyGuild.Conquest.Info.Index`).
        let conquest_index = args.get(1).and_then(|value| value.parse::<i32>().ok());
        {
            let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            resources.stage5_systems.conquest.castle_owner = owner.clone();
            resources
                .stage5_systems
                .conquest
                .event_log
                .push(format!("Castle owner: {owner}"));
        }
        if let Some(index) = conquest_index {
            self.app
                .world_mut()
                .resource_mut::<MapRuntimeResource>()
                .conquest_owners
                .insert(index, owner.clone());
        }
        Vec::new()
    }

    fn stage5_conquest_end(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let castle = args.first().cloned().unwrap_or_else(|| "Sabuk".to_string());
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        resources
            .stage5_systems
            .conquest
            .active_wars
            .retain(|war| !war.eq_ignore_ascii_case(&castle));
        resources
            .stage5_systems
            .conquest
            .event_log
            .push(format!("War ended: {castle}"));
        Vec::new()
    }

    fn stage5_event_spawn(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let monster_name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "BugBat".to_string());
        let count = args
            .get(1)
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(1);
        let language = current_language(self.app.world());
        let Some(template) = crystal_dynamic_monster_template(&monster_name) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let Some(player) = player_entity(self.app.world()) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let Some(origin) = entity_position(self.app.world(), player) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let (config, map_file_name) = {
            let world = self.app.world();
            (
                world.resource::<RuntimeConfigResource>().config.clone(),
                world
                    .resource::<MapRuntimeResource>()
                    .current_map
                    .file_name
                    .clone(),
            )
        };
        let mut spawn_points = crystal_spawn_candidates_on_map(&config, &map_file_name, &origin, 8)
            .into_iter()
            .filter(|point| point != &origin)
            .collect::<Vec<_>>();
        spawn_points.sort_by_key(|point| {
            let dx = (point.x - origin.x).abs();
            let dy = (point.y - origin.y).abs();
            (dx.max(dy), dx + dy, point.y, point.x)
        });

        let mut spawned = 0_u8;
        for index in 0..count {
            let position = spawn_points
                .get(usize::from(index))
                .cloned()
                .unwrap_or_else(|| Point {
                    x: origin.x + 1 + i32::from(index),
                    y: origin.y,
                });
            if spawn_runtime_monster(
                self.app.world_mut(),
                &template,
                position,
                MirDirection::Left,
                Some(player),
                None,
                Some(true),
                Some(WorldEntityDisposition::Hostile),
                0,
            )
            .is_some()
            {
                spawned += 1;
            }
        }
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .conquest
            .event_log
            .push(format!("Event spawned {spawned}x {monster_name}"));
        Vec::new()
    }

    fn stage5_hero_recruit(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args.first().cloned().unwrap_or_else(|| "Hero".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .hero = Some(Stage5HeroState {
            name: name.clone(),
            level: 1,
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            behaviour: 0,
            experience: 0,
            spawned: true,
            auto_pot: true,
            auto_hp_percent: 0,
            auto_mp_percent: 0,
            hp_item_index: 0,
            mp_item_index: 0,
        });
        let _ = spawn_stage5_hero(self.app.world_mut());
        Vec::new()
    }

    fn stage5_hero_behaviour(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let behaviour = args
            .first()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0);
        let language = current_language(self.app.world());
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let Some(hero) = resources.stage5_systems.hero.as_mut() else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        hero.behaviour = behaviour;
        Vec::new()
    }

    fn stage5_mine(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let ore = args
            .get(0)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        resources.stage5_systems.profession.ore =
            resources.stage5_systems.profession.ore.saturating_add(ore);
        resources.stage5_systems.profession.mining_level =
            resources.stage5_systems.profession.mining_level.max(1);
        Vec::new()
    }

    fn stage5_craft(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let item_key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "crafted-blade".to_string());
        let language = current_language(self.app.world());
        {
            let stage5 = self.app.world().resource::<Stage5SystemsResource>();
            if stage5.stage5_systems.profession.ore == 0 {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.CraftingAttemptFailed",
                    "server.CraftingAttemptFailed",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if free_bag_slots(&resources) == 0 {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        {
            let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            resources.stage5_systems.profession.ore -= 1;
            push_unique(
                &mut resources.stage5_systems.profession.crafted_items,
                item_key.clone(),
            );
        }
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Stage 5 crafted item.",
            22,
            1,
            1,
        );
        Vec::new()
    }

    fn stage5_item_add_socket(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let slot = args
            .first()
            .and_then(|value| equipment_slot_from_stage5_arg(value))
            .unwrap_or(EquipmentSlot::Weapon);
        let source_key = args.get(1).cloned();
        let language = current_language(self.app.world());
        let result = {
            let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
            let Some(item_index) = resources
                .equipment_items
                .iter()
                .position(|item| item.slot == slot)
            else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let item = &resources.equipment_items[item_index];
            let Some(unique_id) = equipment_slot_unique_id(item.slot) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let Some(max_slots) = crystal_socket_slot_limit_for_item_key(&item.key) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            if max_slots == 0 || item.socket_slots >= max_slots {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.ItemMaxSockets",
                    "server.ItemMaxSockets",
                ))];
            }
            let source_index = if let Some(source_key) = source_key.as_deref() {
                let Some(source_index) = resources
                    .inventory_items
                    .iter()
                    .position(|item| item.key == source_key || item.name == source_key)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.NotFound",
                        "server.NotFound",
                    ))];
                };
                let source_item = &resources.inventory_items[source_index];
                if !crystal_socket_source_valid_for_item(source_item, &item.key) {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.InvalidCombination",
                        "server.InvalidCombination",
                    ))];
                }
                Some(source_index)
            } else {
                None
            };
            resources.equipment_items[item_index].socket_slots = resources.equipment_items
                [item_index]
                .socket_slots
                .saturating_add(1);
            if let Some(source_index) = source_index {
                if resources.inventory_items[source_index].quantity > 1 {
                    resources.inventory_items[source_index].quantity -= 1;
                } else {
                    resources.inventory_items.remove(source_index);
                }
            }
            (
                unique_id,
                i32::from(resources.equipment_items[item_index].socket_slots),
            )
        };
        let (unique_id, slot_size) = result;

        vec![
            ServerPacket::ItemSlotSizeChanged {
                unique_id,
                slot_size,
            },
            system_message(&localized_text_or_fallback(
                language,
                "server.ItemSocketsIncreased",
                "server.ItemSocketsIncreased",
            )),
        ]
    }

    fn stage5_item_seal(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let slot = args
            .first()
            .and_then(|value| equipment_slot_from_stage5_arg(value))
            .unwrap_or(EquipmentSlot::Weapon);
        let fallback_minutes = args
            .get(1)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(60)
            .max(1);
        let source_key = args.get(2).cloned();
        let now_binary_datetime = current_binary_datetime();
        let language = current_language(self.app.world());
        let result = {
            let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
            let Some(item_index) = resources
                .equipment_items
                .iter()
                .position(|item| item.slot == slot)
            else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let item = &resources.equipment_items[item_index];
            if item.sealed_expiry_time_binary_datetime != 0
                && binary_datetime_ticks(item.sealed_expiry_time_binary_datetime)
                    > binary_datetime_ticks(now_binary_datetime)
            {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.ItemAlreadySealed",
                    "server.ItemAlreadySealed",
                ))];
            }
            if item.sealed_next_time_binary_datetime != 0
                && binary_datetime_ticks(item.sealed_next_time_binary_datetime)
                    > binary_datetime_ticks(now_binary_datetime)
            {
                let remaining_ticks = binary_datetime_ticks(item.sealed_next_time_binary_datetime)
                    - binary_datetime_ticks(now_binary_datetime);
                let remaining_seconds =
                    u64::try_from((remaining_ticks + 9_999_999) / 10_000_000).unwrap_or(1);
                return vec![system_message(&format_localized_text(
                    language,
                    "server.ItemCannotBeResealedFor",
                    [crystal_duration_label_from_seconds(
                        remaining_seconds.max(1),
                    )],
                ))];
            }
            let Some(unique_id) = equipment_slot_unique_id(item.slot) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let source_index_and_minutes = if let Some(source_key) = source_key.as_deref() {
                let Some(source_index) = resources
                    .inventory_items
                    .iter()
                    .position(|item| item.key == source_key || item.name == source_key)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.NotFound",
                        "server.NotFound",
                    ))];
                };
                let source_item = &resources.inventory_items[source_index];
                let Some(minutes) =
                    crystal_seal_minutes_for_source_item(source_item, fallback_minutes)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.InvalidCombination",
                        "server.InvalidCombination",
                    ))];
                };

                Some((source_index, minutes))
            } else {
                None
            };
            let minutes = source_index_and_minutes
                .map(|(_, minutes)| minutes)
                .unwrap_or(fallback_minutes);
            let expiry_date_binary_datetime = future_binary_datetime_minutes(minutes);
            let next_seal_binary_datetime = add_minutes_to_binary_datetime(
                expiry_date_binary_datetime,
                CRYSTAL_ITEM_SEAL_DELAY_MINUTES,
            );

            resources.equipment_items[item_index].sealed_expiry_time_binary_datetime =
                expiry_date_binary_datetime;
            resources.equipment_items[item_index].sealed_next_time_binary_datetime =
                next_seal_binary_datetime;
            if let Some((source_index, _)) = source_index_and_minutes {
                if resources.inventory_items[source_index].quantity > 1 {
                    resources.inventory_items[source_index].quantity -= 1;
                } else {
                    resources.inventory_items.remove(source_index);
                }
            }
            (unique_id, expiry_date_binary_datetime, minutes)
        };
        let (unique_id, expiry_date_binary_datetime, minutes) = result;

        vec![
            ServerPacket::ItemSealChanged {
                unique_id,
                expiry_date_binary_datetime,
            },
            system_message(&format_localized_text(
                language,
                "server.ItemSealedFor",
                [crystal_duration_label_from_seconds(
                    minutes.saturating_mul(60),
                )],
            )),
        ]
    }

    fn stage5_qa_damage_equipment(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let slot = args
            .first()
            .and_then(|value| equipment_slot_from_stage5_arg(value))
            .unwrap_or(EquipmentSlot::Weapon);
        let amount = args
            .get(1)
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(1)
            .max(1);
        let language = current_language(self.app.world());
        let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
        let Some(item) = resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == slot)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if !equipment_uses_durability(item) {
            return Vec::new();
        }
        let _ = damage_equipment_item(item, amount);
        let Some(unique_id) = equipment_slot_unique_id(item.slot) else {
            return Vec::new();
        };

        vec![ServerPacket::DuraChanged {
            unique_id,
            current_dura: item.durability_current,
        }]
    }

    fn stage5_qa_damage_player(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let amount = parse_u32_arg(&args, 0).unwrap_or(25).max(1) as i32;
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let next_vitals = {
            let mut entity = self.app.world_mut().entity_mut(player);
            let mut vitals = entity
                .get_mut::<PlayerVitals>()
                .expect("current player should have vitals");
            vitals.hp = (vitals.hp - amount).max(1);
            *vitals
        };
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .player_vitals = next_vitals;
        object_health_info_for_entity(self.app.world(), player, 0)
            .map(|info| vec![ServerPacket::ObjectHealth { info }])
            .unwrap_or_default()
    }

    fn stage5_qa_give_item(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let item_key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "blue-potion".to_string());
        let quantity = parse_u32_arg(&args, 1).unwrap_or(1).max(1);
        let language = current_language(self.app.world());
        let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
        let Some((container, slot)) =
            find_empty_inventory_item_slot(&resources.inventory_items, ItemContainer::Bag1)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.YouCannotCarryAnymore",
                "server.YouCannotCarryAnymore",
            ))];
        };
        let unique_id = allocate_item_unique_id(&resources, container, slot);
        let (heal_hp, heal_mp) = item_heal_values_for_key(&item_key);
        resources.inventory_items.push(ItemState {
            key: item_key.clone(),
            name: stage5_item_name(&item_key),
            icon: item_icon_for_key(&item_key),
            slot,
            unique_id,
            container,
            quantity,
            description: "Stage 5 QA seeded item.".to_string(),
            durability_current: None,
            durability_max: None,
            weight: 1,
            equip_slot: crystal_equipment_slot_for_item_key(&item_key),
            grade: ItemGrade::None,
            added_attack: 0,
            added_defence: 0,
            added_stats: Vec::new(),
            socketed: Vec::new(),
            user_item_metadata: None,
            cursed: false,
            socket_slots: 0,
            gem_count: 0,
            identified: None,
            soul_bound_id: None,
            sealed_expiry_time_binary_datetime: 0,
            sealed_next_time_binary_datetime: 0,
            rental_binding_flags: 0,
            rental_owner_name: String::new(),
            rental_expiry_binary_datetime: 0,
            rental_locked: false,
            attack: 0,
            defence: 0,
            heal_hp,
            heal_mp,
        });
        Vec::new()
    }

    fn stage5_qa_apply_native_state(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let Some(payload_json) = args.first() else {
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["qa.applyNativeState".to_string()],
            ))];
        };
        let Ok(state) = serde_json::from_str::<QaApplyNativeCharacterState>(payload_json) else {
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["qa.applyNativeState payload".to_string()],
            ))];
        };
        if decode_state_vec::<ItemState>(&state.inventory_items_json).is_none()
            || decode_state_vec::<ItemState>(&state.belt_items_json).is_none()
            || decode_state_vec::<ItemState>(&state.storage_items_json).is_none()
            || decode_state_vec::<EquipmentState>(&state.equipment_items_json).is_none()
        {
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["qa.applyNativeState item state".to_string()],
            ))];
        }

        let Some(selected_character) = self
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .clone()
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let Some(mut save) = snapshot_active_character_save(self.app.world()) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };

        let character = CharacterRecord {
            index: selected_character.index,
            name: if state.character.name.trim().is_empty() {
                selected_character.name
            } else {
                state.character.name
            },
            level: state.character.level.max(1),
            class: state.character.class,
            gender: state.character.gender,
        };
        let (base_max_hp, base_max_mp) = crystal_base_vitals(character.class, character.level);

        save.character = character.clone();
        save.map_file_name = state.map_file_name;
        save.map_title = state.map_title;
        save.position = state.position;
        save.direction = state.direction;
        save.max_hp = state.max_hp.unwrap_or(base_max_hp).max(1);
        save.hp = state.hp.clamp(1, save.max_hp);
        save.max_mp = state.max_mp.unwrap_or(base_max_mp).max(0);
        save.mp = state.mp.clamp(0, save.max_mp);
        if let Some(experience) = state.experience {
            save.experience = experience.max(0);
        }
        if let Some(max_experience) = state.max_experience {
            save.max_experience = max_experience.max(1);
        }
        if let Some(gold) = state.gold {
            save.gold = gold;
        }
        if let Some(credit) = state.credit {
            save.credit = credit;
        }
        if let Some(city_currencies) = state.city_currencies {
            save.city_currencies = city_currencies;
        }
        save.inventory_items_json = state.inventory_items_json;
        save.belt_items_json = state.belt_items_json;
        save.storage_items_json = state.storage_items_json;
        save.equipment_items_json = state.equipment_items_json;
        save.equipment_items_explicit_empty = true;

        if apply_character_save(self.app.world_mut(), &save).is_err() {
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["qa.applyNativeState item state".to_string()],
            ))];
        }
        if let Some(player) = player_entity(self.app.world()) {
            self.app.world_mut().entity_mut(player).insert((
                Position(save.position),
                Facing(save.direction),
                PlayerVitals {
                    hp: save.hp,
                    max_hp: save.max_hp,
                    mp: save.mp,
                    max_mp: save.max_mp,
                },
            ));
        }
        self.start_game(character.index)
    }

    fn stage5_qa_open_npc_dialog(&mut self) -> Vec<ServerPacket> {
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let Some(position) = entity_position(self.app.world(), player) else {
            return Vec::new();
        };
        let npc_position = Point {
            x: position.x,
            y: position.y.saturating_sub(1),
        };
        let world = self.app.world_mut();
        if let Some(npc) = entity_by_object_id(world, 21) {
            world.entity_mut(npc).insert((
                Npc,
                Position(npc_position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        } else {
            world.spawn((
                WorldObject,
                Npc,
                ObjectId(21),
                Position(npc_position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        }
        self.interact_impl(21)
    }

    fn stage5_qa_open_storage(&mut self) -> Vec<ServerPacket> {
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let Some(position) = entity_position(self.app.world(), player) else {
            return Vec::new();
        };
        let world = self.app.world_mut();
        if let Some(npc) = entity_by_object_id(world, 21) {
            world.entity_mut(npc).insert((
                Npc,
                Position(position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        } else {
            world.spawn((
                WorldObject,
                Npc,
                ObjectId(21),
                Position(position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        }
        world.resource_mut::<NpcStateResource>().active_npc_service = Some(ActiveNpcServiceState {
            script_key: "BichonProvince/BichonWall/Warehouse1".to_string(),
            label_key: "STORAGE".to_string(),
            npc_object_id: 21,
        });
        world.resource_mut::<InventoryResource>().storage_unlocked = false;
        crystal_npc_storage_open_packets(world)
    }
}
