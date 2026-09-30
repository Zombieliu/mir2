//! Exercise the actual text pipeline and installed fallback faces without a
//! game connection, renderer, or changes to a player's state.

use super::*;
use bevy::{
    asset::AssetId,
    image::Image,
    prelude::{Color, Entity, FontSize, FontSource, TextFont, Vec2},
    text::{
        ComputedTextBlock, DEFAULT_FONT_DATA, FontAtlasSet, FontCx, FontHinting, Justify, LayoutCx,
        LetterSpacing, LineBreak, LineHeight, ScaleCx, TextBounds, TextLayoutInfo, TextPipeline,
        load_font_assets_into_font_collection,
    },
};
use std::collections::BTreeSet;

struct TextFixture {
    fonts: Assets<Font>,
    _pinned: NativePinnedFonts,
    _traditional: NativeTraditionalFont,
    _bundled: Option<NativeBundledFonts>,
    cx: FontCx,
    layout: LayoutCx,
    scale: ScaleCx,
    pipeline: TextPipeline,
    images: Assets<Image>,
    atlases: FontAtlasSet,
    retained: NativeRetainedFontSources,
}

impl TextFixture {
    fn new() -> Self {
        Self::with_bundled_only(false)
    }

    fn with_bundled_only(bundled_only: bool) -> Self {
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .add_systems(bevy::app::Update, load_font_assets_into_font_collection);
        app.world_mut()
            .resource_mut::<Assets<Font>>()
            .insert(
                AssetId::default(),
                Font::from_bytes(DEFAULT_FONT_DATA.to_vec()),
            )
            .unwrap();
        install_source_identity_retention(&mut app);
        if bundled_only {
            app.world_mut().resource_mut::<FontCx>().collection =
                fontique::Collection::new(fontique::CollectionOptions {
                    system_fonts: false,
                    ..Default::default()
                });
            install_bundled_fonts(&mut app);
            app.init_resource::<NativePinnedFonts>();
            app.add_systems(
                bevy::app::Update,
                restore_bundled_fallbacks.after(load_font_assets_into_font_collection),
            );
        } else {
            // Keep the original OS-fallback regression and its negative
            // controls independent of the new bundled-script path.
            let handle = app
                .world_mut()
                .resource_mut::<Assets<Font>>()
                .add(Font::from_bytes(TRADITIONAL_FONT.to_vec()));
            app.insert_resource(NativeTraditionalFont { _handle: handle });
            install_named_fonts(&mut app);
        }
        app.update();
        Self {
            fonts: app.world_mut().remove_resource().unwrap(),
            _pinned: app.world_mut().remove_resource().unwrap(),
            _traditional: app.world_mut().remove_resource().unwrap(),
            _bundled: app.world_mut().remove_resource(),
            cx: app.world_mut().remove_resource().unwrap(),
            layout: LayoutCx::default(),
            scale: ScaleCx::default(),
            pipeline: TextPipeline::default(),
            images: Assets::default(),
            atlases: FontAtlasSet::default(),
            retained: app.world_mut().remove_resource().unwrap(),
        }
    }

    fn text(
        &mut self,
        source: FontSource,
        text: &str,
        size: f32,
    ) -> (ComputedTextBlock, TextLayoutInfo) {
        let mut computed = ComputedTextBlock::default();
        let font = TextFont {
            font: source,
            font_size: FontSize::Px(size),
            ..Default::default()
        };
        self.pipeline
            .update_buffer(
                &self.fonts,
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
                Justify::Left,
                TextBounds::UNBOUNDED,
                1.0,
                &mut computed,
                &mut self.cx,
                &mut self.layout,
                Vec2::new(1024.0, 768.0),
                16.0,
            )
            .expect("shape game text");
        let mut info = TextLayoutInfo::default();
        self.pipeline
            .update_text_layout_info(
                &mut info,
                &mut self.atlases,
                &mut self.images,
                &mut computed,
                &mut self.scale,
                TextBounds::UNBOUNDED,
                Justify::Left,
                FontHinting::default(),
            )
            .expect("rasterize game text");
        assert!(!info.glyphs.is_empty());
        (computed, info)
    }

