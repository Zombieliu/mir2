use crate::camera::extract_cameras;
use crate::renderer::WgpuWrapper;
use crate::{
    render_resource::{SurfaceTexture, TextureView},
    renderer::{RenderAdapter, RenderDevice, RenderInstance},
    Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderSystems,
};
use bevy_app::{App, Plugin};
use bevy_ecs::entity::EntityHashSet;
use bevy_ecs::{entity::EntityHashMap, prelude::*};
use bevy_log::{debug, info, warn};
use bevy_utils::default;
use bevy_window::{
    AppLifecycle, CompositeAlphaMode, PresentMode, PrimaryWindow, RawHandleWrapper, Window,
    WindowClosing,
};
use core::{
    num::NonZero,
    ops::{Deref, DerefMut},
};
use wgpu::{
    SurfaceConfiguration, SurfaceTargetUnsafe, TextureFormat, TextureUsages, TextureViewDescriptor,
};

pub mod screenshot;

use screenshot::ScreenshotPlugin;

// Temporary, Android-only first-surface investigation. This does not change
// rendering, clear GL errors, or extend the existing camera startup delay.
#[cfg(target_os = "android")]
static ANDROID_SURFACE_PROBE_FRAME: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

#[cfg(target_os = "android")]
static ANDROID_SURFACE_PROBE_CURRENT_RAW: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

#[cfg(target_os = "android")]
static ANDROID_ATTACHMENT_TRANSITION_PROBED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

// Diagnostic only: reproduce an attachment-type transition on private GL
// objects. Never change a wgpu object, draw, allocate surface storage, or consume
// the GL error state. The caller already owns this device's current EGL context.
#[cfg(target_os = "android")]
unsafe fn trace_android_attachment_transition() {
    if ANDROID_ATTACHMENT_TRANSITION_PROBED.swap(true, core::sync::atomic::Ordering::Relaxed) {
        return;
    }
    #[link(name = "GLESv3")]
    unsafe extern "C" {
        fn glGetIntegerv(pname: u32, params: *mut i32);
        fn glGenFramebuffers(n: i32, buffers: *mut u32);
        fn glBindFramebuffer(target: u32, framebuffer: u32);
        fn glDeleteFramebuffers(n: i32, buffers: *const u32);
        fn glGenRenderbuffers(n: i32, buffers: *mut u32);
        fn glBindRenderbuffer(target: u32, renderbuffer: u32);
        fn glRenderbufferStorage(target: u32, internal: u32, width: i32, height: i32);
        fn glDeleteRenderbuffers(n: i32, buffers: *const u32);
        fn glGenTextures(n: i32, textures: *mut u32);
        fn glBindTexture(target: u32, texture: u32);
        fn glTexStorage2D(target: u32, levels: i32, internal: u32, width: i32, height: i32);
        fn glTexParameteri(target: u32, pname: u32, value: i32);
        fn glDeleteTextures(n: i32, textures: *const u32);
        fn glFramebufferRenderbuffer(target: u32, attachment: u32, kind: u32, buffer: u32);
        fn glFramebufferTexture2D(target: u32, attachment: u32, kind: u32, texture: u32, level: i32);
        fn glCheckFramebufferStatus(target: u32) -> u32;
        fn glGetFramebufferAttachmentParameteriv(target: u32, attachment: u32, pname: u32, value: *mut i32);
    }
    const DRAW: u32 = 0x8ca9;
    const RBO: u32 = 0x8d41;
    const TEX: u32 = 0x0de1;
    const COLOR: u32 = 0x8ce0;
    // SAFETY: GLES3 is required by this Android backend. All created objects
    // are private to this one-shot probe. Save/restore every touched binding;
    // deletion happens only after detaching/restoring those objects.
    unsafe {
        let (mut old_draw, mut old_rbo, mut old_tex) = (0, 0, 0);
        glGetIntegerv(0x8ca6, &mut old_draw);
        glGetIntegerv(0x8ca7, &mut old_rbo);
        glGetIntegerv(0x8069, &mut old_tex);
        let (mut fbo, mut rbo, mut tex) = (0, 0, 0);
        glGenFramebuffers(1, &mut fbo);
        glGenRenderbuffers(1, &mut rbo);
        glGenTextures(1, &mut tex);
        glBindRenderbuffer(RBO, rbo);
        glRenderbufferStorage(RBO, 0x8058, 4, 4);
        glBindTexture(TEX, tex);
        glTexStorage2D(TEX, 1, 0x8058, 4, 4);
        glTexParameteri(TEX, 0x2801, 0x2600);
        glTexParameteri(TEX, 0x2800, 0x2600);
        let record = |stage: &str| {
            let status = glCheckFramebufferStatus(DRAW);
            let (mut kind, mut name) = (0, 0);
            glGetFramebufferAttachmentParameteriv(DRAW, COLOR, 0x8cd0, &mut kind);
            glGetFramebufferAttachmentParameteriv(DRAW, COLOR, 0x8cd1, &mut name);
            info!("MIR2_ANDROID_ATTACHMENT_TRANSITION_PROBE stage={stage} status=0x{status:x} actual_type=0x{kind:x} actual_name={name} private_fbo={fbo} private_rbo={rbo} private_tex={tex}");
        };
        info!("MIR2_ANDROID_ATTACHMENT_TRANSITION_PROBE begin private_objects_only=true");
        glBindFramebuffer(DRAW, fbo);
        glFramebufferRenderbuffer(DRAW, COLOR, RBO, rbo);
        record("fresh_rbo");
        glFramebufferTexture2D(DRAW, COLOR, TEX, tex, 0);
        record("replace_with_texture");
        glBindFramebuffer(DRAW, old_draw as u32);
        glBindRenderbuffer(RBO, old_rbo as u32);
        glDeleteRenderbuffers(1, &rbo);
        glBindFramebuffer(DRAW, fbo);
        record("delete_unbound_old_rbo");
        glFramebufferRenderbuffer(DRAW, COLOR, RBO, 0);
        glFramebufferTexture2D(DRAW, COLOR, TEX, tex, 0);
        record("explicit_type_detach");
        glBindFramebuffer(DRAW, old_draw as u32);
        glBindTexture(TEX, old_tex as u32);
        glDeleteFramebuffers(1, &fbo);
        glDeleteTextures(1, &tex);
        info!("MIR2_ANDROID_ATTACHMENT_TRANSITION_PROBE end restored_bindings=true");
    }
}

