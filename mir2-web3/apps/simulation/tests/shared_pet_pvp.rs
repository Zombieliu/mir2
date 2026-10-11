//! Real summons and two independent player lives, through public Zone commands.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, ZoneChatProfile, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager, ZoneOutbound,
    ZoneRuntime,
};

// Ordinary bundled terrain. Each map has its own fixed translation of the
// original logical fixture coordinates; real cross-map proofs remain distinct.
const MAP_A: &str = "D024";
const MAP_B: &str = "D022";
const MAP_C: &str = "D023";

fn map_point(map: &str, x: i32, y: i32) -> Point {
    let (dx, dy) = match map {
        MAP_A => (36, 12),
        MAP_B => (249, -4),
        MAP_C => (225, -14),
        _ => panic!("unknown prepared map"),
    };
    Point {
        x: x + dx,
        y: y + dy,
    }
}

fn ordinary_zone(map: &str) -> ZoneRuntime {
    let zone = ZoneRuntime::new(ZoneKey::for_map(map));
    assert!(zone.has_available_collision(), "ordinary terrain {map}");
    zone
}

fn saved_pet_manager() -> ZoneManager {
    let mut config = mir2_simulation::ZoneNpcTeleportConfig::disabled(0);
    for (index, name, no_pets) in [
        (1, MAP_A, false),
        (2, MAP_B, false),
        (3, MAP_C, true),
    ] {
        config.maps.insert(
            name.into(),
            mir2_simulation::ZoneMapMetadata {
                map_index: index,
                file_name: name.into(),
                title: name.into(),
                mini_map: 0,
                big_map: 0,
                lights: 0,
                map_dark_light: 0,
                music: 0,
                weather: 0,
                fight: false,
                no_fight: false,
                no_experience: false,
                no_group: false,
                no_pets,
            },
        );
    }
    let mut manager = ZoneManager::new();
    for name in [MAP_A, MAP_B, MAP_C] {
        let zone = ZoneRuntime::new_with_collision_and_npc_teleport_config(
            ZoneKey::for_map(name),
            mir2_simulation::ZoneCollision::for_map(name),
            config.clone(),
        );
        assert!(zone.has_available_collision(), "ordinary terrain {name}");
        assert!(manager.install_empty_zone(zone));
    }
    manager
}

fn saved_pet_join(name: &str, id: u32, map: &str, position: Point, mode: u8) -> ZoneJoin {
    // Callers supply the shared logical p() geometry on MAP_A; retain that
    // geometry when admitting the actor into a different real map.
    let position = map_point(map, position.x - 36, position.y - 12);
    let mut player = join(name, id, position);
    player.map_file_name = map.into();
    player.chat_profile.pet_mode = mode;
    player
}

fn manager_summon(manager: &mut ZoneManager, spell: Spell, now: u64) -> u32 {
    let map = manager.zone_key_for_session(&SessionId::new("owner")).unwrap();
    let target = map_point(&map.map_file_name, 11, 20);
    manager.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell,
        direction: MirDirection::Right,
        target,
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: now,
    });
    let spawned = manager.tick_all(now + 500);
    packets(&spawned, "owner")
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name
                    == if spell == Spell::SummonShinsu {
                        "Shinsu"
                    } else {
                        "BoneFamiliar"
                    } =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("ordinary accepted source spell creates its real pet")
}

fn saved_pet_records(snapshot: &mir2_simulation::ZoneSavedPetSnapshot) -> Vec<serde_json::Value> {
    serde_json::to_value(snapshot).unwrap()["pets"]
        .as_array()
        .unwrap()
        .clone()
}

fn sorted_saved_pet_records(snapshot: &mir2_simulation::ZoneSavedPetSnapshot) -> Vec<String> {
    let mut records = saved_pet_records(snapshot)
        .into_iter()
        .map(|record| serde_json::to_string(&record).unwrap())
        .collect::<Vec<_>>();
    records.sort();
    records
}

#[test]
fn shared_owned_pet_manager_normal_map_transfer_preserves_source_modes_and_real_master() {
    let sid = SessionId::new("owner");
    let a = ZoneKey::for_map(MAP_A);
    let b = ZoneKey::for_map(MAP_B);
    for mode in [0, 1, 2, 3, 4] {
        let mut manager = saved_pet_manager();
        manager.join(saved_pet_join("owner", 101, MAP_A, p(10, 20), mode));
        manager.join(saved_pet_join("observer", 102, MAP_A, p(12, 20), 0));
        let pet = manager_summon(&mut manager, Spell::SummonSkeleton, 10);
        let original = manager
            .native_monster_snapshots(&a)
            .into_iter()
            .find(|m| m.object_id == pet)
            .unwrap();
        let saved = manager.capture_player_saved_pets(&sid, 600);
        let online = manager.online_owner_proof_for_session(&sid).unwrap();
        let life = manager.player_life_generation(&sid).unwrap();
        manager.join(saved_pet_join("owner", 101, MAP_B, p(40, 50), mode));
        let recalled = manager.refresh_online_pets_at(620);
        assert_eq!(
            manager.online_owner_proof_for_session(&sid).as_ref(),
            Some(&online)
        );
        assert_eq!(manager.player_life_generation(&sid), Some(life));
        assert!(
            manager.zone(&a).unwrap().player_vitals(&sid).is_none(),
            "cached Master is never a second world player"
        );
        let follows = matches!(mode, 0 | 1 | 4);
        assert_eq!(
            manager.native_monster_snapshots(&a).len(),
            usize::from(!follows),
            "mode {mode}"
        );
        assert_eq!(
            manager.native_monster_snapshots(&b).len(),
            usize::from(follows),
            "mode {mode}"
        );
        if follows {
            let current = &manager.native_monster_snapshots(&b)[0];
            assert_eq!(
                (current.object_id, current.hp, current.max_hp),
                (pet, original.hp, original.max_hp)
            );
            assert_eq!(
                current.position,
                map_point(MAP_B, 40, 49),
                "Master.Back uses Direction - 1"
            );
            assert!(packets(&recalled, "owner").iter().any(|packet| matches!(packet, ServerPacket::ObjectMonster{info} if info.object_id == pet && info.master_object_id == 101)));
            assert!(packets(&recalled, "observer").iter().any(|packet| matches!(packet, ServerPacket::ObjectRemove{object_id} if *object_id == pet)));
        }
        assert_eq!(
            manager.capture_player_saved_pets(&sid, 620),
            saved,
            "all real same-Node pets are captured across maps"
        );
    }
}

#[test]
fn shared_owned_pet_manager_no_pets_freezes_then_forces_recall_and_true_logout_cleans_all_maps() {
    let sid = SessionId::new("owner");
    let a = ZoneKey::for_map(MAP_A);
    let b = ZoneKey::for_map(MAP_B);
    for mode in [0, 1, 2, 3, 4] {
        let mut manager = saved_pet_manager();
        manager.join(saved_pet_join("owner", 101, MAP_A, p(10, 20), mode));
        let pet = manager_summon(&mut manager, Spell::SummonSkeleton, 10);
        let position = manager.native_monster_snapshots(&a)[0].position.clone();
        manager.join(saved_pet_join(
            "owner",
            101,
            MAP_C,
            p(40, 50),
            mode,
        ));
        let frozen = manager.refresh_online_pets_at(620);
        assert!(
            packets(&frozen, "owner")
                .iter()
                .any(|packet| matches!(packet, ServerPacket::Chat { .. })),
            "one source NoPets notice for mode {mode}"
        );
        assert!(
            packets(&manager.refresh_online_pets_at(621), "owner").is_empty(),
            "repeated maintenance stays quiet"
        );
        let idle = manager.tick_all(2600);
        assert!(!packets(&idle, "owner").iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
        )));
        assert_eq!(manager.native_monster_snapshots(&a)[0].position, position);
        manager.join(saved_pet_join("owner", 101, MAP_B, p(40, 50), mode));
        manager.refresh_online_pets_at(2610);
        assert!(manager.native_monster_snapshots(&a).is_empty());
        assert_eq!(
            manager.native_monster_snapshots(&b)[0].object_id,
            pet,
            "NoPets Frozen forces allowed-map recall even mode {mode}"
        );
        let admission = manager.capture_player_pet_experience(&sid, 2610).unwrap();
        let saved = manager
            .capture_player_saved_pets(&sid, 2610)
            .for_durable_storage();
        let old_online = manager.online_owner_proof_for_session(&sid).unwrap();
        manager.handle(ZoneCommand::Leave {
            session_id: sid.clone(),
        });
        for map in [MAP_A, MAP_B, MAP_C] {
            assert!(
                manager
                    .native_monster_snapshots(&ZoneKey::for_map(map))
                    .is_empty(),
                "true logout removes every detached pet"
            );
        }
        manager.join(saved_pet_join("owner", 201, MAP_B, p(40, 50), mode));
        assert_ne!(
            manager.online_owner_proof_for_session(&sid).unwrap(),
            old_online
        );
        assert!(
            manager
                .mirror_player_pet_earned_steps(&sid, &admission, &[60_000], 2700)
                .is_empty(),
            "old capability cannot recreate pets in a fresh online life"
        );
        let restored = manager
            .restore_player_saved_pets(&sid, &saved, 2700)
            .unwrap();
        assert!(packets(&restored, "owner").iter().any(|packet| matches!(packet, ServerPacket::ObjectMonster{info} if info.master_object_id == 201)));
        assert_eq!(
            manager
                .capture_player_saved_pets(&sid, 2700)
                .for_durable_storage(),
            saved
        );
        assert!(
            manager
                .restore_player_saved_pets(&sid, &saved, 2701)
                .unwrap()
                .is_empty(),
            "repeated trusted login adoption does not duplicate live pets"
        );
    }
}

