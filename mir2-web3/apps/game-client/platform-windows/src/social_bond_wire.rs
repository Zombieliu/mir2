//! Ordinary social packets only; never optimistic relationship or permission state.
use crate::native_protocol::NativeOutboundCommand as C;
use mir2_protocol::{ClientPacket as P, ServerPacket as S};
use serde::Deserialize;
use serde_json::Value;
pub fn command(packet: &P) -> Option<C> {
    Some(match packet {
        P::AllowMentor => C::AllowMentor,
        P::AddMentor { name } => C::AddMentor { name: name.clone() },
        P::CancelMentor => C::CancelMentor,
        P::MentorReply { accept_invite } => C::MentorReply {
            accept_invite: *accept_invite,
        },
        P::ChangeMarriage => C::ChangeMarriage,
        P::MarriageRequest => C::MarriageRequest,
        P::MarriageReply { accept_invite } => C::MarriageReply {
            accept_invite: *accept_invite,
        },
        P::DivorceRequest => C::DivorceRequest,
        P::DivorceReply { accept_invite } => C::DivorceReply {
            accept_invite: *accept_invite,
        },
        _ => return None,
    })
}
pub fn packet(name: &str, payload: &Value) -> Option<S> {
    #[derive(Deserialize)]
    struct Name {
        name: String,
    }
    Some(match name {
        "MentorUpdate" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct M {
                name: String,
                level: u16,
                online: bool,
                mentee_exp: i64,
            }
            let p: M = serde_json::from_value(payload.clone()).ok()?;
            S::MentorUpdate {
                name: p.name,
                level: p.level,
                online: p.online,
                mentee_exp: p.mentee_exp,
            }
        }
        "LoverUpdate" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct L {
                name: String,
                date_binary_datetime: i64,
                map_name: String,
                married_days: i16,
            }
            let p: L = serde_json::from_value(payload.clone()).ok()?;
            S::LoverUpdate {
                name: p.name,
                date_binary_datetime: p.date_binary_datetime,
                map_name: p.map_name,
                married_days: p.married_days,
            }
        }
        "MentorRequest" => {
            #[derive(Deserialize)]
            struct M {
                name: String,
                level: u16,
            }
            let p: M = serde_json::from_value(payload.clone()).ok()?;
            S::MentorRequest {
                name: p.name,
                level: p.level,
            }
        }
        "MarriageRequest" => S::MarriageRequest {
            name: serde_json::from_value::<Name>(payload.clone()).ok()?.name,
        },
        "DivorceRequest" => S::DivorceRequest {
            name: serde_json::from_value::<Name>(payload.clone()).ok()?.name,
        },
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn original_fields_are_preserved_and_missing_fields_fail_closed() {
        assert!(matches!(
            packet(
                "MentorUpdate",
                &json!({"name":"Mentor","level":50,"online":true,"menteeExp":1234567})
            ),
            Some(S::MentorUpdate {
                mentee_exp: 1234567,
                ..
            })
        ));
        assert!(packet("MentorUpdate", &json!({"name":"Mentor"})).is_none());
        assert!(matches!(
            packet(
                "LoverUpdate",
                &json!({"name":"Peer","dateBinaryDatetime":638000000000000000i64,"mapName":"Bichon","marriedDays":3})
            ),
            Some(S::LoverUpdate {
                married_days: 3,
                ..
            })
        ));
    }
    #[test]
    fn social_replies_use_ordinary_wire_and_preserve_decline() {
        let p = command(&P::MentorReply {
            accept_invite: false,
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            json!({"type":"mentorReply","acceptInvite":false})
        );
        assert!(command(&P::LogOut).is_none());
    }
}
