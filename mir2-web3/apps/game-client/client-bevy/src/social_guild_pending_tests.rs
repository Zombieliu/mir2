//! Ordinary typed packet cursor -> UI publication. No synthetic GuildMemberChange command echo.

use crate::social::guild_rank_native_tests::{
    apply_typed, full, full_ranks, rank, seeded, status, terminal,
};
use crate::social::{
    GuildRankPermissionRequest, SocialModel, SocialPendingOperation, SocialViewer,
};
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, UserInformation};
use serde_json::json;

#[test]
fn native_guild_read_retains_fresh_full_receipt_before_a_new_incomplete_delta() {
    let mut network = seeded(0);
    let mut local = network.clone();
    let request = local.guild_roster_read_request(1).unwrap();
    assert!(local.begin_pending(SocialPendingOperation::GuildRosterRead(request)));
    assert!(apply_typed(&mut network, full("Scouts")));
    assert!(network.apply_network_packet(
        "GuildMemberChange",
        &json!({"name":"New","status":2,"rankIndex":0,"ranks":[]})
    ));
    local.apply_authoritative(network.clone());
    assert!(
        local.pending.is_empty(),
        "the real full reply is retained even when a later delta needs another refresh"
    );
    assert!(local.guild.roster_needs_refresh);
    let second = local.guild_roster_read_request(1).unwrap();
    assert!(local.begin_pending(SocialPendingOperation::GuildRosterRead(second.clone())));
    local.apply_authoritative(network);
    assert_eq!(
        local.pending,
        vec![SocialPendingOperation::GuildRosterRead(second)],
        "a cached full reply cannot finish the successor read"
    );
}

fn information(name: &str, real_id: u32, object_id: u32) -> ServerPacket {
    ServerPacket::UserInformation {
        info: UserInformation {
            object_id,
            real_id,
            name: name.into(),
            guild_name: "Rank Guild".into(),
            guild_rank: "Leader".into(),
            name_colour_argb: -1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 30,
            location: Point { x: 288, y: 616 },
            direction: MirDirection::Down,
            hair: 0,
            hp: 80,
            mp: 30,
            experience: 0,
            max_experience: 1000,
            level_effects: 0,
            has_hero: false,
            hero_behaviour: 0,
            inventory_section_present: false,
            inventory: None,
            equipment_section_present: false,
            equipment: None,
            quest_inventory_section_present: false,
            quest_inventory: None,
            gold: 100,
            credit: 0,
            has_expanded_storage: false,
            has_storage_password: false,
            require_storage_password: false,
            storage_password_last_set_binary_datetime: 0,
            expanded_storage_expiry_time_binary_datetime: 0,
            magic_count: 0,
            intelligent_creature_count: 0,
            summoned_creature_type: 0,
            creature_summoned: false,
            allow_observe: false,
            observer: false,
            max_hp: Some(80),
            max_mp: Some(30),
        },
    }
}

fn permission_packet(actor: &str, target: i32, options: u8) -> ServerPacket {
    let mut packet = terminal(actor, target, "Scouts");
    if let ServerPacket::GuildMemberChange { ranks, .. } = &mut packet {
        ranks[0].options = options;
    }
    packet
}

fn member_packet(status: u8, name: &str, rank_index: u8) -> ServerPacket {
    ServerPacket::GuildMemberChange {
        name: name.into(),
        rank_index,
        status,
        ranks: Vec::new(),
    }
}

fn full_with_options(options: u8) -> ServerPacket {
    let mut packet = full("Scouts");
    if let ServerPacket::GuildMemberChange { ranks, .. } = &mut packet {
        ranks[2].options = options;
    }
    packet
}

struct Fixture {
    /// Gateway packet cursor has no UI queue. Publications can coalesce multiple packets.
    cursor: SocialModel,
    ui: SocialModel,
}

