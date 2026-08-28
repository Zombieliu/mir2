//! Optional non-blocking native movement trace.
//!
//! The trace is diagnostic-only: gameplay code offers JSON events to a
//! bounded channel and a background writer appends them to a local JSONL file.
//! A slow or unavailable disk can therefore drop diagnostics, but can never
//! delay input, networking, simulation, or rendering.

use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const MOVEMENT_TRACE_PATH_ENV: &str = "MIR2_NATIVE_MOVEMENT_TRACE_PATH";
const MOVEMENT_TRACE_SCHEMA: &str = "mir2.windows.native-movement-trace.v1";
const MOVEMENT_TRACE_QUEUE_CAPACITY: usize = 4_096;

struct MovementTraceSink {
    sender: SyncSender<String>,
    dropped: AtomicU64,
}

static MOVEMENT_TRACE: OnceLock<Option<MovementTraceSink>> = OnceLock::new();

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn configured_path() -> Option<PathBuf> {
    std::env::var_os(MOVEMENT_TRACE_PATH_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn write_json_line(writer: &mut BufWriter<File>, line: &str) -> std::io::Result<()> {
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn writer_loop(path: &Path, receiver: mpsc::Receiver<String>) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    let session = serde_json::json!({
        "schema": MOVEMENT_TRACE_SCHEMA,
        "type": "sessionStarted",
        "capturedAtUnixMs": unix_time_ms(),
        "processId": std::process::id(),
    });
    write_json_line(&mut writer, &session.to_string())?;
    while let Ok(line) = receiver.recv() {
        write_json_line(&mut writer, &line)?;
    }
    Ok(())
}

fn initialize_sink() -> Option<MovementTraceSink> {
    let path = configured_path()?;
    let (sender, receiver) = mpsc::sync_channel(MOVEMENT_TRACE_QUEUE_CAPACITY);
    let thread_path = path.clone();
    let spawn = std::thread::Builder::new()
        .name("mir2-movement-trace".to_owned())
        .spawn(move || {
            if let Err(error) = writer_loop(&thread_path, receiver) {
                eprintln!(
                    "[platform-windows] movement trace disabled after writer error path={} error={error}",
                    thread_path.display()
                );
            }
        });
    if let Err(error) = spawn {
        eprintln!(
            "[platform-windows] movement trace unavailable path={} error={error}",
            path.display()
        );
        return None;
    }
    eprintln!(
        "[platform-windows] movement trace enabled path={} schema={MOVEMENT_TRACE_SCHEMA}",
        path.display()
    );
    Some(MovementTraceSink {
        sender,
        dropped: AtomicU64::new(0),
    })
}

/// Initialize the optional writer before the window starts. When the
/// environment variable is absent this resolves once to a zero-cost disabled
/// state.
pub(crate) fn initialize() {
    let _ = MOVEMENT_TRACE.get_or_init(initialize_sink);
}

/// Offer one event to the background trace without ever blocking the caller.
pub(crate) fn record(mut value: Value) {
    let Some(sink) = MOVEMENT_TRACE.get_or_init(initialize_sink).as_ref() else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object
            .entry("schema")
            .or_insert_with(|| Value::String(MOVEMENT_TRACE_SCHEMA.to_owned()));
        object
            .entry("capturedAtUnixMs")
            .or_insert_with(|| Value::from(unix_time_ms()));
        object
            .entry("processId")
            .or_insert_with(|| Value::from(std::process::id()));
        let dropped = sink.dropped.swap(0, Ordering::Relaxed);
        if dropped > 0 {
            object.insert("droppedBefore".to_owned(), Value::from(dropped));
        }
    }
    let line = value.to_string();
    match sink.sender.try_send(line) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            sink.dropped.fetch_add(1, Ordering::Relaxed);
        }
        Err(TrySendError::Disconnected(_)) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_timestamp_is_finite_and_nonzero() {
        assert!(unix_time_ms() > 1_600_000_000_000);
    }

    #[test]
    fn writer_emits_session_header_and_json_event() {
        let path = std::env::temp_dir().join(format!(
            "mir2-movement-trace-test-{}-{}.jsonl",
            std::process::id(),
            unix_time_ms()
        ));
        let (sender, receiver) = mpsc::sync_channel(2);
        sender
            .send(r#"{"type":"commandSent","atMs":100}"#.to_owned())
            .expect("queue synthetic movement event");
        drop(sender);
        writer_loop(&path, receiver).expect("write synthetic movement trace");
        let lines = fs::read_to_string(&path).expect("read synthetic movement trace");
        fs::remove_file(&path).expect("remove synthetic movement trace");
        let decoded = lines
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid JSONL event"))
            .collect::<Vec<_>>();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0]["schema"], MOVEMENT_TRACE_SCHEMA);
        assert_eq!(decoded[0]["type"], "sessionStarted");
        assert_eq!(decoded[1]["type"], "commandSent");
    }
}
