use bevy_ecs::prelude::World;
use mir2_protocol::{ItemRentalInformation, ServerPacket, UserItemRentalInformation};

use super::crystal_compat::{
    CRYSTAL_BIND_DONT_DROP, CRYSTAL_BIND_DONT_SELL, CRYSTAL_BIND_DONT_STORE,
    CRYSTAL_BIND_DONT_TRADE, CRYSTAL_BIND_DONT_UPGRADE, CRYSTAL_BIND_UNABLE_TO_DISASSEMBLE,
    CRYSTAL_BIND_UNABLE_TO_RENT,
};
use super::inventory::{future_binary_datetime, inventory_container_and_slot_for_index};
use super::items::{
    crystal_item_has_bind_flag, item_has_rental_bind_flag, item_unique_id,
    user_item_from_item_state,
};
use super::resources::{
    ActiveItemRentalState, InventoryResource, ItemRentalRecordState, ItemRentalResource,
    PlayerRuntimeResource, SessionResource,
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
    item.unique_id = super::items::default_item_unique_id(container, slot);
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
            item.unique_id = super::items::default_item_unique_id(container, slot);
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
