//! Requests are queued before the ordinary flood. No transport, window or synthetic ACK.
use super::*;
use crate::social::{
    guild_rank_native_tests as wire, SocialModel, SocialPendingOperation, MAX_SOCIAL_PENDING,
};
use serde_json::json;

fn fixture() -> (SocialModel, NativePlayerUiIntentQueue) {
    let mut social = wire::seeded(0);
    assert!(social.apply_network_packet(
        "UserInformation",
        &json!({"info":{"name":"Leader","realId":1}})
    ));
    assert!(social.guild.roster_complete && !social.guild.roster_needs_refresh);
    (social, NativePlayerUiIntentQueue::default())
}

#[derive(Clone, Copy, Debug)]
enum Protected {
    Permission,
    Read,
    Rename,
}

fn enqueue_protected(
    social: &mut SocialModel,
    queue: &mut NativePlayerUiIntentQueue,
    kind: Protected,
) -> NativePlayerUiIntent {
    match kind {
        Protected::Permission => {
            let request = social
                .guild_rank_permission_request("Leader", 2, 1, true)
                .unwrap();
            assert!(queue.push_guild_rank_permission_pending(social, request.clone()));
            NativePlayerUiIntent::GuildRankPermission(request)
        }
        Protected::Read => {
            let request = social.guild_roster_read_request(1).unwrap();
            assert!(queue.push_guild_roster_read_pending(social, request.clone()));
            NativePlayerUiIntent::GuildRosterRead(request)
        }
        Protected::Rename => {
            let request = social
                .guild_rank_rename_request("Leader", 2, "Veterans", 5100)
                .unwrap();
            assert!(queue.push_guild_rank_rename_pending(social, request));
            NativePlayerUiIntent::GuildEditMember {
                change_type: 3,
                rank_index: 2,
                name: String::new(),
                rank_name: "Veterans".into(),
            }
        }
    }
}

fn ordinary(index: usize) -> NativePlayerUiIntent {
    NativePlayerUiIntent::Chat {
        message: format!("ordinary-{index}"),
    }
}

fn contents(queue: &NativePlayerUiIntentQueue) -> Vec<NativePlayerUiIntent> {
    queue.intents.iter().cloned().collect()
}

fn mixed_prefix(
    social: &mut SocialModel,
    queue: &mut NativePlayerUiIntentQueue,
) -> Vec<NativePlayerUiIntent> {
    let mut prefix = Vec::new();
    for kind in [Protected::Permission, Protected::Read, Protected::Rename] {
        prefix.push(enqueue_protected(social, queue, kind));
    }
    // Legacy reads may still be queued; protecting a queued request is not a new authorization.
    let legacy = NativePlayerUiIntent::GuildRequestInfo { info_type: 0 };
    assert!(queue.push_intent(legacy.clone()));
    prefix.push(legacy);
    prefix
}

#[test]
fn guild_pending_eviction_queued_permission_read_and_rename_survive_later_ordinary_overflow() {
    assert_eq!(MAX_QUEUED, 24);
    for kind in [Protected::Permission, Protected::Read, Protected::Rename] {
        let (mut social, mut queue) = fixture();
        let protected = enqueue_protected(&mut social, &mut queue, kind);
        let pending = social.pending.clone();
        assert_eq!(pending.len(), 1);
        for index in 0..MAX_QUEUED - 1 {
            assert!(queue.push_intent(ordinary(index)));
        }
        assert_eq!(queue.intents.front(), Some(&protected));
        assert!(
            queue.push_intent(ordinary(1000)),
            "ordinary newest input may replace an older ordinary input: {kind:?}"
        );
        let now = contents(&queue);
        assert_eq!(now.len(), MAX_QUEUED);
        assert_eq!(
            now.first(),
            Some(&protected),
            "already queued request must not be silently evicted: {kind:?}"
        );
        assert!(
            !now.contains(&ordinary(0)),
            "oldest eligible ordinary input is evicted: {kind:?}"
        );
        assert_eq!(now.get(1), Some(&ordinary(1)));
        assert_eq!(now.last(), Some(&ordinary(1000)));
        assert_eq!(
            social.pending, pending,
            "queue pressure is neither an ACK nor cancellation: {kind:?}"
        );
        let drained = queue.drain_intents();
        assert_eq!(
            drained
                .iter()
                .filter(|intent| *intent == &protected)
                .count(),
            1
        );
        assert_eq!(
            social.pending, pending,
            "drain cannot invent receipt success: {kind:?}"
        );
    }
}

#[test]
fn guild_pending_eviction_skips_all_mixed_protected_prefixes_and_preserves_fifo_among_ordinary_inputs(
) {
    let (mut social, mut queue) = fixture();
    let prefix = mixed_prefix(&mut social, &mut queue);
    let pending = social.pending.clone();
    let guild = social.guild.clone();
    assert_eq!(pending.len(), 3);
    for index in 0..MAX_QUEUED - prefix.len() {
        assert!(queue.push_intent(ordinary(index)));
    }
    for index in 1000..1000 + MAX_QUEUED * 4 {
        assert!(queue.push_intent(ordinary(index)));
        let now = contents(&queue);
        assert_eq!(&now[..prefix.len()], prefix.as_slice());
        assert_eq!(now.len(), MAX_QUEUED);
        assert_eq!(social.pending, pending);
    }
    let now = contents(&queue);
    let remaining = MAX_QUEUED - prefix.len();
    let end = 1000 + MAX_QUEUED * 4;
    assert_eq!(
        &now[prefix.len()..],
        (end - remaining..end)
            .map(ordinary)
            .collect::<Vec<_>>()
            .as_slice()
    );
    assert_eq!(
        social.guild, guild,
        "flood handling cannot change read models or options"
    );
}

