//! Bounded mailbox/parcel ingress for one accepted character and host epoch.
//! Only own host-correlated feedback is attached to a later authoritative list.
//! No authentication, local custody, eligibility, postage or settlement rules.
use mir2_client_bevy::{
    mail::{
        MailModel, MailOperationFeedback, MailOperationKind, MAX_MAIL_ATTACHMENTS,
        MAX_MAIL_MESSAGES,
    },
    mail_service::MailServiceEvent,
    native_mail_ingress::{
        mail_service_event_from_packet, mail_source, try_transform_mail_model_from_packet,
        try_transform_mail_model_from_snapshot,
    },
};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};

pub(crate) const MAX_MAIL_PACKET_BYTES: usize = 512 * 1024;
const MAX_SERVICE_PACKET_BYTES: usize = 16 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const MAX_MODEL_BYTES: usize = 512 * 1024;
const MAX_PENDING: usize = 64;
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

/// The host dispatches only these read-only packets around other domains'
/// smaller packet caps. Classification grants neither identity nor admission.
pub(crate) fn is_mail_packet(raw: &str) -> bool {
    if raw.len() > MAX_MAIL_PACKET_BYTES {
        return false;
    }
    serde_json::from_str::<Value>(raw)
        .ok()
        .is_some_and(|value| {
            value["type"] == "androidMailResult"
                || (value["type"] == "packet"
                    && value["packet"].as_str().is_some_and(is_mail_packet_name))
        })
}
fn is_mail_packet_name(name: &str) -> bool {
    matches!(
        name,
        "ReceiveMail" | "MailSendRequest" | "MailCost" | "MailLockedItem"
    )
}

#[derive(Clone)]
enum Pending {
    Model(String),
    Service(String),
}
impl Pending {
    fn json(&self) -> &str {
        match self {
            Self::Model(raw) | Self::Service(raw) => raw,
        }
    }
}

