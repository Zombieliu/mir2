//! Strict material-owner transport checks. These fixtures authorize no assets.
use super::*;
use serde_json::json;

fn id() -> String {
    "ab".repeat(32)
}
fn quote() -> Quote {
    Quote {
        quote_id: id(),
        unique_id: U64::new(u64::MAX),
        item_index: 1001,
        input_count: U64::new(10),
        quantity: U64::new(3),
        unit_gold: U64::new(101),
        gold_equivalent: U64::new(303),
        fee_unbound_gold: U64::new(7),
        license_atoms_debit: U64::new(303),
        valuation_version: "server-material-prices-v1".into(),
        created_at_ms: 1000,
        expires_at_ms: 31_000,
    }
}
fn receipt() -> Receipt {
    let q = quote();
    Receipt {
        quote_id: q.quote_id,
        input_unique_id: q.unique_id,
        output_unique_id: U64::new(u64::MAX - 1),
        input_count: q.input_count,
        quantity: q.quantity,
        gold_equivalent: q.gold_equivalent,
        fee_unbound_gold: q.fee_unbound_gold,
        license_atoms_debit: q.license_atoms_debit,
        valuation_version: q.valuation_version,
        operation_key: "cd".repeat(32),
        request_digest: format!("sha256:{}", "ef".repeat(32)),
        business_day: 20_000,
        committed_at_ms: 2000,
    }
}
fn query() -> ClientRequest {
    ClientRequest::new(U64::new(19), Action::Query { quote_id: id() }).unwrap()
}
fn quote_request() -> ClientRequest {
    let q = quote();
    ClientRequest::new(
        U64::new(17),
        Action::Quote {
            unique_id: q.unique_id,
            quantity: q.quantity,
        },
    )
    .unwrap()
}
fn committed(reload_required: bool, snapshot: Option<Value>) -> ServerFrame {
    ServerFrame::new(
        &query(),
        Reply::Committed {
            receipt: receipt(),
            replayed: true,
            reload_required,
        },
        snapshot,
    )
    .unwrap()
}

#[test]
fn material_request_preserves_full_width_decimal_ids_and_bounds_quantities() {
    let request = ClientRequest::new(
        U64::new(u64::MAX),
        Action::Quote {
            unique_id: U64::new(u64::MAX),
            quantity: U64::new(u64::from(u32::MAX)),
        },
    )
    .unwrap();
    let encoded = encode_client_request(&request).unwrap();
    assert!(encoded.contains("\"uniqueId\":\"18446744073709551615\""));
    assert_eq!(parse_client_request(&encoded).unwrap(), request);
    for (field, value) in [
        ("uniqueId", json!(0)),
        ("uniqueId", json!(u64::MAX)),
        ("uniqueId", json!("0")),
        ("uniqueId", json!("01")),
        ("uniqueId", json!("18446744073709551616")),
        ("uniqueId", json!("-1")),
        ("quantity", json!(3)),
        ("quantity", json!("0")),
        ("quantity", json!("03")),
        ("quantity", json!("4294967296")),
    ] {
        let mut raw = serde_json::to_value(&request).unwrap();
        raw["action"][field] = value;
        assert!(
            parse_client_request(&raw.to_string()).is_err(),
            "{field}: {raw}"
        );
    }
    for value in [
        json!("0"),
        json!(19),
        json!("019"),
        json!("18446744073709551616"),
    ] {
        let mut raw = serde_json::to_value(&request).unwrap();
        raw["requestId"] = value;
        assert!(parse_client_request(&raw.to_string()).is_err());
    }
    assert!(ClientRequest::new(U64::new(0), request.action.clone()).is_err());
    assert!(ClientRequest::new(
        U64::new(1),
        Action::Quote {
            unique_id: U64::new(0),
            quantity: U64::new(1),
        }
    )
    .is_err());
}

