//! Trusted source debit and permanent gold destruction in the same transaction.
use super::*;

/// Bound gold is destroyed at face value. This adapter cannot accept an asset
/// appraisal multiplier or a client-supplied exchange rate.
pub const BOUND_GOLD_FACE_VALUE_VALUATION_VERSION: &str = "bound-gold-face-value-v1";

fn validate_debit_burn(burn: &TrustedBurn, source_digest: &str) -> EconomyResult<()> {
    token(&burn.source_kind, "source_kind", 128)?;
    token(&burn.source_key, "source_key", 256)?;
    token(source_digest, "source_digest", 256)?;
    if burn.source_kind == "conversion_fee"
        || burn.gold_equivalent == 0
        || burn.valuation_version != BOUND_GOLD_FACE_VALUE_VALUATION_VERSION
    {
        return Err(EconomyError::InvalidInput("bound_debit_burn_source"));
    }
    bigint(burn.gold_equivalent, "bound_debit_amount")?;
    Ok(())
}

/// The key excludes amount/definition versions so an old operation cannot be
/// executed again by changing its payload. Such changes conflict in the digest.
pub fn bound_debit_burn_operation_identity(
    realm: &str,
    owner: &WalletOwner,
    burn: &TrustedBurn,
    source_digest: &str,
) -> EconomyResult<(String, String)> {
    token(realm, "realm_id", 128)?;
    validate_owner(owner)?;
    validate_debit_burn(burn, source_digest)?;
    let key = digest(&(
        "bound_debit_burn_v1",
        realm,
        owner,
        &burn.source_kind,
        &burn.source_key,
    ))?;
    let hash = digest(&(
        "bound_debit_burn_v1",
        realm,
        owner,
        &burn.source_kind,
        &burn.source_key,
        burn.layer,
        burn.gold_equivalent,
        &burn.valuation_version,
        source_digest,
    ))?;
    Ok((key, hash))
}

/// No raw client may construct this source capability. The authorization
/// callback MUST acquire every required account/character/save/source lock in
/// the game server's fixed order, before the wallet lock. It runs on replays too.
/// The source callback persists the COMPLETE real side effect/CAS in this same
/// transaction; it must not perform external I/O, commit independently, or
/// modify the wallet/pool/economy ledger owned by this adapter.
pub fn debit_bound_and_burn_authorized_with_source_with_timeouts<A, S>(
    client: &mut Client,
    realm: &str,
    burn: &TrustedBurn,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<BoundDebitBurnReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    debit_impl(
        client,
        realm,
        burn,
        source_digest,
        lock_timeout_ms,
        statement_timeout_ms,
        None,
        authorize,
        source,
    )
}

fn debit_impl<A, S>(
    client: &mut Client,
    realm: &str,
    burn: &TrustedBurn,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    fault: Option<Fault>,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<BoundDebitBurnReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    validate_debit_burn(burn, source_digest)?;
    let timeouts = Some((lock_timeout_ms, statement_timeout_ms));
    validate_timeouts(timeouts)?;
    let mut tx = client.transaction().map_err(db)?;
    configure_timeouts(&mut tx, timeouts)?;
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    let owner = authorize(&mut tx, clock)?;
    let (key, hash) = bound_debit_burn_operation_identity(realm, &owner, burn, source_digest)?;
    let result = if let Some(receipt) = existing(&mut tx, realm, &key, "bound_debit_burn", &hash)? {
        Idempotent {
            receipt,
            replayed: true,
        }
    } else {
        let (state, policy) = load_pool(&mut tx, realm)?;
        require_enabled(&policy)?;
        require_caught_up(&state, clock)?;
        let mut balance = wallet(&mut tx, realm, &owner, clock.database_time_ms)?;
        if balance.bound_gold < burn.gold_equivalent {
            return Err(EconomyError::InsufficientBoundGold);
        }
        // A failure here rolls back both the complete source and the debit.
        source(&mut tx, clock)?;
        balance.bound_gold -= burn.gold_equivalent;
        write_wallet(&mut tx, realm, &owner, &mut balance, clock.database_time_ms)?;
        let namespaced = TrustedBurn {
            source_kind: burn.source_kind.clone(),
            source_key: format!("bound-debit-burn:{key}"),
            layer: burn.layer,
            gold_equivalent: burn.gold_equivalent,
            valuation_version: BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
        };
        let burn_event_id = insert_burn(
            &mut tx,
            realm,
            &key,
            &namespaced,
            &policy.policy_version,
            clock,
        )?;
        let receipt = BoundDebitBurnReceipt {
            realm_id: realm.into(),
            operation_key: key.clone(),
            request_digest: hash.clone(),
            owner,
            source_kind: burn.source_kind.clone(),
            source_key: burn.source_key.clone(),
            bound_gold_debit: burn.gold_equivalent,
            burn_event_id,
            layer: burn.layer,
            valuation_version: BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
            policy_version: policy.policy_version,
            business_day: clock.business_day,
            committed_at_ms: clock.database_time_ms,
            wallet_after: balance,
        };
        operation(
            &mut tx,
            realm,
            &key,
            "bound_debit_burn",
            &hash,
            &receipt,
            clock,
        )?;
        Idempotent {
            receipt,
            replayed: false,
        }
    };
    if !result.replayed && matches!(fault, Some(Fault::BeforeCommit)) {
        return Err(injected_failure());
    }
    commit(tx)?;
    if !result.replayed && matches!(fault, Some(Fault::CommitAcknowledgementLost)) {
        return Err(EconomyError::OutcomeUnknown);
    }
    Ok(result)
}