#[derive(Default)]
pub(crate) struct AndroidMailIngress {
    identity: Option<(u32, String)>,
    map: Option<String>,
    pending: VecDeque<Pending>,
    generation: Option<u64>,
    feedback: Option<MailOperationFeedback>,
}
impl AndroidMailIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
    pub(crate) fn clear_scene(&mut self) {
        // Mail and parcel FIFO are personal, matching the shared runtime's
        // SceneReset policy. Only DataReset retires this connection's mailbox.
        self.map = None;
    }
    pub(crate) fn snapshot(&mut self, raw: &str) -> Result<(), &'static str> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("Mail snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid mail snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing mail owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing mail entities")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing mail self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid mail self player");
        }
        let name = bounded_identity(&actor["name"]).ok_or("Invalid mail character")?;
        let map = bounded_identity(&world["mapFileName"]).ok_or("Invalid mail map")?;
        let identity = (owner, name.to_owned());
        if self.identity.as_ref().is_some_and(|old| old != &identity) {
            return Err("Mail owner changed; reconnect");
        }
        let generation = world
            .get("androidMailGeneration")
            .map(|value| host_u64(value).ok_or("Invalid mail host epoch"))
            .transpose()?;
        if self.generation.is_some() && self.generation != generation {
            return Err("Mail host epoch changed; reconnect");
        }
        let mut pending = self.pending.clone();
        let mut feedback_attached = false;
        if let Some(entries) = mail_source(&world) {
            let count = checked_source(entries, true)?;
            let model = try_transform_mail_model_from_snapshot(&world)
                .ok_or("Invalid mail snapshot projection")?;
            let raw = checked_model(model, count)?;
            let raw = with_mail_feedback(raw, self.feedback.as_ref())?;
            feedback_attached = self.feedback.is_some();
            let message = Pending::Model(raw);
            // Packet-first current data follows the initial base, never the reverse.
            if self.identity.is_none() {
                pending.push_front(message);
            } else {
                pending.push_back(message);
            }
        }
        checked_queue(&pending)?;
        self.identity = Some(identity);
        self.map = Some(map.to_owned());
        self.pending = pending;
        self.generation = generation;
        if feedback_attached {
            self.feedback = None;
        }
        Ok(())
    }
    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        if raw.len() > MAX_MAIL_PACKET_BYTES {
            return Err("Mail packet too large");
        }
        let envelope: Value = serde_json::from_str(raw).map_err(|_| "Invalid mail packet")?;
        if envelope["type"] == "androidMailResult" {
            return self.own_result(raw, &envelope);
        }
        if envelope["type"] != "packet" {
            return Ok(false);
        }
        let packet = envelope["packet"].as_str().unwrap_or("");
        if !is_mail_packet_name(packet) {
            return Ok(false);
        }
        if packet != "ReceiveMail" && raw.len() > MAX_SERVICE_PACKET_BYTES {
            return Err("Mail service packet too large");
        }
        let payload = envelope.get("payload").unwrap_or(&Value::Null);
        if !(payload.is_object() || (packet == "MailSendRequest" && payload.is_null())) {
            return Err("Invalid mail payload");
        }
        let body = payload
            .get("data")
            .filter(|v| v.is_object())
            .unwrap_or(payload);
        if !self.matches_owner(payload) || !self.matches_owner(body) {
            return Ok(false);
        }
        let message = if packet == "ReceiveMail" {
            let count = checked_source(&payload["mail"], false)?;
            let model = try_transform_mail_model_from_packet(payload)
                .ok_or("Invalid mail packet projection")?;
            Pending::Model(with_mail_feedback(
                checked_model(model, count)?,
                self.feedback.as_ref(),
            )?)
        } else {
            let event = mail_service_event_from_packet(packet, payload)
                .ok_or("Invalid mail service event")?;
            let json = serde_json::to_string(&event).map_err(|_| "Invalid typed mail event")?;
            let _: MailServiceEvent =
                serde_json::from_str(&json).map_err(|_| "Invalid typed mail event")?;
            Pending::Service(json)
        };
        let mut pending = self.pending.clone();
        pending.push_back(message);
        checked_queue(&pending)?;
        self.pending = pending;
        if packet == "ReceiveMail" {
            self.feedback = None;
        }
        Ok(true)
    }
    fn own_result(&mut self, raw: &str, envelope: &Value) -> Result<bool, &'static str> {
        if raw.len() > MAX_SERVICE_PACKET_BYTES {
            return Err("Mail result too large");
        }
        let Some((owner, name)) = self.identity.as_ref() else {
            return Ok(false);
        };
        if self.generation.is_none()
            || host_u64(&envelope["connectionGeneration"]) != self.generation
            || envelope["ownerObjectId"].as_u64() != Some(u64::from(*owner))
            || envelope["characterName"].as_str() != Some(name.as_str())
        {
            return Ok(false);
        }
        let Some(result) = envelope["result"].as_i64().filter(|n| matches!(n, 1 | -1)) else {
            return Ok(false);
        };
        let (kind, mail_id) = match envelope["packet"].as_str() {
            Some("MailSent") if envelope["claimMailId"].is_null() => {
                (MailOperationKind::Send, None)
            }
            Some("ParcelCollected") => (
                MailOperationKind::Collect,
                Some(host_u64(&envelope["claimMailId"]).ok_or("Invalid owned claim ID")?),
            ),
            _ => return Ok(false),
        };
        if self.feedback.is_some() {
            return Err("Mail feedback already waiting");
        }
        self.feedback = Some(MailOperationFeedback {
            kind,
            success: result == 1,
            mail_id,
        });
        Ok(true)
    }
    fn matches_owner(&self, payload: &Value) -> bool {
        if payload
            .get("hero")
            .is_some_and(|v| v.as_bool() != Some(false))
        {
            return false;
        }
        if payload.get("ownerObjectId").is_some_and(|id| {
            self.identity
                .as_ref()
                .is_none_or(|(owner, _)| id.as_u64() != Some(u64::from(*owner)))
        }) {
            return false;
        }
        if payload.get("characterName").is_some_and(|name| {
            self.identity
                .as_ref()
                .is_none_or(|(_, owner)| name.as_str() != Some(owner.as_str()))
        }) {
            return false;
        }
        if payload
            .get("mapFileName")
            .is_some_and(|map| self.map.is_none() || map.as_str() != self.map.as_deref())
        {
            return false;
        }
        true
    }
    pub(crate) fn flush(
        &mut self,
        mut push_model: impl FnMut(String) -> bool,
        mut push_service: impl FnMut(String) -> bool,
    ) -> bool {
        if self.identity.is_none() {
            return false;
        }
        while let Some(front) = self.pending.front() {
            let admitted = match front {
                Pending::Model(raw) => push_model(raw.clone()),
                Pending::Service(raw) => push_service(raw.clone()),
            };
            if !admitted {
                return false;
            }
            self.pending.pop_front();
        }
        true
    }
    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
}
fn bounded_identity(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|v| !v.trim().is_empty() && v.chars().count() <= 128)
}
fn checked_source(source: &Value, filter_deleted: bool) -> Result<usize, &'static str> {
    let rows = source.as_array().ok_or("Invalid mail list")?;
    let mut count = 0;
    for row in rows {
        if filter_deleted && row["deleted"].as_bool() == Some(true) {
            continue;
        }
        if !row.is_object() {
            return Err("Invalid mail row");
        }
        let items = row["items"]
            .as_array()
            .ok_or("Invalid mail attachment list")?;
        if items.len() > MAX_MAIL_ATTACHMENTS
            || items
                .iter()
                .any(|item| !item.is_object() && !item.is_string())
        {
            return Err("Invalid or oversized mail attachments");
        }
        count += 1;
        if count > MAX_MAIL_MESSAGES {
            return Err("Mail list too large");
        }
    }
    Ok(count)
}
fn checked_model(model: Value, expected: usize) -> Result<String, &'static str> {
    let typed: MailModel =
        serde_json::from_value(model.clone()).map_err(|_| "Invalid typed mail model")?;
    let mut ids = HashSet::new();
    if typed.mails.len() != expected
        || typed
            .mails
            .iter()
            .any(|mail| mail.operation.is_some() || !ids.insert(mail.id))
    {
        return Err("Invalid mail identity or operation domain");
    }
    let raw = model.to_string();
    if raw.len() > MAX_MODEL_BYTES {
        return Err("Mail model too large");
    }
    Ok(raw)
}
fn checked_queue(queue: &VecDeque<Pending>) -> Result<(), &'static str> {
    if queue.len() > MAX_PENDING
        || queue.iter().map(|m| m.json().len()).sum::<usize>() > MAX_PENDING_BYTES
    {
        return Err("Mail ingress queue full");
    }
    Ok(())
}