#[test]
fn material_request_accepts_intent_only_and_rejects_duplicate_or_foreign_envelopes() {
    let request = quote_request();
    let encoded = encode_client_request(&request).unwrap();
    for field in [
        "owner",
        "accountId",
        "unitGold",
        "goldEquivalent",
        "feeUnboundGold",
        "inputCount",
        "checkpoint",
    ] {
        let mut raw = serde_json::to_value(&request).unwrap();
        raw["action"][field] = json!("client-claim");
        assert!(parse_client_request(&raw.to_string()).is_err(), "{field}");
        let mut raw = serde_json::to_value(&request).unwrap();
        raw[field] = json!("client-claim");
        assert!(
            parse_client_request(&raw.to_string()).is_err(),
            "outer {field}"
        );
    }
    for replacement in [
        "\"quantity\":\"3\",\"quantity\":\"3\"",
        "\"quantity\":\"3\",\"quant\\u0069ty\":\"3\"",
    ] {
        let raw = encoded.replacen("\"quantity\":\"3\"", replacement, 1);
        assert_ne!(raw, encoded);
        assert!(parse_client_request(&raw).is_err());
    }
    let duplicate_request = encoded.replacen(
        "\"requestId\":\"17\"",
        "\"requestId\":\"17\",\"request\\u0049d\":\"17\"",
        1,
    );
    assert_ne!(duplicate_request, encoded);
    assert!(parse_client_request(&duplicate_request).is_err());
    assert!(
        parse_client_request(&encoded.replace("huntingMaterialOwner", "huntingRepairOwner"))
            .is_err()
    );
    let mut raw = serde_json::to_value(request).unwrap();
    raw["protocolVersion"] = json!(2);
    assert!(parse_client_request(&raw.to_string()).is_err());
    for quote_id in ["00".repeat(32), "AB".repeat(32), "ab".repeat(31)] {
        assert!(ClientRequest::new(U64::new(1), Action::Confirm { quote_id }).is_err());
    }
}

#[test]
fn material_quote_validates_total_value_atoms_count_ttl_and_version() {
    let q = quote();
    q.validate().unwrap();
    let frame =
        ServerFrame::new(&quote_request(), Reply::Quoted { quote: q.clone() }, None).unwrap();
    for (field, value) in [
        ("uniqueId", json!("0")),
        ("itemIndex", json!(0)),
        ("inputCount", json!("0")),
        ("inputCount", json!("2")),
        ("inputCount", json!("4294967296")),
        ("quantity", json!("0")),
        ("quantity", json!("11")),
        ("unitGold", json!("0")),
        ("goldEquivalent", json!("302")),
        ("goldEquivalent", json!("9223372036854775808")),
        ("licenseAtomsDebit", json!("302")),
        ("feeUnboundGold", json!("0")),
        ("feeUnboundGold", json!("304")),
        ("createdAtMs", json!(-1)),
        ("createdAtMs", json!(i64::MAX)),
        ("expiresAtMs", json!(30_999)),
        ("expiresAtMs", json!(31_001)),
        ("valuationVersion", json!("")),
        ("valuationVersion", json!(" server-prices")),
        ("valuationVersion", json!("bad\u{0000}version")),
        ("valuationVersion", json!("界".repeat(43))),
    ] {
        let mut raw = serde_json::to_value(&frame).unwrap();
        raw["reply"]["quote"][field] = value;
        assert!(
            parse_server_frame(&raw.to_string()).is_err(),
            "{field}: {raw}"
        );
    }
    let mut boundary = q;
    boundary.input_count = U64::new(1);
    boundary.quantity = U64::new(1);
    boundary.unit_gold = U64::new(i64::MAX as u64);
    boundary.gold_equivalent = boundary.unit_gold;
    boundary.license_atoms_debit = boundary.unit_gold;
    boundary.fee_unbound_gold = U64::new(184_467_440_737_095_517);
    boundary.validate().unwrap();
    boundary.input_count = U64::new(2);
    boundary.quantity = U64::new(2);
    assert!(boundary.validate().is_err());
}

