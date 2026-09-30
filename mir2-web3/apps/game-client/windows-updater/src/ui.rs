//! Thread-owned, DPI-aware native progress window. No console or elevated shell.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn locale_index(locale: &str) -> usize {
    let locale = locale.replace('_', "-").to_ascii_lowercase();
    match locale.split('-').next().unwrap_or("en") {
        "zh" => 1,
        "pt" => 2,
        "ru" => 3,
        "hi" => 4,
        "id" => 5,
        "vi" => 6,
        "th" => 7,
        "ar" => 8,
        _ => 0,
    }
}

/// These small native strings are independent of the game catalog version.
pub fn stage(locale: &str, key: &str) -> String {
    let strings: [&str; 9] = match key {
        "checking" => [
            "Checking for updates…",
            "正在檢查更新…",
            "Verificando atualizações…",
            "Проверка обновлений…",
            "अपडेट की जाँच हो रही है…",
            "Memeriksa pembaruan…",
            "Đang kiểm tra cập nhật…",
            "กำลังตรวจสอบการอัปเดต…",
            "جارٍ التحقق من التحديثات…",
        ],
        "downloading" => [
            "Downloading changed files…",
            "正在下載變更的檔案…",
            "Baixando arquivos alterados…",
            "Загрузка изменённых файлов…",
            "बदली गई फ़ाइलें डाउनलोड हो रही हैं…",
            "Mengunduh file yang berubah…",
            "Đang tải các tệp đã thay đổi…",
            "กำลังดาวน์โหลดไฟล์ที่เปลี่ยนแปลง…",
            "جارٍ تنزيل الملفات المعدّلة…",
        ],
        "verifying" => [
            "Verifying downloaded files…",
            "正在驗證下載的檔案…",
            "Verificando os arquivos baixados…",
            "Проверка загруженных файлов…",
            "डाउनलोड की गई फ़ाइलों की जाँच हो रही है…",
            "Memverifikasi file unduhan…",
            "Đang xác minh các tệp đã tải…",
            "กำลังตรวจสอบไฟล์ที่ดาวน์โหลด…",
            "جارٍ التحقق من الملفات المنزّلة…",
        ],
        "applying" => [
            "Installing the update…",
            "正在套用更新…",
            "Instalando a atualização…",
            "Установка обновления…",
            "अपडेट इंस्टॉल हो रहा है…",
            "Memasang pembaruan…",
            "Đang cài đặt bản cập nhật…",
            "กำลังติดตั้งการอัปเดต…",
            "جارٍ تثبيت التحديث…",
        ],
        "recovering" => [
            "Recovering the previous version…",
            "正在還原先前的版本…",
            "Restaurando a versão anterior…",
            "Восстановление предыдущей версии…",
            "पिछला संस्करण बहाल हो रहा है…",
            "Memulihkan versi sebelumnya…",
            "Đang khôi phục phiên bản trước…",
            "กำลังกู้คืนเวอร์ชันก่อนหน้า…",
            "جارٍ استعادة الإصدار السابق…",
        ],
        "recovered" => [
            "The previous version has been restored.",
            "已還原先前的版本。",
            "A versão anterior foi restaurada.",
            "Предыдущая версия восстановлена.",
            "पिछला संस्करण बहाल कर दिया गया है।",
            "Versi sebelumnya telah dipulihkan.",
            "Đã khôi phục phiên bản trước.",
            "กู้คืนเวอร์ชันก่อนหน้าแล้ว",
            "تمت استعادة الإصدار السابق.",
        ],
        "launching" => [
            "Starting the game…",
            "正在啟動遊戲…",
            "Iniciando o jogo…",
            "Запуск игры…",
            "गेम शुरू हो रहा है…",
            "Memulai permainan…",
            "Đang khởi động trò chơi…",
            "กำลังเริ่มเกม…",
            "جارٍ تشغيل اللعبة…",
        ],
        "offline" => [
            "The update server is unavailable. Using the installed version.",
            "無法連線至更新伺服器，將使用已安裝的版本。",
            "Servidor de atualização indisponível. Usando a versão instalada.",
            "Сервер обновлений недоступен. Используется установленная версия.",
            "अपडेट सर्वर उपलब्ध नहीं है। इंस्टॉल किया गया संस्करण इस्तेमाल होगा।",
            "Server pembaruan tidak tersedia. Menggunakan versi terpasang.",
            "Máy chủ cập nhật không khả dụng. Sử dụng phiên bản đã cài đặt.",
            "เซิร์ฟเวอร์อัปเดตไม่พร้อมใช้งาน จะใช้เวอร์ชันที่ติดตั้งไว้",
            "خادم التحديث غير متاح. سيُستخدم الإصدار المثبّت.",
        ],
        "busy" => [
            "Please close the game before updating.",
            "請先關閉遊戲再更新。",
            "Feche o jogo antes de atualizar.",
            "Закройте игру перед обновлением.",
            "अपडेट से पहले गेम बंद करें।",
            "Tutup permainan sebelum memperbarui.",
            "Vui lòng đóng trò chơi trước khi cập nhật.",
            "กรุณาปิดเกมก่อนอัปเดต",
            "يرجى إغلاق اللعبة قبل التحديث.",
        ],
        "failed" => [
            "Update failed",
            "更新失敗",
            "Falha na atualização",
            "Ошибка обновления",
            "अपडेट विफल हुआ",
            "Pembaruan gagal",
            "Cập nhật thất bại",
            "อัปเดตไม่สำเร็จ",
            "فشل التحديث",
        ],
        "cancel" => [
            "Cancel",
            "取消",
            "Cancelar",
            "Отмена",
            "रद्द करें",
            "Batal",
            "Hủy",
            "ยกเลิก",
            "إلغاء",
        ],
        "title" => [
            "Game update",
            "遊戲更新",
            "Atualização do jogo",
            "Обновление игры",
            "गेम अपडेट",
            "Pembaruan permainan",
            "Cập nhật trò chơi",
            "อัปเดตเกม",
            "تحديث اللعبة",
        ],
        _ => return key.to_string(),
    };
    strings[locale_index(locale)].into()
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{cell::Cell, mem::size_of, ptr, sync::mpsc, thread, time::Duration};
    use windows_sys::Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            CreateFontW, DeleteObject, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW,
            DEFAULT_CHARSET, DEFAULT_PITCH, HFONT, OUT_DEFAULT_PRECIS,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{
                InitCommonControlsEx, ICC_PROGRESS_CLASS, INITCOMMONCONTROLSEX, PBM_SETPOS,
                PBM_SETRANGE32, PROGRESS_CLASSW,
            },
            HiDpi::{
                AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow,
                SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            },
            WindowsAndMessaging::*,
        },
    };

    const STATUS_MESSAGE: u32 = WM_APP + 10;
    const CANCEL_ID: usize = 101;
    const HEADING_ID: usize = 102;
    const STATUS_ID: usize = 103;
    const PROGRESS_ID: usize = 104;
    const CLASS: *const u16 = windows_sys::core::w!("NumeronMir2UpdaterProgressV1");
    const STYLE: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

    fn wide(text: &str) -> Vec<u16> {
        // Server diagnostics cannot introduce embedded NULs to native controls.
        text.chars()
            .map(|c| if c == '\0' { '�' } else { c })
            .collect::<String>()
            .encode_utf16()
            .chain(Some(0))
            .collect()
    }

    enum Command {
        Update(String, u32),
        Close,
    }

    struct State {
        receiver: mpsc::Receiver<Command>,
        cancelled: Arc<AtomicBool>,
        heading: Cell<HWND>,
        status: Cell<HWND>,
        progress: Cell<HWND>,
        cancel: Cell<HWND>,
        font: Cell<HFONT>,
        dpi: Cell<u32>,
    }
    impl Drop for State {
        fn drop(&mut self) {
            if !self.font.get().is_null() {
                unsafe {
                    DeleteObject(self.font.get());
                }
            }
        }
    }

    /// Only the UI thread reads or mutates State. Its Box outlives the window.
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
        let state_pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const State;
        if state_pointer.is_null() {
            return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
        }
        // Shared access plus Cells permits synchronous Win32 message reentry
        // (DestroyWindow, SetWindowPos, WM_SETFONT) without aliasing &mut State.
        let state = unsafe { &*state_pointer };
        match message {
            STATUS_MESSAGE => {
                while let Ok(command) = state.receiver.try_recv() {
                    match command {
                        Command::Update(text, percentage) => {
                            let text = wide(&text);
                            unsafe {
                                SetWindowTextW(state.status.get(), text.as_ptr());
                                SendMessageW(
                                    state.progress.get(),
                                    PBM_SETPOS,
                                    percentage.min(100) as WPARAM,
                                    0,
                                );
                            }
                        }
                        Command::Close => {
                            unsafe {
                                DestroyWindow(hwnd);
                            }
                            return 0;
                        }
                    }
                }
                0
            }
            WM_COMMAND if wparam & 0xffff == CANCEL_ID => {
                state.cancelled.store(true, Ordering::Release);
                unsafe {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_CLOSE => {
                state.cancelled.store(true, Ordering::Release);
                unsafe {
                    DestroyWindow(hwnd);
                }
                0
            }
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
                    set_font(state);
                    layout(hwnd, state);
                }
                0
            }
            WM_DESTROY => {
                unsafe {
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

    unsafe fn set_font(state: &State) {
        let font = unsafe {
            CreateFontW(
                -px(15, state.dpi.get()),
                0,
                0,
                0,
                400,
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
        };
        if font.is_null() {
            return;
        }
        let old_font = state.font.replace(font);
        for control in [state.heading.get(), state.status.get(), state.cancel.get()] {
            if !control.is_null() {
                unsafe {
                    SendMessageW(control, WM_SETFONT, font as WPARAM, 1);
                }
            }
        }
        if !old_font.is_null() {
            unsafe {
                DeleteObject(old_font);
            }
        }
    }

    unsafe fn layout(hwnd: HWND, state: &State) {
        let mut bounds = RECT::default();
        if unsafe { GetClientRect(hwnd, &mut bounds) } == 0 {
            return;
        }
        let margin = px(24, state.dpi.get());
        let width = (bounds.right - margin * 2).max(px(100, state.dpi.get()));
        let dpi = state.dpi.get();
        for (control, x, y, width, height) in [
            (state.heading.get(), margin, px(18, dpi), width, px(26, dpi)),
            (state.status.get(), margin, px(58, dpi), width, px(86, dpi)),
            (
                state.progress.get(),
                margin,
                px(155, dpi),
                width,
                px(20, dpi),
            ),
            (
                state.cancel.get(),
                bounds.right - margin - px(118, dpi),
                px(196, dpi),
                px(118, dpi),
                px(32, dpi),
            ),
        ] {
            if !control.is_null() {
                unsafe {
                    MoveWindow(control, x, y, width, height, 1);
                }
            }
        }
    }

    unsafe fn run_window(
        locale: String,
        receiver: mpsc::Receiver<Command>,
        cancelled: Arc<AtomicBool>,
        ready: mpsc::SyncSender<usize>,
        visible: bool,
    ) {
        unsafe {
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
            hbrBackground: (COLOR_WINDOW + 1) as _,
            lpszClassName: CLASS,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 && unsafe { GetLastError() } != 1410 {
            let _ = ready.send(0);
            return;
        }
        let common_controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS,
        };
        if unsafe { InitCommonControlsEx(&common_controls) } == 0 {
            let _ = ready.send(0);
            return;
        }
        let state = Box::new(State {
            receiver,
            cancelled,
            heading: Cell::new(ptr::null_mut()),
            status: Cell::new(ptr::null_mut()),
            progress: Cell::new(ptr::null_mut()),
            cancel: Cell::new(ptr::null_mut()),
            font: Cell::new(ptr::null_mut()),
            dpi: Cell::new(unsafe { GetDpiForSystem() }.max(96)),
        });
        let title = wide(&format!(
            "Numeron - Legend of Rebirth · {}",
            stage(&locale, "title")
        ));
        let ex_style = if locale_index(&locale) == 8 {
            WS_EX_LAYOUTRTL | WS_EX_RTLREADING
        } else {
            0
        };
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: px(580, state.dpi.get()),
            bottom: px(252, state.dpi.get()),
        };
        unsafe {
            AdjustWindowRectExForDpi(&mut bounds, STYLE, 0, ex_style, state.dpi.get());
        }
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                CLASS,
                title.as_ptr(),
                STYLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
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
        let heading = wide(&stage(&locale, "title"));
        let status = wide(&stage(&locale, "checking"));
        let cancel = wide(&stage(&locale, "cancel"));
        let child_style = WS_CHILD | WS_VISIBLE;
        unsafe {
            state.heading.set(CreateWindowExW(
                ex_style,
                windows_sys::core::w!("STATIC"),
                heading.as_ptr(),
                child_style,
                0,
                0,
                0,
                0,
                hwnd,
                HEADING_ID as _,
                instance,
                ptr::null(),
            ));
            state.status.set(CreateWindowExW(
                ex_style,
                windows_sys::core::w!("STATIC"),
                status.as_ptr(),
                child_style,
                0,
                0,
                0,
                0,
                hwnd,
                STATUS_ID as _,
                instance,
                ptr::null(),
            ));
            state.progress.set(CreateWindowExW(
                0,
                PROGRESS_CLASSW,
                ptr::null(),
                child_style,
                0,
                0,
                0,
                0,
                hwnd,
                PROGRESS_ID as _,
                instance,
                ptr::null(),
            ));
            state.cancel.set(CreateWindowExW(
                ex_style,
                windows_sys::core::w!("BUTTON"),
                cancel.as_ptr(),
                child_style | WS_TABSTOP | BS_PUSHBUTTON as u32,
                0,
                0,
                0,
                0,
                hwnd,
                CANCEL_ID as _,
                instance,
                ptr::null(),
            ));
        }
        if [
            state.heading.get(),
            state.status.get(),
            state.progress.get(),
            state.cancel.get(),
        ]
        .iter()
        .any(|handle| handle.is_null())
        {
            unsafe {
                DestroyWindow(hwnd);
            }
            let _ = ready.send(0);
            return;
        }
        state.dpi.set(unsafe { GetDpiForWindow(hwnd) }.max(96));
        // CreateWindow's initial monitor can have a different DPI from the system.
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: px(580, state.dpi.get()),
            bottom: px(252, state.dpi.get()),
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
            set_font(&state);
            layout(hwnd, &state);
            SendMessageW(state.progress.get(), PBM_SETRANGE32, 0, 100);
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
            let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
            if result <= 0 {
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
        // State/its font are dropped only after the owned window is destroyed.
    }

    pub struct ProgressWindow {
        sender: mpsc::Sender<Command>,
        hwnd: usize,
        cancelled: Arc<AtomicBool>,
    }
    impl ProgressWindow {
        pub fn spawn(locale: &str) -> Self {
            Self::spawn_impl(locale, true)
        }
        fn spawn_impl(locale: &str, visible: bool) -> Self {
            let (sender, receiver) = mpsc::channel();
            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            let cancelled = Arc::new(AtomicBool::new(false));
            let locale = locale.to_owned();
            let thread_cancelled = Arc::clone(&cancelled);
            let spawned =
                thread::Builder::new()
                    .name("mir2-updater-ui".into())
                    .spawn(move || unsafe {
                        run_window(locale, receiver, thread_cancelled, ready_sender, visible);
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
            }
        }
        pub fn update(&self, message: &str, percent: u32) {
            if self.hwnd != 0
                && self
                    .sender
                    .send(Command::Update(message.into(), percent))
                    .is_ok()
            {
                unsafe {
                    PostMessageW(self.hwnd as HWND, STATUS_MESSAGE, 0, 0);
                }
            }
        }
        pub fn close(&self) {
            if self.hwnd != 0 && self.sender.send(Command::Close).is_ok() {
                unsafe {
                    PostMessageW(self.hwnd as HWND, STATUS_MESSAGE, 0, 0);
                }
            }
        }
        pub fn cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
        }
    }
    impl Drop for ProgressWindow {
        fn drop(&mut self) {
            self.close();
        }
    }

    pub fn show_error(locale: &str, message: &str) {
        let caption = wide(&stage(locale, "failed"));
        let message = wide(message);
        let flags = MB_OK
            | MB_ICONERROR
            | MB_SETFOREGROUND
            | if locale_index(locale) == 8 {
                MB_RTLREADING | MB_RIGHT
            } else {
                0
            };
        unsafe {
            MessageBoxW(ptr::null_mut(), message.as_ptr(), caption.as_ptr(), flags);
        }
    }

    #[cfg(test)]
    mod native_tests {
        use super::*;
        use windows_sys::Win32::Graphics::Gdi::{
            DrawTextW, GetDC, ReleaseDC, SelectObject, DT_CALCRECT, DT_NOPREFIX, DT_WORDBREAK,
        };

        fn flush(window: &ProgressWindow, message: u32, wparam: usize, lparam: isize) {
            let mut result = 0;
            let sent = unsafe {
                SendMessageTimeoutW(
                    window.hwnd as HWND,
                    message,
                    wparam,
                    lparam,
                    SMTO_ABORTIFHUNG,
                    2000,
                    &mut result,
                )
            };
            assert_ne!(sent, 0, "native UI did not process the test message");
        }

        #[test]
        fn native_window_updates_and_distinguishes_programmatic_close_from_cancel() {
            let window = ProgressWindow::spawn_impl("zh-TW", false);
            assert_ne!(window.hwnd, 0);
            window.update("Native updater smoke", 61);
            flush(&window, STATUS_MESSAGE, 0, 0);
            let progress = unsafe { GetDlgItem(window.hwnd as HWND, PROGRESS_ID as i32) };
            assert_eq!(
                unsafe {
                    SendMessageW(progress, windows_sys::Win32::UI::Controls::PBM_GETPOS, 0, 0)
                },
                61
            );
            window.close();
            flush(&window, STATUS_MESSAGE, 0, 0);
            assert!(!window.cancelled());

            let cancelled = ProgressWindow::spawn_impl("ar", false);
            assert_ne!(cancelled.hwnd, 0);
            flush(&cancelled, WM_CLOSE, 0, 0);
            assert!(cancelled.cancelled());
        }

        #[test]
        fn nine_locale_offline_copy_fits_native_status_at_100_150_and_200_percent_layouts() {
            for locale in ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"] {
                let window = ProgressWindow::spawn_impl(locale, false);
                assert_ne!(window.hwnd, 0);
                let text = stage(locale, "offline");
                window.update(&text, 5);
                flush(&window, STATUS_MESSAGE, 0, 0);
                for dpi in [96_u32, 144, 192] {
                    let mut suggested = RECT {
                        left: 0,
                        top: 0,
                        right: px(600, dpi),
                        bottom: px(300, dpi),
                    };
                    flush(
                        &window,
                        WM_DPICHANGED,
                        ((dpi << 16) | dpi) as usize,
                        (&mut suggested as *mut RECT) as isize,
                    );
                    let status = unsafe { GetDlgItem(window.hwnd as HWND, STATUS_ID as i32) };
                    let mut client = RECT::default();
                    assert_ne!(unsafe { GetClientRect(status, &mut client) }, 0);
                    let dc = unsafe { GetDC(status) };
                    assert!(!dc.is_null());
                    let font = unsafe { SendMessageW(status, WM_GETFONT, 0, 0) } as _;
                    let old = unsafe { SelectObject(dc, font) };
                    let text = wide(&text);
                    let mut measured = RECT {
                        left: 0,
                        top: 0,
                        right: client.right,
                        bottom: 0,
                    };
                    let height = unsafe {
                        DrawTextW(
                            dc,
                            text.as_ptr(),
                            text.len() as i32 - 1,
                            &mut measured,
                            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
                        )
                    };
                    unsafe {
                        SelectObject(dc, old);
                        ReleaseDC(status, dc);
                    }
                    assert!(
                        height > 0 && height <= client.bottom,
                        "{locale} at {dpi} DPI needs {height} px but status has {}",
                        client.bottom
                    );
                }
                window.close();
                flush(&window, STATUS_MESSAGE, 0, 0);
            }
        }
    }
}

#[cfg(windows)]
pub use windows::{show_error, ProgressWindow};

#[cfg(not(windows))]
pub struct ProgressWindow {
    cancelled: Arc<AtomicBool>,
}
#[cfg(not(windows))]
impl ProgressWindow {
    pub fn spawn(_: &str) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn update(&self, _: &str, _: u32) {}
    pub fn close(&self) {}
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}
#[cfg(not(windows))]
pub fn show_error(_: &str, _: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_supported_locale_has_every_native_update_stage() {
        for locale in ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"] {
            for key in [
                "checking",
                "downloading",
                "verifying",
                "applying",
                "recovering",
                "recovered",
                "launching",
                "offline",
                "busy",
                "failed",
                "cancel",
                "title",
            ] {
                let translated = stage(locale, key);
                assert!(!translated.is_empty());
                assert_ne!(translated, key);
                assert!(!translated.contains('\0'));
            }
        }
    }
    #[test]
    fn unknown_locale_uses_english_and_region_variants_are_normalized() {
        assert_eq!(stage("xx-ZZ", "checking"), stage("en", "checking"));
        assert_eq!(stage("pt_BR", "checking"), stage("pt-BR", "checking"));
        assert_eq!(stage("ru-RU", "checking"), stage("ru", "checking"));
    }
}
