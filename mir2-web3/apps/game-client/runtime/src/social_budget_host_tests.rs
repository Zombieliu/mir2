use super::*;

fn owner() -> Value {
    json!({"generation":1,"connectionGeneration":1,"sessionGeneration":2,"playerObjectId":1000,"sceneRevision":3,"mapFileName":"0"})
}
fn call(host: &mut Host, operation: &str, fields: Value) -> Value {
    let mut input = fields;
    input["version"] = json!(1);
    input["operation"] = json!(operation);
    input["owner"] = owner();
    serde_json::from_str(&host.process(&input.to_string())).unwrap()
}
fn ready() -> Host {
    let mut host = Host::default();
    assert_eq!(call(&mut host, "reset", json!({}))["ok"], true);
    host
}

#[test]
fn json_validate_is_a_readonly_preclaim_check_and_invite_epoch_cannot_enter_the_native_api() {
    let mut host = ready();
    let command = json!({"type":"addMember","name":"Peer"});
    let row = call(&mut host, "reserve", json!({"command":command}));
    let before = call(&mut host, "inspect", json!({}));
    for _ in 0..3 {
        let validated = call(
            &mut host,
            "validate",
            json!({"token":row["token"],"command":command}),
        );
        assert_eq!(validated["ok"], true);
        assert_eq!(validated["pending"], before["pending"]);
        assert_eq!(validated["released"], json!([]));
    }
    call(
        &mut host,
        "packet",
        json!({"packet":"SwitchGroup","payload":{"allowGroup":true}}),
    );
    assert_eq!(
        call(
            &mut host,
            "validate",
            json!({"token":row["token"],"command":command})
        )["error"],
        "staleReservation"
    );
    assert_eq!(
        call(
            &mut host,
            "enter",
            json!({"token":row["token"],"command":command})
        )["error"],
        "staleReservation"
    );
    assert_eq!(
        call(&mut host, "cancelUnsent", json!({"token":row["token"]}))["ok"],
        true
    );
    let invalid = call(
        &mut host,
        "reserve",
        json!({"command":{"type":"groupInvite","acceptInvite":true},"invite":{"name":"Inviter","epoch":101}}),
    );
    assert_eq!(
        invalid["error"], "invalidInput",
        "UI epoch is not a Native invite counter field"
    );
}

#[test]
fn json_budget_reserve_exact_enter_and_unknown_transport_custody_use_native_completion() {
    let mut host = ready();
    let command = json!({"type":"delMember","name":"Peer"});
    let row = call(&mut host, "reserve", json!({"command":command}));
    assert_eq!(row["kind"], "groupRemove");
    assert_eq!(row["pendingCount"], 1);
    assert_eq!(row["pending"][0]["entered"], false);
    assert_eq!(
        call(
            &mut host,
            "enter",
            json!({"token":row["token"],"command":{"type":"delMember","name":"Other"}})
        )["ok"],
        false
    );
    assert_eq!(
        call(
            &mut host,
            "enter",
            json!({"token":row["token"],"command":command})
        )["ok"],
        true
    );
    assert_eq!(
        call(&mut host, "cancelUnsent", json!({"token":row["token"]}))["error"],
        "alreadyEntered"
    );
    assert_eq!(
        call(&mut host, "expire", json!({"nowMs":90_000}))["pendingCount"],
        1
    );
    assert_eq!(
        call(
            &mut host,
            "packet",
            json!({"packet":"DeleteMember","payload":{"name":"Other"}})
        )["pendingCount"],
        1
    );
    let receipt = call(
        &mut host,
        "packet",
        json!({"packet":"DeleteMember","payload":{"name":"Peer"}}),
    );
    assert_eq!(receipt["pendingCount"], 0);
    assert_eq!(receipt["released"], json!([row["token"]]));
    assert_eq!(
        call(
            &mut host,
            "packet",
            json!({"packet":"DeleteMember","payload":{"name":"Peer"}})
        )["released"],
        json!([])
    );
}

