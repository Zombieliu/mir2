#![cfg(feature = "catalog-gzip")]

use std::io::Write;

use flate2::{write::GzEncoder, Compression};
use mir2_protocol::catalog_envelope::decode_validated_catalog_batch;
use mir2_protocol::catalog_transport::{
    encode_catalog_batch, CatalogTransportError, CATALOG_GZIP_MAGIC, MAX_CATALOG_DECODED_BYTES,
    MAX_CATALOG_ENVELOPES, MAX_CATALOG_WIRE_BYTES,
};
use mir2_protocol::{ClientQuestInfo, ClientRecipeInfo, GameShopItem, ItemInfo};
use serde_json::{json, Value};

const HEADER_BYTES: usize = 16;
const INVALID_ENVELOPE: &str = "invalid catalog packet envelope";

// Full shared wire structs, matching the established Native catalog fixtures.
// These must not regress to packet-name-only acceptance tests.
fn fixture_values() -> Vec<Value> {
    let item = json!({
        "index": 7, "name": "药品 Potion", "item_type": 0, "grade": 0,
        "required_type": 0, "required_class": 0, "required_gender": 0,
        "item_set": 0, "shape": 0, "weight": 1, "light": 0,
        "required_amount": 0, "image": 1, "durability": 0, "stack_size": 99,
        "price": 1, "start_item": false, "effect": 0, "need_identify": false,
        "show_group_pickup": false, "class_based": false, "level_based": false,
        "can_mine": false, "global_drop_notify": false, "bind": 0, "unique": 0,
        "random_stats_id": 0, "can_fast_run": false, "can_awakening": false,
        "slots": 0, "stats": [], "tooltip": null
    });
    let user_item = json!({
        "unique_id": 0, "item_index": 7, "current_dura": 0, "max_dura": 0,
        "count": 1, "soul_bound_id": 0, "identified": true, "cursed": false,
        "slots": [], "gem_count": 0, "added_stats": [], "awake_type": 0,
        "awake_values": [], "refined_value": 0, "refine_added": 0,
        "refine_success_chance": 0, "wedding_ring": 0, "expire_info": null,
        "rental_information": null, "is_shop_item": false, "sealed_info": null,
        "gm_made": false
    });
    let quest = json!({
        "index": 3, "npc_index": 1, "name": "骷髅试炼", "group": "",
        "description": ["ASCII and 中文"], "task_description": [],
        "return_description": [], "completion_description": [],
        "min_level_needed": 1, "max_level_needed": 30, "quest_needed": 0,
        "class_needed": 0, "quest_type": 0, "time_limit_in_seconds": 0,
        "reward_gold": 0, "reward_exp": 0, "reward_credit": 0,
        "rewards_fixed_item": [], "rewards_select_item": [], "finish_npc_index": 1
    });
    vec![
        json!({"type": "packet", "packet": "NewItemInfo", "payload": {"info": item}}),
        json!({"type": "packet", "packet": "NewRecipeInfo", "payload": {
            "info": {"gold": 1, "chance": 100, "item": user_item, "tools": [], "ingredients": []}
        }}),
        json!({"type": "packet", "packet": "GameShopInfo", "payload": {
            "item": {"item_index": 7, "g_index": 8, "info": item,
                "gold_price": 1, "credit_price": 0, "count": 1, "class": "Warrior",
                "category": "Potion", "stock": 3, "i_stock": true, "deal": false,
                "top_item": false, "date_binary_datetime": 0,
                "can_buy_credit": false, "can_buy_gold": true},
            "stockLevel": 3
        }}),
        json!({"type": "packet", "packet": "NewQuestInfo", "payload": {
            "info": quest, "id": 3, "name": "骷髅试炼", "minLevelNeeded": 1,
            "descriptionLines": ["ASCII and 中文"]
        }}),
    ]
}

fn fixture_texts() -> Vec<String> {
    fixture_values().iter().map(Value::to_string).collect()
}

fn encoded(texts: &[String]) -> Vec<u8> {
    let references = texts.iter().map(String::as_str).collect::<Vec<_>>();
    encode_catalog_batch(&references).unwrap()
}

fn decoded_body(texts: &[&str]) -> Vec<u8> {
    let mut body = Vec::new();
    for text in texts {
        body.extend_from_slice(&(text.len() as u32).to_le_bytes());
        body.extend_from_slice(text.as_bytes());
    }
    body
}

