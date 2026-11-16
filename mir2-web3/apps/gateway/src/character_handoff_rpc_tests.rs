//! Actual in-library host/RPC/journal handoff. No socket, listener or owner loop.
use super::*;
use crate::routing::{InMemoryZoneOwnerLeaseAuthority, ZoneOwnerLeaseAuthority};
use mir2_protocol::{ClientPacket, MirDirection, Point};
use mir2_simulation::WorldRuntime;
use std::sync::atomic::{AtomicU64, Ordering};

const TOKEN: &str = "owned-character-handoff-token";
const AOI_NPC: u32 = 49_987;

fn envelope(session: &str, zone: &ZoneId, request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: session.into(),
        zone_id: zone.as_str().into(),
        auth_token: Some(TOKEN.into()),
        request,
    }
}

fn execute(host: &ZoneHostServer, session: &str, lease: &ZoneOwnerLease, command: WorldCommand) {
    host.handle_envelope(envelope(
        session,
        lease.zone_id(),
        ZoneRpcRequest::Execute {
            owner_lease: lease.into(),
            mode: WireZoneOwnerCommandMode::Direct,
            command: WireWorldCommand::from_world(command).unwrap(),
        },
    ))
    .unwrap();
}

fn prepared(
    host: &ZoneHostServer,
    session: &str,
    lease: &ZoneOwnerLease,
) -> CharacterHandoffCheckpoint {
    let reply = host
        .handle_envelope(envelope(
            session,
            lease.zone_id(),
            ZoneRpcRequest::PrepareCharacterHandoff {
                owner_lease: lease.into(),
            },
        ))
        .unwrap();
    let ZoneRpcPayload::CharacterHandoff { checkpoint } = reply else {
        panic!("real handoff preparation reply")
    };
    *checkpoint
}

fn config() -> (GatewayConfig, std::path::PathBuf) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir().canonicalize().unwrap();
    let dir = base.join(format!(
        "mir2-character-handoff-rpc-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(dir.starts_with(&base));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("accounts.json");
    let mut config = GatewayConfig::default().with_account_store_path(&path);
    config.save_recovery_dir = None;
    config.visible_monsters.clear();
    config.map_hazards.clear();
    config.visible_npcs.push(mir2_simulation::VisibleNpcRecord {
        object_id: AOI_NPC,
        name: "Handoff AOI witness".into(),
        image: 1,
        colour_argb: -1,
        position: Point { x: 101, y: 100 },
        direction: MirDirection::Down,
        quest_ids: vec![],
        script_key: None,
    });
    (config, path)
}

fn hosted(
    host: &ZoneHostServer,
    session: &str,
    zone: &ZoneId,
) -> Arc<crate::routing::HostedZoneOwnerCommandClient> {
    host.sessions
        .lock()
        .unwrap()
        .get(&(session.into(), zone.as_str().into()))
        .unwrap()
        .hosted
        .clone()
}

fn snapshot_diagnostic(snapshot: &WorldSnapshot) -> serde_json::Value {
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer);
    let object = snapshot
        .entities
        .iter()
        .find(|entity| entity.object_id == AOI_NPC);
    serde_json::json!({"map": snapshot.map_file_name,
        "player": player.map(|entity| serde_json::json!({"x": entity.x, "y": entity.y, "id": entity.object_id})),
        "object": object.map(|entity| serde_json::json!({"x": entity.x, "y": entity.y,
            "id": entity.object_id, "name": entity.name, "kind": entity.kind})),
    })
}

