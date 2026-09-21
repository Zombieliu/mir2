use super::*;
use bevy::input::keyboard::Key;

fn password_test_app() -> App {
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
                sync_storage_password_prompt,
                process_overlay_keyboard,
                process_overlay_buttons,
            )
                .chain(),
        );
    app
}

fn type_text(app: &mut App, text: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character(text.into()),
        state: ButtonState::Pressed,
        text: Some(text.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

fn press_key(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
}

fn press_button(app: &mut App, button: OverlayButton) {
    let entity = app
        .world_mut()
        .spawn((Interaction::Pressed, button, Button))
        .id();
    app.update();
    app.world_mut().despawn(entity);
}

#[test]
fn locked_storage_opens_a_masked_modal_and_escape_closes_without_sending() {
    let mut app = password_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
    }
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.has_password = true;
        storage.unlocked = false;
        storage.password_draft = "legacy-secret".into();
    }

    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    let prompt = state.storage_password_prompt.as_ref().expect("locked prompt");
    assert_eq!(prompt.stage, StoragePasswordStage::Unlock);
    assert!(state.blocks_gameplay_keys());
    assert!(state.blocks_world_click());
    assert!(app.world().resource::<StorageModel>().password_draft.is_empty());

    type_text(&mut app, " Secret ");
    let prompt = app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_prompt
        .as_ref()
        .expect("typing stays in visible prompt");
    assert_eq!(prompt.masked(), "********");
    assert!(!app.world().resource::<NativePlayerUiState>().chat_focused());

    press_key(&mut app, KeyCode::Escape);
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(!state.storage_open());
    assert!(state.storage_password_prompt.is_none());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn protect_click_runs_current_new_confirm_and_queues_only_after_exact_confirmation() {
    let mut app = password_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
    }
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.has_password = true;
        storage.unlocked = true;
    }

    press_button(&mut app, OverlayButton::StorageSetPassword);
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .storage_password_prompt
            .as_ref()
            .map(|prompt| prompt.stage),
        Some(StoragePasswordStage::ConfirmChange)
    );
    press_button(&mut app, OverlayButton::StoragePasswordSubmit);
    type_text(&mut app, "Old Pw");
    press_key(&mut app, KeyCode::Enter);
    type_text(&mut app, "New Pw");
    press_key(&mut app, KeyCode::Enter);
    type_text(&mut app, "Wrong");
    press_button(&mut app, OverlayButton::StoragePasswordSubmit);
    let prompt = app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_prompt
        .as_ref()
        .expect("mismatch remains editable");
    assert!(prompt.mismatch);
    assert_eq!(prompt.stage, StoragePasswordStage::ConfirmNew);
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .is_empty());

    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.storage_password_prompt.as_mut().expect("prompt").draft = "New Pw".into();
    }
    press_button(&mut app, OverlayButton::StoragePasswordSubmit);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_prompt
        .is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_input_consumed);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .blocks_gameplay_keys());
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::SetStoragePassword {
            current: "Old Pw".into(),
            new_password: "New Pw".into(),
        }]
    );
}

#[test]
fn rejected_password_queue_keeps_the_visible_draft_and_session_reset_drops_it() {
    let mut state = NativePlayerUiState::default();
    state.core.panel = mir2_ui_core::state::UiPanel::Storage;
    state.storage_password_prompt = Some(StoragePasswordPrompt::unlock());
    state
        .storage_password_prompt
        .as_mut()
        .expect("prompt")
        .push_text("secret");
    let mut storage = StorageModel::default();
    let mut intents = NativePlayerUiIntentQueue::default();
    let mut pending = PendingOperations::default();
    assert!(pending.try_begin(PendingOperationKey::StorageUnlock));

    assert!(!submit_storage_password_prompt(
        &mut state,
        &mut storage,
        &mut intents,
        &mut pending,
    ));
    assert_eq!(
        state
            .storage_password_prompt
            .as_ref()
            .expect("duplicate remains retryable")
            .masked(),
        "******"
    );
    assert!(intents.drain_intents().is_empty());

    state.reset_session();
    assert!(state.storage_password_prompt.is_none());
}

#[test]
fn unlock_submission_stays_closed_while_pending_and_reopens_after_a_result() {
    let mut app = password_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
    }
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.has_password = true;
        storage.unlocked = false;
    }
    app.update();
    type_text(&mut app, "secret");
    press_key(&mut app, KeyCode::Enter);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_prompt
        .is_none());
    assert!(app
        .world()
        .resource::<PendingOperations>()
        .contains(&PendingOperationKey::StorageUnlock));
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_prompt
        .is_none());
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .len(),
        1,
        "a pending unlock must not reopen and queue a duplicate"
    );

    app.world_mut().resource_mut::<PendingOperations>().clear();
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .storage_password_prompt
            .as_ref()
            .map(|prompt| prompt.stage),
        Some(StoragePasswordStage::Unlock),
        "a NACK/result that leaves storage locked permits a fresh visible retry"
    );
}

#[test]
fn session_boundary_clears_visible_and_legacy_password_drafts() {
    let mut app = password_test_app();
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        state.storage_password_prompt = Some(StoragePasswordPrompt::setup(false));
        state
            .storage_password_prompt
            .as_mut()
            .expect("prompt")
            .push_text("new secret");
    }
    {
        let mut storage = app.world_mut().resource_mut::<StorageModel>();
        storage.password_draft = "old secret".into();
        storage.new_password_draft = "new secret".into();
        storage.confirm_password_draft = "new secret".into();
    }
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::ConnectionLost;
    app.update();

    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .storage_password_prompt
        .is_none());
    let storage = app.world().resource::<StorageModel>();
    assert!(storage.password_draft.is_empty());
    assert!(storage.new_password_draft.is_empty());
    assert!(storage.confirm_password_draft.is_empty());
}

#[test]
fn modal_frame_uses_the_centered_source_geometry_and_blocks_world_input() {
    assert_eq!(
        CRYSTAL_STORAGE_PASSWORD_RECT,
        CrystalRect::new(368.0, 306.0, 288.0, 156.0)
    );
    let mut state = NativePlayerUiState::default();
    state.storage_password_prompt = Some(StoragePasswordPrompt::setup(false));
    assert!(state.blocks_world_click());
    assert!(state.blocks_gameplay_keys());
}

#[test]
fn rendered_modal_is_a_visible_full_stage_pointer_blocker() {
    let mut app = super::tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .storage_password_prompt = Some(StoragePasswordPrompt::unlock());
    app.update();

    let world = app.world_mut();
    let (node, focus) = world
        .query_filtered::<(&Node, &FocusPolicy), With<OverlayStoragePasswordModal>>()
        .single(world)
        .expect("storage password modal root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(*focus, FocusPolicy::Block);
    assert_eq!(node.width, Val::Px(1024.0));
    assert_eq!(node.height, Val::Px(768.0));
}
