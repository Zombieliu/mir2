//! Account access is committed together with the exact owned consumable.
//! Gateway supplies verified identity and serializes offline/live ownership.
use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{
    billing_monthly_card_item_template, BILLING_MONTHLY_CARD_GAME_SHOP_INDEX,
    BILLING_MONTHLY_CARD_ITEM_INDEX,
};
use mir2_protocol::{GameShopItem, MirGridType, ServerPacket};

use super::inventory::{find_empty_inventory_item_slot, item_key_for_client_reference};
use super::items::{
    embedded_item_state_from_template, item_info_from_crystal_template, item_unique_id,
    validate_committed_item_state_carrier, ItemState,
};
use super::resources::{
    InventoryResource, PlayerRuntimeResource, RuntimeConfigResource, SessionResource,
};
use super::save::{
    decode_state_vec, encode_state_vec, merge_persisted_mail_into_character_save,
    snapshot_active_character_save, validate_character_save_record,
};
use super::session::SimulationSession;
use crate::config::{
    AccountRecord, CharacterSaveRecord, ItemContainer, SimulationConfig, Stage5SystemsState,
};
use crate::monthly_card::{
    monthly_card_item_identity, monthly_card_item_uid_is_reserved, valid_request_id,
    BillingMonthlyCardActivationReceipt, BillingMonthlyCardItem, BillingMonthlyCardPurchaseReceipt,
    MonthlyCardItemDelivery, MonthlyCardItemReceipt, MonthlyCardLedger, MonthlyCardStatus,
    MONTHLY_CARD_DURATION_MS,
};

const CARD_KEY: &str = "crystal-item-1000001";
const MAX_ITEM_RECEIPTS: usize = 4096;

#[derive(Resource, Clone)]
pub(super) struct TrustedCardClaimUnits(std::collections::BTreeSet<u64>);

pub(super) fn trusted_claim_units(world: &World) -> Result<TrustedCardClaimUnits, String> {
    if let Some(units) = world.get_resource::<TrustedCardClaimUnits>() {
        return Ok(units.clone());
    }
    let config = world
        .get_resource::<RuntimeConfigResource>()
        .ok_or("monthlyCardStoreUnavailable")?;
    let session = world
        .get_resource::<SessionResource>()
        .ok_or("monthlyCardCharacterMissing")?;
    let account_id = session
        .account_id
        .as_deref()
        .ok_or("monthlyCardAccountMissing")?;
    let character = session
        .selected_character
        .as_ref()
        .ok_or("monthlyCardCharacterMissing")?
        .index;
    let store = config
        .config
        .account_store
        .lock()
        .map_err(|_| "monthlyCardStoreUnavailable")?;
    let account = store
        .accounts
        .get(account_id)
        .ok_or("monthlyCardAccountMissing")?;
    let mut result = std::collections::BTreeSet::new();
    if let Some(ledger) = &account.monthly_card {
        ledger.validate()?;
        result.extend(
            ledger
                .item_receipts
                .values()
                .filter(|row| {
                    row.account_id == account_id
                        && row.character_index == character
                        && row.delivery == MonthlyCardItemDelivery::Mail
                        && row.redeemed_at_ms.is_none()
                })
                .map(|row| row.unique_id),
        );
    }
    Ok(TrustedCardClaimUnits(result))
}

pub(super) fn validate_claim_items(world: &World, items: &[ItemState]) -> Result<(), String> {
    if !items.iter().any(is_card) {
        return Ok(());
    }
    let units = trusted_claim_units(world)?;
    for (index, item) in items.iter().enumerate() {
        if !is_card(item) {
            continue;
        }
        validate_card_item(item)?;
        let uid = item_unique_id(item);
        if !units.0.contains(&uid)
            || super::inventory::inventory_unique_id_is_used(
                world.resource::<InventoryResource>(),
                uid,
            )
            || items.iter().enumerate().any(|(other_index, other)| {
                other_index != index
                    && super::inventory::item_list_unique_id_is_used(
                        std::slice::from_ref(other),
                        uid,
                    )
            })
        {
            return Err("monthlyCardItemIdentityCollision".into());
        }
    }
    Ok(())
}

