use bevy::ecs::system::NonSendMarker;
use bevy::prelude::{Entity, Local, Query, With};
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;
use winit::window::Icon;

pub(crate) const PRODUCT_NAME: &str = "numeron-legend of rebirth";

fn window_icon() -> Result<Icon, String> {
    let bytes = include_bytes!("../resources/numeron.png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|error| error.to_string())?;
    let mut rgba = vec![0; reader.output_buffer_size().ok_or("icon is too large")?];
    let frame = reader
        .next_frame(&mut rgba)
        .map_err(|error| error.to_string())?;
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err("window icon must be 8-bit RGBA".into());
    }
    rgba.truncate(frame.buffer_size());
    Icon::from_rgba(rgba, frame.width, frame.height).map_err(|error| error.to_string())
}

// Winit windows may not exist during Startup. Wait for the primary window and
// apply once on the main thread; embed the art so external packs are irrelevant.
pub(crate) fn apply_window_icon(
    primary: Query<Entity, With<PrimaryWindow>>,
    mut applied: Local<Option<Entity>>,
    _main_thread: NonSendMarker,
) {
    let Ok(entity) = primary.single() else { return };
    if *applied == Some(entity) {
        return;
    }
    WINIT_WINDOWS.with_borrow(|windows| {
        if let Some(window) = windows.get_window(entity) {
            match window_icon() {
                Ok(icon) => {
                    window.set_window_icon(Some(icon.clone()));
                    #[cfg(target_os = "windows")]
                    {
                        use winit::platform::windows::WindowExtWindows;
                        window.set_taskbar_icon(Some(icon));
                    }
                }
                Err(error) => eprintln!("[branding] window icon: {error}"),
            }
            *applied = Some(entity);
        }
    });
}
