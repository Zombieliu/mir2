use bevy_ecs::prelude::World;
use mir2_protocol::{ItemRentalInformation, ServerPacket, UserItemRentalInformation};

use crate::config::{
    CharacterSaveRecord, Stage5MailMessage, Stage5SystemsState, new_stage5_mail_delivery_nonce,
};

use super::crystal_compat::{
    CRYSTAL_BIND_DONT_DROP, CRYSTAL_BIND_DONT_SELL, CRYSTAL_BIND_DONT_STORE,
    CRYSTAL_BIND_DONT_TRADE, CRYSTAL_BIND_DONT_UPGRADE, CRYSTAL_BIND_UNABLE_TO_DISASSEMBLE,
    CRYSTAL_BIND_UNABLE_TO_RENT,
};
use super::equipment::{equipment_slot_unique_id, item_state_from_equipment_state};
use super::inventory::{
    add_minutes_to_binary_datetime, binary_datetime_ticks, current_binary_datetime,
    future_binary_datetime, inventory_container_and_slot_for_index,
    normalize_incoming_item_tree_unique_ids,
};
use super::items::{
    crystal_item_has_bind_flag, item_has_rental_bind_flag, item_unique_id,
    user_item_from_item_state,
};
use super::resources::{
    ActiveItemRentalState, InventoryResource, ItemRentalRecordState, ItemRentalResource,
    PlayerRuntimeResource, RuntimeConfigResource, SessionResource, Stage5SystemsResource,
};

const MAX_RENTED_ITEMS: usize = 3;

pub(super) const CRYSTAL_RENTAL_BINDING_FLAGS: i16 = CRYSTAL_BIND_DONT_DROP
    | CRYSTAL_BIND_DONT_STORE
    | CRYSTAL_BIND_DONT_SELL
    | CRYSTAL_BIND_DONT_TRADE
    | CRYSTAL_BIND_UNABLE_TO_RENT
    | CRYSTAL_BIND_DONT_UPGRADE
    | CRYSTAL_BIND_UNABLE_TO_DISASSEMBLE;

pub(super) fn get_rented_items_impl(world: &World) -> Vec<ServerPacket> {
    let rented_items = world
        .resource::<ItemRentalResource>()
        .rented_items
        .iter()
        .map(rental_information_packet)
        .collect();
    vec![ServerPacket::GetRentedItems { rented_items }]
}

pub(super) fn process_expired_rental_items(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if !super::resources::is_in_world(world) {
        return;
    }
    let now = current_binary_datetime();
    return_matching_rented_items(world, RentalReturnTrigger::Expired(now), packets);

    let changed_records = {
        let mut rental = world.resource_mut::<ItemRentalResource>();
        let now_ticks = binary_datetime_ticks(now);
        let original_len = rental.rented_items.len();
        rental.rented_items.retain(|record| {
            record.item_return_date_binary_datetime == 0
                || binary_datetime_ticks(record.item_return_date_binary_datetime) > now_ticks
        });
        rental.rented_items.len() != original_len
    };
    if changed_records {
        packets.extend(get_rented_items_impl(world));
    }
}

pub(super) fn return_rented_items_on_player_death(
    world: &mut World,
    packets: &mut Vec<ServerPacket>,
) {
    if !super::resources::is_in_world(world) {
        return;
    }
    return_matching_rented_items(world, RentalReturnTrigger::Death, packets);
}

#[derive(Debug, Clone, Copy)]
enum RentalReturnTrigger {
    Expired(i64),
    Death,
}