#[test]
fn material_receipt_validates_original_input_and_partial_or_whole_output_identity() {
    let q = quote();
    let r = receipt();
    r.validate().unwrap();
    r.validate_for_quote(&q).unwrap();
    assert_eq!(r.input_unique_id.get(), u64::MAX);
    assert_eq!(r.output_unique_id.get(), u64::MAX - 1);
    for (field, value) in [
        ("inputUniqueId", json!("0")),
        ("outputUniqueId", json!("0")),
        ("outputUniqueId", json!(u64::MAX.to_string())),
        ("inputCount", json!("0")),
        ("inputCount", json!("2")),
        ("inputCount", json!("4294967296")),
        ("quantity", json!("2")),
        ("quantity", json!("0")),
        ("goldEquivalent", json!("9223372036854775808")),
        ("feeUnboundGold", json!("0")),
        ("feeUnboundGold", json!("304")),
        ("licenseAtomsDebit", json!("302")),
        ("operationKey", json!("00".repeat(32))),
        ("operationKey", json!("CD".repeat(32))),
        ("requestDigest", json!("ef".repeat(32))),
        (
            "requestDigest",
            json!(format!("sha256:{}", "EF".repeat(32))),
        ),
        ("committedAtMs", json!(-1)),
    ] {
        let mut raw = serde_json::to_value(committed(false, None)).unwrap();
        raw["reply"]["receipt"][field] = value;
        assert!(
            parse_server_frame(&raw.to_string()).is_err(),
            "{field}: {raw}"
        );
    }
    let mut whole_quote = q;
    whole_quote.quantity = whole_quote.input_count;
    whole_quote.gold_equivalent = U64::new(1010);
    whole_quote.license_atoms_debit = U64::new(1010);
    whole_quote.fee_unbound_gold = U64::new(21);
    let mut whole = r;
    whole.quantity = whole.input_count;
    whole.gold_equivalent = whole_quote.gold_equivalent;
    whole.license_atoms_debit = whole_quote.license_atoms_debit;
    whole.fee_unbound_gold = whole_quote.fee_unbound_gold;
    whole.output_unique_id = whole.input_unique_id;
    whole.validate_for_quote(&whole_quote).unwrap();
    whole.output_unique_id = U64::new(u64::MAX - 1);
    assert!(whole.validate().is_err());
}

#[test]
fn material_receipt_cannot_change_an_immutable_quoted_term() {
    let q = quote();
    let r = receipt();
    r.validate_for_quote(&q).unwrap();
    for (field, value) in [
        ("quoteId", json!("cd".repeat(32))),
        ("inputUniqueId", json!("123")),
        ("inputCount", json!("11")),
        ("quantity", json!("1")),
        ("goldEquivalent", json!("606")),
        ("feeUnboundGold", json!("8")),
        ("valuationVersion", json!("another-official-version")),
    ] {
        let mut raw = serde_json::to_value(&r).unwrap();
        raw[field] = value;
        if field == "goldEquivalent" {
            raw["licenseAtomsDebit"] = json!("606");
        }
        let changed: Receipt = serde_json::from_value(raw).unwrap();
        changed.validate().unwrap();
        assert!(changed.validate_for_quote(&q).is_err(), "{field}");
    }
}

