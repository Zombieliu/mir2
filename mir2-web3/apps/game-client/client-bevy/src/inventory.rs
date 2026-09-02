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

/// Exact `CrystalItemTemplate` fields consumed by Crystal's item label. This
/// renderer-neutral mirror intentionally follows the source snapshot's
/// snake_case schema; it is not reconstructed from a display name or icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrystalItemInfoModel {
    pub item_index: i32,
    pub name: String,
    pub item_type: u8,
    pub grade: u8,
    pub required_type: u8,
    pub required_class: u8,
    pub required_gender: u8,
    pub item_set: u8,
    pub shape: i16,
    pub weight: u8,
    pub light: u8,
    pub required_amount: u8,
    pub image: u16,
    pub durability: u16,
    pub stack_size: u16,
    pub price: u32,
    pub start_item: bool,
    pub effect: u8,
    pub need_identify: bool,
    pub show_group_pickup: bool,
    pub class_based: bool,
    pub level_based: bool,
    pub can_mine: bool,
    pub global_drop_notify: bool,
    pub bind: i16,
    pub unique: i16,
    pub random_stats_id: u8,
    pub can_fast_run: bool,
    pub can_awakening: bool,
    pub slots: u8,
    pub stats: Vec<CrystalItemStatModel>,
    pub tooltip: Option<String>,
}

