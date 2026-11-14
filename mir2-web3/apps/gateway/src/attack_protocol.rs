//! Opt-in ordinary melee launch receipts, scoped to one authenticated socket.
//! These are neither damage receipts nor cross-connection exactly-once proofs.
use mir2_simulation::{ActiveSessionIdentity, NativeAttackAdmission, NativeAttackError};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackV2Request {
    request_id: String,
    pub object_id: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttackV2Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "protocolVersion")]
    version: u16,
    #[serde(rename = "requestId")]
    request_id: String,
    #[serde(rename = "objectId")]
    object_id: u32,
}

impl AttackV2Request {
    fn sequence(&self) -> u64 {
        self.request_id
            .parse()
            .expect("validated canonical request ID")
    }
}

pub fn parse_attack_v2(bytes: &str) -> Result<AttackV2Request, String> {
    // Parse original bytes: duplicate and unknown fields must remain visible.
    let request: AttackV2Envelope =
        serde_json::from_str(bytes).map_err(|_| "invalid attackV2 request".to_owned())?;
    let sequence = request.request_id.parse::<u64>().ok();
    if request.kind != "attackV2"
        || request.version != 2
        || request.object_id == 0
        || !sequence.is_some_and(|id| id > 0 && id.to_string() == request.request_id)
    {
        return Err("invalid attackV2 request".to_owned());
    }
    Ok(AttackV2Request {
        request_id: request.request_id,
        object_id: request.object_id,
    })
}

fn frame(request: &AttackV2Request, result: &str, code: Option<&str>) -> Value {
    let mut value = json!({"type":"attackResultV2", "protocolVersion":2,
        "requestId":request.request_id, "objectId":request.object_id, "result":result});
    if let Some(code) = code {
        value["code"] = json!(code);
    }
    value
}