fn gzip(body: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(body).unwrap();
    encoder.finish().unwrap()
}

// Deliberately bypass the valid encoder to exercise hostile framing separately.
fn raw_frame(body: &[u8], count: u32, declared: u32) -> Vec<u8> {
    let mut frame = CATALOG_GZIP_MAGIC.to_vec();
    frame.extend_from_slice(&declared.to_le_bytes());
    frame.extend_from_slice(&count.to_le_bytes());
    frame.extend_from_slice(&gzip(body));
    frame
}

fn assert_transport_error(bytes: &[u8], expected: CatalogTransportError) {
    assert_eq!(
        decode_validated_catalog_batch(bytes),
        Err(expected.to_string())
    );
}

fn assert_invalid_second(text: &str) {
    let good = fixture_texts().remove(0);
    let frame = encode_catalog_batch(&[good.as_str(), text]).unwrap();
    // This API releases only a complete Vec after every envelope validates.
    // A valid first entry cannot be returned alongside a later semantic failure.
    assert_eq!(
        decode_validated_catalog_batch(&frame),
        Err(INVALID_ENVELOPE.to_owned())
    );
}

#[test]
fn version_one_magic_and_limits_remain_the_shared_wire_contract() {
    assert_eq!(CATALOG_GZIP_MAGIC, *b"M2CATGZ1");
    assert_eq!(MAX_CATALOG_ENVELOPES, 64);
    assert_eq!(MAX_CATALOG_DECODED_BYTES, 128 * 1024);
    assert_eq!(MAX_CATALOG_WIRE_BYTES, 132 * 1024);
}

#[test]
fn full_four_catalog_structs_validate_in_original_order() {
    let values = fixture_values();
    serde_json::from_value::<ItemInfo>(values[0]["payload"]["info"].clone()).unwrap();
    serde_json::from_value::<ClientRecipeInfo>(values[1]["payload"]["info"].clone()).unwrap();
    serde_json::from_value::<GameShopItem>(values[2]["payload"]["item"].clone()).unwrap();
    serde_json::from_value::<ClientQuestInfo>(values[3]["payload"]["info"].clone()).unwrap();
    let texts = fixture_texts();
    assert_eq!(
        decode_validated_catalog_batch(&encoded(&texts)).unwrap(),
        texts
    );
}

#[test]
fn original_strings_preserve_unicode_whitespace_escapes_i64_and_unknown_info_fields() {
    let mut values = fixture_values();
    values[0]["payload"]["info"]["name"] = json!("药品 Potion 🐉\n\\\"\0");
    values[0]["payload"]["info"]["future_field"] =
        json!({"version": 2, "unknown": ["保持原文", true, null]});
    values[1]["payload"]["info"]["future_recipe"] = json!({"kept": true});
    values[2]["payload"]["item"]["date_binary_datetime"] = json!(i64::MAX);
    values[2]["payload"]["item"]["info"]["future_item"] = json!([1, 2, 3]);
    values[3]["payload"]["info"]["future_quest"] = json!({"kept": "✓"});
    let mut texts = values
        .iter()
        .map(|value| format!(" \r\n{}\n\t", serde_json::to_string_pretty(value).unwrap()))
        .collect::<Vec<_>>();
    texts[0] = texts[0].replacen("Potion", r"\u0050otion", 1);
    assert!(texts[0].contains(r"\u0050otion"));
    assert!(texts[2].contains("9223372036854775807"));
    let mut negative_date = values[2].clone();
    negative_date["payload"]["item"]["date_binary_datetime"] = json!(i64::MIN);
    texts.push(format!("\n{}  ", negative_date));
    let result = decode_validated_catalog_batch(&encoded(&texts)).unwrap();
    assert_eq!(result, texts);
    assert!(result
        .iter()
        .zip(&texts)
        .all(|(actual, original)| actual.as_bytes() == original.as_bytes()));
}