pub(super) fn game_shop_product(config: &SimulationConfig) -> Option<(GameShopItem, i32)> {
    let price = config
        .billing_monthly_card_credit_price
        .filter(|price| *price > 0)?;
    Some((
        GameShopItem {
            item_index: BILLING_MONTHLY_CARD_ITEM_INDEX,
            g_index: BILLING_MONTHLY_CARD_GAME_SHOP_INDEX,
            info: item_info_from_crystal_template(billing_monthly_card_item_template()),
            gold_price: 0,
            credit_price: price,
            count: 1,
            class: "All".into(),
            category: "Scroll".into(),
            stock: 0,
            i_stock: false,
            deal: false,
            top_item: false,
            date_binary_datetime: 0,
            can_buy_credit: true,
            can_buy_gold: false,
        },
        0,
    ))
}

pub(super) fn is_card(item: &ItemState) -> bool {
    item.key == CARD_KEY
}
pub(super) fn is_card_key(key: &str) -> bool {
    key == CARD_KEY
}

pub(super) fn validate_card_item(item: &ItemState) -> Result<(), String> {
    validate_committed_item_state_carrier(item).map_err(|_| "monthlyCardItemInvalid")?;
    if !is_card(item)
        || item.quantity != 1
        || !monthly_card_item_uid_is_reserved(item_unique_id(item))
        || item.equip_slot.is_some()
        || !item.socketed.is_empty()
        || item.heal_hp != 0
        || item.heal_mp != 0
        || item.attack != 0
        || item.defence != 0
        || item.added_attack != 0
        || item.added_defence != 0
        || !item.added_stats.is_empty()
        || item.user_item_metadata.is_some()
        || item.durability_current.is_some()
        || item.durability_max.is_some()
        || item.socket_slots != 0
        || item.gem_count != 0
        || item.cursed
        || item.soul_bound_id.is_some()
        || item.sealed_expiry_time_binary_datetime != 0
        || item.sealed_next_time_binary_datetime != 0
        || item.rental_binding_flags != 0
        || !item.rental_owner_name.is_empty()
        || item.rental_expiry_binary_datetime != 0
        || item.rental_locked
    {
        return Err("monthlyCardItemInvalid".into());
    }
    Ok(())
}

fn decode_items(encoded: &[String]) -> Result<Vec<ItemState>, String> {
    decode_state_vec(encoded).ok_or_else(|| "monthlyCardInventoryInvalid".into())
}
fn systems(save: &CharacterSaveRecord) -> Result<Stage5SystemsState, String> {
    save.stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "monthlyCardInventoryInvalid".to_string())
        .map(|value| value.unwrap_or_default())
}
fn account_character<'a>(
    account: &'a AccountRecord,
    character_index: i32,
) -> Result<&'a CharacterSaveRecord, String> {
    let character = account
        .characters
        .iter()
        .find(|row| row.index == character_index)
        .ok_or("monthlyCardCharacterMissing")?;
    let save = account
        .saves
        .get(&character_index)
        .ok_or("monthlyCardCharacterMissing")?;
    if !same_character(&save.character, character) {
        return Err("monthlyCardCharacterIdentityChanged".into());
    }
    Ok(save)
}
fn same_character(
    left: &crate::config::CharacterRecord,
    right: &crate::config::CharacterRecord,
) -> bool {
    left.index == right.index
        && left.name == right.name
        && left.class == right.class
        && left.gender == right.gender
}

fn items_in_save(save: &CharacterSaveRecord) -> Result<Vec<ItemState>, String> {
    let mut result = Vec::new();
    for list in [
        &save.inventory_items_json,
        &save.belt_items_json,
        &save.storage_items_json,
        &save.hero_inventory_items_json,
        &save.hero_equipment_items_json,
    ] {
        result.extend(decode_items(list)?);
    }
    for mail in systems(save)?.mail {
        if !mail.claimed && !mail.deleted {
            result.extend(decode_items(&mail.item_states_json)?);
        }
    }
    Ok(result)
}

