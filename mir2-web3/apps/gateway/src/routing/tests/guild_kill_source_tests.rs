use super::*;

fn award() -> ZoneMonsterKillAward {
    ZoneMonsterKillAward {
        monster_object_id: 912345,
        killed_at_ms: 1000,
        monster_name: "Scarecrow".into(),
        experience: 6,
        drops: vec![],
        boss_audit: None,
        experience_selection: None,
        source_receipt_key: None,
    }
}

#[test]
fn guild_kill_source_known_failure_retains_envelope_and_new_service_replays_durable_receipt() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(state.clone());
    start_new_runtime(&mut runtime, "guild-kill-source", "SourceKeeper");
    let identity = runtime.inner.active_identity().unwrap();
    let key = super::super::ZonePresenceKey::from_identity(&identity);
    let config = runtime.inner.shared_mentor_config().unwrap();
    let before = runtime.inner.world_snapshot().player_experience;
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    assert!(runtime
        .apply_zone_monster_kill_awards(vec![award()])
        .is_empty());
    assert_eq!(runtime.inner.world_snapshot().player_experience, before);
    let pending = state
        .lock()
        .unwrap()
        .take_pending_zone_monster_kill_awards(&key);
    assert_eq!(pending, vec![award()]);
    let packets = runtime.apply_zone_monster_kill_awards(pending);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { amount: 6 })));
    let after = runtime.inner.world_snapshot().player_experience;
    let fixed_key = config.account_store.lock().unwrap().accounts[&identity.account_id].saves
        [&identity.character_index]
        .guild_experience_journal
        .applied_kill_receipts
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut fixed_award = award();
    fixed_award.source_receipt_key = Some(fixed_key);
    runtime.account_inventory_service = Arc::new(InProcessAccountInventoryService::new());
    runtime.inventory_zone_id = ZoneId::new("different-delivery-zone");
    assert!(runtime
        .apply_zone_monster_kill_awards(vec![fixed_award])
        .is_empty());
    assert_eq!(runtime.inner.world_snapshot().player_experience, after);
    assert!(state
        .lock()
        .unwrap()
        .take_pending_zone_monster_kill_awards(&key)
        .is_empty());
}

#[derive(Debug, Default)]
struct UnknownSource {
    keys: Mutex<Vec<String>>,
}
impl SharedAccountInventoryService for UnknownSource {
    fn commit(
        &self,
        _: &mut InProcessWorldRuntime,
        _: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        panic!("source unknown must retain its typed outcome");
    }
    fn commit_fenced(
        &self,
        _: &mut InProcessWorldRuntime,
        _: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        let key = match &envelope.command {
            SharedAccountInventoryCommand::MonsterKillAward(award) => award
                .source_receipt_key
                .clone()
                .unwrap_or_else(|| envelope.idempotency_key().unwrap().0),
            _ => unreachable!(),
        };
        let mut keys = self.keys.lock().unwrap();
        keys.push(key.clone());
        let receipt = SharedAccountInventoryTransactionReceipt {
            kind: SharedAccountInventoryTransactionKind::MonsterKillAward,
            committed: false,
            packets: vec![],
        };
        if keys.len() == 1 {
            SharedAccountInventoryCommitOutcome::LocalSourceOutcomeUnknown {
                idempotency_key: key,
                receipt,
            }
        } else {
            SharedAccountInventoryCommitOutcome::Deferred { receipt }
        }
    }
}

#[test]
fn guild_kill_source_unknown_keeps_original_pending_reward_and_key_without_success_packets() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let service = Arc::new(UnknownSource::default());
    let mut runtime =
        shared_session_runtime_with_account_inventory_service(state.clone(), service.clone());
    start_new_runtime(&mut runtime, "guild-kill-unknown", "PendingKeeper");
    let key =
        super::super::ZonePresenceKey::from_identity(&runtime.inner.active_identity().unwrap());
    let before = runtime.inner.world_snapshot().player_experience;
    assert!(runtime
        .apply_zone_monster_kill_awards(vec![award()])
        .is_empty());
    let pending = state
        .lock()
        .unwrap()
        .take_pending_zone_monster_kill_awards(&key);
    let mut expected = award();
    expected.source_receipt_key = Some(service.keys.lock().unwrap()[0].clone());
    assert_eq!(pending, vec![expected.clone()]);
    // Checkpoint/reconnect transport and a different delivery namespace cannot
    // replace the first attempted receipt identity.
    let pending =
        serde_json::from_slice::<Vec<ZoneMonsterKillAward>>(&serde_json::to_vec(&pending).unwrap())
            .unwrap();
    runtime.inventory_zone_id = ZoneId::new("replacement-delivery-zone");
    assert!(runtime.apply_zone_monster_kill_awards(pending).is_empty());
    assert_eq!(
        state
            .lock()
            .unwrap()
            .take_pending_zone_monster_kill_awards(&key),
        vec![expected]
    );
    assert_eq!(runtime.inner.world_snapshot().player_experience, before);
    let keys = service.keys.lock().unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], keys[1]);
}