#[cfg(target_os = "android")]
pub(crate) fn trace_android_view_outputs(world: &mut World) {
    let frame = ANDROID_SURFACE_PROBE_FRAME.load(core::sync::atomic::Ordering::Relaxed);
    if frame > 64 {
        return;
    }
    let current = ANDROID_SURFACE_PROBE_CURRENT_RAW.load(core::sync::atomic::Ordering::Relaxed);
    let mut query = world.query::<(
        Entity,
        &crate::view::ViewTarget,
        Option<&crate::camera::ExtractedCamera>,
        Option<&crate::view::ExtractedView>,
        Option<&crate::view::Msaa>,
    )>();
    for (entity, target, camera, view, msaa) in query.iter(world) {
        // SAFETY: This keeps both the view and the HAL guard alive. Debug
        // formatting only reads the backend handle, never calls or mutates GL.
        let output = unsafe {
            target
                .out_texture()
                .and_then(|view| view.as_hal::<wgpu::hal::api::Gles>())
                .map(|view| format!("{:?}", &*view))
        };
        info!("MIR2_ANDROID_VIEW_OUTPUT_PROBE frame={frame} entity={entity:?} current_surface_raw={current} has_camera={} has_view={} msaa={msaa:?} physical_target={:?} output={output:?}",
            camera.is_some(), view.is_some(), camera.and_then(|camera| camera.physical_target_size));
    }
}

#[cfg(target_os = "android")]
fn trace_android_surface_configuration(configuration: &SurfaceConfiguration, reason: &str) {
    ANDROID_SURFACE_PROBE_FRAME.store(0, core::sync::atomic::Ordering::Relaxed);
    info!(
        "MIR2_ANDROID_SURFACE_CONFIG_PROBE reason={reason} format={:?} size={}x{}",
        configuration.format, configuration.width, configuration.height
    );
}