fn collect_saved_unique_ids(
    save: &CharacterSaveRecord,
    seen: &mut std::collections::BTreeSet<u64>,
) -> Result<(), String> {
    fn collect(value: &serde_json::Value, seen: &mut std::collections::BTreeSet<u64>) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    if matches!(
                        key.as_str(),
                        "unique_id" | "uniqueId" | "user_item_unique_id" | "userItemUniqueId"
                    ) {
                        if let Some(uid) = value.as_u64() {
                            seen.insert(uid);
                        }
                    }
                    collect(value, seen);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    collect(value, seen);
                }
            }
            _ => {}
        }
    }
    for list in [
        &save.inventory_items_json,
        &save.belt_items_json,
        &save.storage_items_json,
        &save.equipment_items_json,
        &save.hero_inventory_items_json,
        &save.hero_equipment_items_json,
    ] {
        for encoded in list {
            let value = serde_json::from_str(encoded).map_err(|_| "monthlyCardInventoryInvalid")?;
            collect(&value, seen);
        }
    }
    for item in items_in_save(save)? {
        seen.insert(item_unique_id(&item));
    }
    seen.extend(super::item_custody::reserved_ids(&systems(save)?)?);
    Ok(())
}

fn owned_receipt<'a>(
    ledger: &'a MonthlyCardLedger,
    account: &str,
    character: i32,
    uid: u64,
) -> Result<&'a MonthlyCardItemReceipt, String> {
    ledger.validate()?;
    let receipt = ledger
        .item_receipts
        .values()
        .find(|row| row.unique_id == uid)
        .ok_or("monthlyCardItemNotIssued")?;
    if receipt.account_id != account || receipt.character_index != character {
        return Err("monthlyCardItemNotOwned".into());
    }
    Ok(receipt)
}

/// The common issuance/debit operation for ordinary GameShop mail and select-screen bag delivery.
/// Caller is inside the account-store transaction and publishes save + ledger together.
#[allow(clippy::too_many_arguments)]
pub(super) fn issue_credit_purchase(
    account: &mut AccountRecord,
    save: &mut CharacterSaveRecord,
    account_id: &str,
    character: i32,
    request: &str,
    quantity: u16,
    price: u32,
    now_ms: u64,
    delivery: MonthlyCardItemDelivery,
) -> Result<(Vec<ItemState>, bool), String> {
    // Issuance is also used by a Gift transaction, where this account owns the
    // unit but another account pays. Stage the ledger until this owner's debit
    // is validated so the existing self-purchase helper remains all-or-nothing.
    let mut staged_account = account.clone();
    let (items, replayed) = issue_paid_items(
        &mut staged_account, save, account_id, character, request, quantity, price,
        now_ms, delivery,
    )?;
    if !replayed {
        let total = price.checked_mul(u32::from(quantity)).ok_or("monthlyCardPriceOverflow")?;
        let remaining = save.credit.checked_sub(total)
            .ok_or("insufficient game-shop credit at commit")?;
        account.monthly_card = staged_account.monthly_card;
        save.credit = remaining;
    }
    Ok((items, replayed))
}