#[test]
fn shared_owned_pet_manager_earned_steps_keep_other_map_pets_and_only_mirror_committed_current_lives()
 {
    let sid = SessionId::new("owner");
    let a = ZoneKey::for_map(MAP_A);
    let b = ZoneKey::for_map(MAP_B);
    let mut manager = saved_pet_manager();
    manager.join(saved_pet_join("owner", 101, MAP_A, p(10, 20), 2));
    let skeleton = manager_summon(&mut manager, Spell::SummonSkeleton, 10);
    manager.join(saved_pet_join("owner", 101, MAP_B, p(10, 20), 2));
    manager.refresh_online_pets_at(620);
    let shinsu = manager_summon(&mut manager, Spell::SummonShinsu, 2010);
    let admission = manager.capture_player_pet_experience(&sid, 2520).unwrap();
    let before = admission.before_saved_snapshot();
    assert_eq!(saved_pet_records(&before).len(), 2);
    let committed = admission.saved_after_earned_steps(&[60_000, 1]).unwrap();
    let bone_index = mir2_game_data::crystal_monster_by_name("BoneFamiliar")
        .unwrap()
        .monster_index;
    let shinsu_index = mir2_game_data::crystal_monster_by_name("Shinsu")
        .unwrap()
        .monster_index;
    let records = saved_pet_records(&committed);
    let bone = records
        .iter()
        .find(|p| p["monster_index"] == bone_index)
        .unwrap();
    let dog = records
        .iter()
        .find(|p| p["monster_index"] == shinsu_index)
        .unwrap();
    assert_eq!(
        (bone["level"].as_u64(), bone["experience"].as_u64()),
        (Some(2), Some(0)),
        "same-online other-map pets receive no GainExp"
    );
    assert_eq!(
        (dog["level"].as_u64(), dog["experience"].as_u64()),
        (Some(4), Some(40_003)),
        "two actual GainExp steps each get their single threshold check"
    );
    assert_eq!(
        sorted_saved_pet_records(&manager.capture_player_saved_pets(&sid, 2521)),
        sorted_saved_pet_records(&before),
        "preparing or rejecting source CAS must not mutate the live Zone"
    );
    let old = manager
        .native_monster_snapshots(&a)
        .into_iter()
        .find(|m| m.object_id == skeleton)
        .unwrap();
    let current = manager
        .native_monster_snapshots(&b)
        .into_iter()
        .find(|m| m.object_id == shinsu)
        .unwrap();
    let mirrored = manager.mirror_player_pet_earned_steps(&sid, &admission, &[60_000, 1], 2522);
    assert!(packets(&mirrored, "owner").iter().any(|packet|matches!(packet,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id == shinsu && *name_colour_argb == 0xFF6A5ACD_u32 as i32)));
    assert_eq!(manager.native_monster_snapshots(&a)[0], old);
    let grown = manager
        .native_monster_snapshots(&b)
        .into_iter()
        .find(|m| m.object_id == shinsu)
        .unwrap();
    assert_eq!((grown.hp, grown.max_hp), (current.hp, current.max_hp + 40));
    assert_eq!(
        sorted_saved_pet_records(&manager.capture_player_saved_pets(&sid, 2522)),
        sorted_saved_pet_records(&committed)
    );
    assert!(
        manager
            .mirror_player_pet_earned_steps(&sid, &admission, &[60_000, 1], 2523)
            .is_empty(),
        "same committed steps are once-only at the current pet state"
    );
    manager = ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap();
    assert!(
        manager
            .mirror_player_pet_earned_steps(&sid, &admission, &[60_000, 1], 2524)
            .is_empty(),
        "cold-revoked online lives cannot accept an old admission"
    );
}

#[test]
fn shared_owned_pet_actual_personal_gain_exp_and_source_save_are_atomic_and_mirror_once() {
    use mir2_protocol::ClientPacket;
    use mir2_simulation::{
        AccountStoreTransactionFault, SimulationConfig, SimulationSession, WorldEntityDisposition,
        ZoneMonsterSpawn,
    };

    // Only an isolated prepared account is modified. The earned amount below
    // comes from an actual shared native death and personal GainExperience.
    let mut config = SimulationConfig::default();
    config.default_character.class = MirClass::Taoist;
    config.default_character.level = 40;
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        let character = account
            .characters
            .iter_mut()
            .find(|c| c.index == config.default_character.index)
            .unwrap();
        character.class = MirClass::Taoist;
        character.level = 40;
        let updated = character.clone();
        account
            .saves
            .get_mut(&config.default_character.index)
            .unwrap()
            .character = updated;
    }
    let mut personal = SimulationSession::new(config.clone());
    personal.enable_shared_guild_authority();
    assert!(
        personal
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. }))
    );
    assert!(
        personal
            .handle_packet(ClientPacket::StartGame {
                character_index: config.default_character.index
            })
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. }))
    );
    let sid = SessionId::new("owner");
    let mut manager = saved_pet_manager();
    let mut admitted = personal.active_zone_join_snapshot("owner").unwrap();
    admitted.map_file_name = MAP_A.into();
    admitted.position = p(10, 20);
    admitted.direction = MirDirection::Up;
    admitted.chat_profile.pet_mode = 2;
    manager.join(admitted);
    assert!(
        manager.update_experience_profile(&sid, personal.active_zone_experience_profile().unwrap())
    );
    manager.handle(ZoneCommand::sync_player_combat_state(
        sid.clone(),
        MirClass::Taoist,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let pet = manager_summon(&mut manager, Spell::SummonSkeleton, 10);
    let snapshot = manager.capture_player_saved_pets(&sid, 600);
    let admission = manager.capture_player_pet_experience(&sid, 600).unwrap();
    personal
        .set_shared_pet_experience_context(snapshot.clone(), Some(admission))
        .unwrap();
    manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid.clone(),
        now_ms: 600,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 80_001,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 40,
            hp: 1,
            max_hp: 1,
            experience: 20_000,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position: p(10, 19),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: vec![],
        },
    });
    let mut death = manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid.clone(),
        object_id: 80_001,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 610,
    });
    death.extend(manager.tick_all(610));
    let (zone, proof, award) = death
        .into_iter()
        .find_map(|out| match out {
            ZoneOutbound::OwnedMonsterKillAward {
                source,
                session_id,
                online_owner,
                award,
            } if session_id == sid => Some((source, online_owner, award)),
            _ => None,
        })
        .expect("real native fatal hit issues the selected earned amount");
    assert!(manager.issued_monster_award_is_current(&zone, &sid, &proof, &award));
    assert_eq!(
        award.experience_selection.as_ref().unwrap().final_amount,
        20_000
    );
    let before = personal.active_character_checkpoint().unwrap();
    let durable_before = serde_json::to_value(
        &config.account_store.lock().unwrap().accounts["demo"].saves
            [&config.default_character.index],
    )
    .unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(
        personal
            .try_commit_shared_monster_kill_award_with_receipt("actual-pet-kill-1", &award)
            .is_err()
    );
    assert_eq!(
        personal.active_character_checkpoint().unwrap().experience,
        before.experience
    );
    assert_eq!(personal.active_saved_pet_snapshot(), snapshot);
    assert_eq!(
        serde_json::to_value(
            &config.account_store.lock().unwrap().accounts["demo"].saves
                [&config.default_character.index]
        )
        .unwrap(),
        durable_before
    );
    assert!(personal.take_shared_pet_experience_commit().is_none());
    assert_eq!(manager.capture_player_saved_pets(&sid, 611), snapshot);

    let (committed, replayed) = personal
        .try_commit_shared_monster_kill_award_with_receipt("actual-pet-kill-1", &award)
        .unwrap();
    assert!(committed.committed && !replayed);
    let steps = committed
        .packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::GainExperience { amount } => Some(*amount as u32),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        steps,
        vec![20_000],
        "actual positive final GainExp producer records the source amount"
    );
    let (live, recorded) = personal
        .take_shared_pet_experience_commit()
        .expect("only a successful complete source save admits live pet growth");
    assert_eq!(recorded, steps);
    assert!(personal.take_shared_pet_experience_commit().is_none());
    let desired = live
        .saved_after_earned_steps(&steps)
        .unwrap()
        .for_durable_storage();
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves
            [&config.default_character.index]
            .saved_pets,
        desired
    );
    assert_eq!(saved_pet_records(&desired)[0]["level"], 3);
    let mirrored = manager.mirror_player_pet_earned_steps(&sid, &live, &recorded, 612);
    assert!(packets(&mirrored,"owner").iter().any(|packet|matches!(packet,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id == pet && *name_colour_argb == 0xFF20B2AA_u32 as i32)));
    assert_eq!(manager.capture_player_saved_pets(&sid, 612), desired);
    let (replay, duplicate) = personal
        .try_commit_shared_monster_kill_award_with_receipt("actual-pet-kill-1", &award)
        .unwrap();
    assert!(replay.committed && duplicate && replay.packets.is_empty());
    assert!(personal.take_shared_pet_experience_commit().is_none());
    assert!(
        manager
            .mirror_player_pet_earned_steps(&sid, &live, &recorded, 613)
            .is_empty()
    );
    assert_eq!(manager.capture_player_saved_pets(&sid, 613), desired);
    let durable_json = serde_json::to_value(&desired).unwrap();
    let mut fields = durable_json["pets"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    fields.sort();
    assert_eq!(
        fields,
        [
            "experience",
            "hp",
            "level",
            "max_pet_level",
            "monster_index"
        ],
        "durable source keeps only canonical PetInfo, never IDs, epochs or actions"
    );
}

#[test]
fn shared_owned_toad_real_summon_launches_source_mac_projectile_and_rechecks_flight_lives() {
    for change in ["none", "safe", "group", "revive", "logout", "cold"] {
        let mut z = ordinary_zone(MAP_A);
        let mut owner = join("owner", 101, p(10, 20));
        owner.class = MirClass::Archer;
        owner.hp = 800;
        z.handle(ZoneCommand::Join(owner));
        z.handle(ZoneCommand::Join(join("victim", 102, p(12, 20))));
        z.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new("owner"),
            MirClass::Archer,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
        let mut armour = mir2_simulation::ZonePlayerCombatStats::default();
        armour.min_ac = 10_000;
        armour.max_ac = 10_000;
        z.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: SessionId::new("victim"),
            stats: armour,
        });
        let cast = z.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("owner"),
            object_id: 102,
            spell: Spell::SummonToad,
            direction: MirDirection::Right,
            target: p(12, 20),
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 7,
            cooldown_ms: 1000,
            now_ms: 10,
        });
        assert!(packets(&cast, "owner").iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::SummonToad,
                cast: true,
                ..
            }
        )));
        assert!(z.native_monster_snapshots().is_empty());
        // ArcherSummon reaches DelayedType.Spawn after its range flight, then
        // Map.Spawn's second 500 ms delay: 10 + (500 + 2 * 50) + 500.
        z.tick(1109);
        assert!(z.native_monster_snapshots().is_empty());
        let spawned = z.tick(1110);
        let pet = z
            .native_monster_snapshots()
            .into_iter()
            .find(|m| m.name == "SpittingToad")
            .unwrap();
        assert!(packets(&spawned,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info}if info.object_id==pet.object_id && info.master_object_id==101)));
        let mut launched = None;
        for now in (3300..=7500).step_by(300) {
            let out = z.tick(now);
            if packets(&out,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==pet.object_id && info.target_id==102)) {
                launched=Some(now);break;
            }
        }
        let started = launched.expect("real SummonToad assigned Target reaches custom AI");
        let pet = z
            .native_monster_snapshots()
            .into_iter()
            .find(|m| m.object_id == pet.object_id)
            .unwrap();
        let target = p(12, 20);
        let distance = (pet.position.x - target.x).abs().max((pet.position.y - target.y).abs()) as u64;
        assert!((1..=12).contains(&distance));
        let due = started + 500 + distance * 50;
        assert_eq!(hp(&z, "victim"), 1000, "no damage on range animation");
        z.tick(due - 1);
        assert_eq!(hp(&z, "victim"), 1000, "exact projectile deadline {change}");
        match change {
            "safe" => sync_pet_safe(&mut z, "victim", true),
            "group" => {
                let mut mode = z.player_chat_profile(&SessionId::new("owner")).unwrap();
                mode.attack_mode = 1;
                z.handle(ZoneCommand::UpdateChatProfile {
                    session_id: SessionId::new("owner"),
                    profile: mode,
                });
                let mut group = z.player_chat_profile(&SessionId::new("victim")).unwrap();
                group.group_members = vec!["OWNER".into()];
                z.handle(ZoneCommand::UpdateChatProfile {
                    session_id: SessionId::new("victim"),
                    profile: group,
                });
            }
            "revive" => {
                z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: SessionId::new("victim"),
                    hp: 0,
                    max_hp: 1000,
                    mp: 1000,
                    dead: true,
                });
                z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: SessionId::new("victim"),
                    hp: 1000,
                    max_hp: 1000,
                    mp: 1000,
                    dead: false,
                });
            }
            "logout" => {
                z.handle(ZoneCommand::Leave {
                    session_id: SessionId::new("owner"),
                });
            }
            "cold" => {
                z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
            }
            _ => {}
        }
        let impact = z.tick(due);
        if change == "none" {
            assert!(
                hp(&z, "victim") < 1000,
                "MAC source projectile ignores the prepared AC"
            );
            assert!(
                packets(&impact, "victim").iter().any(
                    |p| matches!(p,ServerPacket::Struck{info}if info.attacker_id==pet.object_id)
                )
            );
            assert!(packets(&z.tick(started+2999),"owner").iter().all(|p|!matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==pet.object_id)));
            assert_eq!(
                z.native_monster_snapshots()
                    .into_iter()
                    .find(|m| m.object_id == pet.object_id)
                    .unwrap()
                    .position,
                pet.position,
                "Toad never follows Master"
            );
        } else {
            assert_eq!(
                hp(&z, "victim"),
                1000,
                "old projectile must reject {change}"
            );
        }
    }
}

