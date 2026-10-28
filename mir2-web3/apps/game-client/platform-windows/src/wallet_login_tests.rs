use super::*;
use crate::{gateway::GatewayCommand, native_protocol::NativeOutboundCommand};

const ACCOUNT: &str = "obl_00112233445566778899aabbccddeeff";
const APPROVAL_URL: &str = "https://wallet.dubhe.obelisk.build/connect/approve?session=test-session&relay=https%3A%2F%2Frelay.example.test";

fn app_with_commands(
    commands: GatewayCommands,
) -> (App, SyncSender<WorkerEvent>, Receiver<WalletJob>) {
    let (event_tx, event_rx) = mpsc::sync_channel(8);
    let (job_tx, job_rx) = mpsc::sync_channel(1);
    let mut shell = NativeShellModel::default();
    shell.screen = NativeShellScreen::Login;
    shell.login.password = "previous-password".into();
    let mut app = App::new();
    app.insert_resource(commands)
        .insert_resource(shell)
        .init_resource::<NativeWalletLogin>()
        .init_resource::<NativeWalletLoginActionQueue>()
        .init_resource::<Assets<Image>>()
        .insert_resource(WalletLoginRuntime {
            jobs: Some(job_tx),
            events: Mutex::new(event_rx),
            active: None,
            qr_image: None,
            next_id: Some(1),
            qa_start_once: false,
        })
        .add_systems(PostUpdate, drive_wallet_login);
    (app, event_tx, job_rx)
}

fn fixture() -> (
    App,
    SyncSender<WorkerEvent>,
    Receiver<WalletJob>,
    Receiver<GatewayCommand>,
) {
    let (sender, receiver) = mpsc::channel::<GatewayCommand>();
    let (app, events, jobs) = app_with_commands(GatewayCommands::new(sender));
    (app, events, jobs, receiver)
}

fn action(app: &mut App, action: NativeWalletLoginAction) {
    app.world_mut()
        .resource_mut::<NativeWalletLoginActionQueue>()
        .push(action);
    app.update();
}

fn created(events: &SyncSender<WorkerEvent>, id: u64) {
    assert!(events
        .try_send(WorkerEvent {
            id,
            outcome: WorkerOutcome::Created {
                approval_url: APPROVAL_URL.into(),
                expires_at_ms: unix_ms() + 60_000,
            },
        })
        .is_ok());
}

fn approved(events: &SyncSender<WorkerEvent>, id: u64) {
    assert!(events
        .try_send(WorkerEvent {
            id,
            outcome: WorkerOutcome::Approved(VerifiedWalletLogin {
                account_id: ACCOUNT.into(),
                token: "private-gateway-token".into(),
                expires_at_ms: unix_ms() + 60_000,
            }),
        })
        .is_ok());
}

#[test]
fn native_wallet_login_ecs_cancel_discards_approval_and_removes_owned_qr() {
    let (mut app, events, jobs, sent) = fixture();
    action(&mut app, NativeWalletLoginAction::Begin);
    let job = jobs.try_recv().unwrap();
    created(&events, job.id);
    app.update();
    let image = app
        .world()
        .resource::<NativeWalletLogin>()
        .qr_image
        .clone()
        .unwrap();
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_some());
    // The UI clears visible immediately in Update, before the PostUpdate owner.
    app.world_mut().resource_mut::<NativeWalletLogin>().visible = false;
    approved(&events, job.id);
    action(&mut app, NativeWalletLoginAction::Cancel);
    assert!(job.cancel.load(Ordering::Acquire));
    assert!(!app.world().resource::<NativeWalletLogin>().visible);
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_none());
    assert!(sent.try_recv().is_err());
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        NativeShellScreen::Login
    );
}

