//! Android connection lifetime and bounded FIFO for received public chat.
//! No authentication, sender authorization, local echo or channel-filter rules.
use mir2_client_bevy::native_chat_ingress::transform_chat_line;
use serde_json::Value;
use std::collections::VecDeque;

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_PENDING_LINES: usize = 32;
const MAX_PENDING_BYTES: usize = 128 * 1024;

#[derive(Default)]
pub(crate) struct AndroidChatIngress {
    identity: Option<(u32, String)>,
    pending: VecDeque<String>,
    pending_bytes: usize,
}

impl AndroidChatIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Bind only the already validated personal snapshot, never an ObjectChat
    /// sender. A different character requires a session/data-reset boundary so
    /// the runtime cannot retain the previous character's delivered history.
    pub(crate) fn bind(&mut self, owner: u32, name: &str) -> Result<(), &'static str> {
        if owner == 0 || name.trim().is_empty() || name.chars().count() > 128 {
            return Err("Invalid chat owner");
        }
        if self
            .identity
            .as_ref()
            .is_some_and(|(old_owner, old_name)| *old_owner != owner || old_name != name)
        {
            return Err("Chat character changed without session boundary");
        }
        self.identity = Some((owner, name.to_owned()));
        Ok(())
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        if raw.len() > MAX_PACKET_BYTES {
            return Err("Chat packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid chat packet")?;
        if event["type"] != "packet" || self.identity.is_none() {
            return Ok(false);
        }
        let Some(packet @ ("Chat" | "ObjectChat")) = event["packet"].as_str() else {
            return Ok(false);
        };
        if !event["payload"].is_object() {
            return Err("Invalid chat payload");
        }
        // Peer/AOI chat intentionally has a different objectId from the owner.
        // Use precisely the frozen Windows fields and shared channel semantics.
        let line = transform_chat_line(packet, &event["payload"]).ok_or("Invalid chat text")?;
        let json = serde_json::to_string(&line).map_err(|_| "Invalid chat line")?;
        if json.len() > MAX_PACKET_BYTES
            || self.pending.len() >= MAX_PENDING_LINES
            || self.pending_bytes.saturating_add(json.len()) > MAX_PENDING_BYTES
        {
            return Err("Chat queue overflow");
        }
        self.pending_bytes += json.len();
        self.pending.push_back(json);
        Ok(true)
    }

    pub(crate) fn flush(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        while let Some(front) = self.pending.front() {
            if !push(front.clone()) {
                return false;
            }
            self.pending_bytes -= self.pending.pop_front().unwrap().len();
        }
        true
    }
}

#[cfg(test)]
#[path = "chat_ingress_tests.rs"]
mod tests;
