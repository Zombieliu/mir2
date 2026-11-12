//! One optional aggregate for a completed personal Tick. No identity or payload
//! enters this record, and disabled records do not read the profiling clock.
use std::io::{self, Write};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(crate) enum Leaf {
    Billing,
    Gift,
    CheckpointBegin,
    PersonalTimers,
    SourceDispatch,
    Finalize,
    Finish,
}

struct ActiveProfile {
    threshold: Duration,
    started_at: Instant,
    leaves: [Duration; 7],
    measured: u8,
    ok: bool,
    #[cfg(test)]
    sink: Option<Box<dyn Write>>,
}

pub(crate) struct Profile {
    active: Option<ActiveProfile>,
}

impl Profile {
    pub(crate) fn new() -> Self {
        static THRESHOLD: OnceLock<Option<Duration>> = OnceLock::new();
        let threshold = *THRESHOLD.get_or_init(|| {
            parse_threshold(std::env::var("MIR2_GATEWAY_SLOW_STAGE_MS").ok().as_deref())
        });
        Self::with_threshold(threshold)
    }

    pub(crate) fn disabled() -> Self {
        Self { active: None }
    }

    fn with_threshold(threshold: Option<Duration>) -> Self {
        Self {
            active: threshold.map(|threshold| ActiveProfile {
                threshold,
                started_at: Instant::now(),
                leaves: [Duration::ZERO; 7],
                measured: 0,
                ok: false,
                #[cfg(test)]
                sink: None,
            }),
        }
    }

    pub(crate) fn measure<T>(&mut self, leaf: Leaf, action: impl FnOnce() -> T) -> T {
        let Some(active) = &mut self.active else {
            return action();
        };
        let _timer = LeafTimer {
            elapsed: &mut active.leaves[leaf as usize],
            measured: &mut active.measured,
            bit: 1 << leaf as u8,
            started_at: Instant::now(),
        };
        action()
    }

    pub(crate) fn mark_ok(&mut self) {
        if let Some(active) = &mut self.active {
            active.ok = true;
        }
    }
}

struct LeafTimer<'a> {
    elapsed: &'a mut Duration,
    measured: &'a mut u8,
    bit: u8,
    started_at: Instant,
}

impl Drop for LeafTimer<'_> {
    fn drop(&mut self) {
        *self.elapsed = self.elapsed.saturating_add(self.started_at.elapsed());
        *self.measured |= self.bit;
    }
}

impl ActiveProfile {
    fn emit(&self, total: Duration, sink: &mut dyn Write) -> io::Result<()> {
        writeln!(
            sink,
            "[gateway-slow-stage] stage=personal_tick.leaves duration_ms={} total_us={} billing_us={} gift_us={} checkpoint_begin_us={} personal_timers_us={} source_dispatch_us={} finalize_us={} finish_us={} leaf_mask={} outcome={}",
            total.as_millis().min(u128::from(u64::MAX)),
            micros(total),
            micros(self.leaves[Leaf::Billing as usize]),
            micros(self.leaves[Leaf::Gift as usize]),
            micros(self.leaves[Leaf::CheckpointBegin as usize]),
            micros(self.leaves[Leaf::PersonalTimers as usize]),
            micros(self.leaves[Leaf::SourceDispatch as usize]),
            micros(self.leaves[Leaf::Finalize as usize]),
            micros(self.leaves[Leaf::Finish as usize]),
            self.measured,
            if self.ok { "ok" } else { "error" },
        )
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        let Some(active) = &mut self.active else {
            return;
        };
        let total = active.started_at.elapsed();
        if total < active.threshold {
            return;
        }
        #[cfg(test)]
        if let Some(mut sink) = active.sink.take() {
            let _ = active.emit(total, &mut sink);
            return;
        }
        // stderr errors are diagnostic-only and never replace the Tick result.
        let _ = active.emit(total, &mut io::stderr().lock());
    }
}

fn parse_threshold(value: Option<&str>) -> Option<Duration> {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
}

fn micros(value: Duration) -> u64 {
    value.as_micros().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
#[path = "personal_tick_diagnostics_tests.rs"]
mod tests;