#[test]
fn native_wallet_login_ecs_refresh_discards_old_results_and_submits_new_proof_once() {
    let (mut app, events, jobs, sent) = fixture();
    action(&mut app, NativeWalletLoginAction::Begin);
    let first = jobs.try_recv().unwrap();
    created(&events, first.id);
    app.update();
    let image = app
        .world()
        .resource::<NativeWalletLogin>()
        .qr_image
        .clone()
        .unwrap();
    assert!(events
        .try_send(WorkerEvent {
            id: first.id,
            outcome: WorkerOutcome::Rejected
        })
        .is_ok());
    app.update();
    assert!(app
        .world()
        .resource::<NativeWalletLogin>()
        .stage
        .can_refresh());
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_none());
    // Presentation replaces its view on Refresh. Host ownership still cleans up.
    *app.world_mut().resource_mut::<NativeWalletLogin>() = NativeWalletLogin {
        visible: true,
        ..default()
    };
    action(&mut app, NativeWalletLoginAction::Refresh);
    let second = jobs.try_recv().unwrap();
    assert_ne!(first.id, second.id);
    assert!(first.cancel.load(Ordering::Acquire));
    approved(&events, first.id);
    app.update();
    assert!(sent.try_recv().is_err());
    created(&events, second.id);
    app.update();
    let image = app
        .world()
        .resource::<NativeWalletLogin>()
        .qr_image
        .clone()
        .unwrap();
    approved(&events, second.id);
    app.update();
    let sent_commands = sent.try_iter().collect::<Vec<_>>();
    assert_eq!(sent_commands.len(), 2);
    assert!(matches!(
        &sent_commands[0],
        GatewayCommand::Wire(NativeOutboundCommand::ClientVersion)
    ));
    assert!(
        matches!(&sent_commands[1], GatewayCommand::Wire(NativeOutboundCommand::PasskeyLogin {
        account_id, token,
    }) if account_id == ACCOUNT && token == "private-gateway-token")
    );
    let shell = app.world().resource::<NativeShellModel>();
    assert_eq!(shell.screen, NativeShellScreen::Authenticating);
    assert!(shell.login.password.is_empty());
    assert!(!format!("{shell:?}").contains("private-gateway-token"));
    let view = app.world().resource::<NativeWalletLogin>();
    assert!(!view.visible);
    assert!(view.qr_image.is_none());
    assert!(!format!("{:?}", view.message).contains("private-gateway-token"));
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_none());
    approved(&events, second.id);
    app.update();
    assert!(
        sent.try_recv().is_err(),
        "an approved proof cannot be replayed"
    );
}

#[test]
fn native_wallet_login_ecs_screen_change_retires_challenge_before_approval() {
    for screen in [
        NativeShellScreen::Authenticating,
        NativeShellScreen::CharacterSelect,
        NativeShellScreen::ConnectionLost,
    ] {
        let (mut app, events, jobs, sent) = fixture();
        action(&mut app, NativeWalletLoginAction::Begin);
        let job = jobs.try_recv().unwrap();
        created(&events, job.id);
        app.update();
        let image = app
            .world()
            .resource::<NativeWalletLogin>()
            .qr_image
            .clone()
            .unwrap();
        app.world_mut().resource_mut::<NativeShellModel>().screen = screen;
        approved(&events, job.id);
        app.update();
        assert!(job.cancel.load(Ordering::Acquire));
        assert!(app
            .world()
            .resource::<Assets<Image>>()
            .get(image.id())
            .is_none());
        assert!(!app.world().resource::<NativeWalletLogin>().visible);
        assert!(sent.try_recv().is_err());
        assert_eq!(app.world().resource::<NativeShellModel>().screen, screen);
    }
}

#[test]
fn native_wallet_login_ecs_exact_connection_stamp_change_retires_challenge() {
    let (sender, mut sent) = crate::gateway::command_channel(8);
    let fence = sender.ownership_fence().unwrap();
    fence.test_world_ready(7, 0);
    let (mut app, events, jobs) = app_with_commands(GatewayCommands::new(sender));
    action(&mut app, NativeWalletLoginAction::Begin);
    let job = jobs.try_recv().unwrap();
    created(&events, job.id);
    app.update();
    let image = app
        .world()
        .resource::<NativeWalletLogin>()
        .qr_image
        .clone()
        .unwrap();
    fence.test_reconnect(7, 0);
    approved(&events, job.id);
    app.update();
    assert!(job.cancel.load(Ordering::Acquire));
    assert!(sent.try_recv_for_test().is_err());
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_none());
    assert!(!app.world().resource::<NativeWalletLogin>().visible);
}

