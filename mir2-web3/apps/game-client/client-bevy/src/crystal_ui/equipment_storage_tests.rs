use super::*;
use crate::crystal_ui::hud;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::pending_operations::{apply_inventory_operation_ack, InventoryOperationAck, InventoryOperationFeedback};

fn tooltip(item_index: i32, unique_id: u64, quantity: u32, item_type: u8) -> CrystalItemTooltipSourceModel {
    CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel {
            item_index,
            item_type,
            stack_size: 20,
            ..default()
        },
        user_item: Some(CrystalUserItemModel {
            unique_id,
            item_index,
            count: u16::try_from(quantity).expect("fixture count"),
            ..default()
        }),
        ..default()
    }
}

fn item(container: u8, slot: u32, unique_id: u64, item_index: i32, quantity: u32, item_type: u8) -> ItemModel {
    ItemModel {
        unique_id: Some(unique_id),
        key: format!("item-{unique_id}"),
        name: format!("Item {item_index}"),
        container,
        slot,
        quantity,
        tooltip_source: Some(tooltip(item_index, unique_id, quantity, item_type)),
        ..default()
    }
}

fn equipment_storage_app() -> (App, Entity) {
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
        .init_resource::<hud::CrystalBeltPresentation>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<CursorMoved>()
        .add_message::<bevy::window::WindowEvent>()
        .add_systems(
            Update,
            (sync_storage_inventory_location, process_inventory_item_drag).chain(),
        );
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        state.toggle_equipment();
    }
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        *storage = StorageModel::new();
        storage.size = STORAGE_BASE_SIZE;
    }
    app.update();
    assert!(app.world().resource::<NativePlayerUiState>().storage_open());
    assert!(app.world().resource::<NativePlayerUiState>().equipment_open());
    (app, window)
}

fn storage_point(slot: u32) -> Vec2 {
    let local = slot as usize % crate::storage::STORAGE_PAGE_SIZE as usize;
    Vec2::new(
        9.0 + (local % 10) as f32 * 37.0 + 2.0,
        60.0 + (local / 10) as f32 * 33.0 + 2.0,
    )
}

fn equipment_point(slot: u32) -> Vec2 {
    let (_, rect) = CRYSTAL_CHARACTER_EQUIPMENT_SLOTS
        .iter()
        .find(|(candidate, _)| *candidate == slot)
        .expect("source equipment slot");
    Vec2::new(
        CRYSTAL_CHARACTER_PANEL_RECT.left + rect.left + 2.0,
        CRYSTAL_CHARACTER_PANEL_RECT.top + rect.top + 2.0,
    )
}

fn move_cursor(app: &mut App, window: Entity, point: Vec2) {
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

fn left(app: &mut App, window: Entity, pressed: bool) {
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

fn drag(app: &mut App, window: Entity, source: Vec2, target: Vec2) {
    move_cursor(app, window, source);
    left(app, window, true);
    move_cursor(app, window, target);
    left(app, window, false);
    app.update();
}

fn drain(app: &mut App) -> Vec<NativePlayerUiIntent> {
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
}

#[test]
fn storage_and_character_dialogs_keep_source_bounds_and_slots_reachable_together() {
    let (mut app, _) = equipment_storage_app();
    let state = app.world().resource::<NativePlayerUiState>();
    assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::Storage);
    assert!(state.storage_equipment_visible);
    assert_eq!(state.character_page, CharacterPage::Character);
    assert_eq!(
        equipment_slot_at_cursor(&state, equipment_point(0)),
        Some(0),
        "character equipment hit rect retains its source top-right offset"
    );
    assert!(storage_slot_at_cursor(
        &state,
        app.world().resource::<StorageModel>(),
        app.world().resource::<StorageUiState>(),
        storage_point(5),
    ) == Some(5));

    app.world_mut().resource_mut::<NativePlayerUiState>().toggle_equipment();
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(state.storage_open());
    assert!(!state.equipment_open(), "CharacterDialog closes without closing StorageDialog");
}

#[test]
fn concurrent_storage_character_render_keeps_all_source_character_tabs_and_equipment_cell() {
    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        state.toggle_equipment();
    }
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 0, 9001, 100, 1, 2));
    app.update();

    let world = app.world_mut();
    let (equipment, storage) = {
        let mut equipment = world.query_filtered::<&Node, With<OverlayEquipment>>();
        let mut storage = world.query_filtered::<&Node, With<OverlayStorage>>();
        (equipment.single(world).expect("CharacterDialog root"), storage.single(world).expect("StorageDialog root"))
    };
    assert_eq!(equipment.display, Display::Flex);
    assert_eq!(equipment.left, Val::Px(CRYSTAL_CHARACTER_PANEL_RECT.left));
    assert_eq!(storage.display, Display::Flex);
    assert_eq!(
        world
            .query::<&OverlayButton>()
            .iter(world)
            .filter(|button| matches!(button, OverlayButton::SelectCharacterPage(_)))
            .count(),
        4,
    );
    assert!(world
        .query::<&OverlayButton>()
        .iter(world)
        .any(|button| matches!(button, OverlayButton::InspectEquip(0))));
}

