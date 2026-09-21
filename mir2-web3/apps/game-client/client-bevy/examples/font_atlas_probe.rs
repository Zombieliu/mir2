//! Headless probe for Bevy 0.19 system-font atlas lifetime.
//!
//! Run with:
//! `cargo run --example font_atlas_probe --features native-ui`
//!
//! It deliberately calls the public text pipeline directly. No `App`, window,
//! renderer, or native desktop surface is created.

#[cfg(not(feature = "native-ui"))]
fn main() {
    eprintln!("SKIP font_atlas_probe requires the client-bevy `native-ui` feature; run `cargo run --example font_atlas_probe --features native-ui`.");
}

#[cfg(feature = "native-ui")]
mod native {
    use std::collections::BTreeSet;

    use bevy::{
        asset::Assets,
        color::Color,
        image::Image,
        math::Vec2,
        prelude::{Entity, FontSize, FontSource, TextFont},
        text::{
            ComputedTextBlock, Font, FontAtlasSet, FontCx, FontHinting, Justify, LayoutCx,
            LetterSpacing, LineBreak, LineHeight, ScaleCx, TextBounds, TextLayoutInfo,
            TextPipeline,
        },
    };

    const ARIAL: &str = "Arial";
    const SAMPLE: &str = "Arial atlas probe 0123456789";
    const CYCLES: usize = 100;
    const INACTIVE_PRUNES: usize = 3;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Snapshot {
        keys: usize,
        pages: usize,
        bytes: u64,
        font_ids: Vec<u32>,
    }

    struct Probe {
        fonts: Assets<Font>,
        images: Assets<Image>,
        font_cx: FontCx,
        layout_cx: LayoutCx,
        scale_cx: ScaleCx,
        pipeline: TextPipeline,
        atlases: FontAtlasSet,
    }

    impl Default for Probe {
        fn default() -> Self {
            Self {
                fonts: Assets::default(),
                images: Assets::default(),
                font_cx: FontCx::default(),
                layout_cx: LayoutCx::default(),
                scale_cx: ScaleCx::default(),
                pipeline: TextPipeline::default(),
                atlases: FontAtlasSet::default(),
            }
        }
    }

    fn snapshot(probe: &Probe) -> Snapshot {
        let font_ids = probe
            .atlases
            .keys()
            .map(|key| key.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Snapshot {
            keys: probe.atlases.len(),
            pages: probe.atlases.values().map(Vec::len).sum(),
            bytes: probe.atlases.total_bytes(&probe.images),
            font_ids,
        }
    }

    /// A locally-created `TextFont`, `ComputedTextBlock`, and layout info model
    /// one freshly spawned text entity. They are dropped after this call.
    fn recreate_arial_text(probe: &mut Probe) -> Result<usize, String> {
        let text_font = TextFont {
            font: FontSource::Family(ARIAL.into()),
            font_size: FontSize::Px(32.0),
            ..Default::default()
        };
        let mut computed = ComputedTextBlock::default();
        let mut layout_info = TextLayoutInfo::default();

        probe
            .pipeline
            .update_buffer(
                &probe.fonts,
                std::iter::once((
                    Entity::PLACEHOLDER,
                    0,
                    SAMPLE,
                    &text_font,
                    Color::WHITE,
                    LineHeight::default(),
                    LetterSpacing::default(),
                )),
                LineBreak::WordBoundary,
                Justify::Left,
                TextBounds::UNBOUNDED,
                1.0,
                &mut computed,
                &mut probe.font_cx,
                &mut probe.layout_cx,
                Vec2::new(1920.0, 1080.0),
                16.0,
            )
            .map_err(|error| format!("Arial layout failed: {error}"))?;
        probe
            .pipeline
            .update_text_layout_info(
                &mut layout_info,
                &mut probe.atlases,
                &mut probe.images,
                &mut computed,
                &mut probe.scale_cx,
                TextBounds::UNBOUNDED,
                Justify::Left,
                FontHinting::default(),
            )
            .map_err(|error| format!("Arial atlas update failed: {error}"))?;

        Ok(layout_info.glyphs.len())
    }

    fn run() -> Result<(), String> {
        let mut probe = Probe::default();
        if probe.font_cx.collection.family_id(ARIAL).is_none() {
            eprintln!("SKIP Arial is not present in this system font collection.");
            return Ok(());
        }

        let glyphs = recreate_arial_text(&mut probe)?;
        let warm = snapshot(&probe);
        if glyphs == 0 || warm.pages == 0 || warm.bytes == 0 {
            eprintln!(
                "SKIP Arial resolved but did not populate a text atlas: glyphs={glyphs}, snapshot={warm:?}"
            );
            return Ok(());
        }

        for _ in 0..CYCLES {
            recreate_arial_text(&mut probe)?;
        }
        let continuous = snapshot(&probe);
        if continuous != warm {
            return Err(format!(
                "continuous recreation changed the atlas after {CYCLES} cycles: warm={warm:?}, continuous={continuous:?}"
            ));
        }

        // Bevy's TextPlugin calls the same SourceCache prune operation in Last.
        // No text is laid out for these passes, so this models an all-Arial absence.
        for _ in 0..INACTIVE_PRUNES {
            probe.font_cx.source_cache.prune(2, false);
        }
        let before_recreate = snapshot(&probe);
        let glyphs_after_absence = recreate_arial_text(&mut probe)?;
        let after_recreate = snapshot(&probe);
        if glyphs_after_absence == 0 || after_recreate.pages == 0 {
            eprintln!(
                "SKIP Arial did not repopulate an atlas after absence: glyphs={glyphs_after_absence}, snapshot={after_recreate:?}"
            );
            return Ok(());
        }

        println!("continuous_arial_recreation={continuous:?}");
        println!(
            "after_{INACTIVE_PRUNES}_inactive_prunes_before_recreate={before_recreate:?}"
        );
        println!("after_recreate={after_recreate:?}");
        println!(
            "RESULT continuous recreation remained bounded; the second pair reports whether source-cache eviction creates a retained atlas key."
        );
        Ok(())
    }

    pub fn main() {
        if let Err(error) = run() {
            eprintln!("FAIL font_atlas_probe: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "native-ui")]
fn main() {
    native::main();
}
