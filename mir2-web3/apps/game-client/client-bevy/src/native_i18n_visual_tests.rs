//! Explicit, offline production UI fixture. Run this ignored test alone with
//! MIR2_I18N_VISUAL_ASSET_ROOT and a fresh MIR2_I18N_VISUAL_OUTPUT directory.
//! No window, connection, preference file, account or player save is created.

use super::*;
use crate::native_i18n::{self, Locale, NativeI18nPlugin};
use crate::native_shell::CharacterSummary;
use bevy::{
    asset::{AssetMetaCheck, AssetPlugin},
    camera::RenderTarget,
    image::ImagePlugin,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_asset::RenderAssets,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        texture::GpuImage,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    text::{ComputedTextBlock, Font, FontAtlasSet, FontCx, TextLayoutInfo},
    time::TimeUpdateStrategy,
    ui::{ComputedNode, IsDefaultUiCamera, UiGlobalTransform},
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;

#[derive(Resource, Default)]
struct CapturedI18nImage(Option<Image>);

#[derive(Resource, Default)]
struct FixtureFonts {
    handles: Vec<Handle<Font>>,
    retained: HashMap<u64, Font>,
}

fn retain_fixture_font_sources(
    blocks: Query<&ComputedTextBlock, Changed<ComputedTextBlock>>,
    mut fonts: ResMut<FixtureFonts>,
) {
    // Match the Windows host's stable source ownership without importing that
    // executable crate or reading its persisted settings.
    for block in &blocks {
        for line in block.buffer().lines() {
            for run in line.runs() {
                let data = &run.font().data;
                fonts.retained.entry(data.id()).or_insert_with(|| Font {
                    data: data.clone(),
                    alias: String::new(),
                });
            }
        }
    }
}

fn install_fixture_fonts(app: &mut App) {
    let font_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../platform-windows/assets/fonts");
    // Use exactly the native host's bundled files. Deliberately disable system
    // font discovery: this fixture must not pass because a developer installed
    // a language pack that a clean player machine does not have.
    app.world_mut().resource_mut::<FontCx>().collection =
        fontique::Collection::new(fontique::CollectionOptions {
            system_fonts: false,
            ..default()
        });
    let mut pinned = FixtureFonts::default();
    for file in [
        "NotoSansTC.ttf",
        "NotoSans-Regular.ttf",
        "NotoSans-Bold.ttf",
        "NotoSansDevanagari-Regular.ttf",
        "NotoSansDevanagari-Bold.ttf",
        "NotoSansThai-Regular.ttf",
        "NotoSansThai-Bold.ttf",
        "NotoSansArabic-Regular.ttf",
        "NotoSansArabic-Bold.ttf",
    ] {
        let bytes = fs::read(font_root.join(file)).expect("real pinned font must exist");
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(bytes));
        pinned.handles.push(handle);
    }
    app.world_mut()
        .resource_mut::<FontCx>()
        .source_cache
        .make_shared();
    app.insert_resource(pinned)
        .add_systems(
            PostUpdate,
            configure_fixture_script_fallbacks
                .after(bevy::text::load_font_assets_into_font_collection)
                .before(bevy::ui::UiSystems::Content)
                .before(bevy::sprite::update_text2d_layout),
        )
        .add_systems(
            PostUpdate,
            retain_fixture_font_sources.after(bevy::ui::UiSystems::PostLayout),
        );
}

fn configure_fixture_script_fallbacks(mut cx: ResMut<FontCx>) {
    // Match platform-windows/native_fonts.rs, including opaque names written in
    // a script different from the current interface language.
    for (tag, name) in [
        (*b"Latn", "Noto Sans"),
        (*b"Cyrl", "Noto Sans"),
        (*b"Deva", "Noto Sans Devanagari"),
        (*b"Thai", "Noto Sans Thai"),
        (*b"Arab", "Noto Sans Arabic"),
        (*b"Hani", "Noto Sans TC"),
    ] {
        let family = cx
            .collection
            .family_id(name)
            .expect("bundled fixture family registered");
        let script = fontique::Script::from_bytes(tag);
        if cx.collection.fallback_families(script).next() != Some(family) {
            assert!(cx.collection.set_fallbacks(script, std::iter::once(family)));
        }
        if tag == *b"Hani" {
            for locale in ["zh-TW", "zh-HK", "zh-CN", "ja", "ko"] {
                if cx.collection.fallback_families((script, locale)).next() != Some(family) {
                    assert!(cx
                        .collection
                        .set_fallbacks((script, locale), std::iter::once(family)));
                }
            }
        }
    }
    if cx.get_family(&FontSource::SansSerif) != Some("Noto Sans") {
        cx.set_sans_serif_family("Noto Sans").unwrap();
    }
    if cx.get_family(&FontSource::SystemUi) != Some("Noto Sans") {
        cx.set_system_ui_family("Noto Sans").unwrap();
    }
}

/// Shared only by explicit cfg(test) GPU fixtures. `shell=false` lets an
/// in-game fixture spawn production widgets without a shell covering them.
pub(crate) fn i18n_offscreen_app(root: &Path, shell: bool) -> (App, Handle<Image>, Entity) {
    i18n_offscreen_app_configured(root, shell, |_| {})
}

