//! Crystal Hero requests remain ordinary typed protocol packets.
use crate::native_protocol::NativeOutboundCommand as C;
use mir2_protocol::{ClientPacket, MirGridType};
pub fn command(packet: &ClientPacket) -> Option<C> {
    Some(match packet {
        ClientPacket::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } if matches!(
            grid_from,
            MirGridType::HeroInventory | MirGridType::HeroEquipment
        ) || matches!(
            grid_to,
            MirGridType::HeroInventory | MirGridType::HeroEquipment
        ) =>
        {
            C::MergeItem {
                grid_from: format!("{grid_from:?}"),
                grid_to: format!("{grid_to:?}"),
                id_from: *id_from,
                id_to: *id_to,
            }
        }
        ClientPacket::ChangeHero { list_index } => C::ChangeHero {
            list_index: *list_index,
        },
        ClientPacket::SetHeroBehaviour { behaviour } => C::SetHeroBehaviour {
            behaviour: *behaviour,
        },
        ClientPacket::SetAutoPotValue { stat, value } => C::SetAutoPotValue {
            stat: *stat,
            value: *value,
        },
        ClientPacket::SetAutoPotItem { grid, item_index }
            if matches!(grid, MirGridType::HeroHpItem | MirGridType::HeroMpItem) =>
        {
            C::SetAutoPotItem {
                grid: format!("{grid:?}"),
                item_index: *item_index,
            }
        }
        ClientPacket::TransferHeroItem { from, to } => C::TransferHeroItem {
            from: *from,
            to: *to,
        },
        ClientPacket::TakeBackHeroItem { from, to } => C::TakeBackHeroItem {
            from: *from,
            to: *to,
        },
        ClientPacket::UseItem { grid, unique_id } if *grid == MirGridType::HeroInventory => {
            C::UseItem {
                grid: Some("HeroInventory".into()),
                unique_id: Some(*unique_id),
                key: None,
                slot: None,
            }
        }
        ClientPacket::MoveItem { grid, from, to } if *grid == MirGridType::HeroInventory => {
            C::MoveItem {
                grid: "HeroInventory".into(),
                from: *from,
                to: *to,
            }
        }
        ClientPacket::EquipItem {
            grid,
            unique_id,
            to,
        } if *grid == MirGridType::HeroInventory => C::EquipItem {
            grid: "HeroInventory".into(),
            unique_id: *unique_id,
            to: *to,
        },
        ClientPacket::RemoveItem {
            grid,
            unique_id,
            to,
        } if *grid == MirGridType::HeroInventory => C::RemoveItem {
            grid: "HeroInventory".into(),
            unique_id: *unique_id,
            to: *to,
        },
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero_belt_keeps_exact_uid_and_hero_grid_without_player_slot_alias() {
        let command = command(&ClientPacket::UseItem {
            grid: MirGridType::HeroInventory,
            unique_id: 91,
        })
        .unwrap();
        let value = serde_json::to_value(command).unwrap();
        assert_eq!(value["type"], "useItem");
        assert_eq!(value["grid"], "HeroInventory");
        assert_eq!(value["uniqueId"], 91);
        assert!(value.get("slot").is_none());
        assert!(super::command(&ClientPacket::UseItem {
            grid: MirGridType::Inventory,
            unique_id: 91
        })
        .is_none());
    }
}
