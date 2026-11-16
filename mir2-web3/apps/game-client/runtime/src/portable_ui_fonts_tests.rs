use super::*;
use bevy::text::{
    load_font_assets_into_font_collection, FontAtlasSet, FontHinting, Justify, LayoutCx,
    LetterSpacing, LineBreak, LineHeight, ScaleCx, TextBounds, TextLayoutInfo, TextPipeline,
};
use std::collections::BTreeSet;

#[path = "portable_ui_asset_recovery_io_tests.rs"]
mod recovery_io;

fn font_app(configure: bool) -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .add_systems(PostUpdate, load_font_assets_into_font_collection);
    app.world_mut().resource_mut::<FontCx>().collection =
        fontique::Collection::new(fontique::CollectionOptions {
            system_fonts: false,
            ..default()
        });
    if configure {
        install(&mut app);
        install(&mut app); // Same resource and scheduling, not a second policy.
    } else {
        app.init_resource::<PortableUiFonts>();
    }
    for locale in FONT_LOCALES {
        let path = QuestDisplayLanguage(locale).font_asset();
        let bytes = std::fs::read(format!(
            "{}/../../web/public/{path}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(bytes));
        app.world_mut()
            .resource_mut::<PortableUiFonts>()
            .handles
            .insert(path, handle);
    }
    app.world_mut().run_schedule(PostUpdate);
    app
}

struct Fixture {
    assets: Assets<Font>,
    fonts: PortableUiFonts,
    cx: FontCx,
    layout: LayoutCx,
    scale: ScaleCx,
    pipeline: TextPipeline,
    images: Assets<Image>,
    atlases: FontAtlasSet,
}
impl Fixture {
    fn new(configure: bool) -> Self {
        let mut app = font_app(configure);
        Self {
            assets: app.world_mut().remove_resource().unwrap(),
            fonts: app.world_mut().remove_resource().unwrap(),
            cx: app.world_mut().remove_resource().unwrap(),
            layout: default(),
            scale: default(),
            pipeline: default(),
            images: default(),
            atlases: default(),
        }
    }
    fn shape(&mut self, locale: Option<Locale>, text: &str) -> ComputedTextBlock {
        let mut block = ComputedTextBlock::default();
        let font = TextFont {
            font: self
                .fonts
                .primary(locale.map(QuestDisplayLanguage))
                .unwrap()
                .into(),
            font_size: FontSize::Px(18.),
            ..default()
        };
        self.pipeline
            .update_buffer(
                &self.assets,
                std::iter::once((
                    Entity::PLACEHOLDER,
                    0,
                    text,
                    &font,
                    Color::WHITE,
                    LineHeight::default(),
                    LetterSpacing::default(),
                )),
                LineBreak::WordOrCharacter,
                Justify::Start,
                TextBounds::UNBOUNDED,
                1.,
                &mut block,
                &mut self.cx,
                &mut self.layout,
                Vec2::new(1024., 768.),
                16.,
            )
            .unwrap();
        let mut info = TextLayoutInfo::default();
        self.pipeline
            .update_text_layout_info(
                &mut info,
                &mut self.atlases,
                &mut self.images,
                &mut block,
                &mut self.scale,
                TextBounds::UNBOUNDED,
                Justify::Start,
                FontHinting::default(),
            )
            .unwrap();
        assert!(!info.glyphs.is_empty() && info.size.x > 0. && info.size.y > 0.);
        self.fonts.retain(&block);
        block
    }
    fn atlas_counts(&self) -> (usize, usize, u64) {
        (
            self.atlases
                .keys()
                .map(|key| key.id)
                .collect::<BTreeSet<_>>()
                .len(),
            self.atlases.values().map(Vec::len).sum(),
            self.atlases.total_bytes(&self.images),
        )
    }
}
fn glyphs(block: &ComputedTextBlock) -> Vec<u32> {
    let mut ids = Vec::new();
    for line in block.buffer().lines() {
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                ids.extend(cluster.glyphs().map(|glyph| glyph.id));
            }
        }
    }
    ids
}
fn sources(block: &ComputedTextBlock) -> BTreeSet<u64> {
    block
        .buffer()
        .lines()
        .flat_map(|line| line.runs())
        .map(|run| run.font().data.id())
        .collect()
}