#[test]
fn material_frames_correlate_request_id_action_uid_quantity_and_original_quote() {
    let request = quote_request();
    let quoted = ServerFrame::new(&request, Reply::Quoted { quote: quote() }, None).unwrap();
    quoted.validate_for_request(&request).unwrap();
    for action in [
        Action::Quote {
            unique_id: U64::new(123),
            quantity: U64::new(3),
        },
        Action::Quote {
            unique_id: U64::new(u64::MAX),
            quantity: U64::new(2),
        },
        Action::Query { quote_id: id() },
    ] {
        let other = ClientRequest::new(request.request_id, action).unwrap();
        assert!(quoted.validate_for_request(&other).is_err());
    }
    let mut wrong_request_id = quoted.clone();
    wrong_request_id.request_id = U64::new(18);
    assert!(wrong_request_id.validate_for_request(&request).is_err());
    let frame = committed(false, None);
    for action in [
        Action::Confirm { quote_id: id() },
        Action::Query { quote_id: id() },
    ] {
        frame
            .validate_for_request(&ClientRequest::new(query().request_id, action).unwrap())
            .unwrap();
    }
    let other = ClientRequest::new(
        query().request_id,
        Action::Query {
            quote_id: "cd".repeat(32),
        },
    )
    .unwrap();
    assert!(frame.validate_for_request(&other).is_err());
    assert!(frame.validate_for_request(&quote_request()).is_err());
    let text = encode_server_frame(&frame).unwrap();
    assert_eq!(parse_server_frame(&text).unwrap(), frame);
}

#[test]
fn material_pending_unknown_and_rejections_preserve_original_query_identity() {
    let request = query();
    for reply in [
        Reply::Pending { quote_id: id() },
        Reply::Unknown {
            quote_id: Some(id()),
        },
    ] {
        let frame = ServerFrame::new(&request, reply.clone(), None).unwrap();
        let parsed = parse_server_frame(&encode_server_frame(&frame).unwrap()).unwrap();
        assert_eq!(parsed.reply, reply);
        assert!(matches!(
            parsed.reply,
            Reply::Pending { .. } | Reply::Unknown { .. }
        ));
        assert!(ServerFrame::new(&request, reply, Some(json!({}))).is_err());
    }
    assert!(ServerFrame::new(&request, Reply::Unknown { quote_id: None }, None).is_err());
    assert!(ServerFrame::new(
        &request,
        Reply::Pending {
            quote_id: "cd".repeat(32)
        },
        None
    )
    .is_err());
    ServerFrame::new(&quote_request(), Reply::Unknown { quote_id: None }, None).unwrap();
    assert!(ServerFrame::new(
        &quote_request(),
        Reply::Unknown {
            quote_id: Some(id())
        },
        None
    )
    .is_err());
    for code in [
        Rejection::NotSubmitted,
        Rejection::Unconfigured,
        Rejection::InsufficientPermits,
        Rejection::InsufficientUnboundGold,
    ] {
        let reply = Reply::Rejected {
            quote_id: Some(id()),
            code,
        };
        let frame = ServerFrame::new(&request, reply, None).unwrap();
        assert_eq!(
            parse_server_frame(&encode_server_frame(&frame).unwrap()).unwrap(),
            frame
        );
    }
}

#[test]
fn material_snapshot_requires_explicit_null_and_only_current_committed_can_attach_data() {
    for reload_required in [false, true] {
        let frame = committed(reload_required, None);
        let encoded = encode_server_frame(&frame).unwrap();
        assert!(encoded.contains("\"snapshot\":null"));
        assert_eq!(parse_server_frame(&encoded).unwrap(), frame);
        let mut missing = serde_json::to_value(frame).unwrap();
        missing.as_object_mut().unwrap().remove("snapshot");
        assert!(parse_server_frame(&missing.to_string()).is_err());
    }
    // Snapshot contents are opaque public data, not source authorization.
    let snapshot = json!({"inventory":[{"uniqueId":u64::MAX.to_string()}],"publicValues":[null,true,-1,"英雄"]});
    let current = committed(false, Some(snapshot.clone()));
    assert_eq!(
        parse_server_frame(&encode_server_frame(&current).unwrap())
            .unwrap()
            .snapshot,
        Some(snapshot)
    );
    let mut retired = current.clone();
    if let Reply::Committed {
        reload_required, ..
    } = &mut retired.reply
    {
        *reload_required = true;
    }
    assert!(retired.validate().is_err());
    let mut null_value = current;
    null_value.snapshot = Some(Value::Null);
    assert!(encode_server_frame(&null_value).is_err());
    for reply in [
        Reply::Pending { quote_id: id() },
        Reply::Unknown {
            quote_id: Some(id()),
        },
        Reply::Rejected {
            quote_id: Some(id()),
            code: Rejection::Unavailable,
        },
    ] {
        assert!(ServerFrame::new(&query(), reply, Some(json!({}))).is_err());
    }
    assert!(ServerFrame::new(
        &quote_request(),
        Reply::Quoted { quote: quote() },
        Some(json!({}))
    )
    .is_err());
}

