//! Local compose validation only. Delivery, postage, inventory and wallets remain server-owned.
pub const MESSAGE_UTF16_LIMIT: usize = 500;
pub const MAX_ATTACHMENTS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailComposeError {
    MessageTooLong,
    RecipientAndMessageRequired,
    InvalidAttachments,
}

impl MailComposeError {
    pub fn message(self) -> &'static str {
        match self {
            Self::MessageTooLong => "Mail message exceeds 500 UTF-16 units; draft kept",
            Self::RecipientAndMessageRequired => "Recipient and message are required",
            Self::InvalidAttachments => "Invalid mail attachment selection",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailSendPayload {
    pub recipient: String,
    pub message: String,
    pub gold: u32,
    pub attachment_unique_ids: Vec<u64>,
}

/// Native host proof only. These types are independent of wire DTOs and the
/// existing Web validation API above.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MailSendToken(u64);
impl MailSendToken { pub fn value(self) -> u64 { self.0 } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MailSendStream { pub run: u64, pub connection: u64 }
impl MailSendStream { pub fn is_valid(self) -> bool { self.run != 0 && self.connection != 0 } }

/// A complete original UI draft, distinct from the normalized wire payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailSendDraft {
    pub recipient: String, pub message: String, pub gold: u32,
    pub attachment_unique_ids: Vec<u64>, pub stamped: bool, pub parcel: bool,
    pub generation: u64,
}

/// One checked clock shared by reducer, editor commits and manual controls.
/// Zero permanently means exhausted. Ordinary resource clears retain this
/// handle; neither returning to equal text nor a new stream can revive it.
#[derive(Clone, Debug)]
pub struct MailDraftClock(std::sync::Arc<std::sync::atomic::AtomicU64>);
impl Default for MailDraftClock {
    fn default() -> Self { Self(std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1))) }
}
impl PartialEq for MailDraftClock { fn eq(&self, other: &Self) -> bool { self.generation() == other.generation() } }
impl Eq for MailDraftClock {}
impl MailDraftClock {
    /// Controlled restoration/fixture seam. Native production starts with
    /// Default and retains that handle across clears; no UI action seeds it.
    #[doc(hidden)]
    pub fn from_highwater(value:u64)->Self{Self(std::sync::Arc::new(std::sync::atomic::AtomicU64::new(value)))}
    pub fn generation(&self) -> Option<u64> {
        let value = self.0.load(std::sync::atomic::Ordering::SeqCst);
        (value != 0).then_some(value)
    }
    pub fn advance(&self) -> Option<u64> {
        use std::sync::atomic::Ordering;
        let previous = self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst,
            |value| Some(if value == 0 { 0 } else { value.checked_add(1).unwrap_or(0) })).ok()?;
        previous.checked_add(1).filter(|_| previous != 0)
    }
    #[cfg(test)]
    fn at(value: u64) -> Self { Self::from_highwater(value) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailSendFlight<T> {
    pub token: MailSendToken, pub stream: MailSendStream, pub owner: u64,
    pub entered: bool, pub retired: bool, pub value: T,
}
/// Uncorrelated Crystal ACKs require one occupied slot, including a queued
/// prepublished command and a retired entered tombstone. No timeout frees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailSendSlot<T> {
    stream: Option<MailSendStream>, next_token: Option<u64>, flight: Option<MailSendFlight<T>>,
}
impl<T> Default for MailSendSlot<T> {
    fn default() -> Self { Self { stream: None, next_token: Some(1), flight: None } }
}
impl<T> MailSendSlot<T> {
    pub fn flight(&self) -> Option<&MailSendFlight<T>> { self.flight.as_ref() }
    pub fn flight_mut(&mut self) -> Option<&mut MailSendFlight<T>> { self.flight.as_mut() }
    pub fn start_stream(&mut self, stream: MailSendStream) -> bool {
        if !stream.is_valid() || self.stream.is_some_and(|current| stream < current) { return false; }
        if self.stream != Some(stream) { self.stream = Some(stream); self.flight = None; }
        true
    }
    pub fn reserve(&mut self, stream: MailSendStream, owner: u64, value: T) -> Option<MailSendToken> {
        if self.stream != Some(stream) || self.flight.is_some() { return None; }
        let token = MailSendToken(self.next_token?);
        self.next_token = token.0.checked_add(1);
        self.flight = Some(MailSendFlight { token, stream, owner, entered: false, retired: false, value });
        Some(token)
    }
    pub fn retire_owner(&mut self, owner: u64) {
        if let Some(flight) = self.flight.as_mut() { if flight.owner != owner { flight.retired = true; } }
    }
    pub fn enter(&mut self, token: MailSendToken) -> bool {
        let Some(flight) = self.flight.as_mut() else { return false; };
        if flight.token != token { return false; }
        flight.entered = true; true
    }
    pub fn cancel_unsent(&mut self, token: MailSendToken) -> Option<MailSendFlight<T>> {
        if !self.flight.as_ref().is_some_and(|flight| flight.token == token && !flight.entered) { return None; }
        self.flight.take()
    }
    pub fn acknowledge(&mut self, token: MailSendToken, result: i32) -> Option<MailSendFlight<T>> {
        if !matches!(result, 1 | -1) || !self.flight.as_ref().is_some_and(|flight| flight.token == token && flight.entered) { return None; }
        self.flight.take()
    }
}

