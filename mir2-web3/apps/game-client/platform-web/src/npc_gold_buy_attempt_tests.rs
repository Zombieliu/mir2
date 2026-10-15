use super::*;
use serde_json::json;

fn call(bridge: &mut NpcGoldBuyAttemptBridge, value: Value) -> Value {
    serde_json::from_str(&bridge.transact(&value.to_string())).unwrap()
}
fn status(bridge: &mut NpcGoldBuyAttemptBridge) -> Value { call(bridge, json!({"op":"status"})) }
fn ready() -> NpcGoldBuyAttemptBridge { ready_at("1") }
fn ready_at(connection: &str) -> NpcGoldBuyAttemptBridge {
    let mut bridge = NpcGoldBuyAttemptBridge::new();
    assert_eq!(call(&mut bridge, json!({"op":"connection","run":"1","connection":connection}))["matched"], true);
    assert_eq!(call(&mut bridge, json!({"op":"observe","authority":"owner/service/catalog/full-inventory-A"}))["matched"], true);
    assert_eq!(call(&mut bridge, json!({"op":"availability","available":true}))["matched"], true);
    bridge
}
fn wire(index: u64, count: u16) -> String {
    json!({"type":"buyItem","itemIndex":index,"count":count,"panelType":0}).to_string()
}
fn web_ticket(transport: &str, count: u16) -> Value {
    json!({"transport":transport,"body":wire(0, count)})
}
fn reserve(bridge: &mut NpcGoldBuyAttemptBridge) -> String {
    let response = call(bridge, json!({"op":"reserve"}));
    assert_eq!(response["matched"], true);
    response["state"]["flight"]["token"].as_str().unwrap().to_owned()
}
fn bound(bridge: &mut NpcGoldBuyAttemptBridge, ticket: &Value) -> String {
    let token = reserve(bridge);
    assert_eq!(call(bridge, json!({"op":"bind","token":token,"ticket":ticket}))["matched"], true);
    token
}
fn proof(op: &str, token: &str, ticket: &Value) -> Value {
    json!({"op":op,"token":token,"ticket":ticket})
}
fn receipt(token: &str, ticket: &Value, outcome: &str) -> Value {
    json!({"op":"receipt","token":token,"ticket":ticket,"outcome":outcome})
}
fn refused_without_mutation(bridge: &mut NpcGoldBuyAttemptBridge, input: &str) {
    let before = status(bridge);
    let result: Value = serde_json::from_str(&bridge.transact(input)).unwrap();
    assert_eq!(result, json!({"ok":false,"error":"Invalid NPC gold-buy attempt request"}));
    assert_eq!(status(bridge), before, "malformed request mutated state: {input}");
}

