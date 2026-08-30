//! Shared renderer-neutral inventory model and Bevy inventory panel.
//!
//! Renders the player's carried items (inventory/belt/equipment slots) from a
//! renderer-neutral [`InventoryModel`] so every host shows the same bag. The
//! panel is presentational: item grants/consumption stay server-authoritative.

#[cfg(not(feature = "native-ui"))]
use bevy::prelude::Resource;
#[cfg(feature = "native-ui")]
use bevy::prelude::*;
#[cfg(feature = "native-ui")]
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val,
};
use serde::{Deserialize, Serialize};

/// Grid slot width/height in CSS px for the inventory grid.
pub const SLOT_SIZE: f32 = 40.0;
/// Slots per row in the bag grid.
pub const SLOTS_PER_ROW: u32 = 8;
/// Crystal's unexpanded `User.Inventory` array includes six belt cells plus
/// forty first-bag cells. The second bag exists only above this exact length.
pub const CRYSTAL_BASE_INVENTORY_CAPACITY: u16 = 46;
/// Six belt cells plus both forty-cell bag pages.
pub const CRYSTAL_MAX_INVENTORY_CAPACITY: u16 = 86;
/// Visible carried-item cells on Crystal's first inventory page.
pub const CRYSTAL_FIRST_BAG_PAGE_SLOTS: u16 = 40;

/// A single carried item in renderer-neutral form.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemModel {
    /// Stable server-side identity of this concrete item stack/instance.
    /// Template `key` values are not instance identities and may be non-numeric.
    #[serde(default)]
    pub unique_id: Option<u64>,
    pub key: String,
    pub name: String,
    pub quantity: u32,
    pub slot: u32,
    /// 0 = inventory bag, 1 = belt, 2 = equipment (client-side grouping).
    pub container: u8,
    /// Crystal item image index. `0` is intentionally treated as no image: a
    /// legacy/incomplete snapshot must not cause the native client to guess an
    /// icon from the item name.
    #[serde(default)]
    pub icon: u16,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub durability_current: Option<u16>,
    #[serde(default)]
    pub durability_max: Option<u16>,
    #[serde(default)]
    pub sell_value: u32,
    #[serde(default)]
    pub equip_slot: Option<String>,
    #[serde(default)]
    pub grade: Option<String>,
    #[serde(default)]
    pub attack: i32,
    #[serde(default)]
    pub defence: i32,
    #[serde(default)]
    pub added_attack: i32,
    #[serde(default)]
    pub added_defence: i32,
    #[serde(default)]
    pub added_luck: i32,
    #[serde(default)]
    pub shape: Option<u16>,
    #[serde(default)]
    pub socket_slots: u8,
}

/// Return the canonical exported Crystal icon path, but deliberately do not
/// manufacture a path for a missing/legacy icon index.
pub fn item_icon_path(icon: u16) -> Option<String> {
    (icon != 0).then(|| format!("original-ui/Items/{icon}.png"))
}

/// Compact durability text for a fixed-size slot. Absence remains absence;
/// it is not rendered as a misleading `0/0` durability value.
pub fn item_durability_label(item: &ItemModel) -> Option<String> {
    match (item.durability_current, item.durability_max) {
        (Some(current), Some(maximum)) => Some(format!("{current}/{maximum}")),
        _ => None,
    }
}

/// The renderer-neutral inventory read model.
#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct InventoryModel {
    /// Authoritative Crystal `User.Inventory.Length`, including the six belt
    /// cells. Legacy snapshots fail closed to the unexpanded length instead
    /// of inferring expansion from the number of occupied items.
    #[serde(default = "default_inventory_capacity")]
    pub capacity: u16,
    pub gold: u32,
    pub items: Vec<ItemModel>,
}

const fn default_inventory_capacity() -> u16 {
    CRYSTAL_BASE_INVENTORY_CAPACITY
}

impl Default for InventoryModel {
    fn default() -> Self {
        Self {
            capacity: default_inventory_capacity(),
            gold: 0,
            items: Vec::new(),
        }
    }
}

impl InventoryModel {
    /// Fail-closed normalization for Crystal's actual inventory-array lengths:
    /// the first expansion is +8, then each later expansion is +4.
    pub fn canonical_capacity(value: u16) -> u16 {
        if value == CRYSTAL_BASE_INVENTORY_CAPACITY
            || ((54..=CRYSTAL_MAX_INVENTORY_CAPACITY).contains(&value) && (value - 54) % 4 == 0)
        {
            value
        } else {
            CRYSTAL_BASE_INVENTORY_CAPACITY
        }
    }

    /// Explicit authoritative capacity. Occupied item count and slot values do
    /// not prove that a character purchased Crystal inventory expansion.
    pub fn effective_capacity(&self) -> u16 {
        Self::canonical_capacity(self.capacity)
    }

