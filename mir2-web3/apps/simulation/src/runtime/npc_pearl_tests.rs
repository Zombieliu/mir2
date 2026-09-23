use super::super::resources::Stage5SystemsResource;
use super::*;
use crate::{SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point};

fn session() -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 4990,
        name: "Wicked Trader".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 271 },
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}

#[test]
fn pearl_npc_purchase_uses_only_pearl_wallet_and_rejects_insufficient_funds() {
    let mut s = session();
    s.interact(4990);
    let goods = s.select_npc_dialog_target("@BuySell");
    let uid = goods
        .iter()
        .find_map(|p| match p {
            ServerPacket::NPCGoods { list, .. } => list
                .iter()
                .find(|i| i.item_index == 658)
                .map(|i| i.unique_id),
            _ => None,
        })
        .unwrap();
    // The fixture offers a real source trade catalog through the PearlBuy service.
    s.app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service
        .as_mut()
        .unwrap()
        .label_key = "PEARLBUY".into();
    let gold = s.app.world().resource::<PlayerRuntimeResource>().gold;
    let before = s
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .clone();
    let buy = ClientPacket::BuyItem {
        item_index: uid,
        count: 2,
        panel_type: 0,
    };
    assert!(s.handle_packet(buy.clone()).is_empty());
    assert_eq!(
        serde_json::to_value(
            &s.app
                .world()
                .resource::<InventoryResource>()
                .inventory_items
        )
        .unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    s.app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creature_pearls = 240;
    let packets = s.handle_packet(buy);
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::GainedItem { item } if item.item_index == 658 && item.count == 2)));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { .. })));
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    assert_eq!(
        s.app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creature_pearls,
        80
    );
    s.handle_packet(ClientPacket::LogOut);
    s.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls, 80);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
}

#[test]
fn pearl_npc_packet_and_script_wallet_bounds_match_source() {
    let mut s = session();
    let script = crystal_npc_script_by_key("BichonProvince/NaturalCave/WickedTrader").unwrap();
    let packets = crystal_npc_service_packets_for_label(Some(&script), "[@PEARLBUY]").unwrap();
    assert!(
        matches!(&packets[0], ServerPacket::NPCPearlGoods { list, panel_type: 0, .. } if !list.is_empty())
    );
    let mut state = super::super::npc_script::CrystalNpcExecutionState::default();
    for (line, expected) in [
        ("GIVEPEARLS 4294967295", i32::MAX),
        ("GIVEPEARLS -1", i32::MAX),
        ("TAKEPEARLS 4294967295", 0),
        ("GIVEPEARLS 17", 17),
        ("TAKEPEARLS 3", 14),
    ] {
        super::super::npc_script::execute_crystal_npc_action_line(
            s.app.world_mut(),
            line,
            &mut vec![],
            &mut state,
        );
        assert_eq!(
            s.app
                .world()
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .intelligent_creature_pearls,
            expected
        );
    }
}
