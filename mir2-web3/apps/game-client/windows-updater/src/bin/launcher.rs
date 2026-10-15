#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{fs_safe as safe, model::FileEntry, ui, update};
use std::{fs, process::Command};
fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let root = safe::install_root(exe.parent().context("launcher directory")?)?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(
        args.is_empty() || args == ["--check-only"] || args == ["--verify-only"],
        "unsupported launcher argument"
    );
    if args == ["--verify-only"] {
        let (engine, _) = update::active_engine(&root)?;
        let candidate = update::verify_launch(&root)?;
        update::log(
            &root,
            "verification",
            serde_json::json!({"candidate":candidate,"engine":engine.version,"passed":true}),
        );
        return Ok(());
    }
    ensure!(
        !mir2_windows_updater::platform::game_is_running(
            &root.join("game/mir2-platform-windows.exe")
        )?,
        "game is already running; save and close before updating"
    );
    let lock =
        mir2_windows_updater::platform::InstallLock::acquire(&root).map_err(anyhow::Error::msg)?;
    let (engine, source) = update::active_engine(&root)?;
    let copy = safe::target(&root, &format!(".update/engine-{}.exe", engine.exe_sha256))?;
    let entry = FileEntry {
        path: "Mir2Updater.exe".into(),
        size: engine.exe_size,
        sha256: engine.exe_sha256.clone(),
    };
    if !safe::matches(&copy, &entry)? {
        if copy.exists() {
            safe::regular(&copy)?;
            fs::remove_file(&copy)?;
        }
        safe::write_new(&copy, &safe::read_bounded(&source, entry.size)?)?;
    }
    ensure!(
        safe::matches(&copy, &entry)?,
        "scratch engine integrity failed"
    );
    let mut command = Command::new(copy);
    command.current_dir(&root);
    if !args.is_empty() {
        command.arg("--check-only");
    }
    drop(lock);
    command
        .spawn()
        .context("could not start verified updater")?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        let locale = mir2_windows_updater::locale();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(root) = exe.parent() {
                update::log(root, "launcher-error", error.to_string());
            }
        }
        ui::show_error(
            &locale,
            &format!("{}\n\n{error:#}", ui::stage(&locale, "failed")),
        );
        std::process::exit(1);
    }
}
