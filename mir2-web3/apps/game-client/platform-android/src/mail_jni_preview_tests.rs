use super::*;

const MAIL_JNI_SCENES: [&str; 4] = [
    "mail-claim-jni",
    "mail-claim-failure-jni",
    "mail-send-jni",
    "mail-send-failure-jni",
];

#[test]
fn mail_jni_scenes_wait_for_real_java_models_and_open_the_shared_mail_panel() {
    for scene in MAIL_JNI_SCENES {
        assert!(
            is_personal_jni_preview(scene),
            "Mail still enters the manual specimen path"
        );
        assert!(SCENES.contains(&scene));
        assert_eq!(preview_panel_for_scene(scene), UiPanel::Mail);
    }
}

fn received_models(
    scene: &str,
) -> (
    mir2_client_bevy::mail::MailModel,
    mir2_client_bevy::inventory::InventoryModel,
    UiReadModel,
) {
    use mir2_client_bevy::mail::{MailModel, MailOperationKind};
    let (kind, success) = mail_jni_expected(scene).unwrap();
    let rows = (0..256).map(|index| serde_json::json!({
        "id":if index == 255 {u64::MAX} else {index + 1}, "sender":"JNI mail", "subject":"JNI", "body":"JNI",
        "gold":77,"claimed":false,"items":(0..5).map(|item| serde_json::json!({
            "uniqueId":u64::MAX-(index*5+item),"name":"JNI attachment","count":item+1
        })).collect::<Vec<_>>()
    })).chain(std::iter::once(serde_json::json!({"id":u64::MAX,"operation":{
        "kind":kind,"success":success,"mailId":(kind==MailOperationKind::Collect).then_some(u64::MAX)
    }}))).collect::<Vec<_>>();
    let mail: MailModel = serde_json::from_value(serde_json::json!({"mails":rows})).unwrap();
    assert_eq!(mail.mails.len(), 257);
    let mut inventory = mir2_client_bevy::inventory::InventoryModel::default();
    for index in 0..12 {
        inventory.items.push(Default::default());
        inventory.items.last_mut().unwrap().unique_id = Some(80000 + index);
    }
    let mut ui = UiReadModel::default();
    ui.player.name = Some("OFFLINE JAVA JNI".into());
    ui.player.gold = 777;
    ui.player.credit = 33;
    // No JNI is called here. These typed models test only the read-only observer.
    (mail, inventory, ui)
}

#[test]
fn mail_jni_observer_requires_the_exact_full_mailbox_and_owned_feedback() {
    for scene in MAIL_JNI_SCENES {
        let (mail, inventory, ui) = received_models(scene);
        assert!(mail_jni_models_received(scene, &mail, &inventory, &ui));
        assert!(!mail_jni_models_received("mail", &mail, &inventory, &ui));
        let mut short = mail.clone();
        short.mails.remove(0);
        assert!(!mail_jni_models_received(scene, &short, &inventory, &ui));
        let mut granted = mail.clone();
        granted.mails[255].claimed = true;
        assert!(!mail_jni_models_received(scene, &granted, &inventory, &ui));
        let mut no_feedback = mail.clone();
        no_feedback.mails.pop();
        assert!(!mail_jni_models_received(
            scene,
            &no_feedback,
            &inventory,
            &ui
        ));
    }
}

#[test]
fn mail_jni_scenes_never_seed_or_replace_actual_received_models_or_authorization() {
    use mir2_client_bevy::mail::{MailMessage, MailModel};
    for scene in MAIL_JNI_SCENES {
        let mut world = World::new();
        world.insert_resource(MailModel {
            mails: vec![MailMessage {
                id: 8801,
                subject: "Received sentinel".into(),
                ..default()
            }],
            ..default()
        });
        let mut host = crate::shared_shell::HostState::default();
        host.phase = "DISCONNECTED".into();
        world.insert_resource(host);
        world.insert_resource(crate::AndroidGatewayTransportEnabled(false));
        populate_specimens(&mut world, scene);
        assert_eq!(world.resource::<MailModel>().mails.len(), 1);
        assert_eq!(world.resource::<MailModel>().mails[0].id, 8801);
        assert_eq!(
            world.resource::<crate::shared_shell::HostState>().phase,
            "DISCONNECTED"
        );
        assert!(!world.resource::<crate::AndroidGatewayTransportEnabled>().0);
    }
}

#[test]
fn mail_jni_scene_identity_uses_the_same_declared_java_owner() {
    for scene in MAIL_JNI_SCENES {
        let mut world = World::new();
        world.insert_resource(PreviewRequest {
            scene: Some(scene.into()),
            remaining: 0,
        });
        world.init_resource::<OfflineNpcPreviewReceipt>();
        apply(&mut world);
        let shell = world.resource::<NativeShellModel>();
        assert_eq!(
            shell.active_character.as_ref().unwrap().name,
            "OFFLINE JAVA JNI"
        );
        assert_eq!(shell.selected_character_index, Some(7));
    }
}

#[test]
fn mail_jni_observer_accepts_delayed_models_after_preview_request_is_retired() {
    for scene in MAIL_JNI_SCENES {
        let mut app = App::new();
        app.insert_resource(PreviewRequest {
            scene: Some(scene.into()),
            remaining: 0,
        });
        app.init_resource::<OfflineNpcPreviewReceipt>()
            .init_resource::<OfflineMailJniReceipt>()
            .init_resource::<NativePlayerUiState>();
        for _ in 0..4 {
            apply(app.world_mut());
        }
        assert!(app.world().resource::<PreviewRequest>().scene.is_none());
        // Typed, delayed models test only the real observer's request lifecycle.
        // This is not Java/JNI, a rendered frame, an operation or settlement.
        let (mail, inventory, ui) = received_models(scene);
        let mut host = crate::shared_shell::HostState::default();
        host.phase = "IN_GAME".into();
        app.insert_resource(host)
            .insert_resource(mail)
            .insert_resource(inventory)
            .insert_resource(ui)
            .add_systems(Update, report_mail_jni_consumer);
        app.update();
        let receipt = app.world().resource::<OfflineMailJniReceipt>();
        assert!(receipt.feedback_observed, "Lost delayed scene {scene}");
        assert!(receipt.feedback_this_frame);
        // A later preview must not inherit a completed mail observation.
        *app.world_mut().resource_mut::<PreviewRequest>() = PreviewRequest {
            scene: Some("roster".into()),
            remaining: 0,
        };
        apply(app.world_mut());
        app.update();
        let receipt = app.world().resource::<OfflineMailJniReceipt>();
        assert!(!receipt.feedback_observed);
        assert!(!receipt.feedback_this_frame);
    }
}
