use super::skin::Skin;
use super::transfer::{bytes, duration, TransferMeter};
use super::*;
use std::sync::atomic::AtomicU64;
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    ptr,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{
        CreateFontW, DeleteObject, GetStockObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY,
        CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, HDC, HFONT, HOLLOW_BRUSH,
        OUT_DEFAULT_PRECIS, TRANSPARENT,
    },
    System::{
        LibraryLoader::GetModuleHandleW,
        SystemServices::{SS_ENDELLIPSIS, SS_ICON, SS_NOPREFIX},
    },
    UI::{
        Controls::{
            InitCommonControlsEx, DRAWITEMSTRUCT, ICC_PROGRESS_CLASS, INITCOMMONCONTROLSEX,
            PBM_SETMARQUEE, PBM_SETPOS, PBM_SETRANGE32, PBS_MARQUEE, PROGRESS_CLASSW,
        },
        HiDpi::{
            AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow,
            SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        },
        Input::KeyboardAndMouse::{EnableWindow, SetFocus},
        WindowsAndMessaging::*,
    },
};

const STATUS_MESSAGE: u32 = WM_APP + 10;
const CANCEL_ID: usize = 101;
const HEADING_ID: usize = 102;
const STATUS_ID: usize = 103;
const PROGRESS_ID: usize = 104;
const ACTION_ID: usize = 105;
const DETAIL_ID: usize = 106;
const ELAPSED_ID: usize = 107;
const SUBTITLE_ID: usize = 108;
const ICON_ID: usize = 109;
const RETRY_ID: usize = 110;
const VERSION_ID: usize = 111;
const VERSION_LABEL_ID: usize = 112;
const MINIMIZE_ID: usize = 113;
const CLASS: *const u16 = windows_sys::core::w!("NumeronMir2LauncherV2");
const STYLE: u32 = WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
const WIDTH: i32 = 1120;
const HEIGHT: i32 = 680;

