use super::*;

fn live_unowned_monster_count(packets: &[ServerPacket], object_id: u32) -> usize {
    packets
        .iter()
        .filter(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMonster { info }
                    if info.object_id == object_id
                        && info.master_object_id == 0
                        && !info.dead
            )
        })
        .count()
}

#[test]
fn harvested_zone_corpse_blocks_reactivated_personal_live_projection_until_zone_revive() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    runtime.inner =
        InProcessWorldRuntime::new(GatewayConfig::default().with_crystal_world_runtime());
    start_new_runtime(
        &mut runtime,
        "personal-corpse-projection",
        "CorpseProjection",
    );

    let native_ids = shared
        .lock()
        .expect("shared state should lock")
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map("0"))
        .into_iter()
        .map(|monster| monster.object_id)
        .collect::<BTreeSet<_>>();
    let target = runtime
        .inner
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| {
            entity.kind == WorldEntityKind::Monster
                && native_ids.contains(&entity.object_id)
                && matches!(entity.ai, Some(2 | 4))
                && !entity.dead
                && entity.hp.is_some_and(|hp| hp > 0)
                && runtime
                    .inner
                    .zone_monster_spawn_snapshot(entity.object_id)
                    .is_some_and(|spawn| spawn.respawn.is_some())
        })
        .expect("the real Crystal starter scene should expose a scheduled harvestable monster");
    let object_id = target.object_id;
    let session_id = runtime
        .current_zone_session_id()
        .expect("started runtime should own a Zone session");
    let key = runtime
        .current_presence_key()
        .expect("presence should exist");
    assert!(shared
        .lock()
        .expect("shared state should lock")
        .zone_manager
        .zone(&ZoneKey::for_map("0"))
        .is_some_and(|zone| zone.retains_object_id(object_id)));

    let died = ServerPacket::ObjectDied {
        info: ObjectDiedInfo {
            object_id,
            location: Point {
                x: target.x,
                y: target.y,
            },
            direction: target.direction,
            kind: 0,
        },
    };
    let harvested = ServerPacket::ObjectHarvested {
        movement: ObjectMovement {
            object_id,
            position: Point {
                x: target.x,
                y: target.y,
            },
            direction: target.direction,
        },
    };
    // A personal ObjectDied packet is deliberately not allowed to kill a
    // Zone-native monster. Drive the death through the authoritative Zone
    // attack path before applying the personal harvest acknowledgement.
    let death_now_ms = shared_gateway_now_ms();
    {
        let mut state = shared.lock().expect("shared state should lock");
        let mut outbounds = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: session_id.clone(),
            position: Point {
                x: target.x,
                y: target.y,
            },
            direction: target.direction,
        });
        outbounds.extend(
            state
                .zone_manager
                .handle(ZoneCommand::UpdatePlayerCombatStats {
                    session_id: session_id.clone(),
                    stats: mir2_simulation::ZonePlayerCombatStats {
                        min_dc: 10_000,
                        max_dc: 10_000,
                        accuracy: 100,
                        ..Default::default()
                    },
                }),
        );
        outbounds.extend(
            state
                .zone_manager
                .handle(ZoneCommand::sync_player_combat_state(
                    session_id.clone(),
                    MirClass::Warrior,
                    true,
                    false,
                    true,
                    false,
                    false,
                    false,
                )),
        );
        outbounds.extend(state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: session_id.clone(),
            object_id,
            direction: target.direction,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 10_000,
            now_ms: death_now_ms,
        }));
        outbounds.extend(state.zone_manager.tick_all(death_now_ms));
        let (death_packets, ..) = state.dispatch_zone_outbounds(outbounds, Some(&key));
        assert!(death_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectDied { info } if info.object_id == object_id
        )));

        let outbounds = state
            .zone_manager
            .handle(ZoneCommand::BroadcastSharedObjectPackets {
                session_id: session_id.clone(),
                local_self_object_id: runtime.local_self_object_id(),
                packets: vec![harvested.clone()],
                now_ms: death_now_ms.saturating_add(1),
            });
        let _ = state.dispatch_zone_outbounds(outbounds, Some(&key));
    }
    let lifecycle = vec![died, harvested];
    runtime
        .inner
        .apply_shared_monster_lifecycle_packets(&lifecycle);
    assert!(shared
        .lock()
        .expect("shared state should lock")
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map("0"))
        .iter()
        .any(|monster| monster.object_id == object_id && monster.dead));
    runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 10,
        }))
        .expect("the personal scene should observe the mirrored corpse");

    // Leaving the private activation ring despawns the mirrored corpse. Since
    // the personal spawn table deliberately has no respawn authority, returning
    // can materialise its dormant slot alive. This is the exact lossy projection
    // that must remain private while the Zone retains the harvested corpse.
    let far = Point {
        x: target.x.saturating_add(64),
        y: target.y,
    };
    runtime
        .inner
        .force_authoritative_player_transform(far, MirDirection::Left);
    runtime.inner.reconcile_current_map_monster_activation();
    assert!(!runtime
        .inner
        .world_snapshot()
        .entities
        .iter()
        .any(|entity| entity.object_id == object_id));
    runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 11,
        }))
        .expect("the personal scene should observe the corpse leaving its activation ring");
    runtime.inner.force_authoritative_player_transform(
        Point {
            x: target.x.saturating_add(1),
            y: target.y,
        },
        MirDirection::Left,
    );
    runtime.inner.reconcile_current_map_monster_activation();
    assert!(runtime
        .inner
        .world_snapshot()
        .entities
        .iter()
        .any(|entity| {
            entity.object_id == object_id && !entity.dead && entity.hp.is_some_and(|hp| hp > 0)
        }));

    let raw_personal = runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 12,
        }))
        .expect("the personal compatibility runtime should project its reactivated slot");
    assert_eq!(
        live_unowned_monster_count(&raw_personal, object_id),
        1,
        "the fixture must reproduce the raw personal live ghost before testing the Gateway filter"
    );
    let mut directly_filtered = raw_personal;
    runtime.suppress_personal_retained_monster_projections(&mut directly_filtered);
    assert_eq!(live_unowned_monster_count(&directly_filtered, object_id), 0);

    // Reset the personal scene boundary and reproduce the same activation once
    // more through the normal Gateway execute path.
    runtime.inner.force_authoritative_player_transform(
        Point {
            x: target.x.saturating_add(64),
            y: target.y,
        },
        MirDirection::Left,
    );
    runtime.inner.reconcile_current_map_monster_activation();
    runtime
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 2,
        }))
        .expect("the personal scene should observe the object leaving");
    runtime.inner.force_authoritative_player_transform(
        Point {
            x: target.x.saturating_add(1),
            y: target.y,
        },
        MirDirection::Left,
    );
    runtime.inner.reconcile_current_map_monster_activation();

    let filtered = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 3,
        }))
        .expect("normal serialized execution should succeed");
    assert_eq!(
        live_unowned_monster_count(&filtered, object_id),
        0,
        "a personal reactivation must not publish a live ghost over the retained Zone corpse"
    );

    let premature = runtime.apply_pending_zone_packets();
    assert!(
        !premature.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRevived { info } if info.object_id == object_id
        ) || matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.object_id == object_id && !info.dead
        )),
        "a positive personal slot must not revive the authoritative Zone corpse"
    );
    assert!(shared
        .lock()
        .expect("shared state should lock")
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map("0"))
        .iter()
        .any(|monster| monster.object_id == object_id && monster.dead));

    let (authoritative_revive, post_deadline_native) = {
        let mut state = shared.lock().expect("shared state should lock");
        let outbounds = state
            .zone_manager
            .tick_all(death_now_ms.saturating_add(86_400_000));
        let (packets, ..) = state.dispatch_zone_outbounds(outbounds, Some(&key));
        let native = state
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map("0"))
            .into_iter()
            .find(|monster| monster.object_id == object_id);
        (packets, native)
    };
    assert!(authoritative_revive.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRevived { info } if info.object_id == object_id
    )), "the scheduled Zone respawn must publish ObjectRevived after its deadline; packets={authoritative_revive:?}, native={post_deadline_native:?}");
}

