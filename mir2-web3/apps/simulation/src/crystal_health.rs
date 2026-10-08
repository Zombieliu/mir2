//! Crystal display values, independent of exact vitals and life authority.

/// `MapObject.PercentHealth` divides and multiplies in single precision, then
/// truncates to a byte. Authoritative pools satisfy `0 <= hp <= max_hp` with a
/// positive maximum. A living actor may display zero; 53/100 displays 52.
pub fn crystal_health_percent(hp: i32, max_hp: i32) -> u8 {
    (hp as f32 / max_hp as f32 * 100.0) as u8
}

/// `ObjectHealth.Expire` is a display duration, not a death/respawn timer.
/// Source casts after minimum-five and before its redundant upper minimum;
/// e.g. a remaining 256 seconds becomes byte0, not a saturated 255.
pub fn crystal_health_expire(revelation_until_ms: u64, now_ms: u64) -> u8 {
    (revelation_until_ms.saturating_sub(now_ms) / 1_000).max(5) as u8
}
