//! Exercise actual Bevy AssetServer IO, registration, decoding and shaping.
use super::*;
use bevy::asset::{
    io::{
        memory::{Dir, MemoryAssetReader},
        AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader,
    },
    AssetApp, AssetMetaCheck, AssetPlugin,
};
use bevy::time::TimeUpdateStrategy;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const TC: &str = "original-ui/fonts/NotoSansTC.ttf";
const SKIN: &str = "original-ui/Prguse/827.png";

#[derive(Clone)]
struct ControlledReader {
    memory: MemoryAssetReader,
    counts: Arc<Mutex<HashMap<PathBuf, usize>>>,
    fail_through: usize,
    hold_second: Arc<AtomicBool>,
}
impl ControlledReader {
    fn count(&self, path: &str) -> usize {
        self.counts
            .lock()
            .unwrap()
            .get(Path::new(path))
            .copied()
            .unwrap_or(0)
    }
}
impl AssetReader for ControlledReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        if path == Path::new(TC) || path == Path::new(SKIN) {
            let attempt = {
                let mut counts = self.counts.lock().unwrap();
                let count = counts.entry(path.to_path_buf()).or_default();
                *count += 1;
                *count
            };
            if attempt <= self.fail_through {
                return Err(AssetReaderError::NotFound(path.to_path_buf()));
            }
            while attempt == 2 && self.hold_second.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
        }
        self.memory.read(path).await
    }
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.memory.read_meta(path).await
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.memory.read_directory(path).await
    }
    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.memory.is_directory(path).await
    }
}

struct ReleaseOnDrop(Arc<AtomicBool>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn io_app(
    fail_through: usize,
    hold_second: bool,
    corrupt_tc: bool,
) -> (App, ControlledReader, Handle<Font>, Handle<Image>) {
    let root = Dir::default();
    for locale in FONT_LOCALES {
        let path = QuestDisplayLanguage(locale).font_asset();
        let bytes = if corrupt_tc && path == TC {
            b"invalid font bytes".to_vec()
        } else {
            std::fs::read(format!(
                "{}/../../web/public/{path}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap()
        };
        root.insert_asset(Path::new(path), bytes);
    }
    root.insert_asset(
        Path::new(SKIN),
        std::fs::read(format!(
            "{}/../../web/public/{SKIN}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    );
    let reader = ControlledReader {
        memory: MemoryAssetReader { root },
        counts: default(),
        fail_through,
        hold_second: Arc::new(AtomicBool::new(hold_second)),
    };
    let source = reader.clone();
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(source.clone())),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            watch_for_changes_override: Some(false),
            use_asset_processor_override: Some(false),
            meta_check: AssetMetaCheck::Never,
            ..default()
        },
        bevy::image::ImagePlugin::default(),
    ));
    app.init_asset::<Font>()
        .init_asset_loader::<bevy::text::FontLoader>();
    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::NONE,
    ));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    install(&mut app);
    install(&mut app);
    app.world_mut().resource_mut::<FontCx>().collection =
        fontique::Collection::new(fontique::CollectionOptions {
            system_fonts: false,
            ..default()
        });
    app.add_systems(PostUpdate, load_font_assets_into_font_collection);
    let (font, image) = app
        .world_mut()
        .resource_scope(|world, mut fonts: Mut<PortableUiFonts>| {
            let server = world.resource::<AssetServer>();
            fonts.request(
                server,
                Some(QuestDisplayLanguage(Locale::TraditionalChinese)),
            );
            let image = server.load::<Image>(SKIN);
            fonts.track_images(&[image.clone(), image.clone()]);
            (
                fonts
                    .primary(Some(QuestDisplayLanguage(Locale::TraditionalChinese)))
                    .unwrap(),
                image,
            )
        });
    app.update(); // Initialize Time<Real>; all subsequent IO polling is frozen.
    (app, reader, font, image)
}

fn poll(app: &mut App, condition: impl Fn(&World) -> bool) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        app.update();
        if condition(app.world()) {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("real AssetServer fixture did not settle");
}
fn advance(app: &mut App, millis: u64) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        millis,
    )));
    app.update();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
}
fn readiness(world: &World) -> FontReadiness {
    world.resource::<PortableUiFonts>().readiness(
        world.resource::<AssetServer>(),
        Some(QuestDisplayLanguage(Locale::TraditionalChinese)),
    )
}
fn both_failed(world: &World, font: &Handle<Font>, image: &Handle<Image>) -> bool {
    let server = world.resource::<AssetServer>();
    matches!(server.get_load_state(font.id()), Some(LoadState::Failed(_)))
        && matches!(
            server.get_load_state(image.id()),
            Some(LoadState::Failed(_))
        )
}