/// Register additional production widgets before the render plugins finish.
/// The default fixture retains the exact original initialization behavior.
pub(crate) fn i18n_offscreen_app_configured(
    root: &Path,
    shell: bool,
    configure: impl FnOnce(&mut App),
) -> (App, Handle<Image>, Entity) {
    assert!(root.is_absolute() && root.is_dir());
    assert!(root.join("original-ui/ChrSel/meta.json").is_file());
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: root.to_str().expect("UTF-8 asset path").to_owned(),
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .set(ImagePlugin::default_nearest())
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
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
    );
    app.add_plugins(NativeI18nPlugin);
    if shell {
        app.add_plugins(Mir2NativeShellUiPlugin);
    } else {
        crate::crystal_ui::overlays::register_crystal_additive_ui(&mut app);
    }
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.05, 0.03)))
        .init_resource::<CapturedI18nImage>();
    configure(&mut app);
    install_fixture_fonts(&mut app);
    let started = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "GPU initialization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Bgra8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
            IsDefaultUiCamera,
        ))
        .id();
    (app, target, camera)
}

pub(crate) fn i18n_step(app: &mut App) {
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::wait_indefinitely())
        .expect("offline GPU completion");
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0
    );
}

pub(crate) fn warm_i18n_images(app: &mut App) {
    let started = Instant::now();
    loop {
        i18n_step(app);
        let world = app.world_mut();
        let handles: Vec<_> = world
            .query::<&ImageNode>()
            .iter(world)
            .map(|node| node.image.clone())
            .collect();
        let assets = world.resource::<AssetServer>();
        let loaded = handles.iter().all(|handle| {
            // Procedural render targets are already in Assets; file assets
            // additionally need their dependencies loaded.
            world.resource::<Assets<Image>>().contains(handle.id())
                && (handle.path().is_none() || assets.is_loaded_with_dependencies(handle.id()))
        });
        let layers = crate::crystal_ui::select::preview_render_state_for_tests(world);
        if loaded && layers.iter().all(|row| row["allImagesReady"] == true) {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "real UI images did not load"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..8 {
        i18n_step(app);
    }
}

pub(crate) fn capture_i18n(app: &mut App, target: &Handle<Image>, path: &Path) {
    assert!(
        !path.exists(),
        "do not replace previous screenshot evidence"
    );
    app.world_mut().resource_mut::<CapturedI18nImage>().0 = None;
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |captured: On<ScreenshotCaptured>, mut output: ResMut<CapturedI18nImage>| {
                output.0 = Some(captured.image.clone());
            },
        );
    let started = Instant::now();
    while app.world().resource::<CapturedI18nImage>().0.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "offline screenshot timed out"
        );
        i18n_step(app);
        std::thread::sleep(Duration::from_millis(5));
    }
    let rgba = app
        .world_mut()
        .resource_mut::<CapturedI18nImage>()
        .0
        .take()
        .unwrap()
        .try_into_dynamic()
        .expect("GPU screenshot image")
        .to_rgba8();
    assert_eq!(rgba.dimensions(), (WIDTH, HEIGHT));
    rgba.save(path).expect("save offline GPU screenshot");
}

/// Only controlled fixture strings and presentation measurements, never a live
/// snapshot, account secret, inventory or socket payload.
pub(crate) fn i18n_text_layouts(app: &mut App) -> Vec<Value> {
    let mut rows = Vec::new();
    for (text, layout, node, transform, font, computed) in app
        .world_mut()
        .query::<(
            &Text,
            &TextLayoutInfo,
            &ComputedNode,
            &UiGlobalTransform,
            &TextFont,
            &ComputedTextBlock,
        )>()
        .iter(app.world())
    {
        // Display::None ancestors leave retained shell nodes unlaid-out. They
        // are not visible panels and must not create false overflow records.
        if text.trim().is_empty() || node.size == Vec2::ZERO {
            continue;
        }
        let center = transform.translation;
        let left = center.x - node.size.x / 2.0;
        let top = center.y - node.size.y / 2.0;
        let mut missing_glyphs = 0;
        for line in computed.buffer().lines() {
            for run in line.runs() {
                for cluster in run.clusters() {
                    missing_glyphs += cluster.glyphs().filter(|glyph| glyph.id == 0).count();
                }
            }
        }
        rows.push(json!({
            "text": text.0, "rect": [left, top, node.size.x, node.size.y],
            "layoutSize": [layout.size.x, layout.size.y],
            "glyphs": layout.glyphs.len(), "font": format!("{:?}", font.font),
            "missingGlyphs": missing_glyphs,
            "layoutExceedsNode": layout.size.x > node.size.x + 1.0 || layout.size.y > node.size.y + 1.0,
            "nodeOutsideViewport": left < -1.0 || top < -1.0
                || left + node.size.x > WIDTH as f32 + 1.0
                || top + node.size.y > HEIGHT as f32 + 1.0,
        }));
    }
    rows.sort_by(|a, b| a["text"].as_str().cmp(&b["text"].as_str()));
    rows
}

fn font_counts(app: &App) -> Value {
    let atlas = app.world().resource::<FontAtlasSet>();
    let images = app.world().resource::<Assets<Image>>();
    let retained = app.world().resource::<FixtureFonts>();
    json!({
        "registeredFontAssets": app.world().resource::<Assets<Font>>().len(),
        "pinnedHandles": retained.handles.len(),
        "retainedSourceIds": retained.retained.keys().copied().collect::<BTreeSet<_>>(),
        "atlasSourceIds": atlas.keys().map(|key| key.id).collect::<BTreeSet<_>>(),
        "atlasPages": atlas.values().map(Vec::len).sum::<usize>(),
        "atlasBytes": atlas.total_bytes(images),
        "imageAssets": images.len(),
        "gpuImages": app.sub_app(RenderApp).world().resource::<RenderAssets<GpuImage>>().iter().count(),
    })
}

