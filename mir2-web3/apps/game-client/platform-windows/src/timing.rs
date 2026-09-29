//! Monotonic CPU/transport milestones; never a GPU-present measurement.
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};
static ORIGIN: OnceLock<Instant> = OnceLock::new();
static REQUESTS: Mutex<Requests> = Mutex::new(Requests {
    login: None,
    start: None,
});
#[derive(Default)]
struct Requests {
    login: Option<Instant>,
    start: Option<Instant>,
}
pub fn initialize() {
    let _ = ORIGIN.set(Instant::now());
}
pub fn report(stage: &str, started: Instant) {
    if mir2_bevy_runtime::native_render_diagnostics_enabled() {
        mir2_bevy_runtime::record_native_render_marker("cpuStage", serde_json::json!({
            "stage": stage,
            "durationMs": started.elapsed().as_secs_f64() * 1000.0,
            "measurement": "cpuElapsedNotGpuPresent",
        }));
    }
    if let Some(origin) = ORIGIN.get() {
        eprintln!(
            "[timing] stage={stage} duration_ms={:.3} since_launch_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0,
            origin.elapsed().as_secs_f64() * 1000.0
        );
    }
}
pub fn milestone(stage: &str) {
    if let Some(origin) = ORIGIN.get() {
        report(stage, *origin);
    }
}
pub struct Span {
    stage: &'static str,
    started: Instant,
}

/// Diagnostic-only cost attribution, including early failures. No logging or
/// clock read when render diagnostics are disabled.
pub struct DiagnosticSpan {
    stage: &'static str,
    started: Option<Instant>,
}
impl DiagnosticSpan {
    pub fn new(stage: &'static str) -> Self {
        Self {
            stage,
            started: mir2_bevy_runtime::native_render_diagnostics_enabled().then(Instant::now),
        }
    }
}
impl Drop for DiagnosticSpan {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            mir2_bevy_runtime::record_native_render_marker("cpuStage", serde_json::json!({
                "stage": self.stage,
                "durationMs": started.elapsed().as_secs_f64() * 1000.0,
                "measurement": "cpuElapsedNotGpuPresent",
            }));
        }
    }
}
impl Span {
    pub fn new(stage: &'static str) -> Self {
        Self {
            stage,
            started: Instant::now(),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        report(self.stage, self.started);
    }
}
pub fn reset_requests() {
    if let Ok(mut state) = REQUESTS.lock() {
        *state = Requests::default();
    }
}
pub fn sent(kind: Option<&str>, started: Instant) {
    if ORIGIN.get().is_none() {
        return;
    }
    if let Ok(mut state) = REQUESTS.lock() {
        match kind {
            Some("login") => state.login = Some(started),
            Some("start_game") => state.start = Some(started),
            _ => {}
        }
    }
}
pub fn reply(packet: &str) {
    if ORIGIN.get().is_none() {
        return;
    }
    if let Ok(mut state) = REQUESTS.lock() {
        let (stage, pending) = match packet {
            "LoginSuccess" | "Login" | "LoginBanned" => ("login_reply", &mut state.login),
            "StartGame" => ("start_game_reply", &mut state.start),
            _ => return,
        };
        if let Some(started) = pending.take() {
            report(&format!("{stage}:{packet}"), started);
        }
    }
}
