//! Ordinary shared-session routing for trade item edits.
use super::*;
use mir2_protocol::MirGridType as Grid;

fn split_pair() -> (GatewaySession, GatewaySession, u64, u64, i32, i32) {
    let (mut a, mut b) = started_shared_zone_sessions();
    let original = a
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|i| i.key == "red-potion")
        .unwrap();
    let id = if original.unique_id == 0 {
        u64::from(original.slot)
    } else {
        original.unique_id
    };
    // Crystal fills eligible belt cells before bag cells. Use ordinary split
    // packets until those cells are occupied and the next stack lands in a bag.
    for _ in 0..4 {
        if a.world_snapshot()
            .inventory_items
            .iter()
            .filter(|i| i.key == "red-potion")
            .count()
            == 2
        {
            break;
        }
        let packets = a.handle_packet(ClientPacket::SplitItem {
            grid: Grid::Inventory,
            unique_id: id,
            count: 1,
        });
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::SplitItem1 { success: true, .. })),
            "{packets:?}"
        );
    }
    let items: Vec<_> = a
        .world_snapshot()
        .inventory_items
        .into_iter()
        .filter(|i| i.key == "red-potion")
        .collect();
    assert_eq!(items.len(), 2);
    open_shared_trade_pair(&mut a, &mut b);
    for (item, slot) in items.iter().zip([2, 4]) {
        let packets = a.handle_packet(ClientPacket::DepositTradeItem {
            from: i32::from(item.slot),
            to: slot,
        });
        assert!(packets
            .iter()
            .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));
    }
    let trade = a.world_snapshot().stage5_systems.trade.unwrap();
    let ids = (trade.offered_unique_ids[&2], trade.offered_unique_ids[&4]);
    b.handle_packet(ClientPacket::KeepAlive { time: 994 });
    (
        a,
        b,
        ids.0,
        ids.1,
        i32::from(items[0].slot),
        i32::from(items[1].slot),
    )
}

#[test]
fn shared_trade_move_and_merge_refresh_only_partner_and_preserve_quantity() {
    let (mut a, mut b, source, target, _, _) = split_pair();
    let before: u32 = a
        .world_snapshot()
        .inventory_items
        .iter()
        .filter(|i| i.key == "red-potion")
        .map(|i| i.quantity)
        .sum();
    let moved = a.handle_packet(ClientPacket::MoveItem {
        grid: Grid::Trade,
        from: 2,
        to: 7,
    });
    assert!(moved.contains(&ServerPacket::MoveItem {
        grid: Grid::Trade,
        from: 2,
        to: 7,
        success: true
    }));
    assert!(!moved
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeItem { .. })));
    let peer = b.handle_packet(ClientPacket::KeepAlive { time: 995 });
    assert!(peer.iter().any(|p| matches!(p, ServerPacket::TradeItem { trade_items } if trade_items[7].as_ref().is_some_and(|i| i.unique_id == source))));
    let merged = a.handle_packet(ClientPacket::MergeItem {
        grid_from: Grid::Trade,
        grid_to: Grid::Trade,
        id_from: source,
        id_to: target,
    });
    assert!(merged.contains(&ServerPacket::MergeItem {
        grid_from: Grid::Trade,
        grid_to: Grid::Trade,
        id_from: source,
        id_to: target,
        success: true
    }));
    let peer = b.handle_packet(ClientPacket::KeepAlive { time: 996 });
    assert!(peer.iter().any(|p| matches!(p, ServerPacket::TradeItem { trade_items } if trade_items[7].is_none() && trade_items[4].as_ref().is_some_and(|i| i.unique_id == target && u32::from(i.count) == before))));
    a.handle_packet(ClientPacket::TradeCancel);
    assert_eq!(
        a.world_snapshot()
            .inventory_items
            .iter()
            .filter(|i| i.key == "red-potion")
            .map(|i| i.quantity)
            .sum::<u32>(),
        before
    );
}

#[test]
fn shared_trade_peer_preparation_rejects_move_and_merge_with_exact_ack() {
    let (mut a, mut b, source, target, _, _) = split_pair();
    b.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(
        b.world_snapshot()
            .stage5_systems
            .trade
            .unwrap()
            .escrow_prepared
    );
    let before = a.world_snapshot();
    let moved = a.handle_packet(ClientPacket::MoveItem {
        grid: Grid::Trade,
        from: 2,
        to: 7,
    });
    assert!(moved.contains(&ServerPacket::MoveItem {
        grid: Grid::Trade,
        from: 2,
        to: 7,
        success: false
    }));
    let merged = a.handle_packet(ClientPacket::MergeItem {
        grid_from: Grid::Trade,
        grid_to: Grid::Trade,
        id_from: source,
        id_to: target,
    });
    assert!(merged.contains(&ServerPacket::MergeItem {
        grid_from: Grid::Trade,
        grid_to: Grid::Trade,
        id_from: source,
        id_to: target,
        success: false
    }));
    assert_eq!(a.world_snapshot().inventory_items, before.inventory_items);
    assert_eq!(
        a.world_snapshot()
            .stage5_systems
            .trade
            .unwrap()
            .offered_unique_ids,
        before.stage5_systems.trade.unwrap().offered_unique_ids
    );
}
