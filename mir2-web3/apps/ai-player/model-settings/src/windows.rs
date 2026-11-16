use numeron_alo_model_settings::{
    credential_target, metadata_path, prepare_metadata, read_configuration, read_metadata,
    valid_key, Metadata, Result, Secret, DEFAULT_MODEL, FIXED_ERROR, MAX_KEY_BYTES,
};
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{GetLastError, HWND};
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
use windows_sys::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};
use windows_sys::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE};
use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const SAVE_ID: usize = 100;
const CANCEL_ID: usize = 101;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct CredentialAllocation(*mut CREDENTIALW);
impl Drop for CredentialAllocation {
    fn drop(&mut self) {
        unsafe {
            let value = &mut *self.0;
            if !value.CredentialBlob.is_null() {
                for index in 0..value.CredentialBlobSize as usize {
                    std::ptr::write_volatile(value.CredentialBlob.add(index), 0);
                }
            }
            CredFree(self.0.cast());
        }
    }
}

fn read_credential(target: &str) -> Result<Option<Secret>> {
    let target = wide(target);
    let mut pointer = std::ptr::null_mut();
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) } == 0 {
        return if unsafe { GetLastError() } == 1168 {
            Ok(None)
        } else {
            Err(FIXED_ERROR)
        };
    }
    if pointer.is_null() {
        return Err(FIXED_ERROR);
    }
    let allocation = CredentialAllocation(pointer);
    let credential = unsafe { &*allocation.0 };
    if credential.CredentialBlob.is_null() || credential.CredentialBlobSize as usize > MAX_KEY_BYTES
    {
        return Err(FIXED_ERROR);
    }
    let bytes = Secret(
        unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        }
        .to_vec(),
    );
    if !valid_key(&bytes.0) {
        return Err(FIXED_ERROR);
    }
    Ok(Some(bytes))
}

fn write_credential(target: &str, key: &Secret) -> Result<()> {
    if !valid_key(&key.0) {
        return Err(FIXED_ERROR);
    }
    let target = wide(target);
    let username = wide("Numeron Alo Local OpenAI");
    let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
    credential.Type = CRED_TYPE_GENERIC;
    credential.TargetName = target.as_ptr().cast_mut();
    credential.UserName = username.as_ptr().cast_mut();
    credential.CredentialBlobSize = key.0.len() as u32;
    credential.CredentialBlob = key.0.as_ptr().cast_mut();
    // LOCAL_MACHINE persists across sessions for this Windows user only; it
    // neither grants other users access nor roams to another computer.
    credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        return Err(FIXED_ERROR);
    }
    Ok(())
}

fn delete_credential(target: &str) {
    unsafe {
        CredDeleteW(wide(target).as_ptr(), CRED_TYPE_GENERIC, 0);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialOutput<'a> {
    credential_target: &'a str,
    api_key: &'a str,
}

fn output_key(root: &std::path::Path, target: &str) -> Result<()> {
    // --read-key is an internal pipe protocol, never a console/file exporter.
    if unsafe { GetFileType(GetStdHandle(STD_OUTPUT_HANDLE)) } != FILE_TYPE_PIPE {
        return Err(FIXED_ERROR);
    }
    read_metadata(root, target)?;
    let secret = read_credential(target)?.ok_or(FIXED_ERROR)?;
    let key = std::str::from_utf8(&secret.0).map_err(|_| FIXED_ERROR)?;
    let encoded = Secret(
        serde_json::to_vec(&CredentialOutput {
            credential_target: target,
            api_key: key,
        })
        .map_err(|_| FIXED_ERROR)?,
    );
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&encoded.0).map_err(|_| FIXED_ERROR)?;
    stdout.flush().map_err(|_| FIXED_ERROR)
}

struct App {
    root: PathBuf,
    target: String,
    model: HWND,
    key: HWND,
}

fn control_text(window: HWND, maximum: usize) -> Result<String> {
    let mut units = vec![0u16; maximum + 1];
    let length = unsafe { GetWindowTextW(window, units.as_mut_ptr(), units.len() as i32) };
    if length < 0 {
        return Err(FIXED_ERROR);
    }
    String::from_utf16(&units[..length as usize]).map_err(|_| FIXED_ERROR)
}

