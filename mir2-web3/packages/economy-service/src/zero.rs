//! Complete intrinsically free source mutations without touching wallet/burn state.
use super::*;

fn validate_zero_source(
    source_kind: &str,
    source_key: &str,
    source_digest: &str,
) -> EconomyResult<()> {
    token(source_kind, "source_kind", 128)?;
    token(source_key, "source_key", 256)?;
    token(source_digest, "source_digest", 256)?;
    if source_kind == "conversion_fee" {
        return Err(EconomyError::InvalidInput("zero_fee_source"));
    }
    Ok(())
}

/// Shares the positive debit's source slot, with a distinct fixed-zero payload.
/// Repricing an already committed source conflicts rather than executing it again.
pub fn zero_fee_source_operation_identity(
    realm: &str,
    owner: &WalletOwner,
    source_kind: &str,
    source_key: &str,
    source_digest: &str,
) -> EconomyResult<(String, String)> {
    validate_zero_source(source_kind, source_key, source_digest)?;
    let key = bound_source_operation_key(realm, owner, source_kind, source_key)?;
    let hash = digest(&(
        "zero_fee_source_v1",
        realm,
        owner,
        source_kind,
        source_key,
        source_digest,
    ))?;
    Ok((key, hash))
}

/// Trusted server adapter for an immutable quote whose authoritative fee is zero.
/// A must authorize and lock all account/character/save/source rows in fixed order,
/// even for replays. S persists the COMPLETE side effect/CAS in this transaction.
/// Neither callback may independently commit, perform external I/O or mutate the
/// wallet/pool/economy ledger. In particular, S cannot register a zero-value burn.
/// No wallet is read or provisioned. No burn or permit event is created.
pub fn record_zero_fee_source_authorized_with_source_with_timeouts<A, S>(
    client: &mut Client,
    realm: &str,
    source_kind: &str,
    source_key: &str,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<ZeroFeeSourceReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    zero_impl(
        client,
        realm,
        source_kind,
        source_key,
        source_digest,
        lock_timeout_ms,
        statement_timeout_ms,
        None,
        authorize,
        source,
    )
}

