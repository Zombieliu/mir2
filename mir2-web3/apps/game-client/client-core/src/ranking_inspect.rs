//! Shared admission for Crystal ranking-row inspection.
//! A plan has no transport, gesture custody or persistent cooldown ownership.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingInspectFacts {
    pub opened: Option<bool>,
    /// No ranking page/filter change is waiting to be requested.
    pub rankings_ready: Option<bool>,
    /// An untagged ranking page request is still outstanding.
    pub pending: Option<bool>,
    pub player_id: Option<i64>,
    pub now_ms: u64,
    pub next_ready_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingInspectPlan {
    pub object_id: u32,
    pub next_ready_ms: u64,
}

/// Preserve Native's strict now > ready boundary and saturating 500 ms delay.
/// Unknown flags/identity reject admission; player ID zero remains valid.
/// The host keeps the expected name and literal ranking=true, hero=false flags,
/// rechecks current row/owner custody before applying the clock and dispatching.
pub fn plan_ranking_inspect(facts: &RankingInspectFacts) -> Option<RankingInspectPlan> {
    if facts.opened != Some(true) || facts.rankings_ready != Some(true)
        || facts.pending != Some(false) || facts.now_ms <= facts.next_ready_ms
    {
        return None;
    }
    Some(RankingInspectPlan {
        object_id: u32::try_from(facts.player_id?).ok()?,
        next_ready_ms: facts.now_ms.saturating_add(500),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> RankingInspectFacts {
        RankingInspectFacts { opened: Some(true), rankings_ready: Some(true),
            pending: Some(false), player_id: Some(0), now_ms: 1001, next_ready_ms: 1000 }
    }

    #[test]
    fn ranking_inspect_exact_cooldown_boundary_preserves_zero_identity_and_pure_clock() {
        let mut facts = ready();
        for now in [0, 999, 1000] {
            facts.now_ms = now;
            assert_eq!(plan_ranking_inspect(&facts), None);
        }
        facts.now_ms = 1001;
        let original = facts;
        let expected = Some(RankingInspectPlan { object_id: 0, next_ready_ms: 1501 });
        assert_eq!(plan_ranking_inspect(&facts), expected);
        assert_eq!(plan_ranking_inspect(&facts), expected);
        assert_eq!(facts, original);
        facts.next_ready_ms = 1501;
        facts.now_ms = 1501;
        assert_eq!(plan_ranking_inspect(&facts), None);
        facts.now_ms = 1502;
        assert_eq!(plan_ranking_inspect(&facts), Some(RankingInspectPlan {
            object_id: 0, next_ready_ms: 2002 }));
    }

    #[test]
    fn ranking_inspect_unknown_closed_loading_and_pending_pages_never_admit() {
        for opened in [None, Some(false)] {
            assert_eq!(plan_ranking_inspect(&RankingInspectFacts { opened, ..ready() }), None);
        }
        for rankings_ready in [None, Some(false)] {
            assert_eq!(plan_ranking_inspect(&RankingInspectFacts { rankings_ready, ..ready() }), None);
        }
        for pending in [None, Some(true)] {
            assert_eq!(plan_ranking_inspect(&RankingInspectFacts { pending, ..ready() }), None);
        }
        assert_eq!(plan_ranking_inspect(&RankingInspectFacts { player_id: None, ..ready() }), None);
    }

    #[test]
    fn ranking_inspect_raw_player_id_is_checked_without_truncation() {
        for player_id in [i64::MIN, -1, i64::from(u32::MAX) + 1, i64::MAX] {
            assert_eq!(plan_ranking_inspect(&RankingInspectFacts {
                player_id: Some(player_id), ..ready() }), None);
        }
        for (player_id, object_id) in [(0, 0), (50, 50), (i64::from(u32::MAX), u32::MAX)] {
            assert_eq!(plan_ranking_inspect(&RankingInspectFacts {
                player_id: Some(player_id), ..ready() }), Some(RankingInspectPlan {
                object_id, next_ready_ms: 1501 }));
        }
    }

    #[test]
    fn ranking_inspect_native_clock_overflow_saturates_and_cannot_wrap_into_readiness() {
        for now_ms in [u64::MAX - 500, u64::MAX - 499, u64::MAX] {
            assert_eq!(plan_ranking_inspect(&RankingInspectFacts { now_ms,
                next_ready_ms: now_ms - 1, ..ready() }), Some(RankingInspectPlan {
                object_id: 0, next_ready_ms: u64::MAX }));
        }
        assert_eq!(plan_ranking_inspect(&RankingInspectFacts { now_ms: u64::MAX,
            next_ready_ms: u64::MAX, ..ready() }), None);
    }
}