impl Fixture {
    fn new() -> Self {
        let mut cursor = seeded(0);
        assert!(apply_typed(&mut cursor, information("Leader", 1, 9001)));
        assert!(cursor.guild.roster_complete);
        assert_eq!(cursor.guild.my_options, 255);
        Self {
            ui: cursor.clone(),
            cursor,
        }
    }

    fn publish(&mut self) {
        self.ui.apply_authoritative(self.cursor.clone());
    }

    fn receive(&mut self, packet: ServerPacket) {
        assert!(apply_typed(&mut self.cursor, packet));
        self.publish();
    }

    fn permission(&mut self, option: u8, enabled: bool) -> GuildRankPermissionRequest {
        let request = self
            .ui
            .guild_rank_permission_request("Leader", 2, option, enabled)
            .expect("real viewer, complete roster and rank permission");
        assert!(self
            .ui
            .begin_pending(SocialPendingOperation::GuildRankPermission(request.clone())));
        request
    }

    fn contains(&self, request: &GuildRankPermissionRequest) -> bool {
        self.ui
            .pending
            .contains(&SocialPendingOperation::GuildRankPermission(
                request.clone(),
            ))
    }
}

#[test]
fn guild_permission_user_information_uses_real_character_index_not_world_object_id() {
    let mut model = seeded(0);
    let initial = model.clone();
    assert!(!model.apply_network_packet(
        "ObjectPlayer",
        &json!({"name":"Leader","realId":1,"objectId":9001})
    ));
    assert_eq!(model, initial);
    assert!(apply_typed(&mut model, information("Leader", 1, 9001)));
    assert_eq!(
        model.viewer,
        Some(SocialViewer {
            name: "Leader".into(),
            character_index: 1,
        })
    );
    assert!(
        model.guild.roster_complete,
        "identity arrival does not erase the guild"
    );
    let typed = serde_json::to_value(information("Leader", 1, 12345)).unwrap();
    let flat = &typed["UserInformation"]["info"];
    assert_eq!(flat["realId"], 1);
    assert!(model.apply_network_packet("UserInformation", flat));
    assert_eq!(model.viewer.as_ref().unwrap().character_index, 1);
    let before = model.clone();
    for payload in [
        json!({"name":"Leader","objectId":1}),
        json!({"name":"Leader","realId":u32::MAX}),
        json!({"name":"Leader","realId":-1}),
    ] {
        assert!(!model.apply_network_packet("UserInformation", &payload));
        assert_eq!(model, before, "invalid identity cannot default to an index");
    }
    assert!(!model.apply_network_packet(
        "ObjectPlayer",
        &json!({"name":"Other","realId":2,"objectId":9001})
    ));
    assert_eq!(model, before);
}

#[test]
fn guild_permission_builder_requires_real_viewer_complete_roster_rights_and_an_actual_flip() {
    let mut model = seeded(0);
    assert!(model
        .guild_rank_permission_request("Leader", 2, 1, true)
        .is_none());
    assert!(apply_typed(&mut model, information("Leader", 1, 9001)));
    let request = model
        .guild_rank_permission_request("Leader", 2, 1, true)
        .unwrap();
    assert_eq!(request.requester.character_index, 1);
    assert_eq!(request.rank_index, 2);
    assert_eq!(request.option, 1);
    assert!(request.enabled);
    assert!(model.guild_rank_permission_current(&request));
    for (requester, target, option, enabled) in [
        ("Officer", 2, 1, true),
        ("Leader", 99, 1, true),
        ("Leader", 2, 8, true),
        ("Leader", 2, 1, false),
    ] {
        assert!(model
            .guild_rank_permission_request(requester, target, option, enabled)
            .is_none());
    }
    let mut changed = model.clone();
    // Each independent fence invalidates the exact proof, including at final dispatch.
    changed.guild.roster_complete = false;
    assert!(!changed.guild_rank_permission_current(&request));
    changed = model.clone();
    changed.guild.roster_needs_refresh = true;
    assert!(!changed.guild_rank_permission_current(&request));
    changed = model.clone();
    changed.guild.my_options &= !1;
    assert!(!changed.guild_rank_permission_current(&request));
    changed = model.clone();
    changed.guild.my_rank_id = 3;
    assert!(!changed.guild_rank_permission_current(&request));
    changed = model.clone();
    changed.viewer.as_mut().unwrap().character_index = 777;
    assert!(!changed.guild_rank_permission_current(&request));
    changed = model;
    changed.guild.scope_epoch += 1;
    assert!(!changed.guild_rank_permission_current(&request));
}

