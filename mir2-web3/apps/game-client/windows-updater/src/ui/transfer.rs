use std::time::Duration;

/// Tracks only one reported download, never installation or total launch time.
#[derive(Default)]
pub(super) struct TransferMeter {
    pub received: u64,
    pub total: u64,
    started: Duration,
    last_change: Duration,
}

impl TransferMeter {
    pub fn observe(&mut self, received: u64, total: u64, now: Duration) {
        if received < self.received || total != self.total {
            self.started = now;
            self.last_change = now;
        } else if received > self.received {
            self.last_change = now;
        }
        self.received = received;
        self.total = total;
    }

    pub fn summary(&self, locale: &str, now: Duration) -> String {
        let count = super::render(
            locale,
            "current-download",
            &[
                ("received", bytes(self.received)),
                ("total", bytes(self.total)),
            ],
        );
        let elapsed = now.saturating_sub(self.started).as_secs_f64();
        let detail = if self.total > 0 && self.received >= self.total {
            super::stage(locale, "verifying")
        } else if now.saturating_sub(self.last_change) >= Duration::from_secs(5) {
            super::stage(locale, "download-waiting")
        } else if elapsed >= 1.0 && self.received > 0 && self.received < self.total {
            let rate = self.received as f64 / elapsed;
            let seconds = ((self.total - self.received) as f64 / rate)
                .ceil()
                .min(u64::MAX as f64) as u64;
            super::render(
                locale,
                "download-speed",
                &[
                    ("speed", bytes(rate as u64)),
                    ("time", duration(Duration::from_secs(seconds))),
                ],
            )
        } else {
            super::stage(locale, "estimating")
        };
        format!("{count}\n{detail}")
    }
}

pub(super) fn bytes(value: u64) -> String {
    if value >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", value as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if value >= 1024 * 1024 {
        format!("{:.1} MiB", value as f64 / (1024.0 * 1024.0))
    } else if value >= 1024 {
        format!("{:.1} KiB", value as f64 / 1024.0)
    } else {
        format!("{value} B")
    }
}

pub(super) fn duration(value: Duration) -> String {
    let seconds = value.as_secs();
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_stall_and_resumed_counters_never_reuse_a_previous_download_estimate() {
        let mut meter = TransferMeter::default();
        meter.observe(0, 10000, Duration::ZERO);
        meter.observe(2000, 10000, Duration::from_secs(2));
        assert!(meter.summary("en", Duration::from_secs(2)).contains("0:08"));
        assert!(meter
            .summary("en", Duration::from_secs(7))
            .contains("Waiting"));
        meter.observe(0, 10000, Duration::from_secs(8));
        assert!(meter
            .summary("en", Duration::from_secs(8))
            .contains("Estimating"));
        meter.observe(1000, 10000, Duration::from_secs(9));
        assert!(meter.summary("en", Duration::from_secs(9)).contains("0:09"));
        meter.observe(0, 0, Duration::from_secs(10));
        assert!(meter
            .summary("en", Duration::from_secs(10))
            .contains("Estimating"));
    }

    #[test]
    fn completed_bytes_and_slow_zero_byte_samples_do_not_divide_by_zero() {
        let mut meter = TransferMeter::default();
        meter.observe(0, u64::MAX, Duration::ZERO);
        assert!(!meter.summary("en", Duration::ZERO).contains("NaN"));
        meter.observe(4, 4, Duration::from_secs(1));
        assert!(!meter.summary("en", Duration::from_secs(1)).contains("left"));
        assert!(meter
            .summary("en", Duration::from_secs(8))
            .contains("Verifying"));
        assert_eq!(duration(Duration::from_secs(3661)), "1:01:01");
    }

    #[test]
    fn repeated_zero_byte_polls_preserve_the_waiting_deadline() {
        let mut meter = TransferMeter::default();
        for second in 0..=6 {
            meter.observe(0, 10000, Duration::from_secs(second));
        }
        assert!(meter
            .summary("en", Duration::from_secs(6))
            .contains("Waiting"));
    }
}