#[test]
fn independent_abi_and_exact_compact_initial_and_reserved_response() {
    assert_eq!(npc_gold_buy_attempt_abi_version(), 1);
    let mut bridge = NpcGoldBuyAttemptBridge::new();
    assert_eq!(status(&mut bridge), json!({"ok":true,"matched":true,"state":{
        "authorityRevision":"0","canReserve":false,"flight":null,"lastPhase":null}}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    call(&mut bridge,json!({"op":"connection","run":"1","connection":"1"}));
    call(&mut bridge,json!({"op":"availability","available":true}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    call(&mut bridge,json!({"op":"observe","authority":"A"}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"})),json!({"ok":true,"matched":true,"state":{
        "authorityRevision":"1","canReserve":false,"flight":{
            "token":"1","authorityRevision":"1","ticket":null,"phase":"queued"},"lastPhase":"queued"}}));
}

#[test]
fn canonical_u64_transport_and_safe_zero_or_max_catalog_identity_are_preserved() {
    let mut bridge = ready_at("18446744073709551615");
    let ticket = json!({"transport":"18446744073709551615","body":wire(SAFE_ITEM_INDEX,u16::MAX)});
    let token = bound(&mut bridge,&ticket);
    assert_eq!(status(&mut bridge)["state"]["flight"]["ticket"],ticket);
    let before = status(&mut bridge);
    assert_eq!(call(&mut bridge,proof("allows",&token,&ticket))["matched"],true);
    assert_eq!(status(&mut bridge),before, "allows must not enter");
    assert_eq!(call(&mut bridge,proof("allows","9007199254740993",&ticket))["matched"],false);
    assert_eq!(call(&mut bridge,proof("allows","18446744073709551615",&ticket))["matched"],false);
    assert_eq!(status(&mut bridge),before);
    let mut zero = ready();
    let zero_token = bound(&mut zero,&web_ticket("1",1));
    assert_eq!(call(&mut zero,proof("allows",&zero_token,&web_ticket("1",1)))["matched"],true);
}

#[test]
fn malformed_op_shapes_and_duplicate_members_never_mutate_a_bound_slot() {
    let mut bridge=ready(); let ticket=web_ticket("1",1); bound(&mut bridge,&ticket);
    for input in ["{}", "[]", "null", r#"{"op":"status","extra":0}"#,
        r#"{"op":"status","op":"reserve"}"#, r#"{"op":"status","\u006fp":"reserve"}"#,
        r#"{"op":"observe"}"#, r#"{"op":"observe","authority":null}"#,
        r#"{"op":"observe","authority":""}"#, r#"{"op":"availability","available":1}"#,
        r#"{"op":"availability","available":false,"extra":0}"#,
        r#"{"op":"unknown"}"#, r#"{"op":"status"}{}"#] {
        refused_without_mutation(&mut bridge,input);
    }
    for op in ["bind","allows","enter","receipt","rejectUnpublished"] {
        refused_without_mutation(&mut bridge,&json!({"op":op}).to_string());
    }
}

#[test]
fn tokens_and_transports_reject_numeric_noncanonical_and_overflow_values() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
    for bad in [json!(1),json!(null),json!("0"),json!("01"),json!("+1"),json!("-1"),
        json!(" 1"),json!("1.0"),json!("18446744073709551616"),json!("١")] {
        refused_without_mutation(&mut bridge,&json!({"op":"enter","token":bad,"ticket":ticket}).to_string());
        let mut wrong=ticket.clone();wrong["transport"]=bad;
        refused_without_mutation(&mut bridge,&proof("allows",&token,&wrong).to_string());
    }
}

#[test]
fn body_requires_exact_buy_tuple_and_rejects_nested_duplicate_or_incomplete_ticket() {
    let mut bridge=ready();let token=reserve(&mut bridge);
    for body in [r#"{"type":"buyItem","itemIndex":0,"count":1,"panelType":0,"extra":0}"#,
        r#"{"type":"buyItem","itemIndex":0,"count":1,"panelType":0,"count":2}"#,
        r#"{"type":"buyItem","itemIndex":0,"count":1,"panelType":0,"\u0063ount":2}"#,
        r#"{"type":"buyItem","itemIndex":0,"panelType":0}"#,
        r#"{"type":"sellItem","itemIndex":0,"count":1,"panelType":0}"#,
        r#"{"type":"buyItem","itemIndex":0,"count":0,"panelType":0}"#,
        r#"{"type":"buyItem","itemIndex":0,"count":65536,"panelType":0}"#,
        r#"{"type":"buyItem","itemIndex":9007199254740992,"count":1,"panelType":0}"#,
        r#"{"type":"buyItem","itemIndex":-1,"count":1,"panelType":0}"#,
        r#"{"type":"buyItem","itemIndex":0,"count":1.0,"panelType":0}"#,
        r#"{"type":"buyItem","itemIndex":0,"count":1,"panelType":1}"#] {
        refused_without_mutation(&mut bridge,&proof("bind",&token,&json!({"transport":"1","body":body})).to_string());
    }
    for ticket in [json!({"transport":"1"}),json!({"body":wire(0,1)}),
        json!({"transport":"1","body":wire(0,1),"extra":0})] {
        refused_without_mutation(&mut bridge,&proof("bind",&token,&ticket).to_string());
    }
    let input=format!(r#"{{"op":"bind","token":"{token}","ticket":{{"transport":"1","transport":"2","body":{}}}}}"#,json!(wire(0,1)));
    refused_without_mutation(&mut bridge,&input);
}

#[test]
fn frozen_body_bytes_and_transport_are_both_exact_final_ticket_identity() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
    let before=status(&mut bridge);
    let differently_spelled=json!({"transport":"1","body":" {\"type\":\"buyItem\",\"itemIndex\":0,\"count\":1,\"panelType\":0}"});
    for wrong in [web_ticket("2",1),web_ticket("1",2),differently_spelled] {
        assert_eq!(call(&mut bridge,proof("allows",&token,&wrong))["matched"],false);
        assert_eq!(call(&mut bridge,proof("enter",&token,&wrong))["matched"],false);
        assert_eq!(call(&mut bridge,receipt(&token,&wrong,"definitelyUnsent"))["matched"],false);
        assert_eq!(status(&mut bridge),before);
    }
}

#[test]
fn reject_unpublished_is_only_queued_and_tokens_are_not_reused() {
    let mut bridge=ready();let first=reserve(&mut bridge);
    assert_eq!(call(&mut bridge,json!({"op":"rejectUnpublished","token":first}))["matched"],true);
    assert!(status(&mut bridge)["state"]["flight"].is_null());
    assert_eq!(status(&mut bridge)["state"]["lastPhase"],"definitelyUnsent");
    let second=bound(&mut bridge,&web_ticket("1",2));assert_ne!(first,second);
    assert_eq!(call(&mut bridge,json!({"op":"rejectUnpublished","token":second}))["matched"],false);
    assert_eq!(call(&mut bridge,json!({"op":"rejectUnpublished","token":first}))["matched"],false);
}

#[test]
fn same_authority_different_quantity_is_one_slot_and_status_never_auto_reserves() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    assert_eq!(call(&mut bridge,proof("bind",&token,&web_ticket("1",2)))["matched"],false);
    call(&mut bridge,json!({"op":"observe","authority":"owner/service/catalog/full-inventory-A"}));
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"1");
    assert_eq!(call(&mut bridge,receipt(&token,&ticket,"definitelyUnsent"))["matched"],true);
    for _ in 0..3 { let result=status(&mut bridge);assert_eq!(result["state"]["canReserve"],true);assert!(result["state"]["flight"].is_null()); }
    assert_eq!(reserve(&mut bridge),"2");
}