#[cfg(target_os = "android")]
fn trace_android_surface_attachment(frame: &wgpu::SurfaceTexture, render_device: &RenderDevice) {
    use core::sync::atomic::Ordering;
    let index = ANDROID_SURFACE_PROBE_FRAME.fetch_add(1, Ordering::Relaxed);
    if index >= 64 {
        return;
    }
    #[link(name = "GLESv3")]
    unsafe extern "C" {
        fn glIsRenderbuffer(renderbuffer: u32) -> u8;
        fn glGetIntegerv(pname: u32, params: *mut i32);
        fn glBindRenderbuffer(target: u32, renderbuffer: u32);
        fn glGetRenderbufferParameteriv(target: u32, pname: u32, params: *mut i32);
    }
    // SAFETY: Both objects are owned by this render device and retained while
    // their HAL guards are alive. The HAL context lock makes that exact EGL
    // context current. Query only an existing renderbuffer; never materialize
    // a deleted name, mutate its storage, or consume the driver's error state.
    // The only touched binding is restored before the lock is released.
    unsafe {
        let Some(texture) = frame.texture.as_hal::<wgpu::hal::api::Gles>() else {
            return;
        };
        let Some(device) = render_device.wgpu_device().as_hal::<wgpu::hal::api::Gles>() else {
            return;
        };
        let _context = device.context().lock();
        if let wgpu::hal::gles::TextureInner::Renderbuffer { raw } = texture.inner {
            let raw = raw.0.get();
            ANDROID_SURFACE_PROBE_CURRENT_RAW.store(raw, Ordering::Relaxed);
            let live = glIsRenderbuffer(raw) != 0;
            let mut binding = 0;
            let (mut internal, mut width, mut height, mut samples) = (0, 0, 0, 0);
            if live {
                glGetIntegerv(0x8ca7, &mut binding); // RENDERBUFFER_BINDING
                glBindRenderbuffer(0x8d41, raw); // RENDERBUFFER
                glGetRenderbufferParameteriv(0x8d41, 0x8d44, &mut internal);
                glGetRenderbufferParameteriv(0x8d41, 0x8d42, &mut width);
                glGetRenderbufferParameteriv(0x8d41, 0x8d43, &mut height);
                glGetRenderbufferParameteriv(0x8d41, 0x8cab, &mut samples);
                glBindRenderbuffer(0x8d41, binding as u32);
            }
            info!(
                "MIR2_ANDROID_SURFACE_ATTACHMENT_PROBE frame={index} raw={raw} live={live} internal=0x{internal:x} size={width}x{height} samples={samples} expected={:?}",
                texture.format
            );
        } else {
            info!(
                "MIR2_ANDROID_SURFACE_ATTACHMENT_PROBE frame={index} non_renderbuffer={:?}",
                texture.inner
            );
        }
        trace_android_attachment_transition();
    }
}

pub struct WindowRenderPlugin;

impl Plugin for WindowRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreenshotPlugin);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_gpu_resource::<ExtractedWindows>()
                .init_gpu_resource::<WindowSurfaces>()
                .add_systems(ExtractSchedule, extract_windows.before(extract_cameras))
                .add_systems(
                    Render,
                    create_surfaces
                        .run_if(need_surface_configuration)
                        .before(prepare_windows),
                )
                .add_systems(Render, prepare_windows.in_set(RenderSystems::PrepareViews));
        }
    }
}

pub struct ExtractedWindow {
    /// An entity that contains the components in [`Window`].
    pub entity: Entity,
    pub handle: RawHandleWrapper,
    pub physical_width: u32,
    pub physical_height: u32,
    pub present_mode: PresentMode,
    pub desired_maximum_frame_latency: Option<NonZero<u32>>,
    /// Note: this will not always be the swap chain texture view. When taking a screenshot,
    /// this will point to an alternative texture instead to allow for copying the render result
    /// to CPU memory.
    pub swap_chain_texture_view: Option<TextureView>,
    pub swap_chain_texture: Option<SurfaceTexture>,
    pub swap_chain_texture_format: Option<TextureFormat>,
    /// This is an srgb view of [`ExtractedWindow::swap_chain_texture_format`]
    /// so that in shaders we are always in linear space.
    pub swap_chain_texture_view_format: Option<TextureFormat>,
    pub size_changed: bool,
    pub present_mode_changed: bool,
    pub alpha_mode: CompositeAlphaMode,
    /// Whether this window needs an initial buffer commit.
    ///
    /// On Wayland, windows must present at least once before they are shown.
    /// See <https://wayland.app/protocols/xdg-shell#xdg_surface>
    pub needs_initial_present: bool,
}