#[test]
fn ordered_equipment_to_storage_uses_remove_item_and_occupied_fallback() {
    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 0, 9001, 100, 1, 2));
    drag(&mut app, window, equipment_point(0), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::RemoveItem {
            unique_id: 9001,
            grid: "storage".to_owned(),
            to: 5,
        }]
    );

    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 0, 9001, 100, 1, 2));
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(item(4, 5, 8001, 101, 1, 2));
    drag(&mut app, window, equipment_point(0), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::RemoveItem {
            unique_id: 9001,
            grid: "storage".to_owned(),
            to: 0,
        }],
        "an occupied non-merge target follows MirItemCell's first free storage fallback"
    );
}

#[test]
fn ordered_equipment_storage_amulet_merge_uses_complete_live_identity() {
    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 9, 9001, 100, 2, 8));
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(item(4, 5, 8001, 100, 4, 8));
    drag(&mut app, window, equipment_point(9), storage_point(5));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::MergeItem {
            grid_from: "equipment".to_owned(),
            grid_to: "storage".to_owned(),
            id_from: 9001,
            id_to: 8001,
        }]
    );
}

#[test]
fn equipment_storage_amulet_merge_locks_its_live_endpoints_until_matching_ack() {
    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 9, 9001, 100, 2, 8));
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(item(4, 5, 8001, 100, 4, 8));

    drag(&mut app, window, equipment_point(9), storage_point(5));
    assert_eq!(drain(&mut app).len(), 1);
    drag(&mut app, window, equipment_point(9), storage_point(5));
    assert!(drain(&mut app).is_empty(), "source cells remain locked pending the merge receipt");

    assert_eq!(
        apply_inventory_operation_ack(
            app.world_mut().resource_mut::<PendingOperations>().into_inner(),
            &mut InventoryOperationFeedback::default(),
            InventoryOperationAck::Merge {
                grid_from: "Equipment".to_owned(),
                grid_to: "Storage".to_owned(),
                id_from: 9001,
                id_to: 8001,
                success: true,
            },
        ),
        1,
    );
    drag(&mut app, window, equipment_point(9), storage_point(5));
    assert_eq!(drain(&mut app).len(), 1, "only the matching receipt releases the gesture");
}

#[test]
fn ordered_storage_to_equipment_uses_actual_slot_and_source_amulet_merge() {
    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(item(4, 5, 8001, 100, 1, 2));
    drag(&mut app, window, storage_point(5), equipment_point(0));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::EquipItem {
            unique_id: 8001,
            grid: "storage".to_owned(),
            to: 0,
        }]
    );

    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<StorageModel>()
        .items
        .push(item(4, 5, 8001, 100, 2, 8));
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 9, 9001, 100, 4, 8));
    drag(&mut app, window, storage_point(5), equipment_point(9));
    assert_eq!(
        drain(&mut app),
        vec![NativePlayerUiIntent::MergeItem {
            grid_from: "storage".to_owned(),
            grid_to: "equipment".to_owned(),
            id_from: 8001,
            id_to: 9001,
        }]
    );
}

#[test]
fn equipment_storage_drag_rechecks_source_identity_and_storage_lock() {
    let (mut app, window) = equipment_storage_app();
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(item(2, 0, 9001, 100, 1, 2));
    move_cursor(&mut app, window, equipment_point(0));
    left(&mut app, window, true);
    app.update();
    app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(9002);
    move_cursor(&mut app, window, storage_point(5));
    left(&mut app, window, false);
    app.update();
    assert!(drain(&mut app).is_empty());

    app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(9001);
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.has_password = true;
        storage.unlocked = false;
    }
    drag(&mut app, window, equipment_point(0), storage_point(5));
    assert!(drain(&mut app).is_empty());
}
