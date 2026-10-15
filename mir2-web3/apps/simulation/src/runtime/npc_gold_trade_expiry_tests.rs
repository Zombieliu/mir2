//! Real in-memory BuyItem regression. Current natural Trade scripts have no tags;
//! only the test catalogue is replaced, per World, using real item templates.
use super::*;
use super::super::components::Position;
use super::super::inventory::npc_gold_trade_capacity_evidence;
use super::super::items::{embedded_item_state_from_template, try_item_state_from_user_item,
    try_user_item_from_item_state};
use super::super::npc_gold_trade_expiry::{MAX_TICKS, UTC_KIND};
use crate::{ItemContainer, SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point};

const SCRIPT: &str = "BichonProvince/NaturalCave/WickedTrader";
const NOW: i64 = 638_422_560_123_456_789; // 2024-01-31 UTC + retained fractional time.

fn session() -> (SimulationSession, u64) {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 4990, name: "Wicked Trader".into(), image: 5, colour_argb: -1,
        position: Point { x: 331, y: 271 }, direction: MirDirection::Left,
        quest_ids: vec![], script_key: Some(SCRIPT.into()),
    });
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login { account_id:"demo".into(), password:"demo".into() });
    assert!(login.iter().any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.interact(4990);
    let packets = session.select_npc_dialog_target("@BuySell");
    let natural_uid = packets.iter().find_map(|p| match p {
        ServerPacket::NPCGoods { list, .. } => list.iter()
            .find(|i| i.item_index == 658 && i.count == 1).map(|i| i.unique_id), _ => None,
    }).expect("real Wicked Trader service and catalogue");
    assert_eq!(session.app.world().resource::<NpcStateResource>().active_npc_service
        .as_ref().unwrap().script_key, SCRIPT);
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_capacity = 86;
        inventory.inventory_items.clear(); inventory.belt_items.clear();
        inventory.equipment_items.clear(); inventory.storage_items.clear();
        inventory.reserved_item_unique_ids.clear();
    }
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 100_000;
    (session, natural_uid)
}

fn install_catalog(session: &mut SimulationSession, index: i32, ticks: i64) -> UserItem {
    let template = crystal_item_by_index(index).unwrap();
    let item = crystal_npc_trade_good_from_line(&template.name, 0).unwrap();
    assert_eq!(item.item_index, index);
    assert!(item.is_shop_item);
    assert!(item.expire_info.is_none(), "catalogue selectors do not start the expiry clock");
    session.app.world_mut().insert_resource(NpcGoldTradeExpiryFixture {
        script_key: SCRIPT.into(), trade_goods: vec![item.clone()], now_utc_ticks: ticks,
    });
    item
}
fn buy(session: &mut SimulationSession, uid: u64, count: u16) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::BuyItem { item_index: uid, count, panel_type: 0 })
}
fn snapshot(session: &SimulationSession) -> serde_json::Value {
    let inventory = session.app.world().resource::<InventoryResource>();
    serde_json::json!({"gold":session.app.world().resource::<PlayerRuntimeResource>().gold,
        "capacity":inventory.inventory_capacity, "bag":inventory.inventory_items,
        "belt":inventory.belt_items,"equipment":inventory.equipment_items,
        "storage":inventory.storage_items,"reserved":inventory.reserved_item_unique_ids})
}
fn delivery(packets: &[ServerPacket], count: u16, cost: u32) -> UserItem {
    let economic: Vec<_> = packets.iter().filter(|p| matches!(p,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })).collect();
    assert_eq!(economic.len(), 2, "one real debit followed by one incoming delta");
    assert!(matches!(economic[0], ServerPacket::LoseGold { gold } if *gold == cost));
    let ServerPacket::GainedItem { item } = economic[1] else { panic!("GainedItem"); };
    assert_eq!(item.count, count);
    item.clone()
}
fn no_economic_change(session: &mut SimulationSession, uid: u64) {
    let before = snapshot(session);
    let packets = buy(session, uid, 1);
    assert!(packets.iter().all(|p| !matches!(p,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })));
    assert_eq!(snapshot(session), before, "full wallet/carrier/reservation rollback");
}

// Independent complete carrier; neither the fresh factory nor the gain planner
// is an expected-value oracle. These fixtures only use non-socket templates.
fn fresh_wire(index: i32, uid: u64, count: u16) -> UserItem {
    let info = crystal_item_by_index(index).unwrap();
    UserItem { unique_id:uid,item_index:index,current_dura:info.durability,max_dura:info.durability,
        count,soul_bound_id:-1,identified:false,cursed:false,slots:vec![None;usize::from(info.slots)],
        gem_count:0,added_stats:vec![],awake_type:0,awake_values:vec![],refined_value:0,
        refine_added:0,refine_success_chance:0,wedding_ring:-1,expire_info:None,
        rental_information:None,is_shop_item:false,sealed_info:None,gm_made:false }
}
fn carrier(index: i32, container: ItemContainer, slot: u8, wire: &UserItem) -> ItemState {
    let info = crystal_item_by_index(index).unwrap();
    try_item_state_from_user_item(embedded_item_state_from_template(&info, container, slot), wire).unwrap()
}
fn fill_carried(session: &mut SimulationSession) {
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items = (0u8..80).map(|cell| {
        let (container,slot) = if cell < 40 { (ItemContainer::Bag1,cell) }
            else { (ItemContainer::Bag2,cell-40) };
        carrier(595,container,slot,&fresh_wire(595,10_000+u64::from(cell),1))
    }).collect();
    inventory.belt_items = (0u8..6).map(|slot| {
        let index = if slot < 4 {658} else {712};
        carrier(index,ItemContainer::Belt,slot,&fresh_wire(index,20_000+u64::from(slot),1))
    }).collect();
}

