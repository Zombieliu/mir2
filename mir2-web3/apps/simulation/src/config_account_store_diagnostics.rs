//! Optional, completed-span diagnostics. Inner spans only collect scalars;
//! callers declare the record before their business guards so emission is last.
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    ExperienceAdmission,
    GuildLookup,
    Save,
    Transaction,
    GuildClock,
}

impl Operation {
    fn label(self) -> &'static str {
        match self {
            Self::ExperienceAdmission => "experience_admission",
            Self::GuildLookup => "guild_lookup",
            Self::Save => "save",
            Self::Transaction => "transaction",
            Self::GuildClock => "guild_clock",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    WritableWait,
    PersistWait,
    StoreWait,
    StoreHeld,
    FileWriterWait,
    Serialize,
    TempWriteSync,
    AtomicReplace,
    MarkerWait,
    MarkerStateWait,
    MarkerBeginSync,
    MarkerSettleSync,
    DriverWait,
    DriverHeld,
}

impl Stage {
    const ALL: [Self; 14] = [
        Self::WritableWait,
        Self::PersistWait,
        Self::StoreWait,
        Self::StoreHeld,
        Self::FileWriterWait,
        Self::Serialize,
        Self::TempWriteSync,
        Self::AtomicReplace,
        Self::MarkerWait,
        Self::MarkerStateWait,
        Self::MarkerBeginSync,
        Self::MarkerSettleSync,
        Self::DriverWait,
        Self::DriverHeld,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::WritableWait => "account_store.writable_wait",
            Self::PersistWait => "account_store.persist_wait",
            Self::StoreWait => "account_store.store_wait",
            Self::StoreHeld => "account_store.store_held",
            Self::FileWriterWait => "account_store.file_writer_wait",
            Self::Serialize => "account_store.serialize",
            Self::TempWriteSync => "account_store.temp_write_sync",
            Self::AtomicReplace => "account_store.atomic_replace",
            Self::MarkerWait => "account_store.marker_wait",
            Self::MarkerStateWait => "account_store.marker_state_wait",
            Self::MarkerBeginSync => "account_store.marker_begin_sync",
            Self::MarkerSettleSync => "account_store.marker_settle_sync",
            Self::DriverWait => "account_store.driver_wait",
            Self::DriverHeld => "account_store.driver_held",
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Sample {
    count: u64,
    total: Duration,
    maximum: Duration,
    maximum_start: Duration,
}

struct ActiveRecord {
    threshold: Duration,
    trace: u64,
    started_at: Instant,
    started_unix_ms: u64,
    samples: [Cell<Sample>; Stage::ALL.len()],
}

pub(crate) struct Record {
    operation: Operation,
    active: Option<ActiveRecord>,
}

impl Record {
    pub(crate) fn new(operation: Operation) -> Self {
        static THRESHOLD: OnceLock<Option<Duration>> = OnceLock::new();
        let threshold = *THRESHOLD.get_or_init(|| {
            parse_threshold(std::env::var("MIR2_GATEWAY_SLOW_STAGE_MS").ok().as_deref())
        });
        Self::with_threshold(operation, threshold)
    }

    fn with_threshold(operation: Operation, threshold: Option<Duration>) -> Self {
        static NEXT_TRACE: AtomicU64 = AtomicU64::new(1);
        Self {
            operation,
            active: threshold.map(|threshold| ActiveRecord {
                threshold,
                trace: NEXT_TRACE.fetch_add(1, Ordering::Relaxed),
                started_at: Instant::now(),
                started_unix_ms: millis(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default(),
                ),
                samples: std::array::from_fn(|_| Cell::new(Sample::default())),
            }),
        }
    }

    pub(crate) fn span(&self, stage: Stage) -> Span<'_> {
        Span {
            record: self,
            stage,
            started_at: self.active.as_ref().map(|_| Instant::now()),
        }
    }

    fn capture(&self, stage: Stage, start: Duration, elapsed: Duration) {
        let Some(active) = &self.active else { return };
        let slot = &active.samples[stage as usize];
        let mut sample = slot.get();
        sample.count = sample.count.saturating_add(1);
        sample.total = sample.total.saturating_add(elapsed);
        if sample.count == 1 || elapsed > sample.maximum {
            sample.maximum = elapsed;
            sample.maximum_start = start;
        }
        slot.set(sample);
    }

    fn emit(&self, mut sink: impl FnMut(String)) {
        let Some(active) = &self.active else { return };
        for stage in Stage::ALL {
            let sample = active.samples[stage as usize].get();
            if sample.count == 0 || sample.maximum < active.threshold {
                continue;
            }
            sink(format!(
                "[gateway-slow-stage] stage={} duration_ms={} operation={} trace_id={} started_unix_ms={} max_start_us={} max_end_us={} samples={} total_us={}",
                stage.label(), millis(sample.maximum), self.operation.label(), active.trace,
                active.started_unix_ms, micros(sample.maximum_start),
                micros(sample.maximum_start.saturating_add(sample.maximum)), sample.count,
                micros(sample.total),
            ));
        }
    }
}

impl Drop for Record {
    fn drop(&mut self) {
        self.emit(|line| {
            #[cfg(test)]
            if TEST_SINK.with(|sink| {
                let mut sink = sink.borrow_mut();
                if let Some(sink) = sink.as_mut() {
                    sink(&line);
                    true
                } else {
                    false
                }
            }) {
                return;
            }
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "{line}");
        });
    }
}

pub(crate) struct Span<'a> {
    record: &'a Record,
    stage: Stage,
    started_at: Option<Instant>,
}

