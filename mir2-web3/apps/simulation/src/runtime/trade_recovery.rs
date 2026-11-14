//! The in-process economy commits a pair record before delivery. The external
//! economy locks both prepared saves and fences refunds against its outbox.

use bevy_ecs::prelude::World;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{
    AccountRecord, AccountStoreDatabaseMode, CharacterSaveRecord, CurrencyKind, SimulationConfig,
    Stage5SystemsState, Stage5TradeState,
};

use super::resources::{RuntimeConfigResource, SessionResource};
use super::save::active_session_mutating_account_id;
use super::session::SharedTradeOffer;

pub(super) const LOCAL_RECOVERY_VERSION: u8 = 1;
pub(super) const POSTGRES_RECOVERY_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalTradeMatch {
    pub(super) event_id: String,
    pub(super) own: SharedTradeOffer,
    pub(super) incoming: SharedTradeOffer,
}

fn config(world: &World) -> &SimulationConfig {
    &world.resource::<RuntimeConfigResource>().config
}

pub(super) fn uses_local_source(world: &World) -> bool {
    let config = config(world);
    config.account_store_path.is_some()
        || (config.account_store_database_url.is_some()
            && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth)
}

pub(super) fn uses_postgres_source(world: &World) -> bool {
    let config = config(world);
    config.account_store_path.is_none()
        && config.account_store_database_url.is_some()
        && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth
        && config.zone_save_fence().is_some()
}

fn active_character(world: &World) -> Result<(String, i32), String> {
    let session = world.resource::<SessionResource>();
    let account = active_session_mutating_account_id(session)
        .ok_or("trade recovery requires an authenticated account")?;
    let character = session
        .selected_character
        .as_ref()
        .ok_or("trade recovery requires an active character")?;
    Ok((account, character.index))
}

