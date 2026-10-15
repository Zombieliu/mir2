use mir2_protocol::{
    ClientIntelligentCreature, ClientPacket, IntelligentCreatureItemFilter,
    IntelligentCreatureRules, ServerPacket,
};
use mir2_simulation::{SimulationConfig, SimulationSession, Stage5SystemsState};

fn pet(pet_type: u8, slot_index: i32) -> ClientIntelligentCreature {
    ClientIntelligentCreature {
        pet_type,
        slot_index,
        icon: 44,
        custom_name: "Buddy".into(),
        fullness: 1200,
        expire_binary_datetime: 0,
        blackstone_time: 12000,
        pet_mode: 0,
        creature_rules: IntelligentCreatureRules {
            minimal_fullness: 100,
            mouse_pickup_enabled: true,
            mouse_pickup_range: 3,
            auto_pickup_enabled: true,
            auto_pickup_range: 3,
            semi_auto_pickup_enabled: true,
            semi_auto_pickup_range: 3,
            can_produce_blackstone: true,
        },
        filter: IntelligentCreatureItemFilter {
            pet_pickup_all: true,
            pet_pickup_gold: true,
            pet_pickup_weapons: false,
            pet_pickup_armours: false,
            pet_pickup_helmets: false,
            pet_pickup_boots: false,
            pet_pickup_belts: false,
            pet_pickup_accessories: false,
            pet_pickup_others: false,
        },
        pickup_grade: 2,
        maintain_food_time: 24000,
    }
}

fn fixture(legacy: bool) -> SimulationSession {
    let config = SimulationConfig::default();
    let mut systems = Stage5SystemsState::default();
    systems.intelligent_creatures = vec![pet(1, 0), pet(2, 1)];
    if legacy {
        systems.intelligent_creatures[0].pet_mode = 1;
    }
    let mut json = serde_json::to_value(systems).unwrap();
    if legacy {
        json.as_object_mut()
            .unwrap()
            .remove("summonedIntelligentCreatureType");
    }
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        save.stage5_systems_json = Some(json.to_string());
    }
    SimulationSession::new(config)
}

fn enter(session: &mut SimulationSession) {
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
}

fn update(
    session: &mut SimulationSession,
    creature: ClientIntelligentCreature,
    flags: (bool, bool, bool),
) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::UpdateIntelligentCreature {
        creature,
        summon_me: flags.0,
        unsummon_me: flags.1,
        release_me: flags.2,
    })
}

#[test]
fn client_cannot_create_pet_or_replace_owned_identity() {
    let mut session = fixture(false);
    assert!(update(&mut session, pet(9, 8), (true, false, false)).is_empty());
    enter(&mut session);
    let before = session.world_snapshot().stage5_systems;
    for requested in [pet(9, 8), pet(9, 0), pet(1, 10), pet(1, -1)] {
        for flags in [
            (false, false, false),
            (true, false, false),
            (false, false, true),
        ] {
            let reply = update(&mut session, requested.clone(), flags);
            assert!(!reply
                .iter()
                .any(|packet| matches!(packet, ServerPacket::NewIntelligentCreature { .. })));
            assert_eq!(session.world_snapshot().stage5_systems, before);
        }
    }
}

#[test]
fn owned_pet_layout_updates_follow_crystal_type_identity_and_allow_paired_swap() {
    let mut session = fixture(false);
    enter(&mut session);
    update(&mut session, pet(1, 9), (false, false, false));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures,
        vec![pet(1, 9), pet(2, 1)]
    );
    update(&mut session, pet(1, 1), (false, false, false));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures,
        vec![pet(1, 1), pet(2, 1)]
    );
    update(&mut session, pet(2, 9), (false, false, false));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures,
        vec![pet(1, 1), pet(2, 9)]
    );
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures,
        vec![pet(1, 1), pet(2, 9)]
    );
}

#[test]
fn preferences_whitelist_preserves_authoritative_fields_and_cannot_summon() {
    let mut session = fixture(false);
    enter(&mut session);
    let original = session
        .world_snapshot()
        .stage5_systems
        .intelligent_creatures[0]
        .clone();
    let mut request = original.clone();
    request.custom_name = "NewBuddy".into();
    request.pet_mode = 1;
    request.filter.pet_pickup_gold = false;
    request.icon = 99999;
    request.fullness = i32::MAX;
    request.expire_binary_datetime = i64::MAX;
    request.blackstone_time = i64::MAX;
    request.maintain_food_time = i64::MAX;
    request.pickup_grade = 255;
    request.creature_rules.mouse_pickup_range = i32::MAX;
    update(&mut session, request, (false, false, false));
    let state = session.world_snapshot().stage5_systems;
    let mut expected = original;
    expected.custom_name = "NewBuddy".into();
    expected.pet_mode = 1;
    expected.filter.pet_pickup_gold = false;
    assert_eq!(state.intelligent_creatures[0], expected);
    assert!(state.active_intelligent_creature().is_none());
    let mut invalid = expected.clone();
    invalid.custom_name = "<script>".into();
    invalid.pet_mode = 255;
    update(&mut session, invalid, (false, false, false));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures[0],
        expected
    );
}