fn p(x: i32, y: i32) -> Point {
    map_point(MAP_A, x, y)
}
fn join(name: &str, id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: format!("account-{name}"),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 40,
        hp: 1000,
        max_hp: 1000,
        mp: 1000,
        map_file_name: MAP_A.into(),
        position,
        direction: MirDirection::Down,
        chat_profile: ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: Default::default(),
    }
}
fn packets<'a>(out: &'a [ZoneOutbound], session: &str) -> Vec<&'a ServerPacket> {
    let sid = SessionId::new(session);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == sid => packets.iter().collect(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&sid) => packets.iter().collect(),
            ZoneOutbound::ToAll { packets } => packets.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}
fn spawn(z: &mut ZoneRuntime, session: &str, position: Point, spell: Spell, now: u64) -> u32 {
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new(session),
        object_id: 0,
        spell,
        direction: MirDirection::Right,
        target: position,
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: now,
    });
    let out = z.tick(now + 500);
    packets(&out, session)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info }
                if info.name
                    == if spell == Spell::SummonShinsu {
                        "Shinsu"
                    } else {
                        "BoneFamiliar"
                    } =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("real spell must create pet")
}
fn fixture(mode: u8) -> (ZoneRuntime, u32) {
    fixture_with_attacker_class(mode, MirClass::Taoist)
}

fn fixture_with_attacker_class(mode: u8, attacker_class: MirClass) -> (ZoneRuntime, u32) {
    let mut z = ordinary_zone(MAP_A);
    let mut owner = join("owner", 101, p(10, 20));
    owner.chat_profile.pet_mode = mode;
    z.handle(ZoneCommand::Join(owner));
    let mut attacker = join("victim", 102, p(10, 21));
    attacker.class = attacker_class;
    z.handle(ZoneCommand::Join(attacker));
    for name in ["owner", "victim"] {
        z.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new(name),
            if name == "victim" {
                attacker_class
            } else {
                MirClass::Taoist
            },
            true,
            false,
            false,
            false,
            false,
            false,
        ));
    }
    let pet = spawn(&mut z, "owner", p(11, 20), Spell::SummonSkeleton, 10);
    (z, pet)
}
fn attack(
    z: &mut ZoneRuntime,
    session: &str,
    target: u32,
    direction: MirDirection,
    now: u64,
) -> Vec<ZoneOutbound> {
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new(session),
        object_id: target,
        direction,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: now,
    })
}
fn hp(z: &ZoneRuntime, session: &str) -> i32 {
    z.player_vitals(&SessionId::new(session)).unwrap().0
}

fn pet_hp(z: &ZoneRuntime, pet: u32) -> i32 {
    z.native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .hp
}

fn set_human_pet_damage(z: &mut ZoneRuntime, session: &str) {
    let mut stats = mir2_simulation::ZonePlayerCombatStats::default();
    stats.min_dc = 40;
    stats.max_dc = 40;
    stats.min_mc = 40;
    stats.max_mc = 40;
    stats.min_sc = 40;
    stats.max_sc = 40;
    stats.accuracy = 100;
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new(session),
        stats,
    });
}

fn sync_pet_safe(z: &mut ZoneRuntime, session: &str, safe: bool) {
    let session_id = SessionId::new(session);
    let mut profile = z.player_chat_profile(&session_id).unwrap();
    profile.in_safe_zone = safe;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id,
        profile,
    });
}

#[test]
fn shared_human_owned_pet_real_melee_obeys_source_mode_group_guild_enemy_and_safe_matrix() {
    // Master safe does not protect this pet from HumanObject; attacker safe does.
    for (mode, group, same_guild, enemy, pk, master_safe, attacker_safe, legal) in [
        (0, false, false, false, 0, false, false, false),
        (1, false, false, false, 0, false, false, true),
        (1, true, false, false, 0, false, false, false),
        (2, false, true, false, 0, false, false, false),
        (2, false, false, false, 0, false, false, true),
        (3, false, false, false, 0, false, false, false),
        (3, false, false, true, 0, false, false, true),
        (4, false, false, false, 199, false, false, false),
        (4, false, false, false, 200, false, false, true),
        (5, true, true, false, 0, false, false, true),
        (5, false, false, false, 0, true, false, true),
        (5, false, false, false, 0, false, true, false),
    ] {
        let (mut z, pet) = fixture(3);
        set_human_pet_damage(&mut z, "victim");
        let mut master = ZoneChatProfile {
            attack_mode: 5,
            pet_mode: 3,
            pk_points: pk,
            guild_name: Some("MasterGuild".into()),
            ..Default::default()
        };
        if group {
            master.group_members.push("victim".into());
        }
        if enemy {
            master.active_guild_wars.push("AttackerGuild".into());
        }
        z.handle(ZoneCommand::UpdateChatProfile {
            session_id: SessionId::new("owner"),
            profile: master,
        });
        z.handle(ZoneCommand::UpdateChatProfile {
            session_id: SessionId::new("victim"),
            profile: ZoneChatProfile {
                attack_mode: mode,
                guild_name: Some(
                    if same_guild {
                        "MasterGuild"
                    } else {
                        "AttackerGuild"
                    }
                    .into(),
                ),
                ..Default::default()
            },
        });
        sync_pet_safe(&mut z, "owner", master_safe);
        sync_pet_safe(&mut z, "victim", attacker_safe);
        assert_eq!(
            z.player_can_attack_owned_pet(&SessionId::new("victim"), pet, 600),
            legal
        );
        let before = pet_hp(&z, pet);
        let accepted = attack(&mut z, "victim", pet, MirDirection::UpRight, 600);
        assert_eq!(
            packets(&accepted, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == 102)),
            legal
        );
        let impact = z.tick(600);
        assert_eq!(
            pet_hp(&z, pet) < before,
            legal,
            "mode={mode}, group={group}, guild={same_guild}, enemy={enemy}, pk={pk}, master_safe={master_safe}, attacker_safe={attacker_safe}"
        );
        assert_eq!(hp(&z, "owner"), 1000, "pet HP is independent of its owner");
        assert!(!impact.iter().any(|o| matches!(
            o,
            ZoneOutbound::MonsterKillAward { .. } | ZoneOutbound::OwnedPetPlayerKill { .. }
        )));
        if legal {
            assert!(
                packets(&impact, "victim")
                    .iter()
                    .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
                if info.object_id == pet && info.attacker_id == 102))
            );
            assert_eq!(packets(&impact, "victim").iter().any(|p| matches!(p,
                ServerPacket::ColourChanged { name_colour_argb } if *name_colour_argb == 0xFF8B4513_u32 as i32)), pk < 200,
                "the early HumanObject pet hit branch also browns EnemyGuild aggression");
        }
    }
}

#[test]
fn shared_human_owned_pet_real_spell_and_range_hits_use_typed_damage() {
    for ranged in [false, true] {
        let (mut z, pet) = fixture_with_attacker_class(
            3,
            if ranged {
                MirClass::Archer
            } else {
                MirClass::Taoist
            },
        );
        set_human_pet_damage(&mut z, "victim");
        let before = pet_hp(&z, pet);
        let accepted = if ranged {
            z.handle(ZoneCommand::PlayerRangeAttackObject {
                session_id: SessionId::new("victim"),
                object_id: pet,
                direction: MirDirection::UpRight,
                target: p(11, 20),
                spell: Spell::None,
                level: 0,
                attack_type: 0,
                damage: 1,
                now_ms: 600,
            })
        } else {
            z.handle(ZoneCommand::PlayerCastMagic {
                session_id: SessionId::new("victim"),
                object_id: pet,
                spell: Spell::SoulFireBall,
                direction: MirDirection::UpRight,
                target: p(11, 20),
                cast: true,
                level: 2,
                damage: 1,
                mp_cost: 1,
                cooldown_ms: 1000,
                now_ms: 600,
            })
        };
        assert!(packets(&accepted, "victim").iter().any(|p| matches!(
            p,
            ServerPacket::RangeAttack { .. } | ServerPacket::Magic { .. }
        )));
        let impact = z.tick(600);
        assert!(pet_hp(&z, pet) < before, "ranged={ranged}");
        assert_eq!(hp(&z, "owner"), 1000);
        assert!(
            packets(&impact, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
            if info.object_id == pet && info.attacker_id == 102))
        );
    }
}

#[test]
fn shared_human_owned_pet_delayed_hit_rechecks_friends_safe_and_online_life() {
    for change in 0..4 {
        let (mut z, pet) = fixture(3);
        set_human_pet_damage(&mut z, "victim");
        let before = pet_hp(&z, pet);
        let mode = if change == 1 { 2 } else { 1 };
        z.handle(ZoneCommand::UpdateChatProfile {
            session_id: SessionId::new("victim"),
            profile: ZoneChatProfile {
                attack_mode: mode,
                guild_name: Some("AttackerGuild".into()),
                ..Default::default()
            },
        });
        z.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("victim"),
            object_id: pet,
            direction: MirDirection::UpRight,
            spell: Spell::TwinDrakeBlade as u8,
            level: 0,
            attack_type: 0,
            damage: 1,
            now_ms: 600,
        });
        z.tick(899);
        assert_eq!(pet_hp(&z, pet), before, "source first leg waits 300 ms");
        match change {
            0 => {
                z.handle(ZoneCommand::UpdateChatProfile {
                    session_id: SessionId::new("owner"),
                    profile: ZoneChatProfile {
                        attack_mode: 5,
                        pet_mode: 3,
                        group_members: vec!["victim".into()],
                        ..Default::default()
                    },
                });
            }
            1 => {
                z.handle(ZoneCommand::UpdateChatProfile {
                    session_id: SessionId::new("owner"),
                    profile: ZoneChatProfile {
                        attack_mode: 5,
                        pet_mode: 3,
                        guild_name: Some("AttackerGuild".into()),
                        ..Default::default()
                    },
                });
            }
            2 => sync_pet_safe(&mut z, "victim", true),
            _ => {
                z.handle(ZoneCommand::Leave {
                    session_id: SessionId::new("victim"),
                });
                z.handle(ZoneCommand::Join(join("victim", 102, p(10, 21))));
                set_human_pet_damage(&mut z, "victim");
                sync_pet_safe(&mut z, "victim", false);
            }
        }
        let impact = z.tick(1000);
        assert_eq!(pet_hp(&z, pet), before, "impact change={change}");
        assert!(
            !packets(&impact, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info } if info.object_id == pet))
        );
    }
}

