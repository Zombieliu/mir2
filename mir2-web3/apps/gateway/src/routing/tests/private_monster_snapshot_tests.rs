//! Authoritative GetInfo visibility must also fence periodic client snapshots.
//! The ordinary private metadata source deliberately still contains the actor;
//! otherwise a packet-only fixture would miss the actual Zombie2 leak.
use super::*;

const MONSTER_ID: u32 = 9_750_024;

fn field_position(offset: i32) -> Point {
    Point { x: 390 + offset, y: 270 }
}

struct Fixture {
    runtime: SharedInProcessZoneSessionRuntime,
    shared: Arc<Mutex<SharedInProcessZoneState>>,
    key: ZonePresenceKey,
    session: SessionId,
    join: ZoneJoin,
    entity: WorldEntitySnapshot,
}

impl Fixture {
    fn new(class: MirClass, name: &str) -> Self {
        let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(shared.clone());
        start_new_runtime_with_class(
            &mut runtime, "private-monster-projection", "VisibilityProbe", class,
        );
        // Normal login starts in a safe zone. Relocate through the existing
        // trusted route before taking Join so its real combat/chat profile
        // describes the field; SyncPlayerTransform alone cannot change that
        // profile and Crystal FindNearby must ignore a safe-area player.
        runtime.execute(WorldCommand::ApplyHandoffTransform {
            position: field_position(0), direction: MirDirection::Right,
            hp: None, mp: None,
        }).unwrap();
        assert!(!runtime.world_snapshot().in_safe_zone);
        let key = runtime.current_presence_key().unwrap();
        let session = runtime.current_zone_session_id().unwrap();
        let mut join = runtime.inner.active_zone_join_snapshot(session.as_str()).unwrap();
        assert_eq!(join.position, field_position(0));
        assert!(!join.chat_profile.in_safe_zone);
        assert!(join.hp > 0);
        let template = mir2_game_data::crystal_monster_by_name(name).unwrap();
        let mut info = shared_monster_info(MONSTER_ID, 0);
        info.name = template.name;
        info.image = template.image;
        info.ai = template.ai;
        info.effect = template.effect;
        info.light = template.light;
        info.location = field_position(6);
        let mut entity = world_entity_from_monster_info(&info);
        entity.hp = Some(template.hp);
        entity.max_hp = Some(template.hp);
        entity.disposition = WorldEntityDisposition::Hostile;
        let spawn = zone_monster_spawn_from_shared_entity(&entity, 0).unwrap();
        {
            let mut state = shared.lock().unwrap();
            join.object_id = state.players[&key].zone_object_id;
            state.zone_manager = ZoneManager::new();
            assert!(state.zone_manager.install_empty_zone(ZoneRuntime::new_with_collision(
                ZoneKey::for_map("0"), ZoneCollision::unbounded(),
            )));
            state.zone_manager.handle(ZoneCommand::Join(join.clone()));
            state.update_player_transform(&key, join.position.clone(), join.direction);
            let out = state.zone_manager.handle(ZoneCommand::SpawnMonster {
                session_id: session.clone(), monster: spawn, now_ms: 0,
            });
            let (packets, ..) = state.dispatch_zone_outbounds(out, Some(&key));
            if matches!(info.ai, 5 | 14 | 24) {
                assert!(!packets.iter().any(|packet| matches!(packet,
                    ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID)));
            }
            // This is the richer personal snapshot metadata merge used in
            // production, not a forged public ObjectMonster packet.
            state.sync_map_layer("0".into(), vec![entity.clone()], BTreeSet::new(),
                Vec::new(), BTreeSet::new());
        }
        Self { runtime, shared, key, session, join, entity }
    }

    fn contains(&self) -> bool {
        self.runtime.world_snapshot().entities.iter().any(|e| e.object_id == MONSTER_ID)
    }