pub(crate) fn measure<T>(record: Option<&Record>, stage: Stage, action: impl FnOnce() -> T) -> T {
    let _span = record.map(|record| record.span(stage));
    action()
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        if let (Some(active), Some(started_at)) = (&self.record.active, self.started_at) {
            self.record.capture(
                self.stage,
                started_at.saturating_duration_since(active.started_at),
                started_at.elapsed(),
            );
        }
    }
}

fn parse_threshold(value: Option<&str>) -> Option<Duration> {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
}

fn millis(value: Duration) -> u64 {
    value.as_millis().min(u128::from(u64::MAX)) as u64
}

fn micros(value: Duration) -> u64 {
    value.as_micros().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
thread_local! {
    static TEST_SINK: std::cell::RefCell<Option<Box<dyn FnMut(&str)>>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn diagnostics_disabled_and_positive_threshold_match_gateway_contract() {
        for value in [None, Some(""), Some("0"), Some("bad"), Some("-1")] {
            assert_eq!(parse_threshold(value), None);
        }
        assert_eq!(
            parse_threshold(Some(" 50 ")),
            Some(Duration::from_millis(50))
        );
        let record = Record::with_threshold(Operation::Save, None);
        assert!(record.span(Stage::StoreWait).started_at.is_none());
        record.capture(Stage::StoreWait, Duration::ZERO, Duration::MAX);
        record.emit(|_| panic!("disabled diagnostics must not format or emit"));
    }

    #[test]
    fn diagnostics_fixed_slots_emit_inclusive_maximum_without_private_values() {
        let record =
            Record::with_threshold(Operation::Transaction, Some(Duration::from_millis(50)));
        record.capture(
            Stage::StoreWait,
            Duration::from_millis(2),
            Duration::from_millis(49),
        );
        record.capture(
            Stage::StoreWait,
            Duration::from_millis(60),
            Duration::from_millis(50),
        );
        record.capture(Stage::Serialize, Duration::ZERO, Duration::from_millis(49));
        let mut lines = Vec::new();
        record.emit(|line| lines.push(line));
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert!(line.starts_with("[gateway-slow-stage] stage=account_store.store_wait duration_ms=50 operation=transaction "));
        assert!(line.ends_with("max_start_us=60000 max_end_us=110000 samples=2 total_us=99000"));
        for field in line.split_whitespace().skip(1) {
            let (key, value) = field.split_once('=').unwrap();
            assert!(matches!(
                key,
                "stage"
                    | "duration_ms"
                    | "operation"
                    | "trace_id"
                    | "started_unix_ms"
                    | "max_start_us"
                    | "max_end_us"
                    | "samples"
                    | "total_us"
            ));
            if key != "stage" && key != "operation" {
                assert!(value.parse::<u64>().is_ok());
            }
        }
    }

    #[test]
    fn diagnostics_emit_after_business_guard_on_success_and_error() {
        struct ResetSink;
        impl Drop for ResetSink {
            fn drop(&mut self) {
                TEST_SINK.with(|sink| *sink.borrow_mut() = None);
            }
        }
        let _reset = ResetSink;
        let lock = Arc::new(Mutex::new(()));
        let observed = Arc::new(Mutex::new(0));
        let sink_lock = lock.clone();
        let sink_observed = observed.clone();
        TEST_SINK.with(|sink| {
            *sink.borrow_mut() = Some(Box::new(move |_| {
                assert!(
                    sink_lock.try_lock().is_ok(),
                    "emit must follow original business guard release"
                );
                *sink_observed.lock().unwrap() += 1;
            }))
        });
        let operation = |fail: bool| -> Result<(), ()> {
            let diagnostics =
                Record::with_threshold(Operation::ExperienceAdmission, Some(Duration::ZERO));
            let _guard = lock.lock().unwrap();
            let _held = diagnostics.span(Stage::StoreHeld);
            if fail {
                return Err(());
            }
            Ok(())
        };
        assert_eq!(operation(false), Ok(()));
        assert_eq!(operation(true), Err(()));
        assert_eq!(*observed.lock().unwrap(), 2);
    }
}
