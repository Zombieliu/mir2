//! Serial Source owner purchase: one payer, one frozen recipient, one commit.
use std::collections::{BTreeMap, BTreeSet};

use mir2_game_data::{crystal_item_by_index, BILLING_MONTHLY_CARD_GAME_SHOP_INDEX};
use mir2_protocol::{GameShopItem, ServerPacket};
use serde::{Deserialize, Serialize};

use crate::config::{
    new_stage5_mail_delivery_nonce, with_account_store_postgres_client, AccountStore,
    AccountStoreDatabaseMode, CharacterRecord, CharacterSaveRecord, SimulationConfig,
    Stage5MailMessage, Stage5SystemsState,
};
use crate::{NativeGameShopGiftRequest, NativeGameShopPurchaseRequest};

use super::items::crystal_item_key_for_template;
use super::resources::{
    is_in_world, PlayerRuntimeResource, SessionResource, Stage5SystemsResource,
};
use super::save::{
    merge_persisted_mail_into_character_save, snapshot_active_character_save,
    validate_character_save_record,
};
use super::session::SimulationSession;
use super::stage5::{
    authoritative_game_shop_product_for_world, game_shop_attachment_states_json,
    game_shop_class_matches, game_shop_stock_available, game_shop_stock_level,
    validate_native_game_shop_purchase_request, validate_stage5_systems_item_carriers,
    GameShopPurchaseExecution, GameShopPurchaseFailure, GameShopPurchaseOutcome,
};

const LEDGER_FROM: &str = "Mir2.Internal";
const LEDGER_SUBJECT: &str = "NativeGameShopGiftLedgerV1";
const LEDGER_NONCE: &str = "native-gameshop-gift-ledger-v1";
const LEDGER_CAPACITY: usize = 4096;
const MAIL_CAPACITY: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recipient {
    account_id: String,
    character: CharacterRecord,
}

// A known-no-commit CAS retry must retain the originally resolved character.
// Looking up its name again could redirect a pending gift after name reuse.
enum RecipientSelection {
    Resolve,
    Frozen(Option<Recipient>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GiftEntry {
    purchase: NativeGameShopPurchaseRequest,
    recipient_name: String,
    recipient: Option<Recipient>,
    unit_credit_price: u32,
    charged_credit: u32,
    delivery_nonce: Option<String>,
    monthly_card_uids: Vec<u64>,
    outcome: GameShopPurchaseOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GiftLedger {
    protocol_version: u16,
    account_id: String,
    character_index: i32,
    entries: Vec<GiftEntry>,
}

fn valid_name(name: &str) -> bool {
    (3..=15).contains(&name.chars().count())
        && name.len() <= 45
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '_'
                || ('\u{4e00}'..='\u{9fa5}').contains(&character)
        })
}

fn systems(save: &CharacterSaveRecord) -> Result<Stage5SystemsState, String> {
    save.stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "gameShopGiftMalformedSystems".to_string())
        .map(|value| value.unwrap_or_default())
}

fn same_character(a: &CharacterRecord, b: &CharacterRecord) -> bool {
    a.index == b.index && a.name == b.name && a.class == b.class && a.gender == b.gender
}

fn ledger_candidate(mail: &Stage5MailMessage) -> bool {
    (mail.from == LEDGER_FROM && mail.subject == LEDGER_SUBJECT)
        || mail.delivery_nonce == LEDGER_NONCE
}

