//! Pixel-exact native Crystal viewport setup.
//!
//! Bevy 0.19 forwards WindowResizeConstraints to Winit as *OS logical* sizes,
//! even when WindowResolution has a scale-factor override. A 1024 minimum at
//! 150% DPI therefore enlarges the client area to 1536 physical pixels while
//! our pixel-authored HUD stays 1024 wide. Nonresizable + disabled maximize
//! already enforce the fixed-window product mode; logical min/max locks must
//! not be used as a second, contradictory physical-size constraint.
use bevy::prelude::{MessageReader, Query};
use bevy::window::{Window, WindowResizeConstraints};
use bevy::window::{WindowBackendScaleFactorChanged, WindowResized};

pub(crate) fn configure(window: &mut Window) {
    window.resizable = false;
    window.enabled_buttons.maximize = false;
    window.resolution.set_scale_factor_override(Some(1.0));
    window.resolution.set_physical_resolution(
        crate::session_config::DEFAULT_WINDOW_WIDTH,
        crate::session_config::DEFAULT_WINDOW_HEIGHT,
    );
    window.resize_constraints = WindowResizeConstraints {
        min_width: 1.0,
        min_height: 1.0,
        max_width: f32::INFINITY,
        max_height: f32::INFINITY,
    };
}

/// Winit may apply the OS suggested rectangle when moving between monitors.
/// Restore the same pixel viewport only in response to a real resize/DPI event;
/// do not continuously resize the window or relayout the HUD on ordinary frames.
pub(crate) fn keep_pixel_viewport(
    mut resized: MessageReader<WindowResized>,
    mut dpi: MessageReader<WindowBackendScaleFactorChanged>,
    mut windows: Query<&mut Window>,
) {
    let affected: std::collections::HashSet<_> = resized
        .read()
        .map(|e| e.window)
        .chain(dpi.read().map(|e| e.window))
        .collect();
    for entity in affected {
        if let Ok(mut window) = windows.get_mut(entity) {
            if window.resolution.scale_factor_override() == Some(1.0)
                && !window.resizable
                && (window.physical_width() != crate::session_config::DEFAULT_WINDOW_WIDTH
                    || window.physical_height() != crate::session_config::DEFAULT_WINDOW_HEIGHT)
            {
                window.resolution.set_physical_resolution(
                    crate::session_config::DEFAULT_WINDOW_WIDTH,
                    crate::session_config::DEFAULT_WINDOW_HEIGHT,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Update};
    use winit::dpi::LogicalSize;

    #[test]
    fn fixed_os_logical_constraints_reproduce_the_reported_two_thirds_hud() {
        // These are the actual conversion semantics used by Winit, not the
        // client's CrystalStageTransform (which never caused this resize).
        let minimum = LogicalSize::new(1024.0, 768.0).to_physical::<u32>(1.5);
        assert_eq!((minimum.width, minimum.height), (1536, 1152));
        assert!((1024.0 / minimum.width as f32 - 2.0 / 3.0).abs() < 0.0001);
    }

    #[test]
    fn pixel_viewport_is_not_enlarged_by_os_dpi_constraints() {
        for dpi in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0] {
            let mut window = Window::default();
            window.resolution.set_scale_factor(dpi as f32);
            configure(&mut window);
            let constraints = window.resize_constraints.check_constraints();
            let minimum = LogicalSize::new(constraints.min_width, constraints.min_height)
                .to_physical::<u32>(dpi);
            assert!(minimum.width <= window.physical_width());
            assert!(minimum.height <= window.physical_height());
            assert!(!constraints.max_width.is_finite());
            assert!(!constraints.max_height.is_finite());
            assert_eq!(
                (window.physical_width(), window.physical_height()),
                (1024, 768)
            );
            assert_eq!((window.width(), window.height()), (1024.0, 768.0));
            assert!(!window.resizable && !window.enabled_buttons.maximize);
        }
    }

    #[test]
    fn monitor_dpi_resize_keeps_one_pixel_viewport_without_a_resize_loop() {
        let mut app = App::new();
        app.add_message::<WindowResized>()
            .add_message::<WindowBackendScaleFactorChanged>()
            .add_systems(Update, keep_pixel_viewport);
        let mut window = Window::default();
        configure(&mut window);
        let entity = app.world_mut().spawn(window).id();
        for factor in [1.25, 1.5, 2.0, 3.0, 1.0] {
            app.world_mut()
                .get_mut::<Window>(entity)
                .unwrap()
                .resolution
                .set_physical_resolution((1024.0 * factor) as u32, (768.0 * factor) as u32);
            app.world_mut().write_message(WindowResized {
                window: entity,
                width: 1024.0 * factor,
                height: 768.0 * factor,
            });
            app.world_mut()
                .write_message(WindowBackendScaleFactorChanged {
                    window: entity,
                    scale_factor: f64::from(factor),
                });
            app.update();
            let window = app.world().get::<Window>(entity).unwrap();
            assert_eq!(
                (window.physical_width(), window.physical_height()),
                (1024, 768)
            );
            app.update();
            assert_eq!(
                app.world().get::<Window>(entity).unwrap().physical_width(),
                1024
            );
        }
    }
}
