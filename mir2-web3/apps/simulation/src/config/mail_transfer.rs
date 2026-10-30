//! Append-only send provenance in the actual account-source transaction.
//! This is deliberately not a global physical-ownership registry or claim permit.
use super::{
    AccountRecord, AccountStoreDatabaseMode, AccountStoreMutationPlan, CharacterSaveRecord,
    Stage5MailAttachmentOrigin, Stage5MailMessage, Stage5SystemsState,
};
use crate::runtime::mail_transfer_items::{attachment_manifest, AttachmentManifest};
use postgres::Transaction;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const REFUSED: &str = "MAIL_TRANSFER_SOURCE_REJECTED";

fn systems(save: &CharacterSaveRecord) -> Result<Stage5SystemsState, String> {
    save.stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| REFUSED.to_owned())
        .map(Option::unwrap_or_default)
}

pub(super) fn carries_source(plan: &AccountStoreMutationPlan) -> bool {
    plan.accounts.values().any(|mutation| {
        [
            mutation.original_account.as_ref(),
            mutation.desired_account.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|account| {
            account.saves.values().any(|save| {
                systems(save).is_ok_and(|state| {
                    state.mail.iter().any(|mail| {
                        mail.attachment_origin == Stage5MailAttachmentOrigin::ExistingCustody
                            && !mail.item_states_json.is_empty()
                    })
                })
            })
        })
    })
}

fn mail_at(
    account: Option<&AccountRecord>,
    index: i32,
    nonce: &str,
) -> Result<Option<Stage5MailMessage>, String> {
    let Some(save) = account.and_then(|account| account.saves.get(&index)) else {
        return Ok(None);
    };
    let state = systems(save)?;
    let mut matching = state
        .mail
        .into_iter()
        .filter(|mail| mail.delivery_nonce == nonce);
    let found = matching.next();
    if matching.next().is_some() {
        return Err(REFUSED.into());
    }
    Ok(found)
}

fn payload(mail: &Stage5MailMessage) -> Value {
    json!({"deliveryNonce":mail.delivery_nonce,"from":mail.from,"to":mail.to,
        "subject":mail.subject,"body":mail.body,"gold":mail.gold,"items":mail.items,
        "itemStatesJson":mail.item_states_json,"attachmentOrigin":mail.attachment_origin})
}

fn cleared(mail: &Stage5MailMessage) -> bool {
    mail.gold == 0 && mail.items.is_empty() && mail.item_states_json.is_empty()
}

fn valid_payload(mail: &Stage5MailMessage, original: &Value) -> bool {
    if &payload(mail) == original {
        return true;
    }
    if !(mail.claimed || mail.deleted) || !cleared(mail) {
        return false;
    }
    let mut expected = original.clone();
    expected["gold"] = json!(0);
    expected["items"] = json!([]);
    expected["itemStatesJson"] = json!([]);
    payload(mail) == expected
}

fn guard_existing(
    before: Option<&Stage5MailMessage>,
    after: Option<&Stage5MailMessage>,
    original: &Value,
) -> Result<(), String> {
    if before.is_some_and(|mail| !valid_payload(mail, original)) {
        return Err(REFUSED.into());
    }
    let Some(after) = after else {
        return if before.is_none_or(|mail| mail.claimed || mail.deleted) {
            Ok(())
        } else {
            Err(REFUSED.into())
        };
    };
    let before = before.ok_or(REFUSED)?; // A purged/deleted character cannot revive an old delivery.
    if !valid_payload(after, original)
        || (before.claimed && !after.claimed)
        || (before.deleted && !after.deleted)
        || ((before.claimed || before.deleted) && cleared(before) && !cleared(after))
    {
        return Err(REFUSED.into());
    }
    Ok(())
}