fn zero_impl<A, S>(
    client: &mut Client,
    realm: &str,
    source_kind: &str,
    source_key: &str,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    fault: Option<Fault>,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<ZeroFeeSourceReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    validate_zero_source(source_kind, source_key, source_digest)?;
    let timeouts = Some((lock_timeout_ms, statement_timeout_ms));
    validate_timeouts(timeouts)?;
    let mut tx = client.transaction().map_err(db)?;
    configure_timeouts(&mut tx, timeouts)?;
    let clock = lock_realm_and_read_database_clock(&mut tx, realm)?;
    let owner = authorize(&mut tx, clock)?;
    let (key, hash) =
        zero_fee_source_operation_identity(realm, &owner, source_kind, source_key, source_digest)?;
    let result = if let Some(receipt) = existing(&mut tx, realm, &key, "zero_fee_source", &hash)? {
        Idempotent {
            receipt,
            replayed: true,
        }
    } else {
        let (state, policy) = load_pool(&mut tx, realm)?;
        require_enabled(&policy)?;
        require_caught_up(&state, clock)?;
        source(&mut tx, clock)?;
        let receipt = ZeroFeeSourceReceipt {
            realm_id: realm.into(),
            operation_key: key.clone(),
            request_digest: hash.clone(),
            owner,
            source_kind: source_kind.into(),
            source_key: source_key.into(),
            policy_version: policy.policy_version,
            business_day: clock.business_day,
            committed_at_ms: clock.database_time_ms,
        };
        operation(
            &mut tx,
            realm,
            &key,
            "zero_fee_source",
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

/// Authenticated original-slot lookup, including when new sources are disabled
/// or settlement is behind. This never reruns S or invents a replacement ID.
pub fn lookup_zero_fee_source_authorized_with_timeouts<A>(
    client: &mut Client,
    realm: &str,
    source_kind: &str,
    source_key: &str,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    authorize: A,
) -> EconomyResult<Option<ZeroFeeSourceReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
{
    validate_zero_source(source_kind, source_key, source_digest)?;
    transaction_with_timeouts(
        client,
        realm,
        Some((lock_timeout_ms, statement_timeout_ms)),
        |tx, clock| {
            let owner = authorize(tx, clock)?;
            let (key, hash) = zero_fee_source_operation_identity(
                realm,
                &owner,
                source_kind,
                source_key,
                source_digest,
            )?;
            existing(tx, realm, &key, "zero_fee_source", &hash)
        },
    )
}

#[cfg(any(test, feature = "test-support"))]
pub fn record_zero_fee_source_authorized_with_source_with_test_fault<A, S>(
    client: &mut Client,
    realm: &str,
    source_kind: &str,
    source_key: &str,
    source_digest: &str,
    lock_timeout_ms: u32,
    statement_timeout_ms: u32,
    fault: TestFault,
    authorize: A,
    source: S,
) -> EconomyResult<Idempotent<ZeroFeeSourceReceipt>>
where
    A: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<WalletOwner>,
    S: FnOnce(&mut Transaction<'_>, SourceContext) -> EconomyResult<()>,
{
    let fault = match fault {
        TestFault::BeforeCommit => Fault::BeforeCommit,
        TestFault::CommitAcknowledgementLost => Fault::CommitAcknowledgementLost,
    };
    zero_impl(
        client,
        realm,
        source_kind,
        source_key,
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

    #[test]
    fn zero_fee_source_identity_shares_positive_slot_and_conflicts_on_payload() {
        let who = owner();
        let original =
            zero_fee_source_operation_identity("realm", &who, "npc_repair", "quote-1", "plan")
                .unwrap();
        let burn = TrustedBurn {
            source_kind: "npc_repair".into(),
            source_key: "quote-1".into(),
            layer: BurnLayer::Daily,
            gold_equivalent: 1,
            valuation_version: BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
        };
        let positive = bound_debit_burn_operation_identity("realm", &who, &burn, "plan").unwrap();
        assert_eq!(original.0, positive.0);
        assert_eq!(
            positive.0,
            digest(&(
                "bound_debit_burn_v1",
                "realm",
                &who,
                "npc_repair",
                "quote-1"
            ))
            .unwrap()
        );
        assert_ne!(original.1, positive.1);
        let changed =
            zero_fee_source_operation_identity("realm", &who, "npc_repair", "quote-1", "new-plan")
                .unwrap();
        assert_eq!(original.0, changed.0);
        assert_ne!(original.1, changed.1);
        for changed in [
            WalletOwner {
                account_id: "other".into(),
                ..who.clone()
            },
            WalletOwner {
                character_index: 1,
                ..who.clone()
            },
            WalletOwner {
                incarnation: "pg-character-v1:creation-two".into(),
                ..who.clone()
            },
        ] {
            assert_ne!(
                original.0,
                zero_fee_source_operation_identity(
                    "realm",
                    &changed,
                    "npc_repair",
                    "quote-1",
                    "plan"
                )
                .unwrap()
                .0
            );
        }
    }

    #[test]
    fn zero_fee_source_identity_rejects_invalid_tokens_and_reserved_fee() {
        for (kind, key, hash) in [
            ("conversion_fee", "quote", "plan"),
            ("", "quote", "plan"),
            ("npc_repair", "", "plan"),
            ("npc_repair", "quote", ""),
            (" npc_repair", "quote", "plan"),
            ("npc_repair", "quote\n", "plan"),
        ] {
            assert!(
                zero_fee_source_operation_identity("realm", &owner(), kind, key, hash).is_err()
            );
        }
        assert!(
            zero_fee_source_operation_identity("", &owner(), "npc_repair", "quote", "plan")
                .is_err()
        );
        let invalid = WalletOwner {
            character_index: -1,
            ..owner()
        };
        assert!(zero_fee_source_operation_identity(
            "realm",
            &invalid,
            "npc_repair",
            "quote",
            "plan"
        )
        .is_err());
    }
}
