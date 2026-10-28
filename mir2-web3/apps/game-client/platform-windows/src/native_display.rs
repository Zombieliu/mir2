//! Per-user display preference and read-only monitor geometry.
//! No display settings are written into the signed game package.

use bevy::{prelude::*, winit::WINIT_WINDOWS};
use mir2_client_bevy::native_display::{
    DisplayEnvironment, NativeDisplaySettings, NativeDisplayUiPlugin, ResolutionChoice,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const LIMIT: u64 = 256;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preference {
    schema: u8,
    resolution: String,
}

fn parse_preference(bytes: &[u8]) -> Option<ResolutionChoice> {
    if bytes.len() as u64 > LIMIT {
        return None;
    }
    let value: Preference = serde_json::from_slice(bytes).ok()?;
    if value.schema != 1 {
        return None;
    }
    ResolutionChoice::from_code(&value.resolution)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
fn safe_ancestors(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if is_link(&metadata) => {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
fn read_preference(directory: &Path) -> Option<ResolutionChoice> {
    let file = directory.join("display.json");
    safe_ancestors(&file).ok()?;
    let metadata = fs::symlink_metadata(&file).ok()?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(&file)
        .ok()?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_preference(&bytes)
}
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
        }
        let source = source
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // Same-directory atomic replacement; never use copy or delete/rename.
        if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 1 | 8) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, destination)
    }
}
fn write_preference(directory: &Path, choice: ResolutionChoice) -> io::Result<()> {
    safe_ancestors(directory)?;
    fs::create_dir_all(directory)?;
    safe_ancestors(directory)?;
    let destination = directory.join("display.json");
    safe_ancestors(&destination)?;
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if !metadata.is_file() {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
    }
    let bytes = serde_json::to_vec(&Preference {
        schema: 1,
        resolution: choice.code().into(),
    })?;
    let temp = directory.join(format!(
        ".display-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let outcome = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp, &destination)
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&temp);
    }
    outcome
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MonitorGeometry {
    pub environment: DisplayEnvironment,
    origin: IVec2,
    work_size: UVec2,
    chrome: UVec2,
}
impl MonitorGeometry {
    fn new(origin: IVec2, work_size: UVec2, chrome: UVec2, dpi: f32) -> Self {
        let margin = (8.0 * dpi.clamp(0.5, 4.0)).ceil() as u32;
        let available = work_size
            .saturating_sub(chrome)
            .saturating_sub(UVec2::splat(margin * 2));
        Self {
            origin,
            work_size,
            chrome,
            environment: DisplayEnvironment {
                available,
                os_scale_factor: dpi,
            }
            .normalized(),
        }
    }
    pub fn centered_outer_position(self, client: UVec2) -> IVec2 {
        let outer = client.saturating_add(self.chrome);
        self.origin + (self.work_size.saturating_sub(outer) / 2).as_ivec2()
    }
}

#[cfg(windows)]
mod monitor_api {
    use super::*;
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MonitorFromWindow(window: *mut c_void, flags: u32) -> *mut c_void;
        fn MonitorFromPoint(point: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn GetDpiForSystem() -> u32;
        fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
    }
    fn work_area(monitor: *mut c_void) -> Option<(IVec2, UVec2)> {
        if monitor.is_null() {
            return None;
        }
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return None;
        }
        let width = info.work.right.checked_sub(info.work.left)?;
        let height = info.work.bottom.checked_sub(info.work.top)?;
        if width <= 0 || height <= 0 {
            return None;
        }
        Some((
            IVec2::new(info.work.left, info.work.top),
            UVec2::new(width as u32, height as u32),
        ))
    }
    pub fn primary() -> Option<MonitorGeometry> {
        let monitor = unsafe { MonitorFromPoint(Point { x: 0, y: 0 }, 1) }; // Default primary.
        let (origin, size) = work_area(monitor)?;
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        // Conservative nonclient bounds before a native window exists. After
        // creation we use its measured outer-inner difference instead.
        let metric = |index| unsafe { GetSystemMetricsForDpi(index, dpi) }.max(0) as u32;
        let border_x = metric(32) + metric(92);
        let border_y = metric(33) + metric(92);
        let chrome = UVec2::new(border_x * 2, border_y * 2 + metric(4));
        Some(MonitorGeometry::new(
            origin,
            size,
            chrome,
            dpi as f32 / 96.0,
        ))
    }
    pub fn window(window: &winit::window::Window) -> Option<MonitorGeometry> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let monitor = unsafe { MonitorFromWindow(handle.hwnd.get() as *mut c_void, 2) };
        let (origin, size) = work_area(monitor)?;
        let outer = window.outer_size();
        let inner = window.inner_size();
        let chrome = UVec2::new(
            outer.width.saturating_sub(inner.width),
            outer.height.saturating_sub(inner.height),
        );
        Some(MonitorGeometry::new(
            origin,
            size,
            chrome,
            window.scale_factor() as f32,
        ))
    }
}