fn decode_ledger(mail: &Stage5MailMessage) -> Result<GiftLedger, String> {
    if !ledger_candidate(mail)
        || mail.from != LEDGER_FROM
        || mail.subject != LEDGER_SUBJECT
        || mail.delivery_nonce != LEDGER_NONCE
        || !mail.deleted
        || !mail.claimed
        || !mail.locked
        || !mail.opened
        || mail.gold != 0
        || !mail.items.is_empty()
        || !mail.item_states_json.is_empty()
    {
        return Err("gameShopGiftLedgerMetadataInvalid".into());
    }
    let ledger: GiftLedger =
        serde_json::from_str(&mail.body).map_err(|_| "gameShopGiftLedgerInvalid".to_string())?;
    if ledger.protocol_version != 1
        || ledger.account_id.is_empty()
        || ledger.entries.len() > LEDGER_CAPACITY
    {
        return Err("gameShopGiftLedgerBindingInvalid".into());
    }
    let mut keys = BTreeSet::new();
    let mut client_keys = BTreeSet::new();
    for entry in &ledger.entries {
        let purchase = &entry.purchase;
        validate_native_game_shop_purchase_request(purchase)?;
        if purchase.account_id != ledger.account_id
            || purchase.character_index != ledger.character_index
            || !valid_name(&entry.recipient_name)
            || !keys.insert(&purchase.server_idempotency_key)
            || !client_keys.insert((&purchase.gateway_session_id, &purchase.client_request_id))
            || entry.outcome.g_index != purchase.g_index
            || entry.outcome.quantity != purchase.quantity
            || entry.outcome.price_type != purchase.price_type
        {
            return Err("gameShopGiftLedgerEntryInvalid".into());
        }
        if entry.outcome.success {
            let recipient = entry
                .recipient
                .as_ref()
                .ok_or("gameShopGiftLedgerRecipientMissing")?;
            if purchase.price_type != 0
                || !(1..=99).contains(&purchase.quantity)
                || recipient.account_id.is_empty()
                || !valid_name(&recipient.character.name)
                || !recipient
                    .character
                    .name
                    .eq_ignore_ascii_case(&entry.recipient_name)
                || (recipient.account_id == ledger.account_id
                    && recipient.character.index == ledger.character_index)
                || entry.unit_credit_price == 0
                || entry
                    .unit_credit_price
                    .checked_mul(u32::from(purchase.quantity))
                    != Some(entry.charged_credit)
                || entry.outcome.mail_id.is_none_or(|id| id == 0)
                || entry
                    .delivery_nonce
                    .as_ref()
                    .is_none_or(|nonce| nonce.is_empty())
            {
                return Err("gameShopGiftLedgerSuccessInvalid".into());
            }
            if purchase.g_index == BILLING_MONTHLY_CARD_GAME_SHOP_INDEX {
                let request = format!("gameshop-gift-{}", purchase.server_idempotency_key);
                let expected: Vec<_> = (0..u16::from(purchase.quantity))
                    .map(|ordinal| {
                        crate::monthly_card::monthly_card_item_identity(
                            &recipient.account_id,
                            recipient.character.index,
                            &request,
                            ordinal,
                        )
                        .1
                    })
                    .collect();
                if entry.monthly_card_uids != expected {
                    return Err("gameShopGiftLedgerItemBindingInvalid".into());
                }
            } else if !entry.monthly_card_uids.is_empty() {
                return Err("gameShopGiftLedgerItemBindingInvalid".into());
            }
        } else if entry.unit_credit_price != 0
            || entry.charged_credit != 0
            || entry.delivery_nonce.is_some()
            || !entry.monthly_card_uids.is_empty()
        {
            return Err("gameShopGiftLedgerFailureInvalid".into());
        }
    }
    Ok(ledger)
}

fn ledger_index(state: &Stage5SystemsState) -> Result<Option<usize>, String> {
    let mut candidates = state
        .mail
        .iter()
        .enumerate()
        .filter(|(_, mail)| ledger_candidate(mail));
    let Some((index, mail)) = candidates.next() else {
        return Ok(None);
    };
    if candidates.next().is_some() {
        return Err("gameShopGiftMultipleLedgers".into());
    }
    decode_ledger(mail)?;
    Ok(Some(index))
}

fn existing_outcome(
    state: &Stage5SystemsState,
    request: &NativeGameShopGiftRequest,
) -> Result<Option<GameShopPurchaseOutcome>, String> {
    let Some(index) = ledger_index(state)? else {
        return Ok(None);
    };
    let ledger = decode_ledger(&state.mail[index])?;
    if ledger.account_id != request.purchase.account_id
        || ledger.character_index != request.purchase.character_index
    {
        return Err("gameShopGiftLedgerOwnerMismatch".into());
    }
    let entry = ledger.entries.iter().find(|entry| {
        entry.purchase.server_idempotency_key == request.purchase.server_idempotency_key
            || (entry.purchase.gateway_session_id == request.purchase.gateway_session_id
                && entry.purchase.client_request_id == request.purchase.client_request_id)
    });
    let Some(entry) = entry else {
        return Ok(None);
    };
    if entry.purchase != request.purchase || entry.recipient_name != request.recipient_name {
        return Err("gameShopGiftReplayMismatch".into());
    }
    Ok(Some(entry.outcome.clone()))
}

fn record_entry(
    state: &mut Stage5SystemsState,
    sender: &str,
    entry: GiftEntry,
) -> Result<(), String> {
    let index = ledger_index(state)?;
    let mut ledger = match index {
        Some(index) => decode_ledger(&state.mail[index])?,
        None => GiftLedger {
            protocol_version: 1,
            account_id: entry.purchase.account_id.clone(),
            character_index: entry.purchase.character_index,
            entries: Vec::new(),
        },
    };
    if ledger.entries.len() >= LEDGER_CAPACITY {
        return Err("gameShopGiftLedgerFull".into());
    }
    ledger.entries.push(entry);
    ledger.entries.sort_by(|a, b| {
        a.purchase
            .server_idempotency_key
            .cmp(&b.purchase.server_idempotency_key)
    });
    let body = serde_json::to_string(&ledger).map_err(|_| "gameShopGiftLedgerInvalid")?;
    if let Some(index) = index {
        state.mail[index].body = body;
        decode_ledger(&state.mail[index])?;
    } else {
        let id = next_mail_id(state)?;
        let mail = Stage5MailMessage {
            id,
            delivery_nonce: LEDGER_NONCE.into(),
            from: LEDGER_FROM.into(),
            to: sender.into(),
            subject: LEDGER_SUBJECT.into(),
            body,
            gold: 0,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: true,
            locked: true,
            claimed: true,
            deleted: true,
        };
        decode_ledger(&mail)?;
        state.mail.push(mail);
    }
    Ok(())
}

