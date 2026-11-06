//! Serial private-owner bridge. No raw player WorldCommand bypass exists.
use super::{
    items::{try_user_item_from_item_state, validate_committed_item_state_carrier, ItemState},
    npc_purchase_transaction::PreparedCheckpointMerge,
    resources::{
        is_in_world, InventoryResource, NpcStateResource, PlayerRuntimeResource,
        RuntimeConfigResource, SessionResource,
    },
    save::{
        merge_persisted_mail_into_character_save, snapshot_active_character_save,
        validate_character_save_record,
    },
    session::SimulationSession,
};
use crate::procurement::*;
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::{procurement as wire, ServerPacket};
use rand_core::{OsRng, RngCore};
const HOME: &str = "@procurement:v1:home";
fn authorization_deadline(now: u64, maximum_ms: u64, expires_at_ms: u64) -> Result<u64, String> {
    now.checked_add(maximum_ms)
        .map(|deadline| deadline.min(expires_at_ms))
        .ok_or_else(|| "procurementClockOverflow".into())
}
#[cfg(test)]
pub(crate) fn test_item(uid: u64) -> String {
    let t = mir2_game_data::crystal_item_by_index(245).unwrap();
    let mut i = super::items::embedded_item_state_from_template(&t, crate::ItemContainer::Bag1, 0);
    i.unique_id = uid;
    i.durability_current = Some(12345);
    i.added_stats = vec![mir2_protocol::UserItemStat { stat: 5, value: 7 }];
    let w = try_user_item_from_item_state(&i).unwrap();
    let i = super::items::try_item_state_from_user_item(i, &w).unwrap();
    serde_json::to_string(&i).unwrap()
}
#[cfg(test)]
pub(crate) fn test_eligible(json: &str, c: &wire::EquipmentConditions) -> Result<(), String> {
    let i: ItemState = serde_json::from_str(json).map_err(|e| e.to_string())?;
    eligible(&i, c)
}
pub(crate) fn custody_ids(json: &str) -> Result<std::collections::BTreeSet<u64>, String> {
    let item: ItemState = serde_json::from_str(json).map_err(|e| e.to_string())?;
    super::item_custody::item_ids(&item)
}
#[derive(Resource, Default)]
struct ProcurementAdmission {
    npc: u32,
}
#[derive(Resource, Default)]
struct ProcurementReserved(std::collections::BTreeSet<u64>);
pub(super) fn reserved_ids(world: &World) -> Result<std::collections::BTreeSet<u64>, String> {
    let Some(session) = world.get_resource::<SessionResource>() else {
        return Ok(Default::default());
    };
    let Some(account) = super::save::active_session_mutating_account_id(session) else {
        return Ok(Default::default());
    };
    let Some(runtime) = world.get_resource::<RuntimeConfigResource>() else {
        return Ok(Default::default());
    };
    let l = runtime
        .config
        .account_store
        .lock()
        .map_err(|_| "procurementStoreUnavailable")?
        .accounts
        .get(&account)
        .and_then(|a| a.procurement.clone())
        .unwrap_or_default();
    let mut next = std::collections::BTreeSet::new();
    for json in l
        .locks
        .values()
        .filter(|l| !l.released)
        .map(|l| &l.original_item_json)
        .chain(
            l.deliveries
                .values()
                .filter(|d| !d.claimed)
                .filter_map(|d| d.original_item_json.as_ref()),
        )
    {
        next.extend(custody_ids(json)?)
    }
    Ok(next)
}
fn sync_custody(world: &mut World) -> Result<(), String> {
    let next = reserved_ids(world)?;
    let previous = world
        .get_resource::<ProcurementReserved>()
        .map(|r| r.0.clone())
        .unwrap_or_default();
    let mut inv = world.resource_mut::<InventoryResource>();
    for id in previous {
        inv.reserved_item_unique_ids.remove(&id);
    }
    inv.reserved_item_unique_ids.extend(next.iter().copied());
    world.insert_resource(ProcurementReserved(next));
    Ok(())
}
fn fresh() -> Result<[u8; 32], String> {
    let mut b = [0; 32];
    OsRng
        .try_fill_bytes(&mut b)
        .map_err(|_| "procurementEntropyUnavailable")?;
    Ok(b)
}
pub(super) fn decorate_dialog(world: &World, dialog: &mut super::npc::ActiveNpcDialogState) {
    let Some(runtime) = world.get_resource::<RuntimeConfigResource>() else {
        return;
    };
    let Some(session) = world.get_resource::<SessionResource>() else {
        return;
    };
    let config = &runtime.config;
    let previous_customer =
        super::save::active_session_mutating_account_id(session).is_some_and(|id| {
            config
                .account_store
                .lock()
                .ok()
                .is_some_and(|s| s.accounts.get(&id).is_some_and(|a| a.procurement.is_some()))
        });
    if !previous_customer
        && !world
            .resource::<RuntimeConfigResource>()
            .config
            .procurement_config
            .enabled
    {
        return;
    }
    if !dialog.links.iter().any(|l| l.target == HOME) {
        dialog.links.insert(
            0,
            super::npc::NpcDialogLinkState {
                text: "Personal procurement".into(),
                target: HOME.into(),
            },
        );
    }
}
fn admitted(world: &World) -> bool {
    let Some(a) = world.get_resource::<ProcurementAdmission>() else {
        return false;
    };
    world
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .is_some_and(|d| d.npc_object_id == a.npc && d.links.iter().any(|l| l.target == HOME))
        && super::npc::crystal_npc_object_in_data_range(world, a.npc)
}
fn owner(world: &World) -> Result<(String, crate::CharacterSaveRecord), String> {
    if !is_in_world(world) {
        return Err("procurementInWorldRequired".into());
    }
    let account =
        super::save::active_session_mutating_account_id(world.resource::<SessionResource>())
            .ok_or("procurementAuthenticationRequired")?;
    let save = snapshot_active_character_save(world).ok_or("procurementCheckpointMissing")?;
    validate_character_save_record(&save)?;
    Ok((account, save))
}
fn find_order(config: &crate::SimulationConfig, id: &str) -> Result<(String, Order), String> {
    config.procurement_find_order(id)
}
fn eligible(item: &ItemState, conditions: &wire::EquipmentConditions) -> Result<(), String> {
    validate_committed_item_state_carrier(item).map_err(|_| "procurementInvalidInstance")?;
    let w = try_user_item_from_item_state(item).map_err(|_| "procurementInvalidInstance")?;
    if item.quantity != 1
        || item.unique_id == 0
        || w.item_index != 245
        || w.soul_bound_id >= 0
        || w.expire_info.is_some()
        || w.sealed_info.is_some()
        || w.rental_information.is_some()
        || w.is_shop_item
        || !matches!(
            item.container,
            crate::ItemContainer::Bag1 | crate::ItemContainer::Bag2
        )
        || !super::social_economy::stage5_trade_item_can_enter(item)
        || item.durability_current.unwrap_or(0) < conditions.min_current_durability
        || item.durability_max.unwrap_or(0) < conditions.min_max_durability
        || conditions.min_added_stats.iter().any(|c| {
            w.added_stats
                .iter()
                .find(|s| s.stat == c.stat)
                .map_or(0, |s| s.value)
                < c.value
        })
    {
        return Err("procurementEquipmentRejected".into());
    }
    Ok(())
}
/// Bounded single-line preview of the actual validated carrier, never a newly
/// generated template item. Full attributes remain in the original instance.
fn delivery_item_description(
    world: &World,
    uid: u64,
    conditions: &wire::EquipmentConditions,
) -> Result<String, String> {
    let inventory = world.resource::<InventoryResource>();
    let item = inventory
        .inventory_items
        .iter()
        .find(|item| item.unique_id == uid)
        .ok_or("procurementItemMissing")?;
    eligible(item, conditions)?;
    let item_wire = try_user_item_from_item_state(item).map_err(|e| e.to_string())?;
    let mut description = format!(
        "T{} UID={} Dur={}/{} Ref={} Add={} Stats=",
        item_wire.item_index,
        item.unique_id,
        item_wire.current_dura,
        item_wire.max_dura,
        item_wire.refined_value,
        item_wire.refine_added
    );
    let total = item_wire.added_stats.len();
    if total == 0 {
        description.push_str("none");
    }
    let mut shown = 0;
    for stat in &item_wire.added_stats {
        let label = mir2_protocol::crystal_stat_label(stat.stat);
        let label = if label.len() > 8 {
            format!("S{}", stat.stat)
        } else {
            label
        };
        let entry = format!(
            "{}{label}{:+}",
            if shown == 0 { "" } else { "," },
            stat.value
        );
        let remaining = total - shown - 1;
        let suffix_len = if remaining == 0 {
            0
        } else {
            format!(" +{remaining}more").len()
        };
        if description.len() + entry.len() + suffix_len > 96 {
            break;
        }
        description.push_str(&entry);
        shown += 1;
    }
    if shown < total {
        description.push_str(&format!(" +{}more", total - shown));
    }
    debug_assert!(description.is_ascii() && description.len() <= 96);
    Ok(description)
}
fn fee(config: &ProcurementConfig) -> wire::ServiceFeeQuote {
    wire::ServiceFeeQuote {
        rule_version: config.rule.version.to_string(),
        quota_units: config.rule.fee_units,
        // This is a protocol discriminator, not a UI explanation. Production
        // credit provenance remains a separate gate; do not label it as Test.
        quota_source: if config.allow_test_credits {
            "test"
        } else {
            "paid"
        }
        .into(),
        credit_cost: 0,
        refund_on_cancel: config.rule.cancel_fee_refundable,
        refund_on_expiry: config.rule.expiry_fee_refundable,
        quota_valid_until_ms: None,
        production_billing_enabled: false,
    }
}
fn credit_source_wire(source: CreditSource) -> &'static str {
    match source {
        CreditSource::Test => "test",
        CreditSource::Paid => "paid",
        CreditSource::Gift => "gift",
        CreditSource::Unknown => "unknown",
    }
}
fn publication_config(
    config: &ProcurementConfig,
    quote: &StoredQuote,
) -> Result<ProcurementConfig, String> {
    let rule = quote
        .rule
        .as_ref()
        .ok_or("procurementQuoteRuleSnapshotMissing")?;
    rule.validate()?;
    if quote.intent.rule_version != rule.version
        || quote.public.service_fee.rule_version != rule.version.to_string()
        || quote.public.service_fee.quota_units != rule.fee_units
        || quote.public.service_fee.refund_on_cancel != rule.cancel_fee_refundable
        || quote.public.service_fee.refund_on_expiry != rule.expiry_fee_refundable
    {
        return Err("procurementQuoteRuleMismatch".into());
    }
    let mut frozen = config.clone();
    frozen.rule = rule.clone();
    Ok(frozen)
}
fn order_view(o: &Order, ledger: Option<&ProcurementLedger>) -> wire::ProcurementOrder {
    // These are stable wire values consumed by native action gates. Debug
    // formatting domain enums here silently disables real client actions.
    let chain_state = match o.status {
        OrderStatus::Publishing => "pendingPublish",
        OrderStatus::PublishFailed => "publishFailed",
        OrderStatus::Open => "open",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Expired => "expired",
    };
    let delivery = ledger.and_then(|l| {
        l.deliveries.values().find(|d| {
            d.order_id == o.id
                && d.character_index == o.buyer_character
                && match o.status {
                    OrderStatus::Filled => d.original_item_json.is_some(),
                    OrderStatus::PublishFailed | OrderStatus::Cancelled | OrderStatus::Expired => {
                        d.original_item_json.is_none() && d.gold > 0
                    }
                    _ => false,
                }
        })
    });
    let delivery_state = match o.status {
        OrderStatus::Publishing | OrderStatus::Open => "escrowed",
        OrderStatus::Filled => match delivery {
            Some(d) if d.claimed => "delivered",
            Some(_) => "pendingClaim",
            None => "pending",
        },
        OrderStatus::PublishFailed | OrderStatus::Cancelled | OrderStatus::Expired => {
            match delivery {
                Some(d) if d.claimed => "refunded",
                Some(_) => "pendingClaim",
                None => "pending",
            }
        }
    };
    wire::ProcurementOrder {
        order_id: format!("0x{}", o.id),
        buyer_ref: wire::encode_hex(&o.publish.intent.buyer_ref),
        seller_ref: None,
        template_id: 245,
        price_gold: o.publish.intent.price_gold.to_string(),
        expires_at_ms: o.publish.intent.expires_at_ms.to_string(),
        rule_version: o.rule.version.to_string(),
        conditions: o.conditions.clone(),
        chain_state: chain_state.into(),
        delivery_state: delivery_state.into(),
        fill_id: o.winning_fill.as_ref().map(|x| format!("0x{x}")),
        chain_transaction: o.last_chain_transaction.clone(),
    }
}
fn view(
    config: &crate::SimulationConfig,
    account: &str,
    save: &crate::CharacterSaveRecord,
    now: u64,
) -> Result<wire::ProcurementView, String> {
    let c = &config.procurement_config;
    let l = config.procurement_view(account)?;
    let mut v = wire::ProcurementView {
        enabled: c.enabled,
        test_mode: c.allow_test_credits,
        production_billing_enabled: false,
        actor_ref: wire::encode_hex(&actor_ref(account, save)),
        network_hash: wire::encode_hex(&c.domain.network_hash),
        package_id: wire::encode_hex(&c.domain.package_id),
        dapp_storage_id: wire::encode_hex(&c.domain.dapp_storage_id),
        available_gold: save.gold.to_string(),
        escrow_gold: l
            .orders
            .values()
            .map(|o| o.escrow_gold as u64)
            .sum::<u64>()
            .to_string(),
        fee_rule: fee(c),
        ..Default::default()
    };
    for b in l.credits.values() {
        v.reserved_quota = v
            .reserved_quota
            .checked_add(b.reserved)
            .ok_or("procurementCreditOverflow")?;
        if now < b.expires_at_ms {
            let n = b.granted - b.reserved - b.consumed;
            match b.source {
                CreditSource::Test => {
                    v.test_quota = v
                        .test_quota
                        .checked_add(n)
                        .ok_or("procurementCreditOverflow")?
                }
                CreditSource::Paid => {
                    v.paid_quota = v
                        .paid_quota
                        .checked_add(n)
                        .ok_or("procurementCreditOverflow")?
                }
                _ => {}
            }
        }
    }
    if test_quota_sale_enabled(c) {
        let package_id = test_quota_package_id(&c.rule);
        v.quota_packages.push(wire::QuotaPackage {
            id: package_id.clone(),
            label: "Free Test x10 (once/account/version)".into(),
            units: test_quota_package_units(&c.rule)?,
            credit_cost: 0,
            source: "test".into(),
            enabled: !l.quota_purchases.contains_key(&package_id),
        });
    }
    v.my_orders = l
        .orders
        .values()
        .filter(|o| o.buyer_character == save.character.index)
        .take(200)
        .map(|o| order_view(o, Some(&l)))
        .collect();
    v.public_orders = config
        .procurement_public_orders(now)?
        .iter()
        .take(200)
        .map(|o| order_view(o, None))
        .collect();
    v.deliveries = l
        .deliveries
        .values()
        .filter(|d| d.character_index == save.character.index)
        .take(200)
        .map(|d| wire::ProcurementDelivery {
            delivery_id: d.id.clone(),
            order_id: format!("0x{}", d.order_id),
            item_uid: d.item_uid.unwrap_or(0).to_string(),
            template_id: if d.original_item_json.is_some() {
                245
            } else {
                0
            },
            description: if d.gold > 0 {
                format!("{} gold", d.gold)
            } else {
                "Original equipment instance".into()
            },
            state: if d.claimed {
                "delivered"
            } else {
                "pendingClaim"
            }
            .into(),
        })
        .collect();
    v.fee_history = l
        .orders
        .values()
        .take(200)
        .map(|o| wire::ServiceFeeEntry {
            order_id: format!("0x{}", o.id),
            rule_version: o.rule.version.to_string(),
            source: credit_source_wire(l.credits[&o.fee_batch].source).into(),
            units: o.rule.fee_units,
            state: if o.fee_refunded {
                "refunded"
            } else if o.fee_consumed {
                "consumed"
            } else {
                "reserved"
            }
            .into(),
            credit_cost: 0,
        })
        .collect();
    for receipt in l
        .quota_purchases
        .values()
        .take(200usize.saturating_sub(v.fee_history.len()))
    {
        v.fee_history.push(wire::ServiceFeeEntry {
            order_id: format!("quota:{}", receipt.package_id),
            rule_version: receipt.rule.version.to_string(),
            source: "test".into(),
            units: receipt.units,
            state: "testGrantedNoCash".into(),
            credit_cost: 0,
        });
    }
    Ok(v)
}
impl SimulationSession {
    pub fn execute_procurement_owner(
        &mut self,
        authenticated: bool,
        request: &wire::ProcurementRequest,
        now: u64,
    ) -> Result<ProcurementExecution, String> {
        self.execute_procurement_owner_inner(authenticated, request, now)
            .map_err(|e| {
                if e.contains("COMMIT_OUTCOME_UNKNOWN") || e.contains("PublicationOutcomeUnknown") {
                    format!("{COMMIT_UNKNOWN}: {e}")
                } else {
                    e
                }
            })
    }
    fn execute_procurement_owner_inner(
        &mut self,
        authenticated: bool,
        request: &wire::ProcurementRequest,
        now: u64,
    ) -> Result<ProcurementExecution, String> {
        if !authenticated
            || request.protocol_version != 1
            || request.request_id.is_empty()
            || request.request_id.len() > 128
        {
            return Err("procurementRequestInvalid".into());
        }
        let (account, save) = owner(self.app.world())?;
        if save.character.index != request.character_index {
            return Err("procurementCharacterMismatch".into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        require_source(&config)?;
        config.refresh_account_store_account(&account)?;
        sync_custody(self.app.world_mut())?;
        let mut reply = wire::ProcurementReply {
            protocol_version: 1,
            request_id: request.request_id.clone(),
            character_index: request.character_index,
            status: None,
            quote: None,
            error: None,
            replayed: false,
        };
        let mut packets = vec![];
        match &request.action {
            wire::ProcurementAction::Status { npc_object_id } => {
                let n = self.app.world().resource::<NpcStateResource>();
                let d = n
                    .active_npc_dialog
                    .as_ref()
                    .ok_or("procurementNpcRequired")?;
                if d.npc_object_id != *npc_object_id
                    || !d.links.iter().any(|l| l.target == HOME)
                    || !super::npc::crystal_npc_object_in_data_range(
                        self.app.world(),
                        *npc_object_id,
                    )
                {
                    return Err("procurementNpcRequired".into());
                }
                self.app.world_mut().insert_resource(ProcurementAdmission {
                    npc: *npc_object_id,
                });
            }
            wire::ProcurementAction::List { .. } | wire::ProcurementAction::Reconcile { .. } => {}
            wire::ProcurementAction::QuoteBuyQuota { package_id } => {
                if !admitted(self.app.world()) {
                    return Err("procurementNpcRequired".into());
                }
                reply.quote = Some(config.commit_account_store_transaction(
                    &[account.clone()],
                    |s| {
                        quote_test_quota(
                            s,
                            &config.procurement_config,
                            &account,
                            &save,
                            &request.request_id,
                            package_id,
                            now,
                        )
                    },
                )?);
            }
            wire::ProcurementAction::QuotePublish {
                template_id,
                price_gold,
                expires_at_ms,
                conditions,
                purpose_public_key,
            } => {
                if !admitted(self.app.world()) || !config.procurement_config.enabled {
                    return Err("procurementNpcRequired".into());
                }
                if *template_id != 245 {
                    return Err("procurementTemplateUnsupported".into());
                }
                let key = wire::hex32(purpose_public_key).map_err(str::to_string)?;
                let i = IntentV1 {
                    network_hash: config.procurement_config.domain.network_hash,
                    package_id: config.procurement_config.domain.package_id,
                    dapp_storage_id: config.procurement_config.domain.dapp_storage_id,
                    operation: 1,
                    order_id: fresh()?,
                    fill_id: [0; 32],
                    buyer_ref: actor_ref(&account, &save),
                    seller_ref: [0; 32],
                    template_id: 245,
                    conditions_hash: digest(&conditions.canonical_bcs().map_err(str::to_string)?),
                    quantity: 1,
                    price_gold: wire::decimal(price_gold).map_err(str::to_string)?,
                    expires_at_ms: wire::decimal(expires_at_ms).map_err(str::to_string)?,
                    rule_version: config.procurement_config.rule.version,
                    nonce: fresh()?,
                    valid_until_ms: authorization_deadline(
                        now,
                        config.procurement_config.rule.maximum_authorization_ms,
                        wire::decimal(expires_at_ms).map_err(str::to_string)?,
                    )?,
                    actor_public_key: key,
                };
                reply.quote = Some(self.procurement_store_quote(
                    &config,
                    &account,
                    &save,
                    &request.request_id,
                    i,
                    conditions.clone(),
                    None,
                    None,
                    now,
                )?);
            }
            wire::ProcurementAction::QuoteDeliver {
                order_id,
                purpose_public_key,
                ..
            }
            | wire::ProcurementAction::QuoteCancel {
                order_id,
                purpose_public_key,
            } => {
                if !admitted(self.app.world()) {
                    return Err("procurementNpcRequired".into());
                }
                let (buyer, o) = find_order(&config, order_id)?;
                let mut i = o.publish.intent.clone();
                i.operation = if matches!(
                    &request.action,
                    wire::ProcurementAction::QuoteDeliver { .. }
                ) {
                    2
                } else {
                    3
                };
                if i.operation == 3 && buyer != account {
                    return Err("procurementNotBuyer".into());
                }
                i.actor_public_key = wire::hex32(purpose_public_key).map_err(str::to_string)?;
                i.nonce = fresh()?;
                i.valid_until_ms =
                    authorization_deadline(now, o.rule.maximum_authorization_ms, i.expires_at_ms)?;
                let uid = if let wire::ProcurementAction::QuoteDeliver { item_uid, .. } =
                    &request.action
                {
                    Some(wire::decimal(item_uid).map_err(str::to_string)?)
                } else {
                    None
                };
                if let Some(uid) = uid {
                    i.fill_id = fresh()?;
                    i.seller_ref = actor_ref(&account, &save);
                    let inv = self.app.world().resource::<InventoryResource>();
                    let item = inv
                        .inventory_items
                        .iter()
                        .find(|x| x.unique_id == uid)
                        .ok_or("procurementItemMissing")?;
                    eligible(item, &o.conditions)?;
                    if super::packets::stage5_trade_reserves_item(self.app.world(), item) {
                        return Err("procurementItemAlreadyLocked".into());
                    }
                }
                reply.quote = Some(self.procurement_store_quote(
                    &config,
                    &account,
                    &save,
                    &request.request_id,
                    i,
                    o.conditions,
                    uid,
                    Some(buyer),
                    now,
                )?);
            }
            wire::ProcurementAction::Confirm {
                quote_id,
                signature,
            } => {
                let ledger = config.procurement_view(&account)?;
                if let Some(q) = ledger.quota_quotes.get(quote_id) {
                    if !signature.is_empty() {
                        return Err("procurementQuotaSignatureUnexpected".into());
                    }
                    if !ledger.quota_purchases.contains_key(&q.package_id)
                        && !admitted(self.app.world())
                    {
                        return Err("procurementNpcRequired".into());
                    }
                    reply.replayed =
                        config.commit_account_store_transaction(&[account.clone()], |s| {
                            let durable = s
                                .accounts
                                .get(&account)
                                .and_then(|a| a.saves.get(&save.character.index))
                                .ok_or("procurementCheckpointMissing")?;
                            if durable.revision != save.revision {
                                return Err("procurementOwnerCheckpointStale".into());
                            }
                            confirm_test_quota(
                                s,
                                &config.procurement_config,
                                &account,
                                &save,
                                quote_id,
                                now,
                            )
                        })?;
                    reply.status = Some(view(&config, &account, &save, now)?);
                    return Ok(ProcurementExecution { packets, reply });
                }
                let q = config
                    .procurement_view(&account)?
                    .quotes
                    .get(quote_id)
                    .cloned()
                    .ok_or("procurementQuoteMissing")?;
                if q.character_index != save.character.index
                    || q.character_name != save.character.name
                {
                    return Err("procurementQuoteOwnerMismatch".into());
                }
                let signed = SignedIntent {
                    intent: q.intent.clone(),
                    signature: wire::decode_hex(signature).map_err(str::to_string)?,
                };
                signed.verify()?;
                if let Some(previous) = config
                    .procurement_view(&account)?
                    .requests
                    .get(&q.request_id)
                {
                    if previous != &hex(&signed.intent.hash()) {
                        return Err("procurementIdempotencyConflict".into());
                    }
                    reply.replayed = true;
                    reply.status = Some(view(&config, &account, &save, now)?);
                    return Ok(ProcurementExecution { packets, reply });
                }
                if !admitted(self.app.world()) {
                    return Err("procurementNpcRequired".into());
                }
                let mut staged = save.clone();
                let mut inv = self.app.world().resource::<InventoryResource>().clone();
                let mut removed = None;
                if q.intent.operation == 2 {
                    let uid = q.item_uid.ok_or("procurementItemMissing")?;
                    let index = inv
                        .inventory_items
                        .iter()
                        .position(|x| x.unique_id == uid)
                        .ok_or("procurementItemMissing")?;
                    let item = &inv.inventory_items[index];
                    eligible(item, &q.public.conditions)?;
                    if super::packets::stage5_trade_reserves_item(self.app.world(), item)
                        || !super::item_custody::can_take(&inv, item)
                    {
                        return Err("procurementItemAlreadyLocked".into());
                    }
                    removed = Some((serde_json::to_string(item).map_err(|e| e.to_string())?, uid));
                    inv.inventory_items.remove(index);
                    staged.inventory_items_json =
                        super::save::encode_state_vec(&inv.inventory_items);
                }
                let mut accounts = vec![account.clone()];
                if q.buyer_account != account {
                    accounts.push(q.buyer_account.clone());
                    config.refresh_account_store_account(&q.buyer_account)?;
                }
                let c = if q.intent.operation == 1 {
                    publication_config(&config.procurement_config, &q)?
                } else {
                    config.procurement_config.clone()
                };
                let (replayed, committed) = super::shared_guild_experience::commit_source(
                    self.app.world(),
                    &config,
                    &accounts,
                    |s| {
                        let durable = s
                            .accounts
                            .get(&account)
                            .and_then(|a| a.saves.get(&save.character.index))
                            .ok_or("procurementCheckpointMissing")?;
                        if durable.revision != save.revision {
                            return Err("procurementOwnerCheckpointStale".into());
                        }
                        merge_persisted_mail_into_character_save(&mut staged, durable)?;
                        let replayed = match q.intent.operation {
                            1 => publish(
                                s,
                                &c,
                                &account,
                                &mut staged,
                                &q.request_id,
                                &signed,
                                q.public.conditions.clone(),
                                now,
                            )?,
                            2 => {
                                let (json, uid) = removed.clone().unwrap();
                                offer(
                                    s,
                                    &c,
                                    &account,
                                    &q.buyer_account,
                                    &staged,
                                    &q.request_id,
                                    &signed,
                                    json,
                                    uid,
                                    now,
                                )?
                            }
                            3 => cancel(s, &c, &account, &staged, &q.request_id, &signed, now)?,
                            _ => return Err("procurementOperationInvalid".into()),
                        };
                        if !replayed {
                            staged.revision = save
                                .revision
                                .checked_add(1)
                                .ok_or("procurementRevisionOverflow")?;
                            validate_character_save_record(&staged)?;
                            s.accounts
                                .get_mut(&account)
                                .unwrap()
                                .saves
                                .insert(save.character.index, staged.clone());
                        }
                        Ok((replayed, staged.clone()))
                    },
                )?;
                reply.replayed = replayed;
                if !replayed {
                    self.procurement_publish_live(&save, &committed)?;
                    if q.intent.operation == 1 {
                        packets.push(ServerPacket::LoseGold {
                            gold: u32::try_from(q.intent.price_gold)
                                .map_err(|_| "procurementGoldOverflow")?,
                        })
                    }
                    if let Some((_, id)) = removed {
                        packets.push(ServerPacket::DeleteItem {
                            unique_id: id,
                            count: 1,
                        })
                    }
                }
            }
            wire::ProcurementAction::Claim { delivery_id } => {
                packets = self.procurement_claim_delivery(&account, delivery_id)?;
            }
        }
        let (_, current) = owner(self.app.world())?;
        reply.status = Some(view(&config, &account, &current, now)?);
        Ok(ProcurementExecution { packets, reply })
    }
    fn procurement_store_quote(
        &mut self,
        config: &crate::SimulationConfig,
        account: &str,
        save: &crate::CharacterSaveRecord,
        request: &str,
        i: IntentV1,
        conditions: wire::EquipmentConditions,
        uid: Option<u64>,
        buyer: Option<String>,
        now: u64,
    ) -> Result<wire::ProcurementQuote, String> {
        if now >= i.expires_at_ms
            || now >= i.valid_until_ms
            || i.valid_until_ms > i.expires_at_ms
            || i.price_gold == 0
            || i.price_gold > u32::MAX as u64
        {
            return Err("procurementQuoteInvalid".into());
        }
        let id = hex(&i.hash());
        let item_description = uid
            .map(|uid| delivery_item_description(self.app.world(), uid, &conditions))
            .transpose()?;
        let mut public = wire::ProcurementQuote {
            quote_id: id.clone(),
            operation: match i.operation {
                1 => "publish",
                2 => "deliver",
                _ => "cancel",
            }
            .into(),
            intent: Some(i.to_wire()),
            canonical_intent_hex: Some(wire::encode_hex(&i.bcs_bytes())),
            conditions,
            item_uid: uid.map(|u| u.to_string()),
            item_description,
            service_fee: fee(&config.procurement_config),
            quota_package_id: None,
        };
        let mut quote_rule = config.procurement_config.rule.clone();
        if i.operation != 1 {
            let (_, original) = find_order(config, &hex(&i.order_id))?;
            quote_rule = original.rule.clone();
            public.service_fee.rule_version = i.rule_version.to_string();
            public.service_fee.refund_on_cancel = original.rule.cancel_fee_refundable;
            public.service_fee.refund_on_expiry = original.rule.expiry_fee_refundable;
            public.service_fee.quota_units = 0;
            public.service_fee.credit_cost = 0;
            public.service_fee.quota_source = "none".into();
        }
        let q = StoredQuote {
            public: public.clone(),
            intent: i.clone(),
            buyer_account: buyer.unwrap_or_else(|| account.into()),
            item_uid: uid,
            character_index: save.character.index,
            character_name: save.character.name.clone(),
            request_id: request.into(),
            rule: Some(quote_rule),
        };
        config.commit_account_store_transaction(&[account.into()], |s| {
            let a = s
                .accounts
                .get_mut(account)
                .ok_or("procurementAccountMissing")?;
            a.billing
                .get_or_insert_with(crate::billing::BillingLedger::default);
            let l = a.procurement.get_or_insert_with(ProcurementLedger::default);
            if i.operation == 1 {
                check_rule_version(l, &config.procurement_config.rule)?;
            }
            l.quotes.retain(|_, q| q.intent.valid_until_ms > now);
            l.bindings.retain(|_, b| b.valid_until_ms > now);
            if l.quotes.len() >= 128 {
                return Err("procurementQuoteLimit".into());
            }
            l.bindings.insert(
                hex(&i.actor_public_key),
                ActorBinding {
                    character_index: save.character.index,
                    character_name: save.character.name.clone(),
                    public_key: i.actor_public_key,
                    valid_until_ms: i.valid_until_ms,
                },
            );
            l.quotes.insert(id, q);
            Ok(())
        })?;
        Ok(public)
    }
    fn procurement_publish_live(
        &mut self,
        before: &crate::CharacterSaveRecord,
        after: &crate::CharacterSaveRecord,
    ) -> Result<(), String> {
        let publication = PreparedCheckpointMerge::new(self.app.world(), after)
            .map_err(|e| format!("{COMMIT_UNKNOWN}: {e}"))?;
        let items = after
            .inventory_items_json
            .iter()
            .map(|x| serde_json::from_str::<ItemState>(x).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        if !self
            .app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(before.revision, after.revision)
        {
            return Err(format!(
                "{COMMIT_UNKNOWN}: live owner revision publication failed"
            ));
        }
        publication.apply(self.app.world_mut());
        self.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items = items;
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = after.gold;
        sync_custody(self.app.world_mut())
            .map_err(|e| format!("{COMMIT_UNKNOWN}: custody publication: {e}"))?;
        Ok(())
    }
    fn procurement_claim_delivery(
        &mut self,
        account: &str,
        id: &str,
    ) -> Result<Vec<ServerPacket>, String> {
        sync_custody(self.app.world_mut())?;
        let (_, before) = owner(self.app.world())?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let mut save = before.clone();
        let mut inv = self.app.world().resource::<InventoryResource>().clone();
        let (after, delivery, item) = super::shared_guild_experience::commit_source(
            self.app.world(),
            &config,
            &[account.into()],
            |s| {
                let a = s
                    .accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?;
                let durable = a
                    .saves
                    .get(&before.character.index)
                    .ok_or("procurementCheckpointMissing")?;
                if durable.revision != before.revision {
                    return Err("procurementOwnerCheckpointStale".into());
                }
                merge_persisted_mail_into_character_save(&mut save, durable)?;
                let l = a.procurement.as_mut().ok_or("procurementDeliveryMissing")?;
                let d = l
                    .deliveries
                    .get_mut(id)
                    .ok_or("procurementDeliveryMissing")?;
                if d.character_index != before.character.index {
                    return Err("procurementDeliveryOwnerMismatch".into());
                }
                if d.claimed {
                    return Ok((before.clone(), d.clone(), None));
                }
                save.gold = save
                    .gold
                    .checked_add(d.gold)
                    .ok_or("procurementGoldCapacityPending")?;
                let item = if let Some(json) = &d.original_item_json {
                    let item: ItemState = serde_json::from_str(json).map_err(|e| e.to_string())?;
                    let (planned, changed) =
                        super::item_custody::plan_return(&inv, &item, None, false)
                            .ok_or("procurementInventoryFullPending")?;
                    inv = planned;
                    save.inventory_items_json = super::save::encode_state_vec(&inv.inventory_items);
                    changed.into_iter().next()
                } else {
                    None
                };
                save.revision = save
                    .revision
                    .checked_add(1)
                    .ok_or("procurementRevisionOverflow")?;
                validate_character_save_record(&save)?;
                d.claimed = true;
                d.claim_revision = Some(save.revision);
                let d = d.clone();
                a.saves.insert(before.character.index, save.clone());
                Ok((save.clone(), d, item))
            },
        )?;
        if after.revision == before.revision {
            return Ok(vec![]);
        }
        self.procurement_publish_live(&before, &after)?;
        let mut packets = vec![];
        if delivery.gold > 0 {
            packets.push(ServerPacket::GainedGold {
                gold: delivery.gold,
            })
        }
        if let Some(item) = item {
            packets.push(ServerPacket::GainedItem {
                item: try_user_item_from_item_state(&item).map_err(|e| e.to_string())?,
            })
        }
        Ok(packets)
    }
    pub fn consume_pending_procurement(&mut self) -> Result<Vec<ServerPacket>, String> {
        let Ok((account, save)) = owner(self.app.world()) else {
            return Ok(vec![]);
        };
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if !config
            .account_store
            .lock()
            .map_err(|_| "procurementStoreUnavailable")?
            .accounts
            .get(&account)
            .is_some_and(|a| a.procurement.is_some())
        {
            return Ok(vec![]);
        }
        let l = config.procurement_view(&account)?;
        let ids = l
            .deliveries
            .values()
            .filter(|d| {
                !d.claimed
                    && d.original_item_json.is_none()
                    && d.character_index == save.character.index
            })
            .map(|d| d.id.clone())
            .take(8)
            .collect::<Vec<_>>();
        let mut packets = vec![];
        for id in ids {
            match self.procurement_claim_delivery(&account, &id) {
                Ok(p) => packets.extend(p),
                Err(e) if e == "procurementGoldCapacityPending" => {}
                Err(e) => return Err(e),
            }
        }
        Ok(packets)
    }
}

#[cfg(test)]
mod owner_tests {
    use super::*;
    #[test]
    fn procurement_owner_delivery_description_reads_verified_instance_values() {
        let (_config, mut s) = started("actual-delivery-description");
        let uid = 9_000_001;
        let item: ItemState = serde_json::from_str(&test_item(uid)).unwrap();
        let mut item_wire = try_user_item_from_item_state(&item).unwrap();
        item_wire.current_dura = 12345;
        item_wire.max_dura = 34000;
        item_wire.refined_value = 4;
        item_wire.refine_added = 3;
        item_wire.added_stats = vec![mir2_protocol::UserItemStat { stat: 5, value: 7 }];
        let item = super::super::items::try_item_state_from_user_item(item, &item_wire).unwrap();
        let before = serde_json::to_string(&item).unwrap();
        s.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
        let description =
            delivery_item_description(s.app.world(), uid, &Default::default()).unwrap();
        assert_eq!(
            description,
            "T245 UID=9000001 Dur=12345/34000 Ref=4 Add=3 Stats=MaxDC+7"
        );
        let actual = &s
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items;
        assert_eq!(
            serde_json::to_string(actual.last().unwrap()).unwrap(),
            before
        );
        assert!(delivery_item_description(s.app.world(), uid + 1, &Default::default()).is_err());
        s.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .last_mut()
            .unwrap()
            .soul_bound_id = Some(0);
        assert!(delivery_item_description(s.app.world(), uid, &Default::default()).is_err());
    }
    #[test]
    fn procurement_owner_delivery_description_is_bounded_and_discloses_omitted_stats() {
        let (_config, mut s) = started("bounded-delivery-description");
        let uid = u64::MAX;
        let item: ItemState = serde_json::from_str(&test_item(uid)).unwrap();
        let mut item_wire = try_user_item_from_item_state(&item).unwrap();
        item_wire.current_dura = u16::MAX;
        item_wire.max_dura = u16::MAX;
        item_wire.refined_value = u8::MAX;
        item_wire.refine_added = u8::MAX;
        item_wire.added_stats = (0..=u8::MAX)
            .map(|stat| mir2_protocol::UserItemStat {
                stat,
                value: if stat % 2 == 0 { i32::MAX } else { i32::MIN },
            })
            .collect();
        let item = super::super::items::try_item_state_from_user_item(item, &item_wire).unwrap();
        s.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
        let description =
            delivery_item_description(s.app.world(), uid, &Default::default()).unwrap();
        assert!(
            description.is_ascii() && description.len() <= 96,
            "{description}"
        );
        assert!(description.contains(&uid.to_string()));
        assert!(description.contains("Dur=65535/65535 Ref=255 Add=255"));
        assert!(description.contains("2147483647"));
        assert!(description.ends_with("more"));
    }
    #[test]
    fn procurement_owner_publication_quote_freezes_complete_rule_until_confirmed() {
        let (config, mut s) = started("quote-freeze");
        let save = s.active_character_checkpoint().unwrap();
        let mut original = config.clone();
        original.procurement_config.enabled = true;
        original.procurement_config.rule.fee_units = 2;
        original.procurement_config.rule.cancel_fee_refundable = true;
        let i = IntentV1 {
            network_hash: [1; 32],
            package_id: [2; 32],
            dapp_storage_id: [3; 32],
            operation: 1,
            order_id: [4; 32],
            fill_id: [0; 32],
            buyer_ref: actor_ref("demo", &save),
            seller_ref: [0; 32],
            template_id: 245,
            conditions_hash: digest(
                &wire::EquipmentConditions::default()
                    .canonical_bcs()
                    .unwrap(),
            ),
            quantity: 1,
            price_gold: 500000,
            expires_at_ms: 90000,
            rule_version: 1,
            nonce: [5; 32],
            valid_until_ms: 50000,
            actor_public_key: [6; 32],
        };
        let public = s
            .procurement_store_quote(
                &original,
                "demo",
                &save,
                "quote-freeze",
                i,
                Default::default(),
                None,
                None,
                1000,
            )
            .unwrap();
        let stored = original.procurement_view("demo").unwrap().quotes[&public.quote_id].clone();
        let mut changed = original.procurement_config.clone();
        changed.rule.version = 2;
        changed.rule.fee_units = 99;
        changed.rule.cancel_fee_refundable = false;
        let frozen = publication_config(&changed, &stored).unwrap();
        assert_eq!(frozen.rule, original.procurement_config.rule);
        assert_eq!(frozen.rule.fee_units, public.service_fee.quota_units);
        let mut legacy = stored.clone();
        legacy.rule = None;
        assert!(publication_config(&changed, &legacy)
            .unwrap_err()
            .contains("SnapshotMissing"));
        let mut altered = stored;
        altered.public.service_fee.quota_units += 1;
        assert!(publication_config(&changed, &altered).is_err());
    }
    #[test]
    fn procurement_owner_npc_test_package_quote_confirm_replay_updates_view_without_spending() {
        let (config, mut s) = started("npc-test-package");
        let mut policy = ProcurementConfig::default();
        policy.enabled = true;
        policy.allow_test_credits = true;
        policy.rule.fee_units = 2;
        policy.domain = Domain {
            network_hash: [1; 32],
            package_id: [2; 32],
            dapp_storage_id: [3; 32],
            oracle_key_id: 1,
        };
        s.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .procurement_config = policy.clone();
        let player = super::super::components::player_entity(s.app.world()).unwrap();
        let point = super::super::components::entity_position(s.app.world(), player).unwrap();
        let npc = 9_999_111;
        s.app.world_mut().spawn((
            super::super::components::ObjectId(npc),
            super::super::components::Npc,
            super::super::components::Position(point),
        ));
        super::super::npc::set_dialog(
            s.app.world_mut(),
            super::super::npc::ActiveNpcDialogState {
                npc_object_id: npc,
                npc_name: "Test quota NPC".into(),
                npc_name_key: None,
                stage: None,
                current: 0,
                required: 0,
                title: String::new(),
                body: vec![],
                footer: String::new(),
                links: vec![],
                input: None,
            },
        );
        let request = |id: &str, action| wire::ProcurementRequest {
            protocol_version: 1,
            request_id: id.into(),
            character_index: 0,
            action,
        };
        let before = s.active_character_checkpoint().unwrap();
        s.execute_procurement_owner(
            true,
            &request(
                "status",
                wire::ProcurementAction::Status { npc_object_id: npc },
            ),
            1000,
        )
        .unwrap();
        let q = s
            .execute_procurement_owner(
                true,
                &request(
                    "quota-quote",
                    wire::ProcurementAction::QuoteBuyQuota {
                        package_id: test_quota_package_id(&policy.rule),
                    },
                ),
                1001,
            )
            .unwrap()
            .reply
            .quote
            .unwrap();
        assert_eq!(config.procurement_view("demo").unwrap().credits.len(), 0);
        let confirm = request(
            "quota-confirm",
            wire::ProcurementAction::Confirm {
                quote_id: q.quote_id.clone(),
                signature: String::new(),
            },
        );
        let first = s.execute_procurement_owner(true, &confirm, 1002).unwrap();
        assert!(!first.reply.replayed);
        let view = first.reply.status.unwrap();
        assert_eq!(view.test_quota, 20);
        assert_eq!(view.paid_quota, 0);
        assert_eq!(view.quota_packages.len(), 1);
        assert!(!view.quota_packages[0].enabled);
        assert!(!view.production_billing_enabled);
        assert_eq!(view.fee_rule.quota_source, "test");
        assert_eq!(view.fee_rule.quota_units, 2);
        assert!(
            s.execute_procurement_owner(true, &confirm, 1003)
                .unwrap()
                .reply
                .replayed
        );
        assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
        assert_eq!(
            s.active_character_checkpoint().unwrap().credit,
            before.credit
        );
        assert_eq!(
            config
                .procurement_view("demo")
                .unwrap()
                .quota_purchases
                .len(),
            1
        );
        // Exercise the actual admitted owner request path, rather than a
        // hand-built client quote: source tags must match the wire contract.
        let purpose = ed25519_consensus::SigningKey::from([17; 32]);
        let quote = s
            .execute_procurement_owner(
                true,
                &request(
                    "publication-after-test-package",
                    wire::ProcurementAction::QuotePublish {
                        template_id: 245,
                        price_gold: "500".into(),
                        expires_at_ms: "90000".into(),
                        conditions: Default::default(),
                        purpose_public_key: wire::encode_hex(
                            &purpose.verification_key().to_bytes(),
                        ),
                    },
                ),
                1004,
            )
            .unwrap()
            .reply
            .quote
            .unwrap();
        assert_eq!(quote.service_fee.quota_source, "test");
        assert_eq!(quote.service_fee.quota_units, 2);
        assert!(!quote.service_fee.production_billing_enabled);
        let stored = config.procurement_view("demo").unwrap().quotes[&quote.quote_id].clone();
        assert_eq!(stored.public, quote);
        assert_eq!(stored.rule.as_ref().unwrap().fee_units, 2);
        assert_eq!(quote.intent.as_ref().unwrap().rule_version, "1");
        let published = s
            .execute_procurement_owner(
                true,
                &request(
                    "publication-confirm",
                    wire::ProcurementAction::Confirm {
                        quote_id: quote.quote_id.clone(),
                        signature: wire::encode_hex(
                            &purpose.sign(&stored.intent.hash()).to_bytes(),
                        ),
                    },
                ),
                1005,
            )
            .unwrap()
            .reply
            .status
            .unwrap();
        assert_eq!(published.my_orders[0].chain_state, "pendingPublish");
        assert_eq!(published.my_orders[0].delivery_state, "escrowed");
        assert!(published.public_orders.is_empty());
        let mut receipt = ChainReceipt {
            network_hash: policy.domain.network_hash,
            package_id: policy.domain.package_id,
            dapp_storage_id: policy.domain.dapp_storage_id,
            order_id: stored.intent.order_id,
            object_id: wire::encode_hex(&[77; 32]),
            status: 1,
            fill_id: [0; 32],
            buyer_ref: stored.intent.buyer_ref,
            seller_ref: [0; 32],
            template_id: 245,
            conditions_hash: stored.intent.conditions_hash,
            price_gold: 500,
            rule_version: 1,
            asset_instance_hash: [0; 32],
            fence: 0,
            checkpoint: 1,
            digest: "owner-wire-open-finalized".into(),
            event_seq: 0,
            chain_timestamp_ms: 1006,
        };
        config
            .procurement_apply_chain_receipt(&policy, "demo", &receipt)
            .unwrap();
        let opened = s
            .execute_procurement_owner(
                true,
                &request(
                    "public-after-open",
                    wire::ProcurementAction::List { template_id: None },
                ),
                1007,
            )
            .unwrap()
            .reply
            .status
            .unwrap();
        assert_eq!(opened.my_orders[0].chain_state, "open");
        assert_eq!(opened.public_orders[0], opened.my_orders[0]);
        let serialized = serde_json::to_value(&opened).unwrap();
        assert_eq!(serialized["publicOrders"][0]["chainState"], "open");
        assert_eq!(serialized["publicOrders"][0]["deliveryState"], "escrowed");
        assert_eq!(serialized["feeHistory"][0]["source"], "test");
        let domain_order =
            config.procurement_view("demo").unwrap().orders[&hex(&stored.intent.order_id)].clone();
        assert_wire_order_projections(&domain_order);
        receipt.status = 3;
        receipt.checkpoint = 2;
        receipt.digest = "owner-wire-cancel-finalized".into();
        receipt.chain_timestamp_ms = 1008;
        config
            .procurement_apply_chain_receipt(&policy, "demo", &receipt)
            .unwrap();
        let cancelled = s
            .execute_procurement_owner(
                true,
                &request(
                    "terminal-after-cancel",
                    wire::ProcurementAction::List { template_id: None },
                ),
                1009,
            )
            .unwrap()
            .reply
            .status
            .unwrap();
        assert!(cancelled.public_orders.is_empty());
        assert_eq!(cancelled.my_orders[0].chain_state, "cancelled");
        assert_eq!(cancelled.my_orders[0].delivery_state, "pendingClaim");
        let refund = cancelled
            .deliveries
            .iter()
            .find(|d| d.template_id == 0)
            .unwrap();
        assert_eq!(refund.item_uid, "0");
        assert_eq!(refund.state, "pendingClaim");
        let refunded = s
            .execute_procurement_owner(
                true,
                &request(
                    "claim-refund",
                    wire::ProcurementAction::Claim {
                        delivery_id: refund.delivery_id.clone(),
                    },
                ),
                1010,
            )
            .unwrap()
            .reply
            .status
            .unwrap();
        assert_eq!(refunded.my_orders[0].delivery_state, "refunded");
        assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
        let mut production = policy;
        production.allow_test_credits = false;
        assert_eq!(fee(&production).quota_source, "paid");
        assert!(!fee(&production).production_billing_enabled);
    }
    fn assert_wire_order_projections(original: &Order) {
        // Table-driven projection of an actually published, signature-verified
        // domain order. No hand-written frontend ProcurementOrder fixtures.
        for (status, expected) in [
            (OrderStatus::Publishing, "pendingPublish"),
            (OrderStatus::PublishFailed, "publishFailed"),
            (OrderStatus::Open, "open"),
            (OrderStatus::Filled, "filled"),
            (OrderStatus::Cancelled, "cancelled"),
            (OrderStatus::Expired, "expired"),
        ] {
            let mut o = original.clone();
            o.status = status;
            let base = order_view(&o, None);
            assert_eq!(base.chain_state, expected);
            assert_eq!(base.order_id, wire::encode_hex(&o.publish.intent.order_id));
            assert_eq!(base.template_id, 245);
            assert_eq!(base.price_gold, o.publish.intent.price_gold.to_string());
            let escrowed = matches!(status, OrderStatus::Publishing | OrderStatus::Open);
            assert_eq!(
                base.delivery_state,
                if escrowed { "escrowed" } else { "pending" }
            );
            if escrowed {
                continue;
            }
            let equipment = status == OrderStatus::Filled;
            let mut ledger = ProcurementLedger::default();
            let d = Delivery {
                id: "projection-receipt".into(),
                order_id: o.id.clone(),
                character_index: o.buyer_character,
                character_name: o.buyer_name.clone(),
                gold: if equipment {
                    0
                } else {
                    o.publish.intent.price_gold as u32
                },
                original_item_json: equipment.then(|| test_item(991245)),
                item_uid: equipment.then_some(991245),
                claimed: false,
                claim_revision: None,
            };
            ledger.deliveries.insert(d.id.clone(), d);
            assert_eq!(order_view(&o, Some(&ledger)).delivery_state, "pendingClaim");
            let d = ledger.deliveries.values_mut().next().unwrap();
            d.claimed = true;
            d.claim_revision = Some(123);
            let actual = order_view(&o, Some(&ledger));
            assert_eq!(
                actual.delivery_state,
                if equipment { "delivered" } else { "refunded" }
            );
            let json = serde_json::to_value(actual).unwrap();
            assert_eq!(json["chainState"], expected);
            assert!(json.get("chain_state").is_none());
            assert!(json.get("delivery_state").is_none());
        }
        for (source, expected) in [
            (CreditSource::Test, "test"),
            (CreditSource::Paid, "paid"),
            (CreditSource::Gift, "gift"),
            (CreditSource::Unknown, "unknown"),
        ] {
            assert_eq!(credit_source_wire(source), expected);
        }
    }
    #[test]
    fn procurement_npc_home_survives_real_snapshot_conversion_on_first_nine_row_page() {
        let (_config, mut s) = started("npc-first-page");
        s.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .procurement_config
            .enabled = true;
        let mut dialog = super::super::npc::ActiveNpcDialogState {
            npc_object_id: 427,
            npc_name: "Edward".into(),
            npc_name_key: None,
            stage: None,
            current: 0,
            required: 0,
            title: "Test".into(),
            body: vec!["one".into(), "two".into(), "three".into(), "four".into()],
            footer: String::new(),
            links: (0..5)
                .map(|i| super::super::npc::NpcDialogLinkState {
                    text: format!("Original {i}"),
                    target: format!("@original{i}"),
                })
                .collect(),
            input: None,
        };
        let original_body = dialog.body.clone();
        let original_links = dialog.links.clone();
        decorate_dialog(s.app.world(), &mut dialog);
        decorate_dialog(s.app.world(), &mut dialog);
        let snapshot_links = dialog.snapshot_links();
        let snapshot = dialog.snapshot(mir2_game_data::LanguageCode::English);
        assert_eq!(snapshot.links[0].target, HOME);
        assert_eq!(snapshot_links[0].target, HOME);
        assert_eq!(
            snapshot_links.iter().filter(|l| l.target == HOME).count(),
            1
        );
        assert_eq!(&dialog.links[1..], original_links.as_slice());
        assert_eq!(dialog.body, original_body);
        // The production client paginates body and option rows together by nine.
        let rows = snapshot
            .body
            .iter()
            .map(|_| None)
            .chain(snapshot.links.iter().map(|l| Some(l.target.as_str())))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 10);
        assert!(rows.chunks(9).next().unwrap().contains(&Some(HOME)));
    }
    #[test]
    fn procurement_owner_short_quote_deadline_matches_contract_policy() {
        assert_eq!(authorization_deadline(1000, 300_000, 2000).unwrap(), 2000);
        assert_eq!(
            authorization_deadline(1000, 300_000, 900_000).unwrap(),
            301_000
        );
        // Existing orders use their frozen rule even if the current rule changes.
        assert_eq!(
            authorization_deadline(1000, 20_000, 900_000).unwrap(),
            21_000
        );
        assert!(authorization_deadline(u64::MAX, 1, u64::MAX).is_err());
    }
    fn started(label: &str) -> (crate::SimulationConfig, SimulationSession) {
        let path = std::env::temp_dir().join(format!(
            "mir2-procurement-owner-{label}-{}-{}.json",
            std::process::id(),
            hex(&digest(
                format!("{:?}", std::time::SystemTime::now()).as_bytes()
            ))
        ));
        let config = crate::SimulationConfig::default().with_account_store_path(path);
        let mut s = SimulationSession::new(config.clone());
        s.handle_packet(mir2_protocol::ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        s.handle_packet(mir2_protocol::ClientPacket::StartGame { character_index: 0 });
        s.save_active_character().unwrap();
        (config, s)
    }
    fn pending(
        config: &crate::SimulationConfig,
        s: &SimulationSession,
        id: &str,
        gold: u32,
        item: Option<String>,
    ) {
        let save = s.active_character_checkpoint().unwrap();
        config
            .commit_account_store_transaction(&["demo".into()], |store| {
                let a = store.accounts.get_mut("demo").unwrap();
                a.billing
                    .get_or_insert_with(crate::billing::BillingLedger::default);
                let l = a.procurement.get_or_insert_with(ProcurementLedger::default);
                l.deliveries.insert(
                    id.into(),
                    Delivery {
                        id: id.into(),
                        order_id: "settled-test-order".into(),
                        character_index: 0,
                        character_name: save.character.name.clone(),
                        gold,
                        original_item_json: item.clone(),
                        item_uid: item.as_ref().map(|_| 9_000_001),
                        claimed: false,
                        claim_revision: None,
                    },
                );
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn procurement_owner_offline_pending_claim_preserves_instance_and_replay() {
        let (config, mut s) = started("claim");
        let json = test_item(9_000_001);
        pending(&config, &s, "item", 0, Some(json.clone()));
        let rev = s.active_character_checkpoint().unwrap().revision;
        assert!(!config.procurement_view("demo").unwrap().deliveries["item"].claimed);
        let p = s.procurement_claim_delivery("demo", "item").unwrap();
        assert!(p
            .iter()
            .any(|p| matches!(p,ServerPacket::GainedItem{item}if item.unique_id==9_000_001)));
        assert!(s
            .procurement_claim_delivery("demo", "item")
            .unwrap()
            .is_empty());
        let after = s.active_character_checkpoint().unwrap();
        assert_eq!(after.revision, rev + 1);
        let original: ItemState = serde_json::from_str(&json).unwrap();
        let found = after
            .inventory_items_json
            .iter()
            .map(|j| serde_json::from_str::<ItemState>(j).unwrap())
            .find(|i| i.unique_id == original.unique_id)
            .unwrap();
        assert_eq!(found.durability_current, Some(12345));
        assert_eq!(found.added_stats, original.added_stats);
        s.save_active_character().unwrap();
        assert!(config.procurement_view("demo").unwrap().deliveries["item"].claimed);
    }
    #[test]
    fn procurement_owner_full_bag_keeps_original_pending() {
        let (config, mut s) = started("full");
        let mut items = vec![];
        for slot in 0..crate::config::crystal_bag_slot_capacity(
            s.app
                .world()
                .resource::<InventoryResource>()
                .inventory_capacity,
        ) {
            let mut i: ItemState =
                serde_json::from_str(&test_item(1000 + u64::from(slot))).unwrap();
            let (container, local) =
                super::super::inventory::inventory_container_and_slot_for_index(slot as u8)
                    .unwrap();
            i.container = container;
            i.slot = local;
            items.push(i)
        }
        s.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items = items;
        s.save_active_character().unwrap();
        pending(&config, &s, "full", 0, Some(test_item(9_000_001)));
        assert!(s
            .procurement_claim_delivery("demo", "full")
            .unwrap_err()
            .contains("InventoryFull"));
        assert!(!config.procurement_view("demo").unwrap().deliveries["full"].claimed);
    }
    #[test]
    fn procurement_owner_gold_cap_keeps_pending_without_truncation() {
        let (config, mut s) = started("cap");
        s.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = u32::MAX;
        s.save_active_character().unwrap();
        pending(&config, &s, "gold", 1, None);
        assert_eq!(
            s.procurement_claim_delivery("demo", "gold").unwrap_err(),
            "procurementGoldCapacityPending"
        );
        assert_eq!(s.world_snapshot().gold, u32::MAX);
        assert!(!config.procurement_view("demo").unwrap().deliveries["gold"].claimed);
        s.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold -= 1;
        s.save_active_character().unwrap();
        s.procurement_claim_delivery("demo", "gold").unwrap();
        assert_eq!(s.world_snapshot().gold, u32::MAX);
        assert!(s
            .procurement_claim_delivery("demo", "gold")
            .unwrap()
            .is_empty());
    }
    #[test]
    fn procurement_owner_claim_database_failure_leaves_live_and_receipt_unchanged() {
        let (config, mut s) = started("failedclaim");
        pending(&config, &s, "gold", 7, None);
        let before = s.world_snapshot().gold;
        config.inject_account_store_transaction_fault(
            crate::AccountStoreTransactionFault::BeforeFileRename,
        );
        assert!(s.procurement_claim_delivery("demo", "gold").is_err());
        assert_eq!(s.world_snapshot().gold, before);
        assert!(!config.procurement_view("demo").unwrap().deliveries["gold"].claimed);
        s.procurement_claim_delivery("demo", "gold").unwrap();
        assert_eq!(s.world_snapshot().gold, before + 7);
    }
}