fn control_secret(window: HWND) -> Result<Secret> {
    let mut units = vec![0u16; MAX_KEY_BYTES + 1];
    let length = unsafe { GetWindowTextW(window, units.as_mut_ptr(), units.len() as i32) };
    let result = if length < 0 {
        Err(FIXED_ERROR)
    } else {
        String::from_utf16(&units[..length as usize])
            .map(|text| Secret(text.into_bytes()))
            .map_err(|_| FIXED_ERROR)
    };
    for unit in &mut units {
        unsafe { std::ptr::write_volatile(unit, 0) };
    }
    result
}

fn save(app: &App) -> Result<()> {
    let model = control_text(app.model, 100)?.trim().to_owned();
    let entered = control_secret(app.key)?;
    save_configuration(&app.root, &app.target, model, &entered)
}

fn save_configuration(
    root: &std::path::Path,
    target: &str,
    model: String,
    entered: &Secret,
) -> Result<()> {
    let metadata = Metadata::new(model, target.to_owned())?;
    let previous = read_credential(target)?;
    if entered.0.is_empty() && previous.is_none() {
        return Err(FIXED_ERROR);
    }
    if !entered.0.is_empty() && !valid_key(&entered.0) {
        return Err(FIXED_ERROR);
    }
    let temporary = prepare_metadata(root, &metadata)?;
    if !entered.0.is_empty() && write_credential(target, entered).is_err() {
        let _ = std::fs::remove_file(temporary);
        return Err(FIXED_ERROR);
    }
    if std::fs::rename(&temporary, metadata_path(root)).is_err() {
        if !entered.0.is_empty() {
            if let Some(key) = &previous {
                let _ = write_credential(target, key);
            } else {
                delete_credential(target);
            }
        }
        let _ = std::fs::remove_file(temporary);
        return Err(FIXED_ERROR);
    }
    Ok(())
}

#[derive(Serialize)]
struct Saved<'a> {
    saved: bool,
    model: &'a str,
}

fn configure_stdin(root: &std::path::Path, target: &str) -> Result<()> {
    // OS pipe validation precedes every input read and every credential/file
    // write. A redirected file, console, NUL handle, or missing handle fails.
    if unsafe { GetFileType(GetStdHandle(STD_INPUT_HANDLE)) } != FILE_TYPE_PIPE {
        return Err(FIXED_ERROR);
    }
    let input = read_configuration(std::io::stdin().lock())?;
    save_configuration(root, target, input.model.clone(), &input.api_key)?;
    let output = serde_json::to_vec(&Saved {
        saved: true,
        model: &input.model,
    })
    .map_err(|_| FIXED_ERROR)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output).map_err(|_| FIXED_ERROR)?;
    stdout.flush().map_err(|_| FIXED_ERROR)
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if message == WM_COMMAND {
        let id = wparam & 0xffff;
        if id == SAVE_ID {
            let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut App;
            if !pointer.is_null() {
                if save(&*pointer).is_ok() {
                    SetWindowTextW((*pointer).key, wide("").as_ptr());
                    MessageBoxW(window, wide("已保存。密钥只在当前 Windows 用户的凭证管理器中。\n下次启动本地试用时生效；当前游戏不会被重启。\n保存未调用 OpenAI API。").as_ptr(), wide("阿洛 · 模型设置").as_ptr(), MB_OK | MB_ICONINFORMATION);
                    DestroyWindow(window);
                } else {
                    MessageBoxW(window, wide("未能保存。请检查模型名和 OpenAI API key；首次保存需要填写密钥。\n没有调用付费 API，也没有重置角色或记忆。").as_ptr(), wide("阿洛 · 模型设置").as_ptr(), MB_OK | MB_ICONERROR);
                }
            }
            return 0;
        }
        if id == CANCEL_ID {
            DestroyWindow(window);
            return 0;
        }
    }
    if message == WM_DESTROY {
        PostQuitMessage(0);
        return 0;
    }
    DefWindowProcW(window, message, wparam, lparam)
}

