use mir2_game_data::crystal_map_events::{
    crystal_map_event_manifest, CrystalMapCoordinateAction, CrystalMapCoordinateComparison,
    CrystalMapCoordinateConditionKind,
};
use mir2_protocol::{ChatType, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound,
    ZoneRuntime,
};

#[derive(Clone)]
struct E1Case {
    map: &'static str,
    source: Point,
    threshold: i32,
    condition: CrystalMapCoordinateConditionKind,
    hint: &'static str,
    target_map: &'static str,
    destination: Point,
    binding_line: u32,
    script_file: &'static str,
}

const PENAL_HINT: &str = "A Mysterious evil force seems to be pushing you back.";
const DOGYO_HINT: &str = "Doors are locked until you reach level 50!";

const E1_CASES: [E1Case; 6] = [
    E1Case {
        map: "3",
        source: Point { x: 861, y: 686 },
        threshold: 199,
        condition: CrystalMapCoordinateConditionKind::PkPoints,
        hint: PENAL_HINT,
        target_map: "D1801",
        destination: Point { x: 128, y: 171 },
        binding_line: 12,
        script_file: "SystemScripts/00Default/MapCoords/PenalCavern.txt",
    },
    E1Case {
        map: "3",
        source: Point { x: 862, y: 687 },
        threshold: 199,
        condition: CrystalMapCoordinateConditionKind::PkPoints,
        hint: PENAL_HINT,
        target_map: "D1801",
        destination: Point { x: 128, y: 171 },
        binding_line: 41,
        script_file: "SystemScripts/00Default/MapCoords/PenalCavern.txt",
    },
    E1Case {
        map: "DogYoArena2",
        source: Point { x: 117, y: 26 },
        threshold: 49,
        condition: CrystalMapCoordinateConditionKind::Level,
        hint: DOGYO_HINT,
        target_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
        binding_line: 16,
        script_file: "SystemScripts/00Default/MapCoords/DogYoArena.txt",
    },
    E1Case {
        map: "DogYoArena2",
        source: Point { x: 118, y: 27 },
        threshold: 49,
        condition: CrystalMapCoordinateConditionKind::Level,
        hint: DOGYO_HINT,
        target_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
        binding_line: 19,
        script_file: "SystemScripts/00Default/MapCoords/DogYoArena.txt",
    },
    E1Case {
        map: "DogYoArena2",
        source: Point { x: 119, y: 28 },
        threshold: 49,
        condition: CrystalMapCoordinateConditionKind::Level,
        hint: DOGYO_HINT,
        target_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
        binding_line: 22,
        script_file: "SystemScripts/00Default/MapCoords/DogYoArena.txt",
    },
    E1Case {
        map: "DogYoArena2",
        source: Point { x: 119, y: 29 },
        threshold: 49,
        condition: CrystalMapCoordinateConditionKind::Level,
        hint: DOGYO_HINT,
        target_map: "DogYoHyun",
        destination: Point { x: 21, y: 765 },
        binding_line: 25,
        script_file: "SystemScripts/00Default/MapCoords/DogYoArena.txt",
    },
];

#[test]
fn e1_manifest_has_all_six_exact_typed_crystal_bindings() {
    let manifest = crystal_map_event_manifest();
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.general_event_scripts.status, "open");
    assert_eq!(manifest.typed_map_coordinate_bindings.len(), E1_CASES.len());

    for case in E1_CASES {
        let binding = manifest
            .typed_map_coordinate_bindings
            .iter()
            .find(|binding| {
                binding.map_id == case.map
                    && binding.x == case.source.x
                    && binding.y == case.source.y
            })
            .unwrap_or_else(|| panic!("missing E1 binding for {} {:?}", case.map, case.source));
        assert_eq!(
            binding.binding_source_file,
            "SystemScripts/00Default/MapCoords.txt"
        );
        assert_eq!(binding.binding_source_line, case.binding_line);
        assert_eq!(binding.conditions.len(), 1);
        assert_eq!(binding.conditions[0].kind, case.condition);
        assert_eq!(
            binding.conditions[0].operator,
            CrystalMapCoordinateComparison::GreaterThan
        );
        assert_eq!(binding.conditions[0].value, case.threshold);
        assert_eq!(binding.conditions[0].source_file, case.script_file);
        assert_eq!(binding.conditions[0].source_line, 4);
        assert!(matches!(
            binding.on_pass,
            CrystalMapCoordinateAction::EnterMap { ref source_file, source_line }
                if source_file == case.script_file && source_line == 6
        ));
        assert!(matches!(
            &binding.on_fail,
            CrystalMapCoordinateAction::LocalMessage { message, chat_type, source_file, source_line }
                if message == case.hint
                    && chat_type == "Hint"
                    && source_file == case.script_file
                    && *source_line == 8
        ));
        assert_eq!(binding.need_move.source, case.source);
        assert_eq!(binding.need_move.target_map_file_name, case.target_map);
        assert_eq!(binding.need_move.destination, case.destination);
        assert_eq!(
            binding.need_move.source_file,
            "SystemScripts/00Default/MapCoords.txt"
        );
        assert_eq!(binding.need_move.source_line, case.binding_line);
    }
}