    /// Whether Crystal exposes its second carried-item page.
    pub fn second_bag_unlocked(&self) -> bool {
        self.effective_capacity() > CRYSTAL_BASE_INVENTORY_CAPACITY
    }

    /// Number of bag cells exposed by the authoritative Crystal array.
    pub fn bag_slot_capacity(&self) -> u16 {
        CRYSTAL_FIRST_BAG_PAGE_SLOTS
            + self
                .effective_capacity()
                .saturating_sub(CRYSTAL_BASE_INVENTORY_CAPACITY)
    }

    /// Items in a given container, ordered by slot.
    pub fn items_in(&self, container: u8) -> Vec<&ItemModel> {
        let mut items: Vec<&ItemModel> = self
            .items
            .iter()
            .filter(|item| item.container == container)
            .collect();
        items.sort_by_key(|item| item.slot);
        items
    }
}

/// Marker on the inventory panel root.
#[cfg(feature = "native-ui")]
#[derive(Component)]
pub struct InventoryPanelRoot;

/// Marker on the gold label.
#[cfg(feature = "native-ui")]
#[derive(Component)]
pub struct GoldLabel;

/// Text rendered inside one bag slot.
#[cfg(feature = "native-ui")]
#[derive(Component)]
struct InventorySlotLabel {
    slot: u32,
}

/// Build the shared Mir2 inventory panel.
#[cfg(feature = "native-ui")]
pub struct Mir2InventoryPlugin;

#[cfg(feature = "native-ui")]
impl Plugin for Mir2InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryModel>()
            .add_systems(Startup, spawn_inventory_panel)
            .add_systems(
                Update,
                update_inventory_panel.run_if(resource_changed::<InventoryModel>),
            );
    }
}

#[cfg(feature = "native-ui")]
fn spawn_inventory_panel(mut commands: Commands) {
    commands
        .spawn((
            InventoryPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(SLOT_SIZE * SLOTS_PER_ROW as f32 + 16.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.72)),
        ))
        .with_children(|parent| {
            parent.spawn((
                GoldLabel,
                Text::new(""),
                TextFont {
                    font_size: bevy::prelude::FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.72, 0.22)),
            ));
            parent
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: bevy::ui::FlexWrap::Wrap,
                    column_gap: Val::Px(2.0),
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    for slot in 0..(SLOTS_PER_ROW * 3) {
                        grid.spawn((
                            Node {
                                width: Val::Px(SLOT_SIZE - 2.0),
                                height: Val::Px(SLOT_SIZE - 2.0),
                                padding: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.10, 0.12, 0.16, 0.85)),
                        ))
                        .with_children(|slot_node| {
                            slot_node.spawn((
                                InventorySlotLabel { slot },
                                Text::new(""),
                                TextFont {
                                    font_size: bevy::prelude::FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.90, 0.90, 0.86)),
                            ));
                        });
                    }
                });
        });
}

#[cfg(feature = "native-ui")]
fn update_inventory_panel(
    model: Res<InventoryModel>,
    golds: Query<&mut Text, With<GoldLabel>>,
    slots: Query<(&InventorySlotLabel, &mut Text), Without<GoldLabel>>,
) {
    for mut gold in golds {
        gold.0 = format!("{} Gold", model.gold);
    }
    for (slot, mut text) in slots {
        text.0 = bag_slot_label(&model, slot.slot);
    }
}

#[cfg(feature = "native-ui")]
fn bag_slot_label(model: &InventoryModel, slot: u32) -> String {
    model
        .items
        .iter()
        .find(|item| item.container == 0 && item.slot == slot)
        .map(|item| {
            let name = if item.name.trim().is_empty() {
                item.key.as_str()
            } else {
                item.name.as_str()
            };
            if item.quantity > 1 {
                format!("{name} ×{}", item.quantity)
            } else {
                name.to_owned()
            }
        })
        .unwrap_or_default()
}