impl Default for CrystalItemInfoModel {
    fn default() -> Self {
        Self {
            item_index: 0,
            name: String::new(),
            item_type: 0,
            grade: 0,
            required_type: 0,
            required_class: 31,
            required_gender: 3,
            item_set: 0,
            shape: 0,
            weight: 0,
            light: 0,
            required_amount: 0,
            image: 0,
            durability: 0,
            stack_size: 1,
            price: 0,
            start_item: false,
            effect: 0,
            need_identify: false,
            show_group_pickup: false,
            class_based: false,
            level_based: false,
            can_mine: false,
            global_drop_notify: false,
            bind: 0,
            unique: 0,
            random_stats_id: 0,
            can_fast_run: false,
            can_awakening: false,
            slots: 0,
            stats: Vec::new(),
            tooltip: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrystalItemStatModel {
    pub stat: u8,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrystalUserItemModel {
    pub unique_id: u64,
    pub item_index: i32,
    pub current_dura: u16,
    pub max_dura: u16,
    pub count: u16,
    pub soul_bound_id: i32,
    pub identified: bool,
    pub cursed: bool,
    pub slots: Vec<Option<CrystalUserItemModel>>,
    pub gem_count: u16,
    pub added_stats: Vec<CrystalItemStatModel>,
    pub awake_type: u8,
    pub awake_values: Vec<u8>,
    pub refined_value: u8,
    pub refine_added: u8,
    pub refine_success_chance: i32,
    pub wedding_ring: i32,
    pub expire_info: Option<CrystalUserItemExpireModel>,
    pub rental_information: Option<CrystalUserItemRentalModel>,
    pub is_shop_item: bool,
    pub sealed_info: Option<CrystalUserItemSealedModel>,
    pub gm_made: bool,
}

impl Default for CrystalUserItemModel {
    fn default() -> Self {
        Self {
            unique_id: 0,
            item_index: 0,
            current_dura: 0,
            max_dura: 0,
            count: 0,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 0,
            added_stats: Vec::new(),
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrystalUserItemExpireModel {
    pub expiry_binary_datetime: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrystalUserItemRentalModel {
    pub owner_name: String,
    pub binding_flags: i16,
    pub expiry_binary_datetime: i64,
    pub rental_locked: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrystalUserItemSealedModel {
    pub expiry_binary_datetime: i64,
    pub next_seal_binary_datetime: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CrystalItemTooltipSourceModel {
    pub info: CrystalItemInfoModel,
    pub real_info: Option<CrystalItemInfoModel>,
    pub user_item: Option<CrystalUserItemModel>,
    pub socket_infos: Vec<Option<CrystalItemInfoModel>>,
    pub real_socket_infos: Vec<Option<CrystalItemInfoModel>>,
}

impl CrystalItemTooltipSourceModel {
    /// Crystal's concrete-stack image uses Info and the current authoritative
    /// count, never viewer realInfo or a possibly stale tooltip UserItem count.
    /// Catalogue previews must continue to use `info.image` directly.
    pub fn user_item_image(&self, count: u32) -> u16 {
        mir2_protocol::crystal_user_item_image(
            self.info.item_type,
            self.info.shape,
            self.info.stack_size,
            self.info.image,
            count,
        )
    }
}

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
    /// 0 = inventory bag, 1 = belt, 2 = equipment, 3 = Crystal quest
    /// inventory (read-only client-side grouping).
    pub container: u8,
    /// Crystal item image index. `0` is intentionally treated as no image: a
    /// legacy/incomplete snapshot must not cause the native client to guess an
    /// icon from the item name.
    #[serde(default)]
    pub icon: u16,
    /// Intrinsic `Items.Lib` frame size used by Crystal's `MirItemCell` to
    /// center the icon inside its 36x32 hit cell. Zero means the exported
    /// frame geometry was unavailable and must not be guessed or stretched.
    #[serde(default)]
    pub icon_width: u16,
    #[serde(default)]
    pub icon_height: u16,
    /// Crystal `ItemInfo.Image` in `StateItem.Lib`, used only by the
    /// CharacterDialog paper-doll layers. It is distinct from the inventory
    /// icon above.
    #[serde(default)]
    pub state_image: u16,
    /// Intrinsic `StateItem.Lib` draw geometry. The native gateway resolves
    /// these from the exported library metadata; zero width/height means the
    /// authoritative frame was unavailable and must not be guessed.
    #[serde(default)]
    pub state_image_x: i32,
    #[serde(default)]
    pub state_image_y: i32,
    #[serde(default)]
    pub state_image_width: u16,
    #[serde(default)]
    pub state_image_height: u16,
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
    /// Lossless `ItemInfo`/`UserItem` source for the full Crystal tooltip.
    /// Legacy or partial snapshots leave this absent; presentation code must
    /// then render only fields that are independently authoritative.
    #[serde(default)]
    pub tooltip_source: Option<CrystalItemTooltipSourceModel>,
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

    #[test]
    fn concrete_item_image_uses_current_count_and_base_info_not_tooltip_or_viewer_variant() {
        let source = CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_type: 8,
                shape: 0,
                stack_size: 300,
                image: 2960,
                ..Default::default()
            },
            real_info: Some(CrystalItemInfoModel {
                item_type: 8,
                shape: 2,
                stack_size: 150,
                image: 999,
                ..Default::default()
            }),
            user_item: Some(CrystalUserItemModel {
                count: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(source.user_item_image(199), 3660);
        assert_eq!(source.user_item_image(200), 3661);
        assert_eq!(source.user_item_image(300), 3662);
        assert_eq!(source.info.image, 2960); // Catalogue previews are unchanged.
        assert_eq!(source.user_item.as_ref().unwrap().count, 1);
    }

    #[test]
    fn concrete_item_image_preserves_non_amulets_unknown_shapes_and_zero_stack_size() {
        for (item_type, shape, stack_size) in [(13, 0, 20), (8, 3, 20), (8, 0, 0)] {
            let source = CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_type,
                    shape,
                    stack_size,
                    image: 277,
                    ..Default::default()
                },
                ..Default::default()
            };
            assert_eq!(source.user_item_image(300), 277);
        }
    }

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
    fn crystal_tooltip_source_json_preserves_catalogue_instance_and_socket_metadata() {
        let item: ItemModel = serde_json::from_value(serde_json::json!({
            "uniqueId": 42,
            "key": "wooden-sword",
            "name": "Wooden Sword",
            "quantity": 1,
            "slot": 2,
            "container": 0,
            "tooltipSource": {
                "info": {
                    "item_index": 221,
                    "name": "Wooden Sword",
                    "item_type": 1,
                    "grade": 2,
                    "durability": 4000,
                    "stats": [{ "stat": 5, "value": 4 }]
                },
                "realInfo": {
                    "item_index": 222,
                    "name": "Wooden Sword[Warrior]",
                    "item_type": 1,
                    "grade": 2,
                    "durability": 4000,
                    "stats": [{ "stat": 5, "value": 6 }]
                },
                "userItem": {
                    "unique_id": 42,
                    "item_index": 221,
                    "current_dura": 3000,
                    "max_dura": 4000,
                    "count": 1,
                    "added_stats": [{ "stat": 5, "value": 1 }],
                    "slots": [{
                        "unique_id": 99,
                        "item_index": 900,
                        "current_dura": 1000,
                        "max_dura": 1000,
                        "count": 1
                    }]
                },
                "socketInfos": [{
                    "item_index": 900,
                    "name": "Ruby",
                    "item_type": 29,
                    "stats": [{ "stat": 5, "value": 2 }]
                }],
                "realSocketInfos": [{
                    "item_index": 901,
                    "name": "Ruby[Warrior]",
                    "item_type": 29,
                    "stats": [{ "stat": 5, "value": 3 }]
                }]
            }
        }))
        .expect("lossless Crystal tooltip source");

        let source = item.tooltip_source.expect("tooltip source");
        assert_eq!(source.info.item_index, 221);
        assert_eq!(source.info.grade, 2);
        assert_eq!(source.info.stats[0].value, 4);
        assert_eq!(source.real_info.as_ref().unwrap().item_index, 222);
        assert_eq!(source.real_info.as_ref().unwrap().stats[0].value, 6);
        let user_item = source.user_item.expect("concrete UserItem");
        assert_eq!(user_item.unique_id, 42);
        assert_eq!(user_item.current_dura, 3000);
        assert_eq!(user_item.added_stats[0].value, 1);
        assert_eq!(user_item.slots[0].as_ref().unwrap().item_index, 900);
        assert_eq!(source.socket_infos[0].as_ref().unwrap().name, "Ruby");
        assert_eq!(
            source.real_socket_infos[0].as_ref().unwrap().item_index,
            901
        );

        let legacy: ItemModel = serde_json::from_value(serde_json::json!({
            "key": "wooden-sword",
            "name": "Wooden Sword",
            "quantity": 1,
            "slot": 2,
            "container": 0
        }))
        .expect("legacy item remains compatible");
        assert_eq!(legacy.tooltip_source, None);
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