/// Dedicated GiftV1 union. A conflict is corruption, never a last-writer win.
pub(super) fn merge_game_shop_gift_ledger_mail(
    local: &mut Stage5MailMessage,
    external: &Stage5MailMessage,
) -> Result<Option<bool>, String> {
    if !ledger_candidate(local) && !ledger_candidate(external) {
        return Ok(None);
    }
    let mut ledger = decode_ledger(local)?;
    let incoming = decode_ledger(external)?;
    if local.to != external.to
        || ledger.account_id != incoming.account_id
        || ledger.character_index != incoming.character_index
    {
        return Err("gameShopGiftLedgerMergeBindingMismatch".into());
    }
    let mut entries: BTreeMap<_, _> = ledger
        .entries
        .into_iter()
        .map(|entry| (entry.purchase.server_idempotency_key.clone(), entry))
        .collect();
    for entry in incoming.entries {
        match entries.get(&entry.purchase.server_idempotency_key) {
            Some(previous) if previous != &entry => {
                return Err("gameShopGiftLedgerMergeConflict".into())
            }
            Some(_) => {}
            None => {
                entries.insert(entry.purchase.server_idempotency_key.clone(), entry);
            }
        }
    }
    ledger.entries = entries.into_values().collect();
    let body = serde_json::to_string(&ledger).map_err(|_| "gameShopGiftLedgerInvalid")?;
    let mut merged = local.clone();
    merged.body = body;
    decode_ledger(&merged)?;
    let changed = local.body != merged.body;
    *local = merged;
    Ok(Some(changed))
}

fn next_mail_id(state: &Stage5SystemsState) -> Result<u32, String> {
    state
        .mail
        .iter()
        .map(|mail| mail.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "gameShopGiftMailIdExhausted".into())
}

fn unique_recipient(
    store: &AccountStore,
    name: &str,
) -> Result<Recipient, GameShopPurchaseFailure> {
    let mut matches = store.accounts.iter().flat_map(|(account_id, account)| {
        account
            .characters
            .iter()
            .filter(move |character| character.name.eq_ignore_ascii_case(name))
            .map(move |character| Recipient {
                account_id: account_id.clone(),
                character: character.clone(),
            })
    });
    let recipient = matches
        .next()
        .ok_or(GameShopPurchaseFailure::RecipientUnavailable)?;
    if matches.next().is_some() {
        return Err(GameShopPurchaseFailure::RecipientUnavailable);
    }
    Ok(recipient)
}

fn resolve_recipient(config: &SimulationConfig, name: &str) -> Result<Option<Recipient>, String> {
    if config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth {
        let database_url = config
            .account_store_database_url
            .as_deref()
            .ok_or("gameShopGiftSourceUnavailable")?;
        let found = with_account_store_postgres_client(database_url, |client| {
            let rows = client.query("SELECT account_id, raw_json FROM characters WHERE lower(character_name) = lower($1)", &[&name])
                .map_err(|_| "gameShopGiftRecipientLookupFailed".to_string())?;
            let mut matches = Vec::new();
            for row in rows {
                let character: CharacterRecord = serde_json::from_value(row.get("raw_json"))
                    .map_err(|_| "gameShopGiftRecipientIdentityInvalid".to_string())?;
                if character.name.eq_ignore_ascii_case(name) {
                    matches.push(Recipient {
                        account_id: row.get("account_id"),
                        character,
                    });
                }
            }
            Ok(if matches.len() == 1 {
                matches.pop()
            } else {
                None
            })
        })?;
        if let Some(target) = &found {
            config.refresh_account_store_account(&target.account_id)?;
        }
        return Ok(found);
    }
    let store = config
        .account_store
        .lock()
        .map_err(|_| "gameShopGiftStoreUnavailable")?;
    Ok(unique_recipient(&store, name).ok())
}

struct Product {
    item: GameShopItem,
    item_key: String,
    item_name: String,
    item_count: u32,
    attachments: Vec<String>,
    total: u32,
}

