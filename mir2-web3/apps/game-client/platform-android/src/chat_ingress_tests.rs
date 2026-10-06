use super::*;
use mir2_client_bevy::chat::{ChatLine, ChatModel};
use serde_json::json;

fn packet(kind: &str, payload: Value) -> String {
    json!({"type":"packet","packet":kind,"payload":payload}).to_string()
}
fn ready() -> AndroidChatIngress {
    let mut ingress = AndroidChatIngress::default();
    ingress.bind(42, "Fixture").unwrap();
    ingress
}
fn drain(ingress: &mut AndroidChatIngress) -> ChatModel {
    let mut model = ChatModel::default();
    assert!(ingress.flush(|raw| {
        model.push(serde_json::from_str(&raw).unwrap());
        true
    }));
    model
}

#[test]
fn received_peer_and_system_chat_enter_same_typed_model_in_wire_order() {
    let mut ingress = ready();
    for (kind, payload) in [
        (
            "ObjectChat",
            json!({"objectId":99,"text":"邻居: hello 👋", "chatType":"Normal", "message":"wrong"}),
        ),
        (
            "Chat",
            json!({"message":"server.CannotPickupNotOwner","chatType":"System2", "text":"wrong"}),
        ),
        ("Chat", json!({"message":"  unchanged  "})),
    ] {
        assert!(ingress.packet(&packet(kind, payload)).unwrap());
    }
    let model = drain(&mut ingress);
    assert_eq!(
        model.lines,
        vec![
            ChatLine {
                text: "邻居: hello 👋".into(),
                channel: "Normal".into()
            },
            ChatLine {
                text: "server.CannotPickupNotOwner".into(),
                channel: "System2".into()
            },
            ChatLine {
                text: "  unchanged  ".into(),
                channel: "normal".into()
            },
        ]
    );
    assert!(drain(&mut ingress).lines.is_empty());
    assert_eq!(ingress.pending_bytes, 0);
}

#[test]
fn only_recognized_received_packets_after_bootstrap_are_admitted() {
    let raw = packet("ObjectChat", json!({"objectId":99,"text":"peer"}));
    let mut ingress = AndroidChatIngress::default();
    assert!(!ingress.packet(&raw).unwrap());
    assert!(ingress.bind(0, "Fixture").is_err());
    assert!(ingress.bind(42, " ").is_err());
    ingress.bind(42, "Fixture").unwrap();
    for kind in ["NPCSay", "AdminChat", "Stage5Command", "chat"] {
        assert!(!ingress
            .packet(&packet(
                kind,
                json!({"text":"not chat", "message":"not chat"})
            ))
            .unwrap());
    }
    assert!(!ingress
        .packet(r#"{"type":"chat","message":"outbound command"}"#)
        .unwrap());
    assert!(ingress.packet(&raw).unwrap());
    assert_eq!(drain(&mut ingress).lines.len(), 1);
}

#[test]
fn retry_retains_unsent_fifo_without_replaying_the_accepted_prefix() {
    let mut ingress = ready();
    for text in ["first", "second", "third"] {
        ingress
            .packet(&packet("Chat", json!({"message":text})))
            .unwrap();
    }
    let original_bytes = ingress.pending_bytes;
    let mut accepted = vec![];
    assert!(!ingress.flush(|raw| {
        let line: ChatLine = serde_json::from_str(&raw).unwrap();
        if line.text == "second" {
            false
        } else {
            accepted.push(line.text);
            true
        }
    }));
    assert_eq!(accepted, ["first"]);
    assert!(ingress.pending_bytes < original_bytes);
    assert_eq!(
        drain(&mut ingress)
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
        ["second", "third"]
    );
    assert!(drain(&mut ingress).lines.is_empty());
}

#[test]
fn scene_rebind_preserves_history_but_character_switch_requires_reset() {
    let mut ingress = ready();
    let raw = packet("Chat", json!({"message":"unsent from same character"}));
    ingress.packet(&raw).unwrap();
    ingress.bind(42, "Fixture").unwrap();
    assert!(ingress.bind(43, "Other").is_err());
    assert!(ingress.bind(42, "Renamed").is_err());
    assert_eq!(ingress.pending.len(), 1);
    ingress.reset();
    assert!(drain(&mut ingress).lines.is_empty());
    assert!(!ingress.packet(&raw).unwrap());
    ingress.bind(43, "Other").unwrap();
    assert!(ingress
        .packet(&packet("Chat", json!({"message":"new session"})))
        .unwrap());
    assert_eq!(drain(&mut ingress).lines[0].text, "new session");
}

#[test]
fn malformed_or_oversized_chat_cannot_mutate_an_accepted_line() {
    let mut ingress = ready();
    ingress
        .packet(&packet("Chat", json!({"message":"retained"})))
        .unwrap();
    let prior_bytes = ingress.pending_bytes;
    for raw in [
        "bad json".to_owned(),
        packet("Chat", json!({"text":"wrong field"})),
        packet("ObjectChat", json!({"message":"wrong field"})),
        packet("Chat", json!({"message":4})),
        packet("Chat", json!([])),
        "x".repeat(MAX_PACKET_BYTES + 1),
    ] {
        assert!(ingress.packet(&raw).is_err());
        assert_eq!(ingress.pending.len(), 1);
        assert_eq!(ingress.pending_bytes, prior_bytes);
    }
    assert_eq!(drain(&mut ingress).lines[0].text, "retained");
}

#[test]
fn line_count_overflow_is_fail_closed_and_does_not_evict_oldest() {
    let mut ingress = ready();
    for index in 0..MAX_PENDING_LINES {
        ingress
            .packet(&packet("Chat", json!({"message":index.to_string()})))
            .unwrap();
    }
    assert!(ingress
        .packet(&packet("Chat", json!({"message":"overflow"})))
        .is_err());
    let model = drain(&mut ingress);
    assert_eq!(model.lines.len(), MAX_PENDING_LINES);
    assert_eq!(model.lines[0].text, "0");
    assert_eq!(model.lines[MAX_PENDING_LINES - 1].text, "31");
}

#[test]
fn byte_overflow_is_bounded_independently_of_line_count() {
    let mut ingress = ready();
    let raw = packet("Chat", json!({"message":"界".repeat(4000)}));
    let mut count = 0;
    while ingress.packet(&raw).is_ok() {
        count += 1;
    }
    assert!(count > 0 && count < MAX_PENDING_LINES);
    assert!(ingress.pending_bytes <= MAX_PENDING_BYTES);
    assert_eq!(ingress.pending.len(), count);
    ingress.reset();
    assert_eq!(ingress.pending_bytes, 0);
    assert!(ingress.pending.is_empty());
}

#[test]
fn identical_messages_are_distinct_server_lines_not_deduplicated_or_local_echoed() {
    let mut ingress = ready();
    let raw = packet(
        "ObjectChat",
        json!({"objectId":99,"text":"same message", "chatType":"Guild"}),
    );
    ingress.packet(&raw).unwrap();
    ingress.packet(&raw).unwrap();
    assert_eq!(drain(&mut ingress).lines.len(), 2);
}
