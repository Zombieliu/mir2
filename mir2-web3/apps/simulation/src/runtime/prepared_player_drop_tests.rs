use super::*;
use crate::{AccountStoreDatabaseMode, SimulationConfig, ZoneSaveFence};
use mir2_protocol::ClientPacket;

fn session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}
fn bind_source(session: &mut SimulationSession) -> SimulationConfig {
    let mut config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url =
        Some("postgresql://unused.invalid/player-drop-source".into());
    let config = config
        .with_zone_save_fence(
            ZoneSaveFence::new("owner-group".into(), "owner-a".into(), 3).unwrap(),
        )
        .unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config = config.clone();
    config
}
fn binding() -> PlayerDropOperationBinding {
    PlayerDropOperationBinding::new(
        "original-nonce".into(),
        "rpc-session".into(),
        "demo".into(),
        0,
        "owner-group".into(),
        "owner-a".into(),
        3,
        17,
    )
    .unwrap()
}
fn placement(session: &SimulationSession) -> (ZoneKey, Point) {
    let world = session.app.world();
    let origin = entity_position(world, player_entity(world).unwrap()).unwrap();
    let position = super::super::drops::crystal_ground_drop_position(
        world,
        &origin,
        CRYSTAL_PLAYER_DROP_GOLD_RANGE,
    )
    .unwrap();
    (
        ZoneKey {
            shard_id: "s".into(),
            map_file_name: session.active_character_checkpoint().unwrap().map_file_name,
            channel_id: 0,
            instance_id: String::new(),
        },
        position,
    )
}
fn private_drop_count(session: &mut SimulationSession) -> usize {
    session
        .app
        .world_mut()
        .query::<&super::super::components::GroundDrop>()
        .iter(session.app.world())
        .count()
}

#[test]
fn player_drop_source_requires_original_trusted_postgres_authority() {
    let mut s = session();
    let (zone, position) = placement(&s);
    let before = s.active_character_checkpoint().unwrap();
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Gold { amount: 7 },
        |_| panic!("must not publish without source authority"),
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
}