#[test]
fn guild_pending_eviction_a_full_protected_queue_rejects_new_ordinary_without_losing_any_request() {
    let (mut social, mut queue) = fixture();
    let prefix = mixed_prefix(&mut social, &mut queue);
    while queue.intents.len() < MAX_QUEUED {
        assert!(
            queue.push_intent(NativePlayerUiIntent::GuildBuffUpdate(GuildBuffRequest {
                action: 1,
                id: 9
            }))
        );
    }
    let before = contents(&queue);
    let pending = social.pending.clone();
    assert_eq!(&before[..prefix.len()], prefix.as_slice());
    assert_eq!(before.len(), MAX_QUEUED);
    for index in 0..MAX_QUEUED {
        assert!(!queue.push_intent(ordinary(index)));
        assert_eq!(contents(&queue), before);
        assert_eq!(social.pending, pending);
    }
    assert_eq!(
        queue.drain_intents(),
        before,
        "all original protected work remains drainable once"
    );
    assert!(queue.drain_intents().is_empty());
    assert_eq!(social.pending, pending);
}

#[test]
fn guild_pending_eviction_never_raises_the_eight_social_budget_to_match_twenty_four_queue_slots() {
    assert_eq!(MAX_SOCIAL_PENDING, 8);
    let (mut social, mut queue) = fixture();
    for kind in [Protected::Permission, Protected::Read, Protected::Rename] {
        enqueue_protected(&mut social, &mut queue, kind);
    }
    for index in social.pending.len()..MAX_SOCIAL_PENDING {
        assert!(social.begin_pending(SocialPendingOperation::GroupAdd {
            name: format!("Other{index}")
        }));
    }
    let pending = social.pending.clone();
    assert_eq!(pending.len(), 8);
    while queue.intents.len() < MAX_QUEUED {
        assert!(queue.push_intent(ordinary(queue.intents.len())));
    }
    assert!(queue.push_intent(ordinary(1000)));
    assert_eq!(social.pending, pending);
    let drained = queue.drain_intents();
    assert_eq!(drained.len(), MAX_QUEUED);
    assert!(drained
        .iter()
        .any(|intent| matches!(intent, NativePlayerUiIntent::GuildRankPermission(_))));
    assert!(drained
        .iter()
        .any(|intent| matches!(intent, NativePlayerUiIntent::GuildRosterRead(_))));
    assert!(drained.iter().any(|intent| matches!(
        intent,
        NativePlayerUiIntent::GuildEditMember { change_type: 3, .. }
    )));
    let ninth = social
        .guild_rank_permission_request("Leader", 2, 2, true)
        .unwrap();
    assert!(
        !queue.push_guild_rank_permission_pending(&mut social, ninth),
        "empty outbound queue must still refuse ninth social reservation"
    );
    assert_eq!(social.pending, pending);
    assert!(queue.drain_intents().is_empty());
}

#[test]
fn guild_pending_eviction_typed_read_duplicate_remains_rejected_after_queue_drain_without_new_pending(
) {
    let (mut social, mut queue) = fixture();
    let request = social.guild_roster_read_request(1).unwrap();
    assert!(queue.push_guild_roster_read_pending(&mut social, request.clone()));
    let pending = social.pending.clone();
    assert!(
        matches!(&pending[..], [SocialPendingOperation::GuildRosterRead(found)] if found == &request)
    );
    assert!(!queue.push_guild_roster_read_pending(&mut social, request.clone()));
    assert_eq!(social.pending, pending);
    assert_eq!(
        queue.drain_intents(),
        vec![NativePlayerUiIntent::GuildRosterRead(request.clone())]
    );
    assert!(!queue.push_guild_roster_read_pending(&mut social, request));
    assert_eq!(
        social.pending, pending,
        "same scope duplicate cannot add or replace its old pending read"
    );
    assert!(queue.drain_intents().is_empty());
}

#[test]
fn guild_pending_eviction_stale_read_cannot_reenter_after_guild_change_or_same_name_rejoin() {
    for same_name in [false, true] {
        let (mut social, mut queue) = fixture();
        let old = social.guild_roster_read_request(1).unwrap();
        assert!(wire::apply_typed(&mut social, wire::status("", "", -1)));
        let guild = if same_name {
            "Rank Guild"
        } else {
            "Other Guild"
        };
        assert!(wire::apply_typed(
            &mut social,
            wire::status(guild, "Leader", 0)
        ));
        assert!(wire::apply_typed(&mut social, wire::full("Scouts")));
        let before = social.clone();
        assert!(!queue.push_guild_roster_read_pending(&mut social, old));
        assert_eq!(
            social, before,
            "stale read does not leave a new pending record"
        );
        assert!(queue.drain_intents().is_empty());
        let fresh = social.guild_roster_read_request(1).unwrap();
        assert!(queue.push_guild_roster_read_pending(&mut social, fresh.clone()));
        assert_eq!(
            social.pending,
            vec![SocialPendingOperation::GuildRosterRead(fresh.clone())]
        );
        assert_eq!(
            queue.drain_intents(),
            vec![NativePlayerUiIntent::GuildRosterRead(fresh)]
        );
    }
}
