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
    const CJK_WRAP_WIDTH: f32 = 96.0;

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
        recreate_weighted_arial_text(probe, false)
    }

    fn recreate_weighted_arial_text(probe: &mut Probe, bold: bool) -> Result<usize, String> {
        recreate_family_text(probe, ARIAL, SAMPLE, bold)
    }

    fn recreate_family_text(probe: &mut Probe, family: &str, sample: &str, bold: bool) -> Result<usize, String> {
        let text_font = TextFont {
            font: FontSource::Family(family.into()),
            font_size: FontSize::Px(32.0),
            weight: if bold { bevy::text::FontWeight::BOLD } else { bevy::text::FontWeight::NORMAL },
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
                    sample,
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

    // This intentionally uses WordOrCharacter rather than avoiding the CJK
    // segmentation path. ICU4X may emit its debug-only missing-Japanese-model
    // diagnostic for Han text; the assertion verifies the Bevy layout fallback
    // still wraps complete Chinese glyph output inside a constrained quest-like
    // width.
    fn verify_cjk_word_or_character_wrap(
        probe: &mut Probe,
        family: &str,
        sample: &str,
        expected_glyphs: usize,
    ) -> Result<(), String> {
        let text_font = TextFont {
            font: FontSource::Family(family.into()),
            font_size: FontSize::Px(14.0),
            ..Default::default()
        };
        let bounds = TextBounds::new_horizontal(CJK_WRAP_WIDTH);
        let mut computed = ComputedTextBlock::default();
        let mut layout_info = TextLayoutInfo::default();

        probe
            .pipeline
            .update_buffer(
                &probe.fonts,
                std::iter::once((
                    Entity::PLACEHOLDER,
                    0,
                    sample,
                    &text_font,
                    Color::WHITE,
                    LineHeight::default(),
                    LetterSpacing::default(),
                )),
                LineBreak::WordOrCharacter,
                Justify::Left,
                bounds,
                1.0,
                &mut computed,
                &mut probe.font_cx,
                &mut probe.layout_cx,
                Vec2::new(1920.0, 1080.0),
                16.0,
            )
            .map_err(|error| format!("CJK WordOrCharacter layout failed: {error}"))?;
        probe
            .pipeline
            .update_text_layout_info(
                &mut layout_info,
                &mut probe.atlases,
                &mut probe.images,
                &mut computed,
                &mut probe.scale_cx,
                bounds,
                Justify::Left,
                FontHinting::default(),
            )
            .map_err(|error| format!("CJK WordOrCharacter atlas update failed: {error}"))?;

        let line_count = layout_info
            .glyphs
            .iter()
            .map(|glyph| glyph.line_index)
            .collect::<BTreeSet<_>>()
            .len();
        if layout_info.glyphs.len() != expected_glyphs {
            return Err(format!(
                "CJK WordOrCharacter lost glyphs: expected {expected_glyphs}, got {}",
                layout_info.glyphs.len()
            ));
        }
        if line_count < 2 {
            return Err(format!(
                "CJK WordOrCharacter did not wrap at {CJK_WRAP_WIDTH}px: lines={line_count}, size={:?}",
                layout_info.size
            ));
        }
        if layout_info.size.x > CJK_WRAP_WIDTH + 0.01 {
            return Err(format!(
                "CJK WordOrCharacter overflowed {CJK_WRAP_WIDTH}px: size={:?}",
                layout_info.size
            ));
        }
        let max_glyph_x = layout_info
            .glyphs
            .iter()
            .map(|glyph| glyph.position.x)
            .fold(f32::NEG_INFINITY, f32::max);
        if max_glyph_x > CJK_WRAP_WIDTH + 1.0 {
            return Err(format!(
                "CJK WordOrCharacter placed a glyph past {CJK_WRAP_WIDTH}px: max_x={max_glyph_x}"
            ));
        }

        println!(
            "cjk_word_or_character_wrap=lines={line_count}, glyphs={}, max_glyph_x={max_glyph_x}, size={:?}",
            layout_info.glyphs.len(),
            layout_info.size
        );
        Ok(())
    }

    fn run() -> Result<(), String> {
        let mut probe = Probe::default();
        if probe.font_cx.collection.family_id(ARIAL).is_none() {
            eprintln!("SKIP Arial is not present in this system font collection.");
            return Ok(());
        }

        let glyphs = recreate_arial_text(&mut probe)?;
        recreate_weighted_arial_text(&mut probe, true)?;
        let warm = snapshot(&probe);
        let original_pixels: Vec<_> = probe.images.iter().map(|(_, image)| image.data.clone()).collect();
        if glyphs == 0 || warm.pages == 0 || warm.bytes == 0 {
            eprintln!(
                "SKIP Arial resolved but did not populate a text atlas: glyphs={glyphs}, snapshot={warm:?}"
            );
            return Ok(());
        }

        for _ in 0..CYCLES {
            recreate_arial_text(&mut probe)?;
            recreate_weighted_arial_text(&mut probe, true)?;
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

        // Match the candidate's memory-backed font registration while leaving
        // the public Family("Arial") lookup and system fallback enabled.
        let Some(windows) = std::env::var_os("WINDIR") else {
            eprintln!("SKIP pinned-font comparison: WINDIR is unavailable.");
            return Ok(());
        };
        let mut pinned = Probe::default();
        let mut retained_handles = Vec::new();
        for file in ["arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"] {
            let path = std::path::PathBuf::from(&windows).join("Fonts").join(file);
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("reading installed Arial {}: {error}", path.display()))?;
            let font = Font::from_bytes(bytes);
            let registered = pinned.font_cx.collection.register_fonts(font.data.clone(), None);
            if registered.is_empty() {
                return Err("installed Arial bytes did not register any font family".into());
            }
            retained_handles.push(pinned.fonts.add(font));
        }
        let pinned_glyphs = recreate_arial_text(&mut pinned)?;
        recreate_weighted_arial_text(&mut pinned, true)?;
        if pinned_glyphs != glyphs {
            return Err(format!("pinned Arial changed glyph count: {glyphs} -> {pinned_glyphs}"));
        }
        let pinned_warm = snapshot(&pinned);
        let pinned_pixels: Vec<_> = pinned.images.iter().map(|(_, image)| image.data.clone()).collect();
        if pinned_pixels != original_pixels {
            return Err("pinned Arial changed the sample's raster atlas pixels".into());
        }
        for cycle in 0..CYCLES {
            for _ in 0..INACTIVE_PRUNES {
                pinned.font_cx.source_cache.prune(2, false);
            }
            recreate_arial_text(&mut pinned)?;
            recreate_weighted_arial_text(&mut pinned, true)?;
            if snapshot(&pinned) != pinned_warm {
                return Err(format!("pinned Arial atlas grew after absence cycle {cycle}"));
            }
        }
        println!("pinned_arial_after_{CYCLES}_absence_cycles={:?}", snapshot(&pinned));
        println!("RESULT memory-backed Arial remained bounded with identical sample glyph count and raster atlas pixels; live attribution and whole-UI visual equivalence remain separate gates.");

        // Quest guidance uses YaHei rather than Arial; exercise its TTC faces
        // with actual Chinese glyphs, not just Latin family-name resolution.
        let family = "Microsoft YaHei";
        let sample = "任务目标：前往比奇安全区，击败钉耙猫。0123456789";
        let mut chinese_system = Probe::default();
        if chinese_system.font_cx.collection.family_id(family).is_none() {
            eprintln!("SKIP YaHei comparison: system family unavailable.");
            return Ok(());
        }
        let regular_glyphs = recreate_family_text(&mut chinese_system, family, sample, false)?;
        let bold_glyphs = recreate_family_text(&mut chinese_system, family, sample, true)?;
        let chinese_pixels: Vec<_> = chinese_system.images.iter().map(|(_, image)| image.data.clone()).collect();
        let mut chinese_pinned = Probe::default();
        let mut chinese_handles = Vec::new();
        for file in ["msyh.ttc", "msyhbd.ttc", "msyhl.ttc"] {
            let path = std::path::PathBuf::from(&windows).join("Fonts").join(file);
            let font = Font::from_bytes(std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?);
            if chinese_pinned.font_cx.collection.register_fonts(font.data.clone(), None).is_empty() {
                return Err(format!("no families registered from {file}"));
            }
            chinese_handles.push(chinese_pinned.fonts.add(font));
        }
        if recreate_family_text(&mut chinese_pinned, family, sample, false)? != regular_glyphs
            || recreate_family_text(&mut chinese_pinned, family, sample, true)? != bold_glyphs {
            return Err("pinned YaHei changed sample glyph count".into());
        }
        let pinned_chinese_pixels: Vec<_> = chinese_pinned.images.iter().map(|(_, image)| image.data.clone()).collect();
        if chinese_pixels != pinned_chinese_pixels || regular_glyphs == 0 || bold_glyphs == 0 {
            return Err("pinned YaHei raster pixels differ or glyph sample is empty".into());
        }
        verify_cjk_word_or_character_wrap(&mut chinese_system, family, sample, regular_glyphs)?;
        let chinese_warm = snapshot(&chinese_pinned);
        for cycle in 0..CYCLES {
            for _ in 0..INACTIVE_PRUNES { chinese_pinned.font_cx.source_cache.prune(2, false); }
            recreate_family_text(&mut chinese_pinned, family, sample, false)?;
            recreate_family_text(&mut chinese_pinned, family, sample, true)?;
            if snapshot(&chinese_pinned) != chinese_warm {
                return Err(format!("pinned YaHei atlas grew after absence cycle {cycle}"));
            }
        }
        println!("pinned_yahei_after_{CYCLES}_absence_cycles={:?}", snapshot(&chinese_pinned));
        println!("RESULT Chinese regular/bold sample raster pixels match system YaHei and remain bounded.");
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
