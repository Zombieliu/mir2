//! Offscreen native UI fixture. It creates no window or network connection.
//! The QR encodes only a reserved .invalid URL; it cannot approve a login.
//! PNG/JSON files are written only when MIR2_NATIVE_WALLET_VISUAL_OUT is set.

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    image::ImageSampler,
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{Extent3d, PollType, TextureDimension, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    text::TextLayoutInfo,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use mir2_client_bevy::{
    crystal_ui::login::CrystalLoginAction,
    native_i18n::{self, Locale, NativeI18nPlugin},
    native_login_locale::NativeUiLocale,
    native_shell::{NativeShellModel, NativeShellScreen},
    native_shell_ui::Mir2NativeShellUiPlugin,
    native_wallet_login::{
        NativeWalletLogin, NativeWalletLoginAction, NativeWalletLoginMessage,
        NativeWalletLoginStage,
    },
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const FIXTURE_CAPTION: &str =
    "offscreen native UI fixture · dummy approval QR · no wallet authentication";

const FIXTURE_QR_PAYLOAD: &str = "https://wallet.invalid/approve?fixture=mir2-native-ui";

#[derive(Resource, Default)]
struct CapturedImage(Option<Image>);

fn fixture_qr() -> Image {
    const SIDE: usize = 240;
    let qr = qrcode::QrCode::with_error_correction_level(FIXTURE_QR_PAYLOAD, qrcode::EcLevel::M)
        .expect("deterministic reserved-domain approval fixture");
    let modules = qr.width();
    let scale = SIDE / (modules + 8);
    assert!(scale > 0);
    let offset = (SIDE - modules * scale) / 2;
    let mut rgba = vec![255; SIDE * SIDE * 4];
    for row in 0..modules {
        for column in 0..modules {
            if qr[(column, row)] == qrcode::Color::Dark {
                for y in offset + row * scale..offset + (row + 1) * scale {
                    for x in offset + column * scale..offset + (column + 1) * scale {
                        rgba[(y * SIDE + x) * 4..(y * SIDE + x) * 4 + 3].fill(0);
                    }
                }
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIDE as u32,
            height: SIDE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn offscreen_app() -> (App, Handle<Image>, Entity) {
    let mut app = App::new();
    let public = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/public");
    app.add_plugins(
        DefaultPlugins
            .set(bevy::asset::AssetPlugin {
                file_path: public.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
    );
    app.add_plugins((Mir2NativeShellUiPlugin, NativeI18nPlugin))
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::Login,
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb(0.08, 0.07, 0.05)))
        .init_resource::<CapturedImage>();
    crate::native_fonts::install(&mut app);
    let started = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "offscreen GPU initialization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut texture = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Bgra8UnormSrgb, None);
    texture.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(texture);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
        ))
        .id();
    app.world_mut().spawn((
        UiTargetCamera(camera),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(732.0),
            width: Val::Px(992.0),
            ..default()
        },
        GlobalZIndex(20_000),
        Text::new(FIXTURE_CAPTION),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
    (app, target, camera)
}

fn step(app: &mut App) {
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::Poll)
        .expect("offscreen GPU polling");
    std::thread::sleep(Duration::from_millis(5));
}

fn target_roots(app: &mut App, camera: Entity) {
    let roots = app
        .world_mut()
        .query_filtered::<Entity, (With<Node>, Without<ChildOf>)>()
        .iter(app.world())
        .collect::<Vec<_>>();
    for entity in roots {
        app.world_mut()
            .entity_mut(entity)
            .insert(UiTargetCamera(camera));
    }
}

fn readback(app: &mut App, target: &Handle<Image>) -> Image {
    app.world_mut().resource_mut::<CapturedImage>().0 = None;
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |captured: On<ScreenshotCaptured>, mut output: ResMut<CapturedImage>| {
                output.0 = Some(captured.image.clone());
            },
        );
    let started = Instant::now();
    while app.world().resource::<CapturedImage>().0.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "offscreen screenshot timed out"
        );
        step(app);
    }
    app.world_mut()
        .resource_mut::<CapturedImage>()
        .0
        .take()
        .unwrap()
}

fn verify_text(app: &mut App, expected: &str) {
    let world = app.world_mut();
    let (_, computed, layout) = world
        .query::<(&Text, &ComputedNode, &TextLayoutInfo)>()
        .iter(world)
        .find(|(text, _, _)| text.0 == expected)
        .unwrap_or_else(|| panic!("native text absent: {expected}"));
    assert!(
        !layout.glyphs.is_empty(),
        "native text must rasterize: {expected}"
    );
    assert!(computed.size.x > 0.0 && computed.size.y > 0.0);
    assert!(
        layout.size.x <= computed.size.x + 1.0 && layout.size.y <= computed.size.y + 1.0,
        "native text overflow: {expected}"
    );
}

fn verify_text_absent(app: &mut App, unexpected: &str) {
    let world = app.world_mut();
    assert!(
        !world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == unexpected),
        "stale native language text remains: {unexpected}"
    );
}