#[test]
fn shared_human_owned_pet_brown_starts_on_real_damage_and_expires_at_exact_deadline() {
    let brown = 0xFF8B4513_u32 as i32;
    let (mut z, pet) = fixture(3);
    set_human_pet_damage(&mut z, "victim");
    attack(&mut z, "victim", pet, MirDirection::UpRight, 600);
    let started = z.tick(600);
    assert_eq!(
        packets(&started, "victim")
            .iter()
            .filter(|p| matches!(p,
        ServerPacket::ColourChanged { name_colour_argb } if *name_colour_argb == brown))
            .count(),
        1
    );
    assert_eq!(packets(&started, "owner").iter().filter(|p| matches!(p,
        ServerPacket::ObjectColourChanged { object_id: 102, name_colour_argb } if *name_colour_argb == brown)).count(), 1);
    let early = z.tick(60_599);
    assert!(
        !packets(&early, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ColourChanged { .. }))
    );
    let expired = z.tick(60_600);
    assert_eq!(
        packets(&expired, "victim")
            .iter()
            .filter(|p| matches!(
                p,
                ServerPacket::ColourChanged {
                    name_colour_argb: -1
                }
            ))
            .count(),
        1
    );
    assert_eq!(
        packets(&expired, "owner")
            .iter()
            .filter(|p| matches!(
                p,
                ServerPacket::ObjectColourChanged {
                    object_id: 102,
                    name_colour_argb: -1
                }
            ))
            .count(),
        1
    );
    assert!(
        !packets(&z.tick(60_600), "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ColourChanged { .. }))
    );
}

#[test]
fn shared_human_owned_pet_red_brown_permission_uses_real_master_damage_and_strict_expiry() {
    for (now, legal) in [(60_619, true), (60_620, false)] {
        let (mut z, pet) = fixture(3);
        let aggression = attack(&mut z, "owner", 102, MirDirection::Down, 620);
        assert!(packets(&aggression, "owner").iter().any(|p| matches!(p,
            ServerPacket::ColourChanged { name_colour_argb } if *name_colour_argb == 0xFF8B4513_u32 as i32)));
        set_human_pet_damage(&mut z, "victim");
        z.handle(ZoneCommand::UpdateChatProfile {
            session_id: SessionId::new("victim"),
            profile: ZoneChatProfile {
                attack_mode: 4,
                ..Default::default()
            },
        });
        assert_eq!(
            z.player_can_attack_owned_pet(&SessionId::new("victim"), pet, now),
            legal
        );
        let accepted = attack(&mut z, "victim", pet, MirDirection::UpRight, now);
        assert_eq!(
            packets(&accepted, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == 102)),
            legal
        );
        let impact = z.tick(now);
        assert_eq!(
            packets(&impact, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
            if info.object_id == pet && info.attacker_id == 102)),
            legal
        );
    }
}

#[test]
fn shared_human_owned_pet_same_master_requires_all_and_early_safe_exception() {
    for (mode, legal) in [
        (0, false),
        (1, false),
        (2, false),
        (3, false),
        (4, false),
        (5, true),
    ] {
        let (mut z, pet) = fixture(3);
        set_human_pet_damage(&mut z, "owner");
        z.handle(ZoneCommand::UpdateChatProfile {
            session_id: SessionId::new("owner"),
            profile: ZoneChatProfile {
                attack_mode: mode,
                pet_mode: 3,
                ..Default::default()
            },
        });
        sync_pet_safe(&mut z, "owner", true);
        assert_eq!(
            z.player_can_attack_owned_pet(&SessionId::new("owner"), pet, 600),
            legal
        );
        let accepted = attack(&mut z, "owner", pet, MirDirection::Right, 620);
        assert_eq!(
            packets(&accepted, "owner")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == 101)),
            legal
        );
        let impact = z.tick(620);
        assert_eq!(
            packets(&impact, "owner")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
            if info.object_id == pet && info.attacker_id == 101)),
            legal
        );
        assert!(
            !packets(&impact, "owner")
                .iter()
                .any(|p| matches!(p, ServerPacket::ColourChanged { .. })),
            "source never browns a master hitting its own pet"
        );
    }
}

#[test]
fn shared_human_owned_pet_ground_spell_has_real_delay_and_rechecks_safe_or_cold_source() {
    for change in 0..3 {
        let (mut z, pet) = fixture(3);
        set_human_pet_damage(&mut z, "victim");
        let before = pet_hp(&z, pet);
        let cast = z.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("victim"),
            object_id: 0,
            spell: Spell::FireBang,
            direction: MirDirection::UpRight,
            target: p(11, 20),
            cast: true,
            level: 2,
            damage: 1,
            mp_cost: 1,
            cooldown_ms: 1000,
            now_ms: 600,
        });
        assert!(packets(&cast, "victim").iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::FireBang,
                ..
            }
        )));
        z.tick(1099);
        assert_eq!(pet_hp(&z, pet), before, "real ground magic waits 500 ms");
        match change {
            1 => sync_pet_safe(&mut z, "victim", true),
            2 => {
                z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
            }
            _ => {}
        }
        let queued: serde_json::Value =
            serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
        let ground = &queued["pending_native_ground_spells"];
        let impact = z.tick(1100);
        assert_eq!(
            packets(&impact, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
            if info.object_id == pet && info.attacker_id == 102)),
            change == 0,
            "change={change}, ground={ground:?}, out={impact:?}"
        );
        assert_eq!(pet_hp(&z, pet) < before, change == 0, "change={change}");
    }
}

fn spawn_fire_bounce_wild(z: &mut ZoneRuntime) {
    z.spawn_world_event_monster(
        &mir2_simulation::ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 9111,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 5,
            ai: 0,
            disposition: Some(mir2_simulation::WorldEntityDisposition::Hostile),
            level: 30,
            max_hp: 5000,
            hp: 5000,
            experience: 0,
            move_speed_ms: 100_000,
            attack_speed_ms: 100_000,
            friendly_guild: None,
            defense: Default::default(),
            position: p(11, 21),
            direction: MirDirection::Down,
            respawn: None,
            drops: Vec::new(),
        },
        520,
    );
}

#[test]
fn shared_human_ground_one_shot_keeps_future_action_and_hits_ordinary_wild_once() {
    let (mut z, _) = fixture(3);
    set_human_pet_damage(&mut z, "victim");
    spawn_fire_bounce_wild(&mut z);
    let mut profile = z.player_chat_profile(&SessionId::new("victim")).unwrap();
    profile.attack_mode = 0; // ordinary wild PvE is valid in Peace
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("victim"),
        profile,
    });
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("victim"),
        object_id: 0,
        spell: Spell::FireBang,
        direction: MirDirection::Right,
        target: p(11, 21),
        cast: true,
        level: 2,
        damage: 1,
        mp_cost: 1,
        cooldown_ms: 1000,
        now_ms: 600,
    });
    let early = z.tick(1099);
    assert!(
        !packets(&early, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
        if info.object_id == 9111 && info.attacker_id == 102))
    );
    assert_eq!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == 9111)
            .unwrap()
            .hp,
        5000
    );
    let impact = z.tick(1100);
    assert!(
        packets(&impact, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
        if info.object_id == 9111 && info.attacker_id == 102))
    );
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == 9111)
            .unwrap()
            .hp
            < 5000
    );
    let duplicate = z.tick(1101);
    assert!(
        !packets(&duplicate, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
        if info.object_id == 9111 && info.attacker_id == 102))
    );
}

#[test]
fn shared_human_owned_pet_fire_bounce_first_leg_and_wild_to_pet_chain_keep_cast_authority() {
    for first_owned in [false, true] {
        let (mut z, pet) = fixture(3);
        set_human_pet_damage(&mut z, "victim");
        if !first_owned {
            spawn_fire_bounce_wild(&mut z);
        }
        z.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("victim"),
            object_id: if first_owned { pet } else { 9111 },
            spell: Spell::FireBounce,
            direction: MirDirection::UpRight,
            target: if first_owned { p(11, 20) } else { p(11, 21) },
            cast: true,
            level: 2,
            damage: 1,
            mp_cost: 1,
            cooldown_ms: 1000,
            now_ms: 600,
        });
        let before = pet_hp(&z, pet);
        let early = z.tick(1149);
        assert!(
            !packets(&early, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
            if info.object_id == pet && info.attacker_id == 102))
        );
        let first = z.tick(1150);
        if first_owned {
            assert!(
                packets(&first, "victim")
                    .iter()
                    .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
                if info.object_id == pet && info.attacker_id == 102))
            );
            assert!(pet_hp(&z, pet) < before);
        } else {
            assert!(
                packets(&first, "victim")
                    .iter()
                    .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
                if info.object_id == 9111 && info.attacker_id == 102))
            );
            let second = z.tick(1200);
            assert!(
                packets(&second, "victim")
                    .iter()
                    .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
                if info.object_id == pet && info.attacker_id == 102))
            );
            assert!(pet_hp(&z, pet) < before);
        }
    }
}

#[test]
fn shared_human_owned_pet_fire_bounce_old_wild_leg_cannot_mint_rejoined_source_authority() {
    let (mut z, pet) = fixture(3);
    set_human_pet_damage(&mut z, "victim");
    spawn_fire_bounce_wild(&mut z);
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("victim"),
        object_id: 9111,
        spell: Spell::FireBounce,
        direction: MirDirection::Right,
        target: p(11, 21),
        cast: true,
        level: 2,
        damage: 1,
        mp_cost: 1,
        cooldown_ms: 1000,
        now_ms: 600,
    });
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("victim"),
    });
    z.handle(ZoneCommand::Join(join("victim", 102, p(10, 21))));
    set_human_pet_damage(&mut z, "victim");
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("victim"),
        MirClass::Taoist,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let first = z.tick(1150);
    let second = z.tick(1200);
    for out in [&first, &second] {
        assert!(
            !packets(out, "victim")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectStruck { info }
            if (info.object_id == pet || info.object_id == 9111) && info.attacker_id == 102))
        );
    }
    assert_eq!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == 9111)
            .unwrap()
            .hp,
        5000
    );
}