/// Build a compact one-line inventory summary (renderer-neutral, testable).
pub fn inventory_summary(model: &InventoryModel) -> String {
    let bag = model.items_in(0).len();
    let belt = model.items_in(1).len();
    let equipped = model.items_in(2).len();
    format!(
        "{} gold · {} bag · {} belt · {} equipped",
        model.gold, bag, belt, equipped
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, container: u8, slot: u32) -> ItemModel {
        ItemModel {
            unique_id: None,
            key: key.to_owned(),
            name: key.to_owned(),
            quantity: 1,
            slot,
            container,
            ..ItemModel::default()
        }
    }

    #[test]
    fn items_are_grouped_by_container_and_sorted_by_slot() {
        let model = InventoryModel {
            gold: 500,
            items: vec![
                item("belt2", 1, 2),
                item("bag3", 0, 3),
                item("belt1", 1, 1),
                item("eq", 2, 0),
            ],
            ..Default::default()
        };
        let bag = model.items_in(0);
        assert_eq!(bag.len(), 1);
        assert_eq!(bag[0].key, "bag3");
        let belt = model.items_in(1);
        assert_eq!(belt[0].slot, 1);
        assert_eq!(belt[1].slot, 2);
        assert_eq!(model.items_in(2).len(), 1);
    }

    #[test]
    fn summary_is_compact_and_deterministic() {
        let model = InventoryModel {
            gold: 500,
            items: vec![item("a", 0, 0), item("b", 1, 0), item("c", 2, 0)],
            ..Default::default()
        };
        assert_eq!(
            inventory_summary(&model),
            "500 gold · 1 bag · 1 belt · 1 equipped"
        );
    }

    #[test]
    fn legacy_item_json_without_instance_id_remains_compatible_but_unaddressable() {
        let item: ItemModel = serde_json::from_value(serde_json::json!({
            "key": "small-hp-drug",
            "name": "Small HP Drug",
            "quantity": 1,
            "slot": 0,
            "container": 0
        }))
        .expect("legacy item");
        assert_eq!(item.key, "small-hp-drug");
        assert_eq!(item.unique_id, None);
    }

    #[cfg(feature = "native-ui")]
    #[test]
    fn panel_renders_bag_item_names_and_quantities() {
        let mut app = App::new();
        app.add_plugins(Mir2InventoryPlugin);
        *app.world_mut().resource_mut::<InventoryModel>() = InventoryModel {
            gold: 500,
            items: vec![ItemModel {
                unique_id: Some(42),
                key: "potion".to_owned(),
                name: "Small HP Potion".to_owned(),
                quantity: 3,
                slot: 2,
                container: 0,
                ..ItemModel::default()
            }],
            ..Default::default()
        };
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        let labels = query
            .iter(world)
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        assert!(labels.iter().any(|label| label == "Small HP Potion ×3"));
    }

    #[test]
    fn legacy_item_json_and_full_metadata_are_both_compatible() {
        let legacy: ItemModel = serde_json::from_str(
            r#"{"key":"potion","name":"Potion","quantity":1,"slot":0,"container":0}"#,
        )
        .expect("legacy inventory JSON remains valid");
        assert_eq!(legacy.icon, 0);
        assert_eq!(legacy.description, "");
        assert_eq!(item_icon_path(0), None);

        let full: ItemModel = serde_json::from_str(
            r#"{"uniqueId":42,"key":"sword","name":"Sword","quantity":1,"slot":2,"container":2,"icon":71,"description":"Sharp","durabilityCurrent":35,"durabilityMax":40,"sellValue":123,"equipSlot":"Weapon","grade":"Rare","attack":7,"defence":2,"addedAttack":3,"addedDefence":4,"addedLuck":1,"shape":9,"socketSlots":2}"#,
        )
        .expect("full Crystal item JSON decodes");
        assert_eq!(
            item_icon_path(full.icon).as_deref(),
            Some("original-ui/Items/71.png")
        );
        assert_eq!(item_durability_label(&full).as_deref(), Some("35/40"));
        assert_eq!(full.added_attack, 3);
        assert_eq!(full.socket_slots, 2);
    }

    #[test]
    fn inventory_capacity_is_authoritative_and_legacy_json_fails_closed() {
        let legacy: InventoryModel =
            serde_json::from_str(r#"{"gold":0,"items":[]}"#).expect("legacy inventory model");
        assert_eq!(legacy.capacity, CRYSTAL_BASE_INVENTORY_CAPACITY);
        assert!(!legacy.second_bag_unlocked());
        assert_eq!(legacy.bag_slot_capacity(), 40);

        let first_expansion: InventoryModel =
            serde_json::from_str(r#"{"capacity":54,"gold":0,"items":[]}"#)
                .expect("expanded inventory model");
        assert!(first_expansion.second_bag_unlocked());
        assert_eq!(first_expansion.bag_slot_capacity(), 48);

        let full = InventoryModel {
            capacity: 86,
            ..Default::default()
        };
        assert_eq!(full.bag_slot_capacity(), 80);

        let occupied_slot_without_capacity = InventoryModel {
            items: vec![item("bag2", 0, 40)],
            ..Default::default()
        };
        assert_eq!(occupied_slot_without_capacity.effective_capacity(), 46);
        assert!(!occupied_slot_without_capacity.second_bag_unlocked());

        for illegal in [0, 45, 47, 50, 87, 100, u16::MAX] {
            let model = InventoryModel {
                capacity: illegal,
                ..Default::default()
            };
            assert_eq!(model.effective_capacity(), 46, "illegal value {illegal}");
            assert!(!model.second_bag_unlocked(), "illegal value {illegal}");
        }

        for legal in [46, 54, 58, 62, 66, 70, 74, 78, 82, 86] {
            assert_eq!(InventoryModel::canonical_capacity(legal), legal);
        }
    }
}