#[test]
fn failed_font_and_skin_recover_in_one_app_with_same_handles_and_real_fallbacks() {
    let (mut app, reader, font, image) = io_app(1, true, false);
    let _release_on_unwind = ReleaseOnDrop(reader.hold_second.clone());
    let ids = (font.id(), image.id());
    poll(&mut app, |world| both_failed(world, &font, &image));
    app.update(); // Observe the failure before starting the cooldown.
    let mut error = None;
    crate::quest_ui_host::reconcile_asset_error(
        &mut error,
        readiness(app.world()).loading,
        readiness(app.world()).error,
    );
    assert!(error.is_some());
    advance(&mut app, 999);
    assert_eq!((reader.count(TC), reader.count(SKIN)), (1, 1));
    advance(&mut app, 1);
    poll(&mut app, |world| {
        reader.count(TC) == 2
            && reader.count(SKIN) == 2
            && matches!(
                world.resource::<AssetServer>().get_load_state(font.id()),
                Some(LoadState::Loading)
            )
    });
    advance(&mut app, 20_000);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!((reader.count(TC), reader.count(SKIN)), (2, 2));
    assert!(!readiness(app.world()).ready());
    crate::quest_ui_host::reconcile_asset_error(&mut error, true, false);
    assert!(error.is_some());
    reader.hold_second.store(false, Ordering::SeqCst);
    poll(&mut app, |world| {
        readiness(world).ready()
            && matches!(
                world.resource::<AssetServer>().get_load_state(image.id()),
                Some(LoadState::Loaded)
            )
    });
    let fonts = app.world().resource::<PortableUiFonts>();
    assert_eq!(
        fonts
            .primary(Some(QuestDisplayLanguage(Locale::TraditionalChinese)))
            .unwrap()
            .id(),
        ids.0
    );
    assert_eq!(fonts.skins.len(), 1);
    assert_eq!(fonts.skins.values().next().unwrap().id(), ids.1);
    let server = app.world().resource::<AssetServer>();
    assert_eq!(server.get_handle::<Font>(TC).unwrap().id(), ids.0);
    assert_eq!(server.get_handle::<Image>(SKIN).unwrap().id(), ids.1);
    assert!(app.world().resource::<Assets<Font>>().get(&font).is_some());
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&image)
            .unwrap()
            .size(),
        UVec2::new(40, 38)
    );
    crate::quest_ui_host::reconcile_asset_error(
        &mut error,
        readiness(app.world()).loading,
        readiness(app.world()).error,
    );
    assert!(error.is_none());
    {
        let mut cx = app.world_mut().resource_mut::<FontCx>();
        for (tag, family) in SCRIPT_FAMILIES {
            let expected = cx.collection.family_id(family);
            assert_eq!(
                cx.collection
                    .fallback_families(fontique::Script::from_bytes(tag))
                    .next(),
                expected
            );
        }
    }
    let mut fixture = Fixture {
        assets: app.world_mut().remove_resource().unwrap(),
        fonts: app.world_mut().remove_resource().unwrap(),
        cx: app.world_mut().remove_resource().unwrap(),
        layout: default(),
        scale: default(),
        pipeline: default(),
        images: default(),
        atlases: default(),
    };
    assert!(
        glyphs(&fixture.shape(Some(Locale::TraditionalChinese), "任務完成 · 玩家金魚"))
            .iter()
            .all(|id| *id != 0)
    );
}

#[test]
fn permanent_failures_stop_after_three_total_requests_even_after_tracking_again() {
    let (mut app, reader, font, image) = io_app(usize::MAX, false, false);
    poll(&mut app, |world| both_failed(world, &font, &image));
    app.update();
    advance(&mut app, 1_000);
    poll(&mut app, |world| {
        reader.count(TC) == 2 && reader.count(SKIN) == 2 && both_failed(world, &font, &image)
    });
    app.update();
    advance(&mut app, 1_999);
    assert_eq!((reader.count(TC), reader.count(SKIN)), (2, 2));
    advance(&mut app, 1);
    poll(&mut app, |world| {
        reader.count(TC) == 3 && reader.count(SKIN) == 3 && both_failed(world, &font, &image)
    });
    app.world_mut()
        .resource_mut::<PortableUiFonts>()
        .track_images(&[image.clone()]);
    advance(&mut app, 60_000);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!((reader.count(TC), reader.count(SKIN)), (3, 3));
    assert!(readiness(app.world()).error);
    assert!(!readiness(app.world()).ready());
}

#[test]
fn loaded_invalid_font_cannot_advertise_readiness_or_clear_the_asset_error() {
    let (mut app, reader, font, image) = io_app(0, false, true);
    poll(&mut app, |world| {
        let server = world.resource::<AssetServer>();
        matches!(server.get_load_state(font.id()), Some(LoadState::Loaded))
            && matches!(server.get_load_state(image.id()), Some(LoadState::Loaded))
            && world.resource::<Assets<Font>>().len() == FONT_LOCALES.len()
    });
    assert!(!app.world().resource::<PortableUiFonts>().fallbacks_ready);
    let state = readiness(app.world());
    assert!(state.loading && !state.error && !state.ready());
    let mut error = Some("quest UI font or skin failed to load".to_owned());
    crate::quest_ui_host::reconcile_asset_error(&mut error, state.loading, state.error);
    assert!(error.is_some());
    advance(&mut app, 60_000);
    assert_eq!((reader.count(TC), reader.count(SKIN)), (1, 1));
}
