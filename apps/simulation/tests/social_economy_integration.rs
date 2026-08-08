use mir2_protocol::{ClientPacket, MirDirection};
use mir2_simulation::{
    CharacterSaveRecord, ItemContainer, SimulationConfig, SimulationSession, Stage5MailMessage,
    Stage5SystemsState,
};

fn item_state_json(
    key: &str,
    name: &str,
    container: ItemContainer,
    slot: u8,
    unique_id: u64,
) -> String {
    serde_json::to_string(&serde_json::json!({
        "key": key,
        "name": name,
        "icon": 1,
        "slot": slot,
        "unique_id": unique_id,
        "container": container,
        "quantity": 1,
        "description": format!("{name} test item."),
        "durability_current": 10,
        "durability_max": 10,
        "weight": 1,
        "equip_slot": serde_json::Value::Null,
        "grade": "none",
        "added_attack": 0,
        "added_defence": 0,
        "added_stats": [],
        "cursed": false,
        "socket_slots": 0,
        "gem_count": 0,
        "identified": serde_json::Value::Null,
        "soul_bound_id": serde_json::Value::Null,
        "sealed_expiry_time_binary_datetime": 0,
        "sealed_next_time_binary_datetime": 0,
        "rental_binding_flags": 0,
        "rental_owner_name": "",
        "rental_expiry_binary_datetime": 0,
        "rental_locked": false,
        "attack": 0,
        "defence": 0,
        "heal_hp": 0,
        "heal_mp": 0,
    }))
    .expect("item state json should encode")
}

fn bag_slot(index: u8) -> (ItemContainer, u8) {
    if index < 40 {
        (ItemContainer::Bag1, index)
    } else {
        (ItemContainer::Bag2, index - 40)
    }
}

fn filler_inventory(slot_count: u8) -> Vec<String> {
    (0..slot_count)
        .map(|index| {
            let (container, slot) = bag_slot(index);
            item_state_json(
                &format!("filler-{index}"),
                &format!("Filler {index}"),
                container,
                slot,
                10_000 + u64::from(index),
            )
        })
        .collect()
}

fn config_with_mail_parcel(filled_slots: u8) -> SimulationConfig {
    let config = SimulationConfig::default();
    let mut save = CharacterSaveRecord::new(config.default_character.clone());
    save.direction = MirDirection::Down;
    save.gold = 10;
    save.inventory_items_json = filler_inventory(filled_slots);

    let mut systems = Stage5SystemsState::default();
    systems.mail.push(Stage5MailMessage {
        id: 1,
        from: "Bichon Administrator".to_string(),
        to: "Scout".to_string(),
        subject: "Collected parcel".to_string(),
        body: "Two exact items.".to_string(),
        gold: 77,
        items: vec!["parcel-a".to_string(), "parcel-b".to_string()],
        item_states_json: vec![
            item_state_json("parcel-a", "Parcel A", ItemContainer::Bag1, 0, 77_001),
            item_state_json("parcel-b", "Parcel B", ItemContainer::Bag1, 1, 77_002),
        ],
        opened: true,
        locked: false,
        claimed: false,
        deleted: false,
    });
    save.stage5_systems_json =
        Some(serde_json::to_string(&systems).expect("stage5 systems should encode"));

    {
        let mut store = config.account_store.lock().expect("account store lock");
        let account = store
            .accounts
            .get_mut("demo")
            .expect("default demo account");
        account.saves.insert(0, save);
    }

    config
}

#[test]
fn social_friend_and_blacklist_keep_crystal_single_entry_semantics() {
    let config = SimulationConfig::default();
    let player_name = config.default_character.name.clone();
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    session.stage5_command("social.friend", vec!["Jina".to_string()]);
    session.stage5_command("social.block", vec!["jina".to_string()]);
    let social = session.world_snapshot().stage5_systems.social;
    assert_eq!(social.friends, vec!["Jina".to_string()]);
    assert!(social.blocked.is_empty());

    session.stage5_command("social.unfriend", vec!["JINA".to_string()]);
    session.stage5_command("social.block", vec!["Jina".to_string()]);
    session.stage5_command("social.friend", vec!["jina".to_string()]);
    session.stage5_command("social.friend", vec![player_name.clone()]);
    session.stage5_command("social.block", vec![player_name]);
    let social = session.world_snapshot().stage5_systems.social;
    assert!(social.friends.is_empty());
    assert_eq!(social.blocked, vec!["Jina".to_string()]);

    session.save_active_character();
    let mut reloaded = SimulationSession::new(config);
    reloaded.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let social = reloaded.world_snapshot().stage5_systems.social;
    assert!(social.friends.is_empty());
    assert_eq!(social.blocked, vec!["Jina".to_string()]);
}

#[test]
fn mail_claim_exact_parcel_rolls_back_when_all_attachments_do_not_fit() {
    let mut session = SimulationSession::new(config_with_mail_parcel(79));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    let before = session.world_snapshot();
    assert_eq!(before.free_bag_slots, 1);

    session.stage5_command("mail.claim", vec!["1".to_string()]);
    let after = session.world_snapshot();

    assert_eq!(after.gold, 10);
    assert_eq!(after.inventory_items.len(), before.inventory_items.len());
    assert!(!after
        .inventory_items
        .iter()
        .any(|item| item.key == "parcel-a" || item.key == "parcel-b"));
    let mail = after
        .stage5_systems
        .mail
        .iter()
        .find(|mail| mail.id == 1)
        .expect("mail should remain after failed claim");
    assert!(!mail.claimed);
    assert_eq!(mail.gold, 77);
    assert_eq!(
        mail.items,
        vec!["parcel-a".to_string(), "parcel-b".to_string()]
    );
    assert_eq!(mail.item_states_json.len(), 2);
}

#[test]
fn mail_claim_exact_parcel_consumes_payload_and_persists_collected_state() {
    let config = config_with_mail_parcel(78);
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    session.stage5_command("mail.claim", vec!["1".to_string()]);
    let after = session.world_snapshot();

    assert_eq!(after.gold, 87);
    assert!(after
        .inventory_items
        .iter()
        .any(|item| item.key == "parcel-a"));
    assert!(after
        .inventory_items
        .iter()
        .any(|item| item.key == "parcel-b"));
    let mail = after
        .stage5_systems
        .mail
        .iter()
        .find(|mail| mail.id == 1)
        .expect("mail should stay as collected history");
    assert!(mail.claimed);
    assert_eq!(mail.gold, 0);
    assert!(mail.items.is_empty());
    assert!(mail.item_states_json.is_empty());

    session.save_active_character();
    let mut reloaded = SimulationSession::new(config);
    reloaded.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let reloaded_mail = reloaded
        .world_snapshot()
        .stage5_systems
        .mail
        .into_iter()
        .find(|mail| mail.id == 1)
        .expect("collected mail should persist");
    assert!(reloaded_mail.claimed);
    assert_eq!(reloaded_mail.gold, 0);
    assert!(reloaded_mail.items.is_empty());
    assert!(reloaded_mail.item_states_json.is_empty());
}