fn join_for(case: &E1Case, suffix: &str, allowed: bool) -> ZoneJoin {
    let (level, pk_points) = match case.condition {
        CrystalMapCoordinateConditionKind::Level => (if allowed { 50 } else { 49 }, 0),
        CrystalMapCoordinateConditionKind::PkPoints => (1, if allowed { 200 } else { 199 }),
    };
    let mut chat_profile = ZoneChatProfile::default();
    chat_profile.pk_points = pk_points;
    ZoneJoin {
        session_id: SessionId::new(format!("e1-{suffix}")),
        account_id: format!("e1-{suffix}-account"),
        character_index: 0,
        object_id: 90_000,
        name: format!("E1{suffix}"),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: case.map.to_string(),
        position: Point {
            x: case.source.x - 1,
            y: case.source.y - 1,
        },
        direction: MirDirection::DownRight,
        chat_profile,
        combat_stats: Default::default(),
    }
}

fn walk_to_e1_source(zone: &mut ZoneRuntime, session_id: SessionId) -> Vec<ZoneOutbound> {
    let mut outbounds = zone.handle(ZoneCommand::Walk {
        session_id: session_id.clone(),
        direction: MirDirection::DownRight,
        seq: 1,
        now_ms: 1,
    });
    outbounds.extend(zone.handle(ZoneCommand::TickPlayerMovement {
        session_id,
        now_ms: 1,
    }));
    outbounds
}

fn has_hint(outbounds: &[ZoneOutbound], session_id: &SessionId, hint: &str) -> bool {
    outbounds.iter().any(|outbound| {
        matches!(
            outbound,
            ZoneOutbound::ToSession { session_id: owner, packets }
                if owner == session_id && packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::Chat { message, chat_type: ChatType::Hint } if message == hint
                ))
        )
    })
}

fn emits_map_information(outbounds: &[ZoneOutbound]) -> bool {
    outbounds.iter().any(|outbound| match outbound {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })),
        _ => false,
    })
}

fn turn_at_e1_source(
    zone: &mut ZoneRuntime,
    session_id: SessionId,
    direction: MirDirection,
) -> Vec<ZoneOutbound> {
    let mut outbounds = zone.handle(ZoneCommand::Turn {
        session_id: session_id.clone(),
        direction,
        now_ms: 10_000,
    });
    outbounds.extend(zone.handle(ZoneCommand::TickPlayerMovement {
        session_id,
        now_ms: 10_000,
    }));
    outbounds
}

#[test]
fn e1_boundaries_deny_at_threshold_and_zone_does_not_transform_early() {
    for (index, case) in E1_CASES.iter().enumerate() {
        let denied_join = join_for(case, &format!("{index}-denied"), false);
        let denied_id = denied_join.session_id.clone();
        let mut denied_zone =
            ZoneRuntime::new_with_collision(ZoneKey::for_map(case.map), ZoneCollision::unbounded());
        denied_zone.handle(ZoneCommand::Join(denied_join));
        let denied_outbounds = walk_to_e1_source(&mut denied_zone, denied_id.clone());
        assert_eq!(
            denied_zone.player_position(&denied_id),
            Some(case.source.clone())
        );
        assert!(has_hint(&denied_outbounds, &denied_id, case.hint));
        assert!(!emits_map_information(&denied_outbounds));

        let allowed_join = join_for(case, &format!("{index}-allowed"), true);
        let allowed_id = allowed_join.session_id.clone();
        let mut allowed_zone =
            ZoneRuntime::new_with_collision(ZoneKey::for_map(case.map), ZoneCollision::unbounded());
        allowed_zone.handle(ZoneCommand::Join(allowed_join));
        let allowed_outbounds = walk_to_e1_source(&mut allowed_zone, allowed_id.clone());
        assert_eq!(
            allowed_zone.player_position(&allowed_id),
            Some(case.source.clone()),
            "E1 admission reaches the source cell only; the session/gateway transfer remains authoritative"
        );
        assert!(!has_hint(&allowed_outbounds, &allowed_id, case.hint));
        assert!(!emits_map_information(&allowed_outbounds));
    }
}

#[test]
fn zone_turn_applies_direction_before_current_coordinate_map_event() {
    let denied_case = &E1_CASES[0];
    let denied_join = join_for(denied_case, "turn-denied", false);
    let denied_id = denied_join.session_id.clone();
    let mut denied_zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map(denied_case.map),
        ZoneCollision::unbounded(),
    );
    denied_zone.handle(ZoneCommand::Join(denied_join));
    let _ = walk_to_e1_source(&mut denied_zone, denied_id.clone());
    let denied_outbounds =
        turn_at_e1_source(&mut denied_zone, denied_id.clone(), MirDirection::Left);
    assert_eq!(
        denied_zone.player_position(&denied_id),
        Some(denied_case.source.clone())
    );
    assert_eq!(
        denied_zone.player_direction(&denied_id),
        Some(MirDirection::Left)
    );
    assert!(has_hint(&denied_outbounds, &denied_id, denied_case.hint));
    assert!(!emits_map_information(&denied_outbounds));

    let allowed_case = &E1_CASES[0];
    let allowed_join = join_for(allowed_case, "turn-allowed", true);
    let allowed_id = allowed_join.session_id.clone();
    let mut allowed_zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map(allowed_case.map),
        ZoneCollision::unbounded(),
    );
    allowed_zone.handle(ZoneCommand::Join(allowed_join));
    let _ = walk_to_e1_source(&mut allowed_zone, allowed_id.clone());
    let allowed_outbounds =
        turn_at_e1_source(&mut allowed_zone, allowed_id.clone(), MirDirection::Left);
    assert_eq!(
        allowed_zone.player_position(&allowed_id),
        Some(allowed_case.source.clone())
    );
    assert_eq!(
        allowed_zone.player_direction(&allowed_id),
        Some(MirDirection::Left)
    );
    assert!(!has_hint(
        &allowed_outbounds,
        &allowed_id,
        allowed_case.hint
    ));
    assert!(!emits_map_information(&allowed_outbounds));
}