fn capture_nine_language_popups(
    app: &mut App,
    target: &Handle<Image>,
    output: &Path,
) -> Vec<Value> {
    let mut cases = Vec::new();
    for locale in [Locale::English, Locale::Arabic] {
        set_screen(app, locale, NativeShellScreen::Login);
        native_i18n::set_language_popup_for_tests(app.world_mut(), true);
        warm_i18n_images(app);
        let layouts = i18n_text_layouts(app);
        for choice in Locale::ALL {
            let row = layouts
                .iter()
                .find(|row| row["text"] == choice.label())
                .expect("every language autonym must be visible in the open production popup");
            assert_eq!(row["missingGlyphs"], 0, "missing autonym glyph: {row}");
            assert!(row["glyphs"].as_u64().unwrap() > 0);
            assert!(
                !row["layoutExceedsNode"].as_bool().unwrap(),
                "autonym overflow: {row}"
            );
            assert!(
                !row["nodeOutsideViewport"].as_bool().unwrap(),
                "autonym outside viewport: {row}"
            );
        }
        let filename = format!("shell-{}-language-popup.png", locale.code());
        capture_i18n(app, target, &output.join(&filename));
        cases.push(
            json!({"locale":locale.code(),"image":filename,"visibleAutonyms":9,"texts":layouts}),
        );
        native_i18n::set_language_popup_for_tests(app.world_mut(), false);
        warm_i18n_images(app);
    }
    cases
}

fn fixture_model(screen: NativeShellScreen) -> NativeShellModel {
    let mut model = NativeShellModel::default();
    model.screen = screen;
    // These look like translatable UI words but must remain literal identities.
    model.login.account = "Gold".into();
    model.login.password = "OfflineP123".into();
    model.registration.account_id = "Cancel".into();
    model.registration.password = "OfflineP123".into();
    model.registration.confirm_password = "OfflineP123".into();
    model.character_create.name = "Gold".into();
    model.character_create.class_name = "Wizard".into();
    model.character_create.gender_name = "Male".into();
    model.characters = vec![
        CharacterSummary::new(1, "Gold", 7, "Wizard", "Male"),
        CharacterSummary::new(2, "Cancel", 8, "Taoist", "Female"),
    ];
    model.selected_character_index = Some(1);
    model
}

fn set_screen(app: &mut App, language: Locale, screen: NativeShellScreen) -> NativeShellModel {
    native_i18n::activate(language);
    let expected = fixture_model(screen);
    app.insert_resource(expected.clone());
    warm_i18n_images(app);
    assert_eq!(app.world().resource::<NativeShellModel>(), &expected);
    assert!(app.world().resource::<NativeUiIntentQueue>().is_empty());
    expected
}

#[derive(Component)]
struct ExtraPanelFixture;

fn capture_portuguese_panels(
    app: &mut App,
    target: &Handle<Image>,
    camera: Entity,
    assets_root: &Path,
    output: &Path,
) -> Vec<Value> {
    use crate::crystal_ui::overlays::{keyboard_dialog, ranking_dialog};
    let mut dimensions = HashMap::new();
    for library in ["Title", "Prguse", "Prguse2"] {
        let metadata: Value = serde_json::from_slice(
            &fs::read(assets_root.join(format!("original-ui/{library}/meta.json"))).unwrap(),
        )
        .unwrap();
        for frame in metadata["frames"].as_array().unwrap() {
            dimensions.insert(
                (library.to_owned(), frame["index"].as_u64().unwrap() as u16),
                Vec2::new(
                    frame["width"].as_f64().unwrap() as f32,
                    frame["height"].as_f64().unwrap() as f32,
                ),
            );
        }
    }
    native_i18n::activate(Locale::BrazilianPortuguese);
    app.insert_resource(fixture_model(NativeShellScreen::InGame));
    i18n_step(app);
    let mut cases = Vec::new();
    for name in ["keyboard", "ranking"] {
        let old: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<ExtraPanelFixture>>()
            .iter(app.world())
            .collect();
        for entity in old {
            app.world_mut().despawn(entity);
        }
        let assets = app.world().resource::<AssetServer>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world());
        let mut keyboard = keyboard_dialog::KeyboardDialogUi::default();
        keyboard.open = true;
        let bindings = keyboard.bindings.clone();
        let mut ranking = ranking_dialog::RankingDialogUi::default();
        ranking.open = true;
        ranking.count = 2;
        ranking.rows = vec![
            mir2_protocol::RankCharacterInfo {
                player_id: 1,
                name: "Gold".into(),
                level: 8,
                class: mir2_protocol::MirClass::Warrior,
            },
            mir2_protocol::RankCharacterInfo {
                player_id: 2,
                name: "Cancel".into(),
                level: 7,
                class: mir2_protocol::MirClass::Wizard,
            },
        ];
        commands
            .spawn((
                ExtraPanelFixture,
                UiTargetCamera(camera),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(WIDTH as f32),
                    height: Val::Px(HEIGHT as f32),
                    ..default()
                },
            ))
            .with_children(|parent| {
                let size =
                    |library: &str, index| dimensions.get(&(library.to_owned(), index)).copied();
                let rendered = if name == "keyboard" {
                    keyboard_dialog::view::render(
                        parent,
                        &assets,
                        &mut keyboard,
                        Vec2::new(WIDTH as f32, HEIGHT as f32),
                        size,
                    )
                } else {
                    ranking_dialog::render(
                        parent,
                        &assets,
                        &mut ranking,
                        Vec2::new(WIDTH as f32, HEIGHT as f32),
                        "Gold",
                        size,
                    )
                };
                assert!(rendered, "actual panel geometry must be available");
            });
        queue.apply(app.world_mut());
        assert_eq!(
            keyboard.bindings, bindings,
            "localization must not rewrite key bindings"
        );
        assert_eq!(ranking.rows[0].name, "Gold");
        assert_eq!(ranking.rows[1].name, "Cancel");
        assert_eq!(ranking.rows[1].class, mir2_protocol::MirClass::Wizard);
        warm_i18n_images(app);
        let layouts = i18n_text_layouts(app);
        for source in if name == "keyboard" {
            &["Function", "Default", "Current", "Assign Rule: Strict"][..]
        } else {
            &["Rank", "Name", "Class", "Level", "Not Listed"][..]
        } {
            let translated = native_i18n::tr(source);
            assert_ne!(
                translated, *source,
                "Portuguese catalog entry missing: {source}"
            );
            assert!(
                layouts.iter().any(|row| row["text"] == translated),
                "translated label was not rendered: {source}"
            );
        }
        if name == "ranking" {
            assert!(layouts.iter().any(|row| row["text"] == "Gold"));
            assert!(layouts.iter().any(|row| row["text"] == "Cancel"));
        }
        let filename = format!("panel-pt-BR-{name}.png");
        capture_i18n(app, target, &output.join(&filename));
        cases.push(json!({"locale":"pt-BR", "screen":name, "image":filename, "texts":layouts}));
    }
    cases
}