fn digest(binding: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(binding).map_err(|_| REFUSED)?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn actor(account: &str, save: &CharacterSaveRecord) -> Result<Value, String> {
    if let Some(journal) = &save.npc_purchase_journal {
        journal
            .validate_for(
                account,
                save.character.index,
                &save.character.name,
                save.revision,
            )
            .map_err(|_| REFUSED)?;
        Ok(json!(journal.actor))
    } else {
        Ok(Value::Null)
    }
}

fn identity(
    plan: &AccountStoreMutationPlan,
    account_id: &str,
    index: i32,
) -> Result<Value, String> {
    let mutation = plan.accounts.get(account_id).ok_or(REFUSED)?;
    let before = mutation.original_account.as_ref().ok_or(REFUSED)?;
    let after = mutation.desired_account.as_ref().ok_or(REFUSED)?;
    let character = before
        .characters
        .iter()
        .find(|character| character.index == index)
        .ok_or(REFUSED)?;
    let save = after.saves.get(&index).ok_or(REFUSED)?;
    if save.character != *character
        || !after.characters.iter().any(|row| row == character)
        || mutation.expected_version.is_none()
    {
        return Err(REFUSED.into());
    }
    let old = before.saves.get(&index);
    if old.is_some_and(|old| old.character != *character)
        || old.map_or(0, |old| old.revision).checked_add(1) != Some(save.revision)
    {
        return Err(REFUSED.into());
    }
    let old_actor = old
        .map(|old| actor(account_id, old))
        .transpose()?
        .unwrap_or(Value::Null);
    if actor(account_id, save)? != old_actor {
        return Err(REFUSED.into());
    }
    Ok(
        json!({"accountId":account_id,"characterIndex":index,"characterName":character.name,
        "expectedAccountVersion":mutation.expected_version,
        "expectedSaveVersion":mutation.saves.get(&index).ok_or(REFUSED)?.expected_version,
        "beforeRevision":old.map(|old| old.revision),"afterRevision":save.revision,"actor":old_actor}),
    )
}

struct NewReceipt {
    nonce: String,
    account: String,
    index: i32,
    binding: Value,
}

// This projection is only for filtering already-declared bag carriers and
// detecting retained physical copies after withdrawal. It grants no transfer
// authority: a selected root must still pass the strict ordinary manifest.
// In particular, unrelated monthly cards and old unassigned roots are legal.
#[derive(Deserialize)]
struct BagIdentity {
    #[serde(default)]
    unique_id: u64,
    #[serde(default)]
    socketed: Vec<BagIdentity>,
    #[serde(default)]
    user_item_metadata: Option<BagMetadata>,
}

#[derive(Deserialize)]
struct BagMetadata {
    #[serde(default)]
    slots: Vec<Option<WireIdentity>>,
    #[serde(default)]
    live_socketed_at_capture: bool,
    #[serde(default)]
    socket_layout_hydrated: bool,
}

#[derive(Deserialize)]
struct WireIdentity {
    unique_id: u64,
    #[serde(default)]
    slots: Vec<Option<WireIdentity>>,
}

fn bag_identity(encoded: &str) -> Result<BagIdentity, String> {
    serde_json::from_str(encoded).map_err(|_| REFUSED.to_owned())
}

fn bag_live_uids(item: &BagIdentity) -> Result<BTreeSet<u64>, String> {
    fn budget(depth: usize, children: usize, nodes: &mut usize) -> Result<(), String> {
        *nodes += 1;
        if depth > 8 || children > 64 || *nodes > 256 {
            return Err(REFUSED.into());
        }
        Ok(())
    }
    fn wire(
        item: &WireIdentity,
        depth: usize,
        nodes: &mut usize,
        ids: &mut BTreeSet<u64>,
    ) -> Result<(), String> {
        budget(depth, item.slots.len(), nodes)?;
        if item.unique_id != 0 {
            ids.insert(item.unique_id);
        }
        for child in item.slots.iter().flatten() {
            wire(child, depth + 1, nodes, ids)?;
        }
        Ok(())
    }
    fn live(
        item: &BagIdentity,
        depth: usize,
        nodes: &mut usize,
        ids: &mut BTreeSet<u64>,
    ) -> Result<(), String> {
        budget(depth, item.socketed.len(), nodes)?;
        if item.unique_id != 0 {
            ids.insert(item.unique_id);
        }
        for child in &item.socketed {
            live(child, depth + 1, nodes, ids)?;
        }
        if let Some(metadata) = &item.user_item_metadata {
            if metadata.slots.len() > 64 {
                return Err(REFUSED.into());
            }
            // Match the gameplay carrier interpreter: once live sockets are
            // authoritative, old metadata slots are retained history only.
            if item.socketed.is_empty()
                && !metadata.live_socketed_at_capture
                && !metadata.socket_layout_hydrated
            {
                for child in metadata.slots.iter().flatten() {
                    wire(child, depth + 1, nodes, ids)?;
                }
            }
        }
        Ok(())
    }
    let mut ids = BTreeSet::new();
    live(item, 0, &mut 0, &mut ids)?;
    Ok(ids)
}

fn needs_new_receipt(
    before: Option<&Stage5MailMessage>,
    after: &Stage5MailMessage,
) -> Result<bool, String> {
    let carries = |mail: &Stage5MailMessage| {
        mail.attachment_origin == Stage5MailAttachmentOrigin::ExistingCustody
            && !mail.item_states_json.is_empty()
    };
    if let Some(before) = before {
        if carries(before) || carries(after) {
            // A pre-existing nonce is never a provenance bootstrap. Origin
            // upgrades and attachment additions are mutations, not new sends.
            guard_existing(Some(before), Some(after), &payload(before))?;
        }
        return Ok(false);
    }
    Ok(carries(after))
}

fn carries_items(mail: &Stage5MailMessage) -> bool {
    mail.attachment_origin == Stage5MailAttachmentOrigin::ExistingCustody
        && !mail.item_states_json.is_empty()
}

fn sender(
    plan: &AccountStoreMutationPlan,
    mail: &Stage5MailMessage,
    manifest: &AttachmentManifest,
) -> Result<(String, i32, Vec<String>), String> {
    let originals = manifest.original_items.as_array().ok_or(REFUSED)?;
    let mut source = None;
    let mut raw_items = Vec::new();
    for (ordinal, uid) in manifest.root_uids.iter().enumerate() {
        let mut found = None;
        for (account_id, mutation) in &plan.accounts {
            let Some(before) = &mutation.original_account else {
                continue;
            };
            for (index, save) in &before.saves {
                if save.character.name != mail.from {
                    continue;
                }
                for encoded in &save.inventory_items_json {
                    if bag_identity(encoded)?.unique_id != *uid {
                        continue;
                    }
                    let candidate = attachment_manifest(std::slice::from_ref(encoded))?;
                    if candidate.original_items[0] != originals[ordinal]
                        || serde_json::from_str::<Value>(encoded).map_err(|_| REFUSED)?
                            != serde_json::from_str::<Value>(&mail.item_states_json[ordinal])
                                .map_err(|_| REFUSED)?
                        || found.is_some()
                    {
                        return Err(REFUSED.into());
                    }
                    found = Some((account_id.clone(), *index, encoded.clone()));
                }
            }
        }
        let (account_id, index, encoded) = found.ok_or(REFUSED)?;
        if source
            .as_ref()
            .is_some_and(|old| old != &(account_id.clone(), index))
        {
            return Err(REFUSED.into());
        }
        source = Some((account_id, index));
        raw_items.push(encoded);
    }
    let (account_id, index) = source.ok_or(REFUSED)?;
    let after = plan.accounts[&account_id]
        .desired_account
        .as_ref()
        .and_then(|account| account.saves.get(&index))
        .ok_or(REFUSED)?;
    for encoded in &after.inventory_items_json {
        if !bag_live_uids(&bag_identity(encoded)?)?.is_disjoint(&manifest.live_uids) {
            return Err(REFUSED.into());
        }
    }
    Ok((account_id, index, raw_items))
}

fn new_receipts(plan: &AccountStoreMutationPlan) -> Result<Vec<NewReceipt>, String> {
    let mut receipts = Vec::new();
    let mut nonces = BTreeSet::new();
    let mut transferred = BTreeSet::new();
    for (account_id, mutation) in &plan.accounts {
        let Some(account) = &mutation.desired_account else {
            continue;
        };
        for (index, save) in &account.saves {
            let previous = mutation
                .original_account
                .as_ref()
                .and_then(|account| account.saves.get(index))
                .map(systems)
                .transpose()?
                .unwrap_or_default();
            let mut seen = BTreeSet::new();
            for mail in systems(save)?.mail {
                if !carries_items(&mail)
                    && !previous.mail.iter().any(|before| {
                        before.delivery_nonce == mail.delivery_nonce && carries_items(before)
                    })
                {
                    continue;
                }
                if !mail.delivery_nonce.is_empty() && !seen.insert(mail.delivery_nonce.clone()) {
                    return Err(REFUSED.into());
                }
                let before = mail_at(
                    mutation.original_account.as_ref(),
                    *index,
                    &mail.delivery_nonce,
                )?;
                if !needs_new_receipt(before.as_ref(), &mail)? {
                    continue;
                }
                if mail.delivery_nonce.is_empty()
                    || mail.claimed
                    || mail.deleted
                    || !nonces.insert(mail.delivery_nonce.clone())
                    || mail.to != save.character.name
                {
                    return Err(REFUSED.into());
                }
                let manifest = attachment_manifest(&mail.item_states_json)?;
                if manifest.root_uids.len() != mail.items.len()
                    || manifest
                        .original_items
                        .as_array()
                        .ok_or(REFUSED)?
                        .iter()
                        .zip(&mail.items)
                        .any(|(item, key)| item["key"].as_str() != Some(key.as_str()))
                    || manifest
                        .live_uids
                        .iter()
                        .any(|uid| !transferred.insert(*uid))
                {
                    return Err(REFUSED.into());
                }
                let (sender_account, sender_index, original_source) =
                    sender(plan, &mail, &manifest)?;
                let binding = json!({"version":1,"sender":identity(plan,&sender_account,sender_index)?,
                    "recipient":identity(plan,account_id,*index)?,"mail":payload(&mail),
                    "originalSourceItemJson":original_source,"attachmentManifest":manifest.original_items,
                    "liveUids":manifest.live_uids.iter().map(u64::to_string).collect::<Vec<_>>(),
                    "rootUids":manifest.root_uids.iter().map(u64::to_string).collect::<Vec<_>>()});
                receipts.push(NewReceipt {
                    nonce: mail.delivery_nonce,
                    account: account_id.clone(),
                    index: *index,
                    binding,
                });
            }
        }
    }
    receipts.sort_by(|left, right| left.nonce.cmp(&right.nonce));
    Ok(receipts)
}

pub(super) fn write(
    transaction: &mut Transaction<'_>,
    plan: &AccountStoreMutationPlan,
    mode: AccountStoreDatabaseMode,
) -> Result<(), String> {
    let accounts = plan.accounts.keys().cloned().collect::<Vec<_>>();
    let recorded = transaction.query(
        "SELECT delivery_nonce,recipient_account_id,recipient_character_index,binding,binding_digest FROM public.mail_item_transfer_receipts WHERE source_namespace=current_schema() AND recipient_account_id=ANY($1) ORDER BY delivery_nonce FOR UPDATE",
        &[&accounts]).map_err(|_| REFUSED.to_owned())?;
    for row in recorded {
        let nonce: String = row.try_get(0).map_err(|_| REFUSED)?;
        let account: String = row.try_get(1).map_err(|_| REFUSED)?;
        let index: i32 = row.try_get(2).map_err(|_| REFUSED)?;
        let binding: Value = row.try_get(3).map_err(|_| REFUSED)?;
        let hash: String = row.try_get(4).map_err(|_| REFUSED)?;
        if digest(&binding)? != hash
            || binding["version"] != json!(1)
            || binding["recipient"]["accountId"] != json!(account)
            || binding["recipient"]["characterIndex"] != json!(index)
            || binding["mail"]["deliveryNonce"] != json!(nonce)
        {
            return Err(REFUSED.into());
        }
        let mutation = plan.accounts.get(&account).ok_or(REFUSED)?;
        let before = mail_at(mutation.original_account.as_ref(), index, &nonce)?;
        let after = mail_at(mutation.desired_account.as_ref(), index, &nonce)?;
        if mutation
            .desired_account
            .as_ref()
            .is_some_and(|account| account.saves.contains_key(&index))
        {
            guard_existing(before.as_ref(), after.as_ref(), &binding["mail"])?;
        } // Character/account deletion retains the immutable receipt.
    }
    if mode != AccountStoreDatabaseMode::SourceOfTruth {
        return Ok(());
    }
    for receipt in new_receipts(plan)? {
        let hash = digest(&receipt.binding)?;
        // A collision is not a new send, even when another plan's bytes match.
        transaction.execute(
            "INSERT INTO public.mail_item_transfer_receipts(source_namespace,delivery_nonce,recipient_account_id,recipient_character_index,binding,binding_digest) VALUES(current_schema(),$1,$2,$3,$4,$5)",
            &[&receipt.nonce,&receipt.account,&receipt.index,&receipt.binding,&hash])
            .map_err(|_| REFUSED.to_owned())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "mail_transfer_tests.rs"]
mod tests;
