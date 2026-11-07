use super::*;
use mir2_simulation::VisibleNpcRecord;

const SOURCE: Point = Point { x: 288, y: 616 };
const DESTINATION: Point = Point { x: 296, y: 222 };
const GILBERT: u32 = 4991;

#[test]
fn same_map_paid_npc_move_updates_zone_and_survives_normal_relogin() {
    let mut config = GatewayConfig::default();
    config.spawn = SOURCE.clone();
    config.visible_monsters.clear();
    config.map_hazards.clear();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: GILBERT,
        name: "Teleport_Gilbert".into(),
        image: 15,
        colour_argb: -1,
        position: Point { x: 287, y: 615 },
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/BorderVillage/Transport".into()),
    });
    {
        // Test fixture only. The commands under test require an authenticated
        // player and use the actual NPC's menus and script, never a debug MOVE.
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        save.position = SOURCE.clone();
        save.gold = 5_000;
    }
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    shared.lock().unwrap().experience_authority = Some(config);
    let mut runtime = shared_session_runtime(shared.clone());
    let enter = |runtime: &mut SharedInProcessZoneSessionRuntime| {
        let login = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            }))
            .unwrap();
        assert!(login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let start = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .unwrap();
        assert!(start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    };
    let zone_position = |runtime: &SharedInProcessZoneSessionRuntime| {
        let id = runtime.current_zone_session_id().unwrap();
        shared
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&id)
            .unwrap()
            .0
    };
    let select = |runtime: &mut SharedInProcessZoneSessionRuntime, target: &str| {
        runtime
            .execute(WorldCommand::SelectNpcDialog {
                target: target.into(),
            })
            .unwrap()
    };
    enter(&mut runtime);
    assert_eq!(zone_position(&runtime), SOURCE);
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some("0"));
    runtime
        .execute(WorldCommand::Interact { object_id: GILBERT })
        .unwrap();
    let menu = select(&mut runtime, "@tele");
    assert!(!menu
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
    assert!(runtime
        .world_snapshot()
        .active_npc_dialog
        .as_ref()
        .is_some_and(|dialog| dialog.links.iter().any(|link| link.target == "@BichonWall")));
    assert_eq!(zone_position(&runtime), SOURCE);
    assert_eq!(runtime.world_snapshot().gold, 5_000);

    let packets = select(&mut runtime, "@BichonWall");
    // The classic TAKEGOLD command debits the authoritative balance without a
    // separate LoseGold packet; its value must survive the same save.
    assert_eq!(runtime.world_snapshot().gold, 3_000);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::UserLocation { location } if location.position == DESTINATION)));
    assert_eq!(runtime.world_snapshot().map_file_name.as_deref(), Some("0"));
    assert_eq!(zone_position(&runtime), DESTINATION);
    assert_eq!(runtime.inner.local_player_position(), Some(DESTINATION));

    let logout = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert!(logout
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    enter(&mut runtime);
    assert_eq!(runtime.world_snapshot().gold, 3_000);
    assert_eq!(zone_position(&runtime), DESTINATION);
    assert_eq!(runtime.inner.local_player_position(), Some(DESTINATION));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
}