#[test]
#[ignore = "explicit real-asset offline GPU multilingual screenshots and bounded font switch checks; run alone"]
fn multilingual_shell_screens_and_font_switches_render_offscreen() {
    fn path_env(name: &str) -> PathBuf {
        let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")));
        assert!(path.is_absolute());
        path
    }
    let assets = path_env("MIR2_I18N_VISUAL_ASSET_ROOT")
        .canonicalize()
        .unwrap();
    let output = path_env("MIR2_I18N_VISUAL_OUTPUT");
    assert!(!output.starts_with(&assets));
    fs::create_dir_all(&output).unwrap();
    let output = output.canonicalize().unwrap();
    assert!(!output.starts_with(&assets));
    let report_path = output.join("shell-i18n-report.json");
    let _report = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report_path)
        .expect("use a fresh evidence directory");
    fs::write(&report_path, b"{\"passed\":false,\"complete\":false}").unwrap();
    native_i18n::activate(Locale::English);
    let (mut app, target, camera) = i18n_offscreen_app(&assets, true);
    let screens = [
        ("login", NativeShellScreen::Login, "Login", "Gold"),
        (
            "registration",
            NativeShellScreen::Registration,
            "New Account",
            "Cancel",
        ),
        (
            "create",
            NativeShellScreen::CharacterCreate,
            "Create Character",
            "Gold",
        ),
        (
            "select",
            NativeShellScreen::CharacterSelect,
            "Select Character",
            "Gold",
        ),
    ];
    let mut cases = Vec::new();
    // Warm every glyph/style twice before measuring growth. Capture production
    // screens on the second pass, preserving all language choices and previews.
    for pass in 0..2 {
        for language in Locale::ALL {
            for (name, screen, title, opaque_name) in &screens {
                set_screen(&mut app, language, *screen);
                let layouts = i18n_text_layouts(&mut app);
                assert!(layouts
                    .iter()
                    .any(|row| row["text"] == native_i18n::tr(title)));
                assert!(layouts.iter().any(|row| row["text"] == *opaque_name));
                assert!(layouts
                    .iter()
                    .all(|row| !row["text"].as_str().unwrap().contains("OfflineP123")));
                assert!(layouts
                    .iter()
                    .all(|row| row["glyphs"].as_u64().unwrap() > 0));
                assert!(
                    layouts.iter().all(|row| row["missingGlyphs"] == 0),
                    "missing glyph in {}/{}: {:?}",
                    language.code(),
                    name,
                    layouts
                        .iter()
                        .filter(|row| row["missingGlyphs"] != 0)
                        .collect::<Vec<_>>()
                );
                if *screen == NativeShellScreen::CharacterSelect {
                    assert!(layouts.iter().any(|row| row["text"] == "Cancel"));
                }
                if pass == 1 {
                    let filename = format!("shell-{}-{name}.png", language.code());
                    capture_i18n(&mut app, &target, &output.join(&filename));
                    cases.push(json!({"locale":language.code(),"screen":name,
                        "image":filename,"opaqueIdentity":opaque_name,"texts":layouts}));
                }
            }
        }
    }
    let popups = capture_nine_language_popups(&mut app, &target, &output);
    let baseline = font_counts(&app);
    assert!(baseline["atlasBytes"].as_u64().unwrap() > 0);
    assert_eq!(baseline["pinnedHandles"], 9);
    let mut checkpoints = Vec::new();
    // Thirty complete cycles: 270 language selections / 1080 production screen
    // rebuilds. This is a bounded regression, not a long-duration leak claim.
    for cycle in 0..30 {
        for language in Locale::ALL {
            for (_, screen, _, _) in &screens {
                set_screen(&mut app, language, *screen);
            }
        }
        let current = font_counts(&app);
        for field in [
            "registeredFontAssets",
            "pinnedHandles",
            "retainedSourceIds",
            "atlasSourceIds",
            "atlasPages",
            "atlasBytes",
        ] {
            assert_eq!(
                current[field], baseline[field],
                "font growth in cycle {cycle}: {field}"
            );
        }
        checkpoints.push(json!({"cycle":cycle+1,"counts":current}));
    }
    let extra_panels = capture_portuguese_panels(&mut app, &target, camera, &assets, &output);
    fs::write(&report_path, serde_json::to_vec_pretty(&json!({
        "kind":"offline_production_native_i18n_shell_gpu_fixture",
        "passed":true,"complete":true,"liveAcceptance":false,
        "windowCreated":false,"serverConnected":false,"playerSaveModified":false,
        "viewport":[WIDTH,HEIGHT],"screens":4,"locales":Locale::ALL.len(),
        "captures":cases.len()+extra_panels.len()+popups.len(),"systemFonts":false,
        "boundedSwitchCycles":30,"fontBaseline":baseline,"fontCheckpoints":checkpoints,
        "cases":cases, "extraPortuguesePanels":extra_panels,"languagePopups":popups,
        "limits":["Offline rendering does not validate live account or gameplay flows",
            "Bounded font identity and atlas checks do not replace a long-duration soak",
            "Text bounds are retained for independent screenshot review; input text remains literal"]
    })).unwrap()).unwrap();
}