fn install_actual_cooldown_and_held_refine(
    source: &crate::routing::HostedZoneOwnerCommandClient,
) -> u64 {
    let snapshot = source.world_snapshot().unwrap();
    let tick = snapshot.tick;
    assert!(tick > 0);
    let mut save = source.active_character_checkpoint().unwrap().unwrap();
    let mut skill: serde_json::Value = serde_json::from_str(&save.skill_states_json[0]).unwrap();
    skill["cooldown_ticks"] = serde_json::json!(120);
    skill["cooldown_ends_at"] = serde_json::json!(tick + 120);
    save.skill_states_json[0] = skill.to_string();
    let index = save
        .inventory_items_json
        .iter()
        .position(|encoded| {
            serde_json::from_str::<serde_json::Value>(encoded).unwrap()["key"] == "dagger"
        })
        .expect("actual carried starter weapon");
    let mut held: serde_json::Value =
        serde_json::from_str(&save.inventory_items_json.remove(index)).unwrap();
    let uid = held["unique_id"].as_u64().unwrap();
    assert!(uid > 0);
    let tooltip = snapshot
        .inventory_items
        .iter()
        .find(|item| item.unique_id == uid)
        .expect("the actual carried UID is in the Source snapshot")
        .tooltip_source
        .as_ref()
        .expect("the Source supplied the complete item tooltip");
    let wire = tooltip
        .user_item
        .as_ref()
        .expect("the Source supplied the real UserItem instance");
    assert_eq!(wire.unique_id, uid);
    assert_eq!(wire.item_index, tooltip.info.item_index);
    assert_eq!(held["quantity"].as_u64(), Some(u64::from(wire.count)));
    assert!(
        wire.slots.iter().all(Option::is_none),
        "this fixture uses an unsocketed starter weapon"
    );
    assert!(held["socketed"].as_array().unwrap().is_empty());
    // Use the real Source UserItem and its catalogue binding, following
    // ItemStateUserItemMetadata::from_hydrated_user_item. No identity or
    // instance-only fields are synthesized from a display name/default object.
    held["user_item_metadata"] = serde_json::json!({
        "item_index": wire.item_index, "awake_type": wire.awake_type,
        "awake_values": wire.awake_values, "refined_value": wire.refined_value,
        "refine_added": wire.refine_added, "refine_success_chance": wire.refine_success_chance,
        "wedding_ring": wire.wedding_ring, "expire_info": wire.expire_info,
        "rental_information": wire.rental_information, "sealed_info": wire.sealed_info,
        "slots": [], "is_shop_item": wire.is_shop_item, "gm_made": wire.gm_made,
        "live_socketed_at_capture": true, "socket_layout_hydrated": true,
        "captured_socket_positions": wire.slots.iter().map(|_| serde_json::Value::Null).collect::<Vec<_>>(),
        "captured_socket_position": null,
    });
    held["user_item_metadata"]["refine_added"] = serde_json::json!(1);
    let mut systems: mir2_simulation::Stage5SystemsState =
        serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
    systems.refine = mir2_simulation::Stage5RefineState {
        current_item: Some("dagger".into()),
        oven_item_state_json: Some(held.to_string()),
        remaining_ms: 120_000,
        clock_epoch: Some("previous-owned-source-process".into()),
        collect_deadline_ms: Some(180_000),
        refining: true,
        ..Default::default()
    };
    save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
    // Ordinary active-checkpoint restore validates the complete carrier and
    // rejects any alias retained in another inventory/equipment container.
    source.restore_active_character_checkpoint(&save).unwrap();
    uid
}

