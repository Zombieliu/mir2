use super::*;

fn reader_test_app() -> App {
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
        .init_resource::<crate::social::SocialModel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::audio::NativeUiAudioQueue>()
        .add_message::<KeyboardInput>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });
    super::tests::init_overlay_button_test_resources(&mut app);
    app.add_systems(
        Update,
        (process_overlay_keyboard, process_overlay_buttons).chain(),
    );
    app
}

fn message(id: u64, parcel: bool, read: bool, locked: bool) -> MailMessage {
    MailMessage {
        id,
        sender: format!("Sender{id}"),
        can_reply: id % 2 == 0,
        date_sent_binary_datetime: 638_939_844_960_000_000,
        subject: format!("Subject{id}"),
        body: "first\\r\\nsecond".into(),
        gold: parcel.then_some(25).unwrap_or_default(),
        items: parcel
            .then_some(MailAttachment {
                image: Some(71),
                unique_id: Some(id + 1_000),
                item_index: Some(7),
                name: Some("Potion".into()),
                count: 2,
                ..Default::default()
            })
            .into_iter()
            .collect(),
        read,
        locked,
        ..Default::default()
    }
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
fn opens_unread_once_but_reopens_already_read_without_status_send() {
    let mut app = reader_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.extend([
        message(10, false, false, false),
        message(11, true, true, false),
    ]);

    press_button(&mut app, OverlayButton::ReadMail(10));
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().mail_reader,
        Some(MailReaderUi {
            mail_id: 10,
            kind: MailReaderKind::Letter,
        })
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::ReadMail { mail_id: 10 }]
    );

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = None;
    press_button(&mut app, OverlayButton::ReadMail(11));
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().mail_reader,
        Some(MailReaderUi {
            mail_id: 11,
            kind: MailReaderKind::Parcel,
        })
    );
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = None;
    app.world_mut().resource_mut::<MailModel>().selected_id = Some(10);
    press_button(&mut app, OverlayButton::SelectMail(10));
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().mail_reader,
        Some(MailReaderUi {
            mail_id: 10,
            kind: MailReaderKind::Letter,
        }),
        "source second-click opens the selected row"
    );
}

#[test]
fn exact_id_and_kind_refreshes_reject_stale_reader_actions() {
    let mut model = MailModel {
        mails: vec![message(20, false, true, false)],
        ..Default::default()
    };
    let reader = MailReaderUi {
        mail_id: 20,
        kind: MailReaderKind::Letter,
    };
    assert!(mail_reader_is_current(&reader, &model));
    model.mails[0] = message(20, true, true, false);
    assert!(!mail_reader_is_current(&reader, &model), "attachment changes switch source reader kind");
    model.mails[0] = message(21, false, true, false);
    assert!(!mail_reader_is_current(&reader, &model), "row reordering or replacement cannot retarget ID");

    let mut app = reader_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.push(message(20, false, true, false));
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(reader);
    app.world_mut().resource_mut::<MailModel>().mails.clear();
    press_button(&mut app, OverlayButton::MailReaderDelete);
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn production_refresh_closes_removed_or_retyped_reader_before_input() {
    let mut app = App::new();
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<MailModel>()
        .init_resource::<MailUiState>()
        .init_resource::<InventoryModel>()
        .init_resource::<StorageModel>()
        .init_resource::<StorageUiState>()
        .init_resource::<ShopModel>()
        .init_resource::<ShopUiState>()
        .init_resource::<SkillBindingUi>()
        .init_resource::<SkillModel>()
        .insert_resource(SkillBindingPersistenceRuntime::with_config_path(
            std::env::temp_dir().join(format!(
                "mir2-mail-reader-refresh-{}.json",
                std::process::id()
            )),
        ))
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .add_systems(Update, sync_local_panel_models);
    app.world_mut().resource_mut::<MailModel>().mails.push(message(25, false, true, false));
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 25,
        kind: MailReaderKind::Letter,
    });
    app.world_mut().resource_mut::<MailModel>().mails[0] = message(25, true, true, false);
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(state.mail_reader.is_none());
    assert!(state.mail_reader_input_consumed, "refresh cancellation consumes the exposed frame");
}