#[test]
fn shared_owned_skeleton_and_shinsu_wait_for_a_real_owner_attack_on_non_threat_wild() {
    for spell in [Spell::SummonSkeleton, Spell::SummonShinsu] {
        let mut z = ordinary_zone(MAP_A);
        let mut owner = join("owner", 101, p(20, 20));
        owner.chat_profile.attack_mode = 0; // Peace still permits normal PvE.
        owner.combat_stats.min_sc = 20;
        owner.combat_stats.max_sc = 20;
        owner.combat_stats.accuracy = 100;
        z.handle(ZoneCommand::Join(owner));
        z.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new("owner"),
            MirClass::Taoist,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
        let pet = spawn(&mut z, "owner", p(21, 20), spell, 10);
        z.spawn_world_event_monster(
            &mir2_simulation::ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 9111,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 5,
                ai: 0,
                disposition: Some(mir2_simulation::WorldEntityDisposition::Hostile),
                level: 30,
                max_hp: 5000,
                hp: 5000,
                experience: 0,
                move_speed_ms: 3000,
                attack_speed_ms: 5000,
                friendly_guild: None,
                defense: Default::default(),
                position: p(30, 20), // Outside actual wild acquisition range.
                direction: MirDirection::Left,
                respawn: None,
                drops: Vec::new(),
            },
            520,
        );
        for now in (600..=4200).step_by(300) {
            let idle = z.tick(now);
            assert!(
                !packets(&idle, "owner").iter().any(|packet| matches!(packet,
                ServerPacket::ObjectAttack { info } if info.object_id == pet)),
                "{spell:?} must not attack an unrelated, non-threatening wild monster"
            );
            assert_eq!(
                z.native_monster_snapshots()
                    .iter()
                    .find(|m| m.object_id == 9111)
                    .unwrap()
                    .hp,
                5000
            );
        }
        let cast = z.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("owner"),
            object_id: 9111,
            spell: Spell::SoulFireBall,
            direction: MirDirection::Right,
            target: p(30, 20),
            cast: true,
            level: 2,
            damage: 20,
            mp_cost: 0,
            cooldown_ms: 1000,
            now_ms: 4300,
        });
        assert!(
            packets(&cast, "owner").iter().any(|packet| matches!(
                packet,
                ServerPacket::Magic {
                    spell: Spell::SoulFireBall,
                    cast: true,
                    ..
                }
            )),
            "ordinary owner spell must be admitted"
        );
        let mut owner_hit = false;
        let mut pet_launch = false;
        let mut pet_hit = false;
        for now in (4600..=16_000).step_by(300) {
            let out = z.tick(now);
            let emitted = packets(&out, "owner");
            owner_hit |= emitted.iter().any(|packet| matches!(packet,
                ServerPacket::ObjectStruck { info } if info.object_id == 9111 && info.attacker_id == 101));
            pet_launch |= emitted.iter().any(|packet| {
                matches!(packet,
                ServerPacket::ObjectAttack { info } if info.object_id == pet)
            });
            pet_hit |= emitted.iter().any(|packet| matches!(packet,
                ServerPacket::ObjectStruck { info } if info.object_id == 9111 && info.attacker_id == pet));
        }
        assert!(
            owner_hit,
            "the real owner attack must cause damage: {spell:?}"
        );
        assert!(
            pet_launch && pet_hit,
            "normal {spell:?} must follow the owner's real PvE attack"
        );
    }
}

#[test]
fn shared_owned_pet_protects_owner_with_delayed_real_damage_and_pet_struck_id() {
    let (mut z, pet) = fixture(0);
    attack(&mut z, "victim", 101, MirDirection::Up, 600);
    let exact = z.tick(2510);
    assert!(
        !packets(&exact, "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet))
    );
    let launched = z.tick(2511);
    assert!(
        packets(&launched, "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet))
    );
    assert_eq!(hp(&z, "victim"), 1000);
    z.tick(2810);
    assert_eq!(hp(&z, "victim"), 1000);
    let impact = z.tick(2811);
    assert!(hp(&z, "victim") < 1000);
    assert!(
        packets(&impact, "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::Struck{info}if info.attacker_id==pet))
    );
    assert_eq!(
        packets(&impact, "victim")
            .iter()
            .filter(|p| matches!(p, ServerPacket::Struck { .. }))
            .count(),
        1
    );
    assert!(
        !packets(&impact, "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==102))
    );
    assert!(packets(&impact,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==102 && info.attacker_id==pet)));
    assert!(impact.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged{session_id,settlement:Some(_),..}if *session_id==SessionId::new("victim"))));
}

#[test]
fn shared_owned_pet_focus_on_missed_owner_attack_can_causally_kill_once() {
    let (mut z, pet) = fixture(4);
    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("victim"),
        hp: 8,
        max_hp: 8,
        mp: 1000,
        dead: false,
    });
    let mut stats = Default::default();
    let _: &mir2_simulation::ZonePlayerCombatStats = &stats;
    stats.min_ac = 2;
    stats.max_ac = 2;
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("victim"),
        stats,
    });
    attack(&mut z, "owner", 102, MirDirection::Down, 620);
    assert_eq!(hp(&z, "victim"), 8, "owner hit is absorbed");
    z.tick(2511);
    let impact = z.tick(2811);
    assert_eq!(hp(&z, "victim"), 0);
    let kills: Vec<_> = impact
        .iter()
        .filter_map(|o| {
            if let ZoneOutbound::OwnedPetPlayerKill { receipt } = o {
                Some(receipt)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(kills.len(), 1);
    assert_eq!(kills[0].pet_object_id, pet);
    assert_eq!(kills[0].owner_object_id, 101);
    assert!(kills[0].unlawful);
    assert!(
        packets(&impact, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::Death { .. }))
    );
    assert!(
        !z.tick(3111)
            .iter()
            .any(|o| matches!(o, ZoneOutbound::OwnedPetPlayerKill { .. }))
    );
}

#[test]
fn shared_owned_pet_enemy_guild_mode_does_not_attack_player_after_provocation() {
    let (mut z, pet) = fixture(0);
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.attack_mode = 3;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
    attack(&mut z, "victim", 101, MirDirection::Up, 600);
    for now in [2511, 2811, 3111, 3411] {
        assert!(
            !packets(&z.tick(now), "victim")
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet))
        );
    }
    assert_eq!(hp(&z, "victim"), 1000);
}

#[test]
fn shared_owned_pet_none_is_quiet_and_both_restores_guard() {
    let (mut z, pet) = fixture(3);
    attack(&mut z, "victim", 101, MirDirection::Up, 600);
    for now in [2511, 2811, 3111] {
        assert!(
            !packets(&z.tick(now), "victim")
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet))
        );
    }
    let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    profile.pet_mode = 0;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile,
    });
    z.tick(3711);
    z.tick(4011);
    assert!(hp(&z, "victim") < 1000);
}

#[test]
fn shared_owned_pet_queued_hit_cannot_cross_victim_revive_or_reconnect() {
    for reconnect in [false, true] {
        let (mut z, _) = fixture(0);
        attack(&mut z, "victim", 101, MirDirection::Up, 600);
        z.tick(2511);
        if reconnect {
            z.handle(ZoneCommand::Leave {
                session_id: SessionId::new("victim"),
            });
            z.handle(ZoneCommand::Join(join("victim", 102, p(10, 21))));
        } else {
            z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: SessionId::new("victim"),
                hp: 0,
                max_hp: 1000,
                mp: 1000,
                dead: true,
            });
            z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: SessionId::new("victim"),
                hp: 1000,
                max_hp: 1000,
                mp: 1000,
                dead: false,
            });
        }
        z.tick(2811);
        assert_eq!(hp(&z, "victim"), 1000, "reconnect={reconnect}");
    }
}

#[test]
fn shared_owned_pet_owner_death_emits_real_pet_death_and_cancels_hit() {
    let (mut z, pet) = fixture(0);
    attack(&mut z, "victim", 101, MirDirection::Up, 600);
    z.tick(2511);
    let died = z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("owner"),
        hp: 0,
        max_hp: 1000,
        mp: 1000,
        dead: true,
    });
    assert!(
        packets(&died, "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectDied{info}if info.object_id==pet))
    );
    assert!(
        z.native_monster_snapshots()
            .iter()
            .any(|m| m.object_id == pet && m.dead)
    );
    z.tick(2811);
    assert_eq!(hp(&z, "victim"), 1000);
    let before_expiry = z.tick(182_510);
    assert!(
        !packets(&before_expiry, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectRemove { object_id } if *object_id == pet))
    );
    assert!(
        z.native_monster_snapshots()
            .iter()
            .any(|m| m.object_id == pet && m.dead)
    );
    let expiry = z.tick(182_511);
    assert!(
        packets(&expiry, "victim")
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectRemove { object_id } if *object_id == pet)),
        "source DeadDelay retires the pet corpse at the exact 180000ms deadline"
    );
    assert!(
        !z.native_monster_snapshots()
            .iter()
            .any(|m| m.object_id == pet)
    );
}

#[test]
fn shared_owned_pet_redirects_to_real_enemy_pet_without_damaging_owner_proxy() {
    let (mut z, pet) = fixture(0);
    let other = spawn(&mut z, "victim", p(11, 21), Spell::SummonSkeleton, 20);
    attack(&mut z, "owner", 102, MirDirection::Down, 620);
    let owner_hp = hp(&z, "victim");
    let before = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == other)
        .unwrap()
        .hp;
    z.tick(2511);
    let out = z.tick(2811);
    let after = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == other)
        .unwrap()
        .hp;
    assert!(after < before);
    assert_eq!(hp(&z, "victim"), owner_hp);
    assert!(packets(&out,"victim").iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==other && info.attacker_id==pet)));
    assert!(!out.iter().any(|o| matches!(
        o,
        ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::OwnedPetPlayerKill { .. }
    )));
}

#[test]
fn shared_owned_summon_cast_cannot_cross_owner_death_revive_or_rejoin() {
    for reconnect in [false, true] {
        let mut z = ordinary_zone(MAP_A);
        z.handle(ZoneCommand::Join(join("owner", 101, p(10, 20))));
        z.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("owner"),
            object_id: 0,
            spell: Spell::SummonSkeleton,
            direction: MirDirection::Right,
            target: p(11, 20),
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 7,
            cooldown_ms: 1000,
            now_ms: 10,
        });
        assert!(z.native_monster_snapshots().is_empty());
        if reconnect {
            z.handle(ZoneCommand::Leave {
                session_id: SessionId::new("owner"),
            });
            z.handle(ZoneCommand::Join(join("owner", 101, p(10, 20))));
        } else {
            z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: SessionId::new("owner"),
                hp: 0,
                max_hp: 1000,
                mp: 1000,
                dead: true,
            });
            z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: SessionId::new("owner"),
                hp: 1000,
                max_hp: 1000,
                mp: 1000,
                dead: false,
            });
        }
        let out = z.tick(510);
        assert!(
            z.native_monster_snapshots().is_empty(),
            "reconnect={reconnect}"
        );
        assert!(
            !packets(&out, "owner")
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectMonster { .. }))
        );
    }
}

