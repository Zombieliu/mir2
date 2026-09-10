//! Source count-band icons must follow the authoritative instance, including
//! after real protocol mutations and save/reload; ItemInfo.Image stays raw.

use mir2_game_data::crystal_item_by_index;
use mir2_protocol::{ClientPacket, MirClass, MirGender, MirGridType, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};
use serde_json::{json, Value};

const ACCOUNT: &str = "item-stack-image-parity";
const UID: u64 = 71_001;

fn stack(index: i32, count: u32, container: &str, slot: u8, uid: u64) -> Value {
    let info = crystal_item_by_index(index).expect("Crystal catalogue row");
    json!({
        "key": format!("crystal-item-{index}"), "name": info.name,
        "icon": info.image, "slot": slot, "unique_id": uid, "container": container,
        "quantity": count, "description": "Source quantity-image fixture",
        "durability_current": null, "durability_max": null, "weight": info.weight,
        "equip_slot": "amulet", "grade": "common", "attack": 0, "defence": 0,
        "heal_hp": 0, "heal_mp": 0, "user_item_metadata": {"item_index": index}
    })
}

fn start(
    inventory: Vec<Value>,
    belt: Vec<Value>,
    storage: Vec<Value>,
    equipment: Vec<Value>,
) -> SimulationSession {
    let character = CharacterRecord {
        index: 0,
        name: "StackParity".to_owned(),
        level: 50,
        class: MirClass::Taoist,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.inventory_items_json = inventory.into_iter().map(|item| item.to_string()).collect();
    save.belt_items_json = belt.into_iter().map(|item| item.to_string()).collect();
    save.storage_items_json = storage.into_iter().map(|item| item.to_string()).collect();
    save.equipment_items_json = equipment.into_iter().map(|item| item.to_string()).collect();
    save.equipment_items_explicit_empty = true;
    let config = SimulationConfig::default();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert(ACCOUNT.to_owned(), account);
    let mut session = SimulationSession::new(config);
    login(&mut session);
    session
}

fn login(session: &mut SimulationSession) {
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

#[test]
fn stack_images_follow_every_threshold_in_bag_belt_storage_and_equipment() {
    let cases = [
        (710, 49, 3673),
        (710, 50, 3674),
        (710, 99, 3674),
        (710, 100, 2960),
        (710, 149, 2960),
        (710, 150, 3675),
        (711, 49, 3670),
        (711, 50, 3671),
        (711, 99, 3671),
        (711, 100, 2961),
        (711, 149, 2961),
        (711, 150, 3672),
        (712, 1, 3660),
        (712, 199, 3660),
        (712, 200, 3661),
        (712, 299, 3661),
        (712, 300, 3662),
        (712, 500, 3662),
        (714, 5, 277), // AmuletOfRevival is Shape 3: no count-band substitution.
    ];
    for (index, count, image) in cases {
        let info = crystal_item_by_index(index).unwrap();
        let equipment = json!({
            "key": format!("crystal-item-{index}"), "name": info.name,
            "icon": info.image, "slot": "amulet", "quantity": count,
            "description": "Source stack fixture", "durability_current": 0,
            "durability_max": 0, "attack": 0, "defence": 0,
            "user_item_unique_id": UID + 3, "user_item_metadata": {"item_index": index}
        });
        let session = start(
            vec![stack(index, count, "bag1", 0, UID)],
            vec![stack(index, count, "belt", 0, UID + 1)],
            vec![stack(index, count, "storage", 0, UID + 2)],
            vec![equipment],
        );
        let snapshot = session.world_snapshot();
        for (items, uid) in [
            (&snapshot.inventory_items, UID),
            (&snapshot.belt_items, UID + 1),
            (&snapshot.storage_items, UID + 2),
        ] {
            let item = items
                .iter()
                .find(|item| item.unique_id == uid)
                .expect("exact saved stack");
            assert_eq!(
                (item.quantity, item.icon),
                (count, image),
                "{index} uid={uid}"
            );
            let source = item.tooltip_source.as_ref().unwrap();
            assert_eq!(
                source.info.image, info.image,
                "never rewrite ItemInfo.Image"
            );
            assert_eq!(source.user_item.as_ref().unwrap().count, count as u16);
        }
        let equipped = snapshot
            .equipment_items
            .iter()
            .find(|item| item.unique_id == Some(UID + 3))
            .unwrap();
        assert_eq!((equipped.quantity, equipped.icon), (count, image));
        assert_eq!(
            equipped.state_image, info.image,
            "StateItem image is a different library path"
        );
    }
}

#[test]
fn real_split_merge_equip_remove_and_reload_refresh_count_images_without_caching() {
    let mut session = start(
        vec![stack(712, 300, "bag1", 0, UID)],
        vec![],
        vec![],
        vec![],
    );
    let split = session.handle_packet(ClientPacket::SplitItem {
        grid: MirGridType::Inventory,
        unique_id: UID,
        count: 101,
    });
    assert!(
        split
            .iter()
            .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })),
        "{split:?}"
    );
    let snapshot = session.world_snapshot();
    let source = snapshot
        .inventory_items
        .iter()
        .find(|item| item.unique_id == UID)
        .unwrap();
    assert_eq!((source.quantity, source.icon), (199, 3660));
    let child = snapshot
        .belt_items
        .iter()
        .find(|item| item.unique_id != UID && item.key == source.key)
        .expect("Crystal add-item policy places the split Amulet in the free belt");
    assert_eq!((child.quantity, child.icon), (101, 3660));
    let merge = session.handle_packet(ClientPacket::MergeItem {
        grid_from: MirGridType::Belt,
        grid_to: MirGridType::Inventory,
        id_from: child.unique_id,
        id_to: UID,
    });
    assert!(
        merge
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MergeItem { success: true, .. })),
        "{merge:?}"
    );
    let snapshot = session.world_snapshot();
    assert_eq!(
        snapshot
            .inventory_items
            .iter()
            .find(|item| item.unique_id == UID)
            .map(|item| (item.quantity, item.icon)),
        Some((300, 3662))
    );

    let equip = session.handle_packet(ClientPacket::EquipItem {
        grid: MirGridType::Inventory,
        unique_id: UID,
        to: 9,
    });
    assert!(
        equip
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "{equip:?}"
    );
    assert_eq!(
        session
            .world_snapshot()
            .equipment_items
            .iter()
            .find(|item| item.unique_id == Some(UID))
            .map(|item| (item.quantity, item.icon)),
        Some((300, 3662))
    );
    session.save_active_character().unwrap();
    session.handle_packet(ClientPacket::LogOut);
    login(&mut session);
    assert_eq!(
        session
            .world_snapshot()
            .equipment_items
            .iter()
            .find(|item| item.unique_id == Some(UID))
            .map(|item| (item.quantity, item.icon)),
        Some((300, 3662))
    );
    let remove = session.handle_packet(ClientPacket::RemoveItem {
        grid: MirGridType::Inventory,
        unique_id: UID,
        to: 0,
    });
    assert!(
        remove
            .iter()
            .any(|packet| matches!(packet, ServerPacket::RemoveItem { success: true, .. })),
        "{remove:?}"
    );
    assert_eq!(
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .find(|item| item.unique_id == UID)
            .map(|item| (item.quantity, item.icon)),
        Some((300, 3662))
    );
}

