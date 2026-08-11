//! Deterministic client-side time abstraction.

/// A client-local millisecond timeline.
///
/// Consumers may compare values only when the platform adapter placed them in
/// the same time domain. The core never reads wall-clock or platform APIs.
pub trait Clock {
    fn now_ms(&self) -> u64;

    fn now_secs_f64(&self) -> f64 {
        self.now_ms() as f64 / 1_000.0
    }
}

/// Mutable clock for deterministic tests, replay and offline fixtures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ManualClock {
    now_ms: u64,
}

impl ManualClock {
    pub const fn new(now_ms: u64) -> Self {
        Self { now_ms }
    }

    pub fn set_ms(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    pub fn advance_ms(&mut self, delta_ms: u64) {
        self.now_ms = self.now_ms.saturating_add(delta_ms);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_freezes_and_advances_deterministically() {
        let mut clock = ManualClock::new(1_250);
        assert_eq!(clock.now_ms(), 1_250);
        assert_eq!(clock.now_secs_f64(), 1.25);

        clock.advance_ms(250);
        assert_eq!(clock.now_ms(), 1_500);

        clock.set_ms(42);
        assert_eq!(clock.now_ms(), 42);
    }

    #[test]
    fn advance_saturates_instead_of_wrapping() {
        let mut clock = ManualClock::new(u64::MAX - 1);
        clock.advance_ms(10);
        assert_eq!(clock.now_ms(), u64::MAX);
    }
}