fn product(
    world: &bevy_ecs::world::World,
    request: &NativeGameShopPurchaseRequest,
    character: &CharacterRecord,
) -> Result<Product, GameShopPurchaseFailure> {
    if request.price_type != 0 {
        return Err(GameShopPurchaseFailure::PaymentUnavailable);
    }
    if !(1..=99).contains(&request.quantity) {
        return Err(GameShopPurchaseFailure::InvalidQuantity);
    }
    let (item, _) = authoritative_game_shop_product_for_world(world, request.g_index)
        .ok_or(GameShopPurchaseFailure::UnknownProduct)?;
    if !game_shop_class_matches(&item.class, character.class) {
        return Err(GameShopPurchaseFailure::ClassUnavailable);
    }
    if !item.can_buy_credit {
        return Err(GameShopPurchaseFailure::PaymentUnavailable);
    }
    if item.stock < 0 {
        return Err(GameShopPurchaseFailure::StockUnavailable);
    }
    let template =
        crystal_item_by_index(item.item_index).ok_or(GameShopPurchaseFailure::UnknownProduct)?;
    let count = u32::from(item.count)
        .checked_mul(u32::from(request.quantity))
        .ok_or(GameShopPurchaseFailure::InvalidQuantity)?;
    if count == 0 || count.div_ceil(u32::from(template.stack_size.max(1))) > 5 {
        return Err(GameShopPurchaseFailure::InvalidQuantity);
    }
    let item_key = crystal_item_key_for_template(&template);
    let attachments = game_shop_attachment_states_json(&template, &item_key, count)?;
    let total = item
        .credit_price
        .checked_mul(u32::from(request.quantity))
        .filter(|value| *value > 0)
        .ok_or(GameShopPurchaseFailure::InvalidPriceType)?;
    Ok(Product {
        item,
        item_key,
        item_name: template.name,
        item_count: count,
        attachments,
        total,
    })
}

struct Commit {
    outcome: GameShopPurchaseOutcome,
    sender_save: Option<CharacterSaveRecord>,
    charged_credit: u32,
}

fn finish_commit(
    store: &mut AccountStore,
    mut save: CharacterSaveRecord,
    mut state: Stage5SystemsState,
    request: &NativeGameShopGiftRequest,
    recipient: Option<Recipient>,
    charged_credit: u32,
    delivery_nonce: Option<String>,
    monthly_card_uids: Vec<u64>,
    outcome: GameShopPurchaseOutcome,
) -> Result<Commit, String> {
    record_entry(
        &mut state,
        &save.character.name,
        GiftEntry {
            purchase: request.purchase.clone(),
            recipient_name: request.recipient_name.clone(),
            recipient,
            unit_credit_price: if charged_credit == 0 {
                0
            } else {
                charged_credit
                    .checked_div(u32::from(request.purchase.quantity))
                    .ok_or("gameShopGiftPriceInvalid")?
            },
            charged_credit,
            delivery_nonce,
            monthly_card_uids,
            outcome: outcome.clone(),
        },
    )?;
    save.stage5_systems_json =
        Some(serde_json::to_string(&state).map_err(|_| "gameShopGiftMalformedSystems")?);
    save.revision = save
        .revision
        .checked_add(1)
        .ok_or("gameShopGiftRevisionExhausted")?;
    validate_character_save_record(&save)?;
    store
        .accounts
        .get_mut(&request.purchase.account_id)
        .ok_or("gameShopGiftSenderMissing")?
        .saves
        .insert(request.purchase.character_index, save.clone());
    Ok(Commit {
        outcome,
        sender_save: Some(save),
        charged_credit,
    })
}

fn failure(
    request: &NativeGameShopGiftRequest,
    code: GameShopPurchaseFailure,
    stock: Option<i32>,
) -> GameShopPurchaseOutcome {
    GameShopPurchaseOutcome {
        success: false,
        g_index: request.purchase.g_index,
        quantity: request.purchase.quantity,
        price_type: request.purchase.price_type,
        new_stock_level: stock,
        mail_id: None,
        failure: Some(code),
    }
}

impl SimulationSession {
    /// Paired commercial configuration also enables the authenticated 5s
    /// Source repository poll, including a recipient's first external gift.
    pub(crate) fn game_shop_gift_enabled(&self) -> bool {
        self.app.world().resource::<super::resources::RuntimeConfigResource>()
            .config.billing_monthly_card_credit_price.is_some_and(|price| price > 0)
    }

    pub(crate) fn game_shop_gift_mail_notifications(&mut self) -> Vec<ServerPacket> {
        if self.game_shop_gift_enabled() && self.refresh_active_external_mail() {
            vec![super::packets::stage5_receive_mail_packet(self.app.world())]
        } else { Vec::new() }
    }