#[test]
fn shared_owned_shinsu_guard_uses_real_player_breath_and_550ms_delay() {
    let mut z = ordinary_zone(MAP_A);
    for (name, id, pos) in [("owner", 101, p(10, 20)), ("victim", 102, p(10, 21))] {
        z.handle(ZoneCommand::Join(join(name, id, pos)));
        z.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new(name),
            MirClass::Taoist,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
    }
    let pet = spawn(&mut z, "owner", p(11, 20), Spell::SummonShinsu, 10);
    attack(&mut z, "victim", 101, MirDirection::Up, 600);
    assert!(
        !packets(&z.tick(2510), "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectShow{object_id}if *object_id==pet))
    );
    assert!(
        packets(&z.tick(2511), "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectShow{object_id}if *object_id==pet))
    );
    assert!(
        !packets(&z.tick(3511), "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet))
    );
    assert!(
        packets(&z.tick(3512), "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet))
    );
    z.tick(4061);
    assert_eq!(hp(&z, "victim"), 1000);
    let hit = z.tick(4062);
    assert!(hp(&z, "victim") < 1000);
    assert!(
        packets(&hit, "victim")
            .iter()
            .any(|p| matches!(p,ServerPacket::Struck{info}if info.attacker_id==pet))
    );
}

#[test]
fn shared_owned_pet_issued_death_receipt_survives_online_map_handoff_and_ack_is_exact() {
    let mut manager = ZoneManager::new();
    for (name, id, pos) in [("owner", 101, p(10, 20)), ("victim", 102, p(10, 21))] {
        let mut actor = join(name, id, pos);
        if name == "owner" {
            actor.chat_profile.pet_mode = 4;
        }
        manager.join(actor);
        manager.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new(name),
            MirClass::Taoist,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
    }
    manager.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: p(11, 20),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    manager.tick_all(510);
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("victim"),
        hp: 8,
        max_hp: 8,
        mp: 1000,
        dead: false,
    });
    let mut stats = mir2_simulation::ZonePlayerCombatStats::default();
    stats.min_ac = 2;
    stats.max_ac = 2;
    manager.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("victim"),
        stats,
    });
    manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("owner"),
        object_id: 102,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 620,
    });
    manager.tick_all(2511);
    let killed = manager.tick_all(2811);
    let receipt = killed
        .iter()
        .find_map(|o| {
            if let ZoneOutbound::OwnedPetPlayerKill { receipt } = o {
                Some(receipt.clone())
            } else {
                None
            }
        })
        .expect("real source death");
    assert!(manager.issued_owned_pet_kill_is_current(&receipt));
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("owner"),
        hp: 0,
        max_hp: 1000,
        mp: 1000,
        dead: true,
    });
    assert!(
        manager.issued_owned_pet_kill_is_current(&receipt),
        "owner death cannot clear an already issued kill"
    );
    let mut transferred = join("owner", 101, map_point(MAP_B, 30, 30));
    transferred.map_file_name = MAP_B.into();
    manager.join(transferred);
    assert_eq!(
        manager.zone_key_for_session(&SessionId::new("owner")),
        Some(ZoneKey::for_map(MAP_B))
    );
    assert!(
        manager.issued_owned_pet_kill_is_current(&receipt),
        "Map.Remove preserves the global owner Node"
    );
    let mut forged = receipt.clone();
    forged.sequence += 1;
    manager.acknowledge_owned_pet_kill(&forged);
    assert!(manager.issued_owned_pet_kill_is_current(&receipt));
    manager.acknowledge_owned_pet_kill(&receipt);
    assert!(!manager.issued_owned_pet_kill_is_current(&receipt));
}

#[test]
fn shared_human_real_native_fatal_attack_issues_source_receipt_for_all_three_classes() {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        let mut manager = ZoneManager::new();
        let mut target = join("owner", 101, p(10, 20));
        target.hp = 5;
        target.max_hp = 5;
        target.chat_profile.pk_points = 100;
        manager.join(target);
        let mut attacker = join("victim", 102, p(10, 21));
        attacker.class = class;
        attacker.chat_profile.attack_mode = 2;
        attacker.combat_stats.min_dc = 40;
        attacker.combat_stats.max_dc = 40;
        attacker.combat_stats.accuracy = 100;
        manager.join(attacker);
        manager.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new("victim"),
            class,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
        let out = manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("victim"),
            object_id: 101,
            direction: MirDirection::Up,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 1,
            now_ms: 620,
        });
        assert_eq!(
            manager.player_vitals(&SessionId::new("owner")).unwrap().0,
            0,
            "class={class:?}"
        );
        assert!(
            packets(&out, "owner")
                .iter()
                .any(|p| matches!(p, ServerPacket::Death { .. }))
        );
        let receipt = out
            .into_iter()
            .find_map(|o| match o {
                ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
                _ => None,
            })
            .expect("actual Human death must issue its causal settlement receipt");
        assert!(receipt.direct_player && receipt.unlawful && !receipt.protected_by_law);
        assert_eq!((receipt.pet_object_id, receipt.pet_incarnation), (0, 0));
        assert_eq!(
            (receipt.owner_object_id, receipt.victim_object_id),
            (102, 101)
        );
        assert_eq!(receipt.owner_account_id, "account-victim");
        assert!(manager.issued_owned_pet_kill_is_current(&receipt));
        manager.handle(ZoneCommand::Leave {
            session_id: SessionId::new("victim"),
        });
        let mut restored =
            ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap();
        assert!(restored.issued_owned_pet_kill_is_current(&receipt));
        let mut forged = receipt.clone();
        forged.direct_player = false;
        assert!(!restored.issued_owned_pet_kill_is_current(&forged));
        restored.acknowledge_owned_pet_kill(&receipt);
        assert!(!restored.issued_owned_pet_kill_is_current(&receipt));
    }
}

fn cast_human_damage(
    z: &mut ZoneRuntime,
    object_id: u32,
    spell: Spell,
    target: Point,
    now: u64,
) -> Vec<ZoneOutbound> {
    z.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: SessionId::new("victim"),
        object_id,
        spell,
        direction: MirDirection::Up,
        target,
        cast: true,
        level: 2,
        damage: 40,
        mp_cost: 7,
        cooldown_ms: 1000,
        item_param: 1,
        now_ms: now,
    })
}

#[test]
fn shared_human_projectile_death_is_born_at_real_deferred_impact_and_rechecks_lives() {
    for invalidation in [
        "none", "logout", "rejoin", "cold", "revive", "safe", "group", "armour",
    ] {
        let (mut z, _) = fixture_with_attacker_class(3, MirClass::Wizard);
        set_human_pet_damage(&mut z, "victim");
        z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: SessionId::new("owner"),
            hp: 5,
            max_hp: 5,
            mp: 1000,
            dead: false,
        });
        let cast = cast_human_damage(&mut z, 101, Spell::FireBall, p(10, 20), 620);
        assert_eq!(hp(&z, "owner"), 5);
        assert!(
            !cast
                .iter()
                .any(|o| matches!(o, ZoneOutbound::OwnedPetPlayerKill { .. }))
        );
        assert!(packets(&cast, "victim").iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::FireBall,
                cast: true,
                ..
            }
        )));
        match invalidation {
            "logout" | "rejoin" => {
                z.handle(ZoneCommand::Leave {
                    session_id: SessionId::new("victim"),
                });
                if invalidation == "rejoin" {
                    let mut fresh = join("victim", 102, p(10, 21));
                    fresh.class = MirClass::Wizard;
                    z.handle(ZoneCommand::Join(fresh));
                }
            }
            "cold" => z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap(),
            "revive" => {
                z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: SessionId::new("owner"),
                    hp: 0,
                    max_hp: 5,
                    mp: 1000,
                    dead: true,
                });
                z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: SessionId::new("owner"),
                    hp: 5,
                    max_hp: 5,
                    mp: 1000,
                    dead: false,
                });
            }
            "safe" => sync_pet_safe(&mut z, "owner", true),
            "group" => {
                let mut source = z.player_chat_profile(&SessionId::new("victim")).unwrap();
                source.attack_mode = 1;
                z.handle(ZoneCommand::UpdateChatProfile {
                    session_id: SessionId::new("victim"),
                    profile: source,
                });
                let mut target = z.player_chat_profile(&SessionId::new("owner")).unwrap();
                target.group_members = vec!["victim".into()];
                z.handle(ZoneCommand::UpdateChatProfile {
                    session_id: SessionId::new("owner"),
                    profile: target,
                });
            }
            "armour" => {
                let mut stats = mir2_simulation::ZonePlayerCombatStats::default();
                stats.min_mac = 1000;
                stats.max_mac = 1000;
                z.handle(ZoneCommand::UpdatePlayerCombatStats {
                    session_id: SessionId::new("owner"),
                    stats,
                });
            }
            _ => {}
        }
        z.tick(1169);
        assert_eq!(hp(&z, "owner"), 5, "case={invalidation}");
        let impact = z.tick(1170);
        let killed = invalidation == "none";
        assert_eq!(hp(&z, "owner") == 0, killed, "case={invalidation}");
        let receipts = impact
            .iter()
            .filter_map(|o| match o {
                ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(receipts.len(), usize::from(killed), "case={invalidation}");
        if let Some(receipt) = receipts.first() {
            assert!(receipt.direct_player && receipt.unlawful);
            assert_eq!(
                (
                    receipt.at_ms,
                    receipt.owner_object_id,
                    receipt.victim_object_id
                ),
                (1170, 102, 101)
            );
        }
        assert!(
            !z.tick(1171)
                .iter()
                .any(|o| matches!(o, ZoneOutbound::OwnedPetPlayerKill { .. }))
        );
    }
}

#[test]
fn shared_human_poisoning_applies_after_500ms_ticks_strictly_and_issues_deferred_pk() {
    let (mut z, _) = fixture(3);
    set_human_pet_damage(&mut z, "victim");
    cast_human_damage(&mut z, 101, Spell::Poisoning, p(10, 20), 620);
    assert!(!packets(&z.tick(1119), "owner").iter().any(
        |p| matches!(p,ServerPacket::ObjectPoisoned { object_id: 101, poison } if poison & 1 != 0)
    ));
    let applied = z.tick(1120);
    assert!(packets(&applied, "owner").iter().any(
        |p| matches!(p,ServerPacket::ObjectPoisoned { object_id: 101, poison } if poison & 1 != 0)
    ));
    assert_eq!(hp(&z, "owner"), 1000);
    let first = z.tick(1121);
    let after = hp(&z, "owner");
    assert!(after < 1000);
    assert!(!packets(&first, "owner").iter().any(|p| matches!(
        p,
        ServerPacket::Struck { .. } | ServerPacket::ObjectStruck { .. }
    )));
    z.tick(3121);
    assert_eq!(hp(&z, "owner"), after);
    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("owner"),
        hp: 1,
        max_hp: 1000,
        mp: 1000,
        dead: false,
    });
    let fatal = z.tick(3122);
    assert_eq!(hp(&z, "owner"), 0);
    let receipt = fatal
        .iter()
        .find_map(|o| match o {
            ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
            _ => None,
        })
        .unwrap();
    assert!(receipt.direct_player && receipt.unlawful);
    assert_eq!((receipt.at_ms, receipt.owner_object_id), (3122, 102));
}

