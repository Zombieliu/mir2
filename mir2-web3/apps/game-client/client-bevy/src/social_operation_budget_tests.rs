use super::*;
use crate::social::guild_rank_native_tests as wire;
use serde_json::json;
use std::collections::BTreeSet;

fn owner() -> SocialBudgetOwner {
    SocialBudgetOwner {
        generation: 1,
        connection_generation: 1,
        session_generation: 2,
        player_object_id: 1000,
        scene_revision: 3,
        map_file_name: "0".into(),
    }
}
fn ready() -> SocialOperationBudget {
    let mut budget = SocialOperationBudget::default();
    budget.reset(owner()).unwrap();
    budget.model = wire::seeded(0);
    budget
        .packet(
            &owner(),
            "UserInformation",
            &json!({"name":"Leader","realId":1}),
        )
        .unwrap();
    budget
        .packet(&owner(), "GroupInvite", &json!({"name":"Inviter"}))
        .unwrap();
    budget
        .packet(&owner(), "GuildInvite", &json!({"name":"Recruiter"}))
        .unwrap();
    budget
        .packet(&owner(), "TradeAccept", &json!({"name":"Guest"}))
        .unwrap();
    budget
}
fn reserve(budget: &mut SocialOperationBudget, command: Value) -> SocialBudgetPending {
    budget
        .reserve(&owner(), command, None, None)
        .unwrap()
        .unwrap()
}
fn enter(budget: &mut SocialOperationBudget, command: Value) -> SocialBudgetPending {
    let row = reserve(budget, command.clone());
    budget.enter(&owner(), row.token, &command).unwrap();
    row
}
fn typed_packet(budget: &mut SocialOperationBudget, packet: mir2_protocol::ServerPacket) {
    let value = serde_json::to_value(packet).unwrap();
    let (packet, payload) = value.as_object().unwrap().iter().next().unwrap();
    assert!(budget.packet(&owner(), packet, payload).unwrap());
}
fn twenty(model: &SocialModel) -> Vec<SocialPendingOperation> {
    use SocialPendingOperation::*;
    vec![
        GroupSwitch { allow_group: true },
        GroupAdd { name: "New".into() },
        GroupRemove { name: "Old".into() },
        GroupInviteAccept {
            inviter: "Inviter".into(),
            invite_epoch: model.group.pending_invite_epoch,
        },
        GuildInfo,
        GuildRosterRead(model.guild_roster_read_request(1).unwrap()),
        GuildMember {
            change_type: 1,
            rank_index: 2,
            name: "Alice".into(),
        },
        GuildRankRename(
            model
                .guild_rank_rename_request("Leader", 2, "Veterans", 5000)
                .unwrap(),
        ),
        GuildRankPermission(
            model
                .guild_rank_permission_request("Leader", 2, 1, true)
                .unwrap(),
        ),
        GuildNotice {
            notice: vec!["New notice".into()],
        },
        GuildInviteAccept {
            inviter: "Recruiter".into(),
            invite_epoch: model.guild.pending_invite_epoch,
        },
        GuildStorageGold {
            change_type: 0,
            amount: 10,
        },
        GuildStorageItem {
            change_type: 0,
            from: 1,
            to: 2,
        },
        TradeRequest,
        TradeReply,
        TradeGold { amount: 10 },
        TradeDeposit { from: 42, to: 3 },
        TradeRetrieve { from: 3, to: 42 },
        TradeConfirm { locked: true },
        TradeCancel,
    ]
}

#[test]
fn all_twenty_native_variants_reserve_with_only_group_add_retiring_at_entry() {
    let operations = twenty(&ready().model);
    assert_eq!(operations.len(), 20);
    let mut kinds = BTreeSet::new();
    for operation in operations {
        let mut budget = ready();
        let kind = operation_kind(&operation);
        kinds.insert(kind);
        assert_eq!(
            serde_json::to_value(&operation).unwrap()["kind"],
            kind,
            "Native serde names are the bridge names"
        );
        let command = json!({"testNativeOperation":kind});
        let proof = budget
            .reserve_operation(&owner(), operation.clone(), command.clone())
            .unwrap();
        assert_eq!(budget.model.pending, vec![operation]);
        assert!(!budget.pending()[0].entered);
        budget.enter(&owner(), proof.token, &command).unwrap();
        if kind == "groupAdd" {
            assert!(budget.pending().is_empty());
            assert_eq!(budget.cancel_unsent(&owner(), proof.token), Err("unknownToken"));
            continue;
        }
        assert_eq!(
            budget.enter(&owner(), proof.token, &command),
            Err("staleReservation")
        );
        assert_eq!(
            budget.cancel_unsent(&owner(), proof.token),
            Err("alreadyEntered")
        );
        assert!(!budget
            .packet(&owner(), "Chat", &json!({"message":"not a social ACK"}))
            .unwrap());
        budget.expire(&owner(), 5000).unwrap();
        assert_eq!(
            budget.pending().len(),
            1,
            "no timeout at the rename boundary or for any other variant"
        );
    }
    assert_eq!(kinds.len(), 20);
}

