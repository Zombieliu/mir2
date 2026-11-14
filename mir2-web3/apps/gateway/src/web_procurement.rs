//! Authenticated personal procurement. This path never accepts an account ID,
//! chain receipt, oracle proof, transaction bytes or an administrative adjustment.
use super::*;
use mir2_protocol::procurement::{ProcurementReply, ProcurementRequest, PROTOCOL_VERSION};

fn empty_reply(request: &ProcurementRequest, error: &str) -> ProcurementReply {
    ProcurementReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id.clone(),
        character_index: request.character_index,
        status: None,
        quote: None,
        error: Some(public_error(error).to_owned()),
        replayed: false,
    }
}

fn public_error(error: &str) -> &str {
    if error.starts_with("procurement")
        && error.len() <= 96
        && error.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        error
    } else {
        "procurementResultUnconfirmed"
    }
}

pub(super) fn outcome_unknown(error: &str) -> bool {
    error.contains("PROCUREMENT_COMMIT_OUTCOME_UNKNOWN")
        || error.contains("procurementOwnerPublicationOutcomeUnknown")
        || error.contains("procurementResultUnconfirmed")
        || public_error(error) == "procurementResultUnconfirmed"
}

fn validate_request(request: &ProcurementRequest) -> Result<(), String> {
    if request.protocol_version != PROTOCOL_VERSION
        || request.character_index < 0
        || request.request_id.is_empty()
        || request.request_id.len() > 96
        || !request
            .request_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b))
    {
        return Err("procurementRequestInvalid".into());
    }
    Ok(())
}

pub(super) async fn socket_action(
    session: &mut GatewaySession,
    identity: &IdentityService,
    verified: Option<&VerifiedIdentitySession>,
    authenticated: bool,
    account: Option<&str>,
    request: &ProcurementRequest,
    sender: &SharedWebSocketSender,
    cache: &SharedGatewaySessionCache,
) -> Result<(), String> {
    // All owner work completes in this socket's serial execution turn. There
    // is no detached callback that could mutate another login generation.
    let result = tokio::task::block_in_place(|| {
        validate_request(request)?;
        let account = account
            .filter(|_| authenticated)
            .ok_or("procurementAuthenticationRequired")?;
        let verified = verified
            .filter(|v| v.account_id == account && v.expires_at_ms > gateway_unix_ms())
            .ok_or("procurementAuthenticationRequired")?;
        identity
            .touch_session(verified)
            .map_err(|_| "procurementAuthenticationRequired")?;
        let active = session
            .active_identity()
            .ok_or("procurementCharacterNotInWorld")?;
        if active.account_id != account || active.character_index != request.character_index {
            return Err("procurementCharacterMismatch".into());
        }
        let bucket = format!("procurement:{account}");
        if cache.record_auth_attempt(&bucket, 60)?.0 > 120 {
            return Err("procurementTooManyRequests".into());
        }
        session.execute_procurement_owner(true, request, gateway_unix_ms())
    });
    let (reply, close) = match result {
        Ok(execution) => {
            for packet in execution.packets {
                send_server_packet(sender, &packet)
                    .await
                    .map_err(|_| "procurementSendFailed")?;
            }
            let close = execution
                .reply
                .error
                .as_deref()
                .is_some_and(outcome_unknown);
            // Native inventory and the normal wallet HUD are projected from
            // world snapshots. Packet-only procurement results leave their
            // presentation stale even when custody has already committed.
            if !close {
                send_world_snapshot(sender, session)
                    .await
                    .map_err(|_| "procurementSendFailed")?;
            }
            (execution.reply, close)
        }
        Err(error) => (empty_reply(request, &error), outcome_unknown(&error)),
    };
    sender
        .send(Message::Text(
            json!({"type":"procurement","payload":reply})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|_| "procurementSendFailed")?;
    if close {
        Err("procurementResultUnconfirmed".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::procurement::ProcurementAction;
    fn request() -> ProcurementRequest {
        ProcurementRequest {
            protocol_version: 1,
            request_id: "order-1:retry.2".into(),
            character_index: 0,
            action: ProcurementAction::List {
                template_id: Some(245),
            },
        }
    }
    #[test]
    fn request_has_bounded_stable_identity_and_version() {
        assert!(validate_request(&request()).is_ok());
        for id in ["".to_owned(), "a".repeat(97), "\nsecret".to_owned()] {
            let mut r = request();
            r.request_id = id;
            assert!(validate_request(&r).is_err());
        }
        let mut r = request();
        r.protocol_version = 2;
        assert!(validate_request(&r).is_err());
        r = request();
        r.character_index = -1;
        assert!(validate_request(&r).is_err());
    }
    #[test]
    fn unknown_commit_closes_owner_and_hides_internal_error() {
        assert!(outcome_unknown(
            "PROCUREMENT_COMMIT_OUTCOME_UNKNOWN: lost PG reply"
        ));
        assert!(outcome_unknown("procurementOwnerPublicationOutcomeUnknown"));
        assert!(!outcome_unknown("procurementInsufficientGold"));
        assert_eq!(
            public_error("postgres secret dsn"),
            "procurementResultUnconfirmed"
        );
        assert_eq!(
            public_error("procurementInsufficientGold"),
            "procurementInsufficientGold"
        );
        assert_eq!(
            public_error("procurement: private payload"),
            "procurementResultUnconfirmed"
        );
    }
    #[test]
    fn client_payload_cannot_supply_an_account_or_finality_receipt() {
        let mut value = serde_json::to_value(request()).unwrap();
        value["accountId"] = json!("victim");
        assert!(serde_json::from_value::<ProcurementRequest>(value).is_err());
        let mut value = serde_json::to_value(request()).unwrap();
        value["action"] = json!({"kind":"applyChainReceipt","status":"filled"});
        assert!(serde_json::from_value::<ProcurementRequest>(value).is_err());
    }
}
