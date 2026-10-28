#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{fs_safe as safe, platform, transaction, ui, update};
use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
struct Status {
    window: ui::ProgressWindow,
    locale: String,
}
impl update::Status for Status {
    fn set(&self, key: &str, percent: u32) {
        self.window.update(&ui::stage(&self.locale, key), percent);
    }
    fn cancelled(&self) -> bool {
        self.window.cancelled()
    }
}
fn wait_healthy(child: &mut Child) -> Result<bool> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(true)
}
fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let scratch = exe.parent().context("scratch engine directory")?;
    ensure!(
        scratch.file_name().is_some_and(|s| s == ".update"),
        "start the game through Mir2Launcher.exe"
    );
    let root = safe::install_root(scratch.parent().context("installation directory")?)?;
    let _lock = platform::InstallLock::acquire(&root).map_err(anyhow::Error::msg)?;
    ensure!(
        !platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is running"
    );
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(
        args.is_empty() || args == ["--check-only"],
        "unsupported updater argument"
    );
    let status = Status {
        locale: mir2_windows_updater::locale(),
        window: ui::ProgressWindow::spawn(&mir2_windows_updater::locale()),
    };
    let source = update::HttpsSource::new()?;
    let mut activated = transaction::pending_activation(&root)?;
    match update::check_update(&root, &source, &status) {
        Ok(outcome) => {
            activated = outcome.activated;
            update::log(&root, "update", outcome);
        }
        Err(error) => {
            update::log(&root, "update-error", format!("{error:#}"));
            transaction::recover(&root).context("recovery failed; launch prevented")?;
            if status.window.cancelled() {
                status.window.close();
                return Ok(());
            }
            update::verify_launch(&root).context("installed fallback is not valid")?;
            status
                .window
                .update(&ui::stage(&status.locale, "offline"), 100);
        }
    }
    if args == ["--check-only"] {
        status.window.close();
        return Ok(());
    }
    update::verify_launch(&root)?;
    if status.window.cancelled() {
        status.window.close();
        return Ok(());
    }
    let start = || {
        Command::new(root.join("game/mir2-platform-windows.exe"))
            .current_dir(root.join("game"))
            .spawn()
    };
    let mut child = match start() {
        Ok(child) => child,
        Err(error) => {
            if activated {
                update::quarantine_and_rollback(&root)?;
            }
            status.window.close();
            return Err(error)
                .context("Windows could not start the game; update restored where available");
        }
    };
    status.window.close();
    if activated {
        if !wait_healthy(&mut child)? {
            update::log(
                &root,
                "launch-rollback",
                "new game exited unsuccessfully within ten seconds",
            );
            update::quarantine_and_rollback(&root)?;
            update::verify_launch(&root)?;
            ui::show_error(&status.locale, &ui::stage(&status.locale, "recovered"));
            child = start().context("previous game could not start")?;
        } else {
            transaction::accept(&root)?;
        }
    }
    // Keep the per-install update lock for the child's entire lifetime.
    let exit = child.wait()?;
    update::log(&root, "game-exit", serde_json::json!({"code":exit.code()}));
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        let locale = mir2_windows_updater::locale();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(root) = exe.parent().and_then(|p| p.parent()) {
                update::log(root, "engine-error", format!("{error:#}"));
            }
        }
        ui::show_error(
            &locale,
            &format!("{}\n\n{error:#}", ui::stage(&locale, "failed")),
        );
        std::process::exit(1);
    }
}
