//! Renderer-neutral Crystal horizontal HUD crop geometry.

use super::CrystalRect;

/// Keep the native f32 multiplication, inset, clamp, and floor order intact.
pub fn horizontal_bar_rect(frame: CrystalRect, ratio: f32, source_inset: f32) -> CrystalRect {
    CrystalRect::new(
        frame.left,
        frame.top,
        ((frame.width - source_inset).max(0.0) * ratio.clamp(0.0, 1.0)).floor(),
        frame.height,
    )
}

/// Original MainDialogs weight image selection, shared with the portable HUD.
pub fn weight_bar_asset(ratio: f32) -> (&'static str, u16) {
    if ratio <= 0.50 {
        ("Prguse", 76)
    } else if ratio <= 0.75 {
        ("UI_32bit", 473)
    } else {
        ("UI_32bit", 472)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_experience_crop_is_unchanged() {
        let frame = super::super::spec::hud::EXPERIENCE_BAR.rect;
        for (ratio, width) in [
            (0.0, 0.0),
            (0.5, 500.0),
            (1.0, 1001.0),
            (-1.0, 0.0),
            (2.0, 1001.0),
        ] {
            let crop = horizontal_bar_rect(frame, ratio, 3.0);
            assert_eq!(
                (crop.left, crop.top, crop.width, crop.height),
                (9.0, 759.0, width, 8.0)
            );
        }
    }
}