#[test]
fn json_reserved_reentrant_reply_does_not_become_a_sent_request_and_can_refund() {
    let mut host = ready();
    let command = json!({"type":"switchGroup","allowGroup":true});
    let row = call(&mut host, "reserve", json!({"command":command}));
    let early = call(
        &mut host,
        "packet",
        json!({"packet":"SwitchGroup","payload":{"allowGroup":true}}),
    );
    assert_eq!(early["pendingCount"], 1);
    assert_eq!(early["released"], json!([]));
    assert_eq!(
        call(
            &mut host,
            "enter",
            json!({"token":row["token"],"command":command})
        )["error"],
        "staleReservation"
    );
    assert_eq!(
        call(&mut host, "cancelUnsent", json!({"token":row["token"]}))["pendingCount"],
        0
    );
}

#[test]
fn json_non_social_domains_and_native_nonpending_paths_remain_unhandled_without_allocating() {
    let mut host = ready();
    for command in [
        json!({"type":"refreshFriends"}),
        json!({"type":"marriageReply","acceptInvite":true}),
        json!({"type":"sendMail","gold":20}),
        json!({"type":"getRanking","typeId":0}),
        json!({"type":"groupInvite","acceptInvite":false}),
        json!({"type":"guildInvite","acceptInvite":false}),
        json!({"type":"guildStorageItemChange","changeType":3,"from":0,"to":0}),
    ] {
        let result = call(&mut host, "reserve", json!({"command":command}));
        assert_eq!(result["ok"], true);
        assert_eq!(result["handled"], false);
        assert_eq!(result["token"], Value::Null);
        assert_eq!(result["pendingCount"], 0);
    }
}

#[test]
fn json_owner_version_fields_and_input_size_fail_closed_without_erasing_pending() {
    let mut host = ready();
    call(
        &mut host,
        "reserve",
        json!({"command":{"type":"tradeCancel"}}),
    );
    for input in [
        json!({"version":2,"operation":"inspect","owner":owner()}),
        json!({"version":1,"operation":"inspect","owner":owner(),"extra":true}),
        json!({"version":1,"operation":"enter","owner":owner(),"token":1,"command":{"type":"tradeCancel"},"authority":true}),
        json!({"version":1,"operation":"reserve","owner":owner(),"command":{"type":"tradeCancel","extra":1}}),
    ] {
        let result: Value = serde_json::from_str(&host.process(&input.to_string())).unwrap();
        assert_eq!(result["ok"], false);
        assert_eq!(result["pendingCount"], 1);
    }
    let duplicated = r#"{"version":1,"version":1,"operation":"inspect","owner":{"generation":1,"connectionGeneration":1,"sessionGeneration":2,"playerObjectId":1000,"sceneRevision":3,"mapFileName":"0"}}"#;
    assert_eq!(
        serde_json::from_str::<Value>(&host.process(duplicated)).unwrap()["error"],
        "invalidInput"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&host.process(&"x".repeat(MAX_INPUT + 1))).unwrap()["error"],
        "inputTooLarge"
    );
    for (key, value) in [
        ("generation", json!(0)),
        ("sessionGeneration", json!(9_007_199_254_740_992u64)),
        ("playerObjectId", json!(0)),
        ("sceneRevision", json!(-1)),
        ("mapFileName", json!("0\n")),
    ] {
        let mut invalid = owner();
        invalid[key] = value;
        let input = json!({"version":1,"operation":"reset","owner":invalid});
        let result: Value = serde_json::from_str(&host.process(&input.to_string())).unwrap();
        assert_eq!(result["ok"], false);
        assert_eq!(result["pendingCount"], 1);
    }
}