#[test]
fn reserved_and_entered_share_the_native_eight_limit_and_definitely_unsent_refunds_one() {
    let mut budget = ready();
    let commands: Vec<_> = (0..8)
        .map(|i| json!({"type":"delMember","name":format!("Peer{i}")}))
        .collect();
    let mut rows = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        let row = reserve(&mut budget, command.clone());
        if index % 2 == 0 {
            budget.enter(&owner(), row.token, command).unwrap();
        }
        rows.push(row);
    }
    assert_eq!(budget.pending().len(), MAX_SOCIAL_PENDING);
    assert_eq!(
        budget.reserve(&owner(), json!({"type":"tradeCancel"}), None, None),
        Err("budgetUnavailable")
    );
    assert_eq!(
        budget.cancel_unsent(&owner(), rows[0].token),
        Err("alreadyEntered")
    );
    budget.cancel_unsent(&owner(), rows[1].token).unwrap();
    assert_eq!(
        budget.reserve(&owner(), commands[0].clone(), None, None),
        Err("budgetUnavailable"),
        "Native duplicate still applies with free capacity"
    );
    reserve(&mut budget, json!({"type":"tradeCancel"}));
    assert_eq!(budget.pending().len(), 8);
}

#[test]
fn group_add_reserves_one_slot_then_retires_only_its_own_dispatch() {
    let mut budget = ready();
    let held = enter(&mut budget, json!({"type":"delMember","name":"Peer"}));
    let command = json!({"type":"addMember","name":"pEeR"});
    let original_group = budget.model.group.clone();
    let first = reserve(&mut budget, command.clone());
    assert_eq!(budget.pending().len(), 2);
    assert_eq!(
        budget.reserve(&owner(), command.clone(), None, None),
        Err("budgetUnavailable")
    );
    budget.enter(&owner(), first.token, &command).unwrap();
    assert_eq!(budget.model.group, original_group);
    assert_eq!(budget.pending().len(), 1);
    assert_eq!(budget.pending()[0].token, held.token);
    assert_eq!(
        budget.enter(&owner(), first.token, &command),
        Err("unknownToken")
    );
    let second = enter(&mut budget, command);
    assert!(second.token > first.token);
    assert_eq!(budget.pending()[0].token, held.token);
    budget
        .packet(&owner(), "AddMember", &json!({"name":"Peer"}))
        .unwrap();
    assert!(budget.model.group.members.iter().any(|member| member.name == "Peer"));
    assert_eq!(budget.pending()[0].token, held.token);
}

#[test]
fn packet_before_entry_cannot_complete_reserved_work_or_replay_after_entry() {
    let mut budget = ready();
    let command = json!({"type":"switchGroup","allowGroup":true});
    let row = reserve(&mut budget, command.clone());
    budget
        .packet(&owner(), "SwitchGroup", &json!({"allowGroup":true}))
        .unwrap();
    assert_eq!(budget.pending()[0].token, row.token);
    assert!(!budget.pending()[0].entered);
    assert_eq!(
        budget.enter(&owner(), row.token, &command),
        Err("staleReservation"),
        "the early reply cannot be retagged as an entered request"
    );
    budget.cancel_unsent(&owner(), row.token).unwrap();
    let next = enter(&mut budget, command);
    assert!(next.token > row.token);
    budget.expire(&owner(), 900_000).unwrap();
    assert_eq!(
        budget.pending().len(),
        1,
        "elapsed time cannot replay the early reply"
    );
    budget
        .packet(&owner(), "SwitchGroup", &json!({"allowGroup":true}))
        .unwrap();
    assert!(budget.pending().is_empty());
    assert_eq!(
        budget.cancel_unsent(&owner(), row.token),
        Err("unknownToken")
    );
}