#[test]
fn material_parser_rejects_nested_duplicate_keys_and_asset_checkpoint_injection() {
    let frame = committed(false, None);
    let encoded = encode_server_frame(&frame).unwrap();
    for replacement in [
        "\"snapshot\":null,\"snapshot\":null",
        "\"snapshot\":null,\"snapsh\\u006ft\":{}",
        "\"snapshot\":{\"items\":[{\"uid\":1,\"uid\":2}]}",
        "\"snapshot\":{\"items\":[{\"uid\":1,\"u\\u0069d\":2}]}",
    ] {
        let raw = encoded.replacen("\"snapshot\":null", replacement, 1);
        assert_ne!(raw, encoded);
        assert!(parse_server_frame(&raw).is_err());
    }
    let duplicate_receipt = encoded.replacen(
        "\"quantity\":\"3\"",
        "\"quantity\":\"3\",\"quant\\u0069ty\":\"3\"",
        1,
    );
    assert_ne!(duplicate_receipt, encoded);
    assert!(parse_server_frame(&duplicate_receipt).is_err());
    for field in [
        "checkpoint",
        "walletAfter",
        "unboundGoldCredit",
        "boundGoldDebit",
        "owner",
        "inventory",
    ] {
        for target in ["envelope", "reply", "receipt"] {
            let mut raw = serde_json::to_value(&frame).unwrap();
            match target {
                "envelope" => raw[field] = json!({"forged":true}),
                "reply" => raw["reply"][field] = json!({"forged":true}),
                _ => raw["reply"]["receipt"][field] = json!({"forged":true}),
            }
            assert!(
                parse_server_frame(&raw.to_string()).is_err(),
                "{target} {field}"
            );
        }
    }
}

#[test]
fn material_request_and_opaque_snapshot_frames_enforce_byte_limits() {
    assert_eq!(MAX_REQUEST_BYTES, 4096);
    assert_eq!(MAX_FRAME_BYTES, 2 * 1024 * 1024);
    assert_eq!(MAX_FRAME_BYTES, super::super::MAX_SERVER_FRAME_BYTES);
    let request = quote_request();
    let encoded = encode_client_request(&request).unwrap();
    let exact = format!(
        "{}{}",
        " ".repeat(MAX_REQUEST_BYTES - encoded.len()),
        encoded
    );
    assert_eq!(exact.len(), MAX_REQUEST_BYTES);
    assert_eq!(parse_client_request(&exact).unwrap(), request);
    assert!(parse_client_request(&format!(" {exact}")).is_err());
    let frame = committed(false, Some(json!({"publicData":"a".repeat(32 * 1024)})));
    let encoded = encode_server_frame(&frame).unwrap();
    assert!(encoded.len() > 16_384);
    assert_eq!(parse_server_frame(&encoded).unwrap(), frame);
    let exact = format!("{}{}", " ".repeat(MAX_FRAME_BYTES - encoded.len()), encoded);
    assert_eq!(exact.len(), MAX_FRAME_BYTES);
    assert_eq!(parse_server_frame(&exact).unwrap(), frame);
    assert!(parse_server_frame(&format!(" {exact}")).is_err());
    let oversized = committed(
        false,
        Some(json!({"publicData":"a".repeat(MAX_FRAME_BYTES)})),
    );
    assert!(oversized.validate().is_ok());
    assert!(encode_server_frame(&oversized).is_err());
    assert!(parse_server_frame(&serde_json::to_string(&oversized).unwrap()).is_err());
}
