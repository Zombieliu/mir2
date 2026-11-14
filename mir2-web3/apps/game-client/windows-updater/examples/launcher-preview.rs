//! Offline UI fixture. All versions, downloads and errors are synthetic.
//! Never reads an installation, contacts a server or starts a game.
#![cfg_attr(windows, windows_subsystem = "windows")]
use mir2_windows_updater::ui::{LauncherAction, ProgressWindow};
use std::{thread, time::Duration};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let locale = args
        .iter()
        .find_map(|value| value.strip_prefix("--locale="))
        .unwrap_or("zh-TW")
        .to_owned();
    let failure = args.iter().any(|value| value == "--failure");
    let window = ProgressWindow::spawn(&locale);
    window.installed_available(true);
    let reporter = window.clone();
    let probe_locale = locale.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        reporter.probe_result(
            &probe_locale,
            if failure { None } else { Some("r42") },
            failure,
        );
    });
    match window.home(&locale, "r41 · UI PREVIEW", true) {
        LauncherAction::PlayInstalled => {
            window.begin_launch();
            return;
        }
        LauncherAction::Proceed => {}
        _ => return,
    }
    match window.confirm_update_choice(&locale, "r42 · UI PREVIEW", 80 * 1024 * 1024, true) {
        LauncherAction::PlayInstalled => {
            window.begin_launch();
            return;
        }
        LauncherAction::Proceed => {}
        _ => return,
    }
    window.set_stage("downloading", 10);
    window.transfer(0, 80 * 1024 * 1024);
    for chunk in 1..=80_u64 {
        if window.cancelled() {
            return;
        }
        thread::sleep(Duration::from_millis(150));
        window.transfer(chunk * 1024 * 1024, 80 * 1024 * 1024);
        window.set_stage("downloading", 10 + (chunk * 65 / 80) as u32);
        if failure && chunk == 12 {
            if !window.retry(&locale, "UI PREVIEW · HTTP 503 Service Unavailable") {
                return;
            }
            break;
        }
    }
    window.set_stage("checking-installed:40/40", 85);
    thread::sleep(Duration::from_millis(600));
    loop {
        match window.ready(&locale, "r42 · UI PREVIEW", failure) {
            LauncherAction::Retry => continue,
            _ => break,
        }
    }
    window.close();
}