#[test]
fn exact_dto_owner_map_and_scene_are_rechecked_at_entry_and_old_tokens_do_not_recur() {
    let mut budget = ready();
    let command = json!({"type":"addMember","name":"New"});
    let row = reserve(&mut budget, command.clone());
    assert_eq!(
        budget.enter(
            &owner(),
            row.token,
            &json!({"type":"addMember","name":"Other"})
        ),
        Err("staleReservation")
    );
    let mut scene = owner();
    scene.scene_revision += 1;
    scene.map_file_name = "1".into();
    budget.reset(scene.clone()).unwrap();
    assert_eq!(budget.pending().len(), 1);
    assert_eq!(
        budget.enter(&scene, row.token, &command),
        Err("staleReservation")
    );
    budget.cancel_unsent(&scene, row.token).unwrap();
    assert_eq!(budget.reset(owner()), Err("retiredOwner"));
    let next = budget
        .reserve(&scene, command.clone(), None, None)
        .unwrap()
        .unwrap();
    assert!(next.token > row.token);
    budget.enter(&scene, next.token, &command).unwrap();
    let mut foreign = scene.clone();
    foreign.player_object_id += 1;
    assert_eq!(
        budget.packet(&foreign, "AddMember", &json!({"name":"New"})),
        Err("retiredOwner")
    );
    budget
        .packet(&scene, "AddMember", &json!({"name":"New"}))
        .unwrap();
    assert!(budget.pending().is_empty());
    budget.retire(&scene).unwrap();
    assert_eq!(budget.reset(scene.clone()), Err("retiredOwner"));
    let mut session = scene;
    session.session_generation += 1;
    session.player_object_id += 1;
    budget.reset(session).unwrap();
    assert_eq!(
        budget.enter(&owner(), next.token, &command),
        Err("retiredOwner")
    );
}

#[test]
fn actual_session_retirement_clears_all_but_scene_changes_keep_entered_native_work() {
    let mut budget = ready();
    enter(&mut budget, json!({"type":"delMember","name":"New"}));
    reserve(&mut budget, json!({"type":"tradeCancel"}));
    let mut scene = owner();
    scene.scene_revision += 1;
    scene.map_file_name = "D001".into();
    budget.reset(scene.clone()).unwrap();
    assert_eq!(budget.pending().len(), 2);
    assert!(budget.model.last_event.is_none());
    budget
        .packet(&scene, "DeleteMember", &json!({"name":"New"}))
        .unwrap();
    assert_eq!(budget.pending().len(), 1);
    budget.retire(&scene).unwrap();
    assert!(budget.pending().is_empty());
    assert_eq!(budget.model, SocialModel::default());
}

#[test]
fn entered_trade_terminal_and_guild_retirement_follow_native_scope_without_clearing_other_work() {
    let mut budget = ready();
    enter(&mut budget, json!({"type":"delMember","name":"New"}));
    let trade = enter(
        &mut budget,
        json!({"type":"depositTradeItem","from":42,"to":3}),
    );
    let guild = enter(
        &mut budget,
        json!({"type":"guildStorageGoldChange","changeType":0,"amount":10}),
    );
    budget
        .packet(&owner(), "TradeCancel", &json!({"unlock":true}))
        .unwrap();
    assert_eq!(budget.pending().len(), 3);
    budget
        .packet(&owner(), "TradeCancel", &json!({"unlock":false}))
        .unwrap();
    assert_eq!(budget.pending().len(), 2);
    assert!(!budget.pending().iter().any(|row| row.token == trade.token));
    typed_packet(&mut budget, wire::status("", "", 0));
    assert_eq!(budget.pending().len(), 1);
    assert!(!budget.pending().iter().any(|row| row.token == guild.token));
    assert_eq!(budget.pending()[0].kind, "groupRemove");
}

#[test]
fn matching_success_or_failure_releases_only_entered_trade_tuple_and_late_receipt_does_not_revive()
{
    for success in [false, true] {
        let mut budget = ready();
        let a = enter(
            &mut budget,
            json!({"type":"depositTradeItem","from":42,"to":3}),
        );
        let b = enter(
            &mut budget,
            json!({"type":"retrieveTradeItem","from":4,"to":43}),
        );
        budget
            .packet(
                &owner(),
                "DepositTradeItem",
                &json!({"from":42,"to":2,"success":success}),
            )
            .unwrap();
        assert_eq!(budget.pending().len(), 2);
        budget
            .packet(
                &owner(),
                "DepositTradeItem",
                &json!({"from":42,"to":3,"success":success}),
            )
            .unwrap();
        assert_eq!(budget.pending().len(), 1);
        assert_eq!(budget.pending()[0].token, b.token);
        budget.packet(&owner(), "TradeConfirm", &json!({})).unwrap();
        assert!(budget.pending().is_empty());
        budget
            .packet(
                &owner(),
                "DepositTradeItem",
                &json!({"from":42,"to":3,"success":success}),
            )
            .unwrap();
        assert!(budget.pending().is_empty());
        assert_eq!(
            budget.enter(
                &owner(),
                a.token,
                &json!({"type":"depositTradeItem","from":42,"to":3})
            ),
            Err("unknownToken")
        );
    }
}