    pub(crate) fn game_shop_gift_packet_idempotent(
        &mut self,
        request: NativeGameShopGiftRequest,
    ) -> Result<GameShopPurchaseExecution, String> {
        self.game_shop_gift_attempt(request, true, RecipientSelection::Resolve)
    }

    fn game_shop_gift_attempt(
        &mut self,
        request: NativeGameShopGiftRequest,
        retry_known_cas: bool,
        recipient_selection: RecipientSelection,
    ) -> Result<GameShopPurchaseExecution, String> {
        validate_native_game_shop_purchase_request(&request.purchase)?;
        if !valid_name(&request.recipient_name) {
            return Err("gameShopGiftRecipientNameInvalid".into());
        }
        if !is_in_world(self.app.world()) {
            return Err("gameShopGiftOwnerNotInWorld".into());
        }
        let identity = self
            .active_identity()
            .ok_or("gameShopGiftAuthenticationRequired")?;
        if identity.account_id != request.purchase.account_id
            || identity.character_index != request.purchase.character_index
        {
            return Err("gameShopGiftOwnerIdentityMismatch".into());
        }
        let (config, _) = self.billing_authenticated_config()?;
        // Replay never resolves a name again, even after rename or deletion.
        {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "gameShopGiftStoreUnavailable")?;
            let save = store
                .accounts
                .get(&identity.account_id)
                .and_then(|account| account.saves.get(&identity.character_index))
                .ok_or("gameShopGiftSenderMissing")?;
            if let Some(outcome) = existing_outcome(&systems(save)?, &request)? {
                return Ok(GameShopPurchaseExecution {
                    packets: Vec::new(),
                    outcome,
                });
            }
        }
        // A previously committed receipt remains readable if gifting is
        // later disabled; only fresh economic writes require paired config.
        if !self.game_shop_gift_enabled() { return Err("gameShopGiftUnavailable".into()); }
        let checkpoint = snapshot_active_character_save(self.app.world())
            .ok_or("gameShopGiftOwnerNotInWorld")?;
        validate_character_save_record(&checkpoint)?;
        let expected_revision = checkpoint.revision;
        let product = product(self.app.world(), &request.purchase, &checkpoint.character);
        let recipient = match recipient_selection {
            RecipientSelection::Resolve => resolve_recipient(&config, &request.recipient_name)?,
            RecipientSelection::Frozen(recipient) => recipient,
        };
        let retry_recipient = recipient.clone();
        let uses_global = product
            .as_ref()
            .is_ok_and(|product| product.item.stock > 0 && !product.item.i_stock);
        let mut accounts = vec![identity.account_id.clone()];
        if let Some(target) = &recipient {
            accounts.push(target.account_id.clone());
        }
        accounts.sort();
        accounts.dedup();
        let request_for_commit = request.clone();
        let now_ms = crate::monthly_card::monthly_card_now_ms();
        let transaction = move |store: &mut AccountStore| {
            let request = &request_for_commit;
            let account = store
                .accounts
                .get(&request.purchase.account_id)
                .ok_or("gameShopGiftSenderMissing")?;
            let character = account
                .characters
                .iter()
                .find(|c| c.index == request.purchase.character_index)
                .ok_or("gameShopGiftSenderMissing")?;
            let durable = account
                .saves
                .get(&character.index)
                .ok_or("gameShopGiftSenderSaveMissing")?;
            if !same_character(character, &checkpoint.character)
                || !same_character(character, &durable.character)
            {
                return Err("gameShopGiftOwnerIdentityMismatch".into());
            }
            if let Some(outcome) = existing_outcome(&systems(durable)?, request)? {
                return Ok(Commit {
                    outcome,
                    sender_save: None,
                    charged_credit: 0,
                });
            }
            if durable.revision != expected_revision {
                return Err("gameShopGiftOwnerCheckpointStale".into());
            }
            let mut sender_save = checkpoint;
            merge_persisted_mail_into_character_save(&mut sender_save, durable)?;
            let mut sender_state = systems(&sender_save)?;
            validate_stage5_systems_item_carriers(&sender_state)?;
            let reject = |store: &mut AccountStore, save, state, target, code, stock| {
                finish_commit(
                    store,
                    save,
                    state,
                    request,
                    target,
                    0,
                    None,
                    Vec::new(),
                    failure(request, code, stock),
                )
            };
            let product = match product {
                Ok(product) => product,
                Err(code) => {
                    return reject(store, sender_save, sender_state, recipient, code, None)
                }
            };
            let Some(target) = recipient else {
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    None,
                    GameShopPurchaseFailure::RecipientUnavailable,
                    None,
                );
            };
            if target.account_id == request.purchase.account_id
                && target.character.index == request.purchase.character_index
            {
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    Some(target),
                    GameShopPurchaseFailure::SelfGiftUnavailable,
                    None,
                );
            }
            if unique_recipient(store, &request.recipient_name)
                .ok()
                .is_none_or(|current| {
                    current.account_id != target.account_id
                        || !same_character(&current.character, &target.character)
                })
            {
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    Some(target),
                    GameShopPurchaseFailure::RecipientUnavailable,
                    None,
                );
            }
            let target_account = store
                .accounts
                .get(&target.account_id)
                .ok_or("gameShopGiftRecipientMissing")?;
            let target_save = target_account.saves.get(&target.character.index);
            if target_account.active_ban(now_ms).is_some()
                || target_save
                    .is_none_or(|save| !same_character(&save.character, &target.character))
            {
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    Some(target),
                    GameShopPurchaseFailure::RecipientUnavailable,
                    None,
                );
            }
            let mut recipient_save = target_save.expect("verified recipient save").clone();
            let mut recipient_state = systems(&recipient_save)?;
            validate_stage5_systems_item_carriers(&recipient_state)?;
            if recipient_state
                .mail
                .iter()
                .filter(|mail| !mail.deleted)
                .count()
                >= MAIL_CAPACITY
            {
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    Some(target),
                    GameShopPurchaseFailure::MailFull,
                    None,
                );
            }
            let Some(remaining) = sender_save.credit.checked_sub(product.total) else {
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    Some(target),
                    GameShopPurchaseFailure::InsufficientCurrency,
                    None,
                );
            };
            let stock = product.item.stock;
            let purchases = if product.item.i_stock {
                sender_state
                    .game_shop_individual_purchases
                    .get(&product.item.g_index)
                    .copied()
                    .unwrap_or(0)
            } else {
                store
                    .game_shop_global_purchases
                    .get(&product.item.g_index)
                    .copied()
                    .unwrap_or(0)
            };
            if !game_shop_stock_available(stock, purchases, request.purchase.quantity) {
                let level = game_shop_stock_level(stock, purchases);
                return reject(
                    store,
                    sender_save,
                    sender_state,
                    Some(target),
                    GameShopPurchaseFailure::StockUnavailable,
                    Some(level),
                );
            }
            let mail_id = next_mail_id(&recipient_state)?;
            let nonce = new_stage5_mail_delivery_nonce();
            let mut attachments = product.attachments;
            let mut card_uids = Vec::new();
            if product.item.g_index == BILLING_MONTHLY_CARD_GAME_SHOP_INDEX {
                let receipt_id =
                    format!("gameshop-gift-{}", request.purchase.server_idempotency_key);
                let (items, replayed) = super::billing_monthly_card::issue_paid_items(
                    store
                        .accounts
                        .get_mut(&target.account_id)
                        .ok_or("gameShopGiftRecipientMissing")?,
                    &recipient_save,
                    &target.account_id,
                    target.character.index,
                    &receipt_id,
                    u16::from(request.purchase.quantity),
                    product.item.credit_price,
                    now_ms,
                    crate::monthly_card::MonthlyCardItemDelivery::Mail,
                )?;
                if replayed {
                    return Err("gameShopGiftItemReceiptMissingOutcome".into());
                }
                card_uids = items.iter().map(|item| item.unique_id).collect();
                attachments = items
                    .iter()
                    .map(serde_json::to_string)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "gameShopGiftItemInvalid")?;
            }
            recipient_state.mail.push(Stage5MailMessage {
                id: mail_id,
                delivery_nonce: nonce.clone(),
                from: "Gameshop".into(),
                to: target.character.name.clone(),
                subject: "Game shop gift".into(),
                body: format!(
                    "{} sent {} x {} from the game shop.",
                    sender_save.character.name, product.item_name, product.item_count
                ),
                gold: 0,
                items: vec![product.item_key; attachments.len()],
                item_states_json: attachments,
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });
            recipient_save.stage5_systems_json = Some(
                serde_json::to_string(&recipient_state)
                    .map_err(|_| "gameShopGiftMalformedSystems")?,
            );
            validate_character_save_record(&recipient_save)?;
            // This is external mail delivery, not a recipient owner checkpoint.
            // Keep its revision; ordinary owner save merges the durable mail.
            store
                .accounts
                .get_mut(&target.account_id)
                .ok_or("gameShopGiftRecipientMissing")?
                .saves
                .insert(target.character.index, recipient_save);
            let new_stock_level = if stock == 0 {
                None
            } else {
                let next = purchases
                    .checked_add(u64::from(request.purchase.quantity))
                    .ok_or("gameShopGiftStockOverflow")?;
                if product.item.i_stock {
                    sender_state
                        .game_shop_individual_purchases
                        .insert(product.item.g_index, next);
                } else {
                    store
                        .game_shop_global_purchases
                        .insert(product.item.g_index, next);
                }
                Some(game_shop_stock_level(stock, next))
            };
            sender_save.credit = remaining;
            let outcome = GameShopPurchaseOutcome {
                success: true,
                g_index: request.purchase.g_index,
                quantity: request.purchase.quantity,
                price_type: 0,
                new_stock_level,
                mail_id: Some(u64::from(mail_id)),
                failure: None,
            };
            finish_commit(
                store,
                sender_save,
                sender_state,
                request,
                Some(target),
                product.total,
                Some(nonce),
                card_uids,
                outcome,
            )
        };
        let result = if uses_global {
            config.commit_account_store_transaction_with_global(&accounts, transaction)
        } else {
            config.commit_account_store_transaction(&accounts, transaction)
        };
        let committed = match result {
            Ok(committed) => committed,
            Err(error)
                if retry_known_cas
                    && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth
                    && error == "gameShopGiftItemReceiptMissingOutcome" =>
            {
                // Payer and recipient refreshes are separate reads. A competing
                // atomic gift may commit between them, leaving a stale payer
                // receipt and an already issued recipient unit in this cache.
                // This callback error occurs before persistence. Recover only
                // the complete, matching durable payer receipt; never write a
                // replacement outcome or parcel from a recipient unit alone.
                config.refresh_account_store_account(&identity.account_id)?;
                let store = config.account_store.lock()
                    .map_err(|_| "gameShopGiftStoreUnavailable")?;
                let save = store.accounts.get(&identity.account_id)
                    .and_then(|account| account.saves.get(&identity.character_index))
                    .ok_or("gameShopGiftSenderMissing")?;
                if let Some(outcome) = existing_outcome(&systems(save)?, &request)? {
                    return Ok(GameShopPurchaseExecution { packets: Vec::new(), outcome });
                }
                return Err(error);
            }
            Err(error)
                if retry_known_cas
                    && !error.contains("OUTCOME_UNKNOWN")
                    && !error.contains("frozen")
                    && (error.starts_with("stale postgres account-store write")
                        || error.starts_with("stale postgres character-save write")
                        || error.starts_with("stale postgres game-shop global-stock write")) =>
            {
                for account in &accounts {
                    config.refresh_account_store_account(account)?;
                }
                if uses_global {
                    config.refresh_game_shop_global_stock()?;
                }
                return self.game_shop_gift_attempt(
                    request, false, RecipientSelection::Frozen(retry_recipient),
                );
            }
            Err(error) => return Err(error),
        };
        let Some(save) = committed.sender_save else {
            return Ok(GameShopPurchaseExecution {
                packets: Vec::new(),
                outcome: committed.outcome,
            });
        };
        if !self
            .app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(expected_revision, save.revision)
        {
            return Err(
                "ACCOUNT_STORE_OUTCOME_UNKNOWN: game-shop Gift owner publication failed".into(),
            );
        }
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .credit = save.credit;
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems = systems(&save)?;
        if let Some(code) = committed.outcome.failure {
            return Ok(self.game_shop_rejection_execution(
                request.purchase.g_index,
                request.purchase.quantity,
                request.purchase.price_type,
                code,
                committed.outcome.new_stock_level,
            ));
        }
        let mut packets = vec![ServerPacket::LoseCredit {
            credit: committed.charged_credit,
        }];
        if let Some(stock_level) = committed.outcome.new_stock_level {
            packets.push(ServerPacket::GameShopStock {
                g_index: request.purchase.g_index,
                stock_level,
            });
        }
        Ok(GameShopPurchaseExecution {
            packets,
            outcome: committed.outcome,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use mir2_protocol::{MirClass, MirGender};

    #[test]
    fn gift_name_accepts_only_crystal_ascii_and_bmp_chinese_range() {
        for name in [
            "abc",
            "A_9",
            "一二三四五六七八九十甲乙丙丁戊",
            "Gift\u{4e00}\u{9fa5}",
        ] {
            assert!(valid_name(name), "valid Crystal name {name:?}");
        }
        for name in [
            "ab",
            "abcdefghijklmnop",
            "a b",
            "a-b",
            "a.b",
            "abé",
            "ab😀",
            "ab\u{4dff}",
            "ab\u{9fa6}",
            " ab",
            "ab ",
            "ab\n",
        ] {
            assert!(!valid_name(name), "invalid Crystal name {name:?}");
        }
    }

    fn entry(key: u8) -> GiftEntry {
        GiftEntry {
            purchase: NativeGameShopPurchaseRequest {
                protocol_version: crate::NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
                server_idempotency_key: URL_SAFE_NO_PAD.encode([key; 32]),
                gateway_session_id: "gift-merge-test".into(),
                account_id: "payer".into(),
                character_index: 1,
                client_request_id: format!("request-{key}"),
                g_index: 2,
                quantity: 1,
                price_type: 0,
            },
            recipient_name: "Friend".into(),
            recipient: Some(Recipient {
                account_id: "receiver".into(),
                character: CharacterRecord {
                    index: 2,
                    name: "Friend".into(),
                    level: 10,
                    class: MirClass::Warrior,
                    gender: MirGender::Male,
                },
            }),
            charged_credit: 10,
            unit_credit_price: 10,
            delivery_nonce: Some(format!("gift-merge-{key}")),
            monthly_card_uids: Vec::new(),
            outcome: GameShopPurchaseOutcome {
                success: true,
                g_index: 2,
                quantity: 1,
                price_type: 0,
                new_stock_level: None,
                mail_id: Some(u64::from(key)),
                failure: None,
            },
        }
    }

    fn ledger_mail(entries: Vec<GiftEntry>) -> Stage5MailMessage {
        let mut state = Stage5SystemsState::default();
        for entry in entries {
            record_entry(&mut state, "Sender", entry).unwrap();
        }
        state.mail.pop().unwrap()
    }

    #[test]
    fn gift_ledger_union_is_canonical_and_does_not_erase_older_receipts() {
        let a = ledger_mail(vec![entry(1)]);
        let b = ledger_mail(vec![entry(2)]);
        let mut ab = a.clone();
        let mut ba = b.clone();
        assert_eq!(
            merge_game_shop_gift_ledger_mail(&mut ab, &b).unwrap(),
            Some(true)
        );
        assert_eq!(
            merge_game_shop_gift_ledger_mail(&mut ba, &a).unwrap(),
            Some(true)
        );
        assert_eq!(ab.body, ba.body);
        assert_eq!(decode_ledger(&ab).unwrap().entries.len(), 2);
        assert_eq!(
            merge_game_shop_gift_ledger_mail(&mut ab, &a).unwrap(),
            Some(false)
        );
        assert_eq!(decode_ledger(&ab).unwrap().entries.len(), 2);
    }

    #[test]
    fn gift_ledger_conflicts_and_duplicate_client_binding_fail_without_mutating_local() {
        let mut local = ledger_mail(vec![entry(1)]);
        let original = local.clone();
        let mut changed = entry(1);
        changed.charged_credit = 11;
        changed.unit_credit_price = 11;
        let external = ledger_mail(vec![changed]);
        assert!(merge_game_shop_gift_ledger_mail(&mut local, &external).is_err());
        assert_eq!(local, original);
        let mut collision = entry(2);
        collision.purchase.client_request_id = "request-1".into();
        let external = ledger_mail(vec![collision]);
        assert!(merge_game_shop_gift_ledger_mail(&mut local, &external).is_err());
        assert_eq!(local, original);
    }

    #[test]
    fn external_mail_merge_preserves_gift_union_and_rekeys_ordinary_collision() {
        let newer = ledger_mail(vec![entry(1), entry(2)]);
        let older = ledger_mail(vec![entry(1)]);
        let ordinary = Stage5MailMessage {
            id: newer.id,
            delivery_nonce: "gift-external-ordinary-mail".into(),
            from: "Friend".into(),
            to: "Sender".into(),
            subject: "Ordinary mail".into(),
            body: "No ledger fields".into(),
            gold: 0,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        };
        let external = vec![older, ordinary];
        let mut local = vec![newer.clone()];
        assert!(
            super::super::save::merge_external_stage5_mail(&mut local, external.clone()).unwrap()
        );
        assert_eq!(local.len(), 2);
        assert_eq!(local[0], newer);
        assert_eq!(decode_ledger(&local[0]).unwrap().entries.len(), 2);
        assert_ne!(local[0].id, local[1].id);
        assert_eq!(local[1].subject, "Ordinary mail");
        let after = local.clone();
        assert!(!super::super::save::merge_external_stage5_mail(&mut local, external).unwrap());
        assert_eq!(local, after);
    }

    #[test]
    fn external_mail_merge_conflict_does_not_publish_unrelated_earlier_mail() {
        let mut local = vec![ledger_mail(vec![entry(1)])];
        let before = local.clone();
        let mut changed = entry(1);
        changed.charged_credit = 11;
        changed.unit_credit_price = 11;
        let conflict = ledger_mail(vec![changed]);
        let mut ordinary = conflict.clone();
        ordinary.delivery_nonce = "gift-external-unrelated-before-conflict".into();
        ordinary.from = "Friend".into();
        ordinary.subject = "Ordinary mail".into();
        ordinary.body = "Unrelated delivery must also roll back".into();
        ordinary.opened = false;
        ordinary.locked = false;
        ordinary.claimed = false;
        ordinary.deleted = false;
        assert!(super::super::save::merge_external_stage5_mail(
            &mut local,
            vec![ordinary, conflict],
        )
        .is_err());
        assert_eq!(local, before);
    }
}
