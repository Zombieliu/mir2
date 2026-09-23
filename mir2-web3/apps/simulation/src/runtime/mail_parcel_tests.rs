use super::*;
use crate::config::AccountStoreTransactionFault;

struct Fixture {
    config: SimulationConfig,
    session: SimulationSession,
    path: std::path::PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn fixture(items: Vec<ItemState>) -> Fixture {
    let path = std::env::temp_dir().join(format!(
        "mir2-parcel-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = SimulationConfig::default().with_account_store_path(path.clone());
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items = items;
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = 10_000;
    session.save_active_character();
    assert!(path.is_file());
    Fixture {
        config,
        session,
        path,
    }
}

fn item(index: i32, id: u64, slot: u8, quantity: u32) -> ItemState {
    let mut item = embedded_item_state_from_template(
        &crystal_item_by_index(index).unwrap(),
        ItemContainer::Bag1,
        slot,
    );
    item.unique_id = id;
    item.quantity = quantity;
    assert!(exact_mail_item_state_is_valid(&item));
    item
}

fn send(items_idx: [u64; 5], stamped: bool) -> ClientPacket {
    ClientPacket::SendMail {
        name: "Scout".into(),
        message: "parcel".into(),
        gold: 1_500,
        items_idx,
        stamped,
    }
}

#[test]
fn parcel_quote_is_pure_and_charges_source_gold_and_item_insurance() {
    // Stamp itself is a regular attachment when postage stamping is not requested.
    // Source price 1500 => floor(1500 / 100 * 5) = 75; gold1500 adds100.
    let mut f = fixture(vec![item(838, 80_001, 0, 1)]);
    let before = std::fs::read(&f.path).unwrap();
    assert_eq!(
        f.session.handle_packet(ClientPacket::MailCost {
            gold: 1_500,
            items_idx: [80_001, 0, 0, 0, 0],
            stamped: false,
        }),
        vec![ServerPacket::MailCost { cost: 175 }]
    );
    assert_eq!(std::fs::read(&f.path).unwrap(), before);
    let packets = f.session.handle_packet(send([80_001, 0, 0, 0, 0], false));
    assert!(packets.contains(&ServerPacket::LoseGold { gold: 1_675 }));
    assert!(packets.contains(&ServerPacket::MailSent { result: 1 }));
    assert_eq!(f.session.world_snapshot().gold, 8_325);
}

#[test]
fn parcel_consumes_one_owned_stamp_and_allows_five_items_atomically() {
    // The only source stamp template (838) has StackSize=1. Use separate
    // valid stamps rather than relaxing the production carrier validator.
    assert_eq!(crystal_item_by_index(838).unwrap().stack_size, 1);
    let mut items = vec![item(838, 81_000, 0, 1)];
    for offset in 1..=5 {
        items.push(item(838, 81_000 + offset, offset as u8, 1));
    }
    items.push(item(838, 81_006, 6, 1));
    let mut f = fixture(items);
    let ids = [81_001, 81_002, 81_003, 81_004, 81_005];
    let before = std::fs::read(&f.path).unwrap();
    assert_eq!(
        f.session.handle_packet(ClientPacket::MailCost {
            gold: 1_500,
            items_idx: ids,
            stamped: true
        }),
        vec![ServerPacket::MailCost { cost: 0 }]
    );
    assert_eq!(std::fs::read(&f.path).unwrap(), before);
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let before_world = f.session.world_snapshot();
    assert_eq!(
        f.session.handle_packet(send(ids, true)),
        vec![ServerPacket::MailSent { result: -1 }]
    );
    assert_eq!(f.session.world_snapshot(), before_world);
    assert_eq!(std::fs::read(&f.path).unwrap(), before);

    let packets = f.session.handle_packet(send(ids, true));
    assert!(packets.contains(&ServerPacket::MailSent { result: 1 }));
    assert!(packets.contains(&ServerPacket::LoseGold { gold: 1_500 }));
    assert!(packets.contains(&ServerPacket::DeleteItem {
        unique_id: 81_000,
        count: 1
    }));
    let snapshot = f.session.world_snapshot();
    assert_eq!(snapshot.gold, 8_500);
    let live = &f
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items;
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].quantity, 1);
    assert_eq!(item_unique_id(&live[0]), 81_006);
    assert_eq!(
        snapshot
            .stage5_systems
            .mail
            .last()
            .unwrap()
            .item_states_json
            .len(),
        5
    );
    let store = f.config.account_store.lock().unwrap();
    let save = &store.accounts["demo"].saves[&0];
    let persisted: ItemState = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    assert_eq!(persisted.quantity, 1);
    assert_eq!(item_unique_id(&persisted), 81_006);
}

#[test]
fn parcel_rejects_duplicate_unknown_stamp_attachment_and_unstamped_extra_slots() {
    for (ids, stamped) in [
        ([82_001, 82_001, 0, 0, 0], true),
        ([999_999, 0, 0, 0, 0], false),
        ([82_000, 0, 0, 0, 0], true),
        ([82_001, 82_002, 0, 0, 0], false),
    ] {
        let mut f = fixture(vec![
            item(838, 82_000, 0, 1),
            item(838, 82_001, 1, 1),
            item(838, 82_002, 2, 1),
        ]);
        let before_world = f.session.world_snapshot();
        let before_file = std::fs::read(&f.path).unwrap();
        assert_eq!(
            f.session.handle_packet(ClientPacket::MailCost {
                gold: 1_500,
                items_idx: ids,
                stamped
            }),
            vec![ServerPacket::MailCost { cost: u32::MAX }]
        );
        assert_eq!(
            f.session.handle_packet(send(ids, stamped)),
            vec![ServerPacket::MailSent { result: -1 }]
        );
        assert_eq!(f.session.world_snapshot(), before_world);
        assert_eq!(std::fs::read(&f.path).unwrap(), before_file);
    }
}

#[test]
fn parcel_forged_stamp_never_discounts_and_durable_stamp_change_rejects_send() {
    let mut f = fixture(Vec::new());
    assert_eq!(
        f.session.handle_packet(ClientPacket::MailCost {
            gold: 1_500,
            items_idx: [0; 5],
            stamped: true
        }),
        vec![ServerPacket::MailCost { cost: 100 }]
    );
    assert!(f
        .session
        .handle_packet(send([0; 5], true))
        .contains(&ServerPacket::LoseGold { gold: 1_600 }));

    let mut f = fixture(vec![item(838, 83_000, 0, 1)]);
    {
        let mut store = f.config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        // Another committed operation consumed the source's nonstackable stamp.
        save.inventory_items_json.clear();
        save.revision += 1;
    }
    f.config.save_account_store().unwrap();
    let before = std::fs::read(&f.path).unwrap();
    let before_world = f.session.world_snapshot();
    assert_eq!(
        f.session.handle_packet(send([0; 5], true)),
        vec![ServerPacket::MailSent { result: -1 }]
    );
    assert_eq!(std::fs::read(&f.path).unwrap(), before);
    assert_eq!(f.session.world_snapshot(), before_world);
}

#[test]
fn parcel_no_mail_template_is_rejected_live_and_from_durable_inventory() {
    let template = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|template| template.bind & MAIL_BIND_NO_MAIL != 0 && template.bind & 16 == 0)
        .expect("source item with NoMail independently of DontTrade");
    let no_mail = item(template.item_index, 84_000, 0, 1);
    assert!(stage5_trade_item_can_enter(&no_mail));
    let mut f = fixture(vec![no_mail]);
    assert_eq!(
        f.session.handle_packet(ClientPacket::MailCost {
            gold: 0,
            items_idx: [84_000, 0, 0, 0, 0],
            stamped: false
        }),
        vec![ServerPacket::MailCost { cost: u32::MAX }]
    );
    assert_eq!(
        f.session.handle_packet(send([84_000, 0, 0, 0, 0], false)),
        vec![ServerPacket::MailSent { result: -1 }]
    );
    let config = &f.config;
    let mut save = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    let expected = save.inventory_items_json.clone();
    assert!(stage5_take_mail_attachments_from_save(&mut save, &[84_000], &expected).is_err());
}