#[test]
fn player_drop_rejection_stages_full_debit_without_private_spawn_or_live_debit() {
    let mut s = session();
    let config = bind_source(&mut s);
    let (zone, position) = placement(&s);
    let before = s.active_character_checkpoint().unwrap();
    let drops = private_drop_count(&mut s);
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Gold { amount: 7 },
        |source| {
            source.operation().validate_integrity().unwrap();
            assert_eq!(source.before().gold, before.gold);
            assert_eq!(source.checkpoint().gold, before.gold - 7);
            assert_eq!(source.checkpoint().revision, before.revision + 1);
            assert_eq!(
                source.before().inventory_items_json,
                source.checkpoint().inventory_items_json
            );
            assert!(source
                .checkpoint()
                .guild_experience_journal
                .applied_kill_receipts
                .is_empty());
            Err(PreparedPlayerDropPublicationFailure::Rejected(
                "rolled back".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    let after = s.active_character_checkpoint().unwrap();
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.revision, before.revision);
    assert_eq!(private_drop_count(&mut s), drops);
    config.ensure_account_store_writable().unwrap();
}

#[test]
fn player_drop_unknown_freezes_source_without_live_debit_or_compensation() {
    let mut s = session();
    let config = bind_source(&mut s);
    let (zone, position) = placement(&s);
    let before = s.active_character_checkpoint().unwrap();
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Gold { amount: 7 },
        |_| {
            Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                "commit response lost".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
    assert!(config.ensure_account_store_writable().is_err());
    let readonly_permit = config.zone_economy_write_permit().unwrap();
    assert_eq!(readonly_permit.fence().fencing_token(), 3);
}

#[test]
fn player_drop_false_written_receipt_is_unknown_and_never_installs_checkpoint() {
    let mut s = session();
    let config = bind_source(&mut s);
    let (zone, position) = placement(&s);
    let before = s.active_character_checkpoint().unwrap();
    let result = s.commit_shared_player_drop_via_postgres(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Gold { amount: 7 },
        |_| Ok(PreparedPlayerDropPublication::Written(())),
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
    assert!(config.ensure_account_store_writable().is_err());
}

#[test]
fn player_drop_checkpoint_installer_cannot_overwrite_newer_private_source() {
    let mut s = session();
    let old = s.active_character_checkpoint().unwrap();
    s.app
        .world()
        .resource::<super::super::resources::SessionResource>()
        .bind_active_save_revision(old.revision + 2);
    assert!(s.install_committed_player_drop_checkpoint(&old).is_err());
    assert_eq!(
        s.active_character_checkpoint().unwrap().revision,
        old.revision + 2
    );
}

#[test]
fn player_drop_publisher_panic_freezes_without_private_asset_changes() {
    let mut s = session();
    let config = bind_source(&mut s);
    let (zone, position) = placement(&s);
    let before = s.active_character_checkpoint().unwrap();
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Gold { amount: 7 },
        |_| panic!("worker lost before return"),
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
    assert!(config.ensure_account_store_writable().is_err());
}

#[test]
fn player_drop_retains_actual_durable_preimage_and_exact_working_source() {
    let mut s = session();
    let config = bind_source(&mut s);
    let canonical = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    s.app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold += 19;
    let working = s.active_character_checkpoint().unwrap();
    let (zone, position) = placement(&s);
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Gold { amount: 7 },
        |source| {
            assert_eq!(
                serde_json::to_value(source.before()).unwrap(),
                serde_json::to_value(&canonical).unwrap()
            );
            assert_eq!(source.runtime_before().gold, working.gold);
            assert_eq!(source.checkpoint().gold, working.gold - 7);
            source.operation().validate_integrity().unwrap();
            Err(PreparedPlayerDropPublicationFailure::Rejected(
                "ledger may reject unsaved economic difference".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(s.active_character_checkpoint().unwrap().gold, working.gold);
}

fn stack_item(s: &mut SimulationSession) -> u64 {
    let template = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|template| {
            let key = super::super::items::crystal_item_key_for_template(template);
            template.stack_size >= 5
                && !crystal_item_has_bind_flag(&key, CRYSTAL_BIND_DONT_DROP)
                && !crystal_item_has_bind_flag(&key, CRYSTAL_BIND_DESTROY_ON_DROP)
        })
        .expect("ordinary stackable Crystal fixture");
    let mut item = super::super::items::embedded_item_state_from_template(
        &template,
        crate::ItemContainer::Bag1,
        0,
    );
    item.unique_id = 765_001;
    item.quantity = 5;
    let uid = item.unique_id;
    s.app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items = vec![item];
    uid
}

#[test]
fn player_drop_split_freezes_exact_assigned_item_and_keeps_source_uid_on_remainder() {
    let mut s = session();
    let uid = stack_item(&mut s);
    bind_source(&mut s);
    let request = PlayerDropRequest::Item {
        unique_id: uid,
        count: 2,
        hero_inventory: false,
    };
    let preview = s.preview_shared_player_drop(&request).unwrap();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(mut exact),
                ..
            },
        ..
    } = preview
    else {
        panic!("item preview");
    };
    assert!(!exact.uid_assigned);
    assert_eq!(exact.item.unique_id, 0);
    exact.item.unique_id = 9_000_001;
    exact.uid_assigned = true;
    let expected = exact.clone();
    let (zone, position) = placement(&s);
    let result = s.commit_shared_player_drop_with_resolved_item_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        request,
        Some(exact),
        |source| {
            let PlayerDropOutcome::Item {
                loot:
                    GroundDropLootSnapshot::InventoryItem {
                        exact_item: Some(actual),
                        ..
                    },
                ..
            } = source.intent().outcome()
            else {
                panic!("frozen item");
            };
            assert_eq!(actual, &expected);
            let remainder: super::super::items::ItemState =
                serde_json::from_str(&source.checkpoint().inventory_items_json[0]).unwrap();
            assert_eq!(remainder.unique_id, uid);
            assert_eq!(remainder.quantity, 3);
            Err(PreparedPlayerDropPublicationFailure::Rejected(
                "rolled back".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(
        s.app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .quantity,
        5
    );
}

#[test]
fn player_drop_split_rejects_missing_identity_and_changed_attributes_before_publisher() {
    let mut s = session();
    let uid = stack_item(&mut s);
    bind_source(&mut s);
    let request = PlayerDropRequest::Item {
        unique_id: uid,
        count: 2,
        hero_inventory: false,
    };
    let (zone, position) = placement(&s);
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position.clone(),
        100,
        1_800_100,
        request.clone(),
        |_| panic!("split without reserved identity"),
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    let preview = s.preview_shared_player_drop(&request).unwrap();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(mut exact),
                ..
            },
        ..
    } = preview
    else {
        panic!("item preview");
    };
    exact.item.unique_id = 9_000_001;
    exact.uid_assigned = true;
    exact.item.gm_made = !exact.item.gm_made;
    let result = s.commit_shared_player_drop_with_resolved_item_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        request,
        Some(exact),
        |_| panic!("attributes were forged"),
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(
        s.app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .quantity,
        5
    );
}

fn journal_source(
    s: &SimulationSession,
) -> (PlayerDropPreparedOperation, crate::PlayerDropCommitReceipt) {
    let config = &s.app.world().resource::<RuntimeConfigResource>().config;
    let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    let runtime_before = super::super::save::snapshot_active_character_save(s.app.world()).unwrap();
    let mut after = runtime_before.clone();
    after.revision += 1;
    after.gold -= 7;
    let (zone, position) = placement(s);
    let intent = PlayerDropWorldIntent::new(
        binding(),
        zone,
        position,
        100,
        1_800_100,
        PlayerDropOutcome::Gold { amount: 7 },
    )
    .unwrap();
    let operation =
        PlayerDropPreparedOperation::new(intent, before, runtime_before, after).unwrap();
    let receipt = crate::PlayerDropCommitReceipt::new(
        operation.binding().operation_key(),
        operation.intent().world_effect_id().into(),
        operation.operation_digest().into(),
        operation.intent().intent_digest().into(),
        operation.checkpoint().revision,
        crate::PlayerDropEffectState::Live,
    );
    (operation, receipt)
}

#[test]
fn player_drop_replica_source_rejects_live_and_plain_memory_configs() {
    let mut s = session();
    let (operation, receipt) = journal_source(&s);
    let before = s.active_character_checkpoint().unwrap();
    assert!(s
        .apply_player_drop_replica_source(&operation, &receipt)
        .is_err());
    assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
    bind_source(&mut s);
    assert!(s
        .apply_player_drop_replica_source(&operation, &receipt)
        .is_err());
    assert_eq!(s.active_character_checkpoint().unwrap().gold, before.gold);
}

#[test]
fn player_drop_replica_source_installs_frozen_checkpoint_and_ack_without_debit_replay() {
    let source = session();
    let (operation, receipt) = journal_source(&source);
    let source_config = source
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let config = source_config.fork_for_replica_apply().unwrap();
    let mut replica = SimulationSession::new(config.clone());
    replica
        .restore_replica_session_checkpoint(&source.active_identity().unwrap(), operation.before())
        .unwrap();
    let packets = replica
        .apply_player_drop_replica_source(&operation, &receipt)
        .unwrap();
    assert_eq!(packets, vec![ServerPacket::LoseGold { gold: 7 }]);
    assert_eq!(
        replica.active_character_checkpoint().unwrap().gold,
        operation.checkpoint().gold
    );
    assert_eq!(
        replica.active_character_checkpoint().unwrap().revision,
        operation.checkpoint().revision
    );
    assert_eq!(
        serde_json::to_value(&config.account_store.lock().unwrap().accounts["demo"].saves[&0])
            .unwrap(),
        serde_json::to_value(operation.checkpoint()).unwrap()
    );
    assert_eq!(
        replica
            .apply_player_drop_replica_source(&operation, &receipt)
            .unwrap(),
        packets
    );
    assert_eq!(
        replica.active_character_checkpoint().unwrap().gold,
        operation.checkpoint().gold
    );
    assert!(replica.zone_economy_write_permit().is_err());
    assert_eq!(
        source.active_character_checkpoint().unwrap().gold,
        operation.runtime_before().gold
    );
}

#[test]
fn player_drop_replica_source_rejects_newer_isolated_save_without_installing_old_checkpoint() {
    let source = session();
    let (operation, receipt) = journal_source(&source);
    let config = source
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .fork_for_replica_apply()
        .unwrap();
    let mut replica = SimulationSession::new(config.clone());
    replica
        .restore_replica_session_checkpoint(&source.active_identity().unwrap(), operation.before())
        .unwrap();
    let mut newer = operation.checkpoint().clone();
    newer.revision += 1;
    newer.gold += 31;
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .insert(0, newer.clone());
    assert!(replica
        .apply_player_drop_replica_source(&operation, &receipt)
        .is_err());
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision,
        newer.revision
    );
    assert_eq!(
        replica.active_character_checkpoint().unwrap().revision,
        operation.before().revision
    );
}

#[test]
fn player_drop_recorded_request_binds_original_split_uid_count_and_owner_inventory() {
    let mut s = session();
    let source_uid = stack_item(&mut s);
    let request = PlayerDropRequest::Item {
        unique_id: source_uid,
        count: 2,
        hero_inventory: false,
    };
    let runtime_before = super::super::save::snapshot_active_character_save(s.app.world()).unwrap();
    let canonical_before = s
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts["demo"]
        .saves[&0]
        .clone();
    let mut checkpoint = runtime_before.clone();
    checkpoint.revision += 1;
    let mut remaining = s
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .clone();
    remaining[0].quantity -= 2;
    checkpoint.inventory_items_json = super::super::save::encode_state_vec(&remaining);
    let mut outcome = s.preview_shared_player_drop(&request).unwrap();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(exact),
                ..
            },
        ..
    } = &mut outcome
    else {
        panic!("split item preview");
    };
    exact.item.unique_id = 9_000_001;
    exact.uid_assigned = true;
    let (zone, position) = placement(&s);
    let intent =
        PlayerDropWorldIntent::new(binding(), zone, position, 100, 1_800_100, outcome).unwrap();
    let operation =
        PlayerDropPreparedOperation::new(intent, canonical_before, runtime_before, checkpoint)
            .unwrap();
    request.validate_operation(&operation).unwrap();
    for invalid in [
        PlayerDropRequest::Item {
            unique_id: 9_000_001,
            count: 2,
            hero_inventory: false,
        },
        PlayerDropRequest::Item {
            unique_id: source_uid,
            count: 3,
            hero_inventory: false,
        },
        PlayerDropRequest::Item {
            unique_id: source_uid,
            count: 2,
            hero_inventory: true,
        },
        PlayerDropRequest::Gold { amount: 2 },
    ] {
        assert!(invalid.validate_operation(&operation).is_err());
    }
}

#[test]
fn player_drop_hero_inventory_rejection_matches_existing_runtime_drop_policy() {
    let mut s = session();
    let uid = stack_item(&mut s);
    assert!(s.shared_inventory_item_drop(uid, 2, true).is_none());
    assert!(s
        .preview_shared_player_drop(&PlayerDropRequest::Item {
            unique_id: uid,
            count: 2,
            hero_inventory: true,
        })
        .is_err());
    assert!(s.shared_inventory_item_drop(uid, 2, false).is_some());
    assert!(s
        .preview_shared_player_drop(&PlayerDropRequest::Item {
            unique_id: uid,
            count: 2,
            hero_inventory: false,
        })
        .is_ok());
}

#[test]
fn player_drop_full_stack_preserves_exact_existing_uid_without_reservation() {
    let mut s = session();
    let uid = stack_item(&mut s);
    bind_source(&mut s);
    let (zone, position) = placement(&s);
    let result = s.commit_shared_player_drop_via_postgres::<(), _>(
        binding(),
        &zone,
        position,
        100,
        1_800_100,
        PlayerDropRequest::Item {
            unique_id: uid,
            count: 5,
            hero_inventory: false,
        },
        |source| {
            let PlayerDropOutcome::Item {
                loot:
                    GroundDropLootSnapshot::InventoryItem {
                        exact_item: Some(exact),
                        ..
                    },
                ..
            } = source.intent().outcome()
            else {
                panic!("frozen item");
            };
            assert!(exact.uid_assigned);
            assert_eq!(exact.item.unique_id, uid);
            assert!(source.checkpoint().inventory_items_json.is_empty());
            Err(PreparedPlayerDropPublicationFailure::Rejected(
                "rolled back".into(),
            ))
        },
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(
        s.app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .unique_id,
        uid
    );
}