/// Read only the original operation. Policy disablement, missing wallet or an
/// unsettled day never authorizes a refund, new ID, source rerun or fresh burn.
pub fn lookup_bound_debit_burn_authorized_with_timeouts<A>(
    client: &mut Client,
    realm: &str,
    burn: &TrustedBurn,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
) -> EconomyResult<Option<BoundDebitBurnReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    validate_debit_burn(burn, source_digest)?;
    transaction_with_timeouts(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        |tx, clock| {
            let owner = authorize(tx, clock)?;
            let (key, hash) =
                bound_debit_burn_operation_identity(realm, &owner, burn, source_digest)?;
            existing(tx, realm, &key, "bound_debit_burn", &hash)
        },
    )
}

#[cfg(any(test, feature = "test-support"))]
pub fn debit_bound_and_burn_authorized_with_source_with_test_fault<A, S>(
    client: &mut Client,
    realm: &str,
    burn: &TrustedBurn,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    fault: TestFault,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<BoundDebitBurnReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    let fault = match fault {
        TestFault::BeforeCommit => Fault::BeforeCommit,
        TestFault::CommitAcknowledgementLost => Fault::CommitAcknowledgementLost,
    };
    debit_impl(
        client,
        realm,
        burn,
        source_digest,
        lock_timeout_ms,
        statement_timeout_ms,
        Some(fault),
        authorize,
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner() -> WalletOwner {
        WalletOwner {
            account_id: "player".into(),
            character_index: 0,
            incarnation: "pg-character-v1:creation-one".into(),
        }
    }
    fn burn() -> TrustedBurn {
        TrustedBurn {
            source_kind: "npc_repair".into(),
            source_key: "server-operation-1".into(),
            layer: BurnLayer::Daily,
            gold_equivalent: 100,
            valuation_version: BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
        }
    }
    #[test]
    fn bound_debit_burn_identity_is_owner_scoped_and_payload_changes_conflict() {
        let who = owner();
        let original = burn();
        let (key, hash) =
            bound_debit_burn_operation_identity("realm", &who, &original, "digest").unwrap();
        let mut changed = original.clone();
        changed.gold_equivalent += 1;
        let other = bound_debit_burn_operation_identity("realm", &who, &changed, "digest").unwrap();
        assert_eq!(other.0, key);
        assert_ne!(other.1, hash);
        let changed = TrustedBurn {
            layer: BurnLayer::Siege,
            ..original.clone()
        };
        let other = bound_debit_burn_operation_identity("realm", &who, &changed, "digest").unwrap();
        assert_eq!(other.0, key);
        assert_ne!(other.1, hash);
        assert_eq!(
            bound_debit_burn_operation_identity("realm", &who, &original, "new-definition")
                .unwrap()
                .0,
            key
        );
        assert_ne!(
            bound_debit_burn_operation_identity("realm", &who, &original, "new-definition")
                .unwrap()
                .1,
            hash
        );
        for other in [
            WalletOwner {
                account_id: "other".into(),
                ..who.clone()
            },
            WalletOwner {
                incarnation: "pg-character-v1:creation-two".into(),
                ..who.clone()
            },
            WalletOwner {
                character_index: 1,
                ..who.clone()
            },
        ] {
            assert_ne!(
                bound_debit_burn_operation_identity("realm", &other, &original, "digest")
                    .unwrap()
                    .0,
                key
            );
        }
    }
    #[test]
    fn bound_debit_burn_rejects_non_face_value_zero_fee_and_overflow() {
        for changed in [
            TrustedBurn {
                gold_equivalent: 0,
                ..burn()
            },
            TrustedBurn {
                gold_equivalent: i64::MAX as u64 + 1,
                ..burn()
            },
            TrustedBurn {
                source_kind: "conversion_fee".into(),
                ..burn()
            },
            TrustedBurn {
                valuation_version: "appraisal-multiplier".into(),
                ..burn()
            },
        ] {
            assert!(
                bound_debit_burn_operation_identity("realm", &owner(), &changed, "digest").is_err()
            );
        }
        let maximum = TrustedBurn {
            gold_equivalent: i64::MAX as u64,
            ..burn()
        };
        assert!(bound_debit_burn_operation_identity("realm", &owner(), &maximum, "digest").is_ok());
    }
}