impl ExtractedWindow {
    fn set_swapchain_texture(
        &mut self,
        frame: wgpu::SurfaceTexture,
        texture_view_format: Option<TextureFormat>,
    ) {
        // The camera pipeline still needs a concrete target format when this
        // adapter cannot reinterpret the surface as sRGB. Only the view
        // descriptor must omit the unsupported alternate format.
        self.swap_chain_texture_view_format =
            Some(texture_view_format.unwrap_or_else(|| frame.texture.format()));
        let texture_view_descriptor = TextureViewDescriptor {
            format: texture_view_format,
            ..default()
        };
        self.swap_chain_texture_view = Some(TextureView::from(
            frame.texture.create_view(&texture_view_descriptor),
        ));
        self.swap_chain_texture = Some(SurfaceTexture::from(frame));
    }

    fn has_swapchain_texture(&self) -> bool {
        self.swap_chain_texture_view.is_some() && self.swap_chain_texture.is_some()
    }

    pub fn present(&mut self) {
        if let Some(surface_texture) = self.swap_chain_texture.take() {
            // TODO(clean): winit docs recommends calling pre_present_notify before this.
            // though `present()` doesn't present the frame, it schedules it to be presented
            // by wgpu.
            // https://docs.rs/winit/0.29.9/wasm32-unknown-unknown/winit/window/struct.Window.html#method.pre_present_notify
            surface_texture.present();
        }
    }
}

#[derive(Default, Resource)]
pub struct ExtractedWindows {
    pub primary: Option<Entity>,
    pub windows: EntityHashMap<ExtractedWindow>,
}