fn host_u64(value: &Value) -> Option<u64> {
    let value = value.as_str()?;
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok().filter(|n| *n != 0)
}

fn with_mail_feedback(
    raw: String,
    feedback: Option<&MailOperationFeedback>,
) -> Result<String, &'static str> {
    let Some(feedback) = feedback else {
        return Ok(raw);
    };
    let mut model: Value = serde_json::from_str(&raw).map_err(|_| "Invalid mail feedback base")?;
    let rows = model["mails"]
        .as_array_mut()
        .ok_or("Missing mail feedback list")?;
    let count = rows.len();
    // Exact frozen Windows transient feedback row; shared MailModel keeps it
    // outside the 256 real-message quota and its existing consumer owns cleanup.
    rows.push(
        serde_json::json!({"id":u64::MAX,"sender":"","subject":"","body":"",
        "gold":0,"items":[],"claimed":false,"locked":true,"read":true,"operation":feedback}),
    );
    let typed: MailModel =
        serde_json::from_value(model.clone()).map_err(|_| "Invalid typed mail feedback")?;
    if typed.mails.len() != count + 1 || typed.operation_feedback() != Some(feedback) {
        return Err("Mail feedback was not retained");
    }
    let raw = model.to_string();
    if raw.len() > MAX_MODEL_BYTES {
        return Err("Mail feedback model too large");
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    fn world(owner: u32, name: &str, map: &str) -> Value {
        json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name}]})
    }
    fn row(id: u64) -> Value {
        json!({"mailId":id.to_string(),"senderName":"NPC","message":"Subject\nBody",
            "gold":77,"items":[],"canReply":true,"dateSentBinaryDatetime":"638970336000000000"})
    }
    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }

    fn bound_mail() -> AndroidMailIngress {
        let mut ingress = AndroidMailIngress::default();
        let mut base = world(42, "Fixture", "0");
        base["androidMailGeneration"] = json!("9");
        ingress.snapshot(&base.to_string()).unwrap();
        ingress
    }

    fn own_result(packet: &str, result: i32, claim: Option<u64>) -> Value {
        json!({"type":"androidMailResult","packet":packet,"result":result,
            "connectionGeneration":"9","ownerObjectId":42,"characterName":"Fixture",
            "claimMailId":claim.map(|id|id.to_string())})
    }

    #[test]
    fn mail_result_waits_for_later_authoritative_refresh_and_uses_owned_claim_id() {
        let mut ingress = bound_mail();
        assert!(!ingress
            .packet(&packet("ParcelCollected", json!({"result":1,"mailId":3})))
            .unwrap());
        let event = own_result("ParcelCollected", 1, Some(9007199254740993));
        assert!(is_mail_packet(&event.to_string()));
        assert!(ingress.packet(&event.to_string()).unwrap());
        assert!(ingress.flush(|_| panic!("ACK cannot invent a mailbox"), |_| false));
        assert!(ingress
            .packet(&packet("ReceiveMail", json!({"mail":[row(2)]})))
            .unwrap());
        assert!(ingress.flush(
            |raw| {
                let model: MailModel = serde_json::from_str(&raw).unwrap();
                assert_eq!(model.mails[0].id, 2);
                assert_eq!(model.mails[0].gold, 77);
                assert!(
                    !model.mails[0].claimed,
                    "A receipt must not edit server mail custody"
                );
                assert_eq!(
                    model.operation_feedback().unwrap().mail_id,
                    Some(9007199254740993)
                );
                assert!(model.operation_feedback().unwrap().success);
                true
            },
            |_| false
        ));
    }

    #[test]
    fn mail_result_rejects_anonymous_stale_other_owner_and_invalid_kind_or_result() {
        let mut ingress = bound_mail();
        for (key, value) in [
            ("connectionGeneration", json!("8")),
            ("ownerObjectId", json!(43)),
            ("characterName", json!("Other")),
            ("packet", json!("ReadMail")),
            ("result", json!(0)),
            ("connectionGeneration", Value::Null),
        ] {
            let mut event = own_result("MailSent", 1, None);
            event[key] = value;
            assert!(!ingress
                .packet(&event.to_string())
                .is_ok_and(|accepted| accepted));
        }
        let mut unbound = AndroidMailIngress::default();
        assert!(!unbound
            .packet(&own_result("MailSent", 1, None).to_string())
            .is_ok_and(|a| a));
        assert!(ingress
            .packet(&own_result("MailSent", -1, None).to_string())
            .unwrap());
        assert!(
            ingress
                .packet(&own_result("MailSent", 1, None).to_string())
                .is_err(),
            "No second feedback may replace the first"
        );
        assert!(ingress
            .packet(&packet("ReceiveMail", json!({"mail":[]})))
            .unwrap());
        assert!(ingress.flush(
            |raw| {
                let model: MailModel = serde_json::from_str(&raw).unwrap();
                let result = model.operation_feedback().unwrap();
                assert_eq!(result.kind, mir2_client_bevy::mail::MailOperationKind::Send);
                assert!(!result.success);
                assert_eq!(result.mail_id, None);
                true
            },
            |_| false
        ));
    }

    #[test]
    fn mail_result_full_inbox_preserves_256_real_rows_and_exact_retry_front() {
        let mut ingress = bound_mail();
        assert!(ingress
            .packet(&own_result("ParcelCollected", -1, Some(u64::MAX)).to_string())
            .unwrap());
        let rows: Vec<_> = (1..=MAX_MAIL_MESSAGES).map(|id| row(id as u64)).collect();
        assert!(ingress
            .packet(&packet("ReceiveMail", json!({"mail":rows})))
            .unwrap());
        let mut rejected = None;
        assert!(!ingress.flush(
            |raw| {
                rejected = Some(raw);
                false
            },
            |_| false
        ));
        assert_eq!(ingress.pending_count(), 1);
        assert!(ingress.flush(
            |raw| {
                assert_eq!(Some(&raw), rejected.as_ref());
                let model: MailModel = serde_json::from_str(&raw).unwrap();
                assert_eq!(
                    model.mails.iter().filter(|m| m.operation.is_none()).count(),
                    256
                );
                assert_eq!(model.mails.len(), 257);
                assert_eq!(model.operation_feedback().unwrap().mail_id, Some(u64::MAX));
                assert!(!model.operation_feedback().unwrap().success);
                true
            },
            |_| false
        ));
    }

    #[test]
    fn mail_result_scene_keeps_receipt_and_reset_retires_old_connection() {
        let mut ingress = bound_mail();
        assert!(ingress
            .packet(&own_result("MailSent", 1, None).to_string())
            .unwrap());
        ingress.clear_scene();
        let mut next = world(42, "Fixture", "1");
        next["androidMailGeneration"] = json!("9");
        ingress.snapshot(&next.to_string()).unwrap();
        assert!(!ingress
            .packet(&packet("ReceiveMail", json!({"mail":[],"mapFileName":"0"})))
            .unwrap());
        next["mails"] = json!([]);
        ingress.snapshot(&next.to_string()).unwrap();
        assert!(ingress.flush(
            |raw| {
                assert!(serde_json::from_str::<MailModel>(&raw)
                    .unwrap()
                    .operation_feedback()
                    .is_some());
                true
            },
            |_| false
        ));
        ingress.reset();
        next["androidMailGeneration"] = json!("10");
        ingress.snapshot(&next.to_string()).unwrap();
        assert!(!ingress
            .packet(&own_result("MailSent", 1, None).to_string())
            .unwrap());
        assert!(ingress.flush(
            |raw| {
                assert!(serde_json::from_str::<MailModel>(&raw)
                    .unwrap()
                    .operation_feedback()
                    .is_none());
                true
            },
            |_| false
        ));
    }
    #[test]
    fn mail_packet_first_stays_inert_until_owner_then_follows_snapshot_base() {
        let mut ingress = AndroidMailIngress::default();
        assert!(ingress
            .packet(&packet("ReceiveMail", json!({"mail":[row(2)]})))
            .unwrap());
        assert!(ingress
            .packet(&packet("MailCost", json!({"cost":125})))
            .unwrap());
        assert!(!ingress.flush(|_| panic!("unbound"), |_| panic!("unbound")));
        let mut base = world(42, "Fixture", "0");
        base["stage5Systems"] = json!({"mail":[row(1)]});
        ingress.snapshot(&base.to_string()).unwrap();
        let seen = RefCell::new(Vec::new());
        assert!(ingress.flush(
            |raw| {
                seen.borrow_mut().push(("model", raw));
                true
            },
            |raw| {
                seen.borrow_mut().push(("service", raw));
                true
            }
        ));
        let rows = seen.into_inner();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            serde_json::from_str::<MailModel>(&rows[0].1).unwrap().mails[0].id,
            1
        );
        assert_eq!(
            serde_json::from_str::<MailModel>(&rows[1].1).unwrap().mails[0].id,
            2
        );
        assert_eq!(
            serde_json::from_str::<MailServiceEvent>(&rows[2].1).unwrap(),
            MailServiceEvent::Cost { cost: 125 }
        );
    }
    #[test]
    fn mail_missing_snapshot_binds_but_does_not_clear_existing_inbox() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        assert!(ingress.flush(
            |_| panic!("invented mailbox"),
            |_| panic!("invented service")
        ));
        assert_eq!(ingress.pending_count(), 0);
        let mut base = world(42, "Fixture", "0");
        base["mails"] = json!([]);
        ingress.snapshot(&base.to_string()).unwrap();
        assert!(ingress.flush(
            |raw| {
                assert!(serde_json::from_str::<MailModel>(&raw)
                    .unwrap()
                    .mails
                    .is_empty());
                true
            },
            |_| false
        ));
    }
    #[test]
    fn mail_shared_capacity_256_and_all_five_attachments_are_retained() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        let rows:Vec<_>=(1..=MAX_MAIL_MESSAGES).map(|id|{
            let mut r=row(id as u64);r["message"]=json!("邮件正文".repeat(20));
            r["items"]=json!((1..=MAX_MAIL_ATTACHMENTS).map(|n|
                json!({"uniqueId":(id as u64*10+n as u64).to_string(),"name":"RedPotion","count":n})).collect::<Vec<_>>());
            r
        }).collect();
        let raw = packet("ReceiveMail", json!({"mail":rows}));
        assert!(raw.len() > MAX_SERVICE_PACKET_BYTES && raw.len() < MAX_MAIL_PACKET_BYTES);
        assert!(ingress.packet(&raw).unwrap());
        assert!(ingress.flush(
            |raw| {
                let model: MailModel = serde_json::from_str(&raw).unwrap();
                assert_eq!(model.mails.len(), MAX_MAIL_MESSAGES);
                assert_eq!(model.mails.last().unwrap().id, MAX_MAIL_MESSAGES as u64);
                assert!(model
                    .mails
                    .iter()
                    .all(|mail| mail.items.len() == MAX_MAIL_ATTACHMENTS));
                true
            },
            |_| false
        ));
    }
    #[test]
    fn mail_owner_and_map_guards_apply_to_outer_and_nested_service_fields() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        for wrong in [
            json!({"ownerObjectId":43}),
            json!({"characterName":"Other"}),
            json!({"mapFileName":"1"}),
            json!({"hero":true}),
        ] {
            for nested in [false, true] {
                let mut body = wrong.clone();
                body["cost"] = json!(125);
                let payload = if nested { json!({"data":body}) } else { body };
                assert!(!ingress.packet(&packet("MailCost", payload)).unwrap());
            }
        }
        assert_eq!(ingress.pending_count(), 0);
        assert!(ingress
            .packet(&packet(
                "MailCost",
                json!({"ownerObjectId":42,"characterName":"Fixture","mapFileName":"0","cost":125})
            ))
            .unwrap());
    }
    #[test]
    fn mail_owner_change_failure_keeps_previous_state_until_explicit_reset() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        ingress
            .packet(&packet("ReceiveMail", json!({"mail":[row(1)]})))
            .unwrap();
        assert!(ingress
            .snapshot(&world(43, "Other", "0").to_string())
            .is_err());
        assert_eq!(ingress.pending_count(), 1);
        ingress.reset();
        assert_eq!(ingress.pending_count(), 0);
        assert!(!ingress.flush(|_| panic!("retired"), |_| panic!("retired")));
        ingress
            .snapshot(&world(43, "Other", "0").to_string())
            .unwrap();
        assert!(ingress.flush(|_| false, |_| false));
    }
    #[test]
    fn mail_same_character_scene_reset_keeps_personal_fifo_but_rejects_old_map_tags() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        ingress
            .packet(&packet("MailCost", json!({"cost":1})))
            .unwrap();
        ingress.clear_scene();
        assert!(!ingress
            .packet(&packet("MailCost", json!({"cost":2,"mapFileName":"0"})))
            .unwrap());
        ingress
            .snapshot(&world(42, "Fixture", "1").to_string())
            .unwrap();
        ingress
            .packet(&packet("MailCost", json!({"cost":3,"mapFileName":"1"})))
            .unwrap();
        let mut costs = Vec::new();
        assert!(ingress.flush(
            |_| false,
            |raw| {
                if let MailServiceEvent::Cost { cost } = serde_json::from_str(&raw).unwrap() {
                    costs.push(cost);
                }
                true
            }
        ));
        assert_eq!(costs, [1, 3]);
    }
    #[test]
    fn mail_backpressure_keeps_exact_front_and_never_reorders_other_domains() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        for cost in [1, 2, 3] {
            ingress
                .packet(&packet("MailCost", json!({"cost":cost})))
                .unwrap();
        }
        let mut seen = Vec::new();
        assert!(!ingress.flush(
            |_| false,
            |raw| {
                let e: MailServiceEvent = serde_json::from_str(&raw).unwrap();
                seen.push(e.clone());
                e != MailServiceEvent::Cost { cost: 2 }
            }
        ));
        assert_eq!(ingress.pending_count(), 2);
        assert!(ingress.flush(
            |_| false,
            |raw| {
                seen.push(serde_json::from_str(&raw).unwrap());
                true
            }
        ));
        assert_eq!(
            seen,
            [
                MailServiceEvent::Cost { cost: 1 },
                MailServiceEvent::Cost { cost: 2 },
                MailServiceEvent::Cost { cost: 2 },
                MailServiceEvent::Cost { cost: 3 }
            ]
        );
    }
    #[test]
    fn mail_overflow_is_atomic_and_never_reports_success_or_evicts_old_front() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        for cost in 0..MAX_PENDING {
            ingress
                .packet(&packet("MailCost", json!({"cost":cost})))
                .unwrap();
        }
        assert!(ingress
            .packet(&packet("MailCost", json!({"cost":99})))
            .is_err());
        assert_eq!(ingress.pending_count(), MAX_PENDING);
        let mut base = world(42, "Fixture", "0");
        base["mails"] = json!([]);
        assert!(ingress.snapshot(&base.to_string()).is_err());
        assert_eq!(ingress.pending_count(), MAX_PENDING);
    }
    #[test]
    fn mail_malformed_lists_reject_duplicates_truncation_and_unknown_success_packets() {
        let mut ingress = AndroidMailIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
        for payload in [
            json!({"mail":[row(1),row(1)]}),
            json!({"mail":[null]}),
            json!({"mail":(0..=MAX_MAIL_MESSAGES).map(|id|row(id as u64)).collect::<Vec<_>>()}),
            json!({"mail":[{"id":1,"items":["a","b","c","d","e","f"]}]}),
        ] {
            assert!(ingress.packet(&packet("ReceiveMail", payload)).is_err());
            assert_eq!(ingress.pending_count(), 0);
        }
        for name in ["MailSent", "ParcelCollected", "Stage5Command", "AdminMail"] {
            let raw = packet(name, json!({"result":1,"success":true}));
            assert!(!is_mail_packet(&raw));
            assert!(!ingress.packet(&raw).unwrap());
        }
    }
    #[test]
    fn mail_service_and_mailbox_have_distinct_byte_caps_and_snapshot_identity_validation() {
        let mut ingress = AndroidMailIngress::default();
        let huge = packet(
            "MailCost",
            json!({"cost":1,"unused":"x".repeat(MAX_SERVICE_PACKET_BYTES)}),
        );
        assert!(ingress.packet(&huge).is_err());
        let huge = packet(
            "ReceiveMail",
            json!({"mail":[],"unused":"x".repeat(MAX_MAIL_PACKET_BYTES)}),
        );
        assert!(ingress.packet(&huge).is_err());
        for invalid in [
            world(0, "Fixture", "0"),
            world(42, "", "0"),
            world(42, "Fixture", ""),
        ] {
            assert!(ingress.snapshot(&invalid.to_string()).is_err());
        }
        let mut duplicate = world(42, "Fixture", "0");
        duplicate["entities"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind":"selfPlayer","objectId":42,"name":"Fixture"}));
        assert!(ingress.snapshot(&duplicate.to_string()).is_err());
        assert_eq!(ingress.pending_count(), 0);
    }
}
