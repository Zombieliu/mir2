//! Source-backed Crystal text defaults for the native UI.
//!
//! Crystal's `Settings.FontName` defaults to Arial and `MirLabel` defaults to
//! 8 points. At the 96-DPI logical stage used by the native client that is
//! 10.666... pixels. Callers still choose the source-specific point/px size,
//! but must not silently fall back to Bevy's bundled monospaced font.

use bevy::prelude::{FontSize, FontSource, TextFont};

pub const CRYSTAL_DEFAULT_FONT_FAMILY: &str = "Arial";
pub const CRYSTAL_DEFAULT_FONT_SIZE_PX: f32 = 8.0 * 96.0 / 72.0;

pub fn crystal_text_font(font_size_px: f32) -> TextFont {
    TextFont {
        font: FontSource::Family(CRYSTAL_DEFAULT_FONT_FAMILY.into()),
        font_size: FontSize::Px(font_size_px),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_default_matches_source_arial_eight_point_at_96_dpi() {
        assert_eq!(CRYSTAL_DEFAULT_FONT_FAMILY, "Arial");
        assert!((CRYSTAL_DEFAULT_FONT_SIZE_PX - 10.666_667).abs() < 0.000_01);
        assert_eq!(
            crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX).font,
            FontSource::Family("Arial".into())
        );
    }
}
