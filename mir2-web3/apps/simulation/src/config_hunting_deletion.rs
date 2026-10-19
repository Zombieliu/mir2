//! Protect PostgreSQL character tombstones from orphaning the NEW hunting wallet.
//! The caller already holds `accounts FOR UPDATE` in the actual save transaction.
//! HTTP conversion acquires realm -> account SHARE -> session SHARE -> character
//! SHARE. We take only the account/character subsequence and NEVER acquire a realm
//! or wallet lock afterwards; account -> realm would invert that order.
//!
//! This protects the current authenticated HTTP conversion path. Every future
//! trusted reward/burn source must also lock and validate its account/character in
//! its source callback, inside the same transaction, before publishing new assets.
//! A caller that blindly credits a retired incarnation cannot be made safe by a
//! deletion precheck elsewhere. No such reward route is enabled by this module.

use super::{AccountStoreAccountMutation, Transaction};
use std::collections::BTreeSet;

pub(super) const NONZERO_WALLET: &str =
    "character deletion refused: hunting economy wallet has remaining assets";

fn retained_character_indices(mutation: &AccountStoreAccountMutation) -> Vec<i32> {
    let mut retained = BTreeSet::new();
    if let Some(account) = &mutation.desired_account {
        retained.extend(account.characters.iter().map(|character| character.index));
        // The ordinary PG writer recreates roster entries referenced by a save.
        // Use that exact same union, not just the visible character-list vector.
        retained.extend(account.saves.values().map(|save| save.character.index));
    }
    retained.into_iter().collect()
}

/// Called AFTER the account UPDATE lock and BEFORE any persisted tombstone or
/// save/projection mutation. A rejection rolls back the complete existing CAS
/// transaction; the staged in-memory account image is never published.
pub(super) fn guard_character_tombstones(
    transaction: &mut Transaction<'_>,
    account_id: &str,
    mutation: &AccountStoreAccountMutation,
) -> Result<(), String> {
    let retained = retained_character_indices(mutation);
    let removed = transaction
        .query(
            "SELECT character_index, hunting_incarnation::text AS incarnation
             FROM characters
             WHERE account_id=$1 AND character_index <> ALL($2)
             ORDER BY character_index FOR UPDATE",
            &[&account_id, &retained],
        )
        .map_err(|_| "hunting economy character deletion lookup failed".to_string())?;

    for character in removed {
        let index: i32 = character.get("character_index");
        let incarnation = format!(
            "pg-character-v1:{}",
            character.get::<_, String>("incarnation")
        );
        // Plain MVCC reads are deliberate. Taking a wallet lock after an account
        // lock could deadlock with a trusted source's wallet -> source callback.
        // The source contract above requires the account lock before publication.
        let nonzero: bool = transaction
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM hunting_economy_wallets
                    WHERE account_id=$1 AND character_index=$2
                      AND character_incarnation=$3
                      AND (bound_gold <> 0 OR unbound_gold <> 0)
                 ) AS blocked",
                &[&account_id, &index, &incarnation],
            )
            .map_err(|_| "hunting economy wallet deletion check failed".to_string())?
            .get("blocked");
        if nonzero {
            return Err(NONZERO_WALLET.into());
        }

        // New hunting operations are fully terminal on COMMIT. Their outbox is
        // committed audit/export work, NOT an unfinished asset projection, and
        // therefore does not block a zero-balance deletion. In-flight qualified
        // conversions are serialized by the account lock above. A future durable
        // pending asset projection needs an explicit hold check here when added.
    }
    Ok(())
}

#[cfg(test)]
#[path = "config_hunting_deletion_tests.rs"]
mod tests;