#[test]
fn summon_and_dismiss_are_independent_of_pickup_mode_and_single_pet_only() {
    let mut session = fixture(false);
    enter(&mut session);
    let reply = update(&mut session, pet(1, 0), (true, false, false));
    assert!(reply.iter().any(|packet| matches!(
        packet,
        ServerPacket::UpdateIntelligentCreatureList {
            creature_summoned: true,
            summoned_creature_type: 1,
            ..
        }
    )));
    assert!(session.has_active_intelligent_creature_auto_pickup());
    update(&mut session, pet(2, 1), (true, false, false));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .summoned_intelligent_creature_type,
        Some(1)
    );
    let mut semi = pet(1, 0);
    semi.pet_mode = 1;
    update(&mut session, semi.clone(), (false, false, false));
    assert!(!session.has_active_intelligent_creature_auto_pickup());
    assert!(session
        .world_snapshot()
        .stage5_systems
        .active_intelligent_creature()
        .is_some());
    update(&mut session, semi.clone(), (false, true, false));
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.intelligent_creatures[0].pet_mode, 1);
    assert!(state.active_intelligent_creature().is_none());
    update(&mut session, semi, (true, false, false));
    assert!(!session.has_active_intelligent_creature_auto_pickup());
    assert!(session
        .world_snapshot()
        .stage5_systems
        .active_intelligent_creature()
        .is_some());
}

#[test]
fn release_has_priority_clears_summon_and_compacts_slots() {
    let mut session = fixture(false);
    enter(&mut session);
    update(&mut session, pet(1, 0), (true, false, false));
    update(&mut session, pet(1, 0), (true, true, true));
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.intelligent_creatures, vec![pet(2, 0)]);
    assert_eq!(state.summoned_intelligent_creature_type, Some(99));
}

#[test]
fn old_save_migrates_summon_flag_to_automatic_mode_and_new_save_keeps_semi_mode() {
    let mut session = fixture(true);
    enter(&mut session);
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.summoned_intelligent_creature_type, Some(1));
    assert_eq!(state.intelligent_creatures[0].pet_mode, 0);
    assert!(session.has_active_intelligent_creature_auto_pickup());
    let mut semi = pet(1, 0);
    semi.pet_mode = 1;
    update(&mut session, semi, (false, false, false));
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.summoned_intelligent_creature_type, Some(1));
    assert_eq!(state.intelligent_creatures[0].pet_mode, 1);
    assert!(!session.has_active_intelligent_creature_auto_pickup());
}

#[test]
fn baby_pig_zero_summons_and_none_99_dismisses_across_reload_with_grade_preferences() {
    let config = SimulationConfig::default();
    let mut state = Stage5SystemsState::default();
    state.intelligent_creatures = vec![pet(0, 0), pet(14, 1)];
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    let mut session = SimulationSession::new(config);
    enter(&mut session);
    let packets = update(&mut session, pet(0, 0), (true, false, false));
    assert!(packets.iter().any(|p| matches!(
        p,
        ServerPacket::UpdateIntelligentCreatureList {
            creature_summoned: true,
            summoned_creature_type: 0,
            ..
        }
    )));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .active_intelligent_creature()
            .unwrap()
            .pet_type,
        0
    );
    let mut request = pet(0, 0);
    request.pickup_grade = 5;
    update(&mut session, request, (false, false, false));
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .active_intelligent_creature()
            .unwrap()
            .pickup_grade,
        5
    );
    let packets = update(&mut session, pet(0, 0), (false, true, false));
    assert!(packets.iter().any(|p| matches!(
        p,
        ServerPacket::UpdateIntelligentCreatureList {
            creature_summoned: false,
            summoned_creature_type: 99,
            ..
        }
    )));
    update(&mut session, pet(14, 1), (true, false, false));
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .active_intelligent_creature()
            .unwrap()
            .pet_type,
        14
    );
}

#[test]
fn old_zero_sentinel_migrates_without_misinterpreting_new_baby_pig() {
    let mut old: Stage5SystemsState = serde_json::from_value(
        serde_json::json!({"summonedIntelligentCreatureType":0,"intelligentCreatures":[pet(0,0)]}),
    )
    .unwrap();
    old.migrate_legacy_intelligent_creature_state();
    assert_eq!(old.summoned_intelligent_creature_type, Some(99));
    let mut current = Stage5SystemsState::default();
    current.intelligent_creatures = vec![pet(0, 0)];
    current.summoned_intelligent_creature_type = Some(0);
    current.migrate_legacy_intelligent_creature_state();
    assert_eq!(current.active_intelligent_creature().unwrap().pet_type, 0);
}
