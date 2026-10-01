//! Renderer-neutral received chat projection shared by native hosts.
//! Extracted from frozen Windows 3f5e615 without changing packet field or
//! channel semantics. Authentication, admission and delivery belong to hosts;
//! filtering, history, scrolling and styling belong to the existing shared UI.
use serde_json::Value;

/// Crystal selects `message` for Chat and `text` for ObjectChat. Do not accept
/// a similarly named fallback, trim/local-echo the message or invent a sender.
pub fn transform_chat_line(packet: &str, payload: &Value) -> Option<crate::chat::ChatLine> {
    let text_field = match packet {
        "Chat" => "message",
        "ObjectChat" => "text",
        _ => return None,
    };
    let text = payload
        .get(text_field)
        .and_then(Value::as_str)
        .map(str::to_owned)?;
    let channel = payload
        .get("chatType")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "normal".to_owned());
    Some(crate::chat::ChatLine { text, channel })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exact_kind_selects_the_authoritative_text_without_fallback() {
        for (packet, field, other) in [
            ("Chat", "message", "text"),
            ("ObjectChat", "text", "message"),
        ] {
            let payload = json!({field:"  邻居 👋\nhello  ",other:"wrong", "chatType":"System2"});
            let line = transform_chat_line(packet, &payload).unwrap();
            assert_eq!(line.text, "  邻居 👋\nhello  ");
            assert_eq!(line.channel, "System2");
            assert!(transform_chat_line(packet, &json!({other:"not authoritative"})).is_none());
            for text in [Value::Null, json!(42), json!(true), json!([]), json!({})] {
                assert!(transform_chat_line(packet, &json!({field:text})).is_none());
            }
        }
        assert!(transform_chat_line("NPCSay", &json!({"message":"not chat"})).is_none());
        assert!(transform_chat_line("chat", &json!({"message":"wrong case"})).is_none());
    }

    #[test]
    fn channel_default_and_empty_messages_are_exact_windows_semantics() {
        for chat_type in [Value::Null, json!(3), json!(false), json!([])] {
            let line =
                transform_chat_line("Chat", &json!({"message":"", "chatType":chat_type})).unwrap();
            assert_eq!(line.text, "");
            assert_eq!(line.channel, "normal");
        }
        for channel in ["", "Normal", "Guild", "WhisperIn", "Unlisted"] {
            let line =
                transform_chat_line("ObjectChat", &json!({"text":"hello", "chatType":channel}))
                    .unwrap();
            assert_eq!(line.channel, channel);
        }
    }
}
