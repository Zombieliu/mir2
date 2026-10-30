//! Successor-fenced PG recovery while the installed replica remains quiesced.
use super::*;
use crate::economy::{
    PostgresEconomyStore,
    player_drop::{PlayerDropOperationStatus, PlayerDropRecoveryHostHeads},
};
use crate::routing::SharedAccountInventoryExecutionContext;
use mir2_simulation::AccountStoreDatabaseMode;

const MAX_RECOVERY_OPERATIONS: usize = 1_000_000;

pub(super) struct RecoveredCanonicalZone {
    pub head: ZoneReplicationHead,
    pub valid_until: std::time::Instant,
}

fn recovery_identity(parts: &impl Serialize) -> Result<String, ZoneRpcFault> {
    let bytes = serde_json::to_vec(parts).map_err(|_| {
        ZoneRpcFault::new("canonical_recovery_encode", "recovery identity unavailable")
    })?;
    let mut hash = Sha256::new();
    hash.update(b"mir2.host.canonical-recovery.v1\0");
    hash.update(bytes);
    Ok(format!(
        "zone-recovery:{}",
        hex_lower_bytes(&hash.finalize())
    ))
}

impl ZoneHostServer {
    pub(super) fn recover_source_of_truth_replica(
        &self,
        zone_id: &ZoneId,
        lease: &ZoneOwnerLease,
        pre_head: &ZoneReplicationHead,
    ) -> Result<RecoveredCanonicalZone, ZoneRpcFault> {
        let factory = self
            .runtime_factory
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "Zone factory poisoned"))?
            .clone();
        if !factory.is_zone_replica(zone_id) || factory.autonomous_ticks_enabled(zone_id) {
            return Err(ZoneRpcFault::new(
                "canonical_recovery_not_quiesced",
                "canonical recovery requires an installed inactive replica",
            ));
        }
        self.quiesced_zones
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "quiesced Zone registry poisoned"))?
            .insert(zone_id.as_str().into());
        let source_config = self
            .config
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "Host configuration poisoned"))?
            .clone()
            .with_zone_save_fence(
                ZoneSaveFence::new(
                    zone_id.as_str().into(),
                    lease.owner_id().into(),
                    lease.fencing_token(),
                )
                .map_err(classify_runtime_error)?,
            )
            .map_err(classify_runtime_error)?;
        if source_config.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth {
            return Err(ZoneRpcFault::new(
                "canonical_recovery_mode",
                "canonical recovery requires PostgreSQL SourceOfTruth",
            ));
        }
        let database = source_config
            .account_store_database_url
            .as_ref()
            .ok_or_else(|| {
                ZoneRpcFault::new(
                    "canonical_recovery_database",
                    "canonical source database unavailable",
                )
            })?;
        let permit = source_config
            .zone_economy_write_permit()
            .map_err(classify_runtime_error)?;
        let store = PostgresEconomyStore::new(database);
        let abort_id = recovery_identity(&(
            zone_id.as_str(),
            lease.owner_id(),
            lease.fencing_token(),
            pre_head,
        ))?;
        let initial = store
            .read_zone_player_drop_recovery_set(zone_id.as_str(), MAX_RECOVERY_OPERATIONS)
            .map_err(classify_runtime_error)?;
        for record in initial.records() {
            match record.status() {
                PlayerDropOperationStatus::Prepared => {
                    store
                        .abort_prepared_player_drop_operation(
                            &permit,
                            record.operation().binding(),
                            &abort_id,
                            "quiesced successor canonical recovery; old writer fenced",
                        )
                        .map_err(classify_runtime_error)?;
                }
                PlayerDropOperationStatus::Committed => {
                    // The database, rather than the Host clock, decides expiry.
                    match store.expire_player_drop_effect(&permit, record) {
                        Ok(()) => (),
                        Err(detail) if detail == "player drop is not an expired live effect" => (),
                        Err(detail) => return Err(classify_runtime_error(detail)),
                    }
                }
                _ => (),
            }
        }
        let selected = self
            .sessions
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "Host sessions poisoned"))?
            .iter()
            .filter(|((_, group), _)| group == zone_id.as_str())
            .map(|(key, session)| (key.clone(), Arc::clone(session)))
            .collect::<BTreeMap<_, _>>();
        let set = store
            .read_zone_player_drop_recovery_set(zone_id.as_str(), MAX_RECOVERY_OPERATIONS)
            .map_err(classify_runtime_error)?;
        let mut accounts: BTreeSet<String> = set
            .required_characters()
            .iter()
            .map(|(account, _)| account.clone())
            .collect();
        for session in selected.values() {
            if let Some(identity) = session
                .hosted
                .active_identity()
                .map_err(classify_runtime_error)?
            {
                accounts.insert(identity.account_id);
            }
        }
        let accounts = accounts.into_iter().collect::<Vec<_>>();
        let context = SharedAccountInventoryExecutionContext {
            zone_id: zone_id.clone(),
            fencing_generation: lease.fencing_token(),
            source_sequence: pre_head.next_sequence,
            created_at_ms: unix_now_ms(),
            external_commit_authorized: true,
        };
        let first = source_config
            .prepare_zone_source_recovery(&accounts)
            .map_err(classify_runtime_error)?;
        source_config
            .install_zone_source_recovery_cache(&first)
            .map_err(classify_runtime_error)?;
        for session in selected.values() {
            session
                .hosted
                .recover_canonical_source(&first, &source_config, &context)
                .map_err(classify_runtime_error)?;
        }
        for session in selected.values() {
            let packets = session
                .hosted
                .recover_canonical_deliveries(&context)
                .map_err(|detail| {
                    // Delivery adapters can retain an unresolved projection
                    // after a lost SQL acknowledgement. Probe the shared
                    // source gate so its Unknown freeze reaches the Host.
                    match permit.validate_source_write_state() {
                        Err(source_error) => {
                            self.classify_zone_runtime_error(zone_id.as_str(), source_error)
                        }
                        Ok(()) => classify_runtime_error(detail),
                    }
                })?;
            let mut outbox = session
                .outbox
                .lock()
                .map_err(|_| ZoneRpcFault::new("internal", "recovery outbound queue poisoned"))?;
            for frame in encode_server_frames(packets)? {
                outbox.push(frame, session.max_outbound_messages);
            }
        }
        // Delivery can legitimately advance the source. Read the complete
        // source and CAS versions again; never certify the earlier image.
        let source = source_config
            .prepare_zone_source_recovery(&accounts)
            .map_err(classify_runtime_error)?;
        source_config
            .install_zone_source_recovery_cache(&source)
            .map_err(classify_runtime_error)?;
        for session in selected.values() {
            session
                .hosted
                .recover_canonical_source(&source, &source_config, &context)
                .map_err(classify_runtime_error)?;
        }
        let current = store
            .read_zone_player_drop_recovery_set(zone_id.as_str(), MAX_RECOVERY_OPERATIONS)
            .map_err(classify_runtime_error)?;
        if current
            .required_characters()
            .iter()
            .any(|(account, _)| !accounts.contains(account))
        {
            return Err(ZoneRpcFault::new(
                "canonical_recovery_set_changed",
                "custody subjects changed during source recovery",
            ));
        }
        let mut observation = unix_now_ms();
        for record in current.records() {
            if let Some((_, epoch, sequence)) = record.observation() {
                if epoch == lease.fencing_token() {
                    observation = observation.max(sequence);
                }
            }
        }
        let observation = observation.checked_add(1).ok_or_else(|| {
            ZoneRpcFault::new(
                "canonical_recovery_sequence",
                "recovery observation sequence exhausted",
            )
        })?;
        let proofs = current
            .records()
            .iter()
            .filter(|record| {
                !matches!(
                    record.status(),
                    PlayerDropOperationStatus::Prepared | PlayerDropOperationStatus::Aborted
                )
            })
            .map(|record| record.committed_effect(&permit, database))
            .collect::<Result<Vec<_>, _>>()
            .map_err(classify_runtime_error)?;
        factory
            .reconcile_player_drop_recovery(zone_id, &proofs, unix_now_ms())
            .map_err(classify_runtime_error)?;
        for record in current.records() {
            if !matches!(
                record.status(),
                PlayerDropOperationStatus::Prepared | PlayerDropOperationStatus::Aborted
            ) {
                store
                    .acknowledge_player_drop_effect(&permit, record, observation)
                    .map_err(classify_runtime_error)?;
            }
        }
        let expected = store
            .read_zone_player_drop_recovery_set(zone_id.as_str(), MAX_RECOVERY_OPERATIONS)
            .map_err(classify_runtime_error)?;
        // Runtime checkpoints have same-session clock representations. Exact
        // canonical PG data is bound separately by the opaque source read;
        // the receipt validates the complete private projection, not raw clocks.
        let commitments = session_commitments(&selected)?;
        let snapshot = ZoneBaseSnapshot::new(
            zone_id.as_str().into(),
            pre_head.build_id.clone(),
            pre_head.next_sequence,
            pre_head.latest_digest.clone(),
            factory
                .zone_checkpoint_bytes(zone_id)
                .map_err(classify_runtime_error)?,
            commitments,
        )?;
        let heads = PlayerDropRecoveryHostHeads::new(
            self.host_id.clone(),
            pre_head.clone(),
            snapshot.clone(),
        )
        .map_err(classify_runtime_error)?;
        let recovery_id = recovery_identity(&(
            abort_id,
            source.source_digest(),
            expected.operation_set_digest(),
            snapshot.snapshot_id.as_str(),
            observation,
        ))?;
        let receipt = store
            .commit_zone_player_drop_recovery(
                &permit,
                &source,
                &expected,
                &heads,
                &recovery_id,
                observation,
                5_000,
            )
            .map_err(|detail| {
                self.classify_zone_runtime_error(
                    zone_id.as_str(),
                    format!("{detail}; recovery_id={recovery_id}"),
                )
            })?;
        self.owner_lease_authority
            .validate_owner_lease(lease)
            .map_err(classify_runtime_error)?;
        let reset = WireHostJournalEntry {
            sequence: 0,
            session_id: "__canonical_recovery__".into(),
            zone_id: zone_id.as_str().into(),
            owner_lease: lease.into(),
            mode: WireZoneOwnerCommandMode::Direct,
            command: Some(WireWorldCommand::CanonicalZoneRecovery {
                snapshot: Box::new(snapshot.clone()),
            }),
            closed: false,
            zone_tick_ms: None,
        };
        reset.validate_canonical_zone_recovery(&snapshot, pre_head)?;
        let mut journal = self
            .journal
            .lock()
            .map_err(|_| ZoneRpcFault::new("internal", "Host journal poisoned"))?;
        if !replication_heads_match(&journal.replication.head(zone_id.as_str()), pre_head) {
            return Err(ZoneRpcFault::new(
                "canonical_recovery_head_changed",
                "replica journal changed during canonical recovery",
            ));
        }
        let mut staged = journal.clone();
        staged.compact_to_base(&snapshot)?;
        let staging = Mutex::new(staged);
        append_host_journal(&staging, reset)?;
        let staged = staging
            .into_inner()
            .map_err(|_| ZoneRpcFault::new("internal", "staged recovery journal poisoned"))?;
        let recovered = staged.replication.head(zone_id.as_str());
        // Compression, full checkpoint validation and reset hashing are already
        // finished. Recheck PG source/custody/time just before publication.
        let validation_started = std::time::Instant::now();
        let remaining_ms = store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &receipt, &heads)
            .map_err(classify_runtime_error)?;
        let valid_until = validation_started
            .checked_add(Duration::from_millis(remaining_ms))
            .ok_or_else(|| {
                ZoneRpcFault::new(
                    "canonical_recovery_deadline",
                    "recovery deadline unavailable",
                )
            })?;
        *journal = staged;
        Ok(RecoveredCanonicalZone {
            head: recovered,
            valid_until,
        })
    }
}
