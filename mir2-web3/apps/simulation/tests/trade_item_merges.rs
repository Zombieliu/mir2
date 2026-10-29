//! Trade merging is a deliberate extension of Crystal's client intent.
use mir2_protocol::{ClientPacket, MirGridType as Grid, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession};
use serde_json::{json, Value};
const A: u64 = 91001;
const B: u64 = 91002;
fn fixture(a: u32, b: u32) -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut checkpoint = session.active_character_checkpoint().unwrap();
    let template: Value = checkpoint
        .inventory_items_json
        .iter()
        .map(|raw| serde_json::from_str::<Value>(raw).unwrap())
        .find(|item| item["key"] == "red-potion")
        .unwrap();
    for (slot, id, count) in [(30, A, a), (31, B, b)] {
        let mut item = template.clone();
        item["slot"] = json!(slot);
        item["unique_id"] = json!(id);
        item["quantity"] = json!(count);
        checkpoint.inventory_items_json.push(item.to_string());
    }
    session
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    session.trade_request("Trader");
    session.handle_packet(ClientPacket::TradeReply {
        accept_invite: true,
    });
    session
}
fn deposit(s: &mut SimulationSession, from: i32, to: i32) {
    assert!(s
        .handle_packet(ClientPacket::DepositTradeItem { from, to })
        .contains(&ServerPacket::DepositTradeItem {
            from,
            to,
            success: true
        }));
}
fn merge(s: &mut SimulationSession, from: Grid, to: Grid, a: u64, b: u64, success: bool) {
    assert!(s
        .handle_packet(ClientPacket::MergeItem {
            grid_from: from,
            grid_to: to,
            id_from: a,
            id_to: b
        })
        .contains(&ServerPacket::MergeItem {
            grid_from: from,
            grid_to: to,
            id_from: a,
            id_to: b,
            success
        }));
}
fn move_offer(s: &mut SimulationSession, from: i32, to: i32, success: bool) {
    assert!(s
        .handle_packet(ClientPacket::MoveItem {
            grid: Grid::Trade,
            from,
            to
        })
        .contains(&ServerPacket::MoveItem {
            grid: Grid::Trade,
            from,
            to,
            success
        }));
}
#[test]
fn move_trade_offer_empty_swap_and_same_slot_preserves_items() {
    let mut s = fixture(5, 6);
    deposit(&mut s, 30, 2);
    deposit(&mut s, 31, 4);
    let before = s.world_snapshot().inventory_items;
    move_offer(&mut s, 2, 7, true);
    let trade = s.world_snapshot().stage5_systems.trade.unwrap();
    assert_eq!(trade.offered_unique_ids.get(&7), Some(&A));
    assert!(!trade.offered_slots.contains_key(&2));
    move_offer(&mut s, 7, 4, true);
    let trade = s.world_snapshot().stage5_systems.trade.unwrap();
    assert_eq!(trade.offered_unique_ids.get(&4), Some(&A));
    assert_eq!(trade.offered_unique_ids.get(&7), Some(&B));
    move_offer(&mut s, 4, 4, true);
    assert_eq!(s.world_snapshot().inventory_items, before);
    for (from, to) in [(0, 1), (4, 10), (-1, 2), (4, 256)] {
        move_offer(&mut s, from, to, false);
    }
}
#[test]
fn all_trade_merge_directions_preserve_target_and_partial_source_offers() {
    for (from, to) in [
        (Grid::Inventory, Grid::Trade),
        (Grid::Trade, Grid::Inventory),
        (Grid::Trade, Grid::Trade),
    ] {
        for (a, b, remaining) in [(5, 3, 0), (8, 17, 5)] {
            let mut s = fixture(a, b);
            if from == Grid::Trade {
                deposit(&mut s, 30, 2);
            }
            if to == Grid::Trade {
                deposit(&mut s, 31, 4);
            }
            merge(&mut s, from, to, A, B, true);
            let after = s.world_snapshot();
            assert_eq!(
                after
                    .inventory_items
                    .iter()
                    .find(|i| i.unique_id == B)
                    .unwrap()
                    .quantity,
                (a + b).min(20)
            );
            assert_eq!(
                after
                    .inventory_items
                    .iter()
                    .find(|i| i.unique_id == A)
                    .map_or(0, |i| i.quantity),
                remaining
            );
            let trade = after.stage5_systems.trade.unwrap();
            assert_eq!(
                trade.offered_unique_ids.contains_key(&2),
                from == Grid::Trade && remaining > 0
            );
            assert_eq!(trade.offered_unique_ids.contains_key(&4), to == Grid::Trade);
            let checkpoint = s.active_character_checkpoint().unwrap();
            let mut restored = fixture(1, 1);
            restored
                .restore_active_character_checkpoint(&checkpoint)
                .unwrap();
            assert_eq!(
                restored.world_snapshot().inventory_items,
                after.inventory_items
            );
            assert_eq!(restored.world_snapshot().stage5_systems.trade, Some(trade));
        }
    }
}
#[test]
fn invalid_alias_full_or_incompatible_merge_is_atomic() {
    for mutation in ["none", "full", "metadata", "stale"] {
        let mut s = fixture(5, if mutation == "full" { 20 } else { 3 });
        deposit(&mut s, 31, 4);
        if mutation == "metadata" || mutation == "stale" {
            let mut c = s.active_character_checkpoint().unwrap();
            if mutation == "metadata" {
                let raw = c
                    .inventory_items_json
                    .iter_mut()
                    .find(|raw| {
                        serde_json::from_str::<Value>(raw).unwrap()["unique_id"] == json!(A)
                    })
                    .unwrap();
                let mut item: Value = serde_json::from_str(raw).unwrap();
                item["cursed"] = json!(true);
                *raw = item.to_string();
            } else {
                let mut systems: Value =
                    serde_json::from_str(c.stage5_systems_json.as_ref().unwrap()).unwrap();
                systems["trade"]["offeredUniqueIds"]["4"] = json!(u64::MAX);
                c.stage5_systems_json = Some(systems.to_string());
            }
            s.restore_active_character_checkpoint(&c).unwrap();
        }
        let before = s.world_snapshot();
        if mutation != "none" {
            merge(&mut s, Grid::Inventory, Grid::Trade, A, B, false);
        }
        for (from, to, a, b) in [
            (Grid::Trade, Grid::Trade, A, B),
            (Grid::Inventory, Grid::Trade, B, B),
            (Grid::Belt, Grid::Trade, A, B),
            (Grid::Inventory, Grid::Trade, 999, B),
        ] {
            merge(&mut s, from, to, a, b, false);
        }
        assert_eq!(s.world_snapshot().inventory_items, before.inventory_items);
        assert_eq!(
            s.world_snapshot().stage5_systems.trade,
            before.stage5_systems.trade
        );
    }
}
#[test]
fn reserved_bag_items_cannot_be_moved_or_merged_as_inventory() {
    let mut s = fixture(5, 3);
    deposit(&mut s, 31, 4);
    let before = s.world_snapshot();
    for (from, to) in [(31, 35), (30, 31)] {
        assert!(s
            .handle_packet(ClientPacket::MoveItem {
                grid: Grid::Inventory,
                from,
                to
            })
            .contains(&ServerPacket::MoveItem {
                grid: Grid::Inventory,
                from,
                to,
                success: false
            }));
    }
    merge(&mut s, Grid::Inventory, Grid::Inventory, A, B, false);
    merge(&mut s, Grid::Inventory, Grid::Inventory, B, A, false);
    assert_eq!(s.world_snapshot().inventory_items, before.inventory_items);
    assert_eq!(
        s.world_snapshot().stage5_systems.trade,
        before.stage5_systems.trade
    );
}
#[test]
fn locked_or_prepared_offers_reject_moves_and_merges() {
    for prepared in [false, true] {
        let mut s = fixture(5, 3);
        deposit(&mut s, 31, 4);
        if prepared {
            assert!(s.shared_trade_confirm().1.is_some());
        } else {
            s.handle_packet(ClientPacket::TradeConfirm { locked: true });
        }
        let before = s.world_snapshot();
        move_offer(&mut s, 4, 5, false);
        merge(&mut s, Grid::Inventory, Grid::Trade, A, B, false);
        assert_eq!(s.world_snapshot().inventory_items, before.inventory_items);
        assert_eq!(
            s.world_snapshot().stage5_systems.trade,
            before.stage5_systems.trade
        );
    }
}