#[derive(Component)]
struct GuildNameVisualRoot;

#[derive(Resource, Default)]
struct SabukVisualPress(Option<Entity>);

fn inject_sabuk_visual_press(
    mut press: ResMut<SabukVisualPress>,
    mut buttons: Query<&mut Interaction, With<Button>>,
) {
    if let Some(entity) = press.0.take() {
        *buttons
            .get_mut(entity)
            .expect("current enabled NPC control") = Interaction::Pressed;
    }
}

fn guild_name_visual_dimensions(root: &Path) -> HashMap<(String, u16), Vec2> {
    let mut dimensions = HashMap::new();
    for library in ["Title", "Prguse"] {
        let metadata: Value = serde_json::from_slice(
            &fs::read(root.join(format!("original-ui/{library}/meta.json"))).unwrap(),
        )
        .unwrap();
        for frame in metadata["frames"].as_array().unwrap() {
            dimensions.insert(
                (library.to_owned(), frame["index"].as_u64().unwrap() as u16),
                Vec2::new(
                    frame["width"].as_f64().unwrap() as f32,
                    frame["height"].as_f64().unwrap() as f32,
                ),
            );
        }
    }
    for (library, frames) in [("Prguse", vec![660]), ("Title", (200..=205).collect())] {
        for frame in frames {
            let size = dimensions.get(&(library.into(), frame)).unwrap();
            assert!(size.x > 0.0 && size.y > 0.0);
            assert!(root
                .join(format!("original-ui/{library}/{frame}.png"))
                .is_file());
        }
    }
    dimensions
}

fn spawn_guild_name_visual(
    app: &mut App,
    camera: Entity,
    model: &crate::crystal_ui::overlays::social_bond_dialog::SocialBondDialogs,
    dimensions: &HashMap<(String, u16), Vec2>,
) {
    use crate::crystal_ui::overlays::social_bond_dialog::view;
    let old = app
        .world_mut()
        .query_filtered::<Entity, With<GuildNameVisualRoot>>()
        .iter(app.world())
        .collect::<Vec<_>>();
    for entity in old {
        app.world_mut().despawn(entity);
    }
    let assets = app.world().resource::<AssetServer>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    commands
        .spawn((
            GuildNameVisualRoot,
            UiTargetCamera(camera),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            assert!(view::render_prompt(
                parent,
                &assets,
                model.prompt.as_ref().unwrap(),
                Vec2::new(WIDTH as f32, HEIGHT as f32),
                "Warrior",
                &model.input,
                |library, frame| dimensions.get(&(library.into(), frame)).copied(),
            ));
        });
    queue.apply(app.world_mut());
    warm_i18n_images(app);
}

fn render_guild_name_visual(
    app: &mut App,
    camera: Entity,
    model: &mut crate::crystal_ui::overlays::social_bond_dialog::SocialBondDialogs,
    dimensions: &HashMap<(String, u16), Vec2>,
) {
    use crate::crystal_ui::overlays::friend_dialog;
    // Capture the same Parley layout used by the Windows host before drawing
    // the real caret/IME underline. No estimated character widths are used.
    spawn_guild_name_visual(app, camera, model, dimensions);
    let world = app.world_mut();
    let mut query = world.query::<(
        &friend_dialog::view::FriendEditText,
        &ComputedTextBlock,
        &TextLayoutInfo,
    )>();
    let (tag, block, info) = query.single(world).expect("production name editor");
    friend_dialog::host::capture_layout(&mut model.input, tag, block, info);
    assert_eq!(
        model.input.layout_text,
        model.input.display_editor().unwrap().text()
    );
    spawn_guild_name_visual(app, camera, model, dimensions);
}