#[test]
fn close_reopen_cancels_preentry_and_late_old_receipt_does_not_clear_new_attempt() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let old=bound(&mut bridge,&ticket);
    call(&mut bridge,json!({"op":"availability","available":false}));
    assert!(status(&mut bridge)["state"]["flight"].is_null());
    call(&mut bridge,json!({"op":"availability","available":true}));
    let next=bound(&mut bridge,&ticket);let before=status(&mut bridge);
    assert_eq!(call(&mut bridge,receipt(&old,&ticket,"definitelyUnsent"))["matched"],false);
    assert_eq!(call(&mut bridge,proof("enter",&old,&ticket))["matched"],false);
    assert_eq!(status(&mut bridge),before);assert_ne!(old,next);
}

#[test]
fn entered_close_reopen_and_unsent_receipt_cannot_release_transport_barrier() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
    assert_eq!(call(&mut bridge,proof("enter",&token,&ticket))["matched"],true);
    for available in [false,true] {call(&mut bridge,json!({"op":"availability","available":available}));}
    assert_eq!(call(&mut bridge,receipt(&token,&ticket,"definitelyUnsent"))["matched"],false);
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    assert_eq!(call(&mut bridge,receipt(&token,&ticket,"unknown"))["matched"],true);
    let before=status(&mut bridge);
    for outcome in ["unknown","flushed","definitelyUnsent"] {
        assert_eq!(call(&mut bridge,receipt(&token,&ticket,outcome))["matched"],false);
    }
    assert_eq!(status(&mut bridge),before);
}