#[test]
fn native_wallet_login_ecs_countdown_expires_using_monotonic_lifetime() {
    let (mut app, events, jobs, sent) = fixture();
    action(&mut app, NativeWalletLoginAction::Begin);
    let job = jobs.try_recv().unwrap();
    created(&events, job.id);
    app.update();
    let image = app
        .world()
        .resource::<NativeWalletLogin>()
        .qr_image
        .clone()
        .unwrap();
    app.world_mut()
        .resource_mut::<WalletLoginRuntime>()
        .active
        .as_mut()
        .unwrap()
        .expires_at = Some(Instant::now() - Duration::from_secs(1));
    approved(&events, job.id);
    app.update();
    let view = app.world().resource::<NativeWalletLogin>();
    assert!(view.visible);
    assert_eq!(view.stage, NativeWalletLoginStage::Expired);
    assert_eq!(view.remaining_seconds, 0);
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_none());
    assert!(sent.try_recv().is_err());
}

#[test]
fn native_wallet_login_ecs_failed_auth_admission_clears_view_without_password_fallback() {
    let (mut app, events, jobs, sent) = fixture();
    action(&mut app, NativeWalletLoginAction::Begin);
    let job = jobs.try_recv().unwrap();
    created(&events, job.id);
    app.update();
    let image = app
        .world()
        .resource::<NativeWalletLogin>()
        .qr_image
        .clone()
        .unwrap();
    drop(sent);
    approved(&events, job.id);
    app.update();
    let shell = app.world().resource::<NativeShellModel>();
    assert_eq!(shell.screen, NativeShellScreen::Login);
    assert!(!shell.login_request_in_flight);
    assert!(shell.login.password.is_empty());
    assert!(!app.world().resource::<NativeWalletLogin>().visible);
    assert!(app
        .world()
        .resource::<Assets<Image>>()
        .get(image.id())
        .is_none());
}

#[test]
fn native_wallet_login_qr_has_exact_matrix_quiet_zone_and_nearest_sampling() {
    let image = qr_image(APPROVAL_URL).unwrap();
    let code = QrCode::new(APPROVAL_URL.as_bytes()).unwrap();
    let modules = code.width();
    let side = modules + QR_QUIET_ZONE * 2;
    assert_eq!(image.texture_descriptor.size.width, side as u32);
    assert_eq!(image.texture_descriptor.size.height, side as u32);
    assert!(matches!(&image.sampler, ImageSampler::Descriptor(sampler)
        if sampler.min_filter == bevy::image::ImageFilterMode::Nearest
            && sampler.mag_filter == bevy::image::ImageFilterMode::Nearest));
    let pixels = image.data.as_ref().unwrap();
    for y in 0..side {
        for x in 0..side {
            let offset = (y * side + x) * 4;
            let expected = if x < QR_QUIET_ZONE
                || y < QR_QUIET_ZONE
                || x >= side - QR_QUIET_ZONE
                || y >= side - QR_QUIET_ZONE
            {
                255
            } else if code[(x - QR_QUIET_ZONE, y - QR_QUIET_ZONE)] == Color::Dark {
                0
            } else {
                255
            };
            assert_eq!(&pixels[offset..offset + 3], &[expected; 3]);
            assert_eq!(pixels[offset + 3], 255);
        }
    }
}

#[test]
fn native_wallet_login_qa_auto_begin_requires_both_explicit_gates() {
    assert!(qa_auto_start_enabled(Some("1"), Some("captures")));
    for (flag, directory) in [
        (None, Some("captures")),
        (Some("0"), Some("captures")),
        (Some("true"), Some("captures")),
        (Some("1"), None),
        (Some("1"), Some(" ")),
    ] {
        assert!(!qa_auto_start_enabled(flag, directory));
    }
}

#[test]
fn native_wallet_login_worker_wait_stops_immediately_when_cancelled() {
    assert!(!cancel_aware_wait(
        &AtomicBool::new(true),
        Duration::from_secs(60)
    ));
}
