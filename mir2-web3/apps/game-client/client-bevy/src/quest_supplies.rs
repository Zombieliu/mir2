//! Read-only newcomer supplies. The shared policy is also used by the ordinary
//! journey runner; only the server can buy, consume or equip an item.
use std::{
    collections::BTreeSet,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    inventory::{InventoryModel, ItemModel},
    read_model::PlayerStats,
    skill_model::SkillModel,
};

fn policy() -> &'static serde_json::Value {
    static POLICY: OnceLock<serde_json::Value> = OnceLock::new();
    POLICY.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-supplies.json"
        ))
        .expect("bundled newcomer supply policy")
    })
}

// Guidance refreshes while the character moves. Keep only these templates;
// the game-data convenience lookups clone the complete item catalog per call.
fn supply_templates() -> &'static [mir2_game_data::CrystalItemTemplate] {
    static TEMPLATES: OnceLock<Vec<mir2_game_data::CrystalItemTemplate>> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        mir2_game_data::crystal_item_manifest()
            .items
            .into_iter()
            .filter(|item| matches!(item.item_index, 658..=663 | 710..=712 | 717 | 719))
            .collect()
    })
}

fn number(value: &serde_json::Value) -> u32 {
    value
        .as_u64()
        .and_then(|n| n.try_into().ok())
        .expect("supply policy count")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyVendor {
    Potions,
    General,
    Poison,
}

impl SupplyVendor {
    pub const ALL: [Self; 3] = [Self::Potions, Self::General, Self::Poison];

    fn key(self) -> &'static str {
        match self {
            Self::Potions => "potions",
            Self::General => "general",
            Self::Poison => "poison",
        }
    }

    /// Use the same canonical NPC identity for routing and display lookup.
    /// A pretranslated Simplified Chinese label bypasses the native catalogs
    /// and contains glyphs absent from the bundled Traditional Chinese font.
    pub fn npc_name(self) -> &'static str {
        policy()["merchants"][self.key()]["name"]
            .as_str()
            .expect("bundled supply merchant name")
    }

    pub fn goods(self) -> &'static str {
        match self {
            Self::Potions => "红药、蓝药",
            Self::General => "护身符、回城卷、随机卷",
            Self::Poison => "红毒、绿毒",
        }
    }

    pub fn destination(self) -> Option<SupplyDestination> {
        let merchant = &policy()["merchants"][self.key()];
        let file = merchant["mapFileName"].as_str()?;
        let name = self.npc_name();
        static NPCS: OnceLock<mir2_game_data::CrystalNpcInfoManifest> = OnceLock::new();
        let npc = NPCS
            .get_or_init(mir2_game_data::crystal_npc_info_manifest)
            .npcs
            .iter()
            .find(|npc| npc.name == name && npc.map_file_name.as_deref() == Some(file))?;
        Some(SupplyDestination {
            map_index: npc.map_index,
            x: npc.location.x,
            y: npc.location.y,
            npc_object_id: npc.loaded_object_id?,
        })
    }

    /// Each click requests one ordinary leg. A new map must be acknowledged
    /// before the next entrance or NPC destination is offered.
    pub fn route(self, current_map: i32) -> Option<SupplyRoute> {
        let destination = self.destination()?;
        if current_map == destination.map_index {
            return Some(SupplyRoute {
                map_index: current_map,
                x: destination.x,
                y: destination.y,
                is_entrance: false,
                next_map_title: None,
                remaining_hops: 0,
            });
        }
        match crate::quest_ui::route::resolve(Some(current_map), &[destination.map_index]) {
            crate::quest_ui::route::QuestRoute::NextStep(step) => Some(SupplyRoute {
                map_index: current_map,
                x: step.entrance_x,
                y: step.entrance_y,
                is_entrance: true,
                next_map_title: Some(step.next_map_title),
                remaining_hops: step.remaining_hops,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupplyDestination {
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
    pub npc_object_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupplyRoute {
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
    pub is_entrance: bool,
    pub next_map_title: Option<String>,
    pub remaining_hops: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupplyRow {
    pub label: &'static str,
    pub held: u32,
    pub minimum: u32,
    pub target: u32,
    pub item_name: String,
    pub vendor: SupplyVendor,
    pub unit_price: u32,
}

impl SupplyRow {
    pub fn is_low(&self) -> bool {
        self.held < self.minimum
    }
    pub fn shortage(&self) -> u32 {
        self.target.saturating_sub(self.held)
    }
    pub fn line(&self) -> String {
        crate::player_text::format_named(
            "quest.supply.row",
            "{name}：{held} / 建议 {target}{shortage}",
            &[
                ("name", &crate::player_text::text(self.label)),
                ("held", &self.held.to_string()),
                ("target", &self.target.to_string()),
                (
                    "shortage",
                    &if self.is_low() {
                        crate::player_text::format_named("quest.supply.shortage", "（不足）", &[])
                    } else {
                        String::new()
                    },
                ),
            ],
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupplyPlan {
    pub rows: Vec<SupplyRow>,
    pub potion_size: &'static str,
    pub gold: u32,
    pub estimated_cost: u32,
    pub bag_weight: Option<(u32, u32)>,
    pub bag_full: bool,
    pub needs_taoist_material_guidance: bool,
}

impl SupplyPlan {
    pub fn summary(&self) -> String {
        let missing = self
            .rows
            .iter()
            .filter(|row| row.is_low())
            .map(|row| crate::player_text::text(row.label))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            crate::player_text::text("补给检查 · 查看库存与商店")
        } else {
            crate::player_text::format_named(
                "quest.supply.low",
                "补给不足 · {items}",
                &[("items", &missing.join(crate::player_text::list_separator()))],
            )
        }
    }
}

fn dotnet_ticks_now() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    621_355_968_000_000_000_i64
        .saturating_add(
            i64::try_from(elapsed.as_secs())
                .unwrap_or(i64::MAX)
                .saturating_mul(10_000_000),
        )
        .saturating_add(i64::from(elapsed.subsec_nanos() / 100))
}

fn binary_datetime_ticks(value: i64) -> i64 {
    let ticks = (value as u64 & 0x3fff_ffff_ffff_ffff) as i64;
    // Match Crystal's local-kind wraparound as well as UTC DateTime values.
    if ticks > 3_155_378_975_999_999_999 {
        ticks - 0x4000_0000_0000_0000
    } else {
        ticks
    }
}

fn supply_kind(item: &ItemModel) -> Option<usize> {
    let index = item
        .tooltip_source
        .as_ref()
        .map(|source| source.real_info.as_ref().unwrap_or(&source.info).item_index)
        .or_else(|| {
            item.key
                .strip_prefix("crystal-item-")
                .and_then(|id| id.parse::<i32>().ok())
        });
    if let Some(index) = index {
        return match index {
            658 | 660 | 662 => Some(0),
            659 | 661 | 663 => Some(1),
            712 => Some(2),
            710 | 711 => Some(3),
            719 => Some(4),
            717 => Some(5),
            _ => None,
        };
    }
    // Legacy hosts can omit IDs. Only exact canonical English names are a
    // compatibility fallback; a present, conflicting ID always wins above.
    match item.name.as_str() {
        "(HP)DrugSmall" | "(HP)DrugMedium" | "(HP)DrugLarge" => Some(0),
        "(MP)DrugSmall" | "(MP)DrugMedium" | "(MP)DrugLarge" => Some(1),
        "Amulet" => Some(2),
        "GreenPoison" | "RedPoison" => Some(3),
        "TownTeleport" => Some(4),
        "RandomTeleport" => Some(5),
        _ => None,
    }
}

fn usable_supply_kind(item: &ItemModel, player: &PlayerStats, now_ticks: i64) -> Option<usize> {
    if item.container > 2 || item.quantity == 0 {
        return None;
    }
    let kind = supply_kind(item)?;
    if item.container == 2 && (!matches!(kind, 2 | 3) || !matches!(item.slot, 6 | 9)) {
        return None;
    }
    let minimum_level = match kind {
        2 => 18,
        3 => 14,
        _ => 0,
    };
    if player.level < minimum_level
        || (item.durability_max.unwrap_or(0) > 0 && item.durability_current.unwrap_or(0) == 0)
    {
        return None;
    }
    let info = item
        .tooltip_source
        .as_ref()
        .map(|source| source.real_info.as_ref().unwrap_or(&source.info));
    if let Some(info) = info {
        let class_mask = match player
            .class_name
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "warrior" => 1,
            "wizard" => 2,
            "taoist" => 4,
            _ => 0,
        };
        let gender_mask = match player
            .gender
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "male" => 1,
            "female" => 2,
            _ => 0,
        };
        if info.required_type != 0
            || player.level < u32::from(info.required_amount)
            || (info.required_class != 0 && info.required_class & class_mask == 0)
            || (info.required_gender != 3 && info.required_gender & gender_mask == 0)
        {
            return None;
        }
    }
    let shape = item
        .shape
        .map(i32::from)
        .or_else(|| info.map(|info| i32::from(info.shape)));
    if shape
        .is_some_and(|shape| (kind == 2 && shape != 0) || (kind == 3 && !matches!(shape, 1 | 2)))
    {
        return None;
    }
    if let Some(user) = item
        .tooltip_source
        .as_ref()
        .and_then(|source| source.user_item.as_ref())
    {
        if user.is_shop_item
            || user.expire_info.as_ref().is_some_and(|expiry| {
                binary_datetime_ticks(expiry.expiry_binary_datetime) <= now_ticks
            })
            || user.sealed_info.as_ref().is_some_and(|sealed| {
                binary_datetime_ticks(sealed.expiry_binary_datetime) > now_ticks
            })
        {
            return None;
        }
    }
    Some(kind)
}

fn learned_supply_skill(skills: Option<&SkillModel>, names: &[&str]) -> bool {
    skills.is_some_and(|skills| {
        skills.skills.iter().any(|skill| {
            let identifier = skills
                .bindings
                .iter()
                .find(|binding| binding.skill_id == skill.id)
                .and_then(|binding| binding.spell.as_deref())
                .unwrap_or(skill.name.as_str());
            names
                .iter()
                .any(|name| identifier.eq_ignore_ascii_case(name))
        })
    })
}

pub fn plan(
    player: &PlayerStats,
    inventory: &InventoryModel,
    skills: Option<&SkillModel>,
    quest_id: Option<i32>,
) -> SupplyPlan {
    plan_at(player, inventory, skills, quest_id, dotnet_ticks_now())
}

fn plan_at(
    player: &PlayerStats,
    inventory: &InventoryModel,
    skills: Option<&SkillModel>,
    quest_id: Option<i32>,
    now_ticks: i64,
) -> SupplyPlan {
    let config = policy();
    let class = player.class_name.as_deref().unwrap_or("");
    let taoist = class.eq_ignore_ascii_case("Taoist");
    let wizard = class.eq_ignore_ascii_case("Wizard");
    let tier = config["potionTiers"]
        .as_array()
        .expect("potion tiers")
        .iter()
        .rev()
        .find(|tier| player.level >= number(&tier["minLevel"]))
        .unwrap_or(&config["potionTiers"][0]);
    let potion_size = match tier["name"].as_str() {
        Some("large") => "大型",
        Some("medium") => "中型",
        _ => "小型",
    };
    let learned = |names: &[&str]| learned_supply_skill(skills, names);
    let class_key = class.to_ascii_lowercase();
    let requirements = quest_id
        .and_then(|id| {
            crate::quest_practice::newcomer_config()["quests"]
                .as_array()
                .and_then(|quests| {
                    quests
                        .iter()
                        .find(|quest| quest["id"].as_i64() == Some(i64::from(id)))
                })
        })
        .and_then(|quest| quest["flags"].as_array())
        .into_iter()
        .flatten()
        .flat_map(|flag| {
            flag["requirements"][&class_key]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let needs_amulet = taoist
        && player.level >= number(&config["amulet"]["minimumLevel"])
        && (learned(&["SoulFireBall", "SummonSkeleton"])
            || requirements.contains("SoulFireBall")
            || requirements.contains("SummonSkeleton")
            || requirements.contains("Amulet"));
    let needs_poison = taoist
        && player.level >= number(&config["poison"]["minimumLevel"])
        && (learned(&["Poisoning"])
            || requirements.contains("Poisoning")
            || requirements.contains("poison"));
    let mut held = [0_u32; 6];
    let mut seen = BTreeSet::new();
    for item in &inventory.items {
        let Some(kind) = usable_supply_kind(item, player, now_ticks) else {
            continue;
        };
        if item.unique_id.is_some_and(|id| !seen.insert(id)) {
            continue;
        }
        held[kind] = held[kind].saturating_add(item.quantity);
    }
    let mut rows = Vec::new();
    let mut add = |label, kind, minimum, target, item: String, vendor| {
        let unit_price = supply_templates()
            .iter()
            .find(|template| template.name == item)
            .map_or(0, |template| template.price);
        rows.push(SupplyRow {
            label,
            held: held[kind],
            minimum,
            target,
            item_name: item,
            vendor,
            unit_price,
        });
    };
    let hp_item = supply_templates()
        .iter()
        .find(|item| item.item_index == number(&tier["hpItemIndex"]) as i32)
        .expect("recommended HP template")
        .name
        .clone();
    let mp_item = supply_templates()
        .iter()
        .find(|item| item.item_index == number(&tier["mpItemIndex"]) as i32)
        .expect("recommended MP template")
        .name
        .clone();
    add(
        "红药",
        0,
        number(&config["hp"]["minimum"]),
        number(&config["hp"]["target"]),
        hp_item,
        SupplyVendor::Potions,
    );
    if (wizard || taoist)
        && (player.level >= number(&config["casterMinimumLevel"])
            || skills.is_some_and(|s| !s.skills.is_empty()))
    {
        add(
            "蓝药",
            1,
            number(&tier["mpMinimum"]),
            number(&tier["mpTarget"]),
            mp_item,
            SupplyVendor::Potions,
        );
    }
    if needs_amulet {
        let minimum = quest_id
            .and_then(|id| config["amulet"]["questMinimum"][id.to_string()].as_u64())
            .map(|n| n as u32)
            .unwrap_or_else(|| number(&config["amulet"]["minimum"]));
        add(
            "护身符",
            2,
            minimum,
            number(&config["amulet"]["target"]),
            "Amulet".into(),
            SupplyVendor::General,
        );
    }
    if needs_poison {
        add(
            "毒粉",
            3,
            number(&config["poison"]["minimum"]),
            number(&config["poison"]["target"]),
            "GreenPoison".into(),
            SupplyVendor::Poison,
        );
    }
    let wizard_escape = wizard
        && quest_id.is_some_and(|id| {
            id >= number(&config["wizardTownReserve"]["firstQuestId"]) as i32
                && id <= number(&config["wizardTownReserve"]["lastQuestId"]) as i32
        });
    let town = if wizard_escape {
        number(&config["wizardTownReserve"]["target"])
    } else {
        number(&config["travelRecommendation"]["townTeleport"])
    };
    let town_minimum = if wizard_escape {
        number(&config["wizardTownReserve"]["minimum"])
    } else {
        0
    };
    add(
        "回城卷",
        4,
        town_minimum,
        town,
        "TownTeleport".into(),
        SupplyVendor::General,
    );
    let random = number(&config["travelRecommendation"]["randomTeleport"]);
    add(
        "随机卷",
        5,
        0,
        random,
        "RandomTeleport".into(),
        SupplyVendor::General,
    );
    let estimated_cost = rows
        .iter()
        .map(|r| r.shortage().saturating_mul(r.unit_price))
        .fold(0_u32, u32::saturating_add);
    let occupied = inventory
        .items
        .iter()
        .filter(|item| item.container == 0)
        .map(|item| item.slot)
        .collect::<BTreeSet<_>>()
        .len();
    SupplyPlan {
        rows,
        potion_size,
        gold: player.gold,
        estimated_cost,
        bag_weight: player
            .weights
            .map(|weights| (weights.bag, u32::from(player.max_weight)))
            .or_else(|| {
                player.current_weight_known.then_some((
                    u32::from(player.current_weight),
                    u32::from(player.max_weight),
                ))
            }),
        bag_full: occupied >= usize::from(inventory.bag_slot_capacity()),
        needs_taoist_material_guidance: needs_amulet && needs_poison,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        CrystalItemTooltipSourceModel, CrystalUserItemExpireModel, CrystalUserItemModel,
        CrystalUserItemSealedModel,
    };
    use crate::skill_model::{SkillBinding, SkillEntry};

    fn player(class: &str, level: u32) -> PlayerStats {
        PlayerStats {
            class_name: Some(class.into()),
            level,
            gold: 10000,
            ..Default::default()
        }
    }
    fn item(name: &str, quantity: u32, container: u8, id: u64) -> ItemModel {
        ItemModel {
            name: name.into(),
            quantity,
            container,
            slot: if container == 2 { 9 } else { 0 },
            unique_id: Some(id),
            ..Default::default()
        }
    }
    fn source_item(index: i32, quantity: u32, container: u8, id: u64) -> ItemModel {
        let template = mir2_game_data::crystal_item_by_index(index).unwrap();
        let mut item = item(&template.name, quantity, container, id);
        item.tooltip_source = Some(CrystalItemTooltipSourceModel {
            info: serde_json::from_value(serde_json::to_value(template).unwrap()).unwrap(),
            user_item: Some(CrystalUserItemModel {
                unique_id: id,
                item_index: index,
                ..Default::default()
            }),
            ..Default::default()
        });
        item
    }

    #[test]
    fn wizard_late_route_shows_real_mp_and_upgraded_purchase_size() {
        let inventory = InventoryModel {
            items: vec![item("(MP)DrugSmall", 11, 1, 1)],
            ..Default::default()
        };
        let result = plan(&player("Wizard", 28), &inventory, None, Some(2110021));
        let mp = result.rows.iter().find(|row| row.label == "蓝药").unwrap();
        assert_eq!((mp.held, mp.minimum, mp.target), (11, 12, 24));
        assert_eq!(mp.item_name, "(MP)DrugLarge");
        assert_eq!(mp.unit_price, 225);
        assert!(mp.is_low());
        assert_eq!(
            result
                .rows
                .iter()
                .find(|row| row.label == "回城卷")
                .unwrap()
                .target,
            2
        );
        assert!(!result.rows.iter().any(|row| row.label == "护身符"));
    }

    #[test]
    fn material_inventory_includes_equipment_without_counting_storage_or_duplicates() {
        let inventory = InventoryModel {
            items: vec![
                item("Amulet", 30, 0, 1),
                item("Amulet", 20, 2, 2),
                item("Amulet", 20, 2, 2),
                item("Amulet", 500, 4, 3),
                item("Amulet", 500, 3, 4),
                item("GreenPoison", 2, 2, 5),
                item("RedPoison", 2, 0, 6),
            ],
            ..Default::default()
        };
        let result = plan(&player("Taoist", 28), &inventory, None, Some(2110021));
        let amulet = result
            .rows
            .iter()
            .find(|row| row.label == "护身符")
            .unwrap();
        assert_eq!((amulet.held, amulet.minimum, amulet.target), (50, 48, 100));
        let poison = result.rows.iter().find(|row| row.label == "毒粉").unwrap();
        assert_eq!(poison.held, 4);
        assert!(!poison.is_low());
        assert!(result.needs_taoist_material_guidance);
        assert!(
            !plan(&player("Taoist", 13), &inventory, None, Some(2110021))
                .rows
                .iter()
                .any(|r| matches!(r.label, "护身符" | "毒粉"))
        );
    }

    #[test]
    fn potion_tiers_and_known_weight_do_not_fabricate_inventory_readiness() {
        for (level, expected) in [(15, "小型"), (16, "中型"), (24, "中型"), (25, "大型")] {
            let result = plan(
                &player("Warrior", level),
                &InventoryModel::default(),
                None,
                None,
            );
            assert_eq!(result.potion_size, expected);
            assert_eq!(result.bag_weight, None);
            assert!(!result.rows.iter().any(|row| row.label == "蓝药"));
            assert!(result.estimated_cost > 0);
        }
    }

    #[test]
    fn canonical_item_ids_override_translated_or_conflicting_display_names() {
        let mut hp = source_item(662, 5, 0, 1);
        hp.name = "大型红药".into();
        let mut wrong_id = source_item(221, 99, 0, 2);
        wrong_id.name = "(HP)DrugLarge".into();
        let mut keyed = item("中型蓝药", 7, 1, 3);
        keyed.key = "crystal-item-661".into();
        let mut real = source_item(658, 3, 0, 4);
        real.tooltip_source.as_mut().unwrap().real_info =
            source_item(663, 1, 0, 5).tooltip_source.map(|s| s.info);
        let inventory = InventoryModel {
            items: vec![hp, wrong_id, keyed, real],
            ..Default::default()
        };
        let result = plan(&player("Wizard", 28), &inventory, None, None);
        assert_eq!(
            result.rows.iter().find(|r| r.label == "红药").unwrap().held,
            5
        );
        assert_eq!(
            result.rows.iter().find(|r| r.label == "蓝药").unwrap().held,
            10
        );
    }

    #[test]
    fn expiry_seals_and_broken_instances_use_actual_tooltip_metadata() {
        let now = 638_000_000_000_000_000;
        let mut expired = source_item(712, 10, 0, 1);
        expired
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .expire_info = Some(CrystalUserItemExpireModel {
            expiry_binary_datetime: now,
        });
        let mut fresh = source_item(712, 20, 0, 2);
        fresh
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .expire_info = Some(CrystalUserItemExpireModel {
            expiry_binary_datetime: now + 10_000_000,
        });
        let mut sealed = source_item(712, 30, 2, 3);
        sealed
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .sealed_info = Some(CrystalUserItemSealedModel {
            expiry_binary_datetime: now + 1,
            ..Default::default()
        });
        let mut unsealed = source_item(712, 40, 2, 4);
        unsealed
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .sealed_info = Some(CrystalUserItemSealedModel {
            expiry_binary_datetime: now,
            ..Default::default()
        });
        let mut broken = source_item(712, 50, 0, 5);
        broken.durability_max = Some(10);
        broken.durability_current = Some(0);
        let mut shop = source_item(712, 60, 0, 6);
        shop.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .is_shop_item = true;
        let inventory = InventoryModel {
            items: vec![expired, fresh, sealed, unsealed, broken, shop],
            ..Default::default()
        };
        let result = plan_at(&player("Taoist", 28), &inventory, None, Some(2110021), now);
        assert_eq!(
            result
                .rows
                .iter()
                .find(|r| r.label == "护身符")
                .unwrap()
                .held,
            60
        );
    }

    #[test]
    fn requirements_shapes_and_equipment_slots_reject_unusable_materials_before_deduplication() {
        let owner = player("Taoist", 28);
        let mut wrong_class = source_item(712, 100, 0, 1);
        wrong_class
            .tooltip_source
            .as_mut()
            .unwrap()
            .info
            .required_class = 2;
        let valid_duplicate = source_item(712, 7, 0, 1);
        let mut too_high = source_item(712, 100, 0, 2);
        too_high
            .tooltip_source
            .as_mut()
            .unwrap()
            .info
            .required_amount = 35;
        let mut wrong_shape = source_item(712, 100, 2, 3);
        wrong_shape.shape = Some(1);
        let mut wrong_slot = source_item(710, 100, 2, 4);
        wrong_slot.slot = 0;
        let mut right_bracelet = source_item(711, 4, 2, 5);
        right_bracelet.slot = 6;
        let mut stat_requirement = source_item(712, 100, 0, 6);
        stat_requirement
            .tooltip_source
            .as_mut()
            .unwrap()
            .info
            .required_type = 1;
        let mut wrong_gender = source_item(712, 100, 0, 7);
        wrong_gender
            .tooltip_source
            .as_mut()
            .unwrap()
            .info
            .required_gender = 2;
        let inventory = InventoryModel {
            items: vec![
                wrong_class,
                valid_duplicate,
                too_high,
                wrong_shape,
                wrong_slot,
                right_bracelet,
                stat_requirement,
                wrong_gender,
            ],
            ..Default::default()
        };
        let result = plan(&owner, &inventory, None, Some(2110021));
        assert_eq!(
            result
                .rows
                .iter()
                .find(|r| r.label == "护身符")
                .unwrap()
                .held,
            7
        );
        assert_eq!(
            result.rows.iter().find(|r| r.label == "毒粉").unwrap().held,
            4
        );
    }

    #[test]
    fn learned_material_skills_require_a_matching_authoritative_skill_entry() {
        let mut skills = SkillModel {
            bindings: vec![SkillBinding {
                skill_id: 9,
                spell: Some("SoulFireBall".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!plan(
            &player("Taoist", 28),
            &InventoryModel::default(),
            Some(&skills),
            None
        )
        .rows
        .iter()
        .any(|r| r.label == "护身符"));
        skills.skills.push(SkillEntry {
            id: 9,
            name: "灵魂火符".into(),
            level: 0,
            key: None,
            cooldown_ms: 0,
            mp_cost: 4,
        });
        let learned = plan(
            &player("Taoist", 28),
            &InventoryModel::default(),
            Some(&skills),
            None,
        );
        assert!(learned.rows.iter().any(|r| r.label == "护身符"));
        assert!(!learned.rows.iter().any(|r| r.label == "毒粉"));
        // Requirements are keyed lowercase in the source configuration. The
        // actual N21 Taoist flags require both materials even before learning.
        let practice = plan(
            &player("Taoist", 28),
            &InventoryModel::default(),
            None,
            Some(2110021),
        );
        assert!(practice.needs_taoist_material_guidance);
        assert!(
            !plan(
                &player("Taoist", 28),
                &InventoryModel::default(),
                None,
                Some(2110004)
            )
            .needs_taoist_material_guidance
        );
    }

    #[test]
    fn travel_recommendations_do_not_become_unconfigured_departure_requirements() {
        let result = plan(
            &player("Warrior", 28),
            &InventoryModel::default(),
            None,
            Some(2110021),
        );
        let town = result.rows.iter().find(|r| r.label == "回城卷").unwrap();
        let random = result.rows.iter().find(|r| r.label == "随机卷").unwrap();
        assert_eq!((town.minimum, town.target), (0, 1));
        assert_eq!((random.minimum, random.target), (0, 2));
        assert!(!town.is_low());
        assert!(!random.is_low());
    }

    #[test]
    fn vendor_routes_use_imported_npcs_and_acknowledged_map_leg() {
        for vendor in SupplyVendor::ALL {
            let destination = vendor.destination().expect("ordinary vendor exists");
            let local = vendor.route(destination.map_index).unwrap();
            assert!(!local.is_entrance);
            assert_eq!((local.x, local.y), (destination.x, destination.y));
            assert!(vendor.route(i32::MAX).is_none());
        }
        assert_eq!(
            SupplyVendor::Poison.destination().unwrap().npc_object_id,
            431
        );
        let route = SupplyVendor::Poison
            .route(1)
            .expect("Bichon to poison room ordinary route");
        assert!(route.is_entrance);
        assert!(route.remaining_hops >= 2);
    }
}