impl RentalReturnTrigger {
    fn should_return(self, expiry_binary_datetime: i64) -> bool {
        match self {
            Self::Death => true,
            Self::Expired(now) => {
                expiry_binary_datetime != 0
                    && binary_datetime_ticks(expiry_binary_datetime) <= binary_datetime_ticks(now)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ReturnedRentalItem {
    owner_name: String,
    item_name: String,
    item_key: String,
    item_state_json: String,
}

fn return_matching_rented_items(
    world: &mut World,
    trigger: RentalReturnTrigger,
    packets: &mut Vec<ServerPacket>,
) {
    let mut returned = Vec::new();
    let inventory_candidates = {
        let inventory = world.resource::<InventoryResource>();
        inventory
            .inventory_items
            .iter()
            .enumerate()
            .filter(|(_, item)| rented_item_matches_trigger(item, trigger))
            .map(|(index, item)| {
                (
                    index,
                    item_unique_id(item),
                    item.quantity.min(u32::from(u16::MAX)) as u16,
                    item.clone(),
                )
            })
            .collect::<Vec<_>>()
    };

    for (index, unique_id, count, item) in inventory_candidates.into_iter().rev() {
        let Some(returned_item) = returned_rental_item(item) else {
            continue;
        };
        if !queue_rental_return_mail(world, &returned_item) {
            continue;
        }
        let mut inventory = world.resource_mut::<InventoryResource>();
        if index < inventory.inventory_items.len()
            && item_unique_id(&inventory.inventory_items[index]) == unique_id
        {
            inventory.inventory_items.remove(index);
        } else if let Some(current_index) = inventory
            .inventory_items
            .iter()
            .position(|item| item_unique_id(item) == unique_id)
        {
            inventory.inventory_items.remove(current_index);
        }
        packets.push(ServerPacket::DeleteItem {
            unique_id,
            count: count.max(1),
        });
        returned.push(returned_item);
    }

    let equipment_candidates = {
        let inventory = world.resource::<InventoryResource>();
        inventory
            .equipment_items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                !item.rental_owner_name.is_empty()
                    && trigger.should_return(item.rental_expiry_binary_datetime)
            })
            .filter_map(|(index, item)| {
                let unique_id = equipment_slot_unique_id(item.slot)?;
                let item_state = item_state_from_equipment_state(
                    item.clone(),
                    crate::config::ItemContainer::Bag1,
                    0,
                );
                Some((index, unique_id, item_state))
            })
            .collect::<Vec<_>>()
    };

    for (index, unique_id, item) in equipment_candidates.into_iter().rev() {
        let Some(returned_item) = returned_rental_item(item) else {
            continue;
        };
        if !queue_rental_return_mail(world, &returned_item) {
            continue;
        }
        let mut inventory = world.resource_mut::<InventoryResource>();
        if index < inventory.equipment_items.len()
            && equipment_slot_unique_id(inventory.equipment_items[index].slot) == Some(unique_id)
        {
            inventory.equipment_items.remove(index);
        } else if let Some(current_index) = inventory
            .equipment_items
            .iter()
            .position(|item| equipment_slot_unique_id(item.slot) == Some(unique_id))
        {
            inventory.equipment_items.remove(current_index);
        }
        packets.push(ServerPacket::DeleteItem {
            unique_id,
            count: 1,
        });
        returned.push(returned_item);
    }

    if !returned.is_empty() {
        let still_has_rented_item = current_character_has_rented_item(world);
        world.resource_mut::<ItemRentalResource>().has_rented_item = still_has_rented_item;
    }
}

fn rented_item_matches_trigger(
    item: &super::items::ItemState,
    trigger: RentalReturnTrigger,
) -> bool {
    !item.rental_owner_name.is_empty() && trigger.should_return(item.rental_expiry_binary_datetime)
}

fn current_character_has_rented_item(world: &World) -> bool {
    let inventory = world.resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .any(|item| !item.rental_owner_name.is_empty())
        || inventory
            .equipment_items
            .iter()
            .any(|item| !item.rental_owner_name.is_empty())
}

fn returned_rental_item(mut item: super::items::ItemState) -> Option<ReturnedRentalItem> {
    if item.rental_owner_name.is_empty() {
        return None;
    }
    let owner_name = item.rental_owner_name.clone();
    item.rental_binding_flags = 0;
    item.rental_locked = true;
    let expiry_base = if item.rental_expiry_binary_datetime == 0 {
        current_binary_datetime()
    } else {
        item.rental_expiry_binary_datetime
    };
    item.rental_expiry_binary_datetime = add_minutes_to_binary_datetime(expiry_base, 24 * 60);
    let item_state_json = serde_json::to_string(&item).ok()?;
    Some(ReturnedRentalItem {
        owner_name,
        item_name: item.name.clone(),
        item_key: item.key.clone(),
        item_state_json,
    })
}

fn queue_rental_return_mail(world: &mut World, returned: &ReturnedRentalItem) -> bool {
    let borrower_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "Borrower".to_string());
    let current_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone());
    if current_name.as_deref() == Some(returned.owner_name.as_str()) {
        let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
        push_rental_return_mail(
            &mut stage5.stage5_systems,
            &returned.owner_name,
            &borrower_name,
            returned,
        );
        return true;
    }

    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let delivered = {
        let Ok(mut store) = config.account_store.lock() else {
            return false;
        };
        let mut delivered = false;
        'accounts: for account in store.accounts.values_mut() {
            for character in account.characters.clone() {
                if character.name != returned.owner_name {
                    continue;
                }
                let save = account
                    .saves
                    .entry(character.index)
                    .or_insert_with(|| CharacterSaveRecord::new(character.clone()));
                let mut systems = save
                    .stage5_systems_json
                    .as_deref()
                    .and_then(|state| serde_json::from_str::<Stage5SystemsState>(state).ok())
                    .unwrap_or_default();
                push_rental_return_mail(
                    &mut systems,
                    &returned.owner_name,
                    &borrower_name,
                    returned,
                );
                save.stage5_systems_json = serde_json::to_string(&systems).ok();
                delivered = true;
                break 'accounts;
            }
        }
        delivered
    };
    if delivered {
        let _ = config.save_account_store();
    }
    delivered
}

