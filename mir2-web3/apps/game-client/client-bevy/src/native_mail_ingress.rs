//! Read-only mail projections from frozen Windows 3d735745f.
//! Converts server mailbox and parcel-service fields into existing shared models.
//! It never grants attachments, settles sends/claims, computes postage, or authenticates.
use serde_json::{json, Value};

pub fn mail_source(payload: &Value) -> Option<&Value> {
    payload
        .get("stage5Systems")
        .and_then(|value| value.get("mail"))
        .or_else(|| {
            payload
                .get("stage5_systems")
                .and_then(|value| value.get("mail"))
        })
        .or_else(|| payload.get("mails"))
        .or_else(|| payload.get("mail"))
        .or_else(|| payload.get("stage5").and_then(|value| value.get("mail")))
}

pub fn try_transform_mail_model_from_snapshot(payload: &Value) -> Option<Value> {
    let entries = mail_source(payload)?.as_array()?;
    let visible = entries
        .iter()
        .filter(|mail| {
            !mail
                .get("deleted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    mail_model_from_entries(&visible)
}

pub fn try_transform_mail_model_from_packet(payload: &Value) -> Option<Value> {
    mail_model_from_entries(payload.get("mail")?.as_array()?)
}

fn mail_model_from_entries(entries: &[Value]) -> Option<Value> {
    let mails = entries
        .iter()
        .map(mail_message_json)
        .collect::<Option<Vec<_>>>()?;
    Some(json!({ "mails": mails, "selected_id": Value::Null }))
}

fn mail_message_json(mail: &Value) -> Option<Value> {
    let id = value_u64(
        mail.get("mailId")
            .or_else(|| mail.get("mail_id"))
            .or_else(|| mail.get("id")),
    )?;
    let subject = value_string(mail.get("subject")).unwrap_or_default();
    let body = value_string(mail.get("body")).unwrap_or_default();
    let message = value_string(mail.get("message")).unwrap_or_else(|| {
        if subject.is_empty() {
            body.clone()
        } else if body.is_empty() {
            subject.clone()
        } else {
            format!("{subject}\n{body}")
        }
    });
    let items = mail
        .get("items")?
        .as_array()?
        .iter()
        .map(mail_attachment_json)
        .collect::<Option<Vec<_>>>()?;
    Some(json!({
        "id": id,
        "sender": mail.get("senderName").or_else(|| mail.get("sender")).or_else(|| mail.get("from")).and_then(Value::as_str).unwrap_or("System"),
        "can_reply": mail.get("canReply").or_else(|| mail.get("can_reply")).and_then(Value::as_bool).unwrap_or(false),
        "date_sent_binary_datetime": value_i64(mail.get("dateSentBinaryDatetime").or_else(|| mail.get("date_sent_binary_datetime"))).unwrap_or_default(),
        "metadata_known": mail.get("canReply").or_else(|| mail.get("can_reply")).and_then(Value::as_bool).is_some()
            && value_i64(mail.get("dateSentBinaryDatetime").or_else(|| mail.get("date_sent_binary_datetime"))).is_some(),
        "subject": if subject.is_empty() { message.lines().next().unwrap_or("Mail") } else { &subject },
        "body": message,
        "gold": value_u32(mail.get("gold")).unwrap_or_default(),
        "items": items,
        "claimed": mail.get("collected").or_else(|| mail.get("claimed")).and_then(Value::as_bool).unwrap_or(false),
        "locked": mail.get("locked").and_then(Value::as_bool).unwrap_or(false),
        "read": mail.get("opened").or_else(|| mail.get("read")).and_then(Value::as_bool).unwrap_or(false),
    }))
}

fn mail_attachment_json(item: &Value) -> Option<Value> {
    if let Some(name) = item.as_str().filter(|name| !name.is_empty()) {
        return Some(json!({ "name": name, "count": 1,
            "image": mir2_game_data::crystal_item_by_name(name).map(|template| template.image) }));
    }
    let item_index = value_i32(item.get("itemIndex").or_else(|| item.get("item_index")));
    let name = value_string(item.get("name"));
    let key = value_string(item.get("key"));
    if item_index.is_none() && name.is_none() && key.is_none() {
        return None;
    }
    Some(json!({
        "uniqueId": value_u64(item.get("uniqueId").or_else(|| item.get("unique_id"))),
        "itemIndex": item_index,
        "image": item_index.and_then(mir2_game_data::crystal_item_by_index)
            .or_else(|| name.as_deref().and_then(mir2_game_data::crystal_item_by_name))
            .map(|template| template.image),
        "key": key,
        "name": name,
        "count": value_u32(item.get("count")).and_then(|value| u16::try_from(value).ok()).unwrap_or(1),
        "currentDura": value_u32(item.get("currentDura").or_else(|| item.get("current_dura"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "maxDura": value_u32(item.get("maxDura").or_else(|| item.get("max_dura"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "soulBoundId": value_i32(item.get("soulBoundId").or_else(|| item.get("soul_bound_id"))).unwrap_or_default(),
        "identified": item.get("identified").and_then(Value::as_bool).unwrap_or(false),
        "cursed": item.get("cursed").and_then(Value::as_bool).unwrap_or(false),
        "gemCount": value_u32(item.get("gemCount").or_else(|| item.get("gem_count"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
    }))
}

fn mail_packet_body(payload: &Value) -> &Value {
    payload
        .get("data")
        .filter(|value| value.is_object())
        .unwrap_or(payload)
}

pub fn mail_service_event_from_packet(
    packet: &str,
    payload: &Value,
) -> Option<crate::mail_service::MailServiceEvent> {
    use crate::mail_service::MailServiceEvent;

    let body = mail_packet_body(payload);
    match packet {
        "MailSendRequest" => Some(MailServiceEvent::OpenParcel),
        "MailCost" => Some(MailServiceEvent::Cost {
            cost: value_u32(body.get("cost"))?,
        }),
        "MailLockedItem" => Some(MailServiceEvent::LockedItem {
            unique_id: value_u64(body.get("uniqueId").or_else(|| body.get("unique_id")))?,
            locked: body.get("locked")?.as_bool()?,
        }),
        _ => None,
    }
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}

fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(value_u64_ref)
}

fn value_u64_ref(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::{MailModel, MAX_MAIL_MESSAGES};
    use crate::mail_service::MailServiceEvent;

    fn row(id: u64) -> Value {
        json!({"mailId":id.to_string(),"senderName":"NPC","message":"Subject\nBody",
            "gold":77,"items":[],"canReply":true,"dateSentBinaryDatetime":"638970336000000000",
            "opened":true,"collected":false,"locked":false})
    }

    #[test]
    fn packet_preserves_exact_identity_content_and_metadata() {
        let model =
            try_transform_mail_model_from_packet(&json!({"mail":[row(u64::MAX-1)]})).unwrap();
        let typed: MailModel = serde_json::from_value(model).unwrap();
        let mail = &typed.mails[0];
        assert_eq!(mail.id, u64::MAX - 1);
        assert_eq!(mail.sender, "NPC");
        assert_eq!(mail.subject, "Subject");
        assert_eq!(mail.body, "Subject\nBody");
        assert_eq!(mail.gold, 77);
        assert!(mail.can_reply && mail.metadata_known && mail.read);
        assert!(!mail.claimed && !mail.locked);
        assert_eq!(mail.date_sent_binary_datetime, 638970336000000000);
        assert!(mail.operation.is_none());
    }

    #[test]
    fn all_snapshot_source_aliases_preserve_deleted_filter_and_missing_distinction() {
        for world in [
            json!({"stage5Systems":{"mail":[row(1),{"id":2,"deleted":true}]}}),
            json!({"stage5_systems":{"mail":[row(1),{"id":2,"deleted":true}]}}),
            json!({"mails":[row(1),{"id":2,"deleted":true}]}),
            json!({"mail":[row(1),{"id":2,"deleted":true}]}),
            json!({"stage5":{"mail":[row(1),{"id":2,"deleted":true}]}}),
        ] {
            let typed: MailModel =
                serde_json::from_value(try_transform_mail_model_from_snapshot(&world).unwrap())
                    .unwrap();
            assert_eq!(typed.mails.len(), 1);
            assert_eq!(typed.mails[0].id, 1);
        }
        assert!(try_transform_mail_model_from_snapshot(&json!({})).is_none());
        assert!(try_transform_mail_model_from_snapshot(&json!({"mails":null})).is_none());
        let empty: MailModel = serde_json::from_value(
            try_transform_mail_model_from_snapshot(&json!({"mails":[]})).unwrap(),
        )
        .unwrap();
        assert!(empty.mails.is_empty());
    }

    #[test]
    fn attachment_metadata_and_legacy_display_names_use_existing_content_catalog() {
        let mut mail = row(9);
        mail["items"] = json!([{"uniqueId":"18446744073709551614","itemIndex":1,"key":"fixture",
            "name":"WoodenSword","count":"5","currentDura":100,"maxDura":200,
            "soulBoundId":3,"identified":true,"cursed":true,"gemCount":2},"RedPotion"]);
        let model: MailModel = serde_json::from_value(
            try_transform_mail_model_from_packet(&json!({"mail":[mail]})).unwrap(),
        )
        .unwrap();
        let item = &model.mails[0].items[0];
        assert_eq!(item.unique_id, Some(u64::MAX - 1));
        assert_eq!(item.item_index, Some(1));
        assert_eq!(item.count, 5);
        assert_eq!(item.current_dura, 100);
        assert_eq!(item.max_dura, 200);
        assert_eq!(item.soul_bound_id, 3);
        assert!(item.identified && item.cursed);
        assert_eq!(item.gem_count, 2);
        assert_eq!(
            item.image,
            mir2_game_data::crystal_item_by_index(1)
                .or_else(|| mir2_game_data::crystal_item_by_name("WoodenSword"))
                .map(|t| u32::from(t.image))
        );
        assert_eq!(model.mails[0].items[1].name.as_deref(), Some("RedPotion"));
        assert_eq!(model.mails[0].items[1].count, 1);
    }

    #[test]
    fn malformed_rows_never_invent_an_empty_or_successful_mailbox() {
        for payload in [
            json!({}),
            json!({"mail":null}),
            json!({"mail":[null]}),
            json!({"mail":[{"mailId":"not-an-id","items":[]}]}),
            json!({"mail":[{"mailId":1,"items":[{}]}]}),
        ] {
            assert!(try_transform_mail_model_from_packet(&payload).is_none());
        }
        let projected = mail_model_from_entries(&[
            json!({"id":1,"items":[],"operation":{"kind":"send","success":true}}),
        ])
        .unwrap();
        let model: MailModel = serde_json::from_value(projected).unwrap();
        assert!(model.operation_feedback().is_none());
    }

    #[test]
    fn full_shared_mailbox_capacity_is_not_trimmed_by_projection() {
        let rows: Vec<_> = (1..=MAX_MAIL_MESSAGES).map(|id| row(id as u64)).collect();
        let model: MailModel =
            serde_json::from_value(mail_model_from_entries(&rows).unwrap()).unwrap();
        assert_eq!(model.visible_mails().len(), MAX_MAIL_MESSAGES);
        assert_eq!(model.mails.last().unwrap().id, MAX_MAIL_MESSAGES as u64);
    }

    #[test]
    fn parcel_service_uses_server_cost_and_exact_u64_item_identity_in_fifo_format() {
        assert_eq!(
            mail_service_event_from_packet("MailSendRequest", &Value::Null),
            Some(MailServiceEvent::OpenParcel)
        );
        assert_eq!(
            mail_service_event_from_packet("MailCost", &json!({"data":{"cost":"4294967295"}})),
            Some(MailServiceEvent::Cost { cost: u32::MAX })
        );
        assert_eq!(
            mail_service_event_from_packet(
                "MailLockedItem",
                &json!({"unique_id":"18446744073709551614","locked":true})
            ),
            Some(MailServiceEvent::LockedItem {
                unique_id: u64::MAX - 1,
                locked: true
            })
        );
        for (packet, payload) in [
            ("MailCost", json!({})),
            ("MailCost", json!({"cost":-1})),
            ("MailCost", json!({"cost":4294967296u64})),
            ("MailLockedItem", json!({"uniqueId":1,"locked":"true"})),
            ("MailSent", json!({"result":1})),
            ("ParcelCollected", json!({"result":1})),
            ("AdminMail", json!({})),
        ] {
            assert!(mail_service_event_from_packet(packet, &payload).is_none());
        }
    }
}
