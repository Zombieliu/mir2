use super::*;

fn phone() -> PhoneStoragePresentation {
    PhoneStoragePresentation::fit(Vec2::new(850.0, 393.0), Vec4::ZERO, 0.69)
}
fn fixture() -> (NativePlayerUiState, InventoryModel, StorageModel) {
    let mut state = NativePlayerUiState::default();
    state.core.panel = mir2_ui_core::state::UiPanel::Storage;
    let inventory = InventoryModel {
        items: vec![ItemModel {
            unique_id: Some(501),
            key: "501".into(),
            name: "Potion".into(),
            quantity: 2,
            container: 0,
            slot: 0,
            ..default()
        }],
        ..default()
    };
    let storage = StorageModel {
        has_expanded: true,
        size: 160,
        items: vec![ItemModel {
            unique_id: Some(601),
            key: "601".into(),
            name: "Sword".into(),
            quantity: 1,
            container: 4,
            slot: 159,
            ..default()
        }],
        ..StorageModel::new()
    };
    (state, inventory, storage)
}

#[test]
fn wide_pair_has_disjoint_safe_panes_and_48dp_cells() {
    let p = phone();
    assert!(p.paired());
    assert!(p.pane(false).max.x < p.pane(true).min.x);
    for bag in [false, true] {
        assert!(
            p.columns(bag) as f32 * TAP + (p.columns(bag) - 1) as f32 * GAP
                <= p.pane(bag).width() - 18.0
        );
        assert_eq!(TAP, 48.0);
    }
}
#[test]
fn compact_landscape_still_has_two_reachable_panes() {
    let p = PhoneStoragePresentation::fit(Vec2::new(1560.0 / 2.75, 720.0 / 2.75), Vec4::ZERO, 0.5);
    assert!(p.is_valid() && p.paired());
    assert_eq!(p.columns(false), 4);
}
#[test]
fn narrow_workspace_switches_one_view_without_changing_source_pages() {
    let p = PhoneStoragePresentation::fit(Vec2::new(320.0, 500.0), Vec4::ZERO, 0.4);
    assert!(p.is_valid() && !p.paired());
    assert_eq!(p.pane(true), p.pane(false));
    assert_eq!(InputState::default().shown(), Pane::Storage);
}
#[test]
fn unsafe_geometry_fails_closed() {
    for (viewport, insets, scale) in [
        (Vec2::splat(f32::NAN), Vec4::ZERO, 1.0),
        (Vec2::new(850.0, 393.0), Vec4::new(-1.0, 0.0, 0.0, 0.0), 1.0),
        (Vec2::new(850.0, 393.0), Vec4::ZERO, 0.0),
        (
            Vec2::new(850.0, 393.0),
            Vec4::new(0.0, 0.0, 0.0, 900.0),
            1.0,
        ),
        (Vec2::new(850.0, 180.0), Vec4::ZERO, 1.0),
    ] {
        assert!(!PhoneStoragePresentation::fit(viewport, insets, scale).is_valid());
    }
}
#[test]
fn insets_and_ime_shrink_the_same_pair_workspace() {
    let p = PhoneStoragePresentation::fit(
        Vec2::new(850.0, 600.0),
        Vec4::new(20.0, 10.0, 30.0, 140.0),
        1.0,
    );
    assert_eq!(p.workspace.min, Vec2::new(36.0, 26.0));
    assert_eq!(p.workspace.max, Vec2::new(804.0, 444.0));
}
#[test]
fn vertical_motion_scrolls_even_when_starting_on_an_item() {
    assert_eq!(
        motion(Vec2::ZERO, Vec2::new(4.0, 80.0), 1.0, true),
        Motion::Scroll
    );
    assert_eq!(
        motion(Vec2::ZERO, Vec2::new(80.0, 4.0), 1.0, true),
        Motion::Carry
    );
    assert_eq!(
        motion(Vec2::ZERO, Vec2::new(80.0, 4.0), 1.0, false),
        Motion::Scroll
    );
    assert_eq!(
        motion(Vec2::ZERO, Vec2::new(4.0, 4.0), 1.0, true),
        Motion::Tap
    );
}
#[test]
fn reflow_preserves_both_80_slot_pages_and_sparse_159_identity() {
    let (_, _, storage) = fixture();
    assert_eq!(storage.page(0).slots.len(), 80);
    let page = storage.page(1);
    assert_eq!(page.slots.len(), 80);
    assert_eq!(page.slots.last().unwrap().slot, 159);
    assert_eq!(page.slots.last().unwrap().unique_id, Some(601));
}
#[test]
fn bag_to_storage_calls_shared_pending_path_without_mutating_models() {
    let (mut state, inventory, storage) = fixture();
    let before = serde_json::to_string(&(&inventory, &storage)).unwrap();
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(transfer(
        Cell::Bag(0),
        501,
        Cell::Storage(12),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    let result = intents.drain_intents();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], NativePlayerUiIntent::StoreItem { unique_id, from, to, .. }
        if *unique_id == 501 && *from == 0 && *to == 12)
    );
    assert_eq!(
        serde_json::to_string(&(&inventory, &storage)).unwrap(),
        before
    );
}
#[test]
fn storage_to_bag_retains_wire_slot_159() {
    let (mut state, inventory, storage) = fixture();
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(transfer(
        Cell::Storage(159),
        601,
        Cell::Bag(3),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    let result = intents.drain_intents();
    assert!(
        matches!(&result[0], NativePlayerUiIntent::TakeBackItem { unique_id, from, to, .. }
        if *unique_id == 601 && *from == 159 && *to == 3)
    );
}
#[test]
fn stale_identity_and_password_lock_never_dispatch() {
    let (mut state, inventory, mut storage) = fixture();
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(!transfer(
        Cell::Bag(0),
        999,
        Cell::Storage(12),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    storage.has_password = true;
    storage.unlocked = false;
    assert!(!transfer(
        Cell::Bag(0),
        501,
        Cell::Storage(12),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    assert!(intents.drain_intents().is_empty());
}
#[test]
fn snapshot_and_page_changes_invalidate_pointer_owner() {
    let (mut state, inventory, mut storage) = fixture();
    let mut ui = StorageUiState::default();
    let owner = Owner::capture(&state, &inventory, &storage, &ui);
    ui.cursor.page = 1;
    assert_ne!(owner, Owner::capture(&state, &inventory, &storage, &ui));
    ui.cursor.page = 0;
    storage.items[0].unique_id = Some(700);
    assert_ne!(owner, Owner::capture(&state, &inventory, &storage, &ui));
    storage.items[0].unique_id = Some(601);
    state.inventory_page = 2;
    assert_ne!(owner, Owner::capture(&state, &inventory, &storage, &ui));
}
#[test]
fn missing_presentation_leaves_desktop_path_enabled() {
    assert!(!active(None, &NativePlayerUiState::default()));
}

#[test]
fn equipment_round_trip_uses_existing_remove_and_equip_requests() {
    let (mut state, mut inventory, storage) = fixture();
    inventory.items.push(ItemModel {
        unique_id: Some(801),
        key: "801".into(),
        name: "Sword".into(),
        container: 2,
        slot: 0,
        quantity: 1,
        ..default()
    });
    for (from, id, to, remove) in [
        (Cell::Equipment(0), 801, Cell::Storage(3), true),
        (Cell::Storage(159), 601, Cell::Equipment(0), false),
    ] {
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        let before = serde_json::to_string(&(&inventory, &storage)).unwrap();
        assert!(transfer(
            from,
            id,
            to,
            &mut state,
            &inventory,
            &storage,
            None,
            default(),
            &mut intents,
            &mut pending
        ));
        let result = intents.drain_intents();
        assert_eq!(result.len(), 1);
        assert!(if remove {
            matches!(&result[0], NativePlayerUiIntent::RemoveItem { unique_id: 801, grid, to: 3 } if grid == "storage")
        } else {
            matches!(&result[0], NativePlayerUiIntent::EquipItem { unique_id: 601, grid, to: 0 } if grid == "storage")
        });
        assert_eq!(
            serde_json::to_string(&(&inventory, &storage)).unwrap(),
            before
        );
    }
}

#[test]
fn repeated_transfer_is_rejected_by_shared_pending_keys() {
    let (mut state, inventory, storage) = fixture();
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(transfer(
        Cell::Bag(0),
        501,
        Cell::Storage(3),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    assert!(!transfer(
        Cell::Bag(0),
        501,
        Cell::Storage(4),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    assert_eq!(intents.drain_intents().len(), 1);
}

#[test]
fn bag_move_delegates_to_the_shared_finisher() {
    let (mut state, inventory, storage) = fixture();
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(transfer(
        Cell::Bag(0),
        501,
        Cell::Bag(3),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
    let result = intents.drain_intents();
    assert!(
        matches!(&result[0], NativePlayerUiIntent::MoveItem { unique_id: 501, grid, from: 0, to: 3 } if grid == "inventory")
    );
    assert!(!transfer(
        Cell::Bag(0),
        501,
        Cell::Bag(43),
        &mut state,
        &inventory,
        &storage,
        None,
        default(),
        &mut intents,
        &mut pending
    ));
}

#[test]
fn phone_renderer_has_all_source_slots_no_legacy_button_double_dispatch() {
    let (mut state, inventory, storage) = fixture();
    state.inventory_page = 0;
    let mut storage_ui = StorageUiState::default();
    storage_ui.cursor.page = 1;
    let mut app = App::new();
    app.insert_resource(state)
        .insert_resource(inventory)
        .insert_resource(storage)
        .insert_resource(storage_ui)
        .insert_resource(crate::social::SocialModel::default())
        .insert_resource(UiReadModel::default())
        .add_systems(
            Startup,
            |mut commands: Commands,
             state: Res<NativePlayerUiState>,
             inventory: Res<InventoryModel>,
             storage: Res<StorageModel>,
             storage_ui: Res<StorageUiState>,
             ui: Res<UiReadModel>,
             social: Res<crate::social::SocialModel>| {
                commands.spawn(Node::default()).with_children(|p| {
                    render_storage(
                        p,
                        None,
                        &storage,
                        &storage_ui,
                        &inventory,
                        &state,
                        &ui.player,
                        phone(),
                        None,
                    );
                    render_inventory(
                        p,
                        None,
                        &inventory,
                        &ui,
                        &state,
                        &social,
                        None,
                        &storage,
                        &storage_ui,
                        phone(),
                        None,
                    );
                });
            },
        );
    app.update();
    let world = app.world_mut();
    assert_eq!(world.query::<&Button>().iter(world).count(), 0);
    assert_eq!(world.query::<&OverlayButton>().iter(world).count(), 0);
    let controls = world
        .query::<(&PhoneStorageControl, &Node)>()
        .iter(world)
        .collect::<Vec<_>>();
    assert_eq!(
        controls
            .iter()
            .filter(|(c, _)| matches!(c.action, Some(Action::Cell(Cell::Storage(_)))))
            .count(),
        80
    );
    assert_eq!(
        controls
            .iter()
            .filter(|(c, _)| matches!(c.action, Some(Action::Cell(Cell::Bag(_)))))
            .count(),
        40
    );
    assert!(
        controls
            .iter()
            .any(|(c, _)| c.action == Some(Action::Cell(Cell::Storage(159))))
    );
    for (control, node) in controls {
        if control.clip.is_some() {
            assert_eq!(node.width, Val::Px(48.0 * phone().authored_unit));
        }
        assert_eq!(node.height, Val::Px(48.0 * phone().authored_unit));
    }
}