#[test]
fn json_set_viewer_uses_native_user_information_identity_and_true_session_retirement() {
    let mut host = ready();
    assert_eq!(
        call(
            &mut host,
            "setViewer",
            json!({"payload":{"name":"Leader","objectId":7}})
        )["handled"],
        false,
        "objectId cannot stand in for realId"
    );
    assert_eq!(
        call(
            &mut host,
            "setViewer",
            json!({"payload":{"name":"Leader","realId":7}})
        )["handled"],
        true
    );
    call(
        &mut host,
        "reserve",
        json!({"command":{"type":"addMember","name":"Peer"}}),
    );
    let changed = call(
        &mut host,
        "setViewer",
        json!({"payload":{"name":"Other","realId":8}}),
    );
    assert_eq!(
        changed["pendingCount"], 0,
        "Native actual identity change retires even reserved work"
    );
    let row = call(
        &mut host,
        "reserve",
        json!({"command":{"type":"tradeCancel"}}),
    );
    assert_eq!(
        call(&mut host, "retire", json!({}))["released"],
        json!([row["token"]])
    );
    assert_eq!(call(&mut host, "reset", json!({}))["error"], "retiredOwner");
}

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../web/scripts/fixtures/social-budget-command-mapping.json"
    ))
    .unwrap()
}
fn seeded(fixture: &Value, seed: &str) -> Host {
    assert_eq!(fixture["bridgeVersion"], 1);
    assert_eq!(fixture["owner"], owner());
    let mut host = ready();
    for step in fixture["seedSequences"][seed]
        .as_array()
        .expect("known seed")
    {
        let mut input = step.clone();
        let operation = input.as_object_mut().unwrap().remove("operation").unwrap();
        let result = call(&mut host, operation.as_str().unwrap(), input);
        assert_eq!(result["ok"], true, "seed {seed}");
        assert_eq!(result["handled"], true, "seed {seed}");
    }
    host
}
fn partial(actual: &Value, expected: &Value, id: &str) {
    if let Some(fields) = expected.as_object() {
        for (key, value) in fields {
            partial(&actual[key], value, id);
        }
    } else {
        assert_eq!(actual, expected, "actual Native semantic field for {id}");
    }
}

#[test]
fn independent_wire_fixture_maps_43_cases_to_native_builders_without_optimistic_model_changes() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 43);
    let mut counts = [0; 3];
    let mut kinds = std::collections::BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let mut host = seeded(&fixture, case["seed"].as_str().unwrap());
        let before = host.budget.model().clone();
        let mut input = json!({"command":case["command"]});
        for key in ["invite", "deadlineMs"] {
            if let Some(value) = case.get(key) {
                // Independent UI proof epochs remain outside the Native bridge.
                input[key] = if key == "invite" {
                    json!({"name":value["name"]})
                } else {
                    value.clone()
                };
            }
        }
        let result = call(&mut host, "reserve", input);
        let expected = &case["expected"];
        match expected["classification"].as_str().unwrap() {
            "budgeted" => {
                counts[0] += 1;
                assert_eq!(result["ok"], true, "{id}: {result}");
                assert_eq!(result["handled"], true, "{id}");
                assert_eq!(result["pendingCount"], 1, "{id}");
                assert_eq!(result["kind"], expected["nativeKind"], "{id}");
                kinds.insert(result["kind"].as_str().unwrap().to_owned());
                let operation = host
                    .budget
                    .operation_for_token(result["token"].as_u64().unwrap())
                    .unwrap();
                let native = serde_json::to_value(operation).unwrap();
                assert_eq!(native["kind"], expected["nativeKind"], "{id}");
                if let Some(fields) = expected.get("semanticFields") {
                    partial(&native["value"], fields, id);
                }
                assert_eq!(
                    host.budget.model().group,
                    before.group,
                    "{id}: no optimistic group change"
                );
                assert_eq!(
                    host.budget.model().guild,
                    before.guild,
                    "{id}: no optimistic guild change"
                );
                assert_eq!(
                    host.budget.model().trade,
                    before.trade,
                    "{id}: no optimistic trade change"
                );
            }
            "outsideBudget" => {
                counts[1] += 1;
                assert_eq!(result["ok"], true, "{id}");
                assert_eq!(result["handled"], false, "{id}");
                assert_eq!(result["pendingCount"], 0, "{id}");
            }
            "invalid" => {
                counts[2] += 1;
                assert_eq!(result["ok"], false, "{id}");
                assert_eq!(result["pendingCount"], 0, "{id}");
            }
            _ => panic!("unrecognized classification"),
        }
    }
    assert_eq!(counts, [32, 6, 5]);
    assert_eq!(
        kinds.len(),
        19,
        "the legacy GuildInfo alias does not allocate a twentieth wire operation"
    );
    assert_eq!(fixture["nativeVariants"].as_array().unwrap().len(), 20);
    assert_eq!(fixture["aliases"][0]["createdVariant"], "GuildRosterRead");
}