fn push_rental_return_mail(
    systems: &mut Stage5SystemsState,
    owner_name: &str,
    borrower_name: &str,
    returned: &ReturnedRentalItem,
) {
    let id = systems
        .mail
        .iter()
        .map(|mail| mail.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    systems.mail.push(Stage5MailMessage {
        id,
        delivery_nonce: new_stage5_mail_delivery_nonce(),
        from: borrower_name.to_string(),
        to: owner_name.to_string(),
        subject: "Rental item returned".to_string(),
        body: format!("{} was returned from item rental.", returned.item_name),
        gold: 0,
        items: vec![returned.item_key.clone()],
        item_states_json: vec![returned.item_state_json.clone()],
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    });
}

pub(super) fn item_rental_request_impl(
    world: &mut World,
    partner_name: Option<String>,
    renting: bool,
) -> Vec<ServerPacket> {
    let partner_name = partner_name.unwrap_or_else(|| {
        world
            .resource::<ItemRentalResource>()
            .default_partner_name
            .clone()
    });
    let mut rental = world.resource_mut::<ItemRentalResource>();
    rental.active = Some(ActiveItemRentalState {
        partner_name: partner_name.clone(),
        fee: 0,
        days: 1,
        deposited_item: None,
        deposited_from: None,
        gold_locked: false,
        item_locked: false,
    });
    vec![ServerPacket::ItemRentalRequest {
        name: partner_name,
        renting,
    }]
}

pub(super) fn item_rental_fee_impl(world: &mut World, amount: u32) -> Vec<ServerPacket> {
    if amount == 0 {
        return Vec::new();
    }
    let can_set_fee = world
        .resource::<ItemRentalResource>()
        .active
        .as_ref()
        .is_some_and(|active| !active.gold_locked);
    if !can_set_fee || world.resource::<PlayerRuntimeResource>().gold < amount {
        return Vec::new();
    }

    {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold -= amount;
    }
    {
        let mut rental = world.resource_mut::<ItemRentalResource>();
        let active = rental.active.as_mut().expect("active rental should exist");
        active.fee = active.fee.saturating_add(amount);
    }

    vec![
        ServerPacket::LoseGold { gold: amount },
        ServerPacket::ItemRentalFee { amount },
    ]
}

pub(super) fn item_rental_period_impl(world: &mut World, days: u32) -> Vec<ServerPacket> {
    if !(1..=30).contains(&days) {
        return Vec::new();
    }
    let mut rental = world.resource_mut::<ItemRentalResource>();
    let Some(active) = rental.active.as_mut() else {
        return Vec::new();
    };
    if active.item_locked {
        return Vec::new();
    }
    active.days = days;
    vec![ServerPacket::ItemRentalPeriod { days }]
}

pub(super) fn deposit_rental_item_impl(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let mut failure = vec![ServerPacket::DepositRentalItem {
        from,
        to,
        success: false,
    }];
    if to != 0 || from < 0 {
        return failure;
    }
    let from_slot = u8::try_from(from).ok();
    let Some(from_slot) = from_slot else {
        return failure;
    };
    if inventory_container_and_slot_for_index(from_slot).is_none() {
        return failure;
    }
    let active_allows_deposit = world
        .resource::<ItemRentalResource>()
        .active
        .as_ref()
        .is_some_and(|active| !active.item_locked && active.deposited_item.is_none());
    if !active_allows_deposit {
        return failure;
    }

    let item_index = {
        let resources = world.resource::<InventoryResource>();
        resources
            .inventory_items
            .iter()
            .position(|item| super::inventory::inventory_index_for_item(item) == Some(from_slot))
    };
    let Some(item_index) = item_index else {
        return failure;
    };
    let item = {
        let resources = world.resource::<InventoryResource>();
        resources.inventory_items[item_index].clone()
    };
    if item.rental_binding_flags != 0
        && (item_has_rental_bind_flag(&item, CRYSTAL_BIND_UNABLE_TO_RENT)
            || item_has_rental_bind_flag(&item, CRYSTAL_BIND_DONT_STORE))
    {
        return failure;
    }
    if crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_UNABLE_TO_RENT) {
        return failure;
    }

    let item = world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .remove(item_index);
    let mut rental = world.resource_mut::<ItemRentalResource>();
    let active = rental
        .active
        .as_mut()
        .expect("active rental should exist after preflight");
    active.deposited_from = Some(from);
    active.deposited_item = Some(item.clone());

    failure[0] = ServerPacket::DepositRentalItem {
        from,
        to,
        success: true,
    };
    failure.push(ServerPacket::UpdateRentalItem {
        loan_item: Some(user_item_from_item_state(&item)),
    });
    failure
}

