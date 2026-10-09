use super::*;
use mir2_simulation::{
    PreparedKillAccountSource, PreparedKillPublication, PreparedKillPublicationFailure,
    ZoneMonsterKillAward,
};

struct SimulationSource<'a> {
    source: &'a PreparedKillAccountSource,
    identity: ActiveSessionIdentity,
    legs: Vec<EconomyLeg>,
    expected_before: BTreeMap<EconomyBalanceKey, i64>,
}
impl PreparedKillSource for SimulationSource<'_> {
    fn identity(&self) -> &ActiveSessionIdentity {
        &self.identity
    }
    fn award_hash(&self) -> &str {
        self.source.award_hash()
    }
    fn expected_legs(&self) -> &[EconomyLeg] {
        &self.legs
    }
    fn expected_before(&self) -> &BTreeMap<EconomyBalanceKey, i64> {
        &self.expected_before
    }
    fn lock_guilds(&self, tx: &mut Transaction<'_>) -> Result<(), String> {
        self.source.lock_postgres_guilds(tx)
    }
    fn publish_source(&self, tx: &mut Transaction<'_>) -> Result<PreparedKillProjection, String> {
        let checkpoint = self.source.write_postgres(tx)?;
        PreparedKillProjection::new(
            self.identity.clone(),
            self.source.award_hash().into(),
            &checkpoint,
        )
    }
}

fn balances(
    save: &mir2_simulation::CharacterSaveRecord,
) -> Result<BTreeMap<(String, String), i64>, String> {
    #[derive(Deserialize)]
    struct ItemQuantity {
        key: String,
        quantity: u32,
    }
    let mut result = BTreeMap::from([
        (("experience".into(), "experience".into()), save.experience),
        (("gold".into(), "gold".into()), i64::from(save.gold)),
    ]);
    // Same carrier set as the existing ledger bootstrap. Complete ItemState
    // remains in the source checkpoint; these are the established quantity rows.
    for encoded in save
        .inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .chain(&save.storage_items_json)
        .chain(&save.hero_inventory_items_json)
        .chain(&save.equipment_items_json)
    {
        let item: ItemQuantity = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        if item.key.trim().is_empty() {
            return Err("prepared kill item key missing".into());
        }
        let amount = result
            .entry(("item_quantity".into(), item.key))
            .or_default();
        *amount = amount
            .checked_add(i64::from(item.quantity))
            .ok_or("prepared kill quantity overflow")?;
    }
    Ok(result)
}

fn source_legs(source: &PreparedKillAccountSource) -> Result<Vec<EconomyLeg>, String> {
    let before = balances(source.before())?;
    let after = balances(source.checkpoint())?;
    let keys = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    keys.into_iter()
        .filter_map(|(kind, key)| {
            let pair = (kind.clone(), key.clone());
            let delta = after
                .get(&pair)
                .copied()
                .unwrap_or(0)
                .checked_sub(before.get(&pair).copied().unwrap_or(0));
            match delta {
                Some(0) => None,
                Some(delta) => Some(Ok(EconomyLeg {
                    balance: EconomyBalanceKey {
                        account_id: source.account_id().into(),
                        character_index: source.character_index(),
                        asset_kind: kind,
                        asset_key: key,
                    },
                    delta,
                })),
                None => Some(Err("prepared kill balance difference overflow".into())),
            }
        })
        .collect()
}

