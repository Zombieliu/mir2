//! Crystal MirItemCell Hero UseItem planning. This never mutates custody.
use crate::hero_model::HeroModel;
use crate::inventory::CrystalItemInfoModel;
use mir2_protocol::{ClientPacket as C, MirGridType as G};
#[derive(Debug, Clone, PartialEq)]
pub enum HeroUsePlan {
    Packet(C),
    ConfirmPotion(C),
    Attachment,
    Unavailable,
}
pub fn requirements(model: &HeroModel, item: &CrystalItemInfoModel) -> bool {
    let Some(hero) = model.info.as_ref() else {
        return false;
    };
    if item.required_class & (1u8 << hero.class as u8) == 0
        || item.required_gender & (1u8 << hero.gender as u8) == 0
    {
        return false;
    }
    let required = i32::from(item.required_amount);
    match item.required_type {
        0 => i32::from(hero.level) >= required,
        6 => i32::from(hero.level) <= required,
        t => {
            let stat = match t {
                1 => 1,
                2 => 3,
                3 => 5,
                4 => 7,
                5 => 9,
                7 => 0,
                8 => 2,
                9 => 4,
                10 => 6,
                11 => 8,
                _ => return false,
            };
            model.stats.as_ref().is_some_and(|stats| {
                stats.iter().find(|s| s.stat == stat).map_or(0, |s| s.value) >= required
            })
        }
    }
}
pub fn plan(model: &HeroModel, slot: u8) -> HeroUsePlan {
    let Some(hero) = model.info.as_ref() else {
        return HeroUsePlan::Unavailable;
    };
    let Some(item) = model
        .inventory_view
        .items
        .iter()
        .find(|i| i.container == 0 && i.slot == u32::from(slot))
    else {
        return HeroUsePlan::Unavailable;
    };
    let Some(uid) = item.unique_id.filter(|id| *id != 0) else {
        return HeroUsePlan::Unavailable;
    };
    let Some(source) = item.tooltip_source.as_ref() else {
        return HeroUsePlan::Unavailable;
    };
    let info = source.real_info.as_ref().unwrap_or(&source.info);
    if item.quantity == 0 || !requirements(model, info) {
        return HeroUsePlan::Unavailable;
    }
    let equipped = |slot| {
        model
            .inventory_view
            .items
            .iter()
            .find(|i| i.container == 2 && i.slot == slot)
    };
    let target = match info.item_type {
        1 => Some(0),
        2 => Some(1),
        4 => Some(2),
        5 => Some(4),
        6 => Some(
            if equipped(6).is_none_or(|i| {
                i.tooltip_source
                    .as_ref()
                    .is_some_and(|s| s.info.item_type == 8)
            }) {
                6
            } else {
                5
            },
        ),
        7 => Some(if equipped(8).is_none() { 8 } else { 7 }),
        8 => Some(9),
        9 => Some(10),
        10 => Some(11),
        11 => Some(12),
        12 => Some(3),
        19 => Some(13),
        _ => None,
    };
    if let Some(to) = target {
        if hero.hp <= 0 || (model.riding_mount == Some(true) && info.item_type != 12) {
            return HeroUsePlan::Unavailable;
        }
        if info.item_type == 8 {
            if let Some(target) = equipped(to).filter(|target| {
                target
                    .tooltip_source
                    .as_ref()
                    .is_some_and(|s| s.info.item_index == info.item_index)
                    && target.quantity < u32::from(info.stack_size)
            }) {
                if let Some(id_to) = target.unique_id.filter(|id| *id != 0 && *id != uid) {
                    return HeroUsePlan::Packet(C::MergeItem {
                        grid_from: G::HeroInventory,
                        grid_to: G::HeroEquipment,
                        id_from: uid,
                        id_to,
                    });
                }
            }
        }
        return HeroUsePlan::Packet(C::EquipItem {
            grid: G::HeroInventory,
            unique_id: uid,
            to: to as i32,
        });
    }
    match info.item_type {
        13 | 17 | 20 | 21 | 27 | 36 | 37 | 38 | 40 | 42 => {
            let packet = C::UseItem {
                grid: G::HeroInventory,
                unique_id: uid,
            };
            if info.item_type == 13 && info.shape == 4 {
                HeroUsePlan::ConfirmPotion(packet)
            } else {
                HeroUsePlan::Packet(packet)
            }
        }
        22..=26 | 28..=32 | 39 => HeroUsePlan::Attachment,
        _ => HeroUsePlan::Unavailable,
    }
}

/// The first source-matching bag stack is eligible only after the consumed belt is empty.
pub fn restock_candidate(model: &HeroModel, slot: u8) -> Option<(u8, u8, u64)> {
    if slot > 1 {
        return None;
    }
    let items = model.info.as_ref()?.inventory.as_ref()?;
    let item = items.get(slot as usize)?.as_ref()?;
    if item.count != 1 {
        return None;
    }
    items.iter().enumerate().skip(2).find_map(|(from, other)| {
        let other = other.as_ref()?;
        (other.item_index == item.item_index && other.count > 0 && other.unique_id != 0)
            .then_some((slot, from as u8, other.unique_id))
    })
}
pub fn restock_ready(model: &HeroModel, candidate: (u8, u8, u64)) -> bool {
    let Some(items) = model.info.as_ref().and_then(|h| h.inventory.as_ref()) else {
        return false;
    };
    items.get(candidate.0 as usize).is_some_and(Option::is_none)
        && items
            .get(candidate.1 as usize)
            .and_then(Option::as_ref)
            .is_some_and(|i| i.unique_id == candidate.2 && i.count > 0)
}

pub fn remove_plan(model: &HeroModel, slot: u8) -> Option<C> {
    let hero = model.info.as_ref()?;
    if hero.hp <= 0 || (model.riding_mount == Some(true) && slot != 3) {
        return None;
    }
    let item = hero.equipment.as_ref()?.get(slot as usize)?.as_ref()?;
    let bag = hero.inventory.as_ref()?;
    // Repair original negative belt-index fallback: keep raw HeroInventory indices.
    let to = (2..bag.len())
        .chain(0..bag.len().min(2))
        .find(|i| bag[*i].is_none())?;
    Some(C::RemoveItem {
        grid: G::HeroInventory,
        unique_id: item.unique_id,
        to: to as i32,
    })
}

#[cfg(test)]
#[path = "hero_item_use_tests.rs"]
mod tests;
