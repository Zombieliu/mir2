//! Bounded, cumulative process telemetry. It observes existing deadlines and
//! transaction boundaries; it never changes cadence, admission or game state.
//! A profile must subtract its starting bucket/counter snapshot. Quantiles of
//! this process-lifetime total are not quantiles of an arbitrary later window.
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

pub const WORK_BUDGET_US: u64 = 50_000;
const BUCKET_UPPER_US: [u64; 18] = [
    100, 250, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 35_000, 50_000, 75_000, 100_000, 150_000,
    250_000, 500_000, 1_000_000, 5_000_000, 30_000_000,
];
const BUCKETS: usize = BUCKET_UPPER_US.len() + 1;
const SCOPE: &str = "gateway shared-owner transaction, may contain multiple maps";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestClass {
    Immediate,
    Lifecycle,
    /// Handler/admission completion, including an empty result while the
    /// original movement deadline is pending; never an owner ACK timer.
    MovementAdmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineKind {
    OwnerTick,
    PendingMovement,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationBucketSnapshot {
    /// None is the unbounded overflow bucket, never a finite latency bound.
    pub upper_bound_us: Option<u64>,
    /// Disjoint bucket count. Sum the buckets before calculating a quantile.
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationHistogramSnapshot {
    pub count: u64,
    pub sum_us: u64,
    pub max_us: u64,
    pub over_budget_count: u64,
    pub buckets: Vec<DurationBucketSnapshot>,
    /// Nearest-rank quantile's bucket upper bound, not an interpolated value.
    /// None means empty, overflow bucket, or incomplete saturated counters.
    pub p50_upper_us: Option<u64>,
    pub p95_upper_us: Option<u64>,
    pub p99_upper_us: Option<u64>,
    pub p999_upper_us: Option<u64>,
    pub saturated: bool,
    pub discarded_samples: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMetricsSnapshot {
    /// Includes successful and failed completions; no request IDs or payloads.
    pub duration: DurationHistogramSnapshot,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneMetricsSnapshot {
    pub version: u32,
    pub scope: &'static str,
    pub since_initialization_ms: u64,
    pub budget_us: u64,
    pub immediate_requests: RequestMetricsSnapshot,
    pub lifecycle_requests: RequestMetricsSnapshot,
    pub movement_admissions: RequestMetricsSnapshot,
    pub zone_work: DurationHistogramSnapshot,
    pub owner_tick_deadline_lag: DurationHistogramSnapshot,
    pub pending_movement_deadline_lag: DurationHistogramSnapshot,
    /// False after counter overflow, unrepresentable durations or mutex poison.
    /// Such a snapshot cannot establish an acceptance percentile or fraction.
    pub complete: bool,
}

#[derive(Debug, Clone, Default)]
struct Histogram {
    count: u64,
    sum_us: u64,
    max_us: u64,
    over_budget_count: u64,
    buckets: [u64; BUCKETS],
    saturated: bool,
    discarded_samples: u64,
}

impl Histogram {
    fn observe(&mut self, duration: Duration) -> bool {
        let Some(count) = self.count.checked_add(1) else {
            self.saturated = true;
            self.discarded_samples = self.discarded_samples.saturating_add(1);
            return false;
        };
        // Ceil, rather than floor: 50ms+1ns must not masquerade as <=50ms.
        let micros = duration.as_nanos().div_ceil(1_000);
        let micros = u64::try_from(micros).unwrap_or_else(|_| {
            self.saturated = true;
            u64::MAX
        });
        let bucket = BUCKET_UPPER_US.partition_point(|upper| *upper < micros);
        self.count = count;
        self.sum_us = self.sum_us.checked_add(micros).unwrap_or_else(|| {
            self.saturated = true;
            u64::MAX
        });
        self.max_us = self.max_us.max(micros);
        // A bucket count and over-budget numerator are bounded by count, which
        // was checked before every observation. They cannot wrap independently.
        self.buckets[bucket] += 1;
        if duration > Duration::from_micros(WORK_BUDGET_US) {
            self.over_budget_count += 1;
        }
        true
    }

    fn quantile_upper(&self, numerator: u64, denominator: u64) -> Option<u64> {
        if self.count == 0 || self.saturated {
            return None;
        }
        let rank =
            (u128::from(self.count) * u128::from(numerator)).div_ceil(u128::from(denominator));
        let mut cumulative = 0_u128;
        for (index, count) in self.buckets.iter().enumerate() {
            cumulative += u128::from(*count);
            if cumulative >= rank {
                return BUCKET_UPPER_US.get(index).copied();
            }
        }
        None
    }

    fn snapshot(&self) -> DurationHistogramSnapshot {
        DurationHistogramSnapshot {
            count: self.count,
            sum_us: self.sum_us,
            max_us: self.max_us,
            over_budget_count: self.over_budget_count,
            buckets: self
                .buckets
                .iter()
                .enumerate()
                .map(|(index, count)| DurationBucketSnapshot {
                    upper_bound_us: BUCKET_UPPER_US.get(index).copied(),
                    count: *count,
                })
                .collect(),
            p50_upper_us: self.quantile_upper(1, 2),
            p95_upper_us: self.quantile_upper(95, 100),
            p99_upper_us: self.quantile_upper(99, 100),
            p999_upper_us: self.quantile_upper(999, 1_000),
            saturated: self.saturated,
            discarded_samples: self.discarded_samples,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct RequestMetrics {
    duration: Histogram,
    success_count: u64,
    failure_count: u64,
}

impl RequestMetrics {
    fn observe(&mut self, duration: Duration, success: bool) {
        if self.duration.observe(duration) {
            if success {
                self.success_count += 1;
            } else {
                self.failure_count += 1;
            }
        }
    }

    fn snapshot(&self) -> RequestMetricsSnapshot {
        RequestMetricsSnapshot {
            duration: self.duration.snapshot(),
            success_count: self.success_count,
            failure_count: self.failure_count,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct MetricsState {
    immediate: RequestMetrics,
    lifecycle: RequestMetrics,
    movement: RequestMetrics,
    zone_work: Histogram,
    owner_tick_lag: Histogram,
    pending_movement_lag: Histogram,
    poisoned: bool,
}

struct ProcessZoneMetrics {
    started: Instant,
    state: Mutex<MetricsState>,
}

impl ProcessZoneMetrics {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            state: Mutex::new(MetricsState::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, MetricsState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(error) => {
                let mut guard = error.into_inner();
                guard.poisoned = true;
                guard
            }
        }
    }

    fn record_request(&self, class: RequestClass, duration: Duration, success: bool) {
        let mut state = self.lock();
        match class {
            RequestClass::Immediate => state.immediate.observe(duration, success),
            RequestClass::Lifecycle => state.lifecycle.observe(duration, success),
            RequestClass::MovementAdmission => state.movement.observe(duration, success),
        }
    }

    fn record_zone_work(&self, duration: Duration) {
        self.lock().zone_work.observe(duration);
    }

    fn record_deadline_lag(&self, kind: DeadlineKind, duration: Duration) {
        let mut state = self.lock();
        match kind {
            DeadlineKind::OwnerTick => {
                state.owner_tick_lag.observe(duration);
            }
            DeadlineKind::PendingMovement => {
                state.pending_movement_lag.observe(duration);
            }
        }
    }

    fn snapshot(&self) -> ZoneMetricsSnapshot {
        // Copy fixed storage while locked; bounded JSON projection allocation
        // happens after release, rather than blocking command observations.
        let state = self.lock().clone();
        let complete = !state.poisoned
            && [
                &state.immediate.duration,
                &state.lifecycle.duration,
                &state.movement.duration,
                &state.zone_work,
                &state.owner_tick_lag,
                &state.pending_movement_lag,
            ]
            .iter()
            .all(|histogram| !histogram.saturated);
        ZoneMetricsSnapshot {
            version: 2,
            scope: SCOPE,
            since_initialization_ms: u64::try_from(self.started.elapsed().as_millis())
                .unwrap_or(u64::MAX),
            budget_us: WORK_BUDGET_US,
            immediate_requests: state.immediate.snapshot(),
            lifecycle_requests: state.lifecycle.snapshot(),
            movement_admissions: state.movement.snapshot(),
            zone_work: state.zone_work.snapshot(),
            owner_tick_deadline_lag: state.owner_tick_lag.snapshot(),
            pending_movement_deadline_lag: state.pending_movement_lag.snapshot(),
            complete,
        }
    }
}

fn metrics() -> &'static ProcessZoneMetrics {
    static METRICS: OnceLock<ProcessZoneMetrics> = OnceLock::new();
    METRICS.get_or_init(ProcessZoneMetrics::new)
}

/// Observe authoritative handler completion before socket encoding or delivery.
/// This includes ingress/queue/execution time, but excludes originally scheduled
/// movement or effect completion. Movement admissions are a separate class.
/// Count each request once across fast and queued paths. Failures count too.
pub fn record_request(class: RequestClass, duration: Duration, success: bool) {
    metrics().record_request(class, duration, success);
}

/// Observe shared-owner work including dispatch. This is not a per-Cell timer.
pub fn record_zone_work(duration: Duration) {
    metrics().record_zone_work(duration);
}

/// The caller computes actual start minus its existing scheduled deadline.
/// Recording a value does not reset or coalesce any gameplay deadline.
pub fn record_deadline_lag(kind: DeadlineKind, duration: Duration) {
    metrics().record_deadline_lag(kind, duration);
}

/// Cumulative telemetry with fixed histogram storage and no reset endpoint.
pub fn snapshot() -> ZoneMetricsSnapshot {
    metrics().snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_histograms_have_no_invented_quantiles_and_fixed_acceptance_bounds() {
        let metrics = ProcessZoneMetrics::new();
        let snapshot = metrics.snapshot();
        assert!(snapshot.complete);
        assert_eq!(snapshot.budget_us, 50_000);
        assert_eq!(snapshot.scope, SCOPE);
        assert_eq!(snapshot.zone_work.count, 0);
        assert_eq!(snapshot.zone_work.p99_upper_us, None);
        assert_eq!(snapshot.zone_work.buckets.len(), BUCKETS);
        for bound in [35_000, 50_000, 150_000] {
            assert!(snapshot
                .zone_work
                .buckets
                .iter()
                .any(|bucket| bucket.upper_bound_us == Some(bound)));
        }
        assert_eq!(
            snapshot.zone_work.buckets.last().unwrap().upper_bound_us,
            None
        );
    }

    #[test]
    fn exact_budget_boundary_and_submicrosecond_overrun_cannot_round_down_to_pass() {
        let metrics = ProcessZoneMetrics::new();
        metrics.record_zone_work(Duration::from_millis(35));
        metrics.record_zone_work(Duration::from_millis(50));
        metrics.record_zone_work(Duration::from_millis(50) + Duration::from_nanos(1));
        let snapshot = metrics.snapshot().zone_work;
        assert_eq!(snapshot.count, 3);
        assert_eq!(snapshot.sum_us, 135_001);
        assert_eq!(snapshot.max_us, 50_001);
        assert_eq!(snapshot.over_budget_count, 1);
        assert_eq!(
            snapshot
                .buckets
                .iter()
                .find(|bucket| bucket.upper_bound_us == Some(50_000))
                .unwrap()
                .count,
            1
        );
        assert_eq!(snapshot.p99_upper_us, Some(75_000));
    }

    #[test]
    fn nearest_rank_quantiles_report_bucket_upper_bounds_at_p99_and_p999() {
        let mut histogram = Histogram::default();
        for _ in 0..990 {
            histogram.observe(Duration::from_micros(35_000));
        }
        for _ in 0..9 {
            histogram.observe(Duration::from_micros(50_000));
        }
        histogram.observe(Duration::from_micros(150_000));
        let snapshot = histogram.snapshot();
        assert_eq!(snapshot.count, 1_000);
        assert_eq!(snapshot.p99_upper_us, Some(35_000));
        assert_eq!(snapshot.p999_upper_us, Some(50_000));
        assert_eq!(snapshot.over_budget_count, 1);
        assert_eq!(
            snapshot
                .buckets
                .iter()
                .map(|bucket| bucket.count)
                .sum::<u64>(),
            snapshot.count
        );
    }

    #[test]
    fn overflow_quantiles_are_unknown_even_when_the_observed_maximum_is_finite() {
        let mut histogram = Histogram::default();
        histogram.observe(Duration::from_secs(31));
        let snapshot = histogram.snapshot();
        assert_eq!(snapshot.max_us, 31_000_000);
        assert_eq!(snapshot.buckets.last().unwrap().count, 1);
        assert_eq!(snapshot.p50_upper_us, None);
        assert_eq!(snapshot.p99_upper_us, None);
        assert!(!snapshot.saturated);
    }

    #[test]
    fn request_outcomes_and_both_deadline_kinds_remain_separate() {
        let metrics = ProcessZoneMetrics::new();
        metrics.record_request(RequestClass::Immediate, Duration::from_millis(1), true);
        metrics.record_request(RequestClass::Immediate, Duration::from_millis(2), false);
        metrics.record_request(RequestClass::Lifecycle, Duration::from_secs(1), false);
        metrics.record_request(
            RequestClass::MovementAdmission,
            Duration::from_millis(5),
            true,
        );
        metrics.record_deadline_lag(DeadlineKind::OwnerTick, Duration::from_millis(3));
        metrics.record_deadline_lag(DeadlineKind::PendingMovement, Duration::from_millis(4));
        let snapshot = metrics.snapshot();
        assert_eq!(
            (
                snapshot.immediate_requests.success_count,
                snapshot.immediate_requests.failure_count
            ),
            (1, 1)
        );
        assert_eq!(snapshot.immediate_requests.duration.count, 2);
        assert_eq!(snapshot.lifecycle_requests.failure_count, 1);
        assert_eq!(snapshot.lifecycle_requests.duration.count, 1);
        assert_eq!(snapshot.movement_admissions.success_count, 1);
        assert_eq!(snapshot.movement_admissions.duration.sum_us, 5_000);
        assert_eq!(snapshot.owner_tick_deadline_lag.sum_us, 3_000);
        assert_eq!(snapshot.pending_movement_deadline_lag.sum_us, 4_000);
        assert_eq!(snapshot.zone_work.count, 0);
    }

    #[test]
    fn count_sum_and_duration_overflow_are_explicit_and_never_wrap_to_healthy() {
        let mut count = Histogram {
            count: u64::MAX,
            ..Default::default()
        };
        assert!(!count.observe(Duration::ZERO));
        let snapshot = count.snapshot();
        assert_eq!(snapshot.count, u64::MAX);
        assert_eq!(snapshot.discarded_samples, 1);
        assert!(snapshot.saturated);
        assert_eq!(snapshot.p99_upper_us, None);
        let mut sum = Histogram {
            sum_us: u64::MAX,
            ..Default::default()
        };
        sum.observe(Duration::from_micros(1));
        assert!(sum.saturated);
        assert_eq!(sum.sum_us, u64::MAX);
        let metrics = ProcessZoneMetrics::new();
        metrics.record_zone_work(Duration::MAX);
        assert!(!metrics.snapshot().complete);
        assert!(metrics.snapshot().zone_work.saturated);
    }

    #[test]
    fn fixed_storage_and_concurrent_observations_retain_every_count_without_labels() {
        assert!(std::mem::size_of::<MetricsState>() < 4_096);
        let metrics = std::sync::Arc::new(ProcessZoneMetrics::new());
        let workers = (0..4)
            .map(|_| {
                let metrics = metrics.clone();
                std::thread::spawn(move || {
                    for _ in 0..1_000 {
                        metrics.record_request(
                            RequestClass::Immediate,
                            Duration::from_micros(1),
                            true,
                        );
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.immediate_requests.duration.count, 4_000);
        assert_eq!(snapshot.immediate_requests.duration.sum_us, 4_000);
        assert_eq!(snapshot.immediate_requests.success_count, 4_000);
        assert!(snapshot.complete);
        let json = serde_json::to_value(snapshot).unwrap();
        assert!(json.get("immediateRequests").is_some());
        assert!(json.get("accountId").is_none());
        assert!(json.get("zoneId").is_none());
    }

    #[test]
    fn poison_is_visible_in_snapshot_and_does_not_crash_command_instrumentation() {
        let metrics = std::sync::Arc::new(ProcessZoneMetrics::new());
        let worker = metrics.clone();
        assert!(std::thread::spawn(move || {
            let _guard = worker.state.lock().unwrap();
            panic!("fixture poisons only its local telemetry instance");
        })
        .join()
        .is_err());
        metrics.record_zone_work(Duration::from_micros(1));
        let snapshot = metrics.snapshot();
        assert!(!snapshot.complete);
        assert_eq!(snapshot.zone_work.count, 1);
    }
}