/// Issue exact paid units to their owner inside an enclosing Source transaction.
/// The caller commits its payer debit and delivery together with this ledger;
/// this helper never spends or changes the recipient's character checkpoint.
pub(super) fn issue_paid_items(
    account: &mut AccountRecord,
    save: &CharacterSaveRecord,
    account_id: &str,
    character: i32,
    request: &str,
    quantity: u16,
    price: u32,
    now_ms: u64,
    delivery: MonthlyCardItemDelivery,
) -> Result<(Vec<ItemState>, bool), String> {
    if !valid_request_id(request) || !(1..=99).contains(&quantity) || price == 0 {
        return Err("monthlyCardPurchaseInvalid".into());
    }
    let ledger = account.monthly_card.clone().unwrap_or_default();
    ledger.validate()?;
    let existing: Vec<_> = ledger
        .item_receipts
        .values()
        .filter(|row| {
            row.account_id == account_id
                && row.character_index == character
                && row.request_id == request
        })
        .collect();
    if !existing.is_empty() {
        if existing.len() != usize::from(quantity)
            || existing
                .iter()
                .any(|row| row.purchase_quantity != quantity || row.delivery != delivery)
        {
            return Err("monthlyCardPurchaseReplayMismatch".into());
        }
        let mut result = Vec::new();
        for ordinal in 0..quantity {
            let row = existing
                .iter()
                .find(|row| row.ordinal == ordinal)
                .ok_or("monthlyCardItemReceiptInvalid")?;
            let mut item = embedded_item_state_from_template(
                &billing_monthly_card_item_template(),
                ItemContainer::Bag1,
                0,
            );
            item.unique_id = row.unique_id;
            result.push(item);
        }
        return Ok((result, true));
    }
    if ledger
        .item_receipts
        .len()
        .checked_add(usize::from(quantity))
        .is_none_or(|count| count > MAX_ITEM_RECEIPTS)
    {
        return Err("monthlyCardLedgerFull".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for stored in account.saves.values() {
        collect_saved_unique_ids(stored, &mut seen)?;
    }
    collect_saved_unique_ids(save, &mut seen)?;
    seen.extend(ledger.item_receipts.values().map(|row| row.unique_id));
    let mut staged_ledger = ledger;
    let mut result = Vec::new();
    for ordinal in 0..quantity {
        let (key, uid) = monthly_card_item_identity(account_id, character, request, ordinal);
        if !seen.insert(uid) || staged_ledger.item_receipts.contains_key(&key) {
            return Err("monthlyCardItemIdentityCollision".into());
        }
        staged_ledger.item_receipts.insert(
            key,
            MonthlyCardItemReceipt {
                account_id: account_id.into(),
                character_index: character,
                request_id: request.into(),
                ordinal,
                purchase_quantity: quantity,
                unique_id: uid,
                credit_price: price,
                delivery,
                issued_at_ms: now_ms,
                redeemed_at_ms: None,
                credited_until_ms: None,
            },
        );
        let mut item = embedded_item_state_from_template(
            &billing_monthly_card_item_template(),
            ItemContainer::Bag1,
            0,
        );
        item.unique_id = uid;
        validate_card_item(&item)?;
        result.push(item);
    }
    staged_ledger.validate()?;
    account.monthly_card = Some(staged_ledger);
    Ok((result, false))
}

fn prepare_save(
    account: &AccountRecord,
    character: i32,
    active: Option<&CharacterSaveRecord>,
) -> Result<CharacterSaveRecord, String> {
    let durable = account_character(account, character)?;
    match active {
        Some(active) => {
            if !same_character(&active.character, &durable.character)
                || active.revision != durable.revision
            {
                return Err("monthlyCardStaleCharacterSnapshot".into());
            }
            let mut current = active.clone();
            merge_persisted_mail_into_character_save(&mut current, durable)?;
            Ok(current)
        }
        None => Ok(durable.clone()),
    }
}
fn publish_save(
    account: &mut AccountRecord,
    mut save: CharacterSaveRecord,
) -> Result<CharacterSaveRecord, String> {
    save.revision = save
        .revision
        .checked_add(1)
        .ok_or("monthlyCardRevisionExhausted")?;
    validate_character_save_record(&save)?;
    account.saves.insert(save.character.index, save.clone());
    Ok(save)
}

fn purchase(
    config: &SimulationConfig,
    account_id: &str,
    character: i32,
    request: &str,
    now_ms: u64,
    active: Option<&CharacterSaveRecord>,
) -> Result<(BillingMonthlyCardPurchaseReceipt, CharacterSaveRecord), String> {
    if !valid_request_id(request) {
        return Err("monthlyCardRequestIdInvalid".into());
    }
    config.refresh_account_store_account(account_id)?;
    config.commit_account_store_transaction(&[account_id.to_owned()], |store| {
        let account = store
            .accounts
            .get_mut(account_id)
            .ok_or("monthlyCardAccountMissing")?;
        if account.active_ban(now_ms).is_some() {
            return Err("monthlyCardAccountBanned".into());
        }
        let durable = account_character(account, character)?.clone();
        if let Some(card) = &account.monthly_card {
            card.validate()?;
            if let Some(row) = card.item_receipts.values().find(|row| {
                row.account_id == account_id
                    && row.character_index == character
                    && row.request_id == request
            }) {
                if row.delivery != MonthlyCardItemDelivery::Bag || row.purchase_quantity != 1 {
                    return Err("monthlyCardPurchaseReplayMismatch".into());
                }
                return Ok((
                    BillingMonthlyCardPurchaseReceipt {
                        request_id: request.into(),
                        unique_id: row.unique_id,
                        replayed: true,
                    },
                    durable,
                ));
            }
        }
        let price = config
            .billing_monthly_card_credit_price
            .filter(|price| *price > 0)
            .ok_or("monthlyCardShopUnavailable")?;
        let mut save = prepare_save(account, character, active)?;
        let mut inventory = decode_items(&save.inventory_items_json)?;
        let (container, slot) = find_empty_inventory_item_slot(
            &inventory,
            ItemContainer::Bag1,
            save.inventory_capacity,
        )
        .ok_or("monthlyCardInventoryFull")?;
        let (mut issued, _) = issue_credit_purchase(
            account,
            &mut save,
            account_id,
            character,
            request,
            1,
            price,
            now_ms,
            MonthlyCardItemDelivery::Bag,
        )?;
        let mut item = issued.remove(0);
        item.container = container;
        item.slot = slot;
        let uid = item.unique_id;
        inventory.push(item);
        save.inventory_items_json = encode_state_vec(&inventory);
        let save = publish_save(account, save)?;
        Ok((
            BillingMonthlyCardPurchaseReceipt {
                request_id: request.into(),
                unique_id: uid,
                replayed: false,
            },
            save,
        ))
    })
}

fn remove_card(save: &mut CharacterSaveRecord, uid: u64, allow_mail: bool) -> Result<(), String> {
    validate_character_save_record(save)?;
    for item in items_in_save(save)? {
        if item_unique_id(&item) != uid && super::inventory::item_tree_unique_id_is_used(&item, uid)
        {
            return Err("monthlyCardItemIdentityCollision".into());
        }
    }
    let equipment =
        decode_state_vec::<super::equipment::EquipmentState>(&save.equipment_items_json)
            .ok_or("monthlyCardInventoryInvalid")?;
    if equipment
        .iter()
        .any(|item| super::inventory::equipment_tree_unique_id_is_used(item, uid))
        || super::item_custody::reserved_ids(&systems(save)?)?.contains(&uid)
    {
        return Err("monthlyCardItemIdentityCollision".into());
    }
    let mut matches = 0usize;
    for items in [
        &save.inventory_items_json,
        &save.belt_items_json,
        &save.storage_items_json,
        &save.hero_inventory_items_json,
        &save.hero_equipment_items_json,
    ] {
        for item in decode_items(items)? {
            if item_unique_id(&item) == uid {
                validate_card_item(&item)?;
                matches += 1;
            }
        }
    }
    let mut state = systems(save)?;
    for mail in &state.mail {
        if !mail.claimed && !mail.deleted {
            for item in decode_items(&mail.item_states_json)? {
                if item_unique_id(&item) == uid {
                    validate_card_item(&item)?;
                    matches += 1;
                }
            }
        }
    }
    if matches != 1 {
        return Err("monthlyCardItemNotOwned".into());
    }
    for encoded in [&mut save.inventory_items_json, &mut save.belt_items_json] {
        let mut items = decode_items(encoded)?;
        if let Some(index) = items.iter().position(|item| item_unique_id(item) == uid) {
            items.remove(index);
            *encoded = encode_state_vec(&items);
            return Ok(());
        }
    }
    if allow_mail {
        for mail in &mut state.mail {
            if mail.claimed || mail.deleted {
                continue;
            }
            let mut items = decode_items(&mail.item_states_json)?;
            if let Some(index) = items.iter().position(|item| item_unique_id(item) == uid) {
                if mail.items.len() != items.len() {
                    return Err("monthlyCardInventoryInvalid".into());
                }
                items.remove(index);
                mail.items.remove(index);
                mail.item_states_json = encode_state_vec(&items);
                if items.is_empty() && mail.gold == 0 {
                    mail.claimed = true;
                }
                save.stage5_systems_json =
                    Some(serde_json::to_string(&state).map_err(|_| "monthlyCardInventoryInvalid")?);
                return Ok(());
            }
        }
    }
    Err("monthlyCardItemNotOwned".into())
}

fn activate(
    config: &SimulationConfig,
    account_id: &str,
    character: i32,
    uid: u64,
    now_ms: u64,
    active: Option<&CharacterSaveRecord>,
    allow_mail: bool,
) -> Result<(BillingMonthlyCardActivationReceipt, CharacterSaveRecord), String> {
    config.refresh_account_store_account(account_id)?;
    config.commit_account_store_transaction(&[account_id.to_owned()], |store| {
        let account = store
            .accounts
            .get_mut(account_id)
            .ok_or("monthlyCardAccountMissing")?;
        if account.active_ban(now_ms).is_some() {
            return Err("monthlyCardAccountBanned".into());
        }
        let durable = account_character(account, character)?.clone();
        let card = account
            .monthly_card
            .as_ref()
            .ok_or("monthlyCardItemNotIssued")?;
        let row = owned_receipt(card, account_id, character, uid)?;
        if row.redeemed_at_ms.is_some() {
            return Ok((
                BillingMonthlyCardActivationReceipt {
                    unique_id: uid,
                    replayed: true,
                    status: MonthlyCardStatus::new(
                        config.monthly_card_policy.required,
                        card.expires_at_ms,
                        now_ms,
                    ),
                },
                durable,
            ));
        }
        if now_ms < row.issued_at_ms {
            return Err("monthlyCardTimeInvalid".into());
        }
        let mut save = prepare_save(account, character, active)?;
        remove_card(&mut save, uid, allow_mail)?;
        let card = account
            .monthly_card
            .as_mut()
            .expect("validated monthly card ledger");
        let expiry = card
            .expires_at_ms
            .max(now_ms)
            .checked_add(MONTHLY_CARD_DURATION_MS)
            .ok_or("monthlyCardTimeOverflow")?;
        let row = card
            .item_receipts
            .values_mut()
            .find(|row| row.unique_id == uid)
            .expect("validated monthly card unit");
        row.redeemed_at_ms = Some(now_ms);
        row.credited_until_ms = Some(expiry);
        card.expires_at_ms = expiry;
        card.validate()?;
        let save = publish_save(account, save)?;
        Ok((
            BillingMonthlyCardActivationReceipt {
                unique_id: uid,
                replayed: false,
                status: MonthlyCardStatus::new(config.monthly_card_policy.required, expiry, now_ms),
            },
            save,
        ))
    })
}

impl SimulationConfig {
    pub fn buy_monthly_card_for_character(
        &self,
        account_id: &str,
        character_index: i32,
        request_id: &str,
        now_ms: u64,
    ) -> Result<BillingMonthlyCardPurchaseReceipt, String> {
        purchase(self, account_id, character_index, request_id, now_ms, None).map(|value| value.0)
    }
    pub fn activate_monthly_card_for_character(
        &self,
        account_id: &str,
        character_index: i32,
        unique_id: u64,
        now_ms: u64,
    ) -> Result<BillingMonthlyCardActivationReceipt, String> {
        activate(
            self,
            account_id,
            character_index,
            unique_id,
            now_ms,
            None,
            true,
        )
        .map(|value| value.0)
    }
    pub fn list_billing_monthly_cards(
        &self,
        account_id: &str,
        character_index: i32,
    ) -> Result<Vec<BillingMonthlyCardItem>, String> {
        self.refresh_account_store_account(account_id)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "monthlyCardStoreUnavailable")?;
        let account = store
            .accounts
            .get(account_id)
            .ok_or("monthlyCardAccountMissing")?;
        let save = account_character(account, character_index)?;
        let mut result = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for item in items_in_save(save)? {
            if !is_card(&item) {
                continue;
            }
            validate_card_item(&item)?;
            let uid = item_unique_id(&item);
            let receipt = owned_receipt(
                account
                    .monthly_card
                    .as_ref()
                    .ok_or("monthlyCardItemNotIssued")?,
                account_id,
                character_index,
                uid,
            )?;
            if receipt.redeemed_at_ms.is_some() || !seen.insert(uid) {
                return Err("monthlyCardItemInvalid".into());
            }
            result.push(BillingMonthlyCardItem {
                unique_id: uid,
                label: "30-day account access".into(),
            });
        }
        Ok(result)
    }
}

fn mirror_committed(
    world: &mut World,
    expected_revision: u64,
    save: &CharacterSaveRecord,
) -> Result<(), String> {
    let inventory = decode_items(&save.inventory_items_json)?;
    let belt = decode_items(&save.belt_items_json)?;
    let mail = systems(save)?.mail;
    if !world
        .resource::<SessionResource>()
        .advance_active_save_revision(expected_revision, save.revision)
    {
        return Err("monthlyCardCommittedMirrorRevisionChanged".into());
    }
    world.resource_mut::<PlayerRuntimeResource>().credit = save.credit;
    let mut resources = world.resource_mut::<InventoryResource>();
    resources.inventory_items = inventory;
    resources.belt_items = belt;
    drop(resources);
    world
        .resource_mut::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .mail = mail;
    Ok(())
}

pub(super) fn use_card_packet(
    world: &mut World,
    uid: u64,
    grid: MirGridType,
) -> Option<Vec<ServerPacket>> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let session = world.resource::<SessionResource>();
    let account = session.account_id.clone()?;
    let character = session.selected_character.as_ref()?.index;
    let (known, redeemed) = config
        .account_store
        .lock()
        .ok()?
        .accounts
        .get(&account)?
        .monthly_card
        .as_ref()
        .and_then(|ledger| {
            ledger
                .item_receipts
                .values()
                .find(|row| row.unique_id == uid)
        })
        .map_or((false, false), |row| (true, row.redeemed_at_ms.is_some()));
    let referenced_card =
        item_key_for_client_reference(world, uid, grid).is_some_and(|key| key == CARD_KEY);
    if !known && !referenced_card {
        return None;
    }
    let ack = |success| ServerPacket::UseItem {
        unique_id: uid,
        grid,
        success,
    };
    if !matches!(grid, MirGridType::Inventory | MirGridType::Belt) {
        return Some(vec![ack(false)]);
    }
    if !redeemed && !referenced_card {
        return Some(vec![ack(false)]);
    }
    let Some(active) = snapshot_active_character_save(world) else {
        return Some(vec![ack(false)]);
    };
    match activate(
        &config,
        &account,
        character,
        uid,
        crate::monthly_card::monthly_card_now_ms(),
        Some(&active),
        false,
    ) {
        Ok((receipt, committed)) => {
            if !receipt.replayed {
                if let Err(error) = mirror_committed(world, active.revision, &committed) {
                    super::shared_guild_experience::reject_source(world, error);
                    return Some(Vec::new());
                }
            }
            Some(vec![ack(true)])
        }
        Err(error) => {
            if config.ensure_account_store_writable().is_err() {
                super::shared_guild_experience::reject_source(world, error);
                Some(Vec::new())
            } else {
                Some(vec![ack(false)])
            }
        }
    }
}

