//! Read-only same-turn owner fields used by both actual clients.
use bevy_ecs::prelude::World;
use mir2_game_data::crystal_npc_script_by_key;
use mir2_protocol::ServerPacket;
use crate::config::NativeNpcShopSnapshot;
use super::{npc, resources::{NpcStateResource, SessionResource}};

pub(super) fn current_shop(world: &World) -> Option<NativeNpcShopSnapshot> {
    // A bootstrap/demo World must not manufacture an active player's catalogue.
    world.resource::<SessionResource>().selected_character.as_ref()?;
    let service = npc::current_crystal_npc_service_in_range(world)?;
    if world.resource::<NpcStateResource>().active_npc_dialog.as_ref()
        .is_some_and(|dialog| dialog.npc_object_id != service.npc_object_id) { return None; }
    if !npc::active_crystal_buy_service(&service) { return None; }
    let script = crystal_npc_script_by_key(&service.script_key)?;
    let buy_back = npc::crystal_npc_buy_back_items_for_script(world, &service.script_key);
    let used = npc::crystal_npc_used_goods_for_script(world, &service.script_key);
    let mut packets = npc::crystal_npc_service_packets_for_label_with_markets(
        Some(&script), &service.label_key, &buy_back, &used)?;
    npc::filter_crystal_npc_goods_for_profile(world, &mut packets);
    let (packet_type, list, rate, panel_type, hide_added_stats) = packets.into_iter().find_map(|packet|
        match packet {
            ServerPacket::NPCGoods { list, rate, panel_type, hide_added_stats } =>
                Some(("NPCGoods", list, rate, panel_type, hide_added_stats)),
            ServerPacket::NPCPearlGoods { list, rate, panel_type } =>
                Some(("NPCPearlGoods", list, rate, panel_type, false)),
            _ => None,
        })?;
    let display_goods = mir2_game_data::crystal_npc_goods_list_json(&list, rate).as_array()?.clone();
    let rate = serde_json::to_value(rate).ok()?.as_number()?.clone();
    Some(NativeNpcShopSnapshot { npc_object_id: service.npc_object_id,
        script_key: service.script_key, service: service.label_key,
        packet_type: packet_type.into(), list, display_goods, rate, panel_type, hide_added_stats })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimulationConfig, SimulationSession, VisibleNpcRecord};
    use super::super::{resources::{InventoryResource, NpcStateResource, PlayerRuntimeResource,
        Stage5SystemsResource}, npc::{NpcBuyBackItemState, NpcBuyBackState, NpcUsedGoodsState}};
    use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point};
    const SCRIPT: &str = "BichonProvince/NaturalCave/WickedTrader";
    fn session() -> SimulationSession {
        let mut config = SimulationConfig::default();
        config.visible_npcs.push(VisibleNpcRecord { object_id: 4990, name: "Wicked Trader".into(),
            image: 5, colour_argb: -1, position: Point { x: 331, y: 271 },
            direction: MirDirection::Left, quest_ids: vec![], script_key: Some(SCRIPT.into()) });
        let mut session = SimulationSession::new(config);
        assert!(session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })
            .iter().any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.interact(4990);
        session.select_npc_dialog_target("@BuySell");
        session
    }
    #[test]
    fn npc_purchase_snapshot_trade_matches_actual_packet_without_mutation() {
        let session = session();
        let before = (serde_json::to_value(&session.app.world().resource::<InventoryResource>().inventory_items).unwrap(),
            session.app.world().resource::<PlayerRuntimeResource>().gold);
        let current = session.world_snapshot().native_npc_shop.unwrap();
        let script = crystal_npc_script_by_key(SCRIPT).unwrap();
        let packet = npc::crystal_npc_service_packets_for_label_with_markets(Some(&script), "BUYSELL", &[], &[])
            .unwrap().into_iter().find(|p| matches!(p, ServerPacket::NPCGoods { .. })).unwrap();
        match packet { ServerPacket::NPCGoods { list, rate, panel_type, hide_added_stats } => {
            assert_eq!(current.list, list);
            assert_eq!(current.display_goods, mir2_game_data::crystal_npc_goods_list_json(&list,rate).as_array().unwrap().clone()); assert_eq!(current.rate, serde_json::to_value(rate).unwrap().as_number().unwrap().clone());
            assert_eq!(current.panel_type, panel_type); assert_eq!(current.hide_added_stats, hide_added_stats);
        }, _ => unreachable!() }
        assert_eq!(current.packet_type, "NPCGoods"); assert_eq!(current.service, "BUYSELL");
        assert_eq!(current.npc_object_id, 4990); assert_eq!(current.script_key, SCRIPT);
        assert_eq!(before, (serde_json::to_value(&session.app.world().resource::<InventoryResource>().inventory_items).unwrap(),
            session.app.world().resource::<PlayerRuntimeResource>().gold));
    }
    #[test]
    fn npc_purchase_snapshot_market_preserves_full_u64_and_actual_stock() {
        for service in ["BUYBACK", "BUYUSED"] {
            let mut session = session();
            let mut item = session.world_snapshot().native_npc_shop.unwrap().list.into_iter().find(|i| i.item_index == 658).unwrap();
            item.unique_id = u64::MAX; item.count = 3; item.is_shop_item = false;
            let name = session.app.world().resource::<super::super::resources::SessionResource>()
                .selected_character.as_ref().unwrap().name.clone();
            let mut state = session.app.world_mut().resource_mut::<NpcStateResource>();
            state.active_npc_service.as_mut().unwrap().label_key = service.into();
            if service == "BUYBACK" { state.npc_buy_back_items.push(NpcBuyBackState { script_key: SCRIPT.into(), player_name: name,
                items: vec![NpcBuyBackItemState { item: item.clone(), expires_at_binary_datetime:
                    super::super::inventory::future_binary_datetime_minutes(60) }] }); }
            else { state.npc_used_goods_items.push(NpcUsedGoodsState { script_key: SCRIPT.into(), items: vec![item.clone()] }); }
            drop(state);
            let shop = session.world_snapshot().native_npc_shop.unwrap();
            assert_eq!(shop.service, service); assert_eq!(shop.list, vec![item]);
            let view = serde_json::to_value(session.world_snapshot().client_view()).unwrap();
            assert_eq!(view["nativeNpcShop"]["list"][0]["unique_id"].as_u64(), Some(u64::MAX));
        }
    }
    #[test]
    fn npc_purchase_snapshot_pearl_currency_comes_from_current_service() {
        let mut session = session();
        session.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service.as_mut().unwrap().label_key = "PEARLBUY".into();
        let shop = session.world_snapshot().native_npc_shop.unwrap();
        assert_eq!(shop.packet_type, "NPCPearlGoods"); assert_eq!(shop.service, "PEARLBUY");
        assert!(!shop.list.is_empty()); assert!(!shop.hide_added_stats);
    }
    #[test]
    fn npc_purchase_snapshot_retired_or_nonbuy_context_withdraws_catalogue() {
        let mut session = session();
        session.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service.as_mut().unwrap().label_key = "STORAGE".into();
        assert!(session.world_snapshot().native_npc_shop.is_none());
        session.app.world_mut().resource_mut::<NpcStateResource>().active_npc_service = None;
        assert!(session.world_snapshot().native_npc_shop.is_none());
        assert!(SimulationSession::new(SimulationConfig::default()).world_snapshot().native_npc_shop.is_none());
    }
    #[test]
    fn npc_purchase_snapshot_service_popup_keeps_real_parent_without_text_dialog() {
        let mut session = session();
        // The actual script replaces NPC text with its service popup.
        assert!(session.app.world().resource::<NpcStateResource>().active_npc_dialog.is_none());
        assert_eq!(session.world_snapshot().native_npc_shop.as_ref().map(|s|s.npc_object_id), Some(4990));
        // Capture a real dialog, then reopen the real service. A contradictory
        // other NPC dialog cannot lend its identity to that retained service.
        session.interact(4990);
        let mut other = session.app.world().resource::<NpcStateResource>().active_npc_dialog.clone().unwrap();
        session.select_npc_dialog_target("@BuySell");
        other.npc_object_id = 4991;
        session.app.world_mut().resource_mut::<NpcStateResource>().active_npc_dialog = Some(other);
        assert!(session.world_snapshot().native_npc_shop.is_none());
    }
    #[test]
    fn npc_purchase_snapshot_hero_maximum_is_actual_current_level_table() {
        let mut session = session();
        assert!(session.world_snapshot().hero_max_experience.is_none());
        session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.hero = Some(crate::config::Stage5HeroState {
            name: "SnapshotHero".into(), level: 499, class: MirClass::Wizard, gender: MirGender::Female,
            behaviour: 1, experience: 42, spawned: false, auto_pot: true, auto_hp_percent: 30,
            auto_mp_percent: 40, hp_item_index: 658, mp_item_index: 659 });
        let first = session.world_snapshot();
        assert_eq!(first.hero_max_experience, Some(mir2_game_data::crystal_hero_settings().max_experience(499)));
        assert_eq!(first.stage5_systems.hero.unwrap().experience, 42);
        session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.hero.as_mut().unwrap().level = 500;
        assert_eq!(session.world_snapshot().hero_max_experience, Some(0));
    }
    #[test]
    fn npc_purchase_snapshot_legacy_decode_defaults_only_new_optional_fields() {
        let mut value = serde_json::to_value(session().world_snapshot()).unwrap();
        value.as_object_mut().unwrap().remove("nativeNpcShop"); value.as_object_mut().unwrap().remove("heroMaxExperience");
        let old: crate::WorldSnapshot = serde_json::from_value(value).unwrap();
        assert!(old.native_npc_shop.is_none()); assert!(old.hero_max_experience.is_none());
    }
    #[test]
    fn npc_purchase_snapshot_purchase_updates_same_owner_display_and_keeps_service() {
        let mut session = session();
        let mut item = session.world_snapshot().native_npc_shop.unwrap().list.into_iter().find(|i|i.item_index==658).unwrap();
        item.unique_id=u64::MAX; item.count=3; item.is_shop_item=false;
        let mut second=item.clone(); second.unique_id=u64::MAX-1;
        { let mut state=session.app.world_mut().resource_mut::<NpcStateResource>();
          state.active_npc_service.as_mut().unwrap().label_key="BUYUSED".into();
          state.npc_used_goods_items.push(NpcUsedGoodsState{script_key:SCRIPT.into(),items:vec![item,second]}); }
        session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold=1_000_000;
        // Crystal Used purchases consume the complete stored row; the display
        // panel1 is separate from the actual BuyItem panel0 request.
        for (id,remaining) in [(u64::MAX,1),(u64::MAX-1,0)] {
            let before=session.world_snapshot();
            let packets=session.handle_packet(ClientPacket::BuyItem{item_index:id,count:3,panel_type:0});
            let after=session.world_snapshot();
            let shop=after.native_npc_shop.as_ref().expect("real active used shop remains available");
            let charges:Vec<u32>=packets.iter().filter_map(|p|match p{ServerPacket::LoseGold{gold}=>Some(*gold),_=>None}).collect();
            assert_eq!(charges.len(),1,"one actual charge; packets={packets:?}");
            assert!(after.gold<before.gold,"actual purchase deducts once");
            assert_eq!(before.gold-after.gold,charges[0]);
            assert_eq!(shop.list.len(),remaining); assert_eq!(shop.display_goods.len(),remaining);
            assert!(!shop.list.iter().any(|i|i.unique_id==id));
            assert_eq!(shop.display_goods,mir2_game_data::crystal_npc_goods_list_json(&shop.list,shop.rate.as_f64().unwrap() as f32).as_array().unwrap().clone());
            if remaining==1 {
                assert_eq!(shop.list[0].unique_id,u64::MAX-1); assert_eq!(shop.list[0].count,3);
                assert_eq!(shop.display_goods[0]["count"].as_u64(),Some(3));
                assert_eq!(shop.display_goods[0]["purchaseItemIndex"].as_str(),Some("18446744073709551614"));
            }
            assert_eq!(shop.npc_object_id,4990); assert_eq!(shop.service,"BUYUSED");
            assert!(after.active_npc_dialog.is_none());
        }
    }
}