#[test]
fn flushed_is_transport_only_and_retains_flight_until_real_connection_change() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
    assert_eq!(call(&mut bridge,receipt(&token,&ticket,"flushed"))["matched"],false);
    call(&mut bridge,proof("enter",&token,&ticket));
    assert_eq!(call(&mut bridge,receipt(&token,&ticket,"flushed"))["matched"],true);
    assert_eq!(status(&mut bridge)["state"]["flight"]["phase"],"flushed");
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    call(&mut bridge,json!({"op":"observe","authority":"B"}));
    let held=status(&mut bridge);
    assert_eq!(held["state"]["flight"]["token"],token);
    assert_eq!(held["state"]["flight"]["authorityRevision"],"1");
    assert_eq!(held["state"]["authorityRevision"],"2");
    assert_eq!(held["state"]["lastPhase"],"flushed");
    assert_eq!(held["state"]["canReserve"],false);
    assert_eq!(call(&mut bridge,json!({"op":"connection","run":"1","connection":"2"}))["matched"],true);
    let result=status(&mut bridge);assert_eq!(result["state"]["lastPhase"],"unknown");
    assert!(result["state"]["flight"].is_null());assert_eq!(result["state"]["canReserve"],false);
    assert_eq!(result["state"]["authorityRevision"],"3");
    call(&mut bridge,json!({"op":"availability","available":true}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    call(&mut bridge,json!({"op":"observe","authority":"B"}));
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"4");
    assert_eq!(status(&mut bridge)["state"]["canReserve"],true);
}

#[test]
fn observed_a_b_a_preserves_entered_until_real_connection_without_fabricating_ack() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let old=bound(&mut bridge,&ticket);
    call(&mut bridge,proof("enter",&old,&ticket));
    call(&mut bridge,json!({"op":"observe","authority":"B"}));
    call(&mut bridge,json!({"op":"observe","authority":"owner/service/catalog/full-inventory-A"}));
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"3");
    assert_eq!(status(&mut bridge)["state"]["flight"]["token"],old);
    assert_eq!(status(&mut bridge)["state"]["flight"]["phase"],"entered");
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    assert_eq!(call(&mut bridge,json!({"op":"connection","run":"1","connection":"2"}))["matched"],true);
    call(&mut bridge,json!({"op":"observe","authority":"owner/service/catalog/full-inventory-A"}));
    call(&mut bridge,json!({"op":"availability","available":true}));
    let current=web_ticket("2",1);
    let next=bound(&mut bridge,&current);let before=status(&mut bridge);
    assert_eq!(call(&mut bridge,proof("enter",&old,&ticket))["matched"],false);
    assert_eq!(call(&mut bridge,receipt(&old,&ticket,"flushed"))["matched"],false);
    assert_eq!(status(&mut bridge),before);assert_ne!(old,next);
    assert_eq!(bridge.slot.history().back().unwrap().phase,NpcGoldBuyAttemptPhase::Unknown);
}

#[test]
fn utf8_authority_and_whole_request_caps_reject_without_mutation_and_allow_escaped_limit() {
    let mut bridge=ready();let ticket=web_ticket("1",1);bound(&mut bridge,&ticket);
    refused_without_mutation(&mut bridge,&json!({"op":"observe","authority":"é".repeat(NPC_GOLD_BUY_AUTHORITY_MAX_BYTES/2+1)}).to_string());
    refused_without_mutation(&mut bridge,&" ".repeat(REQUEST_LIMIT+1));
    refused_without_mutation(&mut bridge,&proof("bind","1",&json!({"transport":"1","body":" ".repeat(BODY_LIMIT+1)})).to_string());
    let escaped="\u{0000}".repeat(NPC_GOLD_BUY_AUTHORITY_MAX_BYTES);
    assert_eq!(call(&mut bridge,json!({"op":"observe","authority":escaped}))["matched"],true);
}