#[test]
fn shared_human_poison_cloud_hits_real_player_and_owned_pet_then_owned_dot() {
    for target_pet in [false, true] {
        let (mut z, pet) = fixture(3);
        set_human_pet_damage(&mut z, "victim");
        let location = if target_pet { p(11, 20) } else { p(10, 20) };
        let before = if target_pet {
            pet_hp(&z, pet)
        } else {
            hp(&z, "owner")
        };
        cast_human_damage(&mut z, 0, Spell::PoisonCloud, location, 620);
        z.tick(1169);
        assert_eq!(
            if target_pet {
                pet_hp(&z, pet)
            } else {
                hp(&z, "owner")
            },
            before
        );
        let applied = z.tick(1170);
        let after = if target_pet {
            pet_hp(&z, pet)
        } else {
            hp(&z, "owner")
        };
        assert!(after < before);
        let target_id = if target_pet { pet } else { 101 };
        assert!(packets(&applied,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectPoisoned { object_id, poison } if *object_id==target_id && poison & 1 != 0)));
        let dot = z.tick(1171);
        assert!(
            if target_pet {
                pet_hp(&z, pet)
            } else {
                hp(&z, "owner")
            } < after
        );
        assert!(packets(&dot,"owner").iter().any(|p|matches!(p,ServerPacket::DamageIndicator { object_id, damage, .. } if *object_id==target_id && *damage>0)));
        z.handle(ZoneCommand::Leave {
            session_id: SessionId::new("victim"),
        });
        let after_dot = if target_pet {
            pet_hp(&z, pet)
        } else {
            hp(&z, "owner")
        };
        let cancelled = z.tick(2172);
        assert_eq!(
            if target_pet {
                pet_hp(&z, pet)
            } else {
                hp(&z, "owner")
            },
            after_dot
        );
        assert!(packets(&cancelled,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectPoisoned { object_id, poison } if *object_id==target_id && poison & 1==0)));
    }
}

fn cast_monster_control(
    z: &mut ZoneRuntime,
    pet: u32,
    spell: Spell,
    level: u8,
    now: u64,
) -> Vec<ZoneOutbound> {
    let position = z
        .native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position
        .clone();
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("victim"),
        object_id: pet,
        spell,
        direction: MirDirection::UpRight,
        target: position,
        cast: true,
        level,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1,
        now_ms: now,
    })
}

#[test]
fn shared_human_binding_shot_reaches_actual_owned_monster_and_releases_neighbours_on_hit() {
    let (mut z, pet) = fixture_with_attacker_class(3, MirClass::Archer);
    let neighbour = spawn(&mut z, "owner", p(12, 20), Spell::SummonShinsu, 2010);
    let pets = z.native_monster_snapshots();
    let center = &pets.iter().find(|m| m.object_id == pet).unwrap().position;
    let nearby = &pets
        .iter()
        .find(|m| m.object_id == neighbour)
        .unwrap()
        .position;
    assert!((center.x - nearby.x).abs() <= 1 && (center.y - nearby.y).abs() <= 1);
    set_human_pet_damage(&mut z, "victim");
    let accepted = cast_monster_control(&mut z, pet, Spell::BindingShot, 2, 2620);
    assert!(packets(&accepted, "victim").iter().any(|p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::BindingShot,
            cast: true,
            ..
        }
    )));
    assert!(
        !packets(&z.tick(3169), "owner")
            .iter()
            .any(|p| matches!(p, ServerPacket::SetBindingShot { .. }))
    );
    let applied = z.tick(3170);
    assert!(packets(&applied,"owner").iter().any(|p|matches!(p,ServerPacket::SetBindingShot{object_id,enabled:true,value:20000} if *object_id==pet)));
    assert!(packets(&applied,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id==pet && *name_colour_argb==0xFFCD853F_u32 as i32)));
    assert!(packets(&applied,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id==neighbour && *name_colour_argb==0xFFCD853F_u32 as i32)));
    // An ordinary arrow really damages the shocked pet; binding is not an
    // invulnerability or a synthetic test-only flag.
    let position = z
        .native_monster_snapshots()
        .iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position
        .clone();
    let before = pet_hp(&z, pet);
    z.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: SessionId::new("victim"),
        object_id: pet,
        direction: MirDirection::UpRight,
        target: position,
        spell: Spell::None,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 3500,
    });
    let released = z.tick(3600);
    assert!(pet_hp(&z, pet) < before);
    assert!(packets(&released, "owner").iter().any(
        |p| matches!(p,ServerPacket::SetBindingShot{object_id,enabled:false,..} if *object_id==pet)
    ));
    // RefreshNameColour restores original PetLevel2 Aquamarine after Shock.
    assert!(packets(&released,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id==neighbour && *name_colour_argb==0xFF7FFFD4_u32 as i32)));
}

#[test]
fn shared_human_electric_shock_is_delayed_and_its_source_colour_expires_exactly() {
    let (mut z, pet) = fixture_with_attacker_class(3, MirClass::Wizard);
    let mut started = None;
    // Level3 passes the first original roll. A foreign non-tameable pet
    // receives Shock only on the second 1/2 roll; exercise that real RNG.
    for attempt in 0..100 {
        let now = 620 + attempt * 1000;
        cast_monster_control(&mut z, pet, Spell::ElectricShock, 3, now);
        assert!(!packets(&z.tick(now + 499), "owner").iter().any(
            |p| matches!(p,ServerPacket::ObjectColourChanged{object_id,..} if *object_id==pet)
        ));
        let applied = z.tick(now + 500);
        if packets(&applied,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id==pet && *name_colour_argb==0xFFCD853F_u32 as i32)) {
            started=Some(now+500); break;
        }
    }
    let start = started.expect("100 actual source rolls must produce a shock");
    let expiry = start + 25000;
    assert!(
        !packets(&z.tick(expiry - 1), "owner").iter().any(
            |p| matches!(p,ServerPacket::ObjectColourChanged{object_id,..} if *object_id==pet)
        )
    );
    let ended = z.tick(expiry);
    assert!(packets(&ended,"owner").iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id==pet && *name_colour_argb==0xFF7FFFD4_u32 as i32)));
    assert!(
        !packets(&z.tick(expiry + 1), "owner").iter().any(
            |p| matches!(p,ServerPacket::ObjectColourChanged{object_id,..} if *object_id==pet)
        )
    );
}

#[test]
fn shared_human_electric_shock_really_tames_source_monster_and_releases_at_one_hour() {
    let (mut z, _) = fixture_with_attacker_class(3, MirClass::Wizard);
    let template = mir2_game_data::crystal_monster_by_name("Oma").unwrap();
    assert!(template.can_tame && template.ai == 0);
    let monster = 9000;
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("victim"),
        now_ms: 520,
        monster: mir2_simulation::ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: monster,
            name: template.name.clone(),
            name_colour_argb: -1,
            image: template.image,
            ai: template.ai,
            disposition: Some(mir2_simulation::WorldEntityDisposition::Hostile),
            level: template.level,
            hp: template.hp,
            max_hp: template.hp,
            experience: template.experience,
            move_speed_ms: u64::from(template.move_speed),
            attack_speed_ms: u64::from(template.attack_speed),
            friendly_guild: None,
            position: p(12, 21),
            direction: MirDirection::Left,
            defense: mir2_simulation::ZoneMonsterDefense::from_crystal_template(&template),
            respawn: None,
            drops: Vec::new(),
        },
    });
    let mut tamed_at = None;
    for attempt in 0..200 {
        let now = 620 + attempt * 1000;
        cast_monster_control(&mut z, monster, Spell::ElectricShock, 3, now);
        let resolved = z.tick(now + 500);
        if packets(&resolved,"victim").iter().any(|p|matches!(p,ServerPacket::ObjectName{object_id,name} if *object_id==monster && name=="Oma(victim)")) {
            tamed_at=Some(now+500);break;
        }
    }
    let tamed = tamed_at.expect("real source tame rolls must create an owned Oma");
    assert!(
        !z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == monster)
            .unwrap()
            .hostile_to_player
    );
    // The new owner sees the pet as a real owned entity and can select it
    // under source All mode; this is not only a visual master id.
    assert!(z.player_can_attack_owned_pet(&SessionId::new("victim"), monster, tamed));
    assert!(!packets(&z.tick(tamed+3_600_000-1),"victim").iter().any(|p|matches!(p,ServerPacket::ObjectName{object_id,name} if *object_id==monster && name=="Oma")));
    let released = z.tick(tamed + 3_600_000);
    assert!(packets(&released,"victim").iter().any(|p|matches!(p,ServerPacket::ObjectName{object_id,name} if *object_id==monster && name=="Oma")));
    assert!(!z.player_can_attack_owned_pet(&SessionId::new("victim"), monster, tamed + 3_600_000));
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == monster)
            .unwrap()
            .hostile_to_player
    );
}

#[test]
fn shared_human_vampirism_heals_source_after_strict_delay_in_ten_hp_chunks() {
    let (mut z, _) = fixture(3);
    set_human_pet_damage(&mut z, "victim");
    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("victim"),
        hp: 500,
        max_hp: 1000,
        mp: 1000,
        dead: false,
    });
    cast_human_damage(&mut z, 101, Spell::Vampirism, p(10, 20), 620);
    assert_eq!(hp(&z, "owner"), 1000);
    z.tick(1120);
    assert!(hp(&z, "owner") < 1000);
    assert_eq!(hp(&z, "victim"), 500);
    z.tick(2120);
    assert_eq!(hp(&z, "victim"), 500);
    z.tick(2121);
    assert_eq!(hp(&z, "victim"), 510);
    z.tick(2621);
    assert_eq!(hp(&z, "victim"), 510);
    z.tick(2622);
    assert_eq!(hp(&z, "victim"), 520);
}

