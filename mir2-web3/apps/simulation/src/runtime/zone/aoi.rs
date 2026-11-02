use mir2_protocol::Point;

use super::super::CRYSTAL_OBJECT_DATA_RANGE;

pub(crate) const AOI_X_RANGE: i32 = CRYSTAL_OBJECT_DATA_RANGE;
pub(crate) const AOI_Y_RANGE: i32 = CRYSTAL_OBJECT_DATA_RANGE;

pub(crate) fn points_visible(a: &Point, b: &Point) -> bool {
    (a.x - b.x).abs() <= AOI_X_RANGE && (a.y - b.y).abs() <= AOI_Y_RANGE
}