#[test]
fn malformed_receipt_outcome_missing_members_or_added_ack_do_not_mutate() {
    let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
    for outcome in [json!(null),json!(1),json!("success"),json!("failure"),json!("entered")] {
        refused_without_mutation(&mut bridge,&json!({"op":"receipt","token":token,"ticket":ticket,"outcome":outcome}).to_string());
    }
    refused_without_mutation(&mut bridge,&proof("receipt",&token,&ticket).to_string());
    refused_without_mutation(&mut bridge,&json!({"op":"ack","token":token,"ticket":ticket,"result":1}).to_string());
    let mut extra=receipt(&token,&ticket,"definitelyUnsent");extra["result"]=json!(1);
    refused_without_mutation(&mut bridge,&extra.to_string());
}

// This is the preserved original response encoder, used only as a byte oracle.
fn original_value_response(bridge: &NpcGoldBuyAttemptBridge, matched: bool) -> String {
    let original_flight = |attempt: &NpcGoldBuyAttempt<WebTicket>| json!({
        "token":attempt.token.value().to_string(),
        "authorityRevision":attempt.authority_revision.to_string(),
        "ticket":attempt.ticket.as_ref().map(|ticket| json!({"transport":ticket.transport,"body":ticket.body})),
        "phase":phase(attempt.phase)});
    let last = bridge.slot.flight().or_else(|| bridge.slot.history().back());
    json!({"ok":true,"matched":matched,"state":{
        "authorityRevision":bridge.slot.authority_revision().to_string(),
        "canReserve":bridge.slot.can_reserve(),"flight":bridge.slot.flight().map(original_flight),
        "lastPhase":last.map(|attempt| phase(attempt.phase))}}).to_string()
}
fn exact_original_bytes(bridge: &mut NpcGoldBuyAttemptBridge, input: &str) -> Value {
    let actual = bridge.transact(input);
    let parsed: Value = serde_json::from_str(&actual).unwrap();
    let expected = if parsed["ok"] == true {
        original_value_response(bridge, parsed["matched"].as_bool().unwrap())
    } else { json!({"ok":false,"error":"Invalid NPC gold-buy attempt request"}).to_string() };
    assert_eq!(actual.as_bytes(),expected.as_bytes(),"output bytes changed for {input}");
    parsed
}

#[test]
fn string_response_preserves_original_value_bytes_across_all_operations_and_phases() {
    let mut bridge=NpcGoldBuyAttemptBridge::new();
    let ticket=web_ticket("18446744073709551615",1);
    let sequence=[
        json!({"op":"status"}),json!({"op":"reserve"}),
        json!({"op":"connection","run":"1","connection":"18446744073709551615"}),
        json!({"op":"observe","authority":"A"}),json!({"op":"availability","available":true}),
        json!({"op":"reserve"}),proof("bind","1",&ticket),
        proof("allows","1",&web_ticket("1",1)),proof("allows","1",&ticket),proof("enter","1",&ticket),
        receipt("1",&ticket,"unknown"),receipt("1",&ticket,"definitelyUnsent"),
        json!({"op":"observe","authority":"B"}),
        json!({"op":"connection","run":"2","connection":"18446744073709551615"}),
        json!({"op":"observe","authority":"B"}),json!({"op":"availability","available":false}),
        json!({"op":"availability","available":true}),json!({"op":"reserve"}),proof("bind","2",&ticket),
        receipt("2",&ticket,"definitelyUnsent"),json!({"op":"reserve"}),
        json!({"op":"rejectUnpublished","token":"3"}),json!({"op":"observe","authority":"B"}),
        json!({"op":"reserve"}),json!({"op":"availability","available":false}),
        json!({"op":"availability","available":true}),json!({"op":"reserve"}),proof("bind","5",&ticket),
        proof("enter","5",&ticket),receipt("5",&ticket,"flushed"),
        json!({"op":"observe","authority":"C"}),json!({"op":"status"}),
    ];
    let mut phases=Vec::new();
    for input in sequence {
        let result=exact_original_bytes(&mut bridge,&input.to_string());
        if let Some(value)=result["state"]["lastPhase"].as_str() { phases.push(value.to_owned()); }
    }
    phases.sort();phases.dedup();
    assert_eq!(phases,vec!["bound","definitelyUnsent","entered","flushed","queued","unknown"]);
    for input in [r#"{"op":"status","op":"reserve"}"#,r#"{"op":"unknown"}"#,
        r#"{"op":"enter","token":"18446744073709551616","ticket":{}}"#] {
        exact_original_bytes(&mut bridge,input);
    }
}