    fn move_to(&self, position: Point) -> Vec<ServerPacket> {
        let mut state = self.shared.lock().unwrap();
        let out = state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: self.session.clone(), position: position.clone(),
            direction: MirDirection::Right,
        });
        assert_eq!(state.zone_manager.player_transform(&self.session).unwrap().0, position,
            "the real Zone must reach the requested distance boundary");
        state.update_player_transform(&self.key, position, MirDirection::Right);
        state.dispatch_zone_outbounds(out, Some(&self.key)).0
    }

    fn tick(&self, now_ms: u64) -> Vec<ServerPacket> {
        let mut state = self.shared.lock().unwrap();
        let out = state.zone_manager.tick_all(now_ms);
        state.dispatch_zone_outbounds(out, Some(&self.key)).0
    }

    fn rejoin(&self, position: Point) {
        let mut state = self.shared.lock().unwrap();
        let out = state.zone_manager.handle(ZoneCommand::Leave {
            session_id: self.session.clone(),
        });
        state.dispatch_zone_outbounds(out, Some(&self.key));
        let mut join = self.join.clone();
        join.position = position.clone();
        let out = state.zone_manager.handle(ZoneCommand::Join(join));
        state.update_player_transform(&self.key, position, MirDirection::Right);
        state.dispatch_zone_outbounds(out, Some(&self.key));
    }

    fn assert_retained_private_metadata(&self) {
        let state = self.shared.lock().unwrap();
        assert!(state.maps["0"].entities.contains_key(&MONSTER_ID));
        let zone = state.zone_manager.zone(&ZoneKey::for_map("0")).unwrap();
        assert!(zone.native_monster_snapshots().iter().any(|m| m.object_id == MONSTER_ID));
        assert_eq!(zone.native_monster_is_visible(MONSTER_ID), Some(false));
    }
}