pub(super) fn retrieve_rental_item_impl(
    world: &mut World,
    from: i32,
    to: i32,
) -> Vec<ServerPacket> {
    let mut packets = vec![ServerPacket::RetrieveRentalItem {
        from,
        to,
        success: false,
    }];
    if from != 0 || to < 0 {
        return packets;
    }
    let Some(to_slot) = u8::try_from(to).ok() else {
        return packets;
    };
    let Some((container, slot)) = inventory_container_and_slot_for_index(to_slot) else {
        return packets;
    };
    let destination_occupied = world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .any(|item| super::inventory::inventory_index_for_item(item) == Some(to_slot));
    if destination_occupied {
        return packets;
    }

    let item = {
        let mut rental = world.resource_mut::<ItemRentalResource>();
        let Some(active) = rental.active.as_mut() else {
            return packets;
        };
        active.deposited_item.take()
    };
    let Some(mut item) = item else {
        return packets;
    };
    item.container = container;
    item.slot = slot;
    normalize_incoming_item_tree_unique_ids(
        world.resource::<InventoryResource>(),
        &mut item,
        &[],
    );
    world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .push(item);
    if let Some(active) = world.resource_mut::<ItemRentalResource>().active.as_mut() {
        active.deposited_from = None;
    }
    packets[0] = ServerPacket::RetrieveRentalItem {
        from,
        to,
        success: true,
    };
    packets.push(ServerPacket::UpdateRentalItem { loan_item: None });
    packets
}

pub(super) fn cancel_item_rental_impl(world: &mut World) -> Vec<ServerPacket> {
    let active = world.resource_mut::<ItemRentalResource>().active.take();
    let Some(active) = active else {
        return Vec::new();
    };
    if let Some(mut item) = active.deposited_item {
        let destination = active
            .deposited_from
            .and_then(|slot| u8::try_from(slot).ok())
            .and_then(inventory_container_and_slot_for_index)
            .filter(|(container, slot)| {
                let index = match container {
                    crate::config::ItemContainer::Bag1 => *slot,
                    crate::config::ItemContainer::Bag2 => 40u8.saturating_add(*slot),
                    _ => 0,
                };
                !world
                    .resource::<InventoryResource>()
                    .inventory_items
                    .iter()
                    .any(|existing| {
                        super::inventory::inventory_index_for_item(existing) == Some(index)
                    })
            })
            .or_else(|| {
                (0..80)
                    .filter_map(|slot| u8::try_from(slot).ok())
                    .find_map(|index| {
                        let available = !world
                            .resource::<InventoryResource>()
                            .inventory_items
                            .iter()
                            .any(|existing| {
                                super::inventory::inventory_index_for_item(existing) == Some(index)
                            });
                        available
                            .then(|| inventory_container_and_slot_for_index(index))
                            .flatten()
                    })
            });
        if let Some((container, slot)) = destination {
            item.container = container;
            item.slot = slot;
            normalize_incoming_item_tree_unique_ids(
                world.resource::<InventoryResource>(),
                &mut item,
                &[],
            );
            world
                .resource_mut::<InventoryResource>()
                .inventory_items
                .push(item);
        }
    }
    if active.fee > 0 {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold = player.gold.saturating_add(active.fee);
    }
    vec![ServerPacket::CancelItemRental]
}

