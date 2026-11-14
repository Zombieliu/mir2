//! Ordinary shared-session routing for trade item edits.
use super::*;
use mir2_protocol::MirGridType as Grid;

fn split_pair() -> (GatewaySession, GatewaySession, u64, u64, u32) {
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
    let total_quantity = items.iter().map(|item| item.quantity).sum::<u32>();
    assert!(total_quantity > 0);
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
    let snapshot = a.world_snapshot();
    assert!(snapshot
        .inventory_items
        .iter()
        .all(|item| item.key != "red-potion"));
    let trade = snapshot.stage5_systems.trade.unwrap();
    let ids = (trade.offered_unique_ids[&2], trade.offered_unique_ids[&4]);
    let offered = trade.own_items.as_ref().unwrap();
    for (item, slot) in items.iter().zip([2, 4]) {
        let held = offered[slot]
            .as_ref()
            .expect("the exact detached quote cell");
        assert_eq!(
            (held.unique_id, held.quantity),
            (item.unique_id, item.quantity)
        );
    }
    b.handle_packet(ClientPacket::KeepAlive { time: 994 });
    (a, b, ids.0, ids.1, total_quantity)
}

fn assert_merged_quote(owner: &GatewaySession, peer: &[ServerPacket], target: u64, quantity: u32) {
    let trade = owner.world_snapshot().stage5_systems.trade.unwrap();
    let own = trade.own_items.as_ref().unwrap();
    assert!(own[2].is_none() && own[7].is_none());
    let held = own[4].as_ref().expect("the target stack stays in slot 4");
    assert_eq!((held.unique_id, held.quantity), (target, quantity));
    let raw = held
        .tooltip_source
        .as_ref()
        .unwrap()
        .user_item
        .as_ref()
        .unwrap();
    assert_eq!((raw.unique_id, u32::from(raw.count)), (target, quantity));

    let quote = peer
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::TradeItem { trade_items } => Some(trade_items),
            _ => None,
        })
        .expect("the peer receives the refreshed item quote");
    assert!(quote[2].is_none() && quote[7].is_none());
    let quoted = quote[4].as_ref().expect("the peer sees the target stack");
    assert_eq!(
        (quoted.unique_id, u32::from(quoted.count)),
        (target, quantity)
    );
}

#[test]
fn shared_trade_move_and_merge_refresh_only_partner_and_preserve_quantity() {
    let (mut a, mut b, source, target, before) = split_pair();
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
    assert_merged_quote(&a, &peer, target, before);
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
fn shared_trade_move_and_merge_invalidate_peer_preparation() {
    let (mut a, mut b, source, target, before_quantity) = split_pair();
    b.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(
        b.world_snapshot()
            .stage5_systems
            .trade
            .unwrap()
            .escrow_prepared
    );
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
    assert!(!a
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
    let peer = b.handle_packet(ClientPacket::KeepAlive { time: 997 });
    assert_merged_quote(&a, &peer, target, before_quantity);
    let settled = b.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(
        settled
            .iter()
            .filter(|p| matches!(p, ServerPacket::TradeConfirm))
            .count(),
        1
    );
    assert_eq!(
        settled
            .iter()
            .filter(|p| matches!(p,
        ServerPacket::GainedItem { item } if u32::from(item.count) == before_quantity))
            .count(),
        1
    );
}