#[test]
fn invalid_second_catalog_rejects_whole_batch_without_returning_first() {
    for invalid in [
        r#"{"type":"resumeCredential","packet":"NewItemInfo","payload":{}}"#,
        r#"{"type":"packet","packet":"UserLocation","payload":{"x":99,"y":99}}"#,
        r#"{"type":"packet","packet":"StartGame","payload":{"result":4}}"#,
        r#"{"type":"packet","packet":"NewItemInfo","payload":[]}"#,
        r#"{"type":"packet","packet":"NewItemInfo","payload":{"info":{}}}"#,
        r#"{"type":"packet","packet":"NewRecipeInfo","payload":{"info":{}}}"#,
        r#"{"type":"packet","packet":"NewQuestInfo","payload":{"info":{}}}"#,
        r#"{"type":"packet","packet":"GameShopInfo","payload":{"item":{},"stockLevel":null}}"#,
        "{private-invalid-json",
    ] {
        assert_invalid_second(invalid);
    }
}

#[test]
fn top_level_extra_and_duplicate_fields_are_rejected() {
    let values = fixture_values();
    let payload = &values[0]["payload"];
    for invalid in [
        format!(r#"{{"type":"packet","packet":"NewItemInfo","payload":{payload},"extra":true}}"#),
        format!(
            r#"{{"type":"packet","type":"packet","packet":"NewItemInfo","payload":{payload}}}"#
        ),
        format!(
            r#"{{"type":"packet","\u0074ype":"packet","packet":"NewItemInfo","payload":{payload}}}"#
        ),
        format!(
            r#"{{"type":"packet","packet":"NewItemInfo","packet":"NewItemInfo","payload":{payload}}}"#
        ),
        format!(
            r#"{{"type":"packet","packet":"NewItemInfo","payload":{payload},"payload":{payload}}}"#
        ),
    ] {
        assert_invalid_second(&invalid);
    }
}

#[test]
fn payload_and_info_duplicates_keep_current_value_last_wins_contract() {
    let values = fixture_values();
    let info = values[0]["payload"]["info"].to_string();
    let duplicate_payload = format!(
        r#"{{"type":"packet","packet":"NewItemInfo","payload":{{"info":{{}},"info":{info}}}}}"#
    );
    let duplicate_info = format!(r#"{{"index":"invalid",{}"#, &info[1..]);
    let nested = format!(
        r#"{{"type":"packet","packet":"NewItemInfo","payload":{{"info":{duplicate_info}}}}}"#
    );
    let texts = vec![duplicate_payload, nested];
    assert_eq!(
        decode_validated_catalog_batch(&encoded(&texts)).unwrap(),
        texts
    );

    let bad_payload = format!(
        r#"{{"type":"packet","packet":"NewItemInfo","payload":{{"info":{info},"info":{{}}}}}}"#
    );
    assert_invalid_second(&bad_payload);
    let bad_info = format!(r#"{},"index":"invalid"}}"#, &info[..info.len() - 1]);
    let bad_nested =
        format!(r#"{{"type":"packet","packet":"NewItemInfo","payload":{{"info":{bad_info}}}}}"#);
    assert_invalid_second(&bad_nested);
}

#[test]
fn full_struct_validation_rejects_missing_wrong_typed_and_out_of_range_fields() {
    let values = fixture_values();
    let mut invalid = Vec::new();
    let mut item = values[0].clone();
    item["payload"]["info"]
        .as_object_mut()
        .unwrap()
        .remove("weight");
    invalid.push(item);
    for (field, value) in [
        ("grade", json!(256)),
        ("shape", json!(32768)),
        ("image", json!(65536)),
        ("price", json!(-1)),
        ("start_item", json!(0)),
        ("stats", json!([{"stat": 1, "value": 2147483648_i64}])),
    ] {
        let mut item = values[0].clone();
        item["payload"]["info"][field] = value;
        invalid.push(item);
    }
    let mut extra = values[0].clone();
    extra["payload"]["extra"] = json!(true);
    invalid.push(extra);
    let mut recipe = values[1].clone();
    recipe["payload"]["info"]["chance"] = json!(256);
    invalid.push(recipe);
    let mut recipe = values[1].clone();
    recipe["payload"]["info"]["item"]
        .as_object_mut()
        .unwrap()
        .remove("count");
    invalid.push(recipe);
    for stock in [
        json!(2147483648_i64),
        json!(-2147483649_i64),
        json!(3.0),
        json!("3"),
    ] {
        let mut shop = values[2].clone();
        shop["payload"]["stockLevel"] = stock;
        invalid.push(shop);
    }
    let mut shop = values[2].clone();
    shop["payload"]["item"]["count"] = json!(65536);
    invalid.push(shop);
    for value in invalid {
        assert_invalid_second(&value.to_string());
    }
}

#[test]
fn quest_derived_fields_and_optional_presentation_shapes_match_native_rules() {
    let values = fixture_values();
    let mut valid = values[3].clone();
    for (field, value) in [
        ("group", json!("边境任务")),
        ("timeLimit", json!("00:30")),
        ("returnDescriptionLines", json!(["回城"])),
        ("completionDescriptionLines", json!([])),
        ("objectives", json!([{"label": "训练", "count": 1}])),
        ("rewards", json!({"gold": 1})),
    ] {
        valid["payload"][field] = value;
    }
    let text = valid.to_string();
    assert_eq!(
        decode_validated_catalog_batch(&encoded(&[text.clone()])).unwrap(),
        [text]
    );
    for (field, value) in [
        ("id", json!(999)),
        ("id", json!(3.0)),
        ("name", json!("不匹配")),
        ("minLevelNeeded", json!(2)),
        ("group", json!(1)),
        ("timeLimit", json!(null)),
        ("descriptionLines", json!([1])),
        ("returnDescriptionLines", json!([null])),
        ("completionDescriptionLines", json!({})),
        ("objectives", json!(["not an object"])),
        ("rewards", json!([])),
        ("unknownQuestField", json!(true)),
    ] {
        let mut quest = valid.clone();
        quest["payload"][field] = value;
        assert_invalid_second(&quest.to_string());
    }
}

#[test]
fn exactly_sixty_four_full_catalogs_pass_and_bad_count_headers_fail() {
    let good = fixture_texts().remove(0);
    let texts = vec![good; MAX_CATALOG_ENVELOPES];
    let frame = encoded(&texts);
    assert_eq!(decode_validated_catalog_batch(&frame).unwrap(), texts);
    for count in [0_u32, MAX_CATALOG_ENVELOPES as u32 + 1, u32::MAX] {
        let mut invalid = frame.clone();
        invalid[12..16].copy_from_slice(&count.to_le_bytes());
        assert_transport_error(&invalid, CatalogTransportError::InvalidCount);
    }
}

#[test]
fn exactly_128k_decoded_bytes_pass_and_one_more_is_rejected() {
    let mut text = fixture_texts().remove(0);
    text.push_str(&" ".repeat(MAX_CATALOG_DECODED_BYTES - 4 - text.len()));
    assert_eq!(text.len() + 4, MAX_CATALOG_DECODED_BYTES);
    assert_eq!(
        decode_validated_catalog_batch(&encoded(&[text.clone()])).unwrap(),
        [text.clone()]
    );
    text.push(' ');
    let body = decoded_body(&[text.as_str()]);
    let frame = raw_frame(&body, 1, body.len() as u32);
    assert_transport_error(&frame, CatalogTransportError::DecodedTooLarge);
    for declared in [0_u32, u32::MAX] {
        assert_transport_error(
            &raw_frame(&[0; 5], 1, declared),
            CatalogTransportError::DecodedTooLarge,
        );
    }
}

#[test]
fn exactly_132k_wire_bytes_with_valid_optional_gzip_headers_pass_and_one_more_fails() {
    let text = fixture_texts().remove(0);
    let mut frame = encoded(&[text.clone()]);
    let padding = MAX_CATALOG_WIRE_BYTES - frame.len();
    // Distribute legal, ignored metadata over FEXTRA/FNAME/FCOMMENT so no
    // individual field exceeds flate2's 65,535-byte header-field limit.
    // The separate decoded budget and the body CRC/ISIZE remain unchanged.
    const EXTRA_LEN: u16 = 60_000;
    const FILENAME_LEN: usize = 60_000;
    let comment_bytes = padding - (2 + usize::from(EXTRA_LEN) + FILENAME_LEN + 1);
    assert!((1..=65_536).contains(&comment_bytes));
    assert_eq!(frame[HEADER_BYTES + 3], 0);
    frame[HEADER_BYTES + 3] = 4 | 8 | 16;
    let mut metadata = EXTRA_LEN.to_le_bytes().to_vec();
    metadata.extend(vec![0; usize::from(EXTRA_LEN)]);
    metadata.extend(vec![b'n'; FILENAME_LEN]);
    metadata.push(0);
    metadata.extend(vec![b'c'; comment_bytes - 1]);
    metadata.push(0);
    assert_eq!(metadata.len(), padding);
    frame.splice(HEADER_BYTES + 10..HEADER_BYTES + 10, metadata);
    assert_eq!(frame.len(), MAX_CATALOG_WIRE_BYTES);
    assert_eq!(decode_validated_catalog_batch(&frame).unwrap(), [text]);
    frame.push(0);
    assert_transport_error(&frame, CatalogTransportError::WireTooLarge);
}

#[test]
fn every_truncation_and_both_gzip_trailer_checksums_are_rejected() {
    let frame = encoded(&fixture_texts());
    for cut in 0..frame.len() {
        assert!(
            decode_validated_catalog_batch(&frame[..cut]).is_err(),
            "cut {cut}"
        );
    }
    for trailer_offset in [8, 4] {
        let mut corrupted = frame.clone();
        let at = corrupted.len() - trailer_offset;
        corrupted[at] ^= 1;
        assert_transport_error(&corrupted, CatalogTransportError::InvalidGzip);
    }
}

#[test]
fn concatenated_members_empty_second_member_padding_and_garbage_are_rejected() {
    let frame = encoded(&fixture_texts());
    for suffix in [
        frame[HEADER_BYTES..].to_vec(),
        gzip(&[]),
        vec![0],
        vec![0; 32],
        vec![1, 2, 3],
    ] {
        let mut appended = frame.clone();
        appended.extend_from_slice(&suffix);
        assert_transport_error(&appended, CatalogTransportError::TrailingGzipData);
    }
}

#[test]
fn declared_length_mismatches_and_expansion_bomb_cannot_release_texts() {
    let text = fixture_texts().remove(0);
    let body = decoded_body(&[text.as_str()]);
    for declared in [body.len() as u32 - 1, body.len() as u32 + 1] {
        assert_transport_error(
            &raw_frame(&body, 1, declared),
            CatalogTransportError::DecodedLengthMismatch,
        );
    }
    let bomb = vec![0; MAX_CATALOG_DECODED_BYTES * 16];
    let frame = raw_frame(&bomb, 1, 5);
    assert!(frame.len() < MAX_CATALOG_WIRE_BYTES);
    assert_transport_error(&frame, CatalogTransportError::DecodedLengthMismatch);
}

#[test]
fn entry_boundaries_empty_invalid_utf8_and_trailing_decoded_bytes_are_rejected() {
    for (body, count, expected) in [
        (vec![0, 0, 0, 0], 1, CatalogTransportError::EmptyEnvelope),
        (
            vec![1, 0, 0, 0, 0xff],
            1,
            CatalogTransportError::InvalidUtf8,
        ),
        (
            vec![2, 0, 0, 0, b'a'],
            1,
            CatalogTransportError::InvalidEntryLength,
        ),
        (
            vec![1, 0, 0, 0, b'a'],
            2,
            CatalogTransportError::InvalidEntryLength,
        ),
        (
            vec![0xff, 0xff, 0xff, 0xff, b'a'],
            1,
            CatalogTransportError::InvalidEntryLength,
        ),
    ] {
        assert_transport_error(&raw_frame(&body, count, body.len() as u32), expected);
    }
    let text = fixture_texts().remove(0);
    let mut body = decoded_body(&[text.as_str()]);
    body.push(0);
    assert_transport_error(
        &raw_frame(&body, 1, body.len() as u32),
        CatalogTransportError::TrailingDecodedData,
    );
    // Rust UTF-8 preserves a BOM; the semantic JSON parser must not silently
    // strip it as a default browser TextDecoder would.
    assert_invalid_second(&format!("\u{feff}{text}"));
}

#[test]
fn bad_magic_and_short_outer_headers_are_rejected_without_payload_diagnostics() {
    let mut frame = encoded(&fixture_texts());
    frame[0] ^= 1;
    assert_transport_error(&frame, CatalogTransportError::InvalidMagic);
    for length in 0..HEADER_BYTES {
        assert_transport_error(&frame[..length], CatalogTransportError::InvalidHeader);
    }
    let private = "{PRIVATE_CATALOG_MUST_NOT_APPEAR_IN_ERRORS";
    let good = fixture_texts().remove(0);
    let malformed = encode_catalog_batch(&[good.as_str(), private]).unwrap();
    let error = decode_validated_catalog_batch(&malformed).unwrap_err();
    assert_eq!(error, INVALID_ENVELOPE);
    assert!(!error.contains(private));
}