impl SimulationSession {
    pub fn buy_billing_monthly_card(
        &mut self,
        request_id: &str,
        now_ms: u64,
    ) -> Result<Vec<ServerPacket>, String> {
        let mut packets = self.consume_pending_recharge_credits()?;
        let identity = self
            .active_identity()
            .ok_or("monthlyCardCharacterMissing")?;
        let active = snapshot_active_character_save(self.app.world())
            .ok_or("monthlyCardCharacterMissing")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let (receipt, committed) = purchase(
            &config,
            &identity.account_id,
            identity.character_index,
            request_id,
            now_ms,
            Some(&active),
        )?;
        if !receipt.replayed {
            let spent = active
                .credit
                .checked_sub(committed.credit)
                .ok_or("monthlyCardPriceOverflow")?;
            mirror_committed(self.app.world_mut(), active.revision, &committed)?;
            packets.push(ServerPacket::LoseCredit { credit: spent });
            let item = decode_items(&committed.inventory_items_json)?
                .into_iter()
                .find(|item| item_unique_id(item) == receipt.unique_id)
                .ok_or("monthlyCardItemInvalid")?;
            packets.push(ServerPacket::GainedItem {
                item: super::items::user_item_from_item_state(&item),
            });
        }
        Ok(self.finalize_packets(packets))
    }
    pub fn activate_billing_monthly_card(
        &mut self,
        unique_id: u64,
        now_ms: u64,
    ) -> Result<Vec<ServerPacket>, String> {
        let identity = self
            .active_identity()
            .ok_or("monthlyCardCharacterMissing")?;
        let active = snapshot_active_character_save(self.app.world())
            .ok_or("monthlyCardCharacterMissing")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let (receipt, committed) = activate(
            &config,
            &identity.account_id,
            identity.character_index,
            unique_id,
            now_ms,
            Some(&active),
            false,
        )?;
        if !receipt.replayed {
            mirror_committed(self.app.world_mut(), active.revision, &committed)?;
        }
        let grid = if decode_items(&active.belt_items_json)?
            .iter()
            .any(|item| item_unique_id(item) == unique_id)
        {
            MirGridType::Belt
        } else {
            MirGridType::Inventory
        };
        Ok(self.finalize_packets(vec![ServerPacket::UseItem {
            unique_id,
            grid,
            success: true,
        }]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_allocation_and_mail_normalization_skip_permanent_card_namespace() {
        let session = SimulationSession::new(SimulationConfig::default());
        let mut inventory = session.app.world().resource::<InventoryResource>().clone();
        inventory.inventory_items.clear();
        inventory.belt_items.clear();
        inventory.storage_items.clear();
        inventory.equipment_items.clear();
        inventory.reserved_item_unique_ids.clear();
        let (_, uid) = monthly_card_item_identity("demo", 1, "allocation-test-0001", 0);
        let mut card = embedded_item_state_from_template(
            &billing_monthly_card_item_template(),
            ItemContainer::Bag1,
            0,
        );
        card.unique_id = uid;
        inventory.inventory_items.push(card.clone());
        let template = mir2_game_data::crystal_item_by_name("GoldOre").unwrap();
        let mut ordinary = embedded_item_state_from_template(&template, ItemContainer::Bag1, 1);
        ordinary.unique_id = 1;
        inventory.inventory_items.push(ordinary.clone());
        let allocated =
            super::super::inventory::allocate_item_unique_id(&inventory, ItemContainer::Bag1, 1);
        assert!(allocated > uid);
        assert!(!monthly_card_item_uid_is_reserved(allocated));
        super::super::inventory::normalize_fresh_item_tree_unique_ids(
            &inventory,
            &mut ordinary,
            &[],
        );
        assert!(ordinary.unique_id > uid);
        assert!(!monthly_card_item_uid_is_reserved(ordinary.unique_id));
        super::super::inventory::normalize_inventory_unique_ids(&mut inventory);
        assert_eq!(inventory.inventory_items[0].unique_id, uid);
    }
}
