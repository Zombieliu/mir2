//! Actual bounded native FIFO/reset consumer checks. Packet JSON has a typed
//! protocol counterpart in social_guild_rank_tests; this is not network QA.
use super::*;
use mir2_client_bevy::social::{SocialModel, SocialPendingOperation};
use serde_json::{json, Value};

fn rank(index: i32, name: &str, options: u8, members: &[(&str, i32)]) -> Value {
    json!({"index":index,"name":name,"options":options,"members":members.iter()
        .map(|(name,id)|json!({"name":name,"id":id,"lastLoginBinaryDatetime":123,
            "hasVoted":false,"online":true})).collect::<Vec<_>>()})
}

fn ranks(name: &str) -> Vec<Value> {
    vec![
        rank(0, "Leader", 255, &[("Leader", 1)]),
        rank(1, "Officer", 65, &[("Officer", 2)]),
        rank(2, name, 1, &[("Alice", 3), ("Bob", 4)]),
    ]
}

fn full(name: &str) -> Value {
    json!({"name":"","rankIndex":0,"status":255,"ranks":ranks(name)})
}

fn terminal() -> Value {
    json!({"name":"Leader","rankIndex":0,"status":7,"ranks":[ranks("Veterans")[2].clone()]})
}

fn seeded() -> SocialModel {
    let mut social = SocialModel::default();
    assert!(social.apply_network_packet(
        "GuildStatus",
        &json!({
        "guildName":"Rank Guild","guildRankName":"Leader","level":3,
        "experience":41,"maxExperience":1000,"gold":99123,"sparePoints":2,
        "memberCount":4,"maxMembers":50,"voting":false,"itemCount":0,
        "buffCount":0,"myOptions":255,"myRankId":0})
    ));
    assert!(social.apply_network_packet("GuildMemberChange", &full("Scouts")));
    social
}

fn pending_app() -> (App, SocialModel, SocialPendingOperation) {
    let mut app = ingest_app();
    let network = seeded();
    let mut local = network.clone();
    let request = local
        .guild_rank_rename_request("Leader", 2, "Veterans", 5100)
        .unwrap();
    assert!(local.begin_pending(SocialPendingOperation::GuildRankRename(request)));
    let unrelated = SocialPendingOperation::GroupAdd {
        name: "Friend".into(),
    };
    assert!(local.begin_pending(unrelated.clone()));
    app.insert_resource(local);
    (app, network, unrelated)
}

fn enqueue(network: &SocialModel) {
    assert!(native_ingest::push_native_social_model(
        serde_json::to_string(network).unwrap()
    ));
}

#[test]
fn guild_rank_ingest_fifo_retains_terminal_before_metadata_in_one_main_frame() {
    let _guard = native_ingest::native_queue_test_guard();
    let (mut app, mut network, unrelated) = pending_app();
    assert!(network.apply_network_packet("GuildMemberChange", &terminal()));
    enqueue(&network);
    assert!(network.apply_network_packet("GuildMemberChange", &full("Veterans")));
    enqueue(&network);
    assert_eq!(
        app.world()
            .resource::<native_ingest::NativeInbound>()
            .diagnostics()
            .message_count,
        2
    );
    app.update();
    let local = app.world().resource::<SocialModel>();
    assert_eq!(local.pending, vec![unrelated]);
    assert_eq!(local.guild.rank_packet_status, Some(255));
    assert_eq!(local.guild.ranks.len(), 3);
    assert_eq!(local.guild.members.len(), 4);
    assert_eq!(local.guild.ranks[2].name, "Veterans");
    assert_eq!(
        app.world()
            .resource::<native_ingest::NativeInbound>()
            .diagnostics()
            .message_count,
        0
    );
}

#[test]
fn guild_rank_ingest_metadata_only_does_not_ack_even_if_text_matches() {
    let _guard = native_ingest::native_queue_test_guard();
    let (mut app, mut network, _) = pending_app();
    let pending = app.world().resource::<SocialModel>().pending.clone();
    assert!(network.apply_network_packet("GuildMemberChange", &full("Veterans")));
    enqueue(&network);
    app.update();
    assert_eq!(app.world().resource::<SocialModel>().pending, pending);
}

#[test]
fn guild_rank_ingest_scene_reset_retains_pending_and_ordinary_roster() {
    let _guard = native_ingest::native_queue_test_guard();
    let (mut app, mut network, _) = pending_app();
    let pending = app.world().resource::<SocialModel>().pending.clone();
    assert!(native_ingest::push_native_scene_reset());
    assert!(network.apply_network_packet("GuildMemberChange", &full("Scouts")));
    enqueue(&network);
    app.update();
    let local = app.world().resource::<SocialModel>();
    assert_eq!(local.pending, pending);
    assert_eq!(local.guild.name.as_deref(), Some("Rank Guild"));
    assert_eq!(local.guild.ranks.len(), 3);
    assert_eq!(local.guild.members.len(), 4);
}

#[test]
fn guild_rank_ingest_session_reset_retires_old_queued_terminal_and_pending() {
    let _guard = native_ingest::native_queue_test_guard();
    let (mut app, mut old, _) = pending_app();
    assert!(old.apply_network_packet("GuildMemberChange", &terminal()));
    enqueue(&old);
    assert!(native_ingest::push_native_data_reset());
    let mut fresh = SocialModel::default();
    assert!(fresh.apply_network_packet(
        "GuildStatus",
        &json!({
        "guildName":"Fresh Guild","guildRankName":"Leader","level":3,
        "experience":41,"maxExperience":1000,"gold":99123,"sparePoints":2,
        "memberCount":4,"maxMembers":50,"voting":false,"itemCount":0,
        "buffCount":0,"myRankId":0,"myOptions":255})
    ));
    assert!(fresh.apply_network_packet("GuildMemberChange", &full("Fresh Rank")));
    enqueue(&fresh);
    app.update();
    let local = app.world().resource::<SocialModel>();
    assert!(local.pending.is_empty());
    assert_eq!(local.guild.name.as_deref(), Some("Fresh Guild"));
    assert_eq!(local.guild.ranks[2].name, "Fresh Rank");
    assert!(
        local.guild.rank_change.is_none(),
        "retired terminal cannot confirm new-session work"
    );
}