fn wide(text: &str) -> Vec<u16> {
    text.chars()
        .map(|c| if c == '\0' { '�' } else { c })
        .collect::<String>()
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

enum Command {
    Update(String, u32),
    Stage(String, u32),
    Transfer(u64, u64),
    BeginLaunch(mpsc::SyncSender<bool>),
    Home {
        version: String,
        can_play: bool,
        response: mpsc::SyncSender<LauncherAction>,
    },
    Choice {
        version: String,
        bytes: u64,
        can_play: bool,
        response: mpsc::SyncSender<LauncherAction>,
    },
    Probe(Option<String>, bool),
    Installed(bool),
    Version(String),
    Prompt {
        message: String,
        detail: String,
        action: String,
        percent: u32,
        allow_retry: bool,
        response: mpsc::SyncSender<LauncherAction>,
    },
    Close,
}

struct State {
    receiver: mpsc::Receiver<Command>,
    cancelled: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    response: RefCell<Option<mpsc::SyncSender<LauncherAction>>>,
    locale: String,
    heading: Cell<HWND>,
    subtitle: Cell<HWND>,
    icon: Cell<HWND>,
    status: Cell<HWND>,
    detail: Cell<HWND>,
    elapsed: Cell<HWND>,
    progress: Cell<HWND>,
    cancel: Cell<HWND>,
    action: Cell<HWND>,
    retry: Cell<HWND>,
    font: Cell<HFONT>,
    heading_font: Cell<HFONT>,
    detail_font: Cell<HFONT>,
    small_font: Cell<HFONT>,
    version_label: Cell<HWND>,
    version_value: Cell<HWND>,
    minimize: Cell<HWND>,
    skin: RefCell<Skin>,
    percent: Cell<u32>,
    dpi: Cell<u32>,
    started: Instant,
    busy_started: Cell<Instant>,
    transfer: RefCell<Option<TransferMeter>>,
    busy: Cell<bool>,
    home: Cell<bool>,
    can_play: Cell<bool>,
    primary_action: Cell<LauncherAction>,
    secondary_action: Cell<LauncherAction>,
    secondary_visible: Cell<bool>,
    probe: RefCell<Option<(Option<String>, bool)>>,
    version: RefCell<String>,
}

impl Drop for State {
    fn drop(&mut self) {
        for font in [
            self.font.get(),
            self.heading_font.get(),
            self.detail_font.get(),
            self.small_font.get(),
        ] {
            if !font.is_null() {
                unsafe {
                    DeleteObject(font);
                }
            }
        }
    }
}

unsafe fn text(control: HWND, value: &str) {
    unsafe {
        SetWindowTextW(control, wide(value).as_ptr());
    }
}

unsafe fn set_progress(state: &State, percent: u32, indefinite: bool) {
    state.percent.set(percent.min(100));
    let control = state.progress.get();
    unsafe {
        let style = GetWindowLongPtrW(control, GWL_STYLE) as u32;
        SetWindowLongPtrW(
            control,
            GWL_STYLE,
            if indefinite {
                style | PBS_MARQUEE
            } else {
                style & !PBS_MARQUEE
            } as isize,
        );
        SendMessageW(control, PBM_SETMARQUEE, indefinite as usize, 40);
        SendMessageW(control, PBM_SETPOS, percent.min(100) as usize, 0);
    }
}

unsafe fn refresh_metrics(state: &State) {
    let now = state.started.elapsed();
    unsafe {
        text(state.version_value.get(), &state.version.borrow());
        text(
            state.elapsed.get(),
            &if state.busy.get() {
                render(
                    &state.locale,
                    "elapsed",
                    &[("time", duration(state.busy_started.get().elapsed()))],
                )
            } else {
                String::new()
            },
        );
    }
    if let Some(meter) = state.transfer.borrow().as_ref() {
        unsafe {
            text(state.detail.get(), &meter.summary(&state.locale, now));
        }
    }
}

unsafe fn begin_busy(state: &State) {
    if !state.busy.replace(true) {
        state.busy_started.set(Instant::now());
    }
    state.home.set(false);
    state.secondary_visible.set(false);
    unsafe {
        EnableWindow(state.action.get(), 0);
        ShowWindow(state.retry.get(), SW_HIDE);
        text(state.action.get(), &stage(&state.locale, "working"));
        text(state.cancel.get(), &stage(&state.locale, "cancel"));
    }
}

unsafe fn home_status(state: &State) {
    unsafe {
        text(
            state.status.get(),
            &stage(
                &state.locale,
                if state.can_play.get() {
                    "home-ready"
                } else {
                    "home-repair"
                },
            ),
        );
        let detail = match state.probe.borrow().as_ref() {
            Some((_, true)) => stage(&state.locale, "probe-failed"),
            Some((Some(version), false)) => render(
                &state.locale,
                "optional-found",
                &[("version", version.clone())],
            ),
            Some((None, false)) => stage(&state.locale, "up-to-date"),
            None => stage(&state.locale, "checking-optional"),
        };
        text(
            state.detail.get(),
            &format!(
                "{}\n{}",
                detail,
                stage(
                    &state.locale,
                    if state.can_play.get() {
                        "play-without-update"
                    } else {
                        "repair-hint"
                    }
                )
            ),
        );
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const State;
    if pointer.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // UI-thread-only Cells allow synchronous Win32 reentry without an aliasing &mut.
    let state = unsafe { &*pointer };
    match message {
        STATUS_MESSAGE => {
            while let Ok(command) = state.receiver.try_recv() {
                match command {
                    Command::Update(message, percent) => unsafe {
                        begin_busy(state);
                        text(state.status.get(), &message);
                        set_progress(state, percent, false);
                        state.transfer.replace(None);
                        text(state.detail.get(), "");
                    },
                    Command::Stage(key, percent) => unsafe {
                        begin_busy(state);
                        text(state.status.get(), &stage(&state.locale, &key));
                        set_progress(state, percent, key == "checking");
                        if key.split(':').next() != Some("downloading") {
                            state.transfer.replace(None);
                            let hint = if key == "checking" {
                                stage(&state.locale, "checking-hint")
                            } else {
                                String::new()
                            };
                            text(state.detail.get(), &hint);
                        }
                    },
                    Command::Transfer(received, total) => unsafe {
                        let mut transfer = state.transfer.borrow_mut();
                        transfer.get_or_insert_with(TransferMeter::default).observe(
                            received,
                            total,
                            state.started.elapsed(),
                        );
                        drop(transfer);
                        refresh_metrics(state);
                    },
                    Command::Prompt {
                        message,
                        detail,
                        action,
                        percent,
                        allow_retry,
                        response,
                    } => unsafe {
                        state.busy.set(false);
                        state.home.set(false);
                        state.primary_action.set(LauncherAction::Proceed);
                        state.secondary_action.set(LauncherAction::Retry);
                        state.secondary_visible.set(allow_retry);
                        state.transfer.replace(None);
                        state.response.replace(Some(response));
                        text(state.status.get(), &message);
                        text(state.detail.get(), &detail);
                        text(state.action.get(), &action);
                        text(state.cancel.get(), &stage(&state.locale, "exit"));
                        set_progress(state, percent, false);
                        EnableWindow(state.action.get(), 1);
                        ShowWindow(
                            state.retry.get(),
                            if allow_retry {
                                SW_SHOWNOACTIVATE
                            } else {
                                SW_HIDE
                            },
                        );
                        SetFocus(state.action.get());
                    },
                    Command::Home {
                        version,
                        can_play,
                        response,
                    } => unsafe {
                        state.busy.set(false);
                        state.home.set(true);
                        state.can_play.set(can_play);
                        state.version.replace(version);
                        state.transfer.replace(None);
                        state.response.replace(Some(response));
                        state.primary_action.set(LauncherAction::PlayInstalled);
                        state.secondary_action.set(LauncherAction::Proceed);
                        state.secondary_visible.set(true);
                        text(state.action.get(), &stage(&state.locale, "play"));
                        text(state.retry.get(), &stage(&state.locale, "review-update"));
                        text(state.cancel.get(), &stage(&state.locale, "exit"));
                        EnableWindow(state.action.get(), can_play as i32);
                        ShowWindow(state.retry.get(), SW_SHOWNOACTIVATE);
                        set_progress(state, if can_play { 100 } else { 0 }, false);
                        home_status(state);
                        SetFocus(if can_play {
                            state.action.get()
                        } else {
                            state.retry.get()
                        });
                    },
                    Command::Choice {
                        version,
                        bytes: size,
                        can_play,
                        response,
                    } => unsafe {
                        state.busy.set(false);
                        state.home.set(false);
                        state.can_play.set(can_play);
                        state.transfer.replace(None);
                        state.response.replace(Some(response));
                        state.primary_action.set(if can_play {
                            LauncherAction::PlayInstalled
                        } else {
                            LauncherAction::Proceed
                        });
                        state.secondary_action.set(LauncherAction::Proceed);
                        state.secondary_visible.set(can_play);
                        text(
                            state.status.get(),
                            &stage(&state.locale, "update-available"),
                        );
                        text(
                            state.detail.get(),
                            &format!(
                                "{}\n{}",
                                render(&state.locale, "target-version", &[("version", version)]),
                                render(&state.locale, "download-size", &[("size", bytes(size))])
                            ),
                        );
                        text(
                            state.action.get(),
                            &stage(
                                &state.locale,
                                if can_play { "play-current" } else { "update" },
                            ),
                        );
                        text(state.retry.get(), &stage(&state.locale, "update"));
                        text(state.cancel.get(), &stage(&state.locale, "exit"));
                        EnableWindow(state.action.get(), 1);
                        ShowWindow(
                            state.retry.get(),
                            if can_play { SW_SHOWNOACTIVATE } else { SW_HIDE },
                        );
                        set_progress(state, 0, false);
                        SetFocus(state.action.get());
                    },
                    Command::Probe(latest, failed) => {
                        state.probe.replace(Some((latest, failed)));
                        if state.home.get() {
                            unsafe {
                                home_status(state);
                            }
                        }
                    }
                    Command::Installed(available) => {
                        state.can_play.set(available);
                    }
                    Command::Version(version) => {
                        state.version.replace(version);
                    }
                    Command::Close => {
                        unsafe {
                            DestroyWindow(hwnd);
                        }
                        return 0;
                    }
                    Command::BeginLaunch(response) => {
                        let accepted = !state.cancelled.load(Ordering::Acquire);
                        unsafe {
                            DestroyWindow(hwnd);
                        }
                        let _ = response.send(accepted);
                        return 0;
                    }
                }
            }
            unsafe {
                refresh_metrics(state);
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_COMMAND if wparam & 0xffff == ACTION_ID => {
            if !state.busy.get()
                && unsafe {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(
                        state.action.get(),
                    )
                } != 0
            {
                let action = state.primary_action.get();
                let response = state.response.borrow_mut().take();
                unsafe {
                    begin_busy(state);
                }
                if let Some(response) = response {
                    let _ = response.send(action);
                }
            }
            0
        }
        WM_COMMAND if wparam & 0xffff == RETRY_ID => {
            if !state.busy.get() && state.secondary_visible.get() {
                let action = state.secondary_action.get();
                let response = state.response.borrow_mut().take();
                unsafe {
                    begin_busy(state);
                }
                if let Some(response) = response {
                    let _ = response.send(action);
                }
            }
            0
        }
        WM_COMMAND if wparam & 0xffff == CANCEL_ID => {
            state.cancelled.store(true, Ordering::Release);
            if let Some(response) = state.response.borrow_mut().take() {
                let _ = response.send(LauncherAction::Exit);
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_CLOSE => {
            state.cancelled.store(true, Ordering::Release);
            if let Some(response) = state.response.borrow_mut().take() {
                let _ = response.send(LauncherAction::Exit);
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_TIMER => {
            unsafe {
                refresh_metrics(state);
            }
            0
        }
        WM_PAINT => {
            unsafe {
                state.skin.borrow().paint(
                    hwnd,
                    state.dpi.get(),
                    &state.locale,
                    state.percent.get(),
                    state.busy.get(),
                    state.can_play.get(),
                );
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_DRAWITEM if lparam != 0 => {
            unsafe {
                state
                    .skin
                    .borrow()
                    .button(&*(lparam as *const DRAWITEMSTRUCT), state.dpi.get());
            }
            1
        }
        WM_COMMAND if wparam & 0xffff == MINIMIZE_ID => {
            unsafe {
                ShowWindow(hwnd, SW_MINIMIZE);
            }
            0
        }
        WM_NCHITTEST => {
            let mut point = POINT {
                x: (lparam as i16) as i32,
                y: ((lparam >> 16) as i16) as i32,
            };
            unsafe {
                windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut point);
            }
            if point.y >= 0 && point.y < px(72, state.dpi.get()) {
                HTCAPTION as isize
            } else {
                HTCLIENT as isize
            }
        }
        WM_CTLCOLORSTATIC => unsafe {
            SetBkMode(wparam as HDC, TRANSPARENT as i32);
            let id = GetDlgCtrlID(lparam as HWND) as usize;
            SetTextColor(
                wparam as HDC,
                if id == DETAIL_ID || id == ELAPSED_ID || id == VERSION_LABEL_ID {
                    0x00B9B7B3
                } else if id == SUBTITLE_ID {
                    0x0087B5D7
                } else {
                    0x00E8EBEF
                },
            );
            if id == HEADING_ID || id == SUBTITLE_ID || id == ICON_ID {
                GetStockObject(HOLLOW_BRUSH) as isize
            } else if id == STATUS_ID || id == DETAIL_ID {
                state.skin.borrow().card_brush() as isize
            } else {
                state.skin.borrow().panel_brush() as isize
            }
        },
        WM_SIZE => {
            unsafe {
                layout(hwnd, state);
            }
            0
        }
        WM_DPICHANGED => {
            state.dpi.set((wparam & 0xffff) as u32);
            if lparam != 0 {
                let suggested = unsafe { &*(lparam as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        ptr::null_mut(),
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            unsafe {
                state.skin.borrow_mut().set_dpi(state.dpi.get());
                set_font(state);
                layout(hwnd, state);
            }
            0
        }
        WM_DESTROY => {
            state.alive.store(false, Ordering::Release);
            unsafe {
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
            }
            0
        }
        WM_NCDESTROY => unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, message, wparam, lparam)
        },
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn px(logical: i32, dpi: u32) -> i32 {
    ((i64::from(logical) * i64::from(dpi.max(96)) + 48) / 96) as i32
}

unsafe fn new_font(size: i32, weight: i32, dpi: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -px(size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            windows_sys::core::w!("Segoe UI"),
        )
    }
}

unsafe fn set_font(state: &State) {
    for (slot, size, weight, controls) in [
        (
            &state.font,
            16,
            400,
            vec![
                state.status.get(),
                state.action.get(),
                state.retry.get(),
                state.cancel.get(),
            ],
        ),
        (
            &state.heading_font,
            24,
            700,
            vec![state.heading.get(), state.version_value.get()],
        ),
        (&state.detail_font, 14, 400, vec![state.detail.get()]),
        (
            &state.small_font,
            12,
            400,
            vec![
                state.subtitle.get(),
                state.elapsed.get(),
                state.version_label.get(),
            ],
        ),
    ] {
        let font = unsafe { new_font(size, weight, state.dpi.get()) };
        if !font.is_null() {
            let old = slot.replace(font);
            for control in controls {
                if !control.is_null() {
                    unsafe {
                        SendMessageW(control, WM_SETFONT, font as usize, 1);
                    }
                }
            }
            if !old.is_null() {
                unsafe {
                    DeleteObject(old);
                }
            }
        }
    }
}

unsafe fn layout(hwnd: HWND, state: &State) {
    let dpi = state.dpi.get();
    for (control, x, y, width, height) in [
        (state.icon.get(), 28, 23, 32, 32),
        (state.heading.get(), 72, 14, 380, 32),
        (state.subtitle.get(), 73, 48, 400, 20),
        (state.version_label.get(), 768, 87, 320, 20),
        (state.version_value.get(), 768, 114, 320, 37),
        (state.status.get(), 788, 243, 280, 54),
        (state.detail.get(), 788, 307, 280, 94),
        (state.elapsed.get(), 768, 432, 320, 22),
        (state.progress.get(), 768, 468, 320, 4),
        (state.cancel.get(), 1076, 16, 28, 26),
        (state.minimize.get(), 1036, 16, 28, 26),
        (state.action.get(), 768, 504, 320, 58),
        (state.retry.get(), 768, 574, 320, 40),
    ] {
        if !control.is_null() {
            unsafe {
                MoveWindow(
                    control,
                    px(x, dpi),
                    px(y, dpi),
                    px(width, dpi),
                    px(height, dpi),
                    1,
                );
            }
        }
    }
    unsafe {
        windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, ptr::null(), 0);
    }
}

unsafe fn run_window(
    locale: String,
    receiver: mpsc::Receiver<Command>,
    cancelled: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    ready: mpsc::SyncSender<usize>,
    visible: bool,
) {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let icon = unsafe { LoadIconW(instance, 1usize as _) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hIcon: icon,
        hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
        hbrBackground: ptr::null_mut(),
        lpszClassName: CLASS,
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 && unsafe { GetLastError() } != 1410 {
        let _ = ready.send(0);
        return;
    }
    let controls = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_PROGRESS_CLASS,
    };
    if unsafe { InitCommonControlsEx(&controls) } == 0 {
        let _ = ready.send(0);
        return;
    }
    let initial_dpi = unsafe { GetDpiForSystem() }.max(96);
    let skin = match Skin::new(initial_dpi) {
        Ok(skin) => skin,
        Err(_) => {
            let _ = ready.send(0);
            return;
        }
    };
    let state = Box::new(State {
        receiver,
        cancelled,
        alive,
        response: RefCell::new(None),
        locale: locale.clone(),
        heading: Cell::new(ptr::null_mut()),
        subtitle: Cell::new(ptr::null_mut()),
        icon: Cell::new(ptr::null_mut()),
        status: Cell::new(ptr::null_mut()),
        detail: Cell::new(ptr::null_mut()),
        elapsed: Cell::new(ptr::null_mut()),
        progress: Cell::new(ptr::null_mut()),
        cancel: Cell::new(ptr::null_mut()),
        action: Cell::new(ptr::null_mut()),
        retry: Cell::new(ptr::null_mut()),
        font: Cell::new(ptr::null_mut()),
        heading_font: Cell::new(ptr::null_mut()),
        detail_font: Cell::new(ptr::null_mut()),
        small_font: Cell::new(ptr::null_mut()),
        version_label: Cell::new(ptr::null_mut()),
        version_value: Cell::new(ptr::null_mut()),
        minimize: Cell::new(ptr::null_mut()),
        skin: RefCell::new(skin),
        percent: Cell::new(0),
        dpi: Cell::new(unsafe { GetDpiForSystem() }.max(96)),
        started: Instant::now(),
        busy_started: Cell::new(Instant::now()),
        transfer: RefCell::new(None),
        busy: Cell::new(true),
        home: Cell::new(false),
        can_play: Cell::new(false),
        primary_action: Cell::new(LauncherAction::Proceed),
        secondary_action: Cell::new(LauncherAction::Retry),
        secondary_visible: Cell::new(false),
        probe: RefCell::new(None),
        version: RefCell::new(String::from("—")),
    });
    let ex_style = if locale_index(&locale) == 8 {
        WS_EX_APPWINDOW | WS_EX_RTLREADING
    } else {
        WS_EX_APPWINDOW
    };
    let mut bounds = RECT {
        left: 0,
        top: 0,
        right: px(WIDTH, state.dpi.get()),
        bottom: px(HEIGHT, state.dpi.get()),
    };
    unsafe {
        AdjustWindowRectExForDpi(&mut bounds, STYLE, 0, ex_style, state.dpi.get());
    }
    let title = wide(&format!(
        "Numeron - Legend of Rebirth · {}",
        stage(&locale, "title")
    ));
    let mut work = RECT::default();
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
    }
    let left = work.left + ((work.right - work.left - (bounds.right - bounds.left)) / 2).max(0);
    let top = work.top + ((work.bottom - work.top - (bounds.bottom - bounds.top)) / 2).max(0);
    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            CLASS,
            title.as_ptr(),
            STYLE,
            left,
            top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            (&*state as *const State).cast(),
        )
    };
    if hwnd.is_null() {
        let _ = ready.send(0);
        return;
    }
    let static_style = WS_CHILD | WS_VISIBLE | SS_NOPREFIX;
    for (slot, id, class, caption, style) in [
        (
            &state.icon,
            ICON_ID,
            windows_sys::core::w!("STATIC"),
            String::new(),
            WS_CHILD | SS_ICON,
        ),
        (
            &state.heading,
            HEADING_ID,
            windows_sys::core::w!("STATIC"),
            "NUMERON".into(),
            WS_CHILD | SS_NOPREFIX,
        ),
        (
            &state.subtitle,
            SUBTITLE_ID,
            windows_sys::core::w!("STATIC"),
            "L E G E N D  O F  R E B I R T H".into(),
            WS_CHILD | SS_NOPREFIX,
        ),
        (
            &state.status,
            STATUS_ID,
            windows_sys::core::w!("STATIC"),
            stage(&locale, "checking"),
            static_style,
        ),
        (
            &state.detail,
            DETAIL_ID,
            windows_sys::core::w!("STATIC"),
            stage(&locale, "checking-hint"),
            static_style,
        ),
        (
            &state.elapsed,
            ELAPSED_ID,
            windows_sys::core::w!("STATIC"),
            String::new(),
            static_style,
        ),
        (
            &state.progress,
            PROGRESS_ID,
            PROGRESS_CLASSW,
            String::new(),
            WS_CHILD,
        ),
        (
            &state.cancel,
            CANCEL_ID,
            windows_sys::core::w!("BUTTON"),
            stage(&locale, "cancel"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        ),
        (
            &state.action,
            ACTION_ID,
            windows_sys::core::w!("BUTTON"),
            stage(&locale, "working"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_DISABLED | BS_OWNERDRAW as u32,
        ),
        (
            &state.retry,
            RETRY_ID,
            windows_sys::core::w!("BUTTON"),
            stage(&locale, "retry"),
            WS_CHILD | WS_TABSTOP | BS_OWNERDRAW as u32,
        ),
        (
            &state.version_label,
            VERSION_LABEL_ID,
            windows_sys::core::w!("STATIC"),
            stage(&locale, "current-version"),
            static_style,
        ),
        (
            &state.version_value,
            VERSION_ID,
            windows_sys::core::w!("STATIC"),
            "—".into(),
            static_style | SS_ENDELLIPSIS,
        ),
        (
            &state.minimize,
            MINIMIZE_ID,
            windows_sys::core::w!("BUTTON"),
            stage(&locale, "minimize"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        ),
    ] {
        let child = unsafe {
            CreateWindowExW(
                ex_style,
                class,
                wide(&caption).as_ptr(),
                style,
                0,
                0,
                0,
                0,
                hwnd,
                id as _,
                instance,
                ptr::null(),
            )
        };
        slot.set(child);
        if child.is_null() {
            unsafe {
                DestroyWindow(hwnd);
            }
            let _ = ready.send(0);
            return;
        }
    }
    state.dpi.set(unsafe { GetDpiForWindow(hwnd) }.max(96));
    let mut bounds = RECT {
        left: 0,
        top: 0,
        right: px(WIDTH, state.dpi.get()),
        bottom: px(HEIGHT, state.dpi.get()),
    };
    unsafe {
        AdjustWindowRectExForDpi(&mut bounds, STYLE, 0, ex_style, state.dpi.get());
        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let small = LoadImageW(
            instance,
            1usize as _,
            IMAGE_ICON,
            px(16, state.dpi.get()),
            px(16, state.dpi.get()),
            LR_SHARED,
        );
        let large = LoadImageW(
            instance,
            1usize as _,
            IMAGE_ICON,
            px(32, state.dpi.get()),
            px(32, state.dpi.get()),
            LR_SHARED,
        );
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small as isize);
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
        SendMessageW(state.icon.get(), STM_SETICON, large as usize, 0);
        set_font(&state);
        layout(hwnd, &state);
        SendMessageW(state.progress.get(), PBM_SETRANGE32, 0, 100);
        set_progress(&state, 0, true);
        SetTimer(hwnd, 1, 500, None);
        refresh_metrics(&state);
        if visible {
            ShowWindow(hwnd, SW_SHOWNORMAL);
        }
    }
    if ready.send(hwnd as usize).is_err() {
        unsafe {
            DestroyWindow(hwnd);
        }
        return;
    }
    let mut message = MSG::default();
    loop {
        if unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } <= 0 {
            break;
        }
        if unsafe { IsDialogMessageW(hwnd, &message) } == 0 {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    if unsafe { IsWindow(hwnd) } != 0 {
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}

pub struct ProgressWindow {
    sender: mpsc::Sender<Command>,
    hwnd: usize,
    cancelled: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    owner: bool,
    created: Instant,
    last_transfer_ms: Arc<AtomicU64>,
}

impl Clone for ProgressWindow {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            hwnd: self.hwnd,
            cancelled: Arc::clone(&self.cancelled),
            alive: Arc::clone(&self.alive),
            owner: false,
            created: self.created,
            last_transfer_ms: Arc::clone(&self.last_transfer_ms),
        }
    }
}

impl ProgressWindow {
    pub fn spawn(locale: &str) -> Self {
        Self::spawn_impl(locale, true)
    }
    fn spawn_impl(locale: &str, visible: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let locale = locale.to_owned();
        let thread_cancelled = Arc::clone(&cancelled);
        let thread_alive = Arc::clone(&alive);
        let spawned = thread::Builder::new()
            .name("mir2-launcher-ui".into())
            .spawn(move || unsafe {
                run_window(
                    locale,
                    receiver,
                    thread_cancelled,
                    thread_alive,
                    ready_sender,
                    visible,
                );
            });
        let hwnd = if spawned.is_ok() {
            ready_receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or(0)
        } else {
            0
        };
        Self {
            sender,
            hwnd,
            cancelled,
            alive,
            owner: true,
            created: Instant::now(),
            last_transfer_ms: Arc::new(AtomicU64::new(0)),
        }
    }
    fn send(&self, command: Command) -> bool {
        self.hwnd != 0
            && self.alive.load(Ordering::Acquire)
            && self.sender.send(command).is_ok()
            && unsafe { PostMessageW(self.hwnd as HWND, STATUS_MESSAGE, 0, 0) } != 0
    }
    pub fn update(&self, message: &str, percent: u32) {
        self.send(Command::Update(message.into(), percent));
    }
    fn wait_for_choice(&self, receiver: mpsc::Receiver<LauncherAction>) -> LauncherAction {
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(action) if !self.cancelled() => return action,
                Err(mpsc::RecvTimeoutError::Timeout) if self.is_open() && !self.cancelled() => {}
                _ => return LauncherAction::Exit,
            }
        }
    }
    pub fn home(&self, _: &str, version: &str, can_play: bool) -> LauncherAction {
        let (response, receiver) = mpsc::sync_channel(1);
        if !self.send(Command::Home {
            version: version.into(),
            can_play,
            response,
        }) {
            return LauncherAction::Exit;
        }
        self.wait_for_choice(receiver)
    }
    pub fn probe_result(&self, _: &str, latest: Option<&str>, failed: bool) {
        self.send(Command::Probe(latest.map(str::to_owned), failed));
    }
    pub fn installed_available(&self, available: bool) {
        self.send(Command::Installed(available));
    }
    pub fn confirm_update_choice(
        &self,
        _: &str,
        version: &str,
        size: u64,
        can_play: bool,
    ) -> LauncherAction {
        let (response, receiver) = mpsc::sync_channel(1);
        if !self.send(Command::Choice {
            version: version.into(),
            bytes: size,
            can_play,
            response,
        }) {
            return LauncherAction::Exit;
        }
        self.wait_for_choice(receiver)
    }
    pub fn set_stage(&self, key: &str, percent: u32) {
        self.send(Command::Stage(key.into(), percent));
    }
    pub fn transfer(&self, received: u64, total: u64) {
        let now = self.created.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let last = self.last_transfer_ms.load(Ordering::Relaxed);
        if received != 0 && received < total && now.saturating_sub(last) < 100 {
            return;
        }
        self.last_transfer_ms.store(now, Ordering::Relaxed);
        self.send(Command::Transfer(received, total));
    }
    fn ask(
        &self,
        message: String,
        detail: String,
        action: String,
        percent: u32,
        allow_retry: bool,
    ) -> LauncherAction {
        if self.cancelled() {
            return LauncherAction::Exit;
        }
        let (response, receiver) = mpsc::sync_channel(1);
        if !self.send(Command::Prompt {
            message,
            detail,
            action,
            percent,
            allow_retry,
            response,
        }) {
            return LauncherAction::Exit;
        }
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(accepted) if !self.cancelled() => return accepted,
                Err(mpsc::RecvTimeoutError::Timeout) if !self.cancelled() => {}
                _ => return LauncherAction::Exit,
            }
        }
    }
    pub fn confirm_update(&self, locale: &str, version: &str, download_bytes: u64) -> bool {
        self.ask(
            stage(locale, "update-available"),
            format!(
                "{}\n{}",
                render(locale, "target-version", &[("version", version.into())]),
                render(locale, "download-size", &[("size", bytes(download_bytes))])
            ),
            stage(locale, "update"),
            0,
            false,
        ) == LauncherAction::Proceed
    }
    pub fn ready(&self, locale: &str, version: &str, fallback: bool) -> LauncherAction {
        self.send(Command::Installed(true));
        self.send(Command::Version(version.into()));
        self.ask(
            stage(locale, "home-ready"),
            format!(
                "{}\n{}",
                render(locale, "target-version", &[("version", version.into())]),
                stage(
                    locale,
                    if fallback {
                        "probe-failed"
                    } else {
                        "ready-hint"
                    }
                )
            ),
            stage(locale, "play"),
            100,
            fallback,
        )
    }
    pub fn retry(&self, locale: &str, error: &str) -> bool {
        // Full diagnostics remain in the existing bounded update log.
        let mut detail: String = error.chars().take(180).collect();
        if error.chars().count() > 180 {
            detail.push('…');
        }
        self.ask(
            stage(locale, "failed"),
            detail,
            stage(locale, "retry"),
            0,
            false,
        ) == LauncherAction::Proceed
    }
    pub fn close(&self) {
        self.send(Command::Close);
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn is_open(&self) -> bool {
        self.hwnd != 0 && unsafe { IsWindow(self.hwnd as HWND) } != 0
    }
    /// Serialize cancellation with removing the launcher's controls before spawn.
    pub fn begin_launch(&self) -> bool {
        if !self.is_open() || self.cancelled() {
            return false;
        }
        let (response, receiver) = mpsc::sync_channel(1);
        self.send(Command::BeginLaunch(response))
            && receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or(false)
    }
}
impl Drop for ProgressWindow {
    fn drop(&mut self) {
        if self.owner {
            self.close();
        }
    }
}

pub fn show_error(locale: &str, message: &str) {
    let flags = MB_OK
        | MB_ICONERROR
        | MB_SETFOREGROUND
        | if locale_index(locale) == 8 {
            MB_RTLREADING | MB_RIGHT
        } else {
            0
        };
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(message).as_ptr(),
            wide(&stage(locale, "failed")).as_ptr(),
            flags,
        );
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::{
        DrawTextW, GetDC, ReleaseDC, SelectObject, DT_CALCRECT, DT_NOPREFIX, DT_WORDBREAK,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

    fn flush(window: &ProgressWindow, message: u32, wparam: usize, lparam: isize) {
        let mut result = 0;
        assert_ne!(
            unsafe {
                SendMessageTimeoutW(
                    window.hwnd as HWND,
                    message,
                    wparam,
                    lparam,
                    SMTO_ABORTIFHUNG,
                    2000,
                    &mut result,
                )
            },
            0
        );
    }

    fn window_text(control: HWND) -> String {
        let mut value = vec![0u16; 2048];
        let length = unsafe { GetWindowTextW(control, value.as_mut_ptr(), value.len() as i32) };
        String::from_utf16_lossy(&value[..length.max(0) as usize])
    }

    fn control_text(window: &ProgressWindow, id: usize) -> String {
        window_text(unsafe { GetDlgItem(window.hwnd as HWND, id as i32) })
    }

    #[test]
    fn simplified_native_window_preserves_baseline_copy_and_only_arabic_uses_rtl() {
        for (locale, rtl) in [("zh-CN", false), ("zh-Hans", false), ("ar", true)] {
            let window = ProgressWindow::spawn_impl(locale, false);
            assert_ne!(window.hwnd, 0);
            let style = unsafe { GetWindowLongPtrW(window.hwnd as HWND, GWL_EXSTYLE) } as u32;
            assert_eq!(style & WS_EX_RTLREADING != 0, rtl, "{locale}");
            if !rtl {
                assert!(window_text(window.hwnd as HWND).ends_with("游戏更新"));
                window.update(&stage(locale, "extracting:3/7"), 30);
                flush(&window, STATUS_MESSAGE, 0, 0);
                assert_eq!(
                    control_text(&window, STATUS_ID),
                    "正在解压并验证文件…  (3 / 7)"
                );
            }
            window.close();
            flush(&window, STATUS_MESSAGE, 0, 0);
        }
    }

    #[test]
    fn successful_ready_exposes_simplified_local_play_without_an_idle_timer() {
        let window = ProgressWindow::spawn_impl("zh-CN", false);
        let ready_window = window.clone();
        let choice = thread::spawn(move || ready_window.ready("zh-CN", "r42", false));
        let deadline = Instant::now() + Duration::from_secs(2);
        while control_text(&window, ACTION_ID) != "开始游戏" {
            assert!(Instant::now() < deadline, "ready prompt was not delivered");
            flush(&window, STATUS_MESSAGE, 0, 0);
            thread::sleep(Duration::from_millis(5));
        }
        flush(&window, WM_TIMER, 1, 0);
        assert_eq!(control_text(&window, STATUS_ID), "准备就绪");
        assert_eq!(control_text(&window, VERSION_ID), "r42");
        assert!(control_text(&window, DETAIL_ID).contains("进入登录界面"));
        assert!(control_text(&window, ELAPSED_ID).is_empty());
        assert_ne!(
            unsafe { IsWindowEnabled(GetDlgItem(window.hwnd as HWND, ACTION_ID as i32)) },
            0
        );
        flush(&window, WM_COMMAND, ACTION_ID, 0);
        assert_eq!(choice.join().unwrap(), LauncherAction::Proceed);
        window.close();
        flush(&window, STATUS_MESSAGE, 0, 0);
    }

    #[test]
    fn home_caches_an_early_probe_and_keeps_local_play_independent_of_optional_updates() {
        let window = ProgressWindow::spawn_impl("en", false);
        window.probe_result("en", Some("r42"), false);
        let (response, receiver) = mpsc::sync_channel(1);
        window.send(Command::Home {
            version: "r41".into(),
            can_play: true,
            response,
        });
        flush(&window, STATUS_MESSAGE, 0, 0);
        assert!(control_text(&window, DETAIL_ID).contains("r42"));
        assert!(control_text(&window, ELAPSED_ID).is_empty());
        assert_ne!(
            unsafe { IsWindowEnabled(GetDlgItem(window.hwnd as HWND, ACTION_ID as i32)) },
            0
        );
        flush(&window, WM_COMMAND, ACTION_ID, 0);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            LauncherAction::PlayInstalled
        );
        assert!(!window.cancelled());
        window.begin_launch();
    }

    #[test]
    fn update_choice_distinguishes_skip_from_download_and_missing_install_disables_home_play() {
        let window = ProgressWindow::spawn_impl("en", false);
        for (id, expected) in [
            (ACTION_ID, LauncherAction::PlayInstalled),
            (RETRY_ID, LauncherAction::Proceed),
        ] {
            let (response, receiver) = mpsc::sync_channel(1);
            window.send(Command::Choice {
                version: "r42".into(),
                bytes: 1000,
                can_play: true,
                response,
            });
            flush(&window, STATUS_MESSAGE, 0, 0);
            flush(&window, WM_COMMAND, id, 0);
            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
                expected
            );
        }
        let (response, receiver) = mpsc::sync_channel(1);
        window.send(Command::Home {
            version: "—".into(),
            can_play: false,
            response,
        });
        flush(&window, STATUS_MESSAGE, 0, 0);
        flush(&window, WM_COMMAND, ACTION_ID, 0);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        flush(&window, WM_CLOSE, 0, 0);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            LauncherAction::Exit
        );
    }

    #[test]
    fn background_reporter_drop_does_not_close_the_window_and_late_results_are_ignored() {
        let window = ProgressWindow::spawn_impl("en", false);
        let reporter = window.clone();
        drop(reporter);
        flush(&window, STATUS_MESSAGE, 0, 0);
        assert!(window.is_open());
        let reporter = window.clone();
        assert!(window.begin_launch());
        reporter.probe_result("en", Some("late"), false);
        assert!(!reporter.is_open());
    }

    #[test]
    fn native_window_updates_and_distinguishes_programmatic_close_from_cancel() {
        let window = ProgressWindow::spawn_impl("zh-TW", false);
        assert_ne!(window.hwnd, 0);
        window.update("Native updater smoke", 61);
        flush(&window, STATUS_MESSAGE, 0, 0);
        let progress = unsafe { GetDlgItem(window.hwnd as HWND, PROGRESS_ID as i32) };
        assert_eq!(
            unsafe { SendMessageW(progress, windows_sys::Win32::UI::Controls::PBM_GETPOS, 0, 0) },
            61
        );
        window.close();
        flush(&window, STATUS_MESSAGE, 0, 0);
        assert!(!window.cancelled());
        let cancelled = ProgressWindow::spawn_impl("ar", false);
        flush(&cancelled, WM_CLOSE, 0, 0);
        assert!(cancelled.cancelled());
    }

    #[test]
    fn update_and_play_require_distinct_user_actions_and_busy_clicks_cannot_authorize_them() {
        let window = ProgressWindow::spawn_impl("en", false);
        flush(&window, WM_COMMAND, ACTION_ID, 0); // no prompt yet
        for (message, label) in [
            ("An update is available.", "Update"),
            ("Ready.", "Start game"),
        ] {
            let (response, receiver) = mpsc::sync_channel(1);
            window.send(Command::Prompt {
                message: message.into(),
                detail: String::new(),
                action: label.into(),
                percent: 0,
                allow_retry: false,
                response,
            });
            flush(&window, STATUS_MESSAGE, 0, 0);
            assert!(matches!(
                receiver.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            flush(&window, WM_COMMAND, ACTION_ID, 0);
            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
                LauncherAction::Proceed
            );
            assert_eq!(
                unsafe { IsWindowEnabled(GetDlgItem(window.hwnd as HWND, ACTION_ID as i32)) },
                0
            );
        }
        window.close();
        flush(&window, STATUS_MESSAGE, 0, 0);
    }

    #[test]
    fn close_releases_a_waiting_confirmation_without_authorizing_download() {
        let window = ProgressWindow::spawn_impl("en", false);
        let (response, receiver) = mpsc::sync_channel(1);
        window.send(Command::Prompt {
            message: "Update?".into(),
            detail: String::new(),
            action: "Update".into(),
            percent: 0,
            allow_retry: false,
            response,
        });
        flush(&window, STATUS_MESSAGE, 0, 0);
        flush(&window, WM_CLOSE, 0, 0);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            LauncherAction::Exit
        );
        assert!(window.cancelled());
    }

    #[test]
    fn launch_commit_closes_the_window_without_becoming_user_cancellation() {
        let window = ProgressWindow::spawn_impl("en", false);
        assert!(window.begin_launch());
        assert!(!window.is_open());
        assert!(!window.cancelled());
        assert!(!window.begin_launch());
        let cancelled = ProgressWindow::spawn_impl("en", false);
        flush(&cancelled, WM_CLOSE, 0, 0);
        assert!(!cancelled.begin_launch());
    }

    #[test]
    fn fallback_retry_is_distinct_from_launch_authorization() {
        let window = ProgressWindow::spawn_impl("en", false);
        let (response, receiver) = mpsc::sync_channel(1);
        window.send(Command::Prompt {
            message: stage("en", "offline-ready"),
            detail: String::new(),
            action: stage("en", "play"),
            percent: 100,
            allow_retry: true,
            response,
        });
        flush(&window, STATUS_MESSAGE, 0, 0);
        flush(&window, WM_COMMAND, RETRY_ID, 0);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            LauncherAction::Retry
        );
        assert!(!window.cancelled());
        window.close();
        flush(&window, STATUS_MESSAGE, 0, 0);
    }

    #[test]
    fn ten_locale_launcher_copy_fits_at_100_150_and_200_percent_layouts() {
        for locale in crate::SUPPORTED_LOCALES {
            let window = ProgressWindow::spawn_impl(locale, false);
            assert_ne!(window.hwnd, 0);
            for dpi in [96_u32, 144, 192] {
                let mut suggested = RECT {
                    left: 0,
                    top: 0,
                    right: px(WIDTH + 20, dpi),
                    bottom: px(HEIGHT + 50, dpi),
                };
                flush(
                    &window,
                    WM_DPICHANGED,
                    ((dpi << 16) | dpi) as usize,
                    (&mut suggested as *mut RECT) as isize,
                );
                for (id, key, value) in [
                    (
                        STATUS_ID,
                        "update-available",
                        stage(locale, "update-available"),
                    ),
                    (STATUS_ID, "home-ready", stage(locale, "home-ready")),
                    (
                        DETAIL_ID,
                        "home",
                        format!(
                            "{}\n{}",
                            render(locale, "optional-found", &[("version", "r42".into())]),
                            stage(locale, "play-without-update")
                        ),
                    ),
                    (
                        DETAIL_ID,
                        "probe-failed",
                        format!(
                            "{}\n{}",
                            stage(locale, "probe-failed"),
                            stage(locale, "play-without-update")
                        ),
                    ),
                    (
                        DETAIL_ID,
                        "update-plan",
                        format!(
                            "{}\n{}",
                            render(locale, "target-version", &[("version", "r42".into())]),
                            render(locale, "download-size", &[("size", "80.0 MiB".into())])
                        ),
                    ),
                ] {
                    let control = unsafe { GetDlgItem(window.hwnd as HWND, id as i32) };
                    let mut client = RECT::default();
                    assert_ne!(unsafe { GetClientRect(control, &mut client) }, 0);
                    let dc = unsafe { GetDC(control) };
                    let font = unsafe { SendMessageW(control, WM_GETFONT, 0, 0) } as _;
                    let old = unsafe { SelectObject(dc, font) };
                    let value = wide(&value);
                    let mut measured = RECT {
                        left: 0,
                        top: 0,
                        right: client.right,
                        bottom: 0,
                    };
                    let height = unsafe {
                        DrawTextW(
                            dc,
                            value.as_ptr(),
                            value.len() as i32 - 1,
                            &mut measured,
                            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
                        )
                    };
                    unsafe {
                        SelectObject(dc, old);
                        ReleaseDC(control, dc);
                    }
                    assert!(
                        height > 0 && height <= client.bottom,
                        "{locale}/{key}/{dpi}: {height} > {}",
                        client.bottom
                    );
                }
            }
            window.close();
            flush(&window, STATUS_MESSAGE, 0, 0);
        }
    }
}