#[test]
fn guild_permission_typed_status7_uses_nested_rank_index_and_the_exact_requested_bit() {
    let mut fixture = Fixture::new();
    let request = fixture.permission(1, true);
    let packet = permission_packet("Leader", 2, 3);
    let serialized = serde_json::to_value(&packet).unwrap();
    assert_eq!(serialized["GuildMemberChange"]["rankIndex"], 0);
    assert_eq!(serialized["GuildMemberChange"]["ranks"][0]["index"], 2);
    assert!(serialized["GuildMemberChange"].get("changeType").is_none());
    fixture.receive(packet);
    assert!(fixture.ui.guild.confirms_rank_permission(&request));
    assert!(!fixture.contains(&request));
    assert_eq!(fixture.ui.guild.ranks.len(), 3);
    assert_eq!(fixture.ui.guild.ranks[2].options, 3);
}

#[test]
fn guild_permission_expected_false_requires_a_clear_bit_not_any_status7_reply() {
    let mut fixture = Fixture::new();
    let request = fixture.permission(0, false);
    fixture.receive(permission_packet("Leader", 2, 3));
    assert!(fixture.contains(&request), "bit zero is still set");
    assert!(!fixture.ui.guild.confirms_rank_permission(&request));
    fixture.receive(permission_packet("Leader", 2, 2));
    assert!(!fixture.contains(&request));
    assert!(fixture.ui.guild.confirms_rank_permission(&request));
}

#[test]
fn guild_permission_distinct_options_have_independent_pending_and_share_the_existing_eight_slots() {
    let mut fixture = Fixture::new();
    let first = fixture.permission(1, true);
    let second = fixture.permission(2, true);
    assert!(!fixture
        .ui
        .begin_pending(SocialPendingOperation::GuildRankPermission(first.clone())));
    fixture.receive(permission_packet("Leader", 2, 3));
    assert!(!fixture.contains(&first));
    assert!(
        fixture.contains(&second),
        "another option is not the same operation"
    );
    fixture.receive(permission_packet("Leader", 2, 7));
    assert!(!fixture.contains(&second));

    let mut bounded = Fixture::new();
    for option in 0..8 {
        bounded.permission(option, option != 0);
    }
    assert_eq!(bounded.ui.pending.len(), 8);
    assert!(!bounded.ui.begin_pending(SocialPendingOperation::GroupAdd {
        name: "Peer".into()
    }));
}

#[test]
fn guild_permission_full_broadcast_other_actor_other_rank_or_wrong_bit_are_not_acks() {
    let mut fixture = Fixture::new();
    let request = fixture.permission(1, true);
    for packet in [
        full_with_options(3),
        permission_packet("Officer", 2, 3),
        permission_packet("Leader", 1, 3),
        permission_packet("Leader", 2, 1),
    ] {
        fixture.receive(packet);
        assert!(fixture.contains(&request));
        assert!(!fixture.ui.guild.confirms_rank_permission(&request));
    }
    fixture.receive(permission_packet("Leader", 2, 3));
    assert!(!fixture.contains(&request));
}