fn valid_nonce(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn match_event_id(first: &SharedTradeOffer, second: &SharedTradeOffer) -> Result<String, String> {
    let first_key = (&first.account_id, first.character_index);
    let second_key = (&second.account_id, second.character_index);
    if first_key == second_key
        || first.settlement_nonce == second.settlement_nonce
        || !valid_nonce(&first.settlement_nonce)
        || !valid_nonce(&second.settlement_nonce)
        || !first
            .partner_name
            .eq_ignore_ascii_case(&second.character_name)
        || !second
            .partner_name
            .eq_ignore_ascii_case(&first.character_name)
    {
        return Err("invalid trade participant binding".into());
    }
    let ordered = if first_key < second_key {
        (first, second)
    } else {
        (second, first)
    };
    let encoded = serde_json::to_vec(&ordered).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    digest.update(b"mir2-local-trade-match-v1\0");
    digest.update(encoded);
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn decode_systems(save: &CharacterSaveRecord) -> Result<Stage5SystemsState, String> {
    serde_json::from_str(
        save.stage5_systems_json
            .as_deref()
            .ok_or("trade recovery requires a saved Stage 5 state")?,
    )
    .map_err(|_| "invalid saved Stage 5 state during trade recovery".into())
}

fn verify_saved_offer(
    save: &CharacterSaveRecord,
    offer: &SharedTradeOffer,
    version: u8,
) -> Result<(), String> {
    let systems = decode_systems(save)?;
    if save.character.index != offer.character_index
        || !save
            .character
            .name
            .eq_ignore_ascii_case(&offer.character_name)
        || !systems.trade.as_ref().is_some_and(|trade| {
            trade.trade_recovery_version == Some(version)
                && trade.accepted
                && trade.locked
                && trade.escrow_prepared
                && !trade.completed
                && trade.settlement_nonce == offer.settlement_nonce
                && trade.partner.eq_ignore_ascii_case(&offer.partner_name)
                && trade.offered_currency == CurrencyKind::Gold
                && trade.offered_gold == offer.gold
                && trade.validated_held_gold() == Some(offer.gold)
                && super::trade_custody::matches_offer(trade, offer)
        })
    {
        return Err("trade does not match its saved preparation".into());
    }
    Ok(())
}

/// Publish both receiver records in one source transaction. Neither is ever
/// built from a character checkpoint alone or from a client-provided ID.
pub(super) fn commit_match(
    world: &World,
    own: &SharedTradeOffer,
    incoming: &SharedTradeOffer,
) -> Result<(), String> {
    let (account_id, character_index) = active_character(world)?;
    if !uses_local_source(world)
        || account_id != own.account_id
        || character_index != own.character_index
    {
        return Err("local trade commit requires its active source participant".into());
    }
    let event_id = match_event_id(own, incoming)?;
    let accounts = [own.account_id.clone(), incoming.account_id.clone()];
    let config = config(world);
    for account in &accounts {
        config.refresh_account_store_account(account)?;
    }
    config.commit_account_store_transaction(&accounts, |store| {
        for (own, incoming) in [(own, incoming), (incoming, own)] {
            let account = store
                .accounts
                .get_mut(&own.account_id)
                .ok_or("trade account missing")?;
            let entry = LocalTradeMatch {
                event_id: event_id.clone(),
                own: own.clone(),
                incoming: incoming.clone(),
            };
            if let Some(existing) = account.local_trade_matches.get(&own.character_index) {
                if existing != &entry {
                    return Err("character has another pending local trade".into());
                }
                continue;
            }
            let save = account
                .saves
                .get(&own.character_index)
                .ok_or("trade save missing")?;
            verify_saved_offer(save, own, LOCAL_RECOVERY_VERSION)?;
            account
                .local_trade_matches
                .insert(own.character_index, entry);
        }
        Ok(())
    })
}

pub(super) fn pending_match(world: &World) -> Result<Option<LocalTradeMatch>, String> {
    let (account_id, index) = active_character(world)?;
    let config = config(world);
    // A failed/unknown source publication can leave the live image behind the
    // durable one. An unreadable absence is never authority to refund.
    config.ensure_account_store_writable()?;
    config.refresh_account_store_account(&account_id)?;
    let store = config
        .account_store
        .lock()
        .map_err(|_| "trade account store unavailable")?;
    let account = store
        .accounts
        .get(&account_id)
        .ok_or("trade account missing")?;
    let Some(record) = account.local_trade_matches.get(&index) else {
        return Ok(None);
    };
    if record.own.account_id != account_id
        || record.own.character_index != index
        || match_event_id(&record.own, &record.incoming)? != record.event_id
    {
        return Err("invalid local trade recovery record".into());
    }
    Ok(Some(record.clone()))
}

/// The incoming assets, projection marker, and retirement of this role's
/// journal entry are published together in the character save transaction.
pub(super) fn save_match_projection(
    account: &mut AccountRecord,
    save: &CharacterSaveRecord,
    systems: &Stage5SystemsState,
) -> Result<(), String> {
    let index = save.character.index;
    if let Some(record) = account.local_trade_matches.get(&index) {
        if systems
            .economy_projection_event_ids
            .contains(&record.event_id)
            && systems.trade.is_none()
        {
            account.local_trade_matches.remove(&index);
        } else {
            verify_saved_offer(save, &record.own, LOCAL_RECOVERY_VERSION)?;
        }
    }
    Ok(())
}

/// Called inside the fenced economy transaction, with the same ordering as
/// a scoped AccountStore save. A later refund either sees the committed row
/// or loses its source version CAS. An earlier refund invalidates the hold.
pub(crate) fn lock_postgres_preparations(
    transaction: &mut postgres::Transaction<'_>,
    first: &SharedTradeOffer,
    second: &SharedTradeOffer,
) -> Result<(), String> {
    match_event_id(first, second)?;
    let mut offers = [first, second];
    offers.sort_by(|left, right| {
        (&left.account_id, left.character_index).cmp(&(&right.account_id, right.character_index))
    });
    for offer in offers {
        transaction
            .query_opt(
                "SELECT 1 FROM accounts WHERE account_id=$1 FOR UPDATE",
                &[&offer.account_id],
            )
            .map_err(|_| "prepared trade account lock failed")?
            .ok_or("prepared trade account missing")?;
        let row = transaction.query_opt(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2 FOR UPDATE",
            &[&offer.account_id, &offer.character_index],
        ).map_err(|_| "prepared trade source lock failed")?.ok_or("prepared trade save missing")?;
        let encoded = row
            .try_get("snapshot_json")
            .map_err(|_| "prepared trade snapshot missing")?;
        let save: CharacterSaveRecord =
            serde_json::from_value(encoded).map_err(|_| "prepared trade save is invalid")?;
        verify_saved_offer(&save, offer, POSTGRES_RECOVERY_VERSION)?;
        let changed = transaction.execute(
            "UPDATE character_saves SET save_version=save_version+1, updated_at=now() WHERE account_id=$1 AND character_index=$2",
            &[&offer.account_id, &offer.character_index],
        ).map_err(|_| "prepared trade source version fence failed")?;
        if changed != 1 {
            return Err("prepared trade source version fence lost its row".into());
        }
    }
    Ok(())
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedTradeProjection {
    trade: Option<Stage5TradeState>,
    #[serde(default)]
    economy_projection_event_ids: std::collections::BTreeSet<String>,
}

fn saved_projection(save: Option<&CharacterSaveRecord>) -> Result<SavedTradeProjection, String> {
    save.and_then(|save| save.stage5_systems_json.as_deref())
        .map(serde_json::from_str)
        .transpose()
        .map(|projection| projection.unwrap_or_default())
        .map_err(|_| "invalid durable trade save projection".into())
}

/// This runs after the account/save rows have been locked and the source
/// versions checked. Only a matching applied projection may consume a hold
/// already committed by the ledger. An absent row permits returning it.
pub(crate) fn validate_postgres_preparation_change(
    transaction: &mut postgres::Transaction<'_>,
    account_id: &str,
    original: &CharacterSaveRecord,
    desired: Option<&CharacterSaveRecord>,
) -> Result<(), String> {
    if desired.is_some_and(|save| save.stage5_systems_json == original.stage5_systems_json) {
        return Ok(());
    }
    let Some(held) = saved_projection(Some(original))?.trade.filter(|trade| {
        trade.trade_recovery_version == Some(POSTGRES_RECOVERY_VERSION) && trade.escrow_prepared
    }) else {
        return Ok(());
    };
    let proposed = saved_projection(desired)?;
    if proposed.trade.as_ref() == Some(&held) {
        return Ok(());
    }
    let projections = transaction
        .query(
            "SELECT event_id FROM game_economy_trade_projections \
         WHERE account_id=$1 AND character_index=$2 AND own_offer->>'settlement_nonce'=$3 LIMIT 2",
            &[
                &account_id,
                &original.character.index,
                &held.settlement_nonce,
            ],
        )
        .map_err(|_| "prepared trade outcome query failed")?;
    if projections.len() > 1
        || projections.first().is_some_and(|projection| {
            let event_id: String = projection.get("event_id");
            proposed.trade.is_some() || !proposed.economy_projection_event_ids.contains(&event_id)
        })
    {
        return Err("prepared trade already belongs to a committed settlement".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "trade_recovery_tests.rs"]
mod tests;