#[test]
fn duplicate_accept_preserves_native_offer_open_revision_and_entered_custody() {
    let mut budget = ready();
    budget.model.trade.my_gold = 17;
    enter(&mut budget, json!({"type":"tradeCancel"}));
    let before = budget.model.trade.clone();
    budget
        .packet(&owner(), "TradeAccept", &json!({"name":"Guest"}))
        .unwrap();
    assert_eq!(budget.model.trade.open_revision, before.open_revision);
    assert_eq!(budget.model.trade.my_gold, 17);
    assert_eq!(budget.pending().len(), 1);
    budget
        .packet(&owner(), "TradeAccept", &json!({"name":"Other"}))
        .unwrap();
    assert!(budget.pending().is_empty());
}

#[test]
fn typed_read_permission_and_rename_use_native_viewer_scope_and_exact_real_receipts() {
    let mut budget = ready();
    let read = enter(&mut budget, json!({"type":"requestGuildInfo","infoType":1}));
    typed_packet(&mut budget, wire::terminal("Leader", 2, "Veterans"));
    assert_eq!(budget.pending()[0].token, read.token);
    typed_packet(&mut budget, wire::full("Veterans"));
    assert!(budget.pending().is_empty());
    let command =
        json!({"type":"editGuildMember","changeType":5,"rankIndex":2,"name":"true","rankName":"1"});
    let permission = enter(&mut budget, command);
    assert_eq!(
        budget.model.guild.ranks[2].options, 1,
        "no optimistic permission update"
    );
    let mut wrong = serde_json::to_value(wire::terminal("Officer", 2, "Veterans")).unwrap();
    wrong["GuildMemberChange"]["ranks"][0]["options"] = json!(3);
    budget
        .packet(&owner(), "GuildMemberChange", &wrong["GuildMemberChange"])
        .unwrap();
    assert_eq!(budget.pending()[0].token, permission.token);
    wrong["GuildMemberChange"]["name"] = json!("Leader");
    budget
        .packet(&owner(), "GuildMemberChange", &wrong["GuildMemberChange"])
        .unwrap();
    assert!(budget.pending().is_empty());
    let rename = json!({"type":"editGuildMember","changeType":3,"rankIndex":2,"name":"","rankName":" Scouts "});
    let row = budget
        .reserve(&owner(), rename.clone(), None, Some(5000))
        .unwrap()
        .unwrap();
    budget.enter(&owner(), row.token, &rename).unwrap();
    budget.expire(&owner(), 5000).unwrap();
    assert_eq!(budget.pending().len(), 1);
    budget.expire(&owner(), 5001).unwrap();
    assert!(
        budget.pending().is_empty(),
        "strictly after deadline permits retry without an ACK"
    );
    assert_eq!(budget.expire(&owner(), 5000), Err("invalidClock"));
}

#[test]
fn reserved_typed_read_is_not_finished_by_a_full_reply_before_entry() {
    let mut budget = ready();
    let command = json!({"type":"requestGuildInfo","infoType":1});
    let row = reserve(&mut budget, command.clone());
    typed_packet(&mut budget, wire::full("Scouts"));
    assert_eq!(budget.pending()[0].token, row.token);
    assert_eq!(
        budget.enter(&owner(), row.token, &command),
        Err("staleReservation")
    );
    budget.cancel_unsent(&owner(), row.token).unwrap();
    let next = enter(&mut budget, command);
    assert!(next.token > row.token);
    budget
        .packet(
            &owner(),
            "GuildStatus",
            &serde_json::to_value(wire::status("Rank Guild", "Leader", 0)).unwrap()["GuildStatus"],
        )
        .unwrap();
    assert_eq!(
        budget.pending().len(),
        1,
        "an unrelated packet cannot replay the full reply observed before this new request"
    );
    typed_packet(&mut budget, wire::full("Scouts"));
    assert!(budget.pending().is_empty());
}