#[test]
fn guild_permission_retained_old_matching_cursor_cannot_ack_a_new_request_or_repeated_projection() {
    let mut fixture = Fixture::new();
    fixture.receive(permission_packet("Leader", 2, 3));
    fixture.receive(full_with_options(1));
    let request = fixture.permission(1, true);
    assert_eq!(
        request.observed_change_revision,
        fixture.cursor.guild.rank_change.as_ref().unwrap().revision
    );
    // The retained old status7 has the wanted bit, but it predates this request.
    assert_eq!(
        fixture.cursor.guild.rank_change.as_ref().unwrap().options,
        Some(3)
    );
    for _ in 0..3 {
        fixture.publish();
        assert!(fixture.contains(&request));
        assert!(!fixture.ui.guild.confirms_rank_permission(&request));
    }
    fixture.receive(permission_packet("Leader", 2, 3));
    assert!(!fixture.contains(&request));
}

#[test]
fn guild_permission_actual_status7_survives_a_later_full_packet_in_the_same_ui_publication() {
    let mut fixture = Fixture::new();
    let request = fixture.permission(1, true);
    assert!(apply_typed(
        &mut fixture.cursor,
        permission_packet("Leader", 2, 3)
    ));
    assert!(apply_typed(&mut fixture.cursor, full_with_options(3)));
    assert_eq!(fixture.cursor.guild.rank_packet_status, Some(255));
    assert!(fixture.cursor.guild.confirms_rank_permission(&request));
    fixture.publish();
    assert!(
        !fixture.contains(&request),
        "a retained real ACK must not be hidden by a same-frame roster"
    );
}

#[test]
fn guild_rename_uses_scope_fresh_actor_target_and_expected_name_not_any_rank_broadcast() {
    let mut fixture = Fixture::new();
    let request = fixture
        .ui
        .guild_rank_rename_request("Leader", 2, "Veterans", 5000)
        .unwrap();
    let pending = SocialPendingOperation::GuildRankRename(request.clone());
    assert!(fixture.ui.begin_pending(pending.clone()));
    let mut renamed_full = full("Veterans");
    if let ServerPacket::GuildMemberChange { ranks, .. } = &mut renamed_full {
        ranks[2].options = 1;
    }
    for packet in [
        renamed_full,
        terminal("Officer", 2, "Veterans"),
        terminal("Leader", 1, "Veterans"),
        terminal("Leader", 2, "Other Name"),
    ] {
        fixture.receive(packet);
        assert!(fixture.ui.pending.contains(&pending));
        assert!(!fixture.ui.guild.confirms_rank_rename(&request));
    }
    // Exercise the actual cursor coalescing order, without publishing the status7 separately.
    assert!(apply_typed(
        &mut fixture.cursor,
        terminal("Leader", 2, "Veterans")
    ));
    assert!(apply_typed(&mut fixture.cursor, full("Veterans")));
    fixture.publish();
    assert!(fixture.ui.guild.confirms_rank_rename(&request));
    assert!(!fixture.ui.pending.contains(&pending));
}

#[test]
fn guild_rename_only_has_the_strict_five_second_retry_exception_permission_does_not_expire() {
    let mut fixture = Fixture::new();
    let permission = fixture.permission(1, true);
    let rename = fixture
        .ui
        .guild_rank_rename_request("Leader", 2, "Veterans", 5000)
        .unwrap();
    let pending = SocialPendingOperation::GuildRankRename(rename.clone());
    assert!(fixture.ui.begin_pending(pending.clone()));
    fixture.ui.expire_guild_rank_renames(5000);
    assert!(
        fixture.ui.pending.contains(&pending),
        "equal deadline still blocks retry"
    );
    fixture.ui.expire_guild_rank_renames(5001);
    assert!(!fixture.ui.pending.contains(&pending));
    assert!(fixture.contains(&permission));
    assert!(
        !fixture.ui.guild.confirms_rank_rename(&rename),
        "expiry is not an ACK"
    );
}

