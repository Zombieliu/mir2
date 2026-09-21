use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::storage::STORAGE_PAGE_SIZE;

fn storage_item(slot: u32, unique_id: u64, item_index: i32, quantity: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(unique_id),
        key: format!("item-{item_index}"),
        name: format!("Item {item_index}"),
        quantity,
        slot,
        container: 4,
        tooltip_source: Some(tooltip_source(item_index, unique_id, quantity)),
        ..Default::default()
    }
}

fn bag_item(slot: u32, unique_id: u64, item_index: i32, quantity: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(unique_id),
        key: format!("item-{item_index}"),
        name: format!("Item {item_index}"),
        quantity,
        slot,
        container: 0,
        tooltip_source: Some(tooltip_source(item_index, unique_id, quantity)),
        ..Default::default()
    }
}

fn tooltip_source(item_index: i32, unique_id: u64, quantity: u32) -> CrystalItemTooltipSourceModel {
    CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel {
            item_index,
            name: format!("Item {item_index}"),
            item_type: 13,
            image: 1,
            stack_size: 20,
            ..Default::default()
        },
        user_item: Some(CrystalUserItemModel {
            unique_id,
            item_index,
            count: u16::try_from(quantity).unwrap_or(u16::MAX),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn storage_drag_app() -> (App, Entity) {
    let mut app = App::new();
    let mut primary = Window::default();
    primary.focused = true;
    primary.resolution.set(1024.0, 768.0);
    let window = app.world_mut().spawn((primary, PrimaryWindow)).id();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<InventoryModel>()
        .init_resource::<StorageModel>()
        .init_resource::<StorageUiState>()
        .init_resource::<StorageInventoryPlacement>()
        .init_resource::<crate::crystal_ui::hud::CrystalBeltPresentation>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<CursorMoved>()
        .add_message::<bevy::window::WindowEvent>()
        .add_systems(
            Update,
            (sync_storage_inventory_location, process_inventory_item_drag).chain(),
        );
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Storage;
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        *storage = StorageModel::new();
        storage.size = STORAGE_BASE_SIZE;
    }
    app.update();
    assert!(app.world().resource::<NativePlayerUiState>().inventory_open());
    assert_eq!(
        (
            app.world().resource::<NativePlayerUiState>().inventory_window.left,
            app.world().resource::<NativePlayerUiState>().inventory_window.top,
        ),
        (393.0, 0.0),
    );
    (app, window)
}

fn bag_point(slot: u32) -> Vec2 {
    Vec2::new(
        393.0
            + INVENTORY_GRID_ORIGIN.x as f32
            + (slot as usize % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32
            + 2.0,
        INVENTORY_GRID_ORIGIN.y as f32
            + (slot as usize / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32
            + 2.0,
    )
}

fn storage_point(slot: u32) -> Vec2 {
    let local = slot as usize % STORAGE_PAGE_SIZE as usize;
    Vec2::new(
        9.0 + (local % 10) as f32 * 37.0 + 2.0,
        60.0 + (local / 10) as f32 * 33.0 + 2.0,
    )
}

fn ordered_move(app: &mut App, window: Entity, point: Vec2) {
    app.world_mut()
        .entity_mut(window)
        .get_mut::<Window>()
        .expect("primary window")
        .set_cursor_position(Some(point));
    app.world_mut()
        .write_message(bevy::window::WindowEvent::CursorMoved(CursorMoved {
            window,
            position: point,
            delta: None,
        }));
}

fn ordered_left(app: &mut App, window: Entity, pressed: bool) {
    app.world_mut().write_message(bevy::window::WindowEvent::MouseButtonInput(
        bevy::input::mouse::MouseButtonInput {
            window,
            button: MouseButton::Left,
            state: if pressed {
                bevy::input::ButtonState::Pressed
            } else {
                bevy::input::ButtonState::Released
            },
        },
    ));
}

fn ordered_drag(app: &mut App, window: Entity, source: Vec2, target: Vec2) {
    ordered_move(app, window, source);
    ordered_left(app, window, true);
    ordered_move(app, window, target);
    ordered_left(app, window, false);
    app.update();
}

fn drain(app: &mut App) -> Vec<NativePlayerUiIntent> {
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
}

#[test]
fn ordered_warehouse_bag_drag_uses_exact_store_v2_target_and_identity() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(2, 7001, 100, 2));

    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::StoreItem {
            request_id: "st-0000000000000001".to_owned(),
            unique_id: 7001,
            from: 2,
            to: 5,
        }]
    );
}

#[test]
fn ordered_warehouse_bag_drag_merges_only_complete_compatible_live_stacks() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(2, 7001, 100, 2));
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(storage_item(5, 8001, 100, 4));

    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::MergeItem {
            grid_from: "inventory".to_owned(),
            grid_to: "storage".to_owned(),
            id_from: 7001,
            id_to: 8001,
        }]
    );
}

#[test]
fn warehouse_bag_drag_falls_back_from_incompatible_or_stale_occupied_target() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(2, 7001, 100, 2));
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(storage_item(5, 8002, 101, 4));

    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::StoreItem {
            request_id: "st-0000000000000001".to_owned(),
            unique_id: 7001,
            from: 2,
            to: 0,
        }]
    );

    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(2, 7001, 100, 2));
    let mut stale_target = storage_item(5, 8001, 100, 4);
    stale_target
        .tooltip_source
        .as_mut()
        .and_then(|source| source.user_item.as_mut())
        .expect("complete fixture")
        .count = 3;
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(stale_target);

    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::StoreItem {
            request_id: "st-0000000000000001".to_owned(),
            unique_id: 7001,
            from: 2,
            to: 0,
        }],
        "stale target metadata must not turn an occupied target into MergeItem",
    );
}