#[test]
fn reader_actions_keep_source_guards_and_status_queue_dedup() {
    let mut app = reader_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.extend([
        message(30, false, true, true),
        message(31, false, true, false),
        message(32, true, true, false),
    ]);

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 30,
        kind: MailReaderKind::Letter,
    });
    press_button(&mut app, OverlayButton::MailReaderDelete);
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty(), "locked source letters do not delete");

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 31,
        kind: MailReaderKind::Letter,
    });
    press_button(&mut app, OverlayButton::MailReaderLock);
    press_button(&mut app, OverlayButton::MailReaderLock);
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::LockMail {
            mail_id: 31,
            lock: true,
        }],
        "unacknowledged lock status deduplicates only while queued"
    );

    press_button(&mut app, OverlayButton::MailReaderDelete);
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::DeleteMail { mail_id: 31 }]
    );
    assert!(app.world().resource::<NativePlayerUiState>().mail_reader.is_none());
    // The production sync system clears a close-frame latch on the next UI
    // frame. Model that next frame before exercising an unrelated parcel.
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .mail_reader_input_consumed = false;

    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 32,
        kind: MailReaderKind::Parcel,
    });
    press_button(&mut app, OverlayButton::MailReaderClaim);
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::ClaimMail { mail_id: 32 }]
    );
}

#[test]
fn reply_prefills_only_authorized_sender_and_reader_close_consumes_frame() {
    let mut app = reader_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.extend([
        message(40, false, true, false),
        message(41, false, true, false),
    ]);
    press_button(&mut app, OverlayButton::MailReply(41));
    assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());
    press_button(&mut app, OverlayButton::MailReply(40));
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .core
            .mail_compose
            .as_ref()
            .map(|draft| draft.recipient.as_str()),
        Some("Sender40")
    );

    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.mail_reader = Some(MailReaderUi {
            mail_id: 40,
            kind: MailReaderKind::Letter,
        });
        state.mail_reader_input_consumed = false;
    }
    let close = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::MailReaderClose, Button))
        .id();
    let covered = app
        .world_mut()
        .spawn((Interaction::Pressed, OverlayButton::ReadMail(41), Button))
        .id();
    app.update();
    app.world_mut().despawn(close);
    app.world_mut().despawn(covered);
    assert!(app.world().resource::<NativePlayerUiState>().mail_reader.is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_reader_input_consumed);
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty(), "reader close cannot leak to a covered list action");
}

#[test]
fn source_reader_geometry_body_and_attachment_metadata_are_bounded() {
    let mut app = super::tests::overlay_render_test_app();
    let parcel = message(50, true, true, false);
    app.world_mut().resource_mut::<MailModel>().mails.push(parcel.clone());
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 50,
        kind: MailReaderKind::Parcel,
    });
    app.update();
    let world = app.world_mut();
    let (node, focus) = world
        .query_filtered::<(&Node, &FocusPolicy), With<OverlayMailReadParcel>>()
        .single(world)
        .expect("parcel reader root");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(node.width, Val::Px(236.0));
    assert_eq!(node.height, Val::Px(384.0));
    assert_eq!(*focus, FocusPolicy::Block);
    assert_eq!(mail_reader_text(&parcel), "first\r\nsecond");
    assert!(!mail_date_label(parcel.date_sent_binary_datetime).is_empty());
    assert_eq!(parcel.items.len().min(5), 1);
    assert_eq!(parcel.items[0].image, Some(71), "only explicit attachment images are rendered");
    super::primary_item_image_tests::load_original_images(world);
    let (cell, image, node) = world
        .query::<(&OriginalItemImage, &ImageNode, &Node)>()
        .iter(world)
        .find(|(_, image, _)| image.image.path().is_some_and(|path| path.to_string() == "original-ui/Items/71.png"))
        .expect("parcel attachment uses original item layout");
    assert_eq!((cell.cell_width, cell.cell_height), (35, 31));
    let bitmap = world.resource::<Assets<Image>>().get(&image.image).unwrap();
    let size = bitmap.texture_descriptor.size;
    assert_eq!(node.width, Val::Px(size.width as f32));
    assert_eq!(node.height, Val::Px(size.height as f32));
    assert_eq!(node.display, Display::Flex);
}

#[test]
fn escape_closes_reader_and_session_reset_clears_it() {
    let mut app = reader_test_app();
    app.world_mut().resource_mut::<MailModel>().mails.push(message(60, false, true, false));
    app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(MailReaderUi {
        mail_id: 60,
        kind: MailReaderKind::Letter,
    });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(app.world().resource::<NativePlayerUiState>().mail_reader.is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .mail_reader_input_consumed);

    let mut state = NativePlayerUiState {
        mail_reader: Some(MailReaderUi {
            mail_id: 60,
            kind: MailReaderKind::Letter,
        }),
        ..Default::default()
    };
    state.reset_session();
    assert!(state.mail_reader.is_none());
}