#[test]
fn independent_own_offer_fixture_uses_physical_bag_slot_exact_uid_and_four_negative_controls() {
    let fixture = fixture();
    let data = &fixture["snapshotFixtures"][0];
    let seed = data["seed"].as_str().unwrap();
    let mut host = seeded(&fixture, seed);
    let input = json!({"snapshot":data["snapshot"],"inventoryJson":data["inventoryJson"]});
    assert_eq!(
        call(&mut host, "snapshot", input.clone())["handled"],
        data["expected"]["changedFirstTime"]
    );
    let model = host.budget.model();
    let expected = &data["expected"];
    assert_eq!(
        model.trade.my_gold,
        u32::try_from(expected["myGold"].as_u64().unwrap()).unwrap()
    );
    assert_eq!(
        model.last_event.as_ref().unwrap().packet,
        expected["event"].as_str().unwrap()
    );
    let index = expected["tradeSlot"].as_u64().unwrap() as usize;
    assert_eq!(
        model.trade.my_items[index].as_ref().unwrap().unique_id,
        expected["uniqueId"].as_u64()
    );
    assert_eq!(
        call(&mut host, "snapshot", input)["handled"],
        expected["changedOnIdenticalRepeat"]
    );
    let controls = data["negativeControls"].as_array().unwrap();
    assert_eq!(controls.len(), 4);
    for control in controls {
        let mut host = seeded(&fixture, seed);
        let mut wrong = data.clone();
        *wrong
            .pointer_mut(control["replace"]["pointer"].as_str().unwrap())
            .unwrap() = control["replace"]["value"].clone();
        let result = call(
            &mut host,
            "snapshot",
            json!({"snapshot":wrong["snapshot"],"inventoryJson":wrong["inventoryJson"]}),
        );
        assert_eq!(result["ok"], true);
        assert_eq!(
            result["handled"], control["expectedChanged"],
            "{}",
            control["id"]
        );
        assert_eq!(
            host.budget.model().trade.my_offer_nonce,
            None,
            "rejected source does not partially mutate Native offer"
        );
    }
}

#[test]
fn json_one_eight_slot_budget_is_shared_by_group_guild_and_trade_reserved_and_entered() {
    let fixture = fixture();
    let mut host = seeded(&fixture, "guild");
    call(
        &mut host,
        "packet",
        json!({"packet":"TradeAccept","payload":{"name":"Guest"}}),
    );
    let commands = [
        json!({"type":"switchGroup","allowGroup":true}),
        json!({"type":"addMember","name":"New"}),
        json!({"type":"requestGuildInfo","infoType":1}),
        json!({"type":"editGuildNotice","notice":["New notice"]}),
        json!({"type":"guildStorageGoldChange","changeType":0,"amount":10}),
        json!({"type":"guildStorageItemChange","changeType":2,"from":1,"to":2}),
        json!({"type":"tradeCancel"}),
        json!({"type":"depositTradeItem","from":42,"to":3}),
    ];
    let mut tokens = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        let row = call(&mut host, "reserve", json!({"command":command}));
        assert_eq!(row["ok"], true, "{command}");
        assert_eq!(row["pendingCount"], index + 1);
        tokens.push(row["token"].clone());
        if index % 2 == 0 {
            assert_eq!(
                call(
                    &mut host,
                    "enter",
                    json!({"token":row["token"],"command":command})
                )["ok"],
                true
            );
        }
    }
    let ninth = json!({"type":"tradeGold","amount":5});
    let full = call(&mut host, "reserve", json!({"command":ninth}));
    assert_eq!(full["error"], "budgetUnavailable");
    assert_eq!(full["pendingCount"], 8);
    assert_eq!(
        call(&mut host, "cancelUnsent", json!({"token":tokens[0]}))["error"],
        "alreadyEntered"
    );
    assert_eq!(
        call(&mut host, "cancelUnsent", json!({"token":tokens[1]}))["pendingCount"],
        7
    );
    assert_eq!(
        call(&mut host, "reserve", json!({"command":ninth}))["pendingCount"],
        8
    );
    assert_eq!(
        call(&mut host, "expire", json!({"nowMs":9_000_000}))["pendingCount"],
        8,
        "none of these eight kinds has a generic timeout"
    );
}