fn settle_ui(app: &mut App, camera: Entity) {
    step(app);
    target_roots(app, camera);
    for _ in 0..24 {
        step(app);
    }
}

fn qr_rect(app: &mut App, qr: &Handle<Image>) -> Rect {
    let world = app.world_mut();
    let matches = world
        .query::<(&ImageNode, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .filter(|(image, _, _)| image.image == *qr)
        .map(|(_, node, transform)| {
            assert_eq!(node.size(), Vec2::splat(240.0));
            Rect::from_center_size(transform.affine().translation, node.size())
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "pending fixture must render one QR image");
    matches[0]
}

fn assert_no_windows(app: &mut App) {
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0,
        "offscreen fixture must never create a window"
    );
}

#[test]
fn wallet_visual_fixture_has_a_white_quiet_zone_and_no_authentication_data() {
    let image = fixture_qr();
    assert_eq!(image.width(), 240);
    assert_eq!(image.height(), 240);
    let data = image.data.as_ref().unwrap();
    assert!(data[..240 * 4 * 20].iter().all(|byte| *byte == 255));
    assert!(data.chunks_exact(4).any(|pixel| pixel == [0, 0, 0, 255]));
    assert!(!NativeWalletLogin::default().visible);
}

#[test]
#[ignore = "explicit bounded offscreen GPU fixture; run alone with --test-threads=1; no window or connection"]
fn native_wallet_login_offscreen_fixture() {
    struct RestoreLocale(Locale);
    impl Drop for RestoreLocale {
        fn drop(&mut self) {
            native_i18n::activate(self.0);
        }
    }
    let _restore_locale = RestoreLocale(native_i18n::locale());
    native_i18n::activate(Locale::English);
    let output = std::env::var_os("MIR2_NATIVE_WALLET_VISUAL_OUT").map(PathBuf::from);
    if let Some(output) = &output {
        std::fs::create_dir_all(output).unwrap();
    }
    let (mut app, target, camera) = offscreen_app();
    settle_ui(&mut app, camera);
    assert_no_windows(&mut app);
    assert!(app
        .world_mut()
        .query::<&CrystalLoginAction>()
        .iter(app.world())
        .any(|action| *action == CrystalLoginAction::DubheWallet));
    verify_text(&mut app, "Dubhe Wallet QR login  [F8]");
    verify_text(&mut app, FIXTURE_CAPTION);
    let english_entry = readback(&mut app, &target)
        .try_into_dynamic()
        .unwrap()
        .to_rgba8();
    assert_eq!(english_entry.dimensions(), (WIDTH, HEIGHT));
    if let Some(output) = &output {
        english_entry
            .save(output.join("01-wallet-entry-en-offscreen-fixture.png"))
            .unwrap();
    }
    native_i18n::activate(Locale::TraditionalChinese);
    settle_ui(&mut app, camera);
    verify_text(&mut app, "Dubhe Wallet 掃碼登入  [F8]");
    verify_text_absent(&mut app, "Dubhe Wallet QR login  [F8]");
    let chinese_entry = readback(&mut app, &target)
        .try_into_dynamic()
        .unwrap()
        .to_rgba8();
    assert_ne!(english_entry.as_raw(), chinese_entry.as_raw());
    if let Some(output) = &output {
        chinese_entry
            .save(output.join("02-wallet-entry-zh-TW-offscreen-fixture.png"))
            .unwrap();
    }
    let qr = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(fixture_qr());
    *app.world_mut().resource_mut::<NativeWalletLogin>() = NativeWalletLogin {
        visible: true,
        stage: NativeWalletLoginStage::Pending,
        qr_image: Some(qr.clone()),
        remaining_seconds: 120,
        message: NativeWalletLoginMessage::ConfirmOnPhone,
    };
    settle_ui(&mut app, camera);
    verify_text(&mut app, "Dubhe Wallet 掃碼登入");
    verify_text(&mut app, "等待手機掃碼確認");
    verify_text(&mut app, "二維碼有效期：120 秒");
    verify_text(&mut app, "取消 [Esc]");
    verify_text(&mut app, "請在手機錢包中確認登入。");
    assert_no_windows(&mut app);
    assert!(app
        .world_mut()
        .query::<&NativeWalletLoginAction>()
        .iter(app.world())
        .any(|action| *action == NativeWalletLoginAction::Cancel));
    let chinese_qr_rect = qr_rect(&mut app, &qr);
    let chinese_pending = readback(&mut app, &target)
        .try_into_dynamic()
        .unwrap()
        .to_rgba8();
    assert_eq!(chinese_pending.dimensions(), (WIDTH, HEIGHT));
    assert!(
        chinese_pending
            .pixels()
            .filter(|pixel| pixel[0] > 245 && pixel[1] > 245 && pixel[2] > 245)
            .count()
            > 20_000,
        "real offscreen render must contain the white QR quiet zone"
    );
    assert_ne!(
        chinese_entry.as_raw(),
        chinese_pending.as_raw(),
        "native modal must change rendered pixels"
    );
    if let Some(output) = &output {
        chinese_pending
            .save(output.join("03-wallet-pending-zh-TW-offscreen-fixture.png"))
            .unwrap();
    }

    native_i18n::activate(Locale::English);
    settle_ui(&mut app, camera);
    verify_text(&mut app, "Dubhe Wallet QR login");
    verify_text(&mut app, "Waiting for phone approval");
    verify_text(&mut app, "Expires in 120s");
    verify_text(&mut app, "Cancel [Esc]");
    verify_text(&mut app, "Confirm your login in the wallet on your phone.");
    verify_text_absent(&mut app, "Dubhe Wallet 掃碼登入");
    verify_text_absent(&mut app, "二維碼有效期：120 秒");
    assert_no_windows(&mut app);
    let wallet = app.world().resource::<NativeWalletLogin>();
    assert!(wallet.visible);
    assert_eq!(wallet.stage, NativeWalletLoginStage::Pending);
    assert_eq!(wallet.qr_image.as_ref(), Some(&qr));
    assert_eq!(wallet.remaining_seconds, 120);
    assert_eq!(wallet.message, NativeWalletLoginMessage::ConfirmOnPhone);
    let english_qr_rect = qr_rect(&mut app, &qr);
    assert_eq!(
        chinese_qr_rect, english_qr_rect,
        "changing language must preserve the QR position and size"
    );
    let english_pending = readback(&mut app, &target)
        .try_into_dynamic()
        .unwrap()
        .to_rgba8();
    assert_eq!(english_pending.dimensions(), (WIDTH, HEIGHT));
    assert_ne!(chinese_pending.as_raw(), english_pending.as_raw());
    let qr_min = chinese_qr_rect.min.as_uvec2();
    let qr_max = chinese_qr_rect.max.as_uvec2();
    for y in qr_min.y..qr_max.y {
        for x in qr_min.x..qr_max.x {
            assert_eq!(
                chinese_pending.get_pixel(x, y),
                english_pending.get_pixel(x, y),
                "changing language must not change QR pixels at ({x}, {y})"
            );
        }
    }
    if let Some(output) = &output {
        english_pending
            .save(output.join("04-wallet-pending-en-offscreen-fixture.png"))
            .unwrap();
    }

    let mut smoke_frames = Vec::new();
    for (index, language) in Locale::ALL.into_iter().enumerate() {
        native_i18n::activate(language);
        settle_ui(&mut app, camera);
        let locale = NativeUiLocale(language);
        for expected in [
            locale.wallet_title(),
            locale.wallet_stage(NativeWalletLoginStage::Pending),
            locale.wallet_countdown(120),
            locale.wallet_cancel(),
            locale.message(NativeWalletLoginMessage::ConfirmOnPhone),
        ] {
            verify_text(&mut app, &expected);
        }
        assert_no_windows(&mut app);
        let wallet = app.world().resource::<NativeWalletLogin>();
        assert!(wallet.visible);
        assert_eq!(wallet.stage, NativeWalletLoginStage::Pending);
        assert_eq!(wallet.qr_image.as_ref(), Some(&qr));
        assert_eq!(wallet.remaining_seconds, 120);
        assert_eq!(wallet.message, NativeWalletLoginMessage::ConfirmOnPhone);
        // All locales keep the same image. Their different line wrapping may
        // move the dialog, so cross-locale bounds equality is only required by
        // the dedicated English/Traditional Chinese comparison above.
        let bounds = qr_rect(&mut app, &qr);
        let frame = readback(&mut app, &target)
            .try_into_dynamic()
            .unwrap()
            .to_rgba8();
        assert_eq!(frame.dimensions(), (WIDTH, HEIGHT));
        let code = language.code();
        assert!(code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        let filename = format!(
            "{:02}-wallet-pending-{code}-nine-language-offscreen-fixture.png",
            index + 5
        );
        if let Some(output) = &output {
            frame.save(output.join(&filename)).unwrap();
        }
        smoke_frames.push(serde_json::json!({
            "language": code,
            "file": filename,
            "windowCount": 0,
            "qrRect": { "min": bounds.min.to_array(), "max": bounds.max.to_array() },
            "walletAuthenticated": false,
        }));
    }
    assert_eq!(smoke_frames.len(), 9);
    if let Some(output) = &output {
        std::fs::write(output.join("wallet-offscreen-fixture.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "kind": "offscreen native UI fixture", "liveAcceptance": false,
            "walletAuthenticated": false, "windowCount": 0, "width": WIDTH, "height": HEIGHT,
            "qrPayload": FIXTURE_QR_PAYLOAD,
            "languages": ["en", "zh-TW"],
            "nineLanguageSmoke": smoke_frames,
            "qrRect": { "min": chinese_qr_rect.min.to_array(), "max": chinese_qr_rect.max.to_array() },
            "verified": ["native entry button in English and Chinese", "pending modal in English and Chinese", "countdown", "cancel label", "locale switch preserves pending state and QR handle", "English/Traditional Chinese locale switch preserves QR bounds and pixels", "nine-language pending text glyphs and layout", "nine-language GPU readback"]
        })).unwrap()).unwrap();
        eprintln!("offscreen native UI fixture saved to {}", output.display());
    }
}