#[inline]
fn initial_swap_chain_format() -> Option<TextureFormat> {
    // Android GLES surfaces are configured to the linear RGBA8 format below
    // when alternate surface views are unavailable. `RawHandleWrapper` can be
    // removed for one extraction frame during Activity resume; using the same
    // format here keeps camera and mesh-pipeline specialization stable until
    // the replacement surface has delivered its first texture.
    #[cfg(target_os = "android")]
    {
        Some(TextureFormat::Rgba8Unorm)
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

impl Deref for ExtractedWindows {
    type Target = EntityHashMap<ExtractedWindow>;

    fn deref(&self) -> &Self::Target {
        &self.windows
    }
}

impl DerefMut for ExtractedWindows {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.windows
    }
}

fn extract_windows(
    mut extracted_windows: ResMut<ExtractedWindows>,
    mut lifecycle: Extract<MessageReader<AppLifecycle>>,
    mut closing: Extract<MessageReader<WindowClosing>>,
    windows: Extract<Query<(Entity, &Window, &RawHandleWrapper, Option<&PrimaryWindow>)>>,
    mut removed: Extract<RemovedComponents<RawHandleWrapper>>,
    mut window_surfaces: ResMut<WindowSurfaces>,
) {
    // Android is allowed to reuse the same ANativeWindow pointer after the
    // underlying Surface has been destroyed and recreated. Component removal
    // and raw-handle equality therefore are not sufficient invalidation
    // signals. Drop every acquired view before its old wgpu surface at an app
    // lifecycle boundary; the normal loop below will configure the current
    // handle on the first resumed render frame. Preserve the extracted window
    // entry itself, including the last confirmed target format: camera
    // extraction runs before the new surface is configured, and falling back
    // to the default sRGB format for that single frame can reuse an sRGB mesh
    // pipeline against the resumed GLES surface's linear attachment.
    if lifecycle.read().any(|state| {
        matches!(
            state,
            AppLifecycle::Running
                | AppLifecycle::WillSuspend
                | AppLifecycle::Suspended
                | AppLifecycle::WillResume
        )
    }) {
        for window in extracted_windows.values_mut() {
            drop(window.swap_chain_texture.take());
            drop(window.swap_chain_texture_view.take());
        }
        window_surfaces.surfaces.clear();
        window_surfaces.configured_windows.clear();
    }

    for (entity, window, handle, primary) in windows.iter() {
        if primary.is_some() {
            extracted_windows.primary = Some(entity);
        }

        // Android replaces the native window when an Activity resumes. The
        // render extraction can observe the replacement without first seeing
        // `RemovedComponents<RawHandleWrapper>` (in particular across winit's
        // final suspend / first resume updates), so an entry keyed only by the
        // Bevy window entity may otherwise retain a surface and swap-chain
        // view backed by the destroyed ANativeWindow.
        let raw_handle_changed = extracted_windows.get(&entity).is_some_and(|existing| {
            existing.handle.get_window_handle() != handle.get_window_handle()
                || existing.handle.get_display_handle() != handle.get_display_handle()
        });
        if raw_handle_changed {
            if let Some(existing) = extracted_windows.get_mut(&entity) {
                drop(existing.swap_chain_texture.take());
                drop(existing.swap_chain_texture_view.take());
            }
            window_surfaces.remove(&entity);
            extracted_windows.remove(&entity);
        }

        let (new_width, new_height) = (
            window.resolution.physical_width().max(1),
            window.resolution.physical_height().max(1),
        );

        let extracted_window = extracted_windows.entry(entity).or_insert(ExtractedWindow {
            entity,
            handle: handle.clone(),
            physical_width: new_width,
            physical_height: new_height,
            present_mode: window.present_mode,
            desired_maximum_frame_latency: window.desired_maximum_frame_latency,
            swap_chain_texture: None,
            swap_chain_texture_view: None,
            size_changed: false,
            swap_chain_texture_format: initial_swap_chain_format(),
            swap_chain_texture_view_format: initial_swap_chain_format(),
            present_mode_changed: false,
            alpha_mode: window.composite_alpha_mode,
            needs_initial_present: true,
        });

        if extracted_window.swap_chain_texture.is_none() {
            // If we called present on the previous swap-chain texture last update,
            // then drop the swap chain frame here, otherwise we can keep it for the
            // next update as an optimization. `prepare_windows` will only acquire a new
            // swap chain texture if needed.
            extracted_window.swap_chain_texture_view = None;
        }
        extracted_window.size_changed = new_width != extracted_window.physical_width
            || new_height != extracted_window.physical_height;
        extracted_window.present_mode_changed =
            window.present_mode != extracted_window.present_mode;

        if extracted_window.size_changed {
            debug!(
                "Window size changed from {}x{} to {}x{}",
                extracted_window.physical_width,
                extracted_window.physical_height,
                new_width,
                new_height
            );
            extracted_window.physical_width = new_width;
            extracted_window.physical_height = new_height;
        }

        if extracted_window.present_mode_changed {
            debug!(
                "Window Present Mode changed from {:?} to {:?}",
                extracted_window.present_mode, window.present_mode
            );
            extracted_window.present_mode = window.present_mode;
        }
    }

    for closing_window in closing.read() {
        extracted_windows.remove(&closing_window.window);
        window_surfaces.remove(&closing_window.window);
    }
    for removed_window in removed.read() {
        extracted_windows.remove(&removed_window);
        window_surfaces.remove(&removed_window);
    }
}

struct SurfaceData {
    // TODO: what lifetime should this be?
    surface: WgpuWrapper<wgpu::Surface<'static>>,
    configuration: SurfaceConfiguration,
    texture_view_format: Option<TextureFormat>,
}

#[derive(Resource, Default)]
pub struct WindowSurfaces {
    surfaces: EntityHashMap<SurfaceData>,
    /// List of windows that we have already called the initial `configure_surface` for
    configured_windows: EntityHashSet,
}

impl WindowSurfaces {
    fn remove(&mut self, window: &Entity) {
        self.surfaces.remove(window);
        self.configured_windows.remove(window);
    }
}

/// (re)configures window surfaces, and obtains a swapchain texture for rendering.
///
/// NOTE: `get_current_texture` in `prepare_windows` can take a long time if the GPU workload is
/// the performance bottleneck. This can be seen in profiles as multiple prepare-set systems all
/// taking an unusually long time to complete, and all finishing at about the same time as the
/// `prepare_windows` system. Improvements in bevy are planned to avoid this happening when it
/// should not but it will still happen as it is easy for a user to create a large GPU workload
/// relative to the GPU performance and/or CPU workload.
/// This can be caused by many reasons, but several of them are:
/// - GPU workload is more than your current GPU can manage
/// - Error / performance bug in your custom shaders
/// - wgpu was unable to detect a proper GPU hardware-accelerated device given the chosen
///   [`Backends`](crate::settings::Backends), [`WgpuLimits`](crate::settings::WgpuLimits),
///   and/or [`WgpuFeatures`](crate::settings::WgpuFeatures). For example, on Windows currently
///   `DirectX 11` is not supported by wgpu 0.12 and so if your GPU/drivers do not support Vulkan,
///   it may be that a software renderer called "Microsoft Basic Render Driver" using `DirectX 12`
///   will be chosen and performance will be very poor. This is visible in a log message that is
///   output during renderer initialization.
///   Another alternative is to try to use [`ANGLE`](https://github.com/gfx-rs/wgpu#angle) and
///   [`Backends::GL`](crate::settings::Backends::GL) with the `gles` feature enabled if your
///   GPU/drivers support `OpenGL 4.3` / `OpenGL ES 3.0` or later.
pub fn prepare_windows(
    mut windows: ResMut<ExtractedWindows>,
    mut window_surfaces: ResMut<WindowSurfaces>,
    render_device: Res<RenderDevice>,
    sorted_cameras: Res<crate::camera::SortedCameras>,
    #[cfg(target_os = "linux")] render_instance: Res<RenderInstance>,
) {
    for window in windows.windows.values_mut() {
        // Skip acquiring a swap-chain texture for windows that no camera
        // targets. This avoids a wasted clear pass in
        // `handle_uncovered_swap_chains` that triggers a DMA-fence fd leak on
        // Adreno 740 (Quest 3). The exception is windows that still need their
        // initial present (required on Wayland).
        let is_camera_target = sorted_cameras.0.iter().any(|c| {
            matches!(
                &c.target,
                Some(bevy_camera::NormalizedRenderTarget::Window(w)) if w.entity() == window.entity
            ) && matches!(c.output_mode, bevy_camera::CameraOutputMode::Write { .. })
        });
        if !is_camera_target && !window.needs_initial_present {
            continue;
        }

        let window_surfaces = window_surfaces.deref_mut();
        let Some(surface_data) = window_surfaces.surfaces.get(&window.entity) else {
            continue;
        };

        // We didn't present the previous frame, so we can keep using our existing swapchain texture.
        if window.has_swapchain_texture() && !window.size_changed && !window.present_mode_changed {
            continue;
        }

        // A recurring issue is hitting `wgpu::SurfaceError::Timeout` on certain Linux
        // mesa driver implementations. This seems to be a quirk of some drivers.
        // We'd rather keep panicking when not on Linux mesa, because in those case,
        // the `Timeout` is still probably the symptom of a degraded unrecoverable
        // application state.
        // see https://github.com/bevyengine/bevy/pull/5957
        // and https://github.com/gfx-rs/wgpu/issues/1218
        #[cfg(target_os = "linux")]
        let may_erroneously_timeout = || {
            bevy_tasks::IoTaskPool::get().scope(|scope| {
                scope.spawn(async {
                    render_instance
                        .enumerate_adapters(wgpu::Backends::VULKAN)
                        .await
                        .iter()
                        .any(|adapter| {
                            let name = adapter.get_info().name;
                            name.starts_with("Radeon")
                                || name.starts_with("AMD")
                                || name.starts_with("Intel")
                        })
                });
            })[0]
        };

        let surface = &surface_data.surface;
        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                #[cfg(target_os = "android")]
                trace_android_surface_attachment(&surface_texture, &render_device);
                window.set_swapchain_texture(surface_texture, surface_data.texture_view_format);
            }
            #[cfg(target_os = "linux")]
            wgpu::CurrentSurfaceTexture::Timeout if may_erroneously_timeout() => {
                bevy_log::trace!(
                    "Couldn't get swap chain texture. This is probably a quirk \
                        of your Linux GPU driver, so it can be safely ignored."
                );
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                render_device.configure_surface(surface, &surface_data.configuration);
                #[cfg(target_os = "android")]
                trace_android_surface_configuration(&surface_data.configuration, "outdated");
                let frame = match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(surface_texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
                    variant => {
                        // This is a common occurrence on X11 and Xwayland with NVIDIA drivers
                        // when opening and resizing the window.
                        warn!(
                            "Couldn't get swap chain texture after configuring. Cause: '{variant:?}'"
                        );
                        continue;
                    }
                };
                #[cfg(target_os = "android")]
                trace_android_surface_attachment(&frame, &render_device);
                window.set_swapchain_texture(frame, surface_data.texture_view_format);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {}
            other => {
                bevy_log::error!("Couldn't get swap chain texture: {other:?}");
            }
        }
        window.swap_chain_texture_format = Some(surface_data.configuration.format);
    }
}

