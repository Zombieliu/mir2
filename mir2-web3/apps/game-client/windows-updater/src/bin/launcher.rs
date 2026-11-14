#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{
    fs_safe as safe, generation, generation_runtime::GameLease, model::FileEntry, ui, update,
};
use std::{fs, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LauncherMode {
    Home,
    CheckOnly,
    VerifyOnly,
    PlayInstalled,
}
impl LauncherMode {
    fn parse(args: &[String]) -> Result<Self> {
        match args {
            [] => Ok(Self::Home),
            [arg] if arg == "--check-only" => Ok(Self::CheckOnly),
            [arg] if arg == "--verify-only" => Ok(Self::VerifyOnly),
            [arg] if arg == "--play-installed" => Ok(Self::PlayInstalled),
            _ => anyhow::bail!("unsupported launcher argument"),
        }
    }
    fn updater_argument(self) -> Option<&'static str> {
        match self {
            Self::CheckOnly => Some("--check-only"),
            Self::PlayInstalled => Some("--play-installed"),
            Self::Home | Self::VerifyOnly => None,
        }
    }
}

fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let root = safe::install_root(exe.parent().context("launcher directory")?)?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = LauncherMode::parse(&args)?;
    if mode == LauncherMode::VerifyOnly {
        let (engine, _) = update::active_engine(&root)?;
        if generation::active_record(&root)?.is_some() {
            let selected = generation::verify_current_selection(&root)?;
            update::log(
                &root,
                "verification",
                serde_json::json!({"selection":selected.active,"health":selected.health,"engine":engine.version,"passed":true}),
            );
            return Ok(());
        }
        if let Some(seed) = generation::verify_installed_seed(&root)? {
            update::log(
                &root,
                "verification",
                serde_json::json!({"offlineInstall":seed,"firstLaunchPending":true,"engine":engine.version,"passed":true}),
            );
            return Ok(());
        }
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
    // The actual file lease also detects a packed child whose parent launcher
    // died. A process-list result for the retained legacy EXE cannot do that.
    GameLease::ensure_idle(&root)?;
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
    if let Some(argument) = mode.updater_argument() {
        command.arg(argument);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn explicit_installed_play_is_forwarded_without_changing_existing_modes() {
        for (input, expected, forwarded) in [
            (vec![], LauncherMode::Home, None),
            (
                vec!["--check-only"],
                LauncherMode::CheckOnly,
                Some("--check-only"),
            ),
            (vec!["--verify-only"], LauncherMode::VerifyOnly, None),
            (
                vec!["--play-installed"],
                LauncherMode::PlayInstalled,
                Some("--play-installed"),
            ),
        ] {
            let mode = LauncherMode::parse(&args(&input)).unwrap();
            assert_eq!(mode, expected);
            assert_eq!(mode.updater_argument(), forwarded);
        }
    }

    #[test]
    fn launcher_rejects_combinations_duplicates_unknown_and_executable_arguments() {
        for input in [
            vec!["--play-installed", "--check-only"],
            vec!["--verify-only", "--play-installed"],
            vec!["--play-installed", "--play-installed"],
            vec!["--check-only", "--verify-only"],
            vec!["--play-installed=true"],
            vec!["--unknown"],
            vec![""],
            vec!["game.exe"],
            vec!["--play-installed", "game.exe"],
        ] {
            assert!(LauncherMode::parse(&args(&input)).is_err(), "{input:?}");
        }
    }
}
