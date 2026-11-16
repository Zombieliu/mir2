use std::time::{Duration, Instant};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{WorldCommand, CRYSTAL_FISHING_STEP};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FishingClockInput {
    Cast,
    WorldTick,
    FishingTick,
    Other,
}

impl From<&WorldCommand> for FishingClockInput {
    fn from(command: &WorldCommand) -> Self {
        match command {
            WorldCommand::ClientPacket(ClientPacket::FishingCast { cast_out: true }) => Self::Cast,
            WorldCommand::Tick => Self::WorldTick,
            WorldCommand::TickFishing => Self::FishingTick,
            _ => Self::Other,
        }
    }
}

/// Wakes the current owner independently of the ordinary simulation tick.
/// The authoritative cast deadline is checked again inside the owner.
#[derive(Default)]
pub(crate) struct FishingClock {
    wake: Option<(u32, Instant)>,
}

impl FishingClock {
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.wake.map(|(_, at)| at)
    }

    pub(crate) fn defer(&mut self, now: Instant) {
        if let Some((_, at)) = &mut self.wake {
            *at = now + CRYSTAL_FISHING_STEP;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.wake = None;
    }

    pub(crate) fn observe(
        &mut self,
        input: FishingClockInput,
        packets: &[ServerPacket],
        now: Instant,
        first_delay: Duration,
    ) {
        let accepted_cast = (input == FishingClockInput::Cast)
            .then(|| {
                packets.iter().find_map(|packet| match packet {
                    ServerPacket::FishingUpdate {
                        object_id,
                        fishing: true,
                        progress_percent: 0,
                        ..
                    } => Some(*object_id),
                    _ => None,
                })
            })
            .flatten();
        let Some(actor) = self.wake.map(|(actor, _)| actor).or(accepted_cast) else {
            return;
        };
        let mut active = self.wake.is_some();
        let mut new_cast = false;
        let mut step = false;
        let mut saw_owner = false;
        for packet in packets {
            match packet {
                ServerPacket::FishingUpdate {
                    object_id, fishing, ..
                } if *object_id == actor => {
                    saw_owner = true;
                    if *fishing {
                        new_cast |= !active;
                        step |= active;
                    } else {
                        new_cast = false;
                        step = false;
                    }
                    active = *fishing;
                }
                ServerPacket::MapInformation { .. } | ServerPacket::LogOutSuccess { .. } => {
                    self.clear();
                    return;
                }
                _ => {}
            }
        }
        if !active || (input == FishingClockInput::FishingTick && !saw_owner) {
            self.clear();
        } else if matches!(
            input,
            FishingClockInput::FishingTick | FishingClockInput::WorldTick
        ) && step
        {
            // A native auto-reel followed by a recast publishes a second
            // active update in the same batch. Exhaustion sends only one.
            self.wake = Some((actor, now + CRYSTAL_FISHING_STEP));
        } else if new_cast {
            self.wake = Some((actor, now + first_delay));
        }
    }
}

pub(crate) async fn wait_for_fishing_tick(deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        tokio::time::sleep_until(deadline.into()).await;
    } else {
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::Point;
    use mir2_simulation::CRYSTAL_FISHING_CAST_DELAY;

    fn update(actor: u32, fishing: bool, progress_percent: i32) -> ServerPacket {
        ServerPacket::FishingUpdate {
            object_id: actor,
            fishing,
            progress_percent,
            chance_percent: 38,
            fishing_point: Point { x: 339, y: 247 },
            found_fish: false,
        }
    }

    #[test]
    fn fishing_clock_arms_from_an_accepted_cast_and_stops_on_its_owner() {
        let now = Instant::now();
        let delay = CRYSTAL_FISHING_CAST_DELAY + Duration::from_millis(350);
        let mut clock = FishingClock::default();
        clock.observe(FishingClockInput::Cast, &[], now, delay);
        assert_eq!(clock.next_wake(), None);
        clock.observe(FishingClockInput::Cast, &[update(9, true, 0)], now, delay);
        let due = now + delay;
        assert_eq!(clock.next_wake(), Some(due));
        clock.observe(FishingClockInput::Other, &[update(9, true, 0)], now, delay);
        clock.observe(
            FishingClockInput::Other,
            &[update(8, false, 100)],
            now,
            delay,
        );
        assert_eq!(clock.next_wake(), Some(due));
        clock.observe(
            FishingClockInput::FishingTick,
            &[update(9, true, 3)],
            due,
            delay,
        );
        assert_eq!(clock.next_wake(), Some(due + CRYSTAL_FISHING_STEP));
        clock.observe(FishingClockInput::Other, &[update(9, false, 3)], due, delay);
        assert_eq!(clock.next_wake(), None);
    }

    #[test]
    fn fishing_clock_distinguishes_exhaustion_and_early_reel_continuations() {
        let now = Instant::now();
        let delay = CRYSTAL_FISHING_CAST_DELAY;
        let mut clock = FishingClock::default();
        clock.observe(FishingClockInput::Cast, &[update(9, true, 0)], now, delay);
        let exhausted = now + delay + 30 * CRYSTAL_FISHING_STEP;
        clock.observe(
            FishingClockInput::FishingTick,
            &[
                update(9, true, 103),
                update(9, false, 103),
                update(9, true, 0),
            ],
            exhausted,
            delay,
        );
        assert_eq!(clock.next_wake(), Some(exhausted + delay));
        let reeled = exhausted + delay + CRYSTAL_FISHING_STEP;
        clock.observe(
            FishingClockInput::WorldTick,
            &[update(9, false, 6), update(9, true, 0), update(9, true, 0)],
            reeled,
            delay,
        );
        assert_eq!(clock.next_wake(), Some(reeled + CRYSTAL_FISHING_STEP));
        clock.observe(
            FishingClockInput::FishingTick,
            &[update(8, true, 3)],
            reeled + CRYSTAL_FISHING_STEP,
            delay,
        );
        assert_eq!(
            clock.next_wake(),
            None,
            "another actor cannot leave a stale wake armed"
        );
    }
}