pub fn need_surface_configuration(
    windows: Res<ExtractedWindows>,
    window_surfaces: Res<WindowSurfaces>,
) -> bool {
    for window in windows.windows.values() {
        if !window_surfaces.configured_windows.contains(&window.entity)
            || window.size_changed
            || window.present_mode_changed
        {
            return true;
        }
    }
    false
}

// 2 is wgpu's default/what we've been using so far.
// 1 is the minimum, but may cause lower framerates due to the cpu waiting for the gpu to finish
// all work for the previous frame before starting work on the next frame, which then means the gpu
// has to wait for the cpu to finish to start on the next frame.
const DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY: u32 = 2;

/// Creates window surfaces.
pub fn create_surfaces(
    // By accessing a NonSend resource, we tell the scheduler to put this system on the main thread,
    // which is necessary for some OS's
    #[cfg(any(target_os = "macos", target_os = "ios"))] _marker: bevy_ecs::system::NonSendMarker,
    mut windows: ResMut<ExtractedWindows>,
    mut window_surfaces: ResMut<WindowSurfaces>,
    render_instance: Res<RenderInstance>,
    render_adapter: Res<RenderAdapter>,
    render_device: Res<RenderDevice>,
) {
    for window in windows.windows.values_mut() {
        let data = window_surfaces
            .surfaces
            .entry(window.entity)
            .or_insert_with(|| {
                let surface_target = SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(window.handle.get_display_handle()),
                    raw_window_handle: window.handle.get_window_handle(),
                };
                // SAFETY: The window handles in ExtractedWindows will always be valid objects to create surfaces on
                let surface = unsafe {
                    // NOTE: On some OSes this MUST be called from the main thread.
                    // As of wgpu 0.15, only fallible if the given window is a HTML canvas and obtaining a WebGPU or WebGL2 context fails.
                    render_instance
                        .create_surface_unsafe(surface_target)
                        .expect("Failed to create wgpu surface")
                };
                let caps = surface.get_capabilities(&render_adapter);
                let present_mode = present_mode(window, &caps);
                let formats = caps.formats;
                // For future HDR output support, we'll need to request a format that supports HDR,
                // but as of wgpu 0.15 that is not yet supported.
                // Prefer sRGB formats for surfaces, but fall back to first available format if no sRGB formats are available.
                let mut format = *formats.first().expect("No supported formats for surface");
                for available_format in formats.iter().copied() {
                    // Rgba8UnormSrgb and Bgra8UnormSrgb and the only sRGB formats wgpu exposes that we can use for surfaces.
                    if available_format == TextureFormat::Rgba8UnormSrgb
                        || available_format == TextureFormat::Bgra8UnormSrgb
                    {
                        format = available_format;
                        break;
                    }
                }

                let supports_surface_view_formats = render_adapter
                    .get_downlevel_capabilities()
                    .flags
                    .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS);
                // GLES adapters without surface view reinterpretation can
                // report an sRGB surface on the first Android native window
                // and only its linear pair after the window is recreated.
                // Pick that linear pair up front so cached render pipelines
                // retain the same attachment format across suspend/resume.
                if !supports_surface_view_formats {
                    let linear_format = format.remove_srgb_suffix();
                    if formats.contains(&linear_format) {
                        format = linear_format;
                    }
                }
                let texture_view_format = if !format.is_srgb() && supports_surface_view_formats {
                    Some(format.add_srgb_suffix())
                } else {
                    None
                };
                let configuration = SurfaceConfiguration {
                    format,
                    width: window.physical_width,
                    height: window.physical_height,
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    present_mode,
                    desired_maximum_frame_latency: window
                        .desired_maximum_frame_latency
                        .map(NonZero::<u32>::get)
                        .unwrap_or(DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY),
                    alpha_mode: match window.alpha_mode {
                        CompositeAlphaMode::Auto => wgpu::CompositeAlphaMode::Auto,
                        CompositeAlphaMode::Opaque => wgpu::CompositeAlphaMode::Opaque,
                        CompositeAlphaMode::PreMultiplied => {
                            wgpu::CompositeAlphaMode::PreMultiplied
                        }
                        CompositeAlphaMode::PostMultiplied => {
                            wgpu::CompositeAlphaMode::PostMultiplied
                        }
                        CompositeAlphaMode::Inherit => wgpu::CompositeAlphaMode::Inherit,
                    },
                    view_formats: match texture_view_format {
                        Some(format) => vec![format],
                        None => vec![],
                    },
                };

                render_device.configure_surface(&surface, &configuration);
                #[cfg(target_os = "android")]
                trace_android_surface_configuration(&configuration, "initial");

                SurfaceData {
                    surface: WgpuWrapper::new(surface),
                    configuration,
                    texture_view_format,
                }
            });

        // Camera extraction runs before this prepare system. Publish the
        // confirmed configuration now so the next extraction specializes its
        // pipelines for the replacement Android surface even before the first
        // swap-chain texture is acquired.
        window.swap_chain_texture_format = Some(data.configuration.format);
        window.swap_chain_texture_view_format = Some(
            data.texture_view_format
                .unwrap_or(data.configuration.format),
        );

        if window.size_changed || window.present_mode_changed {
            // normally this is dropped on present but we double check here to be safe as failure to
            // drop it will cause validation errors in wgpu
            drop(window.swap_chain_texture.take());
            #[cfg_attr(
                target_arch = "wasm32",
                expect(clippy::drop_non_drop, reason = "texture views are not drop on wasm")
            )]
            drop(window.swap_chain_texture_view.take());

            data.configuration.width = window.physical_width;
            data.configuration.height = window.physical_height;
            let caps = data.surface.get_capabilities(&render_adapter);
            data.configuration.present_mode = present_mode(window, &caps);
            render_device.configure_surface(&data.surface, &data.configuration);
            #[cfg(target_os = "android")]
            trace_android_surface_configuration(&data.configuration, "resize_or_present_mode");
        }

        window_surfaces.configured_windows.insert(window.entity);
    }
}