pub(super) fn item_rental_lock_fee_impl(world: &mut World) -> Vec<ServerPacket> {
    let mut rental = world.resource_mut::<ItemRentalResource>();
    let Some(active) = rental.active.as_mut() else {
        return Vec::new();
    };
    let success = active.fee > 0;
    if success {
        active.gold_locked = true;
    }
    let mut packets = vec![ServerPacket::ItemRentalLock {
        success,
        gold_locked: active.gold_locked,
        item_locked: active.item_locked,
    }];
    if active.gold_locked {
        packets.push(ServerPacket::ItemRentalPartnerLock {
            gold_locked: true,
            item_locked: active.item_locked,
        });
    }
    if active.gold_locked && active.item_locked {
        packets.push(ServerPacket::CanConfirmItemRental);
    }
    packets
}

pub(super) fn item_rental_lock_item_impl(world: &mut World) -> Vec<ServerPacket> {
    let mut rental = world.resource_mut::<ItemRentalResource>();
    let Some(active) = rental.active.as_mut() else {
        return Vec::new();
    };
    let success = active.deposited_item.is_some();
    if success {
        active.item_locked = true;
    }
    let mut packets = vec![ServerPacket::ItemRentalLock {
        success,
        gold_locked: active.gold_locked,
        item_locked: active.item_locked,
    }];
    if active.item_locked {
        packets.push(ServerPacket::ItemRentalPartnerLock {
            gold_locked: active.gold_locked,
            item_locked: true,
        });
    }
    if active.gold_locked && active.item_locked {
        packets.push(ServerPacket::CanConfirmItemRental);
    }
    packets
}

pub(super) fn confirm_item_rental_impl(world: &mut World) -> Vec<ServerPacket> {
    let can_confirm = {
        let rental = world.resource::<ItemRentalResource>();
        let Some(active) = rental.active.as_ref() else {
            return Vec::new();
        };
        active.gold_locked
            && active.item_locked
            && active.fee > 0
            && active.deposited_item.is_some()
            && rental.rented_items.len() < MAX_RENTED_ITEMS
            && !rental.has_rented_item
    };
    if !can_confirm {
        return cancel_item_rental_impl(world);
    }

    let active = world
        .resource_mut::<ItemRentalResource>()
        .active
        .take()
        .expect("active rental should exist");
    let mut item = active
        .deposited_item
        .expect("deposited item should exist after preflight");
    if crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_UNABLE_TO_RENT)
        || item_has_rental_bind_flag(&item, CRYSTAL_BIND_UNABLE_TO_RENT)
    {
        return vec![ServerPacket::CancelItemRental];
    }

    item.rental_binding_flags = CRYSTAL_RENTAL_BINDING_FLAGS;
    let expiry = future_binary_datetime(u64::from(active.days));
    let owner_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "Owner".to_string());
    item.rental_owner_name = owner_name.clone();
    item.rental_expiry_binary_datetime = expiry;
    item.rental_locked = false;
    let item_id = item_unique_id(&item);
    let item_name = item.name.clone();
    let mut loan_item = user_item_from_item_state(&item);
    loan_item.rental_information = Some(UserItemRentalInformation {
        owner_name: owner_name.clone(),
        binding_flags: CRYSTAL_RENTAL_BINDING_FLAGS,
        expiry_binary_datetime: expiry,
        rental_locked: false,
    });

    {
        let mut rental = world.resource_mut::<ItemRentalResource>();
        rental.rented_items.push(ItemRentalRecordState {
            item_id,
            item_name: item_name.clone(),
            renting_player_name: active.partner_name.clone(),
            item_return_date_binary_datetime: expiry,
        });
    }
    {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold = player.gold.saturating_add(active.fee);
    }

    vec![
        ServerPacket::UpdateRentalItem {
            loan_item: Some(loan_item),
        },
        ServerPacket::ConfirmItemRental,
        ServerPacket::GetRentedItems {
            rented_items: world
                .resource::<ItemRentalResource>()
                .rented_items
                .iter()
                .map(rental_information_packet)
                .collect(),
        },
    ]
}

fn rental_information_packet(record: &ItemRentalRecordState) -> ItemRentalInformation {
    ItemRentalInformation {
        item_id: record.item_id,
        item_name: record.item_name.clone(),
        renting_player_name: record.renting_player_name.clone(),
        item_return_date_binary_datetime: record.item_return_date_binary_datetime,
    }
}
