use mir2_protocol::{MirDirection, Point};

pub(crate) const ZONE_WALK_DELAY_MS: u64 = 600;
pub(crate) const ZONE_RUN_DELAY_MS: u64 = 300;
pub(crate) const ZONE_TURN_DELAY_MS: u64 = 350;
pub(crate) const ZONE_RUN_GRACE_MS: u64 = 1_200;
pub(crate) const ZONE_MOVE_READY_GRACE_MS: u64 = 120;

pub(crate) fn offset_point(source: &Point, direction: MirDirection, amount: i32) -> Point {
    let (dx, dy) = match direction {
        MirDirection::Up => (0, -1),
        MirDirection::UpRight => (1, -1),
        MirDirection::Right => (1, 0),
        MirDirection::DownRight => (1, 1),
        MirDirection::Down => (0, 1),
        MirDirection::DownLeft => (-1, 1),
        MirDirection::Left => (-1, 0),
        MirDirection::UpLeft => (-1, -1),
    };

    Point {
        x: source.x + dx * amount,
        y: source.y + dy * amount,
    }
}

pub(crate) fn movement_delay_ms(running: bool) -> u64 {
    if running {
        ZONE_RUN_DELAY_MS
    } else {
        ZONE_WALK_DELAY_MS
    }
}