#[test]
fn guild_member_move_status5_matches_exact_member_and_destination_and_ignores_fabricated_change_type(
) {
    let mut fixture = Fixture::new();
    let pending = SocialPendingOperation::GuildMember {
        change_type: 2,
        rank_index: 1,
        name: "Alice".into(),
    };
    assert!(fixture.ui.begin_pending(pending.clone()));
    let mut forged = serde_json::to_value(member_packet(0, "Alice", 1)).unwrap();
    forged["GuildMemberChange"]["changeType"] = json!(2);
    assert!(fixture
        .cursor
        .apply_network_packet("GuildMemberChange", &forged["GuildMemberChange"]));
    fixture.publish();
    assert!(fixture.ui.pending.contains(&pending));
    fixture.receive(member_packet(5, "Bob", 1));
    assert!(fixture.ui.pending.contains(&pending));
    fixture.receive(member_packet(5, "Alice", 0));
    assert!(fixture.ui.pending.contains(&pending));
    fixture.receive(member_packet(5, "Alice", 1));
    assert!(!fixture.ui.pending.contains(&pending));
    assert!(fixture
        .ui
        .guild
        .members
        .iter()
        .any(|member| member.name == "Alice" && member.id == 3 && member.rank_index == Some(1)));

    let mut self_move = Fixture::new();
    let mut ranks = full_ranks("Scouts");
    ranks[0] = rank(
        0,
        "Leader",
        255,
        &[("Leader", 1), ("CoLeader", 5), ("ThirdLeader", 6)],
    );
    self_move.receive(ServerPacket::GuildMemberChange {
        name: String::new(),
        rank_index: 0,
        status: 255,
        ranks,
    });
    let own_pending = SocialPendingOperation::GuildMember {
        change_type: 2,
        rank_index: 1,
        name: "Leader".into(),
    };
    assert!(self_move.ui.begin_pending(own_pending.clone()));
    self_move.receive(ServerPacket::GuildMemberChange {
        name: "Leader".into(),
        rank_index: 0,
        status: 8,
        ranks: vec![rank(1, "Officer", 65, &[("Officer", 2), ("Leader", 1)])],
    });
    assert!(!self_move.ui.pending.contains(&own_pending));
    assert_eq!(self_move.ui.guild.my_rank_id, 1);
    assert_eq!(self_move.ui.guild.my_options, 65);
}

#[test]
fn guild_kick_only_accepts_matching_status3_not_leave_online_or_a_fake_legacy_option_echo() {
    let pending = SocialPendingOperation::GuildMember {
        change_type: 1,
        rank_index: 0,
        name: "Bob".into(),
    };
    let mut departed = Fixture::new();
    assert!(departed.ui.begin_pending(pending.clone()));
    departed.receive(member_packet(4, "Bob", 0));
    assert!(
        departed.ui.pending.contains(&pending),
        "a voluntary leave is not a kick ACK"
    );

    let mut fixture = Fixture::new();
    assert!(fixture.ui.begin_pending(pending.clone()));
    fixture.receive(member_packet(3, "Alice", 0));
    assert!(fixture.ui.pending.contains(&pending));
    let mut forged = serde_json::to_value(member_packet(1, "Bob", 0)).unwrap();
    forged["GuildMemberChange"]["changeType"] = json!(1);
    assert!(fixture
        .cursor
        .apply_network_packet("GuildMemberChange", &forged["GuildMemberChange"]));
    fixture.publish();
    assert!(fixture.ui.pending.contains(&pending));
    fixture.receive(member_packet(3, "Bob", 0));
    assert!(!fixture.ui.pending.contains(&pending));

    let mut legacy = Fixture::new();
    let unidentified_bit = SocialPendingOperation::GuildMember {
        change_type: 5,
        rank_index: 2,
        name: "true".into(),
    };
    assert!(legacy.ui.begin_pending(unidentified_bit.clone()));
    legacy.receive(permission_packet("Leader", 2, 3));
    assert!(
        legacy.ui.pending.contains(&unidentified_bit),
        "the old request lost the option identity"
    );
    let before = legacy.cursor.clone();
    assert!(!legacy.cursor.apply_network_packet(
        "GuildMemberChange",
        &json!({
            "name":"true","rankIndex":2,"status":9,"ranks":[],"changeType":5
        })
    ));
    assert_eq!(legacy.cursor, before);
}