#[test]
fn npc_gold_trade_expiry_session_timed_and_maximum_delivery_match_live_sidecar() {
    for (index, expected, cost) in [
        (1295, (NOW+108_000_000_000)|UTC_KIND, 0),
        (794, (638_579_808_000_000_000+123_456_789)|UTC_KIND, 50_000),
        (1291, MAX_TICKS, 200),
    ] {
        let (mut session, _) = session();
        let catalog = install_catalog(&mut session,index,NOW);
        let incoming = delivery(&buy(&mut session,catalog.unique_id,1),1,cost);
        assert_ne!(incoming.unique_id,0); assert_ne!(incoming.unique_id,catalog.unique_id);
        assert_eq!(incoming.item_index,index);
        assert_eq!(incoming.expire_info.as_ref().unwrap().expiry_binary_datetime,expected);
        assert!(!incoming.identified); assert!(!incoming.is_shop_item);
        assert!(incoming.added_stats.is_empty());
        assert_eq!(incoming.current_dura,catalog.current_dura);
        assert_eq!(incoming.max_dura,catalog.max_dura);
        assert!(incoming.slots.iter().all(Option::is_none));
        let inventory = session.app.world().resource::<InventoryResource>();
        let live = inventory.inventory_items.iter().chain(inventory.belt_items.iter())
            .find(|i| i.unique_id==incoming.unique_id).unwrap();
        assert_eq!(try_user_item_from_item_state(live).unwrap(),incoming);
        assert_eq!(live.user_item_metadata.as_ref().unwrap().expire_info,incoming.expire_info);
        assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,100_000-cost);
        assert!(session.app.world().resource::<NpcGoldTradeExpiryFixture>().trade_goods[0].expire_info.is_none());
    }
}

#[test]
fn npc_gold_trade_expiry_session_range_money_and_full_carried_capacity_roll_back() {
    for failure in 0..3 {
        let (mut session, _) = session();
        let catalog = install_catalog(&mut session,794,NOW);
        match failure {
            0 => {
                let player = player_entity(session.app.world()).unwrap();
                session.app.world_mut().entity_mut(player).insert(Position(Point{x:0,y:0}));
            }
            1 => session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=49_999,
            _ => fill_carried(&mut session),
        }
        no_economic_change(&mut session,catalog.unique_id);
    }
}

#[test]
fn npc_gold_trade_expiry_session_datetime_overflow_is_atomic_not_a_clamped_delivery() {
    let (mut session, _) = session();
    let catalog = install_catalog(&mut session,794,MAX_TICKS);
    no_economic_change(&mut session,catalog.unique_id);
    // Restoring a valid instant proves the same live service/catalogue was usable.
    session.app.world_mut().resource_mut::<NpcGoldTradeExpiryFixture>().now_utc_ticks=NOW;
    let incoming = delivery(&buy(&mut session,catalog.unique_id,1),1,50_000);
    assert_eq!(incoming.expire_info.unwrap().expiry_binary_datetime,
        (638_579_808_000_000_000+123_456_789)|UTC_KIND);
}

#[test]
fn npc_gold_trade_expiry_capacity_tags_are_occupied_only_and_never_clock_oracles() {
    let (mut session, _) = session();
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_items = vec![
            carrier(658,ItemContainer::Bag1,0,&fresh_wire(658,30_000,18)),
            carrier(1295,ItemContainer::Bag1,1,&fresh_wire(1295,30_001,1)),
            carrier(1291,ItemContainer::Bag1,2,&fresh_wire(1291,30_002,1)),
        ];
    }
    let inventory = session.app.world().resource::<InventoryResource>();
    let before = serde_json::to_vec(&inventory.inventory_items).unwrap();
    let proof = npc_gold_trade_capacity_evidence(inventory);
    assert!(proof.roster_valid);
    assert_eq!(proof.fresh_compatible_unique_ids,vec![30_000]);
    assert_eq!(serde_json::to_vec(&inventory.inventory_items).unwrap(),before);
}

#[test]
fn npc_gold_trade_expiry_session_untagged_natural_trade_keeps_original_delivery() {
    let (mut session, uid) = session();
    assert!(session.app.world().get_resource::<NpcGoldTradeExpiryFixture>().is_none());
    let incoming = delivery(&buy(&mut session,uid,2),2,160);
    assert_eq!(incoming.item_index,658); assert!(incoming.expire_info.is_none());
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,99_840);
    let inventory = session.app.world().resource::<InventoryResource>();
    let live = inventory.inventory_items.iter().chain(inventory.belt_items.iter())
            .find(|i| i.unique_id==incoming.unique_id).unwrap();
    assert_eq!(try_user_item_from_item_state(live).unwrap(),incoming);
}

#[test]
fn npc_gold_trade_expiry_test_catalogue_is_world_local_and_exact_service_bound() {
    let (mut timed, _) = session();
    let catalog = install_catalog(&mut timed,1295,NOW);
    timed.app.world_mut().resource_mut::<NpcGoldTradeExpiryFixture>().script_key="other/service".into();
    no_economic_change(&mut timed,catalog.unique_id);
    timed.app.world_mut().resource_mut::<NpcGoldTradeExpiryFixture>().script_key=SCRIPT.into();
    let incoming=delivery(&buy(&mut timed,catalog.unique_id,1),1,0);
    assert_eq!(incoming.expire_info.unwrap().expiry_binary_datetime,(NOW+108_000_000_000)|UTC_KIND);
    let (mut natural,uid)=session();
    assert!(natural.app.world().get_resource::<NpcGoldTradeExpiryFixture>().is_none());
    let incoming=delivery(&buy(&mut natural,uid,2),2,160);
    assert!(incoming.expire_info.is_none());
    timed.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service=None;
    no_economic_change(&mut timed,catalog.unique_id);
}