unsafe fn child(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: usize,
) -> Result<HWND> {
    let handle = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        width,
        height,
        parent,
        id as _,
        GetModuleHandleW(std::ptr::null()),
        std::ptr::null(),
    );
    if handle.is_null() {
        return Err(FIXED_ERROR);
    }
    SendMessageW(
        handle,
        WM_SETFONT,
        GetStockObject(DEFAULT_GUI_FONT) as usize,
        1,
    );
    Ok(handle)
}

fn gui(root: PathBuf, target: String) -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("NumeronAloModelSettingsV1");
        let definition = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: 6usize as _,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&definition) == 0 {
            return Err(FIXED_ERROR);
        }
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("阿洛 · OpenAI 模型设置").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            (GetSystemMetrics(SM_CXSCREEN) - 620) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - 350) / 2,
            620,
            350,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            return Err(FIXED_ERROR);
        }
        let result = (|| {
            child(
                window,
                "STATIC",
                "使用你自己的 OpenAI API key（输入已遮罩）",
                0,
                22,
                18,
                565,
                24,
                0,
            )?;
            child(window, "STATIC", "模型名", 0, 22, 58, 100, 24, 0)?;
            let model = child(
                window,
                "EDIT",
                DEFAULT_MODEL,
                WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                128,
                55,
                440,
                27,
                0,
            )?;
            SendMessageW(model, 0x00c5, 100, 0); // EM_LIMITTEXT
            if let Ok(metadata) = read_metadata(&root, &target) {
                SetWindowTextW(model, wide(&metadata.model).as_ptr());
            }
            child(window, "STATIC", "API key", 0, 22, 101, 100, 24, 0)?;
            let key = child(
                window,
                "EDIT",
                "",
                WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32 | ES_PASSWORD as u32,
                128,
                98,
                440,
                27,
                0,
            )?;
            SendMessageW(key, 0x00c5, MAX_KEY_BYTES, 0);
            child(window, "STATIC", "已有密钥时可留空保留。关闭窗口不会改动配置。\r\n上限：每小时 20 次模型调用。实际调用由你的 OpenAI 账户计费。\r\n保存仅写入本机设置，不发送 API 请求。", 0, 22, 145, 565, 79, 0)?;
            child(
                window,
                "BUTTON",
                "保存并启用",
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                325,
                245,
                125,
                33,
                SAVE_ID,
            )?;
            child(
                window, "BUTTON", "取消", WS_TABSTOP, 465, 245, 103, 33, CANCEL_ID,
            )?;
            let mut app = Box::new(App {
                root,
                target,
                model,
                key,
            });
            SetWindowLongPtrW(window, GWLP_USERDATA, (&mut *app as *mut App) as isize);
            ShowWindow(window, SW_SHOW);
            SetForegroundWindow(window);
            SetFocus(key);
            let mut message: MSG = std::mem::zeroed();
            loop {
                let result = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
                if result == 0 {
                    break;
                }
                if result < 0 {
                    return Err(FIXED_ERROR);
                }
                if IsDialogMessageW(window, &message) == 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            Ok(())
        })();
        if result.is_err() {
            DestroyWindow(window);
        }
        result
    }
}

pub fn run() -> Result<()> {
    let executable = std::env::current_exe().map_err(|_| FIXED_ERROR)?;
    let root = executable
        .parent()
        .ok_or(FIXED_ERROR)?
        .canonicalize()
        .map_err(|_| FIXED_ERROR)?;
    let target = credential_target(&root)?;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.is_empty() {
        return gui(root, target);
    }
    if arguments.len() == 1 && arguments[0] == "--read-key" {
        return output_key(&root, &target);
    }
    if arguments.len() == 1 && arguments[0] == "--configure-stdin" {
        return configure_stdin(&root, &target);
    }
    Err(FIXED_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_user_credential_roundtrip_is_bound_to_a_unique_temporary_bundle() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "numeron-model-settings-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let target = credential_target(&root).unwrap();
        assert!(read_credential(&target).unwrap().is_none());
        let key = Secret(b"sk-synthetic-test-only-never-an-api-key".to_vec());
        write_credential(&target, &key).unwrap();
        let value = read_credential(&target).unwrap().unwrap();
        let equal = value.0 == key.0;
        delete_credential(&target);
        std::fs::remove_dir(&root).unwrap();
        assert!(equal);
        assert!(read_credential(&target).unwrap().is_none());
    }
}