#[test]
fn guild_info_waits_for_a_fresh_status255_not_status_metadata_other_deltas_or_republication() {
    let mut fixture = Fixture::new();
    assert!(fixture.ui.begin_pending(SocialPendingOperation::GuildInfo));
    fixture.publish();
    assert!(fixture
        .ui
        .pending
        .contains(&SocialPendingOperation::GuildInfo));
    for packet in [
        status("Rank Guild", "Leader", 0),
        member_packet(0, "Bob", 0),
        member_packet(2, "NewMember", 0),
        terminal("Leader", 2, "Scouts"),
    ] {
        fixture.receive(packet);
        assert!(fixture
            .ui
            .pending
            .contains(&SocialPendingOperation::GuildInfo));
    }
    fixture.receive(full("Scouts"));
    assert!(!fixture
        .ui
        .pending
        .contains(&SocialPendingOperation::GuildInfo));
    assert!(fixture.ui.begin_pending(SocialPendingOperation::GuildInfo));
    fixture.publish();
    assert!(
        fixture
            .ui
            .pending
            .contains(&SocialPendingOperation::GuildInfo),
        "identical publication is not a new full-list packet"
    );
}

#[test]
fn guild_scope_change_cancels_only_guild_work_map_preserves_pending_and_session_reset_clears_all() {
    let mut fixture = Fixture::new();
    fixture.permission(1, true);
    assert!(fixture.ui.begin_pending(SocialPendingOperation::GuildInfo));
    assert!(fixture
        .ui
        .begin_pending(SocialPendingOperation::GuildMember {
            change_type: 2,
            rank_index: 1,
            name: "Alice".into()
        }));
    let rename = fixture
        .ui
        .guild_rank_rename_request("Leader", 2, "Veterans", 5000)
        .unwrap();
    assert!(fixture
        .ui
        .begin_pending(SocialPendingOperation::GuildRankRename(rename)));
    let group = SocialPendingOperation::GroupAdd {
        name: "Peer".into(),
    };
    let trade = SocialPendingOperation::TradeDeposit { from: 2, to: 0 };
    assert!(fixture.ui.begin_pending(group.clone()));
    assert!(fixture.ui.begin_pending(trade.clone()));
    let pending = fixture.ui.pending.clone();
    let viewer = fixture.ui.viewer.clone();
    fixture.ui.clear_scene();
    fixture.cursor.clear_scene();
    assert_eq!(fixture.ui.pending, pending);
    assert_eq!(fixture.ui.viewer, viewer);
    fixture.receive(status("Different Guild", "Leader", 0));
    assert_eq!(
        fixture.ui.pending,
        vec![group, trade],
        "guild retirement is not successful delivery and must not cancel other ledgers"
    );
    assert_eq!(fixture.ui.viewer, viewer);
    fixture.ui.clear_session();
    assert!(fixture.ui.pending.is_empty());
    assert!(fixture.ui.viewer.is_none());
    assert!(fixture.ui.guild.name.is_none());
}

#[test]
fn guild_same_viewer_refresh_preserves_pending_but_same_name_with_a_new_real_id_retires_it() {
    let mut fixture = Fixture::new();
    let request = fixture.permission(1, true);
    let group = SocialPendingOperation::GroupAdd {
        name: "Peer".into(),
    };
    assert!(fixture.ui.begin_pending(group.clone()));
    fixture.receive(information("Leader", 1, 9876));
    assert!(fixture.contains(&request));
    assert!(fixture.ui.pending.contains(&group));
    assert_eq!(fixture.ui.viewer.as_ref().unwrap().character_index, 1);
    fixture.receive(information("Leader", 77, 9876));
    assert_eq!(fixture.ui.viewer.as_ref().unwrap().character_index, 77);
    assert!(fixture.ui.pending.is_empty());
    assert!(fixture.ui.guild.name.is_none());
    assert!(!fixture.ui.guild_rank_permission_current(&request));
}