/// Match FriendTextEditor multiline filtering. Reject the whole edit on overflow;
/// never manufacture a fitting substring or split a Unicode grapheme.
pub fn normalize_message(message: &str) -> Result<String, MailComposeError> {
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let clean: String = normalized
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    if clean.encode_utf16().count() > MESSAGE_UTF16_LIMIT {
        Err(MailComposeError::MessageTooLong)
    } else {
        Ok(clean)
    }
}

pub fn prepare_send(
    recipient: &str,
    message: &str,
    gold: u32,
    attachments: &[u64],
) -> Result<MailSendPayload, MailComposeError> {
    let message = normalize_message(message)?;
    let recipient = recipient.trim();
    let message = message.trim();
    if recipient.is_empty() || message.is_empty() {
        return Err(MailComposeError::RecipientAndMessageRequired);
    }
    if attachments.len() > MAX_ATTACHMENTS
        || attachments.iter().enumerate().any(|(index, id)| {
            *id == 0 || attachments[..index].contains(id)
        })
    {
        return Err(MailComposeError::InvalidAttachments);
    }
    Ok(MailSendPayload {
        recipient: recipient.to_owned(),
        message: message.to_owned(),
        gold,
        attachment_unique_ids: attachments.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_send_slot_retains_provisional_and_tombstone_until_exact_terminal() {
        let stream = MailSendStream { run: 1, connection: 1 };
        let mut slot = MailSendSlot::default();
        assert!(slot.reserve(stream, 0, "A").is_none());
        assert!(slot.start_stream(stream));
        let first = slot.reserve(stream, 1, "raw A").unwrap();
        slot.retire_owner(2);
        assert!(slot.start_stream(stream));
        assert!(slot.reserve(stream, 2, "B").is_none());
        assert!(slot.enter(first));
        assert!(slot.cancel_unsent(first).is_none());
        assert!(slot.acknowledge(first, 0).is_none());
        let old = slot.acknowledge(first, -1).unwrap();
        assert!(old.retired); assert_eq!(old.value, "raw A");
        let next = slot.reserve(stream, 2, "B").unwrap();
        assert_ne!(first, next);
        assert!(slot.acknowledge(first, 1).is_none());
        assert!(slot.cancel_unsent(first).is_none());
        assert!(slot.cancel_unsent(next).is_some());
        let third = slot.reserve(stream, 2, "C").unwrap();
        assert!(slot.start_stream(MailSendStream { run: 1, connection: 2 }));
        assert!(slot.acknowledge(third, 1).is_none());
        assert!(!slot.start_stream(stream));
    }

    #[test]
    fn mail_send_token_and_full_draft_clock_never_wrap_or_accept_aba() {
        let clock = MailDraftClock::default(); let alias = clock.clone();
        let a = clock.generation().unwrap();
        clock.advance(); alias.advance();
        assert_ne!(a, clock.generation().unwrap(), "A to B to A remains a new generation");
        let exhausted = MailDraftClock::at(u64::MAX);
        assert_eq!(exhausted.advance(), None); assert_eq!(exhausted.generation(), None);
        assert_eq!(exhausted.advance(), None);
        let stream = MailSendStream { run: 1, connection: 1 };
        let mut slot = MailSendSlot { stream: Some(stream), next_token: Some(u64::MAX), flight: None };
        let last = slot.reserve(stream, 1, ()).unwrap(); assert_eq!(last.value(), u64::MAX);
        assert!(slot.cancel_unsent(last).is_some());
        assert!(slot.reserve(stream, 1, ()).is_none());
        assert!(slot.start_stream(stream));assert!(slot.reserve(stream,1,()).is_none());
        let newer=MailSendStream{run:u64::MAX,connection:u64::MAX};
        assert!(slot.start_stream(newer));assert!(slot.reserve(newer,1,()).is_none(),"a new stream cannot reuse an exhausted local token allocator");
    }

    #[test]
    fn normalization_matches_multiline_editor_without_partial_overflow() {
        assert_eq!(normalize_message("a\r\nb\rc\t\0\n").unwrap(), "a\nb\nc\n");
        assert_eq!(normalize_message(&"😀".repeat(250)).unwrap(), "😀".repeat(250));
        for text in [format!("{}😀", "中".repeat(499)), "😀".repeat(251),
            format!("{}👩‍👩‍👧‍👦", "a".repeat(495)), format!("{}e\u{301}", "a".repeat(499))] {
            assert_eq!(normalize_message(&text), Err(MailComposeError::MessageTooLong));
        }
    }

    #[test]
    fn sends_keep_native_trim_and_gold_and_exact_id_order() {
        assert_eq!(prepare_send(" Receiver ", "\r\nhello\t世界 \r", u32::MAX, &[9, 7]).unwrap(),
            MailSendPayload {recipient:"Receiver".into(), message:"hello世界".into(),
                gold:u32::MAX, attachment_unique_ids:vec![9,7]});
        for ids in [vec![0], vec![7,7], vec![1,2,3,4,5,6]] {
            assert_eq!(prepare_send("R", "body", 0, &ids), Err(MailComposeError::InvalidAttachments));
        }
        assert_eq!(prepare_send(" R ", "\r\n\t", 0, &[]), Err(MailComposeError::RecipientAndMessageRequired));
        assert_eq!(prepare_send("R", &format!("{} ", "a".repeat(500)), 0, &[]), Err(MailComposeError::MessageTooLong));
    }
}