#[test]
fn string_value_encoding_matches_serde_value_for_control_and_unicode_metacharacters() {
    for value in ["", "\u{0000}\u{0001}\u{0008}\u{000c}\n\r\t", "quote\"slash\\",
        "中文😀\u{2028}\u{2029}", "{\"type\":\"buyItem\",\"itemIndex\":0,\"count\":1,\"panelType\":0}"] {
        let mut output=String::new();quoted(&mut output,value);
        assert_eq!(output.as_bytes(),json!(value).to_string().as_bytes());
        assert_eq!(serde_json::from_str::<String>(&output).unwrap(),value);
    }
}

#[test]
fn accepted_wire_escaping_and_whitespace_remain_exact_bytes_in_every_flight_response() {
    let mut bridge=ready();
    for (index,body) in [
        " {\n\"type\":\"buyItem\",\r\n\"itemIndex\":0,\"count\":1,\"panelType\":0} ",
        r#"{"ty\u0070e":"buy\u0049tem","itemIndex":0,"count":1,"panelType":0}"#,
    ].into_iter().enumerate() {
        exact_original_bytes(&mut bridge,&json!({"op":"connection","run":(index+2).to_string(),"connection":"9007199254740993"}).to_string());
        exact_original_bytes(&mut bridge,&json!({"op":"observe","authority":format!("escaped-{index}")}).to_string());
        exact_original_bytes(&mut bridge,&json!({"op":"availability","available":true}).to_string());
        let response=exact_original_bytes(&mut bridge,&json!({"op":"reserve"}).to_string());
        let token=response["state"]["flight"]["token"].as_str().unwrap();
        let ticket=json!({"transport":"9007199254740993","body":body});
        let bound=exact_original_bytes(&mut bridge,&proof("bind",token,&ticket).to_string());
        assert_eq!(bound["matched"],true);
        assert_eq!(bound["state"]["flight"]["ticket"]["body"],body);
        exact_original_bytes(&mut bridge,&proof("enter",token,&ticket).to_string());
        exact_original_bytes(&mut bridge,&receipt(token,&ticket,"unknown").to_string());
    }
}

#[test]
fn connection_requires_exact_positive_u64_strings_and_duplicate_members_are_rejected() {
    let mut bridge = ready(); let ticket = web_ticket("1", 1); bound(&mut bridge, &ticket);
    for bad in [json!(null), json!(1), json!("0"), json!("01"), json!("+1"), json!("-1"),
        json!("1.0"), json!(" 1"), json!("18446744073709551616"), json!("١")] {
        for field in ["run", "connection"] {
            let mut value = json!({"op":"connection","run":"1","connection":"1"});
            value[field] = bad.clone(); refused_without_mutation(&mut bridge, &value.to_string());
        }
    }
    for raw in [r#"{"op":"connection","run":"1"}"#,
        r#"{"op":"connection","connection":"1"}"#,
        r#"{"op":"connection","run":"1","connection":"1","extra":0}"#,
        r#"{"op":"connection","run":"1","run":"2","connection":"1"}"#,
        r#"{"op":"connection","run":"1","connection":"1","\u0063onnection":"2"}"#] {
        refused_without_mutation(&mut bridge, raw);
    }
}

