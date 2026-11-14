#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
mod windows;

fn launch() -> io::Result<()> {
    let root = std::env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::other("missing launcher directory"))?
        .to_owned();
    #[cfg(target_os = "windows")]
    if windows::activate_game(&root) {
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    let _instance = match windows::Instance::acquire()? {
        Some(instance) => instance,
        None => {
            if !windows::activate_startup() {
                windows::report_already_running();
            }
            return Ok(());
        }
    };
    let data = root.join("apps/ai-player/.data/local-playtest");
    fs::create_dir_all(&data)?;
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(data.join("launcher.log"))?;
    writeln!(
        log,
        "{{\"type\":\"localLauncherStarted\",\"pid\":{}}}",
        std::process::id()
    )?;
    let mut command = Command::new(root.join("runtime/node.exe"));
    command
        .arg(root.join("apps/ai-player/src/local-playtest.mjs"))
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW for Node only.
    }
    #[cfg(target_os = "windows")]
    let mut progress = Some(windows::Progress::new()?);
    let mut child = command.spawn()?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(view) = &mut progress {
                view.update(started.elapsed().as_secs());
            }
            if windows::activate_game(&root) {
                drop(progress.take());
                break child.wait()?;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    #[cfg(target_os = "windows")]
    drop(progress.take());
    #[cfg(target_os = "windows")]
    if status.code() == Some(20) {
        if !windows::activate_game(&root) && !windows::activate_startup() {
            windows::report_already_running();
        }
        return Ok(());
    }
    if !status.success() {
        return Err(io::Error::other("local trial failed"));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn report_failure() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut core::ffi::c_void,
            text: *const u16,
            title: *const u16,
            flags: u32,
        ) -> i32;
    }
    let text: Vec<u16> = "阿洛本地试用未能启动。请查看同目录 apps/ai-player/.data/local-playtest/launcher.log。已有角色和记忆未重置；请确认没有另一份试用正在运行。\0"
        .encode_utf16().collect();
    let title: Vec<u16> = "Numeron 阿洛 · R16 本地试用\0".encode_utf16().collect();
    // Local fixed text only; no credentials, links or commands enter this dialog.
    unsafe {
        MessageBoxW(core::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
    }
}

fn main() {
    if launch().is_err() {
        #[cfg(target_os = "windows")]
        report_failure();
        std::process::exit(1);
    }
}