    fn counts(&self) -> (usize, usize, u64) {
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

    fn frame(&mut self, copies: usize) {
        // Like real UI measurement, keep all of this frame's layouts alive
        // until the collector runs after rasterization, then despawn them all.
        let blocks: Vec<_> = (0..copies)
            .flat_map(|_| game_samples())
            .map(|(source, text, size)| self.text(source, text, size).0)
            .collect();
        for block in &blocks {
            self.retained.retain(block);
        }
    }

    fn expire_sources(&mut self) {
        for _ in 0..3 {
            self.cx.source_cache.prune(2, false);
        }
    }
}

fn game_samples() -> Vec<(FontSource, &'static str, f32)> {
    vec![
        (
            FontSource::Family("Arial".into()),
            "a1  BichonWall Board  Skeleton  HP 162/162",
            10.0,
        ),
        (
            FontSource::Family("Arial".into()),
            "前往入口 · 自动寻路  已设置入口路线 (147,33) · 按 Esc 可停止",
            12.0,
        ),
        (
            FontSource::Family("Microsoft YaHei".into()),
            "当前任务 · Return to BichonWall - Board（334,259）",
            12.0,
        ),
        (
            FontSource::default(),
            "设为当前  查看任务详情  SHARE  Finish  →  ✓",
            11.0,
        ),
        (
            FontSource::Family("Arial".into()),
            "Online Players: 1  Damage +128  骷髅 ⚔ ★",
            10.0,
        ),
    ]
}

fn glyph_ids(block: &ComputedTextBlock) -> Vec<u32> {
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

#[test]
fn nine_languages_shape_without_system_fonts_or_missing_glyphs() {
    let mut fixture = TextFixture::with_bundled_only(true);
    let allowed: BTreeSet<_> = fixture
        .fonts
        .iter()
        .map(|(_, font)| font.data.id())
        .collect();
    for (locale, family, text) in NINE_LANGUAGE_SAMPLES {
        let (block, info) = fixture.text(FontSource::Family(family.into()), text, 18.0);
        assert!(info.size.x > 0.0 && info.size.y > 0.0, "{locale}");
        assert!(
            glyph_ids(&block).iter().all(|id| *id != 0),
            "missing glyph in {locale}"
        );
        for line in block.buffer().lines() {
            for run in line.runs() {
                assert!(
                    allowed.contains(&run.font().data.id()),
                    "{locale} used a system font"
                );
            }
        }
    }
    // Opaque cross-language player text must work under an unrelated UI font.
    let mixed = "Gold Cancel 玩家 Игрок खिलाड़ी ผู้เล่น لاعب Việt";
    let (block, _) = fixture.text(FontSource::Family("Noto Sans".into()), mixed, 18.0);
    assert!(glyph_ids(&block).iter().all(|id| *id != 0));
}

#[test]
fn bundled_arabic_uses_joining_and_bidirectional_runs_without_reversing_numbers() {
    let mut fixture = TextFixture::with_bundled_only(true);
    let source = FontSource::Family("Noto Sans Arabic".into());
    let (joined, _) = fixture.text(source.clone(), "سلام", 24.0);
    let joined_ids = glyph_ids(&joined);
    let isolated_ids: Vec<_> = "سلام"
        .chars()
        .flat_map(|ch| {
            let (block, _) = fixture.text(source.clone(), &ch.to_string(), 24.0);
            glyph_ids(&block)
        })
        .collect();
    assert_ne!(
        joined_ids, isolated_ids,
        "Arabic must use contextual forms, not isolated codepoint glyphs"
    );
    assert!(joined_ids.iter().all(|id| *id != 0));
    let text = "مرحبا Gold 123 عالم";
    let (mixed, _) = fixture.text(source, text, 24.0);
    let mut rtl = 0;
    let mut ltr = 0;
    let mut numbers_ltr = false;
    for line in mixed.buffer().lines() {
        for run in line.runs() {
            let starts: Vec<_> = run
                .visual_clusters()
                .map(|cluster| cluster.text_range().start)
                .collect();
            if run.is_rtl() {
                rtl += 1;
                assert!(starts.windows(2).all(|pair| pair[0] >= pair[1]));
            } else {
                ltr += 1;
                assert!(starts.windows(2).all(|pair| pair[0] <= pair[1]));
                numbers_ltr |= text[run.text_range()].contains("123");
            }
        }
    }
    assert!(
        rtl >= 1 && ltr >= 1 && numbers_ltr,
        "mixed Arabic/Latin/numbers must remain separate directional runs"
    );
}

#[test]
fn bundled_devanagari_and_thai_keep_real_cluster_shaping() {
    let mut fixture = TextFixture::with_bundled_only(true);
    let allowed: BTreeSet<_> = fixture
        .fonts
        .iter()
        .map(|(_, font)| font.data.id())
        .collect();
    let glyph_signature = |block: &ComputedTextBlock| {
        let mut signature = Vec::new();
        for line in block.buffer().lines() {
            for run in line.runs() {
                for cluster in run.visual_clusters() {
                    for glyph in cluster.glyphs() {
                        assert_ne!(glyph.id, 0, "shaping produced a missing glyph");
                        assert!(
                            glyph.x.is_finite() && glyph.y.is_finite() && glyph.advance.is_finite()
                        );
                        signature.push((glyph.id, glyph.x, glyph.y, glyph.advance));
                    }
                }
            }
        }
        assert!(!signature.is_empty());
        signature
    };
    for (family, text) in [("Noto Sans Devanagari", "क्षि"), ("Noto Sans Thai", "กิ้")]
    {
        let source = FontSource::Family(family.into());
        let (block, _) = fixture.text(source.clone(), text, 24.0);
        let mut compound = false;
        let mut covered_bytes = 0;
        for line in block.buffer().lines() {
            for run in line.runs() {
                assert!(
                    allowed.contains(&run.font().data.id()),
                    "{family} used a system font"
                );
                assert!(
                    !run.is_rtl(),
                    "these two shaping fixtures are left-to-right"
                );
                let mut ligature_chars = 0;
                for cluster in run.clusters() {
                    let range = cluster.text_range();
                    assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
                    assert_eq!(
                        range.start, covered_bytes,
                        "{family} lost source text coverage"
                    );
                    assert!(range.end > range.start);
                    covered_bytes = range.end;
                    let chars = text[range].chars().count();
                    // Parley 0.9 splits a HarfRust merged cluster into a
                    // LigatureStart and per-character LigatureComponents.
                    // Their text_range() values each cover one character;
                    // the continuation flags preserve the actual grouping.
                    if cluster.is_ligature_continuation() {
                        assert!(ligature_chars > 0, "{family} has an orphan continuation");
                        assert!(!cluster.is_ligature_start());
                        assert!(cluster.glyphs().next().is_none());
                        ligature_chars += chars;
                    } else {
                        if ligature_chars > 0 {
                            assert!(ligature_chars > 1, "{family} has an incomplete ligature");
                            compound = true;
                        }
                        ligature_chars = if cluster.is_ligature_start() {
                            chars
                        } else {
                            0
                        };
                    }
                    assert!(cluster.glyphs().all(|glyph| glyph.id != 0));
                }
                if ligature_chars > 0 {
                    assert!(ligature_chars > 1, "{family} has an incomplete ligature");
                    compound = true;
                }
            }
        }
        assert_eq!(covered_bytes, text.len());
        assert!(
            compound,
            "{family} lost its multi-codepoint shaping cluster"
        );
        let shaped = glyph_signature(&block);
        let isolated: Vec<_> = text
            .chars()
            .flat_map(|ch| {
                let (isolated, _) = fixture.text(source.clone(), &ch.to_string(), 24.0);
                glyph_signature(&isolated)
            })
            .collect();
        assert_ne!(
            shaped, isolated,
            "{family} must apply contextual substitution or mark positioning"
        );
    }
}

#[test]
fn nine_language_switching_keeps_font_identity_and_atlas_memory_bounded() {
    let mut fixture = TextFixture::with_bundled_only(true);
    let asset_count = fixture.fonts.len();
    for (_, family, text) in NINE_LANGUAGE_SAMPLES {
        let (block, _) = fixture.text(FontSource::Family(family.into()), text, 18.0);
        fixture.retained.retain(&block);
    }
    let baseline = fixture.counts();
    let retained = fixture.retained.sources.len();
    for switch in 0..180 {
        let (_, family, text) = NINE_LANGUAGE_SAMPLES[switch % 9];
        fixture.expire_sources();
        let (block, _) = fixture.text(FontSource::Family(family.into()), text, 18.0);
        fixture.retained.retain(&block);
        assert_eq!(fixture.counts(), baseline, "atlas grew at switch {switch}");
        assert_eq!(fixture.retained.sources.len(), retained);
        assert_eq!(fixture.fonts.len(), asset_count);
    }
    eprintln!(
        "nine-language-font-soak switches=180 system_fonts=false baseline={baseline:?} retained={retained} assets={asset_count}"
    );
}

#[test]
fn game_fallback_fonts_remain_bounded_across_hidden_and_rebuilt_text() {
    let mut fixture = TextFixture::new();
    fixture.frame(1);
    let warm = fixture.counts();
    let retained_sources = fixture.retained.sources.len();
    for cycle in 0..2_000 {
        // UI panels can disappear entirely between rebuilds. Exercise the
        // same source expiry as TextPlugin's Last schedule, not an idle sleep.
        fixture.expire_sources();
        fixture.frame(1);
        assert_eq!(
            fixture.counts(),
            warm,
            "identical text grew at cycle {cycle}"
        );
        assert_eq!(fixture.retained.sources.len(), retained_sources);
    }
    eprintln!(
        "font-soak cycles=2000 layouts=10005 warm={warm:?} final={:?} retained_sources={retained_sources}",
        fixture.counts()
    );
    assert_eq!(
        fixture.counts(),
        warm,
        "identical game text must reuse its font atlas after source expiry"
    );
}

#[test]
fn source_sharing_and_strong_retention_are_both_required() {
    for (shared, retained) in [(false, true), (true, false)] {
        let mut fixture = TextFixture::new();
        if !shared {
            fixture.cx.source_cache = Default::default();
        }
        fixture.frame(1);
        let warm = fixture.counts();
        for _ in 0..8 {
            if !retained {
                fixture.retained.sources.clear();
            }
            fixture.expire_sources();
            fixture.frame(1);
        }
        eprintln!(
            "negative-control shared={shared} retained={retained} warm={warm:?} final={:?}",
            fixture.counts()
        );
        assert!(
            fixture.counts().0 > warm.0,
            "negative control must reproduce identity churn"
        );
        assert!(
            fixture.counts().2 > warm.2,
            "negative control must reproduce atlas growth"
        );
    }
}

#[test]
fn dense_game_text_stays_bounded_through_per_layout_cache_pruning() {
    let mut fixture = TextFixture::new();
    // Parley prunes at age 128 on every layout; exercise more than that in
    // one frame, including recurring Latin/CJK and symbol fallback sources.
    fixture.frame(40);
    let warm = fixture.counts();
    for _ in 0..50 {
        fixture.expire_sources();
        fixture.frame(40);
        assert_eq!(fixture.counts(), warm);
    }
    eprintln!(
        "font-dense frames=51 layouts=10200 warm={warm:?} final={:?}",
        fixture.counts()
    );
}

#[test]
fn retained_fallback_sources_preserve_raster_pixels_and_glyph_positions() {
    use sha2::{Digest, Sha256};
    let mut old = TextFixture::new();
    old.cx.source_cache = Default::default();
    let mut fixed = TextFixture::new();
    for (source, text, size) in game_samples() {
        let (old_block, old_info) = old.text(source.clone(), text, size);
        let (fixed_block, fixed_info) = fixed.text(source, text, size);
        fixed.retained.retain(&fixed_block);
        assert_eq!(old_info.size, fixed_info.size);
        assert_eq!(old_info.glyphs.len(), fixed_info.glyphs.len());
        for (before, after) in old_info.glyphs.iter().zip(&fixed_info.glyphs) {
            assert_eq!(before.position, after.position);
            assert_eq!(before.line_index, after.line_index);
            assert_eq!(before.atlas_info.rect, after.atlas_info.rect);
        }
        let font_faces = |block: &ComputedTextBlock| {
            block
                .buffer()
                .lines()
                .flat_map(|line| line.runs())
                .map(|run| {
                    (
                        Sha256::digest(run.font().data.as_ref()).to_vec(),
                        run.font().index,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            font_faces(&old_block),
            font_faces(&fixed_block),
            "same selected font bytes and TTC face for {text}"
        );
    }
    let pixels = |images: &Assets<Image>| {
        let mut hashes: Vec<_> = images
            .iter()
            .map(|(_, image)| {
                Sha256::digest(image.data.as_ref().expect("font atlas pixels")).to_vec()
            })
            .collect();
        hashes.sort();
        hashes
    };
    assert_eq!(pixels(&old.images), pixels(&fixed.images));
}

#[test]
fn installed_collector_runs_after_layout_and_survives_entity_teardown() {
    use bevy::prelude::{Commands, ResMut};
    #[derive(Resource)]
    struct ReadyBlock(Option<ComputedTextBlock>);
    fn publish_after_layout(mut commands: Commands, mut ready: ResMut<ReadyBlock>) {
        if let Some(block) = ready.0.take() {
            commands.spawn(block);
        }
    }
    let mut fixture = TextFixture::new();
    let block = fixture.text(FontSource::default(), "任务完成 ✓", 11.0).0;
    let ids: BTreeSet<_> = block
        .buffer()
        .lines()
        .flat_map(|line| line.runs())
        .map(|run| run.font().data.id())
        .collect();
    assert!(ids.len() >= 2, "mixed text must exercise fallback fonts");
    let mut app = App::new();
    // No Assets<Font>/WINDIR dependency: the retention installer must still
    // protect fallback sources when named font pinning is unavailable.
    install(&mut app);
    app.insert_resource(ReadyBlock(Some(block))).add_systems(
        PostUpdate,
        publish_after_layout.in_set(bevy::ui::UiSystems::PostLayout),
    );
    app.update();
    let retained_ids: BTreeSet<_> = app
        .world()
        .resource::<NativeRetainedFontSources>()
        .sources
        .keys()
        .copied()
        .collect();
    assert_eq!(retained_ids, ids);
    let entities: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, bevy::prelude::With<ComputedTextBlock>>()
        .iter(app.world())
        .collect();
    for entity in entities {
        app.world_mut().despawn(entity);
    }
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world()
            .resource::<NativeRetainedFontSources>()
            .sources
            .len(),
        ids.len()
    );
}