#[test]
fn both_poisons_refresh_size_band_after_real_split_and_cross_belt_merge() {
    for (index, small, middle, large) in [(710, 3673, 2960, 3675), (711, 3670, 2961, 3672)] {
        let mut session = start(
            vec![stack(index, 150, "bag1", 0, UID)],
            vec![],
            vec![],
            vec![],
        );
        let split = session.handle_packet(ClientPacket::SplitItem {
            grid: MirGridType::Inventory,
            unique_id: UID,
            count: 101,
        });
        assert!(
            split
                .iter()
                .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })),
            "{split:?}"
        );
        let snapshot = session.world_snapshot();
        let remaining = snapshot
            .inventory_items
            .iter()
            .find(|item| item.unique_id == UID)
            .unwrap();
        assert_eq!((remaining.quantity, remaining.icon), (49, small));
        let child = snapshot
            .belt_items
            .iter()
            .find(|item| item.key == remaining.key)
            .unwrap();
        assert_eq!((child.quantity, child.icon), (101, middle));
        let merged = session.handle_packet(ClientPacket::MergeItem {
            grid_from: MirGridType::Belt,
            grid_to: MirGridType::Inventory,
            id_from: child.unique_id,
            id_to: UID,
        });
        assert!(
            merged
                .iter()
                .any(|packet| matches!(packet, ServerPacket::MergeItem { success: true, .. })),
            "{merged:?}"
        );
        assert_eq!(
            session
                .world_snapshot()
                .inventory_items
                .iter()
                .find(|item| item.unique_id == UID)
                .map(|item| (item.quantity, item.icon)),
            Some((150, large))
        );
    }
}

#[test]
fn known_source_identity_overrides_a_stale_legacy_icon_without_rewriting_item_info() {
    let mut legacy = stack(658, 1, "bag1", 0, UID);
    legacy["icon"] = json!(24);
    let session = start(vec![legacy], vec![], vec![], vec![]);
    let snapshot = session.world_snapshot();
    let item = snapshot
        .inventory_items
        .iter()
        .find(|item| item.unique_id == UID)
        .unwrap();
    assert_eq!(item.icon, 398);
    assert_eq!(item.tooltip_source.as_ref().unwrap().info.image, 398);
}
