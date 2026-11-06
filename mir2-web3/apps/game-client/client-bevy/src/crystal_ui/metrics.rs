//! Logical-stage transforms shared by Crystal-native UI and pointer hit tests.

use super::spec::{STAGE_HEIGHT, STAGE_WIDTH};

/// Live OS scale-factor changes (moving the window onto a monitor with a
/// different DPI) are not applied in-process. Restart the client so window
/// creation picks up the new scale. A normal resize still refits letterbox
/// from the current physical viewport.
pub const LIVE_CROSS_MONITOR_DPI_REQUIRES_RESTART: bool = true;

/// Uniform fit from Crystal's fixed 1024x768 logical stage into a client area.
///
/// The same transform must be used for drawing, pointer input, screenshots, and
/// DPI changes. Keeping it as pure data makes those contracts testable without
/// opening a Bevy window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalStageTransform {
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

impl CrystalStageTransform {
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        offset_x: 0.0,
        offset_y: 0.0,
        viewport_width: STAGE_WIDTH,
        viewport_height: STAGE_HEIGHT,
    };

    /// Fit the logical stage without changing its aspect ratio.
    pub fn fit(viewport_width: f32, viewport_height: f32) -> Self {
        let viewport_width = viewport_width.max(1.0);
        let viewport_height = viewport_height.max(1.0);
        let scale = (viewport_width / STAGE_WIDTH)
            .min(viewport_height / STAGE_HEIGHT)
            .max(f32::EPSILON);
        let drawn_width = STAGE_WIDTH * scale;
        let drawn_height = STAGE_HEIGHT * scale;
        Self {
            scale,
            offset_x: (viewport_width - drawn_width) * 0.5,
            offset_y: (viewport_height - drawn_height) * 0.5,
            viewport_width,
            viewport_height,
        }
    }

    pub fn logical_to_physical(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.offset_x + x * self.scale,
            self.offset_y + y * self.scale,
        )
    }

    pub fn physical_to_logical(self, x: f32, y: f32) -> (f32, f32) {
        (
            (x - self.offset_x) / self.scale,
            (y - self.offset_y) / self.scale,
        )
    }

    pub fn contains_physical_point(self, x: f32, y: f32) -> bool {
        let (logical_x, logical_y) = self.physical_to_logical(x, y);
        (0.0..STAGE_WIDTH).contains(&logical_x) && (0.0..STAGE_HEIGHT).contains(&logical_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f32, right: f32) {
        assert!((left - right).abs() < 0.001, "{left} != {right}");
    }

    #[test]
    fn native_baseline_is_identity() {
        assert_eq!(
            CrystalStageTransform::fit(STAGE_WIDTH, STAGE_HEIGHT),
            CrystalStageTransform::IDENTITY
        );
    }

    #[test]
    fn wide_viewport_letterboxes_horizontally() {
        let transform = CrystalStageTransform::fit(1280.0, 720.0);
        assert_close(transform.scale, 0.9375);
        assert_close(transform.offset_x, 160.0);
        assert_close(transform.offset_y, 0.0);
        assert_eq!(transform.logical_to_physical(0.0, 0.0), (160.0, 0.0));
        assert_eq!(
            transform.logical_to_physical(STAGE_WIDTH, STAGE_HEIGHT),
            (1120.0, 720.0)
        );
    }

    #[test]
    fn dpi_scaled_stage_preserves_logical_coordinates() {
        for dpi_scale in [1.0, 1.25, 1.5] {
            let transform =
                CrystalStageTransform::fit(STAGE_WIDTH * dpi_scale, STAGE_HEIGHT * dpi_scale);
            assert_close(transform.scale, dpi_scale);
            let physical = transform.logical_to_physical(575.0, 355.0);
            let logical = transform.physical_to_logical(physical.0, physical.1);
            assert_close(logical.0, 575.0);
            assert_close(logical.1, 355.0);
        }
    }

    #[test]
    fn letterbox_is_not_an_interactive_stage_region() {
        let transform = CrystalStageTransform::fit(1280.0, 720.0);
        assert!(!transform.contains_physical_point(100.0, 300.0));
        assert!(transform.contains_physical_point(160.0, 0.0));
        assert!(transform.contains_physical_point(1119.99, 719.99));
        assert!(!transform.contains_physical_point(1120.0, 720.0));
        assert_eq!(STAGE_WIDTH, 1024.0);
        assert_eq!(STAGE_HEIGHT, 768.0);
    }

    fn dpi_profiles() -> [(f32, f32, f32); 3] {
        [
            (1.0, 1024.0, 768.0),
            (1.25, 1280.0, 960.0),
            (1.5, 1536.0, 1152.0),
        ]
    }

    fn round_trip_error(transform: CrystalStageTransform, x: f32, y: f32) -> f32 {
        let physical = transform.logical_to_physical(x, y);
        let logical = transform.physical_to_logical(physical.0, physical.1);
        (logical.0 - x).abs().max((logical.1 - y).abs())
    }

    fn assert_hit_round_trip(
        transform: CrystalStageTransform,
        rect: crate::crystal_ui::spec::CrystalRect,
    ) {
        assert!(rect.is_valid_hit_target(), "{rect:?}");
        let (cx, cy) = rect.center();
        assert!(
            round_trip_error(transform, cx, cy) <= 2.0,
            "round-trip {cx},{cy} scale={}",
            transform.scale
        );
        let physical = transform.logical_to_physical(cx, cy);
        let logical = transform.physical_to_logical(physical.0, physical.1);
        assert!(
            rect.contains(logical.0, logical.1),
            "{rect:?} missed {logical:?}"
        );
        assert!(physical.0.is_finite() && physical.1.is_finite());
    }

    #[test]
    fn dpi_profiles_preserve_logical_stage_and_control_hits() {
        use crate::crystal_ui::spec::{character_select, hud, login, CrystalRect};

        let bag = CrystalRect::new(16.0, 170.0, 360.0, 360.0);
        let npc_dialog = CrystalRect::new(310.0, 90.0, 404.0, 180.0);
        for (scale, width, height) in dpi_profiles() {
            let transform = CrystalStageTransform::fit(width, height);
            assert_close(transform.scale, scale);
            assert_eq!(STAGE_WIDTH, 1024.0);
            assert_eq!(STAGE_HEIGHT, 768.0);
            assert_hit_round_trip(transform, login::ACCOUNT_FIELD);
            assert_hit_round_trip(transform, login::PASSWORD_FIELD);
            assert_hit_round_trip(transform, login::OK.rect);
            assert_hit_round_trip(
                transform,
                CrystalRect::new(
                    character_select::SLOT_LEFT,
                    character_select::SLOT_TOPS[0],
                    character_select::SLOT_WIDTH,
                    character_select::OCCUPIED_SLOT_HEIGHT,
                ),
            );
            assert_hit_round_trip(transform, character_select::START.rect);
            assert_hit_round_trip(transform, hud::MAIN.rect);
            assert_hit_round_trip(transform, hud::MINIMAP.rect);
            assert_hit_round_trip(transform, hud::INVENTORY.rect);
            assert_hit_round_trip(transform, bag);
            assert_hit_round_trip(transform, npc_dialog);
        }
    }

    #[test]
    fn invalid_rects_and_viewports_never_produce_nan_or_zero_hits() {
        let zero = crate::crystal_ui::spec::CrystalRect::new(10.0, 10.0, 0.0, 20.0);
        let negative = crate::crystal_ui::spec::CrystalRect::new(10.0, 10.0, 20.0, -4.0);
        let nan = crate::crystal_ui::spec::CrystalRect::new(f32::NAN, 0.0, 10.0, 10.0);
        assert!(!zero.is_valid_hit_target());
        assert!(!negative.is_valid_hit_target());
        assert!(!nan.is_valid_hit_target());

        let tiny = CrystalStageTransform::fit(0.0, 0.0);
        assert!(tiny.scale.is_finite() && tiny.scale > 0.0);
        let (px, py) = tiny.logical_to_physical(512.0, 384.0);
        assert!(px.is_finite() && py.is_finite());
        let (lx, ly) = tiny.physical_to_logical(px, py);
        assert!(lx.is_finite() && ly.is_finite());
        assert!((lx - 512.0).abs() <= 2.0);
        assert!((ly - 384.0).abs() <= 2.0);
    }

    #[test]
    fn resized_window_letterbox_keeps_logical_stage() {
        let transform = CrystalStageTransform::fit(800.0, 600.0);
        assert!(transform.scale < 1.0);
        assert_eq!(STAGE_WIDTH, 1024.0);
        assert_eq!(STAGE_HEIGHT, 768.0);
        let (cx, cy) = crate::crystal_ui::spec::login::OK.rect.center();
        assert!(round_trip_error(transform, cx, cy) <= 2.0);
        assert!(transform.contains_physical_point(
            transform.logical_to_physical(cx, cy).0,
            transform.logical_to_physical(cx, cy).1
        ));
    }

    #[test]
    fn world_and_ui_hits_use_a_single_stage_transform() {
        let transform = CrystalStageTransform::fit(1536.0, 1152.0);
        let physical = transform.logical_to_physical(288.0, 616.0);
        let once = transform.physical_to_logical(physical.0, physical.1);
        assert!((once.0 - 288.0).abs() <= 2.0);
        assert!((once.1 - 616.0).abs() <= 2.0);
        let doubled = transform.physical_to_logical(once.0, once.1);
        assert!(
            (doubled.0 - 288.0).abs() > 2.0,
            "applying the stage transform twice would offset world clicks"
        );
        assert!(LIVE_CROSS_MONITOR_DPI_REQUIRES_RESTART);
    }

    #[test]
    fn hud_and_minimap_stay_inside_the_logical_stage() {
        use crate::crystal_ui::spec::hud;
        for rect in [
            hud::MAIN.rect,
            hud::MINIMAP.rect,
            hud::INVENTORY.rect,
            hud::HEALTH_ORB.rect,
        ] {
            assert!(rect.is_valid_hit_target(), "{rect:?}");
            assert!(rect.left >= 0.0 && rect.top >= 0.0, "{rect:?}");
            assert!(
                rect.left + rect.width <= STAGE_WIDTH + 2.0,
                "minimap may sit on the Crystal 1024 edge: {rect:?}"
            );
            assert!(rect.top + rect.height <= STAGE_HEIGHT + 2.0, "{rect:?}");
        }
    }
}
