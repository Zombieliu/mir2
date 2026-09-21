use super::*;

fn rental_test_app() -> App {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<InventoryModel>()
        .init_resource::<MailModel>()
        .init_resource::<MailComposeUi>()
        .init_resource::<ShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<StorageUiState>()
        .init_resource::<MailUiState>()
        .init_resource::<ShopUiState>()
        .init_resource::<crate::social::SocialModel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::audio::NativeUiAudioQueue>()
        .add_message::<KeyboardInput>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .add_systems(
            Update,
            (
                sync_storage_rental_confirmation,
                process_overlay_keyboard,
                process_overlay_buttons,
            )
                .chain(),
        );
    app
}

fn press(app: &mut App, action: OverlayButton) {
    let entity = app
        .world_mut()
        .spawn((Interaction::Pressed, action, Button))
        .id();
    app.update();
    app.world_mut().despawn(entity);
}

fn press_key(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
}

#[test]
fn second_storage_tab_remains_selectable_and_unrented_page_is_a_rental_cover() {
    let mut app = rental_test_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Storage;
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.size = STORAGE_BASE_SIZE;
        storage.has_expanded = false;
        storage.unlocked = true;
    }

    press(&mut app, OverlayButton::StoragePage(1));
    assert_eq!(app.world().resource::<StorageUiState>().cursor.page, 1);
    let page = app.world().resource::<StorageModel>().page(1);
    assert!(!page.locked, "password state cannot disable the tab");
    assert!(page.rental_locked, "RefreshStorage2 must show the rent cover");
    assert!(page.slots.iter().all(|slot| slot.locked));
}

#[test]
fn rent_only_queues_after_ok_and_renewal_uses_the_source_confirmation() {
    let mut app = rental_test_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Storage;

    // Source does not locally disable Rent for gold; it asks and lets the
    // server produce the authoritative low-gold result.
    press(&mut app, OverlayButton::StorageExpand);
    let prompt = app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_rental_confirmation
        .expect("initial rental confirmation");
    assert!(!prompt.renew);
    assert_eq!(
        prompt.message(),
        "Would you like to rent extra storage for 10 days at a cost of 1,000,000 gold?"
    );
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .is_empty());

    press(&mut app, OverlayButton::StorageRentalCancel);
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .is_empty());

    app.world_mut().resource_mut::<StorageModel>().has_expanded = true;
    press(&mut app, OverlayButton::StorageExpand);
    let prompt = app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_rental_confirmation
        .expect("renewal confirmation");
    assert!(prompt.renew);
    assert_eq!(
        prompt.message(),
        "Would you like to extend your rental period for 10 days at a cost of 1,000,000 gold?"
    );
    press_key(&mut app, KeyCode::Enter);
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .iter()
        .any(|intent| matches!(intent, NativePlayerUiIntent::ExpandStorage)));
    assert!(!app
        .world()
        .resource::<PendingOperations>()
        .contains(&PendingOperationKey::StorageExpand),
        "source low-gold feedback is ordinary chat, so rent cannot reserve an unresolvable pending key"
    );
}

#[test]
fn rental_modal_consumes_close_frame() {
    let mut app = rental_test_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Storage;
    press(&mut app, OverlayButton::StorageExpand);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .blocks_gameplay_keys());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .blocks_world_click());

    // Escape and a covered tab click arrive in one input frame. Cancel owns
    // that frame, so the tab cannot switch under the just-dismissed modal.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
    let covered = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::StoragePage(1), Button))
        .id();
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Escape);
    app.world_mut().despawn(covered);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_rental_confirmation
        .is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_rental_input_consumed);
    assert_eq!(app.world().resource::<StorageUiState>().cursor.page, 0);

}

#[test]
fn low_gold_rental_can_retry_after_an_unchanged_snapshot_and_double_enter_queues_once() {
    let mut app = rental_test_app();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel =
        mir2_ui_core::state::UiPanel::Storage;

    press(&mut app, OverlayButton::StorageExpand);
    press(&mut app, OverlayButton::StorageRentalConfirm);
    let first = app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents();
    assert_eq!(first, vec![NativePlayerUiIntent::ExpandStorage]);
    assert!(!app
        .world()
        .resource::<PendingOperations>()
        .contains(&PendingOperationKey::StorageExpand));

    // A source low-gold response changes neither storage size nor expiry.
    // With no correlated response, that unchanged snapshot must still allow
    // a later funded retry.
    app.update();
    press(&mut app, OverlayButton::StorageExpand);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Enter);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::NumpadEnter);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Enter);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::NumpadEnter);

    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_rental_confirmation
        .is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_rental_input_consumed);
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::ExpandStorage],
        "Enter and NumpadEnter in one frame share the rental close-frame latch"
    );
}

#[test]
fn rendered_unrented_page_hides_cells_and_rental_prompt_blocks_the_full_stage() {
    let mut app = super::tests::overlay_render_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        state.storage_rental_confirmation = Some(StorageRentalConfirmation { renew: false });
    }
    {
        let mut storage_ui = app.world_mut().resource_mut::<StorageUiState>();
        storage_ui.cursor.page = 1;
    }
    app.update();

    let world = app.world_mut();
    let (node, focus) = world
        .query_filtered::<(&Node, &FocusPolicy), With<OverlayStorageRentalModal>>()
        .single(world)
        .expect("rental modal root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(*focus, FocusPolicy::Block);
    assert_eq!(node.width, Val::Px(1024.0));
    assert_eq!(node.height, Val::Px(768.0));
    let storage_cells = world
        .query::<&OverlayButton>()
        .iter(world)
        .filter(|button| matches!(button, OverlayButton::SelectStorage(_)))
        .count();
    assert_eq!(storage_cells, 0, "locked rental page hides every grid cell");
}

#[test]
fn rental_expiry_decodes_dotnet_binary_without_displaying_raw_ticks() {
    assert_eq!(storage_expiry_label(0), None);
    assert_eq!(
        storage_expiry_label(621_355_968_000_000_000),
        Some("1970/01/01 00:00:00".to_owned())
    );
    assert_eq!(
        storage_expiry_label(i64::MAX),
        None,
        "out-of-range values must not reach the label as raw ticks"
    );
}
