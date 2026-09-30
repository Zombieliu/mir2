use super::*;
use crate::{InProcessWorldRuntime, SimulationConfig, WorldCommand, WorldRuntime};
use mir2_game_data::MapBounds;
use mir2_protocol::{
    ClientIntelligentCreature, ClientPacket, IntelligentCreatureItemFilter,
    IntelligentCreatureRules, MirDirection, Point,
};

fn assert_lightweight_matches_full(session: &SimulationSession) {
    let lightweight = session.local_player_vitals_snapshot();
    let full = session.world_snapshot();
    assert_eq!(lightweight.player_object_id, full.player_object_id);
    assert_eq!(lightweight.player_hp, full.player_hp);
    assert_eq!(lightweight.player_max_hp, full.player_max_hp);
    assert_eq!(lightweight.player_mp, full.player_mp);
    assert_eq!(session.current_map_file_name(), full.map_file_name);
}

fn started_session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}

#[test]
fn lightweight_player_projection_matches_full_snapshot_before_and_after_login() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert_lightweight_matches_full(&session);
    assert_eq!(session.current_map_file_name(), None);
    assert_eq!(session.local_player_position(), None);

    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert_lightweight_matches_full(&session);
    assert_eq!(session.current_map_file_name(), None);
    assert_eq!(session.local_player_position(), None);

    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_lightweight_matches_full(&session);
    assert!(session.current_map_file_name().is_some());
    let full = session.world_snapshot();
    let full_position = full
        .entities
        .iter()
        .find(|entity| entity.kind == crate::WorldEntityKind::SelfPlayer)
        .map(|entity| Point {
            x: entity.x,
            y: entity.y,
        });
    assert_eq!(session.local_player_position(), full_position);
}

#[test]
fn lightweight_player_projection_tracks_death_revive_and_max_hp_reconciliation() {
    let mut session = started_session();
    session.force_authoritative_player_vitals_with_max_hp(Some(0), Some(321), Some(7));
    assert_lightweight_matches_full(&session);
    assert_eq!(session.local_player_vitals_snapshot().player_hp, Some(0));
    assert_eq!(
        session.local_player_vitals_snapshot().player_max_hp,
        Some(321)
    );

    session.force_authoritative_player_vitals(Some(123), Some(11));
    assert_lightweight_matches_full(&session);
    let revived = session.local_player_vitals_snapshot();
    assert_eq!(revived.player_hp, Some(123));
    assert_eq!(revived.player_mp, Some(11));
}

#[test]
fn lightweight_map_projection_tracks_transfer_without_building_world_snapshot() {
    let mut config = SimulationConfig::default();
    config.map_transfers = vec![crate::MapTransferRecord {
        key: "test-to-map2".into(),
        from_map_file_name: "0".into(),
        from_bounds: MapBounds {
            min_x: 339,
            max_x: 341,
            min_y: 268,
            max_y: 271,
        },
        to_map_file_name: "2".into(),
        to_map_title: "SerpentValley".into(),
        to_position: Point { x: 460, y: 422 },
        to_direction: MirDirection::Down,
        conquest_index: 0,
    }];
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.force_authoritative_player_transform(
        Point { x: 340, y: 269 },
        MirDirection::Down,
    );
    session.transfer_map("test-to-map2");
    assert_lightweight_matches_full(&session);
    assert_eq!(session.current_map_file_name().as_deref(), Some("2"));
}

#[test]
fn in_process_runtime_forwards_lightweight_projection_exactly() {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    assert_eq!(runtime.current_map_file_name(), None);
    assert_eq!(runtime.local_player_position(), None);
    assert_eq!(runtime.active_intelligent_creature_snapshot(), None);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .unwrap();

    let lightweight = runtime.local_player_vitals_snapshot();
    let full = WorldRuntime::world_snapshot(&runtime);
    assert_eq!(lightweight.player_object_id, full.player_object_id);
    assert_eq!(lightweight.player_hp, full.player_hp);
    assert_eq!(lightweight.player_max_hp, full.player_max_hp);
    assert_eq!(lightweight.player_mp, full.player_mp);
    assert_eq!(runtime.current_map_file_name(), full.map_file_name);

    let forced_position = Point { x: 123, y: 234 };
    runtime.force_authoritative_player_transform(forced_position.clone(), MirDirection::Left);
    assert_eq!(runtime.local_player_position(), Some(forced_position));
}

fn intelligent_creature_fixture() -> ClientIntelligentCreature {
    ClientIntelligentCreature {
        pet_type: 1,
        icon: 44,
        custom_name: "Buddy".into(),
        fullness: 1_200,
        slot_index: 0,
        expire_binary_datetime: 638_000_000_000_000_000,
        blackstone_time: 12_000,
        pet_mode: 1,
        creature_rules: IntelligentCreatureRules {
            minimal_fullness: 0,
            mouse_pickup_enabled: false,
            mouse_pickup_range: 0,
            auto_pickup_enabled: false,
            auto_pickup_range: 0,
            semi_auto_pickup_enabled: false,
            semi_auto_pickup_range: 0,
            can_produce_blackstone: true,
        },
        filter: IntelligentCreatureItemFilter {
            pet_pickup_all: false,
            pet_pickup_gold: true,
            pet_pickup_weapons: false,
            pet_pickup_armours: false,
            pet_pickup_helmets: false,
            pet_pickup_boots: false,
            pet_pickup_belts: false,
            pet_pickup_accessories: false,
            pet_pickup_others: true,
        },
        pickup_grade: 2,
        maintain_food_time: 24_000,
    }
}

#[test]
fn lightweight_active_intelligent_creature_matches_full_snapshot_selection() {
    let mut session = started_session();
    assert_eq!(session.active_intelligent_creature_snapshot(), None);

    let creature = intelligent_creature_fixture();
    {
        let mut stage5 = session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.intelligent_creatures.push(creature.clone());
        stage5.stage5_systems.summoned_intelligent_creature_type = Some(creature.pet_type);
    }

    let lightweight = session.active_intelligent_creature_snapshot();
    let full = session
        .world_snapshot()
        .stage5_systems
        .active_intelligent_creature()
        .cloned();
    assert_eq!(lightweight, full);
    assert_eq!(lightweight, Some(creature));
}
use crate::runtime::resources::Stage5SystemsResource;
