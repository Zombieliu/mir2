//! Product launcher UI only. Existing games are matched by their full EXE path.
use std::ffi::c_void;
use std::io;
use std::path::Path;

type Handle = *mut c_void;
type Window = *mut c_void;
const CLASS: &str = "NumeronAloStartupProgressV2";

#[repr(C)]
struct WindowClass {
    style: u32,
    procedure: Option<unsafe extern "system" fn(Window, u32, usize, isize) -> isize>,
    class_extra: i32,
    window_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu: *const u16,
    class_name: *const u16,
}
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Message {
    window: Window,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16) -> Handle;
    fn GetLastError() -> u32;
    fn CloseHandle(handle: Handle) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn GetModuleHandleW(name: *const u16) -> Handle;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(Window, isize) -> i32>,
        context: isize,
    ) -> i32;
    fn IsWindowVisible(window: Window) -> i32;
    fn GetWindowThreadProcessId(window: Window, pid: *mut u32) -> u32;
    fn ShowWindow(window: Window, show: i32) -> i32;
    fn SetForegroundWindow(window: Window) -> i32;
    fn FindWindowW(class: *const u16, title: *const u16) -> Window;
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Window,
        menu: Handle,
        instance: Handle,
        param: *const c_void,
    ) -> Window;
    fn DefWindowProcW(window: Window, message: u32, wparam: usize, lparam: isize) -> isize;
    fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    fn GetSystemMetrics(index: i32) -> i32;
    fn SetWindowTextW(window: Window, text: *const u16) -> i32;
    fn SendMessageW(window: Window, message: u32, wparam: usize, lparam: isize) -> isize;
    fn PeekMessageW(message: *mut Message, window: Window, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn DestroyWindow(window: Window) -> i32;
    fn MessageBoxW(window: Window, text: *const u16, title: *const u16, flags: u32) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetStockObject(index: i32) -> Handle;
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn normalized(value: &str) -> String {
    value
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .to_lowercase()
}

pub struct Instance(Handle);
impl Instance {
    pub fn acquire() -> io::Result<Option<Self>> {
        let name = wide("Local\\NumeronAloLocalTrialLauncherV2");
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        if unsafe { GetLastError() } == 183 {
            unsafe {
                CloseHandle(handle);
            }
            return Ok(None);
        }
        Ok(Some(Self(handle)))
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Search {
    expected: String,
    found: Window,
}
unsafe extern "system" fn find_game(window: Window, context: isize) -> i32 {
    if IsWindowVisible(window) == 0 {
        return 1;
    }
    let search = &mut *(context as *mut Search);
    let mut pid = 0;
    GetWindowThreadProcessId(window, &mut pid);
    let process = OpenProcess(0x1000, 0, pid);
    if process.is_null() {
        return 1;
    }
    let mut name = vec![0u16; 32_768];
    let mut size = name.len() as u32;
    let read = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut size);
    CloseHandle(process);
    if read != 0 && normalized(&String::from_utf16_lossy(&name[..size as usize])) == search.expected
    {
        search.found = window;
        return 0;
    }
    1
}
fn activate(window: Window) -> bool {
    if window.is_null() {
        return false;
    }
    unsafe {
        ShowWindow(window, 9);
        SetForegroundWindow(window);
    }
    true
}
pub fn activate_game(root: &Path) -> bool {
    let path = root.join("game/mir2-platform-windows.exe");
    let mut search = Search {
        expected: normalized(&path.to_string_lossy()),
        found: std::ptr::null_mut(),
    };
    unsafe {
        EnumWindows(Some(find_game), &mut search as *mut Search as isize);
    }
    activate(search.found)
}
pub fn activate_startup() -> bool {
    let class = wide(CLASS);
    activate(unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) })
}
pub fn report_already_running() {
    let text = wide(
        "另一份阿洛试用正在启动或运行。请等待现有启动完成；游戏打开后会继续使用已有角色和记忆。",
    );
    let title = wide("Numeron 阿洛 · 已在启动");
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x40);
    }
}
unsafe extern "system" fn procedure(
    window: Window,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    // Initialization is owned by the child lifecycle; closing this view cannot
    // interrupt account/store writes. No close button is offered while loading.
    if message == 0x10 {
        return 0;
    }
    DefWindowProcW(window, message, wparam, lparam)
}

pub struct Progress {
    window: Window,
    label: Window,
    last_second: Option<u64>,
}
impl Progress {
    pub fn new() -> io::Result<Self> {
        let class = wide(CLASS);
        let title = wide("Numeron 阿洛 · 正在启动");
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let definition = WindowClass {
            style: 0,
            procedure: Some(procedure),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: std::ptr::null_mut(),
            cursor: unsafe { LoadCursorW(std::ptr::null_mut(), 32512usize as *const u16) },
            background: 6usize as Handle,
            menu: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        if unsafe { RegisterClassW(&definition) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                0x10c0_0000,
                (GetSystemMetrics(0) - 560) / 2,
                (GetSystemMetrics(1) - 200) / 2,
                560,
                200,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        if window.is_null() {
            return Err(io::Error::last_os_error());
        }
        let label_class = wide("STATIC");
        let label = unsafe {
            CreateWindowExW(
                0,
                label_class.as_ptr(),
                wide("").as_ptr(),
                0x5000_0001,
                18,
                28,
                520,
                120,
                window,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        if label.is_null() {
            unsafe {
                DestroyWindow(window);
            }
            return Err(io::Error::last_os_error());
        }
        unsafe {
            SendMessageW(label, 0x30, GetStockObject(17) as usize, 1);
        }
        activate(window);
        let mut view = Self {
            window,
            label,
            last_second: None,
        };
        view.update(0);
        Ok(view)
    }
    pub fn update(&mut self, seconds: u64) {
        if self.last_second != Some(seconds) {
            let text = wide(&format!("正在准备本地世界和阿洛，已等待 {seconds} 秒。\r\n\r\n首次需要初始化角色；完成后会自动打开游戏。\r\n重复打开会回到现有窗口，请稍候。"));
            unsafe {
                SetWindowTextW(self.label, text.as_ptr());
            }
            self.last_second = Some(seconds);
        }
        unsafe {
            let mut message: Message = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, 1) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
impl Drop for Progress {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_match_handles_mixed_separators_without_matching_another_directory() {
        assert_eq!(
            normalized(r"\\?\F:\Trial\game/mir2-platform-windows.exe"),
            normalized(r"F:\trial\game\mir2-platform-windows.exe")
        );
        assert_ne!(
            normalized(r"F:\Trial\game/mir2-platform-windows.exe"),
            normalized(r"F:\Other\game\mir2-platform-windows.exe")
        );
    }
    #[test]
    fn startup_view_renders_and_processes_messages_then_closes() {
        let mut view = Progress::new().unwrap();
        assert_ne!(unsafe { IsWindowVisible(view.window) }, 0);
        for second in 0..20 {
            view.update(second);
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        // Drop releases the actual OS window; no Node, world, account or file
        // is opened in this presentation-only smoke test.
    }
}
