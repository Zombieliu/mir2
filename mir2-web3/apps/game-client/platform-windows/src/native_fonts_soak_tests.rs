//! Exercise the actual text pipeline and installed fallback faces without a
//! game connection, renderer, or changes to a player's state.

use super::*;
use bevy::{
    asset::AssetId,
    image::Image,
    prelude::{Color, Entity, FontSize, FontSource, TextFont, Vec2},
    text::{
        load_font_assets_into_font_collection, ComputedTextBlock, FontAtlasSet, FontCx,
        FontHinting, Justify, LayoutCx, LetterSpacing, LineBreak, LineHeight, ScaleCx, TextBounds,
        TextLayoutInfo, TextPipeline, DEFAULT_FONT_DATA,
    },
};
use std::collections::BTreeSet;

struct TextFixture {
    fonts: Assets<Font>,
    _pinned: NativePinnedFonts,
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
        install(&mut app);
        app.update();
        Self {
            fonts: app.world_mut().remove_resource().unwrap(),
            _pinned: app.world_mut().remove_resource().unwrap(),
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
    eprintln!("font-soak cycles=2000 layouts=10005 warm={warm:?} final={:?} retained_sources={retained_sources}", fixture.counts());
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