fn guild_name_visual_marks(
    app: &mut App,
    dimensions: &HashMap<(String, u16), Vec2>,
    composing: bool,
    target: &Handle<Image>,
    output: &Path,
) -> Value {
    use crate::crystal_ui::overlays::friend_dialog::view::FriendEditText;
    let frame = dimensions[&("Prguse".into(), 660)];
    let input = [
        ((WIDTH as f32 - frame.x) / 2.0).floor() + 23.0,
        ((HEIGHT as f32 - frame.y) / 2.0).floor() + 86.0,
        240.0,
        19.0,
    ];
    let world = app.world_mut();
    let mut editors = world.query::<(&FriendEditText, &ChildOf)>();
    let editor_parent = editors
        .single(world)
        .expect("one real production Guild name editor")
        .1
        .parent();
    let mut carets = Vec::new();
    let mut underlines = Vec::new();
    for (node, transform, color, parent) in world
        .query::<(
            &ComputedNode,
            &UiGlobalTransform,
            &BackgroundColor,
            &ChildOf,
        )>()
        .iter(world)
    {
        let min = transform.translation - node.size / 2.0;
        if color.0 != Color::WHITE || parent.parent() != editor_parent {
            continue;
        }
        let rect = [min.x, min.y, node.size.x, node.size.y];
        if node.size.x <= 1.1 && node.size.x > 0.0 && node.size.y > 3.0 {
            carets.push(rect);
        } else if node.size.y <= 1.1 && node.size.y > 0.0 && node.size.x > 1.1 {
            underlines.push(rect);
        }
    }
    // Retain every real editor mark before checking its bounds. A clipped mark
    // must be reported with its actual geometry, never discarded as "missing".
    // The production Guild editor has no blink phase, and the fixture already
    // uses a fixed zero-duration clock.
    let inside = |rect: &[f32; 4]| {
        rect.iter().all(|value| value.is_finite())
            && rect[0] >= input[0] - 1.0
            && rect[1] >= input[1] - 1.0
            && rect[0] + rect[2] <= input[0] + input[2] + 1.0
            && rect[1] + rect[3] <= input[1] + input[3] + 1.0
    };
    let in_bounds = carets.iter().chain(&underlines).all(inside);
    let evidence = json!({"editorRect":input,"caretRects":carets,
        "compositionUnderlines":underlines,"allMarksInsideEditor":in_bounds});
    let phase = if composing { "ime" } else { "committed" };
    let stem = format!("guild-{}-{phase}-marks", native_i18n::locale().code());
    let mut geometry_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join(format!("{stem}.json")))
        .expect("retain earlier caret geometry evidence");
    serde_json::to_writer_pretty(&mut geometry_file, &evidence).unwrap();
    if !in_bounds || carets.len() != 1 || !underlines.is_empty() != composing {
        capture_i18n(app, target, &output.join(format!("{stem}-invalid.png")));
    }
    assert_eq!(
        carets.len(),
        1,
        "one production caret must exist: {evidence}"
    );
    assert!(
        in_bounds,
        "production caret/IME marks must remain inside their editor: {evidence}"
    );
    assert_eq!(
        !underlines.is_empty(),
        composing,
        "production IME underline"
    );
    evidence
}

fn sabuk_visual_dialog(case: &str) -> crate::quest_model::NpcDialogModel {
    use crate::quest_model::{NpcDialogLine, NpcDialogModel, NpcDialogOption};
    // Controlled server-dialog presentation fixtures mirror the public page
    // strings/labels in runtime/npc_conquest.rs. They do not authorize a guild,
    // call a server command, or replace the ordinary CallNpc integration tests.
    let mut lines = vec![
        if matches!(case, "registrar-status" | "officer-status") {
            "Siege status".to_owned()
        } else {
            "Castle management".to_owned()
        },
        "Castle owner: Gold Dragon Knights".into(),
        "Registered attacker: Gold龍城Драконы".into(),
        "Siege schedule: daily 18:00-18:30 (UTC+8:00).".into(),
        "War in progress.".into(),
    ];
    let mut options = Vec::new();
    let option = |label: String, target: String| NpcDialogOption {
        option_id: target,
        label,
        enabled: true,
    };
    if !matches!(case, "registrar-status" | "officer-status") {
        lines.extend([
            "Tax rate: 25%".into(),
            "Tax treasury: 4294967295 gold".into(),
            "Guild bank: 4294967295 gold".into(),
        ]);
    }
    match case {
        "registrar-status" => {}
        "officer-status" => {
            options.push(option("Castle management".into(), "@sabuk:manage".into()))
        }
        "manage" => {
            for rate in [10, 15, 20, 25] {
                options.push(option(
                    format!("Set tax: {rate}%"),
                    format!("@sabuk:tax:{rate}"),
                ));
            }
            for (label, target) in [
                ("Open gate", "@sabuk:open"),
                ("Close gate", "@sabuk:close"),
                ("Repair defenses", "@sabuk:repairs"),
                ("Restore archers", "@sabuk:archers"),
                ("Withdraw taxes to guild bank", "@sabuk:withdraw"),
            ] {
                options.push(option(label.into(), target.into()));
            }
        }
        "repairs" => {
            lines.extend(["Gate 1: 5000/10000".into(), "Repair cost: 500 gold.".into()]);
            options.push(option("Repair gate".into(), "@sabuk:repair:gate:1".into()));
            for slot in 1..=3 {
                lines.extend([
                    format!("Wall {slot}: 5000/10000"),
                    "Repair cost: 500 gold.".into(),
                ]);
                options.push(option(
                    format!("Repair wall {slot}"),
                    format!("@sabuk:repair:wall:{slot}"),
                ));
            }
        }
        "archers" => {
            for slot in 1..=12 {
                lines.extend([
                    format!("Archer {slot}: 0/1000"),
                    "Repair cost: 1000 gold.".into(),
                ]);
                options.push(option(
                    format!("Restore archer {slot}"),
                    format!("@sabuk:repair:archer:{slot}"),
                ));
            }
        }
        _ => panic!("unknown public Sabuk presentation page"),
    }
    if matches!(case, "manage" | "repairs" | "archers") {
        options.push(option("Back".into(), "@sabuk:manage".into()));
    }
    options.push(option("Close".into(), "@Exit".into()));
    NpcDialogModel {
        is_open: true,
        npc_object_id: Some(if case == "registrar-status" {
            427
        } else {
            1146
        }),
        npc_name: Some(
            if case == "registrar-status" {
                "Administrator"
            } else {
                "Conquest"
            }
            .into(),
        ),
        lines: lines
            .into_iter()
            .map(|text| NpcDialogLine { text })
            .collect(),
        options,
    }
}

