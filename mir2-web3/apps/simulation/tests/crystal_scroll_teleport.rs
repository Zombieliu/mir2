use mir2_game_data::crystal_item_by_name;
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket,
};
use mir2_simulation::{
    AccountRecord, CharacterBindPoint, CharacterRecord, CharacterSaveRecord, SimulationConfig,
    SimulationSession,
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
    session_on_map(scroll, "2", Point { x: 406, y: 453 }, None)
}

fn session_on_map(
    scroll: &str,
    map_file_name: &str,
    position: Point,
    bind_point: Option<CharacterBindPoint>,
) -> (SimulationConfig, SimulationSession) {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let character = CharacterRecord {
        index: 0,
        name: "ScrollParity".to_owned(),
        level: 20,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = map_file_name.to_owned();
    save.map_title = if map_file_name == "0" {
        "BichonProvince"
    } else {
        "SerpentValley"
    }
    .to_owned();
    save.position = position;
    save.direction = MirDirection::Left;
    save.bind_point = bind_point;
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
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some(map_file_name));
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

#[test]
fn visiting_bichon_city_safe_zone_rebinds_scroll_and_persists_after_logout() {
    let (config, mut session) = session_on_map(
        "TownTeleport",
        "0",
        Point { x: 205, y: 62 },
        None,
    );
    let city_safe_tile = Point { x: 330, y: 265 };
    let city_bind = Point { x: 328, y: 264 };

    // Accepted shared-Zone movement is projected into the personal save.
    session.force_authoritative_player_transform(city_safe_tile, MirDirection::Right);
    session.force_authoritative_player_transform(Point { x: 205, y: 62 }, MirDirection::Left);
    session.save_active_character().expect("persist new safe-zone bind");
    drop(session);

    let saved = config
        .account_store
        .lock()
        .expect("account store")
        .accounts[ACCOUNT]
        .saves[&0]
        .clone();
    assert_eq!(saved.bind_point.as_ref().map(|bind| bind.position.clone()), Some(city_bind.clone()));
    assert_eq!(saved.bind_point.as_ref().map(|bind| bind.map_file_name.as_str()), Some("0"));

    let mut reloaded = SimulationSession::new(config);
    login_and_start(&mut reloaded);
    let packets = reloaded.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });
    assert_eq!(player_transform(&reloaded), (city_bind.clone(), MirDirection::Left));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == city_bind && location.direction == MirDirection::Left
    )));
}

#[test]
fn town_scroll_uses_persisted_city_bind_from_another_map() {
    let city_bind = Point { x: 328, y: 264 };
    let (_config, mut session) = session_on_map(
        "TownTeleport",
        "2",
        Point { x: 406, y: 453 },
        Some(CharacterBindPoint {
            map_file_name: "0".to_owned(),
            position: city_bind.clone(),
        }),
    );
    let packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
    assert_eq!(player_transform(&session), (city_bind, MirDirection::Left));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MapInformation { info }
            if info.file_name == "0" && info.mini_map == 101 && info.map_index == 1
    )));
}

#[test]
fn dungeon_escape_samples_around_persisted_city_bind() {
    let city_bind = Point { x: 328, y: 264 };
    let (_config, mut session) = session_on_map(
        "DungeonEscape",
        "2",
        Point { x: 406, y: 453 },
        Some(CharacterBindPoint {
            map_file_name: "0".to_owned(),
            position: city_bind.clone(),
        }),
    );
    let packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });
    let (landed, _) = player_transform(&session);
    assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
    assert!((landed.x - city_bind.x).abs() <= 100);
    assert!((landed.y - city_bind.y).abs() <= 100);
    assert!(packets.iter().any(|packet| matches!(
        packet, ServerPacket::UseItem { unique_id: ITEM_UID, success: true, .. }
    )));
}

#[test]
fn legacy_save_inside_city_safe_zone_recovers_its_bind_on_start() {
    let (_config, mut session) = session_on_map(
        "TownTeleport",
        "0",
        Point { x: 330, y: 265 },
        None,
    );
    session.force_authoritative_player_transform(Point { x: 205, y: 62 }, MirDirection::Left);
    session.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });
    assert_eq!(player_transform(&session).0, (Point { x: 328, y: 264 }));
}

#[test]
fn most_recent_safe_zone_overrides_an_earlier_city_bind() {
    let (_config, mut session) = session_on_map(
        "TownTeleport",
        "0",
        Point { x: 330, y: 265 },
        Some(CharacterBindPoint {
            map_file_name: "0".to_owned(),
            position: Point { x: 328, y: 264 },
        }),
    );
    // The authoritative Zone has accepted a later visit to the starting
    // village. Crystal binds to this latest safe area, even if a larger city
    // was previously visited.
    session.force_authoritative_player_transform(Point { x: 283, y: 608 }, MirDirection::Left);
    let packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: ITEM_UID,
        grid: MirGridType::Inventory,
    });
    let village = Point { x: 288, y: 616 };
    assert_eq!(player_transform(&session).0, village);
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::UserLocation { location } if location.position == village
    )));
}