#[test]
fn exact_opt_in_uses_quest_font_policy_and_missing_opt_in_keeps_legacy_cjk() {
    assert_eq!(primary_path(None), LEGACY_CJK_FONT);
    for locale in Locale::ALL {
        assert_eq!(
            primary_path(Some(QuestDisplayLanguage(locale))),
            QuestDisplayLanguage(locale).font_asset()
        );
        native_i18n::with_locale(locale, || {
            assert_eq!(display_language(), Some(QuestDisplayLanguage(locale)));
        });
    }
}

#[test]
fn a_ready_old_cjk_face_cannot_authorize_a_pending_multilingual_font_set() {
    let mut fonts = PortableUiFonts::default();
    let mut assets = Assets::<Font>::default();
    fonts
        .handles
        .insert(LEGACY_CJK_FONT, assets.add(Font::from_bytes(Vec::new())));
    assert!(fonts
        .readiness_from(None, |_| Some(LoadState::Loaded))
        .ready());
    let display = Some(QuestDisplayLanguage(Locale::Russian));
    assert!(!fonts
        .readiness_from(display, |_| Some(LoadState::Loaded))
        .ready());
    for locale in FONT_LOCALES {
        fonts
            .handles
            .entry(QuestDisplayLanguage(locale).font_asset())
            .or_insert_with(|| assets.add(Font::from_bytes(Vec::new())));
    }
    // Loaded bytes alone are insufficient until the real collection has maps.
    assert!(!fonts
        .readiness_from(display, |_| Some(LoadState::Loaded))
        .ready());
    fonts.fallbacks_ready = true;
    assert!(fonts
        .readiness_from(display, |_| Some(LoadState::Loaded))
        .ready());
    assert!(!fonts
        .readiness_from(display, |_| Some(LoadState::Loading))
        .ready());
    assert!(!fonts.readiness_from(display, |_| None).ready());
    let failure = std::sync::Arc::new(bevy::asset::AssetLoadError::AssetReaderError(
        bevy::asset::io::AssetReaderError::NotFound("known-font.ttf".into()),
    ));
    let failed = fonts.readiness_from(display, |_| Some(LoadState::Failed(failure.clone())));
    assert!(failed.error && !failed.ready());
    // A subsequent legacy payload deliberately selects its old independent path.
    assert!(fonts
        .readiness_from(None, |_| Some(LoadState::Loaded))
        .ready());
}

#[test]
fn shared_owned_left_alignment_follows_bidi_and_preserves_legacy_and_other_layouts() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>();
    install(&mut app);
    app.edit_schedule(PostUpdate, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    let root = app
        .world_mut()
        .spawn((
            mir2_client_bevy::crystal_ui::shared_hud::SharedMainHudRoot,
            Node::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            ChildOf(root),
            bevy::text::TextLayout {
                justify: Justify::Left,
                ..default()
            },
        ))
        .id();
    let center = app
        .world_mut()
        .spawn((
            ChildOf(root),
            bevy::text::TextLayout {
                justify: Justify::Center,
                ..default()
            },
        ))
        .id();
    let outside = app
        .world_mut()
        .spawn(bevy::text::TextLayout {
            justify: Justify::Left,
            ..default()
        })
        .id();
    app.world_mut().run_schedule(PostUpdate);
    assert_eq!(
        app.world()
            .get::<bevy::text::TextLayout>(child)
            .unwrap()
            .justify,
        Justify::Left
    );
    native_i18n::with_locale(Locale::Arabic, || app.world_mut().run_schedule(PostUpdate));
    assert_eq!(
        app.world()
            .get::<bevy::text::TextLayout>(child)
            .unwrap()
            .justify,
        Justify::Start
    );
    assert_eq!(
        app.world()
            .get::<bevy::text::TextLayout>(center)
            .unwrap()
            .justify,
        Justify::Center
    );
    assert_eq!(
        app.world()
            .get::<bevy::text::TextLayout>(outside)
            .unwrap()
            .justify,
        Justify::Left
    );
}