#[test]
fn ordered_warehouse_storage_drag_uses_exact_take_back_v2_target_and_compatible_merge() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(storage_item(5, 8001, 100, 2));

    ordered_drag(&mut app, window, storage_point(5), bag_point(3));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::TakeBackItem {
            request_id: "st-0000000000000001".to_owned(),
            unique_id: 8001,
            from: 5,
            to: 3,
        }]
    );

    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(storage_item(5, 8001, 100, 2));
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(3, 7001, 100, 4));
    ordered_drag(&mut app, window, storage_point(5), bag_point(3));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::MergeItem {
            grid_from: "storage".to_owned(),
            grid_to: "inventory".to_owned(),
            id_from: 8001,
            id_to: 7001,
        }]
    );
}

#[test]
fn pending_warehouse_gesture_locks_its_item_but_not_independent_cells() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .extend([bag_item(2, 7001, 100, 2), bag_item(3, 7002, 101, 2)]);
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(storage_item(5, 8001, 100, 4));

    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    ordered_drag(&mut app, window, bag_point(2), storage_point(6));
    ordered_drag(&mut app, window, bag_point(3), storage_point(6));

    assert_eq!(
        drain(&mut app),
        vec![
            NativePlayerUiIntent::MergeItem {
                grid_from: "inventory".to_owned(),
                grid_to: "storage".to_owned(),
                id_from: 7001,
                id_to: 8001,
            },
            NativePlayerUiIntent::StoreItem {
                request_id: "st-0000000000000002".to_owned(),
                unique_id: 7002,
                from: 3,
                to: 6,
            },
        ],
        "a same-item drag remains source-locked until acknowledgement, while an unrelated cell proceeds",
    );
}

fn ordered_cursor_left(app: &mut App, window: Entity) {
    app.world_mut()
        .write_message(bevy::window::WindowEvent::CursorLeft(
            bevy::window::CursorLeft { window },
        ));
}

#[test]
fn warehouse_drag_rechecks_both_source_identities_and_cancels_on_cursor_exit() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(2, 7001, 100, 2));
    ordered_move(&mut app, window, bag_point(2));
    ordered_left(&mut app, window, true);
    app.update();
    app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(7999);
    ordered_move(&mut app, window, storage_point(5));
    ordered_left(&mut app, window, false);
    app.update();
    assert!(drain(&mut app).is_empty());

    app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(7001);
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(storage_item(5, 8001, 100, 2));
    ordered_move(&mut app, window, storage_point(5));
    ordered_left(&mut app, window, true);
    app.update();
    app.world_mut().resource_mut::<StorageModel>().items[0].unique_id = Some(8999);
    ordered_move(&mut app, window, bag_point(3));
    ordered_left(&mut app, window, false);
    app.update();
    assert!(drain(&mut app).is_empty());

    app.world_mut().resource_mut::<StorageModel>().items[0].unique_id = Some(8001);
    ordered_move(&mut app, window, storage_point(5));
    ordered_left(&mut app, window, true);
    app.update();
    ordered_cursor_left(&mut app, window);
    ordered_move(&mut app, window, bag_point(3));
    ordered_left(&mut app, window, false);
    app.update();
    assert!(drain(&mut app).is_empty());
}

#[test]
fn warehouse_drag_rejects_password_locked_inaccessible_and_full_destinations() {
    let (mut app, window) = storage_drag_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(2, 7001, 100, 2));
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.has_password = true;
        storage.unlocked = false;
    }
    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    assert!(drain(&mut app).is_empty());

    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.has_password = false;
        storage.unlocked = true;
    }
    // The same open StorageDialog restores the concurrent bag as soon as the
    // authoritative unlock state arrives; it does not require a close/reopen.
    ordered_drag(&mut app, window, bag_point(2), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::StoreItem {
            request_id: "st-0000000000000001".to_owned(),
            unique_id: 7001,
            from: 2,
            to: 5,
        }],
    );
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(bag_item(3, 7002, 101, 2));
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.size = STORAGE_EXPANDED_SIZE;
        storage.has_expanded = false;
    }
    app.world_mut().resource_mut::<StorageUiState>().cursor.page = 1;
    ordered_drag(&mut app, window, bag_point(3), storage_point(80));
    assert!(drain(&mut app).is_empty());

    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.size = 1;
        storage.has_expanded = false;
        storage.items = vec![storage_item(0, 8002, 102, 4)];
    }
    app.world_mut().resource_mut::<StorageUiState>().cursor.page = 0;
    ordered_drag(&mut app, window, bag_point(3), storage_point(0));
    assert!(drain(&mut app).is_empty());
}

#[test]
fn warehouse_open_close_and_npc_reopen_keep_their_source_bag_positions() {
    let (mut app, _) = storage_drag_app();
    app.init_resource::<NpcDialogModel>()
        .init_resource::<ShopModel>()
        .add_systems(
            Update,
            sync_npc_dialog_inventory_location.before(sync_storage_inventory_location),
        );
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::None;
    app.update();
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::NpcShop;
    app.update();
    assert_eq!(app.world().resource::<NativePlayerUiState>().inventory_window.left, 445.0);

    app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Storage;
    app.update();
    assert_eq!(
        (
            app.world().resource::<NativePlayerUiState>().inventory_window.left,
            app.world().resource::<NativePlayerUiState>().inventory_window.top,
        ),
        (393.0, 0.0),
    );
}