fn primary_geometry() -> MonitorGeometry {
    #[cfg(windows)]
    {
        if let Some(geometry) = monitor_api::primary() {
            return geometry;
        }
    }
    MonitorGeometry::new(IVec2::ZERO, UVec2::new(1280, 960), UVec2::new(16, 48), 1.0)
}
pub(crate) fn geometry_for_window(entity: Entity) -> Option<MonitorGeometry> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        #[cfg(windows)]
        {
            if let Some(geometry) = monitor_api::window(window) {
                return Some(geometry);
            }
        }
        let monitor = window.current_monitor()?;
        let size = monitor.size();
        let position = monitor.position();
        let outer = window.outer_size();
        let inner = window.inner_size();
        Some(MonitorGeometry::new(
            IVec2::new(position.x, position.y),
            UVec2::new(size.width, size.height),
            UVec2::new(
                outer.width.saturating_sub(inner.width),
                outer.height.saturating_sub(inner.height),
            ),
            monitor.scale_factor() as f32,
        ))
    })
}

#[derive(Resource)]
struct Persistence {
    directory: Option<PathBuf>,
    observed_choice: Option<ResolutionChoice>,
}
fn persist(mut state: ResMut<Persistence>, settings: Res<NativeDisplaySettings>) {
    if state.observed_choice == Some(settings.choice) {
        return;
    }
    state.observed_choice = Some(settings.choice);
    if let Some(directory) = &state.directory {
        if write_preference(directory, settings.choice).is_err() {
            eprintln!("[native-display] preference could not be saved; using in-memory selection");
        }
    }
}

pub(crate) fn install(app: &mut App) {
    let directory = std::env::var_os("APPDATA")
        .map(|path| PathBuf::from(path).join("mir2-web3"))
        .filter(|path| safe_ancestors(path).is_ok());
    let choice = directory
        .as_deref()
        .and_then(read_preference)
        .unwrap_or(ResolutionChoice::Auto);
    let geometry = primary_geometry();
    let settings = NativeDisplaySettings::new(choice, geometry.environment);
    let size = settings.size();
    let world = app.world_mut();
    for mut window in world.query::<&mut bevy::window::Window>().iter_mut(world) {
        window.resolution.set_scale_factor_override(Some(1.0));
        window.resolution.set_physical_resolution(size.x, size.y);
        window.position = bevy::window::WindowPosition::At(geometry.centered_outer_position(size));
    }
    app.insert_resource(UiScale(settings.scale()))
        .insert_resource(settings)
        .insert_resource(Persistence {
            directory,
            observed_choice: None,
        })
        .add_plugins(NativeDisplayUiPlugin)
        .add_systems(
            PreUpdate,
            crate::window_sizing::sync_display_settings.before(bevy::ui::UiSystems::Focus),
        )
        .add_systems(Last, persist);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_rejects_corruption_unknown_choices_and_extra_fields() {
        for invalid in [
            b"invalid".as_slice(),
            br#"{"schema":2,"resolution":"auto"}"#,
            br#"{"schema":1,"resolution":"1920x1080"}"#,
            br#"{"schema":1,"resolution":"AUTO"}"#,
            br#"{"schema":1,"resolution":"auto","password":"x"}"#,
            br#"{"schema":1,"resolution":"auto","resolution":"1024x768"}"#,
        ] {
            assert!(parse_preference(invalid).is_none());
        }
        assert!(parse_preference(&vec![b' '; 257]).is_none());
    }
    #[test]
    fn all_choices_round_trip_atomically_and_bad_file_recovers_auto() {
        let root = std::env::temp_dir().join(format!(
            "mir2-display-preference-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for choice in ResolutionChoice::ALL {
            write_preference(&root, choice).unwrap();
            assert_eq!(read_preference(&root), Some(choice));
            assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        }
        fs::write(root.join("display.json"), b"broken").unwrap();
        assert_eq!(
            read_preference(&root).unwrap_or(ResolutionChoice::Auto),
            ResolutionChoice::Auto
        );
        write_preference(&root, ResolutionChoice::Auto).unwrap();
        assert_eq!(read_preference(&root), Some(ResolutionChoice::Auto));
        fs::remove_file(root.join("display.json")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn frame_taskbar_and_margin_are_subtracted_before_resolution_availability() {
        let geometry = MonitorGeometry::new(
            IVec2::new(-1920, 0),
            UVec2::new(1920, 1040),
            UVec2::new(16, 39),
            1.5,
        );
        let size = NativeDisplaySettings::new(ResolutionChoice::Auto, geometry.environment).size();
        assert_eq!(size, UVec2::new(1280, 960));
        let position = geometry.centered_outer_position(size);
        assert!(position.x >= -1920 && position.y >= 0);
        assert!(position.x + size.x as i32 + 16 <= 0);
        assert!(position.y + size.y as i32 + 39 <= 1040);
    }
}