#[test]
fn projection_filter_preserves_owner_pet_and_unadmitted_initial_monster() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    start_demo_runtime(&mut runtime);
    let retained_id = runtime
        .inner
        .world_snapshot()
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::Monster)
        .map(|entity| entity.object_id)
        .expect("starter scene should expose a retained monster");
    let retained = runtime
        .inner
        .zone_monster_spawn_snapshot(retained_id)
        .map(|spawn| zone_monster_spawn_packet(&spawn))
        .expect("retained monster should produce a projection");
    let mut pet = retained.clone();
    if let ServerPacket::ObjectMonster { info } = &mut pet {
        info.master_object_id = runtime.local_self_object_id().unwrap_or(1);
    }
    let unadmitted_id = retained_id.saturating_add(9_000_000);
    let mut unadmitted = retained.clone();
    if let ServerPacket::ObjectMonster { info } = &mut unadmitted {
        info.object_id = unadmitted_id;
    }
    let mut packets = vec![retained, pet, unadmitted];

    runtime.suppress_personal_retained_monster_projections(&mut packets);

    assert_eq!(live_unowned_monster_count(&packets, retained_id), 0);
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == retained_id && info.master_object_id != 0
    )));
    assert_eq!(live_unowned_monster_count(&packets, unadmitted_id), 1);
}