fn record_sabuk_visual(
    app: &mut App,
    target: &Handle<Image>,
    output: &Path,
    filename: &str,
    metadata: Value,
    failures: &mut Vec<Value>,
) -> Value {
    let rows = i18n_text_layouts(app);
    assert!(!rows.is_empty(), "real production text must be laid out");
    failures.extend(
        rows.iter()
            .filter(|row| {
                row["glyphs"] == 0
                    || row["missingGlyphs"] != 0
                    || row["layoutExceedsNode"] != false
                    || row["nodeOutsideViewport"] != false
            })
            .map(|row| json!({"image":filename,"locale":native_i18n::locale().code(),"row":row})),
    );
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0
    );
    capture_i18n(app, target, &output.join(filename));
    json!({"image":filename,"locale":native_i18n::locale().code(),"metadata":metadata,"textRows":rows})
}

#[test]
#[ignore = "explicit production Guild name and Sabuk NPC offline GPU fixture; run alone with real assets and fresh evidence output"]
fn guild_name_and_sabuk_npc_pages_render_offscreen() {
    use crate::crystal_ui::overlays::{
        social_bond_dialog::{GuildCreationContext, SocialBondDialogs},
        text_input::Composition,
    };
    use crate::quest_model::NpcDialogModel;
    use crate::quest_ui::{Mir2QuestUiPlugin, QuestUiIntentQueue, QuestUiState};
    use mir2_protocol::{ClientPacket, ServerPacket};
    let assets = PathBuf::from(
        std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("explicit real packaged assets"),
    );
    let output = PathBuf::from(
        std::env::var_os("MIR2_SABUK_I18N_VISUAL_OUTPUT")
            .expect("explicit fresh Sabuk evidence output"),
    );
    assert!(assets.is_absolute() && output.is_absolute());
    let assets = assets.canonicalize().unwrap();
    assert!(!output.starts_with(&assets));
    fs::create_dir_all(&output).unwrap();
    let output = output.canonicalize().unwrap();
    assert!(!output.starts_with(&assets));
    let report_path = output.join("sabuk-guild-i18n-layouts.json");
    let _reservation = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report_path)
        .expect("retain earlier screenshot evidence");
    fs::write(&report_path, b"{\"passed\":false,\"complete\":false}").unwrap();
    struct RestoreLocale(Locale);
    impl Drop for RestoreLocale {
        fn drop(&mut self) {
            native_i18n::activate(self.0);
        }
    }
    let _restore = RestoreLocale(native_i18n::locale());
    let dimensions = guild_name_visual_dimensions(&assets);
    let (mut app, target, camera) = i18n_offscreen_app_configured(&assets, false, |app| {
        app.add_plugins(Mir2QuestUiPlugin)
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .init_resource::<SabukVisualPress>()
            .add_systems(
                PreUpdate,
                inject_sabuk_visual_press.after(bevy::ui::UiSystems::Focus),
            );
    });
    let mut screenshots = Vec::new();
    let mut failures = Vec::new();
    for locale in Locale::ALL {
        native_i18n::activate(locale);
        app.insert_resource(NpcDialogModel::default());
        let composition = match locale {
            Locale::English => "Guild",
            Locale::TraditionalChinese => "龍城",
            Locale::BrazilianPortuguese => "Dragões",
            Locale::Russian => "Драконы",
            Locale::Hindi => "वीर",
            Locale::Indonesian => "Naga",
            Locale::Vietnamese => "Rồng",
            Locale::Thai => "มังกร",
            Locale::Arabic => "فرسان",
        };
        let mut model = SocialBondDialogs::default();
        assert!(model.begin_guild_creation_request(GuildCreationContext {
            npc_object_id: 427,
            map_epoch: 1
        }));
        assert!(model.observe(&ServerPacket::GuildNameRequest));
        let revision = model.prompt.as_ref().unwrap().revision;
        model.input_name(revision, "Gold");
        model.input.composition = Some(Composition {
            value: composition.into(),
            cursor: Some((composition.len(), composition.len())),
        });
        render_guild_name_visual(&mut app, camera, &mut model, &dimensions);
        assert_eq!(
            model.input.editor.as_ref().unwrap().text(),
            "Gold",
            "IME preedit must not commit a name"
        );
        let expected = format!("Gold{composition}");
        let rows = i18n_text_layouts(&mut app);
        assert!(rows.iter().any(|row| row["text"] == expected));
        assert!(rows
            .iter()
            .any(|row| row["text"] == model.prompt.as_ref().unwrap().text("Warrior")));
        let marks = guild_name_visual_marks(&mut app, &dimensions, true, &target, &output);
        screenshots.push(record_sabuk_visual(&mut app, &target, &output, &format!("guild-{}-ime.png",locale.code()),
            json!({"case":"guild-name-ime","committedDraft":"Gold","preedit":composition,"displayedName":expected,"marks":marks}), &mut failures));
        assert_eq!(
            model.input.editor.as_ref().unwrap().text(),
            "Gold",
            "GPU capture cannot consume the draft"
        );
        model.input.composition = None;
        model.input.paste(composition);
        model.sync_draft();
        render_guild_name_visual(&mut app, camera, &mut model, &dimensions);
        let marks = guild_name_visual_marks(&mut app, &dimensions, false, &target, &output);
        screenshots.push(record_sabuk_visual(
            &mut app,
            &target,
            &output,
            &format!("guild-{}-committed.png", locale.code()),
            json!({"case":"guild-name-committed","opaqueName":expected,"marks":marks}),
            &mut failures,
        ));
        assert!(
            matches!(model.answer(revision, true), Some(ClientPacket::GuildNameReturn { name }) if name == expected)
        );
        let roots = app
            .world_mut()
            .query_filtered::<Entity, With<GuildNameVisualRoot>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        for root in roots {
            app.world_mut().despawn(root);
        }

        for case in [
            "registrar-status",
            "officer-status",
            "manage",
            "repairs",
            "archers",
        ] {
            let dialog = sabuk_visual_dialog(case);
            let page_count_before = screenshots.len();
            app.insert_resource(dialog.clone());
            app.world_mut()
                .resource_mut::<QuestUiState>()
                .dialog_scroll_top = 0;
            let mut closed_visible = false;
            let mut body_rows = Vec::new();
            for page in 0..16 {
                warm_i18n_images(&mut app);
                let rows = i18n_text_layouts(&mut app);
                let mut visible_body = rows
                    .iter()
                    .filter(|row| {
                        let rect = row["rect"].as_array().unwrap();
                        let (x, y) = (rect[0].as_f64().unwrap(), rect[1].as_f64().unwrap());
                        x >= 17.0 && x < 409.0 && y >= 30.0 && y < 185.0
                    })
                    .collect::<Vec<_>>();
                assert!(!visible_body.is_empty());
                // The layout evidence helper sorts by text for stable reports.
                // Reassemble NPC body rows in their real on-screen order so a
                // normally wrapped opaque name is checked across adjacent rows.
                visible_body.sort_by(|a, b| {
                    a["rect"][1]
                        .as_f64()
                        .unwrap()
                        .total_cmp(&b["rect"][1].as_f64().unwrap())
                        .then_with(|| {
                            a["rect"][0]
                                .as_f64()
                                .unwrap()
                                .total_cmp(&b["rect"][0].as_f64().unwrap())
                        })
                });
                body_rows.extend(
                    visible_body
                        .iter()
                        .map(|row| row["text"].as_str().unwrap().to_owned()),
                );
                let scroll_top = app.world().resource::<QuestUiState>().dialog_scroll_top;
                screenshots.push(record_sabuk_visual(&mut app, &target, &output,
                    &format!("sabuk-{}-{case}-page-{page}.png",locale.code()),
                    json!({"case":case,"npcId":dialog.npc_object_id,"serverFixtureLines":dialog.lines,"serverFixtureOptions":dialog.options,"scrollTop":scroll_top}), &mut failures));
                if visible_body
                    .iter()
                    .any(|row| row["text"] == native_i18n::npc_text("Close"))
                {
                    closed_visible = true;
                    break;
                }
                let world = app.world_mut();
                let mut texts = world.query::<(&Text, &ChildOf)>();
                let next = texts
                    .iter(world)
                    .find_map(|(text, parent)| {
                        (text.0 == "v" && world.get::<Button>(parent.parent()).is_some())
                            .then_some(parent.parent())
                    })
                    .expect("ordinary NPC page must expose its enabled scroll-down button");
                world.resource_mut::<SabukVisualPress>().0 = Some(next);
                i18n_step(&mut app);
                assert!(
                    app.world().resource::<QuestUiState>().dialog_scroll_top > scroll_top,
                    "actual public scroll control must advance"
                );
            }
            assert!(
                closed_visible,
                "ordinary NPC Close link must be reachable in {case}/{}",
                locale.code()
            );
            let body_text = body_rows.join(" ");
            for prefix in ["Castle owner: ", "Registered attacker: "] {
                let source = dialog
                    .lines
                    .iter()
                    .find(|line| line.text.starts_with(prefix))
                    .expect("each owned Sabuk fixture includes its owner and attacker");
                let expected = native_i18n::npc_text(&source.text);
                assert!(
                    body_text.contains(&expected),
                    "complete localized caption and opaque name preserved in {case}/{}: expected {expected:?}, rendered rows {body_rows:?}",
                    locale.code()
                );
            }
            if case == "archers" {
                assert!(
                    screenshots.len() > page_count_before + 1,
                    "12 archer controls require real pagination"
                );
                assert!(
                    body_text.contains(&native_i18n::npc_text("Restore archer 12")),
                    "last archer action must be reachable"
                );
            }
            assert!(
                app.world().resource::<QuestUiIntentQueue>().is_empty(),
                "offline pagination cannot send a server command"
            );
        }
    }
    fs::write(&report_path, serde_json::to_vec_pretty(&json!({
        "kind":"offline_production_guild_name_and_sabuk_npc_gpu_fixture","passed":failures.is_empty(),"complete":true,
        "liveAcceptance":false,"windowCreated":false,"serverConnected":false,"playerSaveModified":false,
        "viewport":[WIDTH,HEIGHT],"systemFonts":false,"locales":Locale::ALL.map(Locale::code),
        "guildCases":["uncommitted IME with caret/underline","committed opaque name with caret"],
        "npcPages":["registrar-status","officer-status","manage","repairs","archers"],
        "layoutFailures":failures,"screenshots":screenshots,
        "limits":["Controlled server-dialog presentation fixtures do not grant guild or castle authority",
            "Offscreen glyph/caret checks do not validate OS-specific IME windows or live account/NPC flows"]
    })).unwrap()).unwrap();
    assert!(
        failures.is_empty(),
        "Guild/Sabuk production layout failures: {failures:?}"
    );
}