#[test]
fn handoff_actual_hosted_rpc_binds_saved_source_replays_verified_journal_and_retires_failed_target()
{
    let (config, path) = config();
    let source_zone = ZoneId::new("map:0");
    let target_zone = ZoneId::new("map:1");
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let source_lease = authority.owner_lease(&source_zone);
    let target_lease = authority.owner_lease(&target_zone);
    let source = ZoneHostServer::with_identity_and_factory(
        "handoff-source",
        4,
        config.clone(),
        authority.clone(),
        Some(TOKEN.into()),
        ZoneRpcLimits::default(),
        Arc::new(
            SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&source_zone),
        ),
    );
    let target_factory = Arc::new(
        SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&target_zone),
    );
    let warm_diagnostic = {
        // Populate a real retained object before the tested admission. The
        // ordinary bootstrap's include_owner:false avoids duplicate initial
        // viewport packets; it does not mark newly seeded objects visible to
        // that same owner. A later join must discover this controlled object
        // through the actual Zone AOI diff. This setup has no RPC journal.
        let mut world_setup = target_factory.create_runtime(config.clone(), &target_zone);
        world_setup
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            }))
            .unwrap();
        world_setup
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .unwrap();
        world_setup
            .execute(WorldCommand::TransferMap {
                key: "crystal:1:100:100".into(),
            })
            .unwrap();
        let transfer_only = snapshot_diagnostic(&world_setup.world_snapshot());
        // TransferMap rebuilds the native map pool; configured fixture NPCs
        // are loaded by the existing active-checkpoint bootstrap. Publish that
        // real scaffold through an ordinary NPC command's metadata sync before
        // any tested admission, rather than assuming a transfer seeded it.
        let scaffold = world_setup.active_character_checkpoint().unwrap();
        world_setup
            .restore_active_character_checkpoint(&scaffold)
            .unwrap();
        world_setup
            .execute(WorldCommand::Interact { object_id: AOI_NPC })
            .unwrap();
        let warm_snapshot = world_setup.world_snapshot();
        let warm_probe = crate::routing::character_handoff_test_access::runtime_presence(
            &world_setup,
            "demo",
            0,
        );
        let diagnostic = serde_json::json!({"transferOnly": transfer_only, "snapshot": snapshot_diagnostic(&warm_snapshot),
            "zone": warm_probe.diagnostic(AOI_NPC)});
        assert_eq!(
            warm_snapshot.map_file_name.as_deref(),
            Some("1"),
            "{diagnostic}"
        );
        assert_eq!(
            warm_probe.diagnostic(AOI_NPC)["playerPosition"],
            serde_json::json!({"x": 100, "y": 100}),
            "{diagnostic}"
        );
        world_setup
            .execute(WorldCommand::TransferMap {
                key: format!("crystal:0:{}:{}", config.spawn.x, config.spawn.y),
            })
            .unwrap();
        world_setup.save_active_character().unwrap();
        diagnostic
    };
    let target = ZoneHostServer::with_identity_and_factory(
        "handoff-target",
        4,
        config.clone(),
        authority.clone(),
        Some(TOKEN.into()),
        ZoneRpcLimits::default(),
        target_factory,
    );
    execute(
        &source,
        "source",
        &source_lease,
        // An ordinary password admission must remain a password account after
        // its cross-process continuation and verified standby reconstruction.
        WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }),
    );
    execute(
        &source,
        "source",
        &source_lease,
        WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
    );
    let isolated = config.fork_for_replica_apply().unwrap();
    let old = isolated.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    execute(
        &source,
        "source",
        &source_lease,
        WorldCommand::TransferMap {
            key: "crystal:1:100:100".into(),
        },
    );
    let held_uid =
        install_actual_cooldown_and_held_refine(&hosted(&source, "source", &source_zone));
    let handoff = prepared(&source, "source", &source_lease);
    let source_diagnostic = snapshot_diagnostic(
        &hosted(&source, "source", &source_zone)
            .world_snapshot()
            .unwrap(),
    );
    assert!(handoff.source_tick > 0);
    let skill: serde_json::Value =
        serde_json::from_str(&handoff.runtime.skill_states_json[0]).unwrap();
    assert!(skill["cooldown_ends_at"].as_u64().unwrap() > handoff.source_tick);
    assert_eq!(handoff.durable.map_file_name, "1");
    assert!(handoff.durable.revision > old.revision);
    let bytes = std::fs::read(&path).unwrap();
    let store: mir2_simulation::AccountStore = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        serde_json::to_value(&handoff.durable).unwrap(),
        serde_json::to_value(&store.accounts["demo"].saves[&0]).unwrap()
    );
    target
        .handle_envelope(envelope("target", &target_zone, ZoneRpcRequest::OnConnect))
        .unwrap();
    let target_runtime = hosted(&target, "target", &target_zone);
    let presence =
        crate::routing::character_handoff_test_access::hosted_presence(&target_runtime, "demo", 0);
    assert!(!presence.is_present());
    let resumed = target
        .handle_envelope(envelope(
            "target",
            &target_zone,
            ZoneRpcRequest::ResumeCharacterHandoff {
                owner_lease: (&target_lease).into(),
                checkpoint: Box::new(handoff.clone()),
            },
        ))
        .unwrap();
    let ZoneRpcPayload::CharacterHandoffCommitment { commitment } = resumed else {
        panic!("complete target readback")
    };
    handoff.validate_commitment(&commitment).unwrap();
    assert!(presence.is_present());
    let target_diagnostic = serde_json::json!({"snapshot": snapshot_diagnostic(&target_runtime.world_snapshot().unwrap()),
        "zone": presence.diagnostic(AOI_NPC)});
    let diagnostic = serde_json::json!({"warm": warm_diagnostic, "source": source_diagnostic,
        "checkpointPosition": handoff.runtime.position, "target": target_diagnostic});
    eprintln!("character handoff AOI diagnostic: {diagnostic}");
    assert_eq!(
        diagnostic["warm"]["zone"]["resource"], diagnostic["target"]["zone"]["resource"],
        "{diagnostic}"
    );
    assert_eq!(diagnostic["warm"]["zone"]["retained"], true, "{diagnostic}");
    assert_eq!(diagnostic["warm"]["zone"]["visible"], true, "{diagnostic}");
    assert_eq!(
        presence.has_visible_object(AOI_NPC),
        Some(true),
        "{diagnostic}"
    );
    assert_eq!(
        target_runtime.world_snapshot().unwrap().tick,
        handoff.source_tick
    );
    let skill: serde_json::Value =
        serde_json::from_str(&commitment.runtime.skill_states_json[0]).unwrap();
    assert!(skill["cooldown_ends_at"].as_u64().unwrap() > handoff.source_tick);
    let systems: mir2_simulation::Stage5SystemsState =
        serde_json::from_str(commitment.runtime.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert!(systems.refine.remaining_ms > 0 && !systems.refine.ready);
    let held: serde_json::Value =
        serde_json::from_str(systems.refine.oven_item_state_json.as_deref().unwrap()).unwrap();
    assert_eq!(held["unique_id"].as_u64(), Some(held_uid));
    assert_eq!(
        commitment.runtime.default_npc_events,
        handoff.durable.default_npc_events
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bytes,
        "target continuation cannot create another Login/save"
    );
    let journal_len = target.journal.lock().unwrap().entries.len();
    for mode in [
        WireZoneOwnerCommandMode::Direct,
        WireZoneOwnerCommandMode::ProductionPlayer {
            authenticated: true,
        },
    ] {
        assert!(target
            .handle_envelope(envelope(
                "target",
                &target_zone,
                ZoneRpcRequest::Execute {
                    owner_lease: (&target_lease).into(),
                    mode,
                    command: WireWorldCommand::ResumeCharacterHandoff {
                        checkpoint: Box::new(handoff.clone())
                    },
                }
            ))
            .is_err());
    }
    assert_eq!(target.journal.lock().unwrap().entries.len(), journal_len);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    // The standby still has the original map/row. Only the verified journal
    // batch carries the later saved baseline; no production store is attached.
    assert_eq!(
        isolated.account_store.lock().unwrap().accounts["demo"].saves[&0].map_file_name,
        old.map_file_name
    );
    let batch = target
        .handle_envelope(envelope(
            "target",
            &target_zone,
            ZoneRpcRequest::ExportMutationBatch {
                first_sequence: 0,
                max_entries: 32,
                max_payload_bytes: 4 * 1024 * 1024,
            },
        ))
        .unwrap();
    let ZoneRpcPayload::MutationBatch { batch } = batch else {
        panic!("actual digest-verified mutation batch")
    };
    let replica = ZoneHostServer::with_identity_and_factory(
        "handoff-replica",
        4,
        isolated.clone(),
        authority.clone(),
        Some(TOKEN.into()),
        ZoneRpcLimits::default(),
        Arc::new(
            SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&target_zone),
        ),
    );
    replica
        .handle_envelope(envelope(
            "replica-admin",
            &target_zone,
            ZoneRpcRequest::ApplyMutationBatch {
                batch: batch.clone(),
            },
        ))
        .unwrap();
    let installed_config = replica.config.lock().unwrap().clone();
    assert!(
        installed_config.account_store_path.is_none()
            && installed_config.account_store_database_url.is_none()
    );
    let mirrored =
        installed_config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(
        serde_json::to_value(&mirrored).unwrap(),
        serde_json::to_value(&handoff.durable).unwrap()
    );
    let replica_image =
        serde_json::to_value(&*installed_config.account_store.lock().unwrap()).unwrap();
    let (replica_journal, replica_head) = {
        let journal = replica.journal.lock().unwrap();
        (
            serde_json::to_value(&journal.entries).unwrap(),
            serde_json::to_value(journal.replication.head(target_zone.as_str())).unwrap(),
        )
    };
    let duplicate_error = replica
        .handle_envelope(envelope(
            "replica-admin",
            &target_zone,
            ZoneRpcRequest::ApplyMutationBatch { batch },
        ))
        .unwrap_err();
    assert_eq!(duplicate_error.code, "mutation_cursor_mismatch");
    let duplicate_config = replica.config.lock().unwrap().clone();
    assert_eq!(
        serde_json::to_value(&*duplicate_config.account_store.lock().unwrap()).unwrap(),
        replica_image
    );
    {
        let journal = replica.journal.lock().unwrap();
        assert_eq!(
            serde_json::to_value(&journal.entries).unwrap(),
            replica_journal
        );
        assert_eq!(
            serde_json::to_value(journal.replication.head(target_zone.as_str())).unwrap(),
            replica_head
        );
    }
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    let fresh = prepared(&source, "source", &source_lease);
    let fresh_bytes = std::fs::read(&path).unwrap();
    target
        .handle_envelope(envelope(
            "stale-target",
            &target_zone,
            ZoneRpcRequest::OnConnect,
        ))
        .unwrap();
    let retired = target
        .sessions
        .lock()
        .unwrap()
        .get(&("stale-target".into(), target_zone.as_str().into()))
        .unwrap()
        .hosted
        .clone();
    let stale_lease = ZoneOwnerLease::new(
        target_zone.clone(),
        "retired-owner",
        target_lease.fencing_token(),
    );
    assert!(target
        .handle_envelope(envelope(
            "stale-target",
            &target_zone,
            ZoneRpcRequest::ResumeCharacterHandoff {
                owner_lease: (&stale_lease).into(),
                checkpoint: Box::new(fresh.clone()),
            }
        ))
        .is_err());
    assert_eq!(retired.active_identity().unwrap(), None);
    assert!(target
        .handle_envelope(envelope(
            "stale-target",
            &target_zone,
            ZoneRpcRequest::ResumeCharacterHandoff {
                owner_lease: (&target_lease).into(),
                checkpoint: Box::new(handoff),
            }
        ))
        .is_err());
    assert!(
        retired.active_identity().is_err(),
        "a failed prepared target cannot keep active Source custody"
    );
    assert!(retired
        .execute_request(ZoneOwnerCommandRequest::direct(
            target_lease.clone(),
            WorldCommand::PasskeyLogin {
                account_id: "demo".into()
            }
        ))
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), fresh_bytes);
    target
        .handle_envelope(envelope(
            "fresh-target",
            &target_zone,
            ZoneRpcRequest::OnConnect,
        ))
        .unwrap();
    let resumed = target
        .handle_envelope(envelope(
            "fresh-target",
            &target_zone,
            ZoneRpcRequest::ResumeCharacterHandoff {
                owner_lease: (&target_lease).into(),
                checkpoint: Box::new(fresh.clone()),
            },
        ))
        .unwrap();
    let ZoneRpcPayload::CharacterHandoffCommitment { commitment } = resumed else {
        panic!("fresh target readback")
    };
    fresh.validate_commitment(&commitment).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), fresh_bytes);
    let mut password_relogin = mir2_simulation::InProcessWorldRuntime::new(config);
    assert!(password_relogin
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

#[test]
fn handoff_password_account_base_snapshot_restore_does_not_convert_credentials() {
    let (config, path) = config();
    let zone = ZoneId::new("map:0");
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let lease = authority.owner_lease(&zone);
    let source = ZoneHostServer::with_identity_and_factory(
        "password-base-source",
        4,
        config.clone(),
        authority.clone(),
        Some(TOKEN.into()),
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&zone)),
    );
    execute(
        &source,
        "password-session",
        &lease,
        WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }),
    );
    execute(
        &source,
        "password-session",
        &lease,
        WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
    );
    let bytes = std::fs::read(&path).unwrap();
    let account: mir2_simulation::AccountStore = serde_json::from_slice(&bytes).unwrap();
    let ZoneRpcPayload::BaseSnapshot { snapshot } = source
        .handle_envelope(envelope(
            "base-admin",
            &zone,
            ZoneRpcRequest::ExportBaseSnapshot,
        ))
        .unwrap()
    else {
        panic!("actual password-account base snapshot")
    };
    let replica = ZoneHostServer::with_identity_and_factory(
        "password-base-replica",
        4,
        config.clone(),
        authority,
        Some(TOKEN.into()),
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&zone)),
    );
    replica
        .handle_envelope(envelope(
            "base-admin",
            &zone,
            ZoneRpcRequest::InstallBaseSnapshot { snapshot },
        ))
        .unwrap();
    let restored = crate::routing::character_handoff_test_access::hosted_configuration(&hosted(
        &replica,
        "password-session",
        &zone,
    ));
    assert!(restored.account_store_path.is_none() && restored.account_store_database_url.is_none());
    assert_eq!(
        restored.account_store.lock().unwrap().accounts["demo"].password,
        account.accounts["demo"].password
    );
    assert_eq!(
        hosted(&replica, "password-session", &zone)
            .active_identity()
            .unwrap(),
        hosted(&source, "password-session", &zone)
            .active_identity()
            .unwrap()
    );
    assert!(
        std::fs::read(&path).unwrap() == bytes,
        "base restore wrote the live source File"
    );
    let mut password_relogin = mir2_simulation::InProcessWorldRuntime::new(config);
    assert!(password_relogin
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}