fn present_mode(
    window: &mut ExtractedWindow,
    caps: &wgpu::SurfaceCapabilities,
) -> wgpu::PresentMode {
    let present_mode = match window.present_mode {
        PresentMode::Fifo => wgpu::PresentMode::Fifo,
        PresentMode::FifoRelaxed => wgpu::PresentMode::FifoRelaxed,
        PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        PresentMode::Immediate => wgpu::PresentMode::Immediate,
        PresentMode::AutoVsync => wgpu::PresentMode::AutoVsync,
        PresentMode::AutoNoVsync => wgpu::PresentMode::AutoNoVsync,
    };
    let fallbacks = match present_mode {
        wgpu::PresentMode::AutoVsync => {
            &[wgpu::PresentMode::FifoRelaxed, wgpu::PresentMode::Fifo][..]
        }
        wgpu::PresentMode::AutoNoVsync => &[
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Fifo,
        ][..],
        wgpu::PresentMode::Mailbox => &[
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Fifo,
        ][..],
        // Always end in FIFO to make sure it's always supported
        x => &[x, wgpu::PresentMode::Fifo][..],
    };
    let new_present_mode = fallbacks
        .iter()
        .copied()
        .find(|fallback| caps.present_modes.contains(fallback))
        .unwrap_or_else(|| {
            unreachable!(
                "Fallback system failed to choose present mode. \
                            This is a bug. Mode: {:?}, Options: {:?}",
                window.present_mode, &caps.present_modes
            );
        });
    if new_present_mode != present_mode && fallbacks.contains(&present_mode) {
        info!(
            "PresentMode {present_mode:?} requested but not available. Falling back to {new_present_mode:?}"
        );
    }
    new_present_mode
}
