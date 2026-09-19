use mir2_game_data::crystal_item_by_name;
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};
use serde_json::json;

const ACCOUNT: &str = "crystal-scroll-teleport";
const ITEM_UID: u64 = 91_001;

fn inventory_scroll(name: &str) -> String {
    let info = crystal_item_by_name(name).expect("Crystal scroll catalogue row");
    json!({
        "key": format!("crystal-item-{}", info.item_index),
        "name": info.name,
        "icon": info.image,
        "slot": 0,
        "unique_id": ITEM_UID,
        "container": "bag1",
        "quantity": 1,
        "description": "Crystal scroll teleport parity fixture",
        "durability_current": null,
        "durability_max": null,
        "weight": info.weight,
        "equip_slot": "amulet",
        "grade": "common",
        "attack": 0,
        "defence": 0,
        "heal_hp": 0,
        "heal_mp": 0,
        "user_item_metadata": {"item_index": info.item_index}
    })
    .to_string()
}

fn field_session(scroll: &str) -> (SimulationConfig, SimulationSession) {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let character = CharacterRecord {
        index: 0,
        name: "ScrollParity".to_owned(),
        level: 20,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = "2".to_owned();
    save.map_title = "SerpentValley".to_owned();
    save.position = Point { x: 406, y: 453 };
    save.direction = MirDirection::Left;
    save.inventory_items_json = vec![inventory_scroll(scroll)];
    save.belt_items_json.clear();

    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .expect("account store")
        .accounts
        .insert(ACCOUNT.to_owned(), account);

    let mut session = SimulationSession::new(config.clone());
    login_and_start(&mut session);
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("2"));
    (config, session)
}

fn login_and_start(session: &mut SimulationSession) {
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: ACCOUNT.to_owned(),
            password: "demo".to_owned(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
}

fn player_transform(session: &SimulationSession) -> (Point, MirDirection) {
    let snapshot = session.world_snapshot();
    let player = snapshot
        .entities
        .iter()
        .find(|entity| Some(entity.object_id) == snapshot.player_object_id)
        .expect("self player");
    (
        Point {
            x: player.x,
            y: player.y,
        },
        player.direction,
    )
}

#[test]
fn dungeon_escape_uses_bind_map_and_bind_location_radius_then_persists() {
    let (config, mut session) = field_session("DungeonEscape");
    let bind_map = config.map.file_name.clone();
    let bind = config.spawn.clone();

    let packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });
    let snapshot = session.world_snapshot();
    let (landed, direction) = player_transform(&session);

    assert_eq!(snapshot.map_file_name.as_deref(), Some(bind_map.as_str()));
    assert!((landed.x - bind.x).abs() <= 100 && (landed.y - bind.y).abs() <= 100);
    assert_eq!(direction, MirDirection::Left);
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == bind_map
    )));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UseItem {
            unique_id: ITEM_UID,
            success: true,
            ..
        }
    )));
    assert!(!snapshot
        .inventory_items
        .iter()
        .any(|item| item.unique_id == ITEM_UID));

    session
        .save_active_character()
        .expect("save escaped transform");
    drop(session);
    let mut reloaded = SimulationSession::new(config);
    login_and_start(&mut reloaded);
    assert_eq!(
        reloaded.world_snapshot().map_file_name.as_deref(),
        Some("0")
    );
    assert_eq!(player_transform(&reloaded), (landed, MirDirection::Left));
}

#[test]
fn town_teleport_uses_exact_bind_transform_and_preserves_direction() {
    let (config, mut session) = field_session("TownTeleport");
    let bind_map = config.map.file_name.clone();
    let bind = config.spawn.clone();

    let packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });

    assert_eq!(
        session.world_snapshot().map_file_name.as_deref(),
        Some(bind_map.as_str())
    );
    assert_eq!(
        player_transform(&session),
        (bind.clone(), MirDirection::Left)
    );
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info } if info.file_name == bind_map
    )));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == bind && location.direction == MirDirection::Left
    )));
}
