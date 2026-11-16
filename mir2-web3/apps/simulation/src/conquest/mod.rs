//! Server-owned classic Sabuk conquest. Personal sessions only project this state.
mod management;
mod runtime;
mod types;
pub use management::*;
pub use types::*;

/// The selected source Server.MirDB record is Request + Classic, not CapturePalace.
pub fn sabuk_policy() -> ConquestPolicy {
    ConquestPolicy {
        index: 1,
        name: "Sabuk Wall".into(),
        map_file_name: "3".into(),
        palace_file_name: "0150".into(),
        start_minute: 18 * 60,
        duration_minutes: 30,
        enabled_days: [true; 7],
        utc_offset_minutes: 0,
        capture_interval_ms: 10_000,
    }
}