impl PostgresEconomyStore {
    /// The original award hash is sufficient to recover the old *complete*
    /// outcome. Never run gameplay just to reconstruct the old balance legs.
    fn lookup_kill_identity(
        &self,
        key: &str,
        identity: &ActiveSessionIdentity,
        hash: &str,
    ) -> Result<Option<(EconomyTransactionReceipt, PreparedKillProjection)>, String> {
        let mut client = self.connect()?;
        mir2_simulation::apply_migrations(&mut client)?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&key],
        )
        .map_err(|e| e.to_string())?;
        let row=tx.query_opt("SELECT tx.receipt,outbox.payload FROM game_economy_transactions tx JOIN game_economy_outbox outbox ON outbox.event_id=tx.event_id WHERE tx.idempotency_key=$1",&[&key]).map_err(|e|e.to_string())?;
        row.map(|row| {
            let receipt:EconomyTransactionReceipt=serde_json::from_value(row.get(0)).map_err(|e|e.to_string())?;
            let event:EconomyOutboxEvent=serde_json::from_value(row.get(1)).map_err(|e|e.to_string())?;
            validate_stored_economy_transaction(&event,&receipt)?;
            let projection=event.source_checkpoint.clone().ok_or("kill receipt predates complete source transaction; requires explicit reconciliation")?;
            if projection.identity.account_id!=identity.account_id || projection.identity.character_index!=identity.character_index || projection.award_hash!=hash {
                return Err("prepared kill identity or award hash conflict".into());
            }
            Ok((duplicate_receipt_from_stored(&event,&receipt,&event.envelope)?,projection))
        }).transpose()
    }

    pub(in crate::economy) fn commit_runtime_kill(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: &SharedAccountInventoryExecutionContext,
        identity: &ActiveSessionIdentity,
        key: &str,
        award: &ZoneMonsterKillAward,
    ) -> Result<(EconomyTransactionReceipt, Vec<ServerPacket>), PreparedKillPublicationFailure>
    {
        if runtime.active_identity().as_ref() != Some(identity) {
            return Err("prepared kill runtime identity mismatch".to_string().into());
        }
        let hash = hex_lower(&Sha256::digest(
            serde_json::to_vec(award).map_err(|e| e.to_string())?,
        ));
        if let Some((receipt, _)) = self.lookup_kill_identity(key, identity, &hash)? {
            runtime.reconcile_shared_monster_kill_receipt(key, &hash)?;
            return Ok((receipt, vec![]));
        }
        if !context.external_commit_authorized {
            return Err("standby cannot publish a new kill source"
                .to_string()
                .into());
        }
        runtime.commit_shared_monster_kill_via_postgres(key, award, |source| {
            if source.database_url() != self.database_url {
                return Err(
                    "kill ledger and source require the same PostgreSQL connection configuration"
                        .to_string()
                        .into(),
                );
            }
            let legs = source_legs(source)?;
            let before = balances(source.before())?;
            let expected_before = legs
                .iter()
                .map(|leg| {
                    (
                        leg.balance.clone(),
                        before
                            .get(&(
                                leg.balance.asset_kind.clone(),
                                leg.balance.asset_key.clone(),
                            ))
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect();
            let participant = SimulationSource {
                source,
                identity: identity.clone(),
                legs,
                expected_before,
            };
            let envelope = EconomyTransactionEnvelope {
                idempotency_key: key.into(),
                transaction_kind: EconomyTransactionKind::Adjustment,
                zone_id: context.zone_id.as_str().to_string(),
                fencing_generation: context.fencing_generation,
                source_sequence: context.source_sequence,
                created_at_ms: context.created_at_ms,
                legs: participant.legs.clone(),
                metadata: BTreeMap::from([
                    ("operation".into(), "preparedMonsterKillExperience".into()),
                    ("awardHash".into(), hash.clone()),
                    ("sourceAccount".into(), identity.account_id.clone()),
                    (
                        "sourceCharacter".into(),
                        identity.character_index.to_string(),
                    ),
                ]),
            };
            match self.transact_prepared_kill(&envelope, &participant) {
                Ok((receipt, _)) if receipt.duplicate => {
                    Ok(PreparedKillPublication::AlreadyCommitted(receipt))
                }
                Ok((receipt, _)) => Ok(PreparedKillPublication::Written(receipt)),
                Err(error) if error.starts_with("PREPARED_KILL_COMMIT_OUTCOME_UNKNOWN:") => {
                    Err(PreparedKillPublicationFailure::OutcomeUnknown(error))
                }
                Err(error) => Err(PreparedKillPublicationFailure::Rejected(error)),
            }
        })
    }
}