#[test]
fn real_font_registration_restores_script_and_han_maps_after_collection_rebuild() {
    let mut app = font_app(true);
    assert!(app.world().resource::<PortableUiFonts>().fallbacks_ready);
    let before = {
        let mut cx = app.world_mut().resource_mut::<FontCx>();
        SCRIPT_FAMILIES.map(|(tag, _)| {
            cx.collection
                .fallback_families(fontique::Script::from_bytes(tag))
                .collect::<Vec<_>>()
        })
    };
    let temporary = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            std::fs::read(format!(
                "{}/../../web/public/original-ui/fonts/NotoSans-Regular.ttf",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        ));
    app.world_mut().run_schedule(PostUpdate);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .remove(temporary.id());
    app.world_mut().run_schedule(PostUpdate);
    assert!(app.world().resource::<PortableUiFonts>().fallbacks_ready);
    let mut cx = app.world_mut().resource_mut::<FontCx>();
    for ((tag, family), old) in SCRIPT_FAMILIES.into_iter().zip(before) {
        let script = fontique::Script::from_bytes(tag);
        let mapped = cx.collection.fallback_families(script).collect::<Vec<_>>();
        assert_eq!(mapped.len(), old.len());
        assert_eq!(mapped[0], cx.collection.family_id(family).unwrap());
    }
    let han = fontique::Script::from_bytes(*b"Hani");
    let traditional = cx.collection.family_id("Noto Sans TC").unwrap();
    let simplified = cx.collection.family_id("Noto Sans CJK SC").unwrap();
    for code in ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "ja", "ko"] {
        assert_eq!(
            cx.collection
                .fallback_families((han, code))
                .collect::<Vec<_>>(),
            [traditional, simplified]
        );
    }
    assert_eq!(
        cx.collection
            .fallback_families((han, "zh-SG"))
            .collect::<Vec<_>>(),
        [simplified, traditional]
    );
}

#[test]
fn all_ten_primaries_shape_and_rasterize_mixed_opaque_names_without_system_fonts() {
    let mut f = Fixture::new(true);
    let allowed = f
        .assets
        .iter()
        .map(|(_, font)| font.data.id())
        .collect::<BTreeSet<_>>();
    for (locale, sample) in [
        (Locale::English, "Quest complete · Gold 123"),
        (Locale::TraditionalChinese, "任務完成 · 金幣 123"),
        (Locale::SimplifiedChinese, "任务完成 · 金币 123"),
        (Locale::BrazilianPortuguese, "Missão concluída · Gold 123"),
        (Locale::Russian, "Задание выполнено · Gold 123"),
        (Locale::Hindi, "कार्य पूरा हुआ · Gold 123"),
        (Locale::Indonesian, "Misi selesai · Gold 123"),
        (Locale::Vietnamese, "Nhiệm vụ hoàn thành · Gold 123"),
        (Locale::Thai, "ภารกิจสำเร็จ · Gold 123"),
        (Locale::Arabic, "اكتملت المهمة · Gold 123"),
    ] {
        for text in [sample, "Gold 玩家金魚 Игрок खिलाड़ी ผู้เล่น لاعب Việt {opaque}"]
        {
            let block = f.shape(Some(locale), text);
            assert!(
                glyphs(&block).iter().all(|id| *id != 0),
                "{locale:?}: {text}"
            );
            assert!(
                sources(&block).is_subset(&allowed),
                "system font in {locale:?}"
            );
        }
    }
    let old = f.shape(None, "装备背包 · 玩家金魚");
    assert!(glyphs(&old).iter().all(|id| *id != 0));
}

#[test]
fn handles_without_fallbacks_have_missing_glyphs_but_the_real_helper_recovers_them() {
    let mixed = "玩家 खिलाड़ी ผู้เล่น لاعب";
    let mut old = Fixture::new(false);
    let block = old.shape(Some(Locale::English), mixed);
    assert!(
        glyphs(&block).contains(&0),
        "negative control must expose missing fallback"
    );
    assert!(configure_fallbacks(&mut old.cx));
    let fixed = old.shape(Some(Locale::English), mixed);
    assert!(glyphs(&fixed).iter().all(|id| *id != 0));
}

#[test]
fn arabic_joins_and_reopened_opaque_runs_keep_source_identity_and_atlas_counts() {
    let mut f = Fixture::new(true);
    let joined = glyphs(&f.shape(Some(Locale::Arabic), "سلام"));
    let isolated = "سلام"
        .chars()
        .flat_map(|ch| glyphs(&f.shape(Some(Locale::Arabic), &ch.to_string())))
        .collect::<Vec<_>>();
    assert_ne!(joined, isolated);
    let text = "مرحبا Gold 123 玩家 खिलाड़ी ผู้เล่น";
    let block = f.shape(Some(Locale::Arabic), text);
    let ids = sources(&block);
    let counts = f.atlas_counts();
    drop(block);
    for _ in 0..4 {
        for _ in 0..3 {
            f.cx.source_cache.prune(2, false);
        }
        let block = f.shape(Some(Locale::Arabic), text);
        assert_eq!(sources(&block), ids);
        assert_eq!(f.atlas_counts(), counts);
        assert!(glyphs(&block).iter().all(|id| *id != 0));
    }
}
