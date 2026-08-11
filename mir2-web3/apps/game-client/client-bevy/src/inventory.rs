//! Shared renderer-neutral inventory model and Bevy inventory panel.
//!
//! Renders the player's carried items (inventory/belt/equipment slots) from a
//! renderer-neutral [`InventoryModel`] so every host shows the same bag. The
//! panel is presentational: item grants/consumption stay server-authoritative.

use bevy::prelude::*;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val,
};
use serde::{Deserialize, Serialize};

/// Grid slot width/height in CSS px for the inventory grid.
pub const SLOT_SIZE: f32 = 40.0;
/// Slots per row in the bag grid.
pub const SLOTS_PER_ROW: u32 = 8;

/// A single carried item in renderer-neutral form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemModel {
    pub key: String,
    pub name: String,
    pub quantity: u32,
    pub slot: u32,
    /// 0 = inventory bag, 1 = belt, 2 = equipment (client-side grouping).
    pub container: u8,
}

/// The renderer-neutral inventory read model.
#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct InventoryModel {
    pub gold: u32,
    pub items: Vec<ItemModel>,
}

impl InventoryModel {
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
#[derive(Component)]
pub struct InventoryPanelRoot;

/// Marker on the gold label.
#[derive(Component)]
pub struct GoldLabel;

/// Text rendered inside one bag slot.
#[derive(Component)]
struct InventorySlotLabel {
    slot: u32,
}

/// Build the shared Mir2 inventory panel.
pub struct Mir2InventoryPlugin;

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
            key: key.to_owned(),
            name: key.to_owned(),
            quantity: 1,
            slot,
            container,
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
        };
        assert_eq!(
            inventory_summary(&model),
            "500 gold · 1 bag · 1 belt · 1 equipped"
        );
    }

    #[test]
    fn panel_renders_bag_item_names_and_quantities() {
        let mut app = App::new();
        app.add_plugins(Mir2InventoryPlugin);
        *app.world_mut().resource_mut::<InventoryModel>() = InventoryModel {
            gold: 500,
            items: vec![ItemModel {
                key: "potion".to_owned(),
                name: "Small HP Potion".to_owned(),
                quantity: 3,
                slot: 2,
                container: 0,
            }],
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
}