#[test]
fn observe_and_availability_without_connection_never_grant_a_purchase() {
    let mut bridge = NpcGoldBuyAttemptBridge::new();
    call(&mut bridge, json!({"op":"observe","authority":"A"}));
    call(&mut bridge, json!({"op":"availability","available":true}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    assert_eq!(call(&mut bridge,json!({"op":"connection","run":"1","connection":"1"}))["matched"],true);
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"1");
    assert_eq!(status(&mut bridge)["state"]["canReserve"],false);
    call(&mut bridge,json!({"op":"availability","available":true}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    call(&mut bridge,json!({"op":"observe","authority":"A"}));
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"2");
    let token=reserve(&mut bridge);
    assert_eq!(call(&mut bridge,proof("bind",&token,&web_ticket("2",1)))["matched"],false);
    assert_eq!(status(&mut bridge)["state"]["flight"]["phase"],"queued");
    assert_eq!(call(&mut bridge,proof("bind",&token,&web_ticket("1",1)))["matched"],true);
}

#[test]
fn every_unresolved_phase_survives_authority_aba_and_duplicate_connection() {
    for outcome in [None,Some("flushed"),Some("unknown")] {
        let mut bridge=ready();let ticket=web_ticket("1",1);let token=bound(&mut bridge,&ticket);
        assert_eq!(call(&mut bridge,proof("enter",&token,&ticket))["matched"],true);
        if let Some(outcome)=outcome { assert_eq!(call(&mut bridge,receipt(&token,&ticket,outcome))["matched"],true); }
        let original=status(&mut bridge)["state"]["flight"].clone();
        for authority in ["catalog B","wallet B","character B","owner/service/catalog/full-inventory-A"] {
            assert_eq!(call(&mut bridge,json!({"op":"observe","authority":authority}))["matched"],true);
            call(&mut bridge,json!({"op":"availability","available":false}));
            call(&mut bridge,json!({"op":"availability","available":true}));
            let before=status(&mut bridge);
            assert_eq!(call(&mut bridge,json!({"op":"connection","run":"1","connection":"1"}))["matched"],true);
            assert_eq!(status(&mut bridge),before);
            assert_eq!(status(&mut bridge)["state"]["flight"],original);
            assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
            assert_eq!(call(&mut bridge,receipt(&token,&ticket,"definitelyUnsent"))["matched"],false);
        }
    }
}

#[test]
fn newer_connection_requires_fresh_authority_and_old_pair_or_receipt_cannot_clear_fresh_token() {
    let mut bridge=ready_at("9007199254740993");
    let old_ticket=web_ticket("9007199254740993",1);let old=bound(&mut bridge,&old_ticket);
    call(&mut bridge,proof("enter",&old,&old_ticket));
    assert_eq!(call(&mut bridge,json!({"op":"connection","run":"1","connection":"18446744073709551615"}))["matched"],true);
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"2");
    assert_eq!(status(&mut bridge)["state"]["lastPhase"],"unknown");
    call(&mut bridge,json!({"op":"availability","available":true}));
    assert_eq!(call(&mut bridge,json!({"op":"reserve"}))["matched"],false);
    call(&mut bridge,json!({"op":"observe","authority":"fresh owner/model"}));
    assert_eq!(status(&mut bridge)["state"]["authorityRevision"],"3");
    let current=web_ticket("18446744073709551615",2);let fresh=bound(&mut bridge,&current);
    let before=status(&mut bridge);
    assert_eq!(call(&mut bridge,json!({"op":"connection","run":"1","connection":"9007199254740993"}))["matched"],false);
    assert_eq!(call(&mut bridge,receipt(&old,&old_ticket,"unknown"))["matched"],false);
    assert_eq!(call(&mut bridge,proof("enter",&old,&old_ticket))["matched"],false);
    assert_eq!(status(&mut bridge),before);assert_ne!(old,fresh);
    assert_eq!(call(&mut bridge,proof("enter",&fresh,&current))["matched"],true);
}