pub fn error_frame(request: &AttackV2Request, error: &NativeAttackError) -> Value {
    match error {
        NativeAttackError::Unsupported => frame(request, "unavailable", Some("unsupported")),
        NativeAttackError::BeforeExecution(_) => {
            frame(request, "unavailable", Some("executionUnavailable"))
        }
        NativeAttackError::Unknown(_) => frame(request, "unknown", Some("outcomeUnknown")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActorBinding {
    account: String,
    character: i32,
    name: String,
}
impl From<&ActiveSessionIdentity> for ActorBinding {
    fn from(identity: &ActiveSessionIdentity) -> Self {
        Self {
            account: identity.account_id.clone(),
            character: identity.character_index,
            name: identity.character_name.clone(),
        }
    }
}

#[derive(Debug)]
pub enum PreparedAttack {
    Execute,
    Reply(Value),
}

#[derive(Debug, Default)]
pub struct AttackV2Connection {
    high_water: u64,
    pending: Option<(AttackV2Request, ActorBinding)>,
    completed: Option<(AttackV2Request, ActorBinding, Value)>,
}

impl AttackV2Connection {
    pub fn prepare(
        &mut self,
        request: &AttackV2Request,
        authenticated: bool,
        identity: Option<&ActiveSessionIdentity>,
    ) -> PreparedAttack {
        let Some(identity) = identity.filter(|identity| {
            authenticated
                && !identity.account_id.is_empty()
                && identity.character_index >= 0
                && !identity.character_name.is_empty()
        }) else {
            return PreparedAttack::Reply(frame(
                request,
                "unavailable",
                Some("authenticationRequired"),
            ));
        };
        let binding = ActorBinding::from(identity);
        if request.sequence() <= self.high_water {
            if let Some((previous, actor, result)) = &self.completed {
                if previous == request && *actor == binding {
                    // Replay only the immutable result, never ordinary packets.
                    return PreparedAttack::Reply(result.clone());
                }
            }
            return PreparedAttack::Reply(frame(request, "unavailable", Some("requestIdNotFresh")));
        }
        if self.pending.is_some() {
            return PreparedAttack::Reply(frame(
                request,
                "unknown",
                Some("previousOutcomeUnknown"),
            ));
        }
        // Reserve before invoking any mutation. Failure never resets this floor.
        self.high_water = request.sequence();
        self.completed = None;
        self.pending = Some((request.clone(), binding));
        PreparedAttack::Execute
    }

    pub fn complete(
        &mut self,
        request: &AttackV2Request,
        admission: NativeAttackAdmission,
        identity: Option<&ActiveSessionIdentity>,
    ) -> Result<Value, NativeAttackError> {
        let actor = identity.map(ActorBinding::from);
        let Some((pending, binding)) = self.pending.as_ref() else {
            return Err(NativeAttackError::Unknown(
                "no reserved attack result".into(),
            ));
        };
        if pending != request || actor.as_ref() != Some(binding) {
            return Err(NativeAttackError::Unknown(
                "attack result actor/request changed".into(),
            ));
        }
        let result = frame(
            request,
            match admission {
                NativeAttackAdmission::Launched => "launched",
                NativeAttackAdmission::NotLaunched => "notLaunched",
            },
            None,
        );
        let binding = binding.clone();
        self.pending = None;
        self.completed = Some((request.clone(), binding, result.clone()));
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str, target: u32) -> AttackV2Request {
        parse_attack_v2(
            &json!({"type":"attackV2","protocolVersion":2,"requestId":id,"objectId":target})
                .to_string(),
        )
        .unwrap()
    }
    fn actor() -> ActiveSessionIdentity {
        ActiveSessionIdentity {
            account_id: "ordinary".into(),
            character_index: 0,
            character_name: "fighter".into(),
        }
    }

    #[test]
    fn strict_original_bytes_reject_unknown_duplicate_missing_and_noncanonical_fields() {
        for raw in [
            r#"{"type":"attackV2","protocolVersion":2,"requestId":"1","objectId":7,"extra":true}"#,
            r#"{"type":"attackV2","protocolVersion":2,"requestId":"1","requestId":"2","objectId":7}"#,
            r#"{"type":"attackV2","type":"attack","protocolVersion":2,"requestId":"1","objectId":7}"#,
            r#"{"type":"attackV2","protocolVersion":2,"objectId":7}"#,
        ] {
            assert!(parse_attack_v2(raw).is_err());
        }
        for id in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            "1.0",
            " 1",
            "18446744073709551616",
        ] {
            assert!(parse_attack_v2(
                &json!({"type":"attackV2","protocolVersion":2,"requestId":id,"objectId":7})
                    .to_string()
            )
            .is_err());
        }
        for (key, value) in [
            ("protocolVersion", json!(1)),
            ("protocolVersion", json!("2")),
            ("objectId", json!(0)),
            ("objectId", json!(-1)),
            ("objectId", json!(4294967296u64)),
            ("requestId", json!(1)),
            ("type", json!("attack")),
        ] {
            let mut value_request =
                json!({"type":"attackV2","protocolVersion":2,"requestId":"1","objectId":7});
            value_request[key] = value;
            assert!(parse_attack_v2(&value_request.to_string()).is_err());
        }
        assert_eq!(
            request("18446744073709551615", u32::MAX).sequence(),
            u64::MAX
        );
    }

    #[test]
    fn auth_and_active_actor_are_required_before_any_reservation() {
        let mut connection = AttackV2Connection::default();
        let request = request("1", 7);
        let actor = actor();
        assert!(matches!(
            connection.prepare(&request, false, Some(&actor)),
            PreparedAttack::Reply(_)
        ));
        assert!(matches!(
            connection.prepare(&request, true, None),
            PreparedAttack::Reply(_)
        ));
        assert_eq!(connection.high_water, 0);
        assert!(matches!(
            connection.prepare(&request, true, Some(&actor)),
            PreparedAttack::Execute
        ));
    }

    #[test]
    fn replay_resends_result_without_reexecution_and_changed_payload_or_actor_is_denied() {
        let mut connection = AttackV2Connection::default();
        let request = request("1", 7);
        let actor = actor();
        assert!(matches!(
            connection.prepare(&request, true, Some(&actor)),
            PreparedAttack::Execute
        ));
        let result = connection
            .complete(&request, NativeAttackAdmission::Launched, Some(&actor))
            .unwrap();
        let PreparedAttack::Reply(replay) = connection.prepare(&request, true, Some(&actor)) else {
            panic!("must replay")
        };
        assert_eq!(replay, result);
        assert_eq!(replay["result"], "launched");
        let mut changed = request.clone();
        changed.object_id = 8;
        let PreparedAttack::Reply(denied) = connection.prepare(&changed, true, Some(&actor)) else {
            panic!("must deny")
        };
        assert_eq!(denied["code"], "requestIdNotFresh");
        let mut other = actor.clone();
        other.character_index = 1;
        assert!(
            matches!(connection.prepare(&request,true,Some(&other)),PreparedAttack::Reply(value) if value["result"]=="unavailable")
        );
    }

    #[test]
    fn eviction_never_revives_old_request_and_ambiguous_execution_blocks_new_mutation() {
        let mut connection = AttackV2Connection::default();
        let actor = actor();
        for id in ["1", "2"] {
            let request = request(id, 7);
            assert!(matches!(
                connection.prepare(&request, true, Some(&actor)),
                PreparedAttack::Execute
            ));
            connection
                .complete(&request, NativeAttackAdmission::NotLaunched, Some(&actor))
                .unwrap();
        }
        assert!(
            matches!(connection.prepare(&request("1",7),true,Some(&actor)),PreparedAttack::Reply(value) if value["code"]=="requestIdNotFresh")
        );
        assert!(matches!(
            connection.prepare(&request("3", 7), true, Some(&actor)),
            PreparedAttack::Execute
        ));
        assert!(
            matches!(connection.prepare(&request("4",7),true,Some(&actor)),PreparedAttack::Reply(value) if value["result"]=="unknown")
        );
        assert_eq!(connection.high_water, 3);
    }

    #[test]
    fn completion_cannot_change_reserved_actor_and_errors_never_expose_internal_details() {
        let mut connection = AttackV2Connection::default();
        let request = request("1", 7);
        let actor = actor();
        connection.prepare(&request, true, Some(&actor));
        assert!(connection
            .complete(&request, NativeAttackAdmission::Launched, None)
            .is_err());
        assert!(connection.pending.is_some());
        for error in [
            NativeAttackError::Unsupported,
            NativeAttackError::BeforeExecution("secret detail".into()),
            NativeAttackError::Unknown("secret detail".into()),
        ] {
            let value = error_frame(&request, &error);
            assert!(!value.to_string().contains("secret"));
            assert_ne!(value["result"], "notLaunched");
        }
    }
}