#[test]
fn validate_is_readonly_and_cannot_enter_or_revive_a_stale_reservation() {
    let mut budget = ready();
    let command = json!({"type":"addMember","name":"Peer"});
    let row = reserve(&mut budget, command.clone());
    let before = budget.model.clone();
    let pending = budget.pending();
    for _ in 0..3 {
        budget.validate(&owner(), row.token, &command).unwrap();
    }
    assert_eq!(budget.model, before);
    assert_eq!(budget.pending(), pending);
    budget
        .packet(&owner(), "SwitchGroup", &json!({"allowGroup":true}))
        .unwrap();
    assert_eq!(
        budget.validate(&owner(), row.token, &command),
        Err("staleReservation")
    );
    assert_eq!(
        budget.enter(&owner(), row.token, &command),
        Err("staleReservation")
    );
    assert!(!budget.pending()[0].entered);
    budget.cancel_unsent(&owner(), row.token).unwrap();
    let next = enter(&mut budget, command.clone());
    assert_eq!(
        budget.validate(&owner(), next.token, &command),
        Err("unknownToken")
    );
}

#[test]
fn ordinary_mapping_uses_native_invite_epoch_and_does_not_swallow_other_domains() {
    let mut budget = ready();
    let hint = SocialBudgetInvite {
        name: "Inviter".into(),
    };
    let row = budget
        .reserve(
            &owner(),
            json!({"type":"groupInvite","acceptInvite":true}),
            Some(&hint),
            None,
        )
        .unwrap()
        .unwrap();
    let operation = &budget
        .rows
        .iter()
        .find(|r| r.token == row.token)
        .unwrap()
        .operation;
    assert!(
        matches!(operation,SocialPendingOperation::GroupInviteAccept{invite_epoch,..} if *invite_epoch==budget.model.group.pending_invite_epoch)
    );
    for command in [
        json!({"type":"groupInvite","acceptInvite":false}),
        json!({"type":"guildInvite","acceptInvite":false}),
        json!({"type":"guildStorageItemChange","changeType":3,"from":0,"to":0}),
        json!({"type":"refreshFriends"}),
        json!({"type":"marriageReply","acceptInvite":true}),
        json!({"type":"sendMail","gold":1}),
    ] {
        assert_eq!(budget.reserve(&owner(), command, None, None).unwrap(), None);
    }
    for command in [
        json!({"type":"tradeCancel","extra":1}),
        json!({"type":"editGuildMember","changeType":5,"rankIndex":2,"name":"TRUE","rankName":"1"}),
        json!({"type":"editGuildMember","changeType":5,"rankIndex":2,"name":"true","rankName":"8"}),
    ] {
        assert!(budget.reserve(&owner(), command, None, None).is_err());
    }
}

#[test]
fn own_offer_projection_is_required_for_exact_trade_gold_delta_and_ignores_equal_or_foreign_snapshots(
) {
    let mut budget = ready();
    let inventory = json!({"capacity":46,"gold":100,"items":[]});
    let snapshot = |gold| {
        json!({"stage5Systems":{"trade":{"partner":"Guest","settlementNonce":"a","offeredSlots":{},
        "offeredUniqueIds":{},"offeredGold":gold,"offeredCurrency":"gold","locked":false,"completed":false}}})
    };
    assert!(budget
        .snapshot(&owner(), &snapshot(10), &inventory)
        .unwrap());
    let row = enter(&mut budget, json!({"type":"tradeGold","amount":5}));
    budget
        .packet(&owner(), "TradeGold", &json!({"amount":15}))
        .unwrap();
    assert_eq!(budget.pending()[0].token, row.token);
    assert!(!budget
        .snapshot(&owner(), &snapshot(10), &inventory)
        .unwrap());
    assert_eq!(budget.pending().len(), 1);
    assert!(budget
        .snapshot(&owner(), &snapshot(14), &inventory)
        .unwrap());
    assert_eq!(
        budget.pending().len(),
        1,
        "wrong positive delta is not an ACK"
    );
    // A fresh request begins at the observed 14; it cannot borrow the earlier 10 baseline.
    budget
        .packet(&owner(), "TradeCancel", &json!({"unlock":false}))
        .unwrap();
    assert!(budget.pending().is_empty());
    budget
        .packet(&owner(), "TradeAccept", &json!({"name":"Guest"}))
        .unwrap();
    budget
        .snapshot(&owner(), &snapshot(10), &inventory)
        .unwrap();
    enter(&mut budget, json!({"type":"tradeGold","amount":5}));
    let mut wrong = snapshot(15);
    wrong["stage5Systems"]["trade"]["partner"] = json!("Other");
    assert!(!budget.snapshot(&owner(), &wrong, &inventory).unwrap());
    assert_eq!(budget.pending().len(), 1);
    assert!(budget
        .snapshot(&owner(), &snapshot(15), &inventory)
        .unwrap());
    assert!(budget.pending().is_empty());
}