#[test]
fn shared_human_poison_death_retains_global_node_across_maps_and_revokes_real_new_lives() {
    for change in [
        "source_map",
        "target_map",
        "both_maps",
        "source_dead",
        "source_revive",
        "source_leave",
        "source_rejoin",
        "cold",
    ] {
        let mut manager = ZoneManager::new();
        let mut source = join("victim", 102, p(10, 21));
        source.combat_stats.min_sc = 40;
        source.combat_stats.max_sc = 40;
        manager.join(source.clone());
        let mut target = join("owner", 101, p(10, 20));
        manager.join(target.clone());
        manager.handle(ZoneCommand::PlayerCastMagicWithItem {
            session_id: SessionId::new("victim"),
            object_id: 101,
            spell: Spell::Poisoning,
            direction: MirDirection::Up,
            target: p(10, 20),
            cast: true,
            level: 2,
            damage: 40,
            mp_cost: 7,
            cooldown_ms: 1000,
            item_param: 1,
            now_ms: 620,
        });
        manager.tick_all(1120);
        manager.tick_all(1121);
        assert!(
            manager.player_vitals(&SessionId::new("owner")).unwrap().0 < 1000,
            "{change}"
        );
        manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: SessionId::new("owner"),
            hp: 1,
            max_hp: 1000,
            mp: 1000,
            dead: false,
        });
        if change == "source_map" || change == "both_maps" {
            source.map_file_name = MAP_B.into();
            source.position = map_point(MAP_B, 30, 30);
            manager.join(source.clone());
        }
        if change == "target_map" || change == "both_maps" {
            target.map_file_name = MAP_C.into();
            target.position = map_point(MAP_C, 40, 40);
            target.hp = 1;
            manager.join(target.clone());
        }
        if change == "source_dead" || change == "source_revive" {
            manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: SessionId::new("victim"),
                hp: 0,
                max_hp: 1000,
                mp: 1000,
                dead: true,
            });
            if change == "source_revive" {
                manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: SessionId::new("victim"),
                    hp: 1000,
                    max_hp: 1000,
                    mp: 1000,
                    dead: false,
                });
            }
        }
        if change == "source_leave" || change == "source_rejoin" {
            manager.handle(ZoneCommand::Leave {
                session_id: SessionId::new("victim"),
            });
            if change == "source_rejoin" {
                manager.join(source);
            }
        }
        if change == "cold" {
            manager =
                ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap();
        }
        manager.tick_all(3121);
        assert_eq!(
            manager.player_vitals(&SessionId::new("owner")).unwrap().0,
            1,
            "strict clock {change}"
        );
        let fatal = manager.tick_all(3122);
        let should_die = matches!(
            change,
            "source_map" | "target_map" | "both_maps" | "source_dead"
        );
        assert_eq!(
            manager.player_vitals(&SessionId::new("owner")).unwrap().0 == 0,
            should_die,
            "{change}"
        );
        let receipts = fatal
            .iter()
            .filter_map(|o| {
                if let ZoneOutbound::OwnedPetPlayerKill { receipt } = o {
                    Some(receipt)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(receipts.len(), usize::from(should_die), "{change}");
        if let Some(receipt) = receipts.first() {
            assert!(receipt.direct_player && receipt.unlawful);
            assert_eq!(
                (
                    receipt.owner_object_id,
                    receipt.victim_object_id,
                    receipt.at_ms
                ),
                (102, 101, 3122)
            );
            assert!(manager.issued_owned_pet_kill_is_current(receipt));
        }
        assert!(
            !manager
                .tick_all(3123)
                .iter()
                .any(|o| matches!(o, ZoneOutbound::OwnedPetPlayerKill { .. }))
        );
    }
}

fn real_vampire_summon(owner_hp: i32) -> (ZoneRuntime, u32) {
    real_archer_summon(Spell::SummonVampire, owner_hp)
}

fn real_archer_summon(spell: Spell, owner_hp: i32) -> (ZoneRuntime, u32) {
    let mut z = ordinary_zone(MAP_A);
    let mut owner = join("owner", 101, p(10, 20));
    owner.class = MirClass::Archer;
    owner.hp = owner_hp;
    z.handle(ZoneCommand::Join(owner));
    let mut victim = join("victim", 102, p(12, 20));
    victim.class = MirClass::Warrior;
    z.handle(ZoneCommand::Join(victim));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("owner"),
        MirClass::Archer,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let mut armour = mir2_simulation::ZonePlayerCombatStats::default();
    armour.min_ac = 10_000;
    armour.max_ac = 10_000;
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("victim"),
        stats: armour,
    });
    let cast = z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 102,
        spell,
        direction: MirDirection::Right,
        target: p(12, 20),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    assert!(packets(&cast, "owner").iter().any(|packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: sent_spell,
            cast: true,
            ..
        } if *sent_spell == spell
    )));
    z.tick(1109);
    assert!(z.native_monster_snapshots().is_empty());
    let spawned = z.tick(1110);
    let id = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| {
            m.name
                == if spell == Spell::SummonVampire {
                    "VampireSpider"
                } else {
                    "SpittingToad"
                }
        })
        .unwrap()
        .object_id;
    assert!(packets(&spawned, "owner").iter().any(|packet| matches!(packet,
        ServerPacket::ObjectMonster { info } if info.object_id == id && info.master_object_id == 101)));
    (z, id)
}

#[test]
fn shared_owned_custom_vampire_and_toad_pause_modes_retain_real_assigned_target_for_focus_resume() {
    for spell in [Spell::SummonVampire, Spell::SummonToad] {
        for paused_mode in [1, 3] {
            let (mut z, pet) = real_archer_summon(spell, 1000);
            let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
            profile.pet_mode = paused_mode;
            z.handle(ZoneCommand::UpdateChatProfile {
                session_id: SessionId::new("owner"),
                profile,
            });
            let paused = z.tick(3300);
            assert_eq!(hp(&z, "victim"), 1000);
            assert!(
                packets(&paused, "owner")
                    .iter()
                    .all(|packet| !matches!(packet,
                ServerPacket::ObjectAttack { info } if info.object_id == pet))
            );
            let mut profile = z.player_chat_profile(&SessionId::new("owner")).unwrap();
            profile.pet_mode = 4;
            z.handle(ZoneCommand::UpdateChatProfile {
                session_id: SessionId::new("owner"),
                profile,
            });
            let now = if spell == Spell::SummonToad {
                6300
            } else {
                3600
            };
            let resumed = z.tick(now);
            if spell == Spell::SummonToad {
                assert!(packets(&resumed, "owner").iter().any(|packet| matches!(packet,
                    ServerPacket::ObjectRangeAttack { info } if info.object_id == pet && info.target_id == 102)));
                assert_eq!(hp(&z, "victim"), 1000);
                z.tick(now + 550);
            } else {
                assert!(
                    packets(&resumed, "owner")
                        .iter()
                        .any(|packet| matches!(packet,
                    ServerPacket::ObjectAttack { info } if info.object_id == pet))
                );
            }
            assert!(
                hp(&z, "victim") < 1000,
                "{spell:?}/{paused_mode}: actual assigned Target survived the pause"
            );
        }
    }
}

#[test]
fn shared_owned_vampire_real_summon_bites_immediately_mac_and_heals_on_strict_source_clock() {
    let (mut z, pet) = real_vampire_summon(800);
    let mut bite = None;
    for now in (3300..=7500).step_by(300) {
        let out = z.tick(now);
        if packets(&out, "victim").iter().any(|packet| {
            matches!(packet,
            ServerPacket::ObjectAttack { info } if info.object_id == pet)
        }) {
            let damage = out
                .iter()
                .find_map(|event| match event {
                    ZoneOutbound::PlayerDamaged {
                        session_id, damage, ..
                    } if session_id == &SessionId::new("victim") => Some(*damage),
                    _ => None,
                })
                .expect("the source bite settles in the same update as ObjectAttack");
            assert!(damage > 0);
            assert_eq!(
                hp(&z, "victim"),
                1000 - damage,
                "MACAgility ignores the victim's prepared AC"
            );
            assert!(
                packets(&out, "victim").iter().any(|packet| matches!(packet,
                ServerPacket::ObjectEffect { info } if info.object_id == 102 && info.effect == 18))
            );
            bite = Some((now, damage));
            break;
        }
    }
    let (now, damage) = bite.expect("actual assigned SummonVampire Target reaches its bite");
    let expected = (damage as f32 * 3.0 * 0.25) as u32 as u16;
    assert!(expected > 0);
    assert_eq!(hp(&z, "owner"), 800);
    let position = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position;
    let mut mode = z.player_chat_profile(&SessionId::new("owner")).unwrap();
    mode.pet_mode = 1;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("owner"),
        profile: mode,
    });
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: p(16, 20),
        direction: MirDirection::Right,
    });
    // Cold recovery revokes issued Human healing authority and retains the
    // exact passive-regen state. Its independent ordinary tick path prevents
    // a real 3% + 1 pulse from masquerading as early VampTime healing.
    let mut passive = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    z.tick(now + 1000);
    passive.tick(now + 1000);
    assert_eq!(
        hp(&z, "owner") - hp(&passive, "owner"),
        0,
        "source VampTime comparison is strict >"
    );
    z.tick(now + 1001);
    passive.tick(now + 1001);
    assert_eq!(
        hp(&z, "owner") - hp(&passive, "owner"),
        i32::from(expected.min(10))
    );
    z.tick(now + 1501);
    passive.tick(now + 1501);
    assert_eq!(
        hp(&z, "owner") - hp(&passive, "owner"),
        i32::from(expected.min(10)),
        "next source +500 boundary is also strict"
    );
    z.tick(now + 1502);
    passive.tick(now + 1502);
    assert_eq!(
        hp(&z, "owner") - hp(&passive, "owner"),
        i32::from(expected.min(20))
    );
    assert_eq!(
        z.native_monster_snapshots()
            .into_iter()
            .find(|m| m.object_id == pet)
            .unwrap()
            .position,
        position,
        "custom Vampire ProcessAI has no idle Master follow even in MoveOnly"
    );
}

#[test]
fn shared_owned_vampire_public_fatal_hit_releases_master_before_safe_owner_death_and_never_bills_pk()
 {
    let (mut z, pet) = real_vampire_summon(20);
    sync_pet_safe(&mut z, "owner", true);
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("victim"),
        MirClass::Warrior,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let mut lethal = mir2_simulation::ZonePlayerCombatStats::default();
    lethal.min_dc = 10_000;
    lethal.max_dc = 10_000;
    lethal.accuracy = 255;
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("victim"),
        stats: lethal,
    });
    let pet_position = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == pet)
        .unwrap()
        .position;
    assert_eq!(
        pet_position,
        p(11, 19),
        "actual occupied target spawn chooses first valid source cell"
    );
    let mut out = attack(&mut z, "victim", pet, MirDirection::UpLeft, 3300);
    out.extend(z.tick(3600));
    assert_eq!(
        pet_hp(&z, pet),
        0,
        "ordinary Human stat attack actually kills the owned spider"
    );
    assert_eq!(
        hp(&z, "owner"),
        0,
        "Master-null burst can kill its former safe Master"
    );
    assert_eq!(
        hp(&z, "victim"),
        980,
        "same burst also applies actual MAC damage to its killer"
    );
    assert!(
        packets(&out, "owner")
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Death { .. }))
    );
    assert!(
        out.iter().all(|event| !matches!(
            event,
            ZoneOutbound::OwnedPetPlayerKill { .. } | ZoneOutbound::MonsterKillAward { .. }
        )),
        "former Master is not the source of death or a newly invented EXPOwner"
    );
    let after = z.tick(5000);
    assert!(after.iter().all(|event| !matches!(
        event,
        ZoneOutbound::PlayerHealed { .. } | ZoneOutbound::OwnedPetPlayerKill { .. }
    )));
    assert_eq!(
        hp(&z, "victim"),
        980,
        "death burst is once per actual source life"
    );
}