#[test]
fn shared_snapshot_omits_buried_zombie_until_real_three_cell_reveal() {
    let f = Fixture::new(MirClass::Wizard, "Zombie2");
    assert_eq!(f.entity.ai, Some(24));
    f.assert_retained_private_metadata();
    assert!(!f.contains(), "private snapshot-only Zombie2 must never enter the public client world");
    f.move_to(field_position(2)); // Four cells: inside AOI, outside FindNearby(3).
    assert!(!f.tick(1).iter().any(|p| matches!(p,
        ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
    assert!(!f.contains());
    f.move_to(field_position(3)); // Exactly three cells.
    assert!(!f.tick(2_001).iter().any(|p| matches!(p,
        ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
    let revealed = f.tick(2_002);
    let info = revealed.iter().position(|p| matches!(p,
        ServerPacket::ObjectMonster { info } if info.object_id == MONSTER_ID)).unwrap();
    let show = revealed.iter().position(|p| matches!(p,
        ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)).unwrap();
    assert!(info < show, "Crystal materializes the public actor before Show");
    assert!(f.contains(), "the real authoritative reveal must also admit the snapshot actor");
}

#[test]
fn shared_snapshot_visibility_survives_aoi_leave_rejoin_and_never_reburies_zombie() {
    let f = Fixture::new(MirClass::Taoist, "Zombie2");
    f.assert_retained_private_metadata();
    assert!(!f.contains());
    f.move_to(field_position(70));
    assert!(!f.contains());
    f.move_to(field_position(0));
    assert!(!f.contains(), "AOI reentry cannot materialize a still-buried monster");
    f.rejoin(field_position(0));
    f.assert_retained_private_metadata();
    assert!(!f.contains(), "late/rejoining clients inherit the authoritative burial state");
    f.move_to(field_position(3));
    assert!(f.tick(1).iter().any(|p| matches!(p,
        ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
    assert!(f.contains());
    let removed = f.move_to(field_position(70));
    assert!(removed.iter().any(|p| matches!(p,
        ServerPacket::ObjectRemove { object_id } if *object_id == MONSTER_ID)));
    assert!(!f.contains());
    f.rejoin(field_position(0));
    assert!(f.contains(), "DigOutZombie stays revealed after AOI loss and session rejoin");
}

#[test]
fn shared_snapshot_private_plants_do_not_hide_stone_zuma_or_owned_summon_metadata() {
    for name in ["CannibalPlant", "EvilCentipede"] {
        let f = Fixture::new(MirClass::Taoist, name);
        f.assert_retained_private_metadata();
        assert!(!f.contains(), "{name} GetInfo is private before its real reveal");
        if name == "CannibalPlant" {
            f.move_to(field_position(3));
            assert!(f.tick(1).iter().any(|p| matches!(p,
                ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
            assert!(f.contains());
            f.move_to(field_position(2)); // Still in AOI, now outside its three-cell trigger.
            assert!(f.tick(2_002).iter().any(|p| matches!(p,
                ServerPacket::ObjectHide { object_id } if *object_id == MONSTER_ID)));
            f.assert_retained_private_metadata();
            assert!(!f.contains(), "a real Hide must also withdraw the periodic snapshot actor");
            f.move_to(field_position(3));
            assert!(!f.tick(5_002).iter().any(|p| matches!(p,
                ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
            assert!(f.tick(5_003).iter().any(|p| matches!(p,
                ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
            assert!(f.contains(), "the later real Show restores public snapshot visibility");
        }
    }
    let f = Fixture::new(MirClass::Taoist, "ZumaGuardian");
    assert_eq!(f.entity.ai, Some(15));
    assert!(f.contains(), "stone Zuma are public even while immune");
    let summon_id = MONSTER_ID + 1;
    {
        let mut state = f.shared.lock().unwrap();
        let mut info = shared_monster_info(summon_id, f.join.object_id);
        info.location = field_position(1);
        state.apply_shared_entity_packets("0", &[ServerPacket::ObjectMonster { info }]);
    }
    let summon = f.runtime.world_snapshot().entities.into_iter()
        .find(|e| e.object_id == summon_id).expect("ordinary owned summon metadata remains public");
    assert_eq!(summon.owner_name.as_deref(), Some("VisibilityProbe"));
}

#[test]
fn shared_snapshot_map_transition_and_global_remove_do_not_resurrect_private_actors() {
    let mut f = Fixture::new(MirClass::Warrior, "Zombie2");
    f.assert_retained_private_metadata();
    let transfer = f.runtime.execute(WorldCommand::TransferMap {
        key: "crystal:0102:3:7".into(),
    }).unwrap(); // Internal bounded map-transition fixture; never a client debug command.
    assert!(transfer.iter().any(|p| matches!(p,
        ServerPacket::MapInformation { info } if info.file_name == "0102")));
    assert_eq!(f.runtime.world_snapshot().map_file_name.as_deref(), Some("0102"));
    assert!(!f.contains(), "the destination client snapshot cannot retain the source-map actor");
    f.runtime.execute(WorldCommand::TransferMap {
        key: "crystal:0:390:270".into(),
    }).unwrap();
    f.assert_retained_private_metadata();
    assert!(!f.contains(), "map reentry must not turn retained private metadata into a public actor");
    // CannibalPlant owns Crystal's harvestable-corpse expiry lifecycle. Use
    // that genuine removal rather than inventing a Zombie2 expiry policy.
    let f = Fixture::new(MirClass::Warrior, "CannibalPlant");
    assert!(!f.contains());
    f.move_to(field_position(5));
    assert!(f.tick(1).iter().any(|p| matches!(p,
        ServerPacket::ObjectShow { object_id } if *object_id == MONSTER_ID)));
    assert!(f.contains());
    let died = {
        let mut state = f.shared.lock().unwrap();
        state.zone_manager.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: f.session.clone(), stats: ZonePlayerCombatStats {
                min_dc: 10_000, max_dc: 10_000, accuracy: 100, ..Default::default()
            },
        });
        state.zone_manager.handle(ZoneCommand::sync_player_combat_state(
            f.session.clone(), MirClass::Warrior, true, false, true, false, false, false,
        ));
        let mut out = state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: f.session.clone(), object_id: MONSTER_ID,
            direction: MirDirection::Right, spell: 0, level: 0, attack_type: 0,
            damage: 10_000, now_ms: 3_000,
        });
        out.extend(state.zone_manager.tick_all(3_000));
        state.dispatch_zone_outbounds(out, Some(&f.key)).0
    };
    assert!(died.iter().any(|p| matches!(p,
        ServerPacket::ObjectDied { info } if info.object_id == MONSTER_ID)));
    let removed = f.tick(183_000);
    assert!(removed.iter().any(|p| matches!(p,
        ServerPacket::ObjectRemove { object_id } if *object_id == MONSTER_ID)));
    {
        let mut state = f.shared.lock().unwrap();
        state.sync_map_layer("0".into(), vec![f.entity.clone()], BTreeSet::new(),
            Vec::new(), BTreeSet::new());
        assert!(!state.maps["0"].entities.contains_key(&MONSTER_ID),
            "the existing global removal ledger must reject stale personal metadata");
    }
    assert!(!f.contains());
}
