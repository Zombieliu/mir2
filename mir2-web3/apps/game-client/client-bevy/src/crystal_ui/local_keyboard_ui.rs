use std::time::{Duration, Instant};
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalKeyboardUi {
    pub camera_hidden: bool,
    pub auto_run: bool,
    pub auto_run_notice: Option<&'static str>,
    drop_until: Option<Instant>,
}
impl LocalKeyboardUi {
    pub fn show_drops(&mut self, now: Instant) {
        if self.drop_until.is_none_or(|until| now > until) {
            self.drop_until = Some(now + Duration::from_secs(5));
        }
    }
    pub fn drops_visible(&self, now: Instant) -> bool {
        self.drop_until.is_some_and(|until| now < until)
    }
    pub fn set_auto_run(&mut self, on: bool) {
        if self.auto_run != on {
            self.auto_run = on;
            self.auto_run_notice = Some(if on {
                "[AutoRun: On]"
            } else {
                "[AutoRun: Off]"
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_drop_view_has_five_seconds_and_repeated_key_does_not_extend_it() {
        let now = Instant::now();
        let mut ui = LocalKeyboardUi::default();
        ui.show_drops(now);
        assert!(ui.drops_visible(now + Duration::from_millis(4999)));
        ui.show_drops(now + Duration::from_secs(4));
        assert!(!ui.drops_visible(now + Duration::from_secs(5)));
        ui.show_drops(now + Duration::from_secs(5));
        assert!(!ui.drops_visible(now + Duration::from_secs(5)));
        ui.show_drops(now + Duration::from_millis(5001));
        assert!(ui.drops_visible(now + Duration::from_secs(6)));
    }
    #[test]
    fn autorun_hint_only_on_real_state_change() {
        let mut ui = LocalKeyboardUi::default();
        ui.set_auto_run(true);
        assert_eq!(ui.auto_run_notice.take(), Some("[AutoRun: On]"));
        ui.set_auto_run(true);
        assert!(ui.auto_run_notice.is_none());
        ui.set_auto_run(false);
        assert_eq!(ui.auto_run_notice, Some("[AutoRun: Off]"));
    }
}
